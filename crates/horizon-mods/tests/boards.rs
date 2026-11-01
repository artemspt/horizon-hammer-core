//! A real board (`mods/example_board`) called by a real mod (`mods/board_user`):
//! nothing opens until the player turned on that exact file. One test: the
//! registry is one per process, as in the game.
use horizon_mods::boards::{self, BoardRow, MANIFEST, library_entry};
use horizon_mods::{
    GameMode, Mods, Output, ServerPlayer, Snapshot, Status, manifest::componentize,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn built(root: &Path, args: &[&str]) {
    let status = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["build", "--release", "--target-dir", "target/mods-test"])
        .args(args)
        .status()
        .expect("cargo runs");
    assert!(status.success());
}

fn package(manifest: &str, library: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file(MANIFEST, options).unwrap();
    zip.write_all(manifest.as_bytes()).unwrap();
    zip.start_file(library_entry(), options).unwrap();
    zip.write_all(library).unwrap();
    zip.finish().unwrap().into_inner()
}

struct Air;
impl horizon_mods::WorldView for Air {
    fn block(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        Some("minecraft:air".to_owned())
    }
    fn biome(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        Some("minecraft:plains".to_owned())
    }
    fn has_dimension(&self, _dimension: &str) -> bool {
        true
    }
    fn check_state(&self, _state: &str) -> Result<(), String> {
        Ok(())
    }
}

fn the_board(rows: &[BoardRow]) -> &BoardRow {
    rows.iter()
        .find(|r| r.manifest.as_ref().is_some_and(|m| m.id == "example_board"))
        .expect("the board is listed")
}

#[test]
fn a_board_runs_only_when_its_exact_file_is_on() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    built(&root, &["-p", "example_board"]);
    built(
        &root,
        &["-p", "board_user", "--target", "wasm32-unknown-unknown"],
    );
    let target = root.join("target/mods-test");
    let library = std::fs::read(target.join("release").join(format!(
        "{}example_board{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )))
    .unwrap();
    let manifest =
        std::fs::read_to_string(root.join("mods/example_board/horizon-board.toml")).unwrap();

    let work: PathBuf = target.join("boards-test");
    let _ = std::fs::remove_dir_all(&work);
    let dir = work.join(boards::DIR);
    let core =
        std::fs::read(target.join("wasm32-unknown-unknown/release/board_user.wasm")).unwrap();
    let user_manifest =
        std::fs::read_to_string(root.join("mods/board_user/horizon-mod.toml")).unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let user = work.join("board_user.hmod");
    std::fs::write(
        &user,
        horizon_mods::Package::write(&user_manifest, &componentize(&core).unwrap()).unwrap(),
    )
    .unwrap();
    let host = &horizon_mods::BOARD_HOST;

    // The folder appears by itself, and an empty one is fine.
    assert!(boards::start(&dir, host, |_| false).is_empty());
    assert!(dir.is_dir());

    // In the folder but off: listed, not running, and the mod says what it needs.
    std::fs::write(dir.join("example.hboard"), package(&manifest, &library)).unwrap();
    let rows = boards::start(&dir, host, |_| false);
    let row = the_board(&rows).row.clone();
    assert!(!the_board(&rows).running && the_board(&rows).problem.is_none());
    assert!(!dir.join(".cache").exists(), "nothing unpacked while off");
    let load = || Mods::load(std::slice::from_ref(&user), |_| true, |_| false);
    let off = load();
    let Status::Failed(why) = &off.entries()[0].status else {
        panic!("{:?}", off.entries()[0].status);
    };
    assert!(why.contains("example_board"), "{why}");

    // A changed file is a new row: the player's switch doesn't carry over.
    let other = format!("{manifest}\n# changed\n");
    std::fs::write(dir.join("example.hboard"), package(&other, &library)).unwrap();
    let rows = boards::start(&dir, host, |id| id == row);
    assert_ne!(the_board(&rows).row, row);
    assert!(!the_board(&rows).running);

    // Declared conflicts refuse both, naming the other; nothing opens.
    std::fs::write(dir.join("example.hboard"), package(&manifest, &library)).unwrap();
    let rival = manifest.replace("id = \"example_board\"", "id = \"rival\"")
        + "conflicts = [\"example_board\"]\n";
    std::fs::write(dir.join("rival.hboard"), package(&rival, &library)).unwrap();
    let rows = boards::start(&dir, host, |_| true);
    assert!(!the_board(&rows).running);
    assert!(the_board(&rows).problem.as_ref().unwrap().contains("rival"));
    std::fs::remove_file(dir.join("rival.hboard")).unwrap();

    // On: the mod starts and calls it, with what the manifest doesn't allow refused.
    let rows = boards::start(&dir, host, |id| id == row);
    assert!(the_board(&rows).running, "{:?}", the_board(&rows).problem);
    let mut mods = load();
    assert_eq!(mods.entries()[0].status, Status::Running);
    let steve = ServerPlayer {
        id: "00000000-0000-0000-0000-000000000001".into(),
        name: "Steve".into(),
        x: 0.5,
        y: 64.0,
        z: 0.5,
        dimension: "minecraft:overworld".into(),
        game_mode: GameMode::Survival,
        health: 20.0,
    };
    mods.set_snapshot(Snapshot {
        players: vec![steve.clone()],
        ..Snapshot::default()
    });
    mods.lend_world(Box::new(Air));
    assert!(mods.server_command(&steve, "board_demo"));
    let out = mods.drain();
    let said = format!("{out:?}");
    assert!(said.contains("40 + 2 = 42"), "{said}");
    assert!(said.contains("the board panicked"), "{said}");
    assert!(said.contains("unknown refused: true"), "{said}");
    assert!(said.contains("undeclared board refused: true"), "{said}");
    // The board's own world write comes out as the mod's would.
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::SetBlocks { blocks, .. }
            if blocks.iter().any(|b| b.state == "minecraft:gold_block"))),
        "{said}"
    );

    // Turned off, it stops answering, and the next mod start says so.
    let rows = boards::start(&dir, host, |_| false);
    assert!(!the_board(&rows).running);
    assert!(boards::call("example_board", "echo", b"x").is_err());
}
