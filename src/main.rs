use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

use anyhow::Result as AResult;
use arcsys::bbcf::hip::{BBCFHip, BBCFHipImage};
use arcsys::bbcf::hpl::BBCFHpl;
use arcsys::bbcf::pac::{BBCFPac, BBCFPacEntry};
use arcsys::{IndexedImage, RGBAColor};
use image::{DynamicImage, GenericImageView, GrayImage, RgbaImage};
use rayon::prelude::*;
use structopt::StructOpt;

const META_FILENAME: &str = "meta.json";

#[derive(StructOpt, Debug)]
#[structopt(name = "unPAC")]
struct Run {
    input_files: Vec<PathBuf>,
}

fn main() {
    if let Err(e) = run() {
        println!("ERROR: {}", e.to_string());
    }
}

fn run() -> AResult<()> {
    let opt = Run::from_args();

    println!("unPAC - Written by Pangaea");

    let input_files: Vec<PathBuf> = opt.input_files;

    input_files.into_par_iter().for_each(|path| {
        if path.is_file() {
            let mut file_buf = Vec::new();
            if let Err(e) = File::open(&path).and_then(|mut f| f.read_to_end(&mut file_buf)) {
                println!("Error reading file {}: {}", path.display(), e);
                return;
            };

            let res = match path.extension().map(|e| e.to_str()).flatten() {
                Some("pac") => handle_pac(file_buf, path.with_extension("")),
                Some("hip") => handle_hip(file_buf, path.with_extension("")),
                Some("hpl") => handle_hpl(file_buf, path.with_extension("")),
                _ => Err(anyhow::anyhow!(
                    "File either has no extension or is unrecognized"
                )),
            };

            if let Err(e) = res {
                println!("Error extracting {}:", path.display());
                println!("{}", e);
            }
        } else if path.is_dir() {
            if let Err(e) = repack_dir(path) {
                println!("Error: {}", e)
            };
        }
    });

    println!("Done!");
    pause();

    Ok(())
}

