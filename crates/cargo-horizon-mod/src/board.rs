//! `cargo horizon-mod board new|build`: makes `.hboard` files
//! (`docs/boards.md`). A package holds one library per system; building adds
//! or replaces this computer's and keeps the others, so one file can collect
//! every system's library, from CI or by hand.
use horizon_mods::board_manifest::{BoardManifest, EXTENSION, MANIFEST, library_entry};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn main(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("new") => new(args.get(1).ok_or("board new <path>")?),
        Some("build") => build(Path::new(".")).map(|p| println!("Built {}", p.display())),
        _ => Err("usage: cargo horizon-mod board new <path> | board build".into()),
    }
}

fn new(path: &str) -> Result<(), String> {
    let root = Path::new(path);
    if root.exists() {
        return Err(format!("{path} already exists"));
    }
    let id = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("board")
        .replace('-', "_");
    std::fs::create_dir_all(root.join("src")).map_err(|e| e.to_string())?;
    let write = |name: &str, text: String| std::fs::write(root.join(name), text);
    write(
        "Cargo.toml",
        format!(
            "[package]\nname = \"{id}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n\
             [dependencies]\nhorizon-board-sdk = {{ git = \"https://github.com/artemspt/horizon-hammer-core\" }}\n\n[workspace]\n"
        ),
    )
    .map_err(|e| e.to_string())?;
    write(
        MANIFEST,
        format!(
            "id = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\ndescription = \"What it does, for the player deciding to trust it.\"\nfunctions = [\"echo\"]\n"
        ),
    )
    .map_err(|e| e.to_string())?;
    write(
        "src/lib.rs",
        "fn echo(input: &[u8]) -> Result<Vec<u8>, String> {\n    Ok(input.to_vec())\n}\n\nhorizon_board_sdk::export_board!(\"echo\" => echo);\n".to_owned(),
    )
    .map_err(|e| e.to_string())?;
    println!("Made {path}. Next: cd {path} && cargo horizon-mod board build");
    Ok(())
}

/// The library Cargo built, and its target folder.
fn compile(root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(root)
        .args([
            "build",
            "--lib",
            "--release",
            "--message-format=json-render-diagnostics",
        ])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("cargo build failed".into());
    }
    let library = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|m| m["reason"] == "compiler-artifact")
        .flat_map(|m| m["filenames"].as_array().cloned().unwrap_or_default())
        .filter_map(|f| f.as_str().map(PathBuf::from))
        .rfind(|f| {
            f.extension()
                .is_some_and(|e| e == "so" || e == "dll" || e == "dylib")
        })
        .ok_or("the build made no library: is the crate a cdylib?")?;
    let target = library
        .ancestors()
        .nth(2)
        .ok_or("unexpected target folder")?
        .to_path_buf();
    Ok((library, target))
}

fn build(root: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(root.join(MANIFEST))
        .map_err(|e| format!("{MANIFEST}: {e} (run this in a board's folder)"))?;
    let manifest = BoardManifest::parse(&text)?;
    let (library, target) = compile(root)?;
    let library = std::fs::read(&library).map_err(|e| e.to_string())?;
    let dir = target.join("horizon-mod");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.{EXTENSION}", manifest.id));

    let mine = library_entry();
    let mut kept = Vec::new();
    if let Ok(old) = std::fs::read(&path)
        && let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(old))
    {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().to_owned();
            if name.starts_with("native/") && name != mine {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                kept.push((name, bytes));
            }
        }
    }
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let entries = [(MANIFEST.to_owned(), text.into_bytes()), (mine, library)];
    for (name, bytes) in entries.iter().chain(kept.iter()) {
        zip.start_file(name, options).map_err(|e| e.to_string())?;
        zip.write_all(bytes).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    let partial = path.with_extension("partial");
    std::fs::write(&partial, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    Ok(path)
}
