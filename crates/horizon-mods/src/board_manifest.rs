//! What describes an expansion board (`docs/boards.md`): its file, its manifest and
//! a mod's claim on it. Apart from [`crate::boards`] so tools read them without a runtime.
use serde::Deserialize;

/// The folder of `.hboard` files, beside `horizon-mods/`.
pub const DIR: &str = "horizon-boards";
pub const EXTENSION: &str = "hboard";
pub const MANIFEST: &str = "horizon-board.toml";
/// A `horizon-board.toml`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BoardManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    /// The functions mods may call; a call to any other is refused.
    pub functions: Vec<String>,
    /// Boards that can't run beside this one.
    #[serde(default)]
    pub conflicts: Vec<String>,
}

impl BoardManifest {
    pub fn parse(text: &str) -> Result<Self, String> {
        let m: Self = toml::from_str(text).map_err(|e| format!("{MANIFEST}: {e}"))?;
        if !valid_id(&m.id) {
            return Err(format!(
                "{MANIFEST}: id {:?} is not [a-z0-9_]{{1,64}}",
                m.id
            ));
        }
        if m.functions.is_empty() {
            return Err(format!("{MANIFEST}: a board offers at least one function"));
        }
        Ok(m)
    }
    /// The first number of `version`.
    pub(crate) fn major(&self) -> &str {
        self.version.split('.').next().unwrap_or_default()
    }
}

pub(crate) fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// The package's library for this computer, `native/linux-x86_64.so`.
pub fn library_entry() -> String {
    library_entry_for(std::env::consts::OS, std::env::consts::ARCH)
}
/// [`library_entry`] of another system.
pub fn library_entry_for(os: &str, arch: &str) -> String {
    let extension = match os {
        "windows" => "dll",
        "macos" => "dylib",
        _ => "so",
    };
    format!("native/{os}-{arch}.{extension}")
}

/// A mod's `boards` entry: `id`, or `id@2` for that major version.
pub(crate) fn parse_need(need: &str) -> (&str, Option<&str>) {
    match need.split_once('@') {
        Some((id, major)) => (id, Some(major)),
        None => (need, None),
    }
}

/// A mod's `boards` entry is well formed.
pub fn check_need(need: &str) -> Result<(), String> {
    let (id, major) = parse_need(need);
    let major_ok = major.is_none_or(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit()));
    if valid_id(id) && major_ok {
        Ok(())
    } else {
        Err(format!("board {need:?} is not id or id@major"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifests_are_checked() {
        let good = "id = \"grid\"\nname = \"Grid\"\nversion = \"2.1.0\"\nfunctions = [\"a\"]\n";
        let m = BoardManifest::parse(good).unwrap();
        assert_eq!(m.major(), "2");
        assert!(BoardManifest::parse(&good.replace("grid", "../x")).is_err());
        assert!(BoardManifest::parse(&good.replace("[\"a\"]", "[]")).is_err());
        assert!(BoardManifest::parse(&format!("{good}surprise = 1\n")).is_err());
    }

    #[test]
    fn needs_are_checked() {
        assert!(check_need("grid").is_ok() && check_need("grid@2").is_ok());
        assert!(check_need("Grid").is_err() && check_need("grid@").is_err());
        assert!(check_need("grid@x").is_err() && check_need("").is_err());
    }

    #[test]
    fn a_library_name_is_per_system() {
        assert_eq!(
            library_entry_for("linux", "x86_64"),
            "native/linux-x86_64.so"
        );
        assert_eq!(
            library_entry_for("windows", "x86_64"),
            "native/windows-x86_64.dll"
        );
        assert_eq!(
            library_entry_for("macos", "aarch64"),
            "native/macos-aarch64.dylib"
        );
    }
}
