//! GSSG savegame archive codec.
//!
//! ## File format (reverse-engineered from FS26 aarch64 binary)
//!
//! ### Outer envelope
//!
//! | Offset | Size | Description                    |
//! |--------|------|--------------------------------|
//! | 0x00   | 4    | Magic marker (`SAVEGAME_ARCHIVE_MARKER`) |
//! | 0x04   | 4    | Uncompressed payload size (u32 LE)       |
//! | 0x08   | N    | zlib-compressed payload (level 9)        |
//!
//! ### Inner payload (decompressed)
//!
//! | Offset | Size | Description                               |
//! |--------|------|-------------------------------------------|
//! | 0x00   | 4    | Number of file entries (u32 LE)            |
//! | 0x04   | …    | Sequential entry records                   |
//!
//! Each entry:
//!
//! | Field         | Size                              |
//! |---------------|-----------------------------------|
//! | filename_len  | 4 bytes (u32 LE)                  |
//! | filename      | `pad4_next(filename_len)` bytes   |
//! | data_len      | 4 bytes (u32 LE)                  |
//! | data          | `pad4_up(data_len)` bytes         |
//!
//! Where:
//! - `pad4_next(x) = (x & !3) + 4`  — always advances to the *next* 4-byte boundary
//! - `pad4_up(x)   = (x + 3) & !3`  — rounds up to 4-byte boundary (stays if already aligned)

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

const GSSG_MAGIC: &[u8; 4] = b"GSSG";

fn pad4_next(x: u32) -> u32 {
    (x & !3) + 4
}

fn pad4_up(x: u32) -> u32 {
    (x + 3) & !3
}

// ── Data types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SaveEntry {
    pub name: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct SaveArchive {
    pub magic: [u8; 4],
    pub entries: Vec<SaveEntry>,
}

// ── Decoder ──────────────────────────────────────────────────────────────────

pub fn decode(raw: &[u8]) -> Result<SaveArchive, String> {
    if raw.len() < 8 {
        return Err("file too small (need at least 8 bytes)".into());
    }

    let magic: [u8; 4] = raw[0..4].try_into().unwrap();
    let uncomp_size = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;

    let compressed = &raw[8..];
    let mut decoder = ZlibDecoder::new(compressed);
    let mut payload = Vec::with_capacity(uncomp_size);
    decoder
        .read_to_end(&mut payload)
        .map_err(|e| format!("zlib decompress: {}", e))?;

    if payload.len() < 4 {
        return Err("decompressed payload too small".into());
    }

    let num_files = u32::from_le_bytes(payload[0..4].try_into().unwrap()) as usize;
    let mut entries = Vec::with_capacity(num_files);
    let mut pos: usize = 4;

    for i in 0..num_files {
        if pos + 4 > payload.len() {
            return Err(format!("entry {}: unexpected EOF reading filename_len", i));
        }
        let fname_len = u32::from_le_bytes(payload[pos..pos + 4].try_into().unwrap());
        pos += 4;

        let fname_padded = pad4_next(fname_len) as usize;
        if pos + fname_padded > payload.len() {
            return Err(format!("entry {}: unexpected EOF reading filename", i));
        }
        let name = String::from_utf8_lossy(&payload[pos..pos + fname_len as usize])
            .trim_end_matches('\0')
            .to_string();
        pos += fname_padded;

        if pos + 4 > payload.len() {
            return Err(format!("entry {}: unexpected EOF reading data_len", i));
        }
        let data_len = u32::from_le_bytes(payload[pos..pos + 4].try_into().unwrap());
        pos += 4;

        let data_padded = pad4_up(data_len) as usize;
        if pos + data_padded > payload.len() {
            return Err(format!("entry {}: unexpected EOF reading data", i));
        }
        let data = payload[pos..pos + data_len as usize].to_vec();
        pos += data_padded;

        entries.push(SaveEntry { name, data });
    }

    Ok(SaveArchive { magic, entries })
}

pub fn decode_file(path: &Path) -> Result<SaveArchive, String> {
    let raw = fs::read(path).map_err(|e| format!("read {:?}: {}", path, e))?;
    decode(&raw)
}

pub fn extract_to_dir(archive: &SaveArchive, out_dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {:?}: {}", out_dir, e))?;
    for entry in &archive.entries {
        let dest = out_dir.join(&entry.name);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
        }
        fs::write(&dest, &entry.data).map_err(|e| format!("write {:?}: {}", dest, e))?;
    }
    Ok(archive.entries.len())
}

// ── Encoder ──────────────────────────────────────────────────────────────────

