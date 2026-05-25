mod savegame;

use savegame::{
    collect_entries_from_dir, decode_file, encode_dir, encode_file, encode_to_file, extract_to_dir,
    SaveEntry,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        usage_global(&args[0]);
        return Err("missing command".into());
    }
    let cmd = args[1].as_str();
    match cmd {
        "-h" | "--help" | "help" => {
            usage_global(&args[0]);
            return Ok(());
        }
        "-v" | "--version" => {
            println!("gsSaveGame {}", VERSION);
            return Ok(());
        }
        _ => {}
    }
    let opts = parse_flags(&args[2..])?;
    if has_flag(&opts, "-h", "--help") {
        usage_cmd(&args[0], cmd);
        return Ok(());
    }
    match cmd {
        "encoder" => cmd_encoder(&opts),
        "decoder" => cmd_decoder(&opts),
        _ => {
            usage_global(&args[0]);
            Err(format!("unknown command: {}", cmd))
        }
    }
}

// ── Usage ─────────────────────────────────────────────────────────────────────

fn usage_global(bin: &str) {
    eprintln!("gsSaveGame {} — GSSG savegame archive tool", VERSION);
    eprintln!();
    eprintln!("Usage:  {} <command> [options]", bin);
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  encoder    Encode files into a .gssg archive");
    eprintln!("  decoder    Decode a .gssg archive into files");
    eprintln!();
    eprintln!("Global flags:");
    eprintln!("  -v, --version   Show version");
    eprintln!("  -h, --help      Show help");
}

fn usage_cmd(bin: &str, cmd: &str) {
    match cmd {
        "encoder" => {
            eprintln!("Usage: {} encoder [options]", bin);
            eprintln!();
            eprintln!("  -f, --file <FILE>       Encode a single file into a .gssg archive");
            eprintln!("  -d, --dir <DIR>         Encode a directory into a .gssg archive");
            eprintln!("  -b, --batch <FILE>      Add specific files (repeatable)");
            eprintln!("  -r, --recursive         Recursive (only with --dir)");
            eprintln!("  -o, --output <OUT.gssg> Output archive file");
            eprintln!("  -h, --help              Show this help");
        }
        "decoder" => {
            eprintln!("Usage: {} decoder [options]", bin);
            eprintln!();
            eprintln!("  -f, --file <*.gssg>     Input .gssg archive");
            eprintln!("  -d, --dir <DIR>         Output directory for extracted files");
            eprintln!("  -b, --batch <FILE>      Multiple .gssg files (repeatable)");
            eprintln!("  -r, --recursive         Scan directory recursively for .gssg files");
            eprintln!("  -h, --help              Show this help");
        }
        _ => usage_global(bin),
    }
}

// ── Flag parsing ──────────────────────────────────────────────────────────────