fn pause() {
    println!("Press enter to exit...");
    std::io::stdin().read(&mut []).unwrap();
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum MetaKind {
    Pac(BBCFPac),
    Hip(BBCFHip),
    Hpl(BBCFHpl),
}

fn repack_dir(path: PathBuf) -> AResult<()> {
    let mut meta_reader = BufReader::new(File::open(path.join(META_FILENAME))?);

    let meta: MetaKind = serde_json::from_reader(&mut meta_reader)?;

    match meta {
        MetaKind::Pac(mut pac) => {
            pac.files = pac
                .files
                .into_iter()
                .filter_map(|mut entry| {
                    let mut contents = Vec::new();
                    if File::open(path.join(&entry.name))
                        .and_then(|mut f| f.read_to_end(&mut contents))
                        .is_ok()
                    {
                        entry.contents = contents;
                        Some(entry)
                    } else {
                        println!("Failed to read {}! Excluding from PAC file", entry.name);
                        None
                    }
                })
                .collect::<Vec<BBCFPacEntry>>();

            let compressed = pac.to_bytes_compressed();

            write_repacked_file(&path, compressed, "pac")?;
        }
        MetaKind::Hpl(mut hpl) => {
            let palette: Vec<RGBAColor> = image::open(path.join("palette.png"))?
                .pixels()
                .map(|(_, _, c)| {
                    let color = c.0;
                    RGBAColor {
                        red: color[0] as u8,
                        green: color[1] as u8,
                        blue: color[2] as u8,
                        alpha: color[3] as u8,
                    }
                })
                .collect();

            hpl.palette = palette;

            let bytes = hpl.to_bytes();
            write_repacked_file(&path, bytes, "hpl")?;
        }
        MetaKind::Hip(mut hip) => {
            hip.image = match hip.image {
                BBCFHipImage::Indexed {
                    width: _,
                    height: _,
                    data: _,
                } => {
                    let image = image::open(path.join("image.png"))?;
                    let palette = image::open(path.join("palette.png"))?;

                    let (width, height) = image.dimensions();

                    let palette: Vec<RGBAColor> = palette
                        .pixels()
                        .map(|(_, _, c)| {
                            let color = c.0;
                            RGBAColor {
                                red: color[0] as u8,
                                green: color[1] as u8,
                                blue: color[2] as u8,
                                alpha: color[3] as u8,
                            }
                        })
                        .collect();

                    let image = image.to_luma8().to_vec();
                    BBCFHipImage::Indexed {
                        width,
                        height,
                        data: IndexedImage { palette, image },
                    }
                }
                BBCFHipImage::Raw {
                    width: _,
                    height: _,
                    data: _,
                } => {
                    let image = image::open(path.join("image.png"))?;

                    let (width, height) = image.dimensions();

                    let image: Vec<RGBAColor> = image
                        .pixels()
                        .map(|(_, _, c)| {
                            let color = c.0;
                            RGBAColor {
                                red: color[0] as u8,
                                green: color[1] as u8,
                                blue: color[2] as u8,
                                alpha: color[3] as u8,
                            }
                        })
                        .collect();

                    BBCFHipImage::Raw {
                        width,
                        height,
                        data: image,
                    }
                }
            };

            let bytes = hip.to_bytes();
            write_repacked_file(&path, bytes, "hip")?;
        }
    }

    Ok(())
}

fn write_repacked_file(
    path: &PathBuf,
    bytes: Vec<u8>,
    extension: &str,
) -> Result<(), anyhow::Error> {
    let write_path = path.with_extension(extension);
    if write_path.exists() {
        println!(
            "{} is being overwritten!",
            write_path.file_name().unwrap().to_string_lossy()
        )
    }
    File::create(write_path)?.write_all(&bytes)?;
    Ok(())
}

fn handle_pac(input: Vec<u8>, storage_folder: PathBuf) -> AResult<()> {
    use arcsys::bbcf::pac::*;

    let mut pac = BBCFPac::parse(&input)?;

    std::fs::create_dir_all(&storage_folder)?;

    for i in &mut pac.files {
        let mut content_file = File::create(storage_folder.join(&i.name))?;
        content_file.write_all(&mut i.contents)?;
    }

    let meta_file = File::create(storage_folder.join(META_FILENAME))?;
    let mut serializer = serde_json::Serializer::new(meta_file);

    let meta = MetaKind::Pac(pac);
    meta.serialize(&mut serializer)?;

    Ok(())
}

fn handle_hpl(input: Vec<u8>, storage_folder: PathBuf) -> AResult<()> {
    use arcsys::bbcf::hpl::*;

    let mut hpl = BBCFHpl::parse(&input)?;

    let width = hpl.palette.len();
    let palette = raw_to_rgba(hpl.palette, width as u32, 1);

    // replace moved palette with empty vec
    hpl.palette = Vec::new();

    let hpl = MetaKind::Hpl(hpl);

    std::fs::create_dir_all(&storage_folder)?;

    palette.save_with_format(storage_folder.join("palette.png"), image::ImageFormat::Png)?;

    let meta_file = File::create(storage_folder.join(META_FILENAME))?;
    let mut serializer = serde_json::Serializer::new(meta_file);

    hpl.serialize(&mut serializer)?;

    Ok(())
}

fn handle_hip(input: Vec<u8>, storage_folder: PathBuf) -> AResult<()> {
    use arcsys::bbcf::hip::*;

    let hip = BBCFHip::parse(&input)?;

    let image = hip_to_image(hip.image.clone());

    std::fs::create_dir_all(&storage_folder)?;

    if let BBCFHipImage::Indexed {
        width: _,
        height: _,
        data,
    } = &hip.image
    {
        let palette = palette_to_image(&data.palette);
        palette.save_with_format(storage_folder.join("palette.png"), image::ImageFormat::Png)?;
    }

    image.save_with_format(storage_folder.join("image.png"), image::ImageFormat::Png)?;
    let meta = MetaKind::Hip(hip);

    let meta_file = File::create(storage_folder.join(META_FILENAME))?;

    let mut serializer = serde_json::Serializer::new(meta_file);

    meta.serialize(&mut serializer)?;

    Ok(())
}

fn hip_to_image(hip: BBCFHipImage) -> DynamicImage {
    match hip {
        BBCFHipImage::Indexed {
            width,
            height,
            data,
        } => DynamicImage::ImageLuma8(indexed_to_luma(data.image, width, height)),
        BBCFHipImage::Raw {
            width,
            height,
            data,
        } => DynamicImage::ImageRgba8(raw_to_rgba(data, width, height)),
    }
}

fn raw_to_rgba(raw: Vec<RGBAColor>, width: u32, height: u32) -> RgbaImage {
    let pixels: Vec<u8> = raw.into_iter().flat_map(|c| c.to_rgba_slice()).collect();

    RgbaImage::from_vec(width, height, pixels).unwrap()
}

fn indexed_to_luma(pixels: Vec<u8>, width: u32, height: u32) -> GrayImage {
    GrayImage::from_vec(width, height, pixels).unwrap()
}

fn palette_to_image(palette: &Vec<RGBAColor>) -> DynamicImage {
    let width = palette.len();
    let pixels: Vec<u8> = palette
        .into_iter()
        .flat_map(|c| c.to_rgba_slice())
        .collect();

    DynamicImage::ImageRgba8(RgbaImage::from_vec(width as u32, 1, pixels).unwrap())
}
