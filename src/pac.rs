use crate::utils;

use byteorder::{WriteBytesExt, LE};
use miniserde::{Deserialize, Serialize};

pub const HEADER_SIZE: usize = 0x20;
pub const HEADER_MAGIC: &[u8; 4] = b"FPAC";

// Type used for storing data about the FPAC in a meta.json to be serialized/deserialized
#[derive(Debug, Serialize, Deserialize)]
pub struct PacMeta {
    pub unknown: u32,
    pub file_entries: Vec<PacMetaEntry>,
}

impl PacMeta {
    pub fn new(unknown: u32) -> Self {
        Self {
            unknown,
            file_entries: Vec::new(),
        }
    }

    pub fn add_file_entry(&mut self, file_name: String, file_id: u32) {
        let entry = PacMetaEntry { file_name, file_id };

        self.file_entries.push(entry);
    }

    pub fn string_size(&self) -> Option<usize> {
        let max = self.file_entries.iter().map(|x| x.file_name.len()).max();

        if let Some(max_unaligned) = max {
            let string_size = utils::pad_to_nearest_with_excess(max_unaligned, 0x4);

            return Some(string_size);
        }

        None
    }

    pub fn entry_size(&self) -> Option<usize> {
        if let Some(string_size) = self.string_size() {
            let size_unaligned = string_size + 0xC;
            let single_entry_size = utils::pad_to_nearest(size_unaligned, 0x10);

            return Some(single_entry_size * self.file_entries.len());
        }

        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PacMetaEntry {
    pub file_name: String,
    pub file_id: u32,
}

const VEC_WRITE_ERR: &str = "Could not write u32 to entry Vec";
impl PacMetaEntry {
    pub fn to_entry_bytes(&self, offset: u32, file_size: u32, string_size: usize) -> Vec<u8> {
        let mut entry = utils::to_fixed_length(&self.file_name, string_size);
        entry.write_u32::<LE>(self.file_id).expect(VEC_WRITE_ERR);
        entry.write_u32::<LE>(offset).expect(VEC_WRITE_ERR);
        entry
            .write_u32::<LE>(file_size as u32)
            .expect(VEC_WRITE_ERR);

        let leftover_nulls = utils::needed_to_align_with_excess(entry.len(), 0x10);

        for _ in 0..leftover_nulls {
            entry.push(0x00);
        }

        entry
    }
}
