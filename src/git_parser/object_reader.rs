use flate2::read::ZlibDecoder;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::models::git_object::{GitObject, GitObjectType, GitVizError, Result};

pub struct ObjectReader {
    objects_dir: PathBuf,
    pack_dir: PathBuf,
}

impl ObjectReader {
    pub fn new(git_dir: &Path) -> Self {
        ObjectReader {
            objects_dir: git_dir.join("objects"),
            pack_dir: git_dir.join("objects").join("pack"),
        }
    }

    pub fn read_object(&self, hash: &str) -> Result<GitObject> {
        if hash.len() < 4 {
            return Err(GitVizError::InvalidFormat(format!(
                "hash too short: {}",
                hash
            )));
        }

        let (prefix, suffix) = hash.split_at(2);
        let loose_path = self.objects_dir.join(prefix).join(suffix);

        if loose_path.exists() {
            return self.read_loose_object(&loose_path, hash);
        }

        self.read_packed_object(hash)
    }

    fn read_loose_object(&self, path: &Path, hash: &str) -> Result<GitObject> {
        let compressed = std::fs::read(path)?;
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| GitVizError::Decompression(e.to_string()))?;

        let header_end = decompressed
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| GitVizError::InvalidFormat("no null byte in object header".into()))?;

        let header = String::from_utf8_lossy(&decompressed[..header_end]);
        let mut header_parts = header.split(' ');

        let type_str = header_parts
            .next()
            .ok_or_else(|| GitVizError::InvalidFormat("missing object type".into()))?;
        let size_str = header_parts
            .next()
            .ok_or_else(|| GitVizError::InvalidFormat("missing object size".into()))?;

        let obj_type: GitObjectType = type_str
            .parse()
            .map_err(|_| GitVizError::InvalidFormat(format!("unknown type: {}", type_str)))?;
        let size: usize = size_str
            .parse()
            .map_err(|_| GitVizError::InvalidFormat(format!("invalid size: {}", size_str)))?;

        let content = decompressed[header_end + 1..].to_vec();

        Ok(GitObject {
            obj_type,
            hash: hash.to_string(),
            size,
            content,
        })
    }

    fn read_packed_object(&self, hash: &str) -> Result<GitObject> {
        let hex_hash = hex_to_uint(hash)?;

        if !self.pack_dir.exists() {
            return Err(GitVizError::ObjectNotFound(hash.to_string()));
        }

        for entry in std::fs::read_dir(&self.pack_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "idx").unwrap_or(false) {
                if let Some(idx_path) = path.to_str() {
                    let pack_path = idx_path.replace(".idx", ".pack");
                    if let Ok(Some(obj)) =
                        self.search_pack_file(&pack_path, idx_path, hex_hash, hash)
                    {
                        return Ok(obj);
                    }
                }
            }
        }

        Err(GitVizError::ObjectNotFound(hash.to_string()))
    }

    fn search_pack_file(
        &self,
        pack_path: &str,
        idx_path: &str,
        hex_hash: u64,
        hash_str: &str,
    ) -> Result<Option<GitObject>> {
        let idx_data = std::fs::read(idx_path)?;
        let pack_data = std::fs::read(pack_path)?;

        let offset = match self.find_in_index(&idx_data, hex_hash, hash_str) {
            Some(off) => off,
            None => return Ok(None),
        };

        let obj = self.unpack_object(&pack_data, offset, hash_str)?;
        Ok(Some(obj))
    }

    fn find_in_index(&self, idx_data: &[u8], hash: u64, hash_str: &str) -> Option<u64> {
        if idx_data.len() < 8 {
            return None;
        }

        let magic = u32::from_be_bytes(idx_data[0..4].try_into().ok()?);
        if magic != 0xFF744F63 {
            return None;
        }
        let version = u32::from_be_bytes(idx_data[4..8].try_into().ok()?);
        if version != 2 {
            return None;
        }

        let fanout_offset = 8;
        let first_byte = hash_str.as_bytes().first().copied().unwrap_or(0);
        let start = if first_byte == 0 {
            0u32
        } else {
            u32::from_be_bytes(
                idx_data[fanout_offset + ((first_byte - 1) as usize) * 4
                    ..fanout_offset + (first_byte as usize) * 4]
                    .try_into()
                    .ok()?,
            )
        };
        let end = u32::from_be_bytes(
            idx_data[fanout_offset + (first_byte as usize) * 4
                ..fanout_offset + (first_byte as usize + 1) * 4]
                .try_into()
                .ok()?,
        );

        let num_entries = u32::from_be_bytes(
            idx_data[fanout_offset + 255 * 4..fanout_offset + 256 * 4]
                .try_into()
                .ok()?,
        );

        let hash_table_offset = fanout_offset + 256 * 4;
        let crc_table_offset = hash_table_offset + (num_entries as usize) * 20;
        let offset_table_offset = crc_table_offset + (num_entries as usize) * 4;

        for i in start..end {
            let entry_offset = hash_table_offset + (i as usize) * 20;
            if entry_offset + 20 > idx_data.len() {
                continue;
            }
            let entry_hash = &idx_data[entry_offset..entry_offset + 20];
            let entry_hex = hex::encode_upper(entry_hash);
            let entry_uint = hex_to_uint(&entry_hex).ok()?;
            if entry_uint == hash {
                let off_entry = offset_table_offset + (i as usize) * 4;
                if off_entry + 4 > idx_data.len() {
                    return None;
                }
                let offset_val =
                    u32::from_be_bytes(idx_data[off_entry..off_entry + 4].try_into().ok()?);
                if offset_val & 0x80000000 != 0 {
                    let large_off_idx = (offset_val & 0x7FFFFFFF) as usize;
                    let large_offset_table = offset_table_offset + (num_entries as usize) * 4;
                    let large_off_entry = large_offset_table + large_off_idx * 8;
                    if large_off_entry + 8 <= idx_data.len() {
                        return Some(u64::from_be_bytes(
                            idx_data[large_off_entry..large_off_entry + 8]
                                .try_into()
                                .ok()?,
                        ));
                    }
                    return None;
                }
                return Some(offset_val as u64);
            }
        }

        None
    }

    fn unpack_object(&self, pack_data: &[u8], offset: u64, hash_str: &str) -> Result<GitObject> {
        let offset = offset as usize;
        if offset >= pack_data.len() {
            return Err(GitVizError::InvalidFormat(
                "pack offset out of bounds".into(),
            ));
        }

        let byte = pack_data[offset];
        let type_bits = (byte >> 4) & 0x07;
        let obj_type = match type_bits {
            1 => GitObjectType::Commit,
            2 => GitObjectType::Tree,
            3 => GitObjectType::Blob,
            4 => GitObjectType::Tag,
            _ => {
                return Err(GitVizError::InvalidFormat(format!(
                    "unknown pack object type: {}",
                    type_bits
                )))
            }
        };

        let mut size = ((byte & 0x0F) as usize) << 3;
        let mut shift: usize = 4;
        let mut pos = offset + 1;
        let mut continue_bit = byte & 0x80 != 0;

        while continue_bit && pos < pack_data.len() {
            let b = pack_data[pos];
            size |= ((b & 0x7F) as usize) << shift;
            shift += 7;
            continue_bit = b & 0x80 != 0;
            pos += 1;
        }

        let mut content = vec![0u8; size];
        let mut decoder = ZlibDecoder::new(&pack_data[pos..]);
        decoder
            .read_exact(&mut content)
            .map_err(|e| GitVizError::Decompression(e.to_string()))?;

        Ok(GitObject {
            obj_type,
            hash: hash_str.to_string(),
            size,
            content,
        })
    }

    pub fn read_ref(&self, git_dir: &Path, ref_path: &str) -> Result<String> {
        let ref_file = git_dir.join(ref_path);
        if !ref_file.exists() {
            return Err(GitVizError::ObjectNotFound(ref_path.to_string()));
        }
        let content = std::fs::read_to_string(&ref_file)?;
        let content = content.trim();
        if let Some(target) = content.strip_prefix("ref: ") {
            return self.read_ref(git_dir, target);
        }
        Ok(content.to_string())
    }
}