fn parse_flags(args: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a.starts_with('-') {
            let key = a.clone();
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                map.entry(key).or_default().push(args[i + 1].clone());
                i += 2;
            } else {
                map.entry(key).or_default();
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Ok(map)
}

fn has_flag(opts: &HashMap<String, Vec<String>>, short: &str, long: &str) -> bool {
    opts.contains_key(short) || opts.contains_key(long)
}

fn get_val(opts: &HashMap<String, Vec<String>>, short: &str, long: &str) -> Option<String> {
    opts.get(short)
        .or_else(|| opts.get(long))
        .and_then(|v| v.first().cloned())
}

fn get_vals(opts: &HashMap<String, Vec<String>>, short: &str, long: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(v) = opts.get(short) {
        out.extend(v.iter().cloned());
    }
    if let Some(v) = opts.get(long) {
        out.extend(v.iter().cloned());
    }
    out
}

// ── Encoder command ───────────────────────────────────────────────────────────

fn cmd_encoder(opts: &HashMap<String, Vec<String>>) -> Result<(), String> {
    let file = get_val(opts, "-f", "--file");
    let dir = get_val(opts, "-d", "--dir");
    let batch = get_vals(opts, "-b", "--batch");
    let recursive = has_flag(opts, "-r", "--recursive");
    let output = get_val(opts, "-o", "--output");

    let out_path = output
        .map(PathBuf::from)
        .ok_or("--output is required for encoder")?;

    if let Some(d) = &dir {
        let blob = encode_dir(Path::new(d), recursive)?;
        std::fs::write(&out_path, &blob)
            .map_err(|e| format!("write {:?}: {}", out_path, e))?;
        eprintln!("encoded directory {:?} -> {:?}", d, out_path);
        return Ok(());
    }

    if let Some(f) = &file {
        let blob = encode_file(Path::new(f))?;
        std::fs::write(&out_path, &blob)
            .map_err(|e| format!("write {:?}: {}", out_path, e))?;
        eprintln!("encoded {:?} -> {:?}", f, out_path);
        return Ok(());
    }

    if !batch.is_empty() {
        let mut entries = Vec::new();
        for b in &batch {
            let p = Path::new(b);
            if p.is_dir() {
                let mut dir_entries = collect_entries_from_dir(p, recursive)?;
                entries.append(&mut dir_entries);
            } else {
                let name = p
                    .file_name()
                    .ok_or(format!("no filename: {:?}", p))?
                    .to_string_lossy()
                    .to_string();
                let data = std::fs::read(p).map_err(|e| format!("read {:?}: {}", p, e))?;
                entries.push(SaveEntry { name, data });
            }
        }
        encode_to_file(&entries, &out_path)?;
        eprintln!("encoded {} files -> {:?}", entries.len(), out_path);
        return Ok(());
    }

    Err("encoder requires --file, --dir, or --batch".into())
}

// ── Decoder command ───────────────────────────────────────────────────────────

fn cmd_decoder(opts: &HashMap<String, Vec<String>>) -> Result<(), String> {
    let file = get_val(opts, "-f", "--file");
    let dir = get_val(opts, "-d", "--dir");
    let batch = get_vals(opts, "-b", "--batch");
    let recursive = has_flag(opts, "-r", "--recursive");

    let out_dir = dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if let Some(f) = &file {
        let archive = decode_file(Path::new(f))?;
        eprintln!(
            "archive {:?}: magic={:?} entries={}",
            f,
            String::from_utf8_lossy(&archive.magic),
            archive.entries.len()
        );
        for entry in &archive.entries {
            eprintln!("  {} ({} bytes)", entry.name, entry.data.len());
        }
        let n = extract_to_dir(&archive, &out_dir)?;
        eprintln!("extracted {} files to {:?}", n, out_dir);
        return Ok(());
    }

    if !batch.is_empty() {
        for b in &batch {
            decode_single(Path::new(b), &out_dir)?;
        }
        return Ok(());
    }

    if recursive {
        let scan_dir = get_val(opts, "-f", "--file")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        scan_and_decode(&scan_dir, &out_dir)?;
        return Ok(());
    }

    Err("decoder requires --file, --batch, or --recursive with a directory".into())
}

fn decode_single(path: &Path, out_dir: &Path) -> Result<(), String> {
    let archive = decode_file(path)?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let dest = out_dir.join(stem.as_ref());
    let n = extract_to_dir(&archive, &dest)?;
    eprintln!("extracted {} files from {:?} to {:?}", n, path, dest);
    Ok(())
}

fn scan_and_decode(dir: &Path, out_dir: &Path) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("readdir {:?}: {}", dir, e))?;
    for item in rd {
        let item = item.map_err(|e| format!("readdir entry: {}", e))?;
        let path = item.path();
        if path.is_dir() {
            scan_and_decode(&path, out_dir)?;
        } else if path.extension().map_or(false, |ext| ext == "gssg") {
            decode_single(&path, out_dir)?;
        }
    }
    Ok(())
}
