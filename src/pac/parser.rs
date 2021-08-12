use std::str;

use nom::combinator::map_res;
use nom::{
    bytes::complete::{take, take_until},
    combinator,
    number::complete::le_u32,
    IResult,
};
use utils::needed_to_align;

use crate::{error::PacError, pac::PacMeta, utils};

use super::ParsedPac;

pub fn parse(i: &[u8]) -> Result<ParsedPac, nom::Err<PacError>> {
    let original_input = <&[u8]>::clone(&i);
    let (i, _) = nom::bytes::complete::tag(b"FPAC")(i)?;

    let (i, data_start) = le_u32(i)?;
    let (i, _total_size) = le_u32(i)?;
    let (i, file_count) = combinator::verify(le_u32, |x| *x > 0)(i)?;
    let (i, unknown) = le_u32(i)?;
    let (i, string_size) = le_u32(i)?;

    // padding
    let (i, _) = take(8u8)(i)?;

    let (_, entries): (_, Vec<FileEntry>) =
        nom::multi::count(|i| parse_entry(i, string_size), file_count as usize)(i)
            .map_err(|_e| nom::Err::Error(PacError::FileEntry))?;

    let mut data = &original_input[data_start as usize..];

    let mut pac_meta = PacMeta::new(unknown);
    let mut file_contents = Vec::new();
    for entry in entries {
        let (new_data_slice, file_data) = take(entry.size)(data)?;
        let (new_data_slice, _) = take(needed_to_align(entry.size as usize, 0x10))(new_data_slice)?;
        let entry_name = entry.name.to_string();

        data = new_data_slice;

        pac_meta.add_file_entry(entry_name.clone(), entry.id);

        file_contents.push(NamedFile {
            name: entry_name,
            contents: Vec::from(file_data),
        })
    }

    Ok(
        ParsedPac {
            meta: pac_meta,
            files: file_contents
        })
}

fn parse_entry(i: &[u8], string_size: u32) -> IResult<&[u8], FileEntry> {
    let (i, file_name) = take_str_of_size(i, string_size)?;
    let (i, id) = le_u32(i)?;
    let (i, offset) = le_u32(i)?;
    let (i, size) = le_u32(i)?;

    let needed_padding = utils::needed_to_align_with_excess((string_size + 0xC) as usize, 0x10);
    let (i, _) = take(needed_padding)(i)?;

    let file_entry = FileEntry {
        name: file_name.to_string(),
        id,
        offset,
        size,
    };
    Ok((i, file_entry))
}

fn take_str_of_size(i: &[u8], size: u32) -> IResult<&[u8], &str> {
    let (i, bytes) = take(size)(i)?;
    let (_, parsed_string) = map_res(take_until("\0"), str::from_utf8)(bytes)?;

    Ok((i, parsed_string))
}

#[derive(Debug)]
struct FileEntry {
    name: String,
    id: u32,
    offset: u32,
    size: u32,
}

#[derive(Debug)]
pub struct NamedFile {
    pub name: String,
    pub contents: Vec<u8>,
}
