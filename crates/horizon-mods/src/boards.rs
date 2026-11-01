//! Expansion boards (`docs/boards.md`): `.hboard` files in `horizon-boards/`,
//! native libraries offering functions to sandboxed mods. A board has the
//! player's rights, so nothing here opens its library until the player turned
//! on that exact file: the row's id carries the file's hash, and a changed file
//! is a new, switched-off row.
//!
//! The registry is global: one process has one copy of a library, whichever of
//! its `Mods` (window, server thread) asked for it.
use horizon_board_sdk::{ABI, abi};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub use crate::board_manifest::*;

/// What one call may hand a mod.
const ANSWER_BYTES: usize = 16 << 20;
/// A library's size in a package, so a broken zip can't fill a disk.
const LIBRARY_BYTES: u64 = 512 << 20;
/// The prefix of a board's row on the Mods page and in `horizon-mods.txt`.
const ROW: &str = "board.";

/// A board as the Mods page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardRow {
    /// `board.<id>@<hash>`: what the player's switch is kept under.
    pub row: String,
    pub file: PathBuf,
    pub manifest: Option<BoardManifest>,
    /// Why it isn't running although it's on, or can't run at all.
    pub problem: Option<String>,
    pub running: bool,
}

/// A library that is open. Never closed: a mod's call may be on its way
/// through it, and code it started may still run.
struct Open {
    call: extern "C" fn(abi::Bytes, abi::Bytes, *mut abi::Owned) -> i32,
    free: extern "C" fn(abi::Owned),
    _library: libloading::Library,
}

#[derive(Default)]
struct Registry {
    /// By the file's hash: a library is opened once.
    open: BTreeMap<String, Arc<Open>>,
    /// By board id: what mods may call now.
    running: BTreeMap<String, (BoardManifest, Arc<Open>)>,
    rows: Vec<BoardRow>,
}

static REGISTRY: Mutex<Option<Registry>> = Mutex::new(None);

fn with_registry<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    let mut slot = REGISTRY.lock().unwrap_or_else(|p| p.into_inner());
    f(slot.get_or_insert_with(Registry::default))
}

fn hash_of(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A package's manifest and its library's bytes for this computer, if any.
fn read_package(bytes: &[u8]) -> Result<(BoardManifest, Option<Vec<u8>>), String> {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("not a board: {e}"))?;
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|_| format!("no {MANIFEST}"))?
        .take(1 << 16)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let manifest = BoardManifest::parse(&text)?;
    // Found by its exact name, and written under a name of ours: nothing in
    // the zip chooses a path on the player's disk.
    let library = match zip.by_name(&library_entry()) {
        Ok(entry) => {
            let mut out = Vec::new();
            entry
                .take(LIBRARY_BYTES)
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            Some(out)
        }
        Err(_) => None,
    };
    Ok((manifest, library))
}

/// Opens a library and greets it; the caller knows the player turned it on.
fn open(dir: &Path, hash: &str, library: &[u8], host: &'static abi::Host) -> Result<Open, String> {
    let cache = dir.join(".cache");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let extension = library_entry();
    let extension = extension.rsplit('.').next().unwrap_or("so");
    let path = cache.join(format!("{hash}.{extension}"));
    if !path.exists() {
        let partial = path.with_extension("partial");
        std::fs::write(&partial, library).map_err(|e| e.to_string())?;
        std::fs::rename(&partial, &path).map_err(|e| e.to_string())?;
    }
    // SAFETY: not safe in any sense the compiler knows: a board is native code
    // with the player's rights. The player turned on this exact file, with the
    // warning in front of them; that is all the safety there is.
    unsafe {
        let library = libloading::Library::new(&path).map_err(|e| format!("can't open: {e}"))?;
        let symbol = |e: libloading::Error| format!("not a board: {e}");
        let version = *library
            .get::<extern "C" fn() -> u32>(b"horizon_board_abi")
            .map_err(symbol)?;
        if version() != ABI {
            return Err(format!(
                "made for board interface {}, this game has {ABI}",
                version()
            ));
        }
        let init = *library
            .get::<extern "C" fn(*const abi::Host)>(b"horizon_board_init")
            .map_err(symbol)?;
        let call = *library
            .get::<extern "C" fn(abi::Bytes, abi::Bytes, *mut abi::Owned) -> i32>(
                b"horizon_board_call",
            )
            .map_err(symbol)?;
        let free = *library
            .get::<extern "C" fn(abi::Owned)>(b"horizon_board_free")
            .map_err(symbol)?;
        init(host);
        Ok(Open {
            call,
            free,
            _library: library,
        })
    }
}