fn hex_to_uint(hex: &str) -> Result<u64> {
    let bytes = hex::decode(hex).map_err(|e| GitVizError::Parse(e.to_string()))?;
    let mut result: u64 = 0;
    for &b in &bytes {
        result = (result << 8) | (b as u64);
    }
    Ok(result)
}

mod hex {
    pub fn encode_upper(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02X}", b)).collect()
    }

    pub fn decode(hex: &str) -> std::result::Result<Vec<u8>, String> {
        if !hex.len().is_multiple_of(2) {
            return Err("hex string has odd length".into());
        }
        (0..hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| format!("invalid hex: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encode_decode() {
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let encoded = hex::encode_upper(&data);
        assert_eq!(encoded, "DEADBEEF");
        let decoded = hex::decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_git_object_type_from_str() {
        assert_eq!(
            "commit".parse::<GitObjectType>().unwrap(),
            GitObjectType::Commit
        );
        assert_eq!(
            "tree".parse::<GitObjectType>().unwrap(),
            GitObjectType::Tree
        );
        assert_eq!(
            "blob".parse::<GitObjectType>().unwrap(),
            GitObjectType::Blob
        );
        assert!("unknown".parse::<GitObjectType>().is_err());
    }

    #[test]
    fn test_hex_to_uint() {
        let val = hex_to_uint("FF").unwrap();
        assert_eq!(val, 255);
    }
}
