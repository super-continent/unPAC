mod utils;
mod pac;
mod parser;

use pac::PacMeta;

use std::io::{BufWriter, prelude::*};
use std::path::PathBuf;
use std::{collections::HashMap, fs};
use std::{fs::File, io::Write};

use anyhow::anyhow;
use anyhow::Result as AResult;
use byteorder::{WriteBytesExt, LE};
use miniserde::json;
use structopt::StructOpt;

const META_FILENAME: &str = "meta.json";

#[derive(StructOpt, Debug)]
#[structopt(name = "unPAC", author = "Made by Pangaea")]
enum Run {
    /// Parse a .pac file into its contained parts
    Parse {
        /// Path to the input .pac file
        #[structopt(parse(from_os_str))]
        input: PathBuf,
        #[structopt(parse(from_os_str))]
        output: PathBuf,
        /// Specify the program should overwrite the output path if it already exists
        #[structopt(short, long)]
        overwrite: bool,
    },
    /// Rebuild a parsed .pac file into its original format
    Rebuild {
        /// Path to the input folder containing the parsed .pac files and a meta.json
        #[structopt(parse(from_os_str))]
        input: PathBuf,
        #[structopt(parse(from_os_str))]
        output: PathBuf,
        /// Specify the program should overwrite the output path if it already exists
        #[structopt(short, long)]
        overwrite: bool,
    },
}

fn main() -> AResult<()> {
    if let Err(e) = run() {
        println!("ERROR: {}", e.root_cause());
    }

    Ok(())
}

fn run() -> AResult<()> {
    let opt = Run::from_args();

    match opt {
        Run::Parse {
            input,
            output,
            overwrite,
        } => parse_fpac(input, output, overwrite)?,
        Run::Rebuild {
            input,
            output,
            overwrite,
        } => rebuild_fpac(input, output, overwrite)?,
    }

    Ok(())
}

fn parse_fpac(input: PathBuf, output_path: PathBuf, overwrite: bool) -> AResult<()> {
    if output_path.exists() && !overwrite {
        return Err(anyhow!(
            "Output `{}` already exists! You can overwrite with -o",
            output_path.to_string_lossy()
        ));
    }

    if !input.is_file() {
        return Err(anyhow!(
            "Input `{}`, does not exist!",
            input.to_string_lossy()
        ));
    }

    println!("Reading file...");
    let mut in_file = File::open(&input)?;
    
    let mut file_data = Vec::new();
    in_file.read_to_end(&mut file_data)?;

    let (meta, named_files) = match parser::parse(&file_data) {
        Ok((_, o)) => o,
        Err(e) => return Err(anyhow!("Parsing file failed: `{}`", e.to_string()))
    };

    println!(
        "Parsed FPAC `{}`\nWriting {} files...",
        &input.to_string_lossy(),
        meta.file_entries.len()
    );

    fs::create_dir_all(&output_path)?;

    for file in named_files {
        let file_name = file.name;

        let mut file_path = output_path.clone();
        file_path.push(&file_name);

        let mut out_file = File::create(file_path)?;
        out_file.write_all(&file.contents)?;
    }

    let serialized = json::to_string(&meta);
    let mut meta_file = File::create(output_path.join(META_FILENAME))?;
    meta_file.write(serialized.as_bytes())?;

    Ok(())
}

fn rebuild_fpac(input: PathBuf, output: PathBuf, overwrite: bool) -> AResult<()> {
    if !input.is_dir() {
        return Err(anyhow!(
            "Input directory `{}` does not exist!",
            input.to_string_lossy()
        ));
    }

    if output.exists() && !overwrite {
        return Err(anyhow!(
            "Output `{}` already exists! You can overwrite with -o",
            output.to_string_lossy()
        ));
    }

    let mut meta_file = File::open(input.join(META_FILENAME))?;
    let mut meta_contents = String::new();

    meta_file.read_to_string(&mut meta_contents)?;

    let mut meta = json::from_str::<PacMeta>(&meta_contents)?;
    meta.file_entries.sort_by(|x, y| x.file_id.cmp(&y.file_id));

    if meta.string_size() == None {
        return Err(anyhow!("FPAC metadata has no file entries!"));
    }

    ////// Collect metadata and file data //////

    let meta_data_start = pac::HEADER_SIZE + meta.entry_size().unwrap();
    let meta_entry_count = meta.file_entries.len();
    let meta_unknown = meta.unknown;
    let meta_string_size = meta.string_size().unwrap();

    // Actual data contained in the pac that the entries point to
    let mut data: Vec<u8> = Vec::new();
    let mut id_offsets_sizes = HashMap::new();

    for file_entry in &meta.file_entries {
        let file_path = input.join(&file_entry.file_name);

        let offset = data.len();

        let mut file = File::open(file_path)?;
        file.read_to_end(&mut data)?;

        let size = data.len() - offset;

        let leftover = utils::needed_to_align(size, 0x10);

        for _ in 0..leftover {
            data.write_u8(0x0)?;
        }

        // Store calculated file entry offsets and sizes for later when rebuilding the FPAC entries
        id_offsets_sizes.insert(file_entry.file_id, (offset as u32, size as u32));
    }

    let total_size = meta_data_start + data.len();

    ////// Assemble the data back into a single FPAC //////
    let mut fpac = BufWriter::new(File::create(output)?);

    // Write the headers to the fpac
    // contents:
    // 00 magic b"FPAC"
    // 04 data start offset
    // 08 total size
    // 0C files contained total
    // 10 unknown
    // 14 string size
    // 18..20 null padding
    // 20...N file entries

    fpac.write(pac::HEADER_MAGIC)?;
    fpac.write_u32::<LE>(meta_data_start as u32)?;
    fpac.write_u32::<LE>(total_size as u32)?;
    fpac.write_u32::<LE>(meta_entry_count as u32)?;
    fpac.write_u32::<LE>(meta_unknown)?;
    fpac.write_u32::<LE>(meta_string_size as u32)?;
    fpac.write_u64::<LE>(0x00)?;

    for entry in &meta.file_entries {
        if let Some((offset, size)) = id_offsets_sizes.get(&entry.file_id) {
            let mut entry_bytes = entry.to_entry_bytes(*offset, *size, meta_string_size);
            fpac.write(&mut entry_bytes)?;
        }
    }

    fpac.write_all(&mut data)?;

    Ok(())
}