/// Makes the folder, reads what's in it and opens the boards the player turned
/// on. Boards turned off stop answering; their libraries stay open until the
/// game exits. Safe to call again.
pub fn start(dir: &Path, host: &'static abi::Host, is_on: impl Fn(&str) -> bool) -> Vec<BoardRow> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        log::warn!("{}: {e}", dir.display());
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXTENSION))
        .collect();
    files.sort();

    let mut found = Vec::new();
    for file in files {
        let read = std::fs::read(&file)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                let hash = hash_of(&bytes);
                read_package(&bytes).map(|(manifest, library)| (hash, manifest, library))
            });
        found.push((file, read));
    }

    // Who conflicts, among the boards that are on: nothing of theirs is opened.
    let on: Vec<(&str, &BoardManifest)> = found
        .iter()
        .filter_map(|(_, r)| r.as_ref().ok())
        .filter(|(hash, m, _)| is_on(&row_id(&m.id, hash)))
        .map(|(_, m, _)| (m.id.as_str(), m))
        .collect();
    let clash = |m: &BoardManifest| -> Option<String> {
        if on.iter().filter(|(id, _)| *id == m.id).count() > 1 {
            return Some(format!("two files are board {}: turn one off", m.id));
        }
        on.iter()
            .find(|(id, other)| {
                *id != m.id
                    && (m.conflicts.iter().any(|c| c == id)
                        || other.conflicts.iter().any(|c| *c == m.id))
            })
            .map(|(id, _)| format!("conflicts with {id}: turn one off"))
    };

    with_registry(|reg| {
        reg.running.clear();
        reg.rows.clear();
        for (file, read) in &found {
            let mut row = BoardRow {
                row: String::new(),
                file: file.clone(),
                manifest: None,
                problem: None,
                running: false,
            };
            match read {
                Err(e) => row.problem = Some(e.clone()),
                Ok((hash, manifest, library)) => {
                    row.row = row_id(&manifest.id, hash);
                    row.manifest = Some(manifest.clone());
                    let wanted = is_on(&row.row);
                    let opened = match (library, clash(manifest)) {
                        (None, _) => Err(format!(
                            "not built for this computer: no {}",
                            library_entry()
                        )),
                        (_, Some(reason)) if wanted => Err(reason),
                        (Some(library), _) if wanted => match reg.open.get(hash) {
                            Some(open) => Ok(Some(open.clone())),
                            None => open(dir, hash, library, host).map(|o| {
                                let o = Arc::new(o);
                                reg.open.insert(hash.clone(), o.clone());
                                Some(o)
                            }),
                        },
                        _ => Ok(None),
                    };
                    match opened {
                        Ok(Some(open)) => {
                            log::warn!(
                                "Board {} {} is running: native code, not sandboxed",
                                manifest.id,
                                manifest.version
                            );
                            reg.running
                                .insert(manifest.id.clone(), (manifest.clone(), open));
                            row.running = true;
                        }
                        Ok(None) => {}
                        Err(e) => row.problem = Some(e),
                    }
                }
            }
            reg.rows.push(row);
        }
        reg.rows.clone()
    })
}

fn row_id(id: &str, hash: &str) -> String {
    format!("{ROW}{id}@{hash}")
}

/// The boards as [`start`] last found them.
pub fn rows() -> Vec<BoardRow> {
    with_registry(|reg| reg.rows.clone())
}

/// What `/hmod boards` says.
pub fn lines() -> Vec<String> {
    let rows = rows();
    if rows.is_empty() {
        return vec![format!("No boards in {DIR}/")];
    }
    rows.iter()
        .map(|r| {
            let name = r.manifest.as_ref().map_or_else(
                || r.file.display().to_string(),
                |m| format!("{} {}", m.id, m.version),
            );
            let state = match (&r.problem, r.running) {
                (Some(problem), _) => problem.as_str(),
                (None, true) => "running (unsafe)",
                (None, false) => "off",
            };
            format!("{name}: {state}")
        })
        .collect()
}

/// Why a mod needing these boards can't start.
pub fn missing(needs: &[String]) -> Option<String> {
    with_registry(|reg| {
        needs.iter().find_map(|need| {
            let (id, major) = parse_need(need);
            match reg.running.get(id) {
                None => Some(format!(
                    "needs board {need}: put it in {DIR}/ and turn it on in Options > Mods"
                )),
                Some((m, _)) if major.is_some_and(|w| w != m.major()) => Some(format!(
                    "needs board {need}, and {id} {} is what's on",
                    m.version
                )),
                _ => None,
            }
        })
    })
}

/// The running board's version.
pub fn version(id: &str) -> Option<String> {
    with_registry(|reg| reg.running.get(id).map(|(m, _)| m.version.clone()))
}

/// Calls a board's function. The registry isn't held meanwhile: the board may
/// take its time, and call the game back.
pub fn call(id: &str, function: &str, input: &[u8]) -> Result<Vec<u8>, String> {
    let open = with_registry(|reg| {
        let (manifest, open) = reg
            .running
            .get(id)
            .ok_or_else(|| format!("board {id} isn't running"))?;
        if !manifest.functions.iter().any(|f| f == function) {
            return Err(format!("board {id} has no function {function}"));
        }
        Ok::<_, String>(open.clone())
    })?;
    let mut out = abi::Owned::EMPTY;
    let code = (open.call)(
        abi::Bytes::of(function.as_bytes()),
        abi::Bytes::of(input),
        &raw mut out,
    );
    // SAFETY: the board wrote `out`, or it stayed empty; freed only below.
    let answer = unsafe {
        abi::Bytes {
            ptr: out.ptr,
            len: out.len,
        }
        .get()
    };
    let result = if code != abi::OK {
        Err(String::from_utf8_lossy(answer).into_owned())
    } else if answer.len() > ANSWER_BYTES {
        Err(format!(
            "board {id} answered {} bytes: at most {ANSWER_BYTES}",
            answer.len()
        ))
    } else {
        Ok(answer.to_vec())
    };
    (open.free)(out);
    result
}