pub fn encode(entries: &[SaveEntry]) -> Result<Vec<u8>, String> {
    let mut payload: Vec<u8> = Vec::new();

    payload.extend_from_slice(&(entries.len() as u32).to_le_bytes());

    for entry in entries {
        let fname_bytes = entry.name.as_bytes();
        let fname_len = fname_bytes.len() as u32;
        let fname_padded = pad4_next(fname_len) as usize;

        payload.extend_from_slice(&fname_len.to_le_bytes());
        payload.extend_from_slice(fname_bytes);
        let pad_fname = fname_padded - fname_bytes.len();
        payload.extend(std::iter::repeat(0u8).take(pad_fname));

        let data_len = entry.data.len() as u32;
        let data_padded = pad4_up(data_len) as usize;

        payload.extend_from_slice(&data_len.to_le_bytes());
        payload.extend_from_slice(&entry.data);
        let pad_data = data_padded - entry.data.len();
        payload.extend(std::iter::repeat(0u8).take(pad_data));
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(9));
    encoder
        .write_all(&payload)
        .map_err(|e| format!("zlib compress: {}", e))?;
    let compressed = encoder
        .finish()
        .map_err(|e| format!("zlib finish: {}", e))?;

    let uncomp_size = payload.len() as u32;

    let mut out = Vec::with_capacity(8 + compressed.len());
    out.extend_from_slice(GSSG_MAGIC);
    out.extend_from_slice(&uncomp_size.to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

pub fn collect_entries_from_dir(dir: &Path, recursive: bool) -> Result<Vec<SaveEntry>, String> {
    let mut entries = Vec::new();
    collect_dir_inner(dir, dir, recursive, &mut entries)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn collect_dir_inner(
    base: &Path,
    current: &Path,
    recursive: bool,
    entries: &mut Vec<SaveEntry>,
) -> Result<(), String> {
    let rd = fs::read_dir(current).map_err(|e| format!("readdir {:?}: {}", current, e))?;
    for item in rd {
        let item = item.map_err(|e| format!("readdir entry: {}", e))?;
        let ft = item
            .file_type()
            .map_err(|e| format!("filetype {:?}: {}", item.path(), e))?;
        if ft.is_file() {
            let rel = item
                .path()
                .strip_prefix(base)
                .unwrap_or(&item.path())
                .to_string_lossy()
                .replace('\\', "/");
            let data =
                fs::read(item.path()).map_err(|e| format!("read {:?}: {}", item.path(), e))?;
            entries.push(SaveEntry { name: rel, data });
        } else if ft.is_dir() && recursive {
            collect_dir_inner(base, &item.path(), recursive, entries)?;
        }
    }
    Ok(())
}

pub fn encode_dir(dir: &Path, recursive: bool) -> Result<Vec<u8>, String> {
    let entries = collect_entries_from_dir(dir, recursive)?;
    if entries.is_empty() {
        return Err(format!("no files found in {:?}", dir));
    }
    encode(&entries)
}

pub fn encode_file(path: &Path) -> Result<Vec<u8>, String> {
    let name = path
        .file_name()
        .ok_or("no filename")?
        .to_string_lossy()
        .to_string();
    let data = fs::read(path).map_err(|e| format!("read {:?}: {}", path, e))?;
    encode(&[SaveEntry { name, data }])
}

pub fn encode_to_file(entries: &[SaveEntry], out_path: &Path) -> Result<(), String> {
    let blob = encode(entries)?;
    fs::write(out_path, &blob).map_err(|e| format!("write {:?}: {}", out_path, e))?;
    Ok(())
}

// ── Roundtrip test ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_file() {
        let entries = vec![SaveEntry {
            name: "test.xml".into(),
            data: b"<root>hello</root>".to_vec(),
        }];
        let blob = encode(&entries).unwrap();
        assert_eq!(&blob[0..4], GSSG_MAGIC);
        let archive = decode(&blob).unwrap();
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.entries[0].name, "test.xml");
        assert_eq!(archive.entries[0].data, b"<root>hello</root>");
    }

    #[test]
    fn roundtrip_multiple_files() {
        let entries = vec![
            SaveEntry {
                name: "a.txt".into(),
                data: b"aaa".to_vec(),
            },
            SaveEntry {
                name: "subdir/b.bin".into(),
                data: vec![0u8; 1024],
            },
            SaveEntry {
                name: "c.xml".into(),
                data: b"<c/>".to_vec(),
            },
        ];
        let blob = encode(&entries).unwrap();
        let archive = decode(&blob).unwrap();
        assert_eq!(archive.entries.len(), 3);
        for (orig, decoded) in entries.iter().zip(archive.entries.iter()) {
            assert_eq!(orig.name, decoded.name);
            assert_eq!(orig.data, decoded.data);
        }
    }

    #[test]
    fn roundtrip_empty_data() {
        let entries = vec![SaveEntry {
            name: "empty.txt".into(),
            data: vec![],
        }];
        let blob = encode(&entries).unwrap();
        let archive = decode(&blob).unwrap();
        assert_eq!(archive.entries[0].data.len(), 0);
    }

    #[test]
    fn pad4_next_values() {
        assert_eq!(pad4_next(0), 4);
        assert_eq!(pad4_next(1), 4);
        assert_eq!(pad4_next(3), 4);
        assert_eq!(pad4_next(4), 8);
        assert_eq!(pad4_next(5), 8);
        assert_eq!(pad4_next(7), 8);
        assert_eq!(pad4_next(8), 12);
    }

    #[test]
    fn pad4_up_values() {
        assert_eq!(pad4_up(0), 0);
        assert_eq!(pad4_up(1), 4);
        assert_eq!(pad4_up(3), 4);
        assert_eq!(pad4_up(4), 4);
        assert_eq!(pad4_up(5), 8);
        assert_eq!(pad4_up(8), 8);
    }
}
