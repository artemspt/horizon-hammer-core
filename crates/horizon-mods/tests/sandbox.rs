//! Loads real mods built with the SDK: the coords-hud and predator-missile
//! examples and a guest
//! that misbehaves on command. Needs `rustup target add wasm32-unknown-unknown`.
use horizon_mods::{
    BlockPos, Clock, Corner, EntityInfo, GameMode, HeldItem, Hit, HostStack, Mods, Output, Package,
    Player, Runs, ServerAction, ServerPlayer, Shape, Snapshot, Status,
    manifest::componentize,
    testing::{Script, TestGame},
};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
    time::Instant,
};

const GUEST_MANIFEST: &str = r#"
id = "test_guest"
name = "Test Guest"
version = "0.1.0"
api = "0.1"
capabilities = ["chat", "storage"]
"#;
const UI_MANIFEST: &str = r#"
id = "test_ui_guest"
name = "Test UI Guest"
version = "0.1.0"
api = "0.1"
capabilities = ["game-read", "chat", "command", "ui", "render", "input", "recipes"]
experimental = ["ui", "render", "input", "recipes"]
"#;

/// Builds the mods once and writes their `.hmod` files; returns the folder.
fn mods_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let target = root.join("target/mods-test");
        let status = Command::new(env!("CARGO"))
            .current_dir(&root)
            .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
            .args([
                "-p",
                "coords-hud",
                "-p",
                "predator-missile",
                "-p",
                "welcome",
                "-p",
                "beacons",
                "-p",
                "block-guard",
                "-p",
                "iron-press",
                "-p",
                "horizon-mods-test-guest",
                "-p",
                "horizon-mods-test-ui-guest",
                "--target-dir",
            ])
            .arg(&target)
            .status()
            .expect("cargo runs");
        assert!(
            status.success(),
            "building the test mods needs `rustup target add wasm32-unknown-unknown`"
        );
        let built = target.join("wasm32-unknown-unknown/release");
        let dir = target.join("mods");
        std::fs::create_dir_all(&dir).unwrap();
        let example = |name: &str| {
            let path = format!("crates/horizon-mod-sdk/examples/{name}/horizon-mod.toml");
            std::fs::read_to_string(root.join(path)).unwrap()
        };
        let mods = [
            ("coords_hud", example("coords-hud")),
            ("predator_missile", example("predator-missile")),
            ("welcome", example("welcome")),
            ("beacons", example("beacons")),
            ("block_guard", example("block-guard")),
            ("iron_press", example("iron-press")),
            ("horizon_mods_test_guest", GUEST_MANIFEST.to_string()),
            ("horizon_mods_test_ui_guest", UI_MANIFEST.to_string()),
        ];
        for (name, manifest) in mods {
            let core = std::fs::read(built.join(format!("{name}.wasm"))).unwrap();
            let hmod = Package::write(&manifest, &componentize(&core).unwrap()).unwrap();
            std::fs::write(dir.join(format!("{name}.hmod")), hmod).unwrap();
        }
        dir
    })
}

fn load(name: &str) -> Mods {
    Mods::load(&[mods_dir().join(name)], |_| true, |c| c == "give")
}

fn player() -> Snapshot {
    Snapshot {
        player: Some(Player {
            x: 12.5,
            y: 64.0,
            z: -3.25,
            yaw: 180.0,
            pitch: 0.0,
            health: 20.0,
            food: 20,
            dimension: "minecraft:overworld".into(),
        }),
        clock: Some(Clock {
            game_time: 100,
            day_time: 24000 + 6000,
        }),
        gui_size: (427.0, 240.0),
        ..Snapshot::default()
    }
}

/// Capability `command`: `/missile` queues vanilla commands, and the chat
/// line its impact check prints sets off the warhead.
#[test]
fn predator_missile_runs_commands() {
    let mut mods = load("predator_missile.hmod");
    assert_eq!(mods.entries()[0].status, Status::Running);
    mods.set_snapshot(player());
    let commands = |mods: &mut Mods| -> Vec<String> {
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Command { line } => Some(line),
                _ => None,
            })
            .collect()
    };
    assert!(mods.command("missile"));
    let launch = commands(&mut mods);
    assert_eq!(launch.len(), 6);
    assert!(launch.contains(&"gamemode spectator".to_string()));
    mods.tick(1);
    mods.tick(2);
    let flight = commands(&mut mods);
    assert!(flight.iter().any(|l| l.ends_with("run tp @s ^ ^ ^1.5")));
    mods.chat("Predator: impact");
    let blast = commands(&mut mods);
    assert_eq!(blast.len(), 4);
    assert!(blast.iter().all(|l| l.contains("summon tnt")));
}

#[test]
fn coords_hud_draws_and_answers() {
    let mut mods = load("coords_hud.hmod");
    assert_eq!(mods.entries()[0].status, Status::Running);
    assert_eq!(mods.commands().collect::<Vec<_>>(), ["where", "coords"]);
    mods.set_snapshot(player());
    mods.tick(1);
    let panel = mods.panels().next().expect("a panel after a tick");
    assert_eq!(panel.corner, Corner::TopLeft);
    let lines: Vec<&str> = panel.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        lines,
        [
            "XYZ 12.5 / 64.0 / -3.2",
            "Facing North (-Z)",
            "Day 2, 12:00"
        ]
    );

    assert!(mods.command("where"));
    let chat = Output::Chat {
        name: "Coordinates HUD".into(),
        text: "12, 64, -4 in minecraft:overworld".into(),
    };
    assert_eq!(mods.drain(), [chat]);
    assert!(mods.command("coords"));
    mods.tick(2);
    assert_eq!(mods.panels().count(), 0);
    let hidden = Output::Feedback {
        text: "Coordinates panel hidden".into(),
        error: false,
    };
    assert_eq!(mods.drain(), [hidden]);
    assert!(!mods.command("gamemode creative"));
}

#[test]
fn off_mods_cost_nothing() {
    let mods = Mods::load(&[mods_dir().join("coords_hud.hmod")], |_| false, |_| false);
    assert_eq!(mods.entries()[0].status, Status::Off);
    assert!(!mods.is_running());
    assert_eq!(mods.commands().count(), 0);
}

#[test]
fn undeclared_capabilities_are_refused() {
    let dir = mods_dir().join("undeclared");
    std::fs::create_dir_all(&dir).unwrap();
    let wasm = Package::read(&mods_dir().join("horizon_mods_test_guest.hmod"))
        .unwrap()
        .wasm
        .expect("the guest has code");
    let manifest = GUEST_MANIFEST.replace("\"chat\", ", "");
    std::fs::write(dir.join("horizon-mod.toml"), manifest).unwrap();
    std::fs::write(dir.join("mod.wasm"), wasm).unwrap();
    let mods = Mods::load(&[dir], |_| true, |_| false);
    let refused = Status::Failed("uses capability chat, missing from horizon-mod.toml".into());
    assert_eq!(mods.entries()[0].status, refused);
}

/// A mod calling a function this Horizon doesn't have is refused with
/// "update Horizon", not a linking error; the test mods' levels are read
/// from the functions they import.
#[test]
fn a_mod_for_a_newer_horizon_is_refused() {
    use horizon_mods::api_level::{LEVEL, imports, needed};
    use wit_parser::{ManglingAndAbi, Resolve};
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wit = std::fs::read_to_string(root.join("crates/horizon-mod-sdk/wit/horizon.wit"))
        .unwrap()
        .replace(
            "    set-minimap: func(minimap: option<minimap>);",
            "    set-minimap: func(minimap: option<minimap>);\n    set-sky: func(on: bool);",
        );
    let mut resolve = Resolve::default();
    let package = resolve.push_str("horizon.wit", &wit).unwrap();
    let world = resolve
        .select_world(&[package], Some("horizon-ui-mod"))
        .unwrap();
    let mut module = wit_component::dummy_module(&resolve, world, ManglingAndAbi::Standard32);
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
    )
    .unwrap();
    let wasm = wit_component::ComponentEncoder::default()
        .module(&module)
        .unwrap()
        .encode()
        .unwrap();
    let functions = imports(&wasm).unwrap();
    assert!(functions.iter().any(|f| f == "render.set-sky"));
    let dir = mods_dir().join("too_new");
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = r#"
id = "too_new"
name = "Too New"
version = "0.1.0"
api = "0.1"
capabilities = ["game-read", "chat", "hud", "storage", "command", "ui", "render", "input", "recipes", "net"]
experimental = ["ui", "render", "input", "recipes"]
"#;
    std::fs::write(dir.join("horizon-mod.toml"), manifest).unwrap();
    std::fs::write(dir.join("mod.wasm"), wasm).unwrap();
    let mods = Mods::load(&[dir], |_| true, |_| false);
    let refused = "made for a newer Horizon (it uses render.set-sky): update Horizon to run it";
    assert_eq!(mods.entries()[0].status, Status::Failed(refused.into()));

    let level = |file: &str| {
        let wasm = Package::read(&mods_dir().join(file)).unwrap().wasm.unwrap();
        needed(imports(&wasm).unwrap().iter().map(String::as_str)).0
    };
    assert_eq!(level("coords_hud.hmod"), 0);
    // The UI guest replaces the inventory (`ui.replace-screen`, level 8).
    assert_eq!(level("horizon_mods_test_ui_guest.hmod"), 8);
    // Block Guard keeps data on blocks (`world.set-data`, level 9).
    assert_eq!(level("block_guard.hmod"), 9);
    // The Iron Press uses storages, timers and entities: the newest level.
    assert_eq!(level("iron_press.hmod"), LEVEL);
}

#[test]
fn vanilla_commands_win() {
    let mods = Mods::load(
        &[mods_dir().join("coords_hud.hmod")],
        |_| true,
        |c| c == "where",
    );
    assert_eq!(mods.commands().collect::<Vec<_>>(), ["coords"]);
}

#[test]
fn a_misbehaving_mod_is_stopped_alone() {
    for (command, reason) in [
        ("spin", "it took too long (an endless loop?)"),
        ("panic", "it crashed (a panic)"),
        ("grow", "it crashed (a panic)"),
    ] {
        let paths = [
            mods_dir().join("coords_hud.hmod"),
            mods_dir().join("horizon_mods_test_guest.hmod"),
        ];
        let mut mods = Mods::load(&paths, |_| true, |_| false);
        let started = Instant::now();
        assert!(mods.command(command));
        assert!(started.elapsed().as_secs() < 5, "/{command} was cut short");
        let stopped = Output::Stopped {
            name: "Test Guest".into(),
            reason: reason.into(),
        };
        assert_eq!(mods.drain(), [stopped], "/{command}");
        assert_eq!(mods.entries()[1].status, Status::Failed(reason.into()));
        // The other mod runs on, and the stopped one's command is gone.
        assert!(mods.is_running());
        assert!(!mods.command("hello"));
        mods.set_snapshot(player());
        mods.tick(1);
        assert_eq!(mods.panels().count(), 1);
    }
}

#[test]
fn hot_reload_restarts_a_changed_mod() {
    let dir = mods_dir().join("reload");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("guest.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &path).unwrap();
    let mut mods = Mods::load(std::slice::from_ref(&path), |_| true, |_| false);
    mods.tick(1);
    assert!(!mods.reload_changed(|_| false));
    // A rebuild: same bytes, newer file.
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(mods.reload_changed(|_| false));
    assert!(mods.command("hello world"));
    let chat = Output::Chat {
        name: "Test Guest".into(),
        text: "hello world after 0 ticks".into(),
    };
    assert_eq!(mods.drain(), [chat]);
}

#[test]
fn storage_persists_across_reload() {
    let dir = mods_dir().join("storage_reload");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("guest.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &path).unwrap();
    let mut mods = Mods::load(std::slice::from_ref(&path), |_| true, |_| false);
    assert!(mods.command("save remembered"));
    let saved = Output::Feedback {
        text: "saved".into(),
        error: false,
    };
    assert_eq!(mods.drain(), [saved]);

    // A rebuild (hot_reload_restarts_a_changed_mod's pattern): a fresh Store,
    // but the same on-disk data directory.
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(mods.reload_changed(|_| false));

    assert!(mods.command("load"));
    let chat = Output::Chat {
        name: "Test Guest".into(),
        text: "remembered".into(),
    };
    assert_eq!(mods.drain(), [chat]);
}

#[test]
fn storage_write_budget_limits_one_call() {
    let dir = mods_dir().join("storage_budget");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("guest.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &path).unwrap();
    let mut mods = Mods::load(std::slice::from_ref(&path), |_| true, |_| false);
    // Under the 4 MiB on-disk quota, but over the 1 MiB a single call may
    // write: a loop replacing one key can never bypass the quota by
    // writing a lot in one call, only across many.
    let over_budget = "a".repeat((1 << 20) + 1024);
    assert!(mods.command(&format!("save {over_budget}")));
    let refused = Output::Feedback {
        text: "too much storage traffic in one call".into(),
        error: true,
    };
    assert_eq!(mods.drain(), [refused]);
}

#[test]
fn storage_quota_is_enforced() {
    let dir = mods_dir().join("storage_quota");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("guest.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &path).unwrap();
    let mut mods = Mods::load(std::slice::from_ref(&path), |_| true, |_| false);
    // Each call is under the per-call write budget; distinct keys so the
    // quota (not the fixed "note" key being overwritten) is what's tested.
    let chunk = "a".repeat(900 << 10);
    for i in 0..4 {
        assert!(mods.command(&format!("save_key k{i} {chunk}")));
        assert_eq!(
            mods.drain(),
            [Output::Feedback {
                text: "saved".into(),
                error: false,
            }]
        );
    }
    assert!(mods.command(&format!("save_key k4 {chunk}")));
    let refused = Output::Feedback {
        text: "would use over 4 MiB of storage".into(),
        error: true,
    };
    assert_eq!(mods.drain(), [refused]);
}

#[test]
fn storage_key_count_is_bounded() {
    let dir = mods_dir().join("storage_keys");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("guest.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &path).unwrap();
    let mut mods = Mods::load(std::slice::from_ref(&path), |_| true, |_| false);
    for i in 0..64 {
        assert!(mods.command(&format!("save_key k{i} x")));
        assert_eq!(
            mods.drain(),
            [Output::Feedback {
                text: "saved".into(),
                error: false,
            }],
            "key {i}"
        );
    }
    assert!(mods.command("save_key k64 x"));
    let refused = Output::Feedback {
        text: "would hold over 64 keys".into(),
        error: true,
    };
    assert_eq!(mods.drain(), [refused]);
}

#[test]
fn duplicate_mod_ids_are_refused() {
    let dir = mods_dir().join("duplicate_ids");
    std::fs::create_dir_all(&dir).unwrap();
    let first = dir.join("first.hmod");
    let second = dir.join("second.hmod");
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &first).unwrap();
    std::fs::copy(mods_dir().join("horizon_mods_test_guest.hmod"), &second).unwrap();
    // Both declare id "test_guest" (the guest's own manifest): sharing a
    // `data_dir` would let the second read, overwrite or delete the
    // first's storage, so only the first-loaded one may start.
    let mods = Mods::load(&[first, second], |_| true, |_| false);
    assert_eq!(mods.entries()[0].status, Status::Running);
    let refused = Status::Failed("another running mod already uses id test_guest".into());
    assert_eq!(mods.entries()[1].status, refused);
}

/// `cargo test -p horizon-mods --release -- --ignored --nocapture`: the
/// cost of the per-tick and per-frame hooks (docs/mods.md, Performance).
#[test]
#[ignore]
fn hook_overhead() {
    let time = |mods: &mut Mods, frame: bool| {
        let n = 200_000;
        for i in 0..1000 {
            mods.tick(i);
        }
        let started = Instant::now();
        for i in 0..n {
            if frame {
                mods.frame(0.016);
            } else {
                mods.tick(i);
            }
        }
        started.elapsed().as_nanos() as f64 / n as f64
    };
    let mut guest = load("horizon_mods_test_guest.hmod");
    let mut hud = load("coords_hud.hmod");
    hud.set_snapshot(player());
    let mut none = Mods::default();
    println!("no mods, tick:        {:8.1} ns", time(&mut none, false));
    println!("empty tick hook:      {:8.1} ns", time(&mut guest, false));
    println!("empty frame hook:     {:8.1} ns", time(&mut guest, true));
    println!("coords-hud tick hook: {:8.1} ns", time(&mut hud, false));
    let started = Instant::now();
    let _ = load("coords_hud.hmod");
    println!(
        "load coords-hud:      {:8.1} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
}

#[test]
fn ui_screens_settings_and_events() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    assert_eq!(mods.entries()[0].status, Status::Running);
    assert!(!mods.entries()[0].has_ui_settings());
    // `/ui_screen` pushes the screen; the host clips nothing of this one.
    assert!(mods.command("ui_screen"));
    assert_eq!(mods.drain(), [Output::OpenScreen { owner: 0 }]);
    let screen = mods.screen(0).expect("a pushed screen");
    assert_eq!(screen.title, "Guest's screen");
    assert_eq!(screen.widgets.len(), 3);
    // What the player did comes back through `ui-events`.
    mods.ui_select(0, 7, 2);
    mods.ui_closed(0);
    assert_eq!(
        mods.drain(),
        [
            Output::Chat {
                name: "Test UI Guest".into(),
                text: "selected 7:2".into(),
            },
            Output::Chat {
                name: "Test UI Guest".into(),
                text: "closed".into(),
            },
        ]
    );
    // The settings page: empty until the mod fills it, refreshed on open.
    assert!(mods.command("ui_settings"));
    assert_eq!(mods.settings_widgets(0).len(), 3);
    assert!(mods.entries()[0].has_ui_settings());
    mods.ui_settings_opened(0);
    assert_eq!(mods.settings_widgets(0).len(), 2);
    // `/ui_close` closes the mod's own screen.
    assert!(mods.command("ui_close"));
    assert!(mods.drain().contains(&Output::CloseScreen { owner: 0 }));
    assert!(mods.screen(0).is_none());
}

/// A world of a few blocks for `world.get-data` / `set-data`.
struct FewBlocks(Vec<((i32, i32, i32), String)>);
impl horizon_mods::WorldView for FewBlocks {
    fn block(&self, _dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let found = self.0.iter().find(|(p, _)| *p == (x, y, z));
        Some(found.map_or_else(|| "minecraft:air".to_owned(), |(_, s)| s.clone()))
    }
    fn biome(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        Some("minecraft:plains".into())
    }
    fn has_dimension(&self, dimension: &str) -> bool {
        dimension == "minecraft:overworld"
    }
    fn check_state(&self, _state: &str) -> Result<(), String> {
        Ok(())
    }
}

/// A world for machines: blocks, vanilla containers, redstone and mobs.
#[derive(Default)]
struct Machines {
    blocks: Vec<((i32, i32, i32), String)>,
    containers: Vec<((i32, i32, i32), Vec<Option<HostStack>>)>,
    redstone: u8,
    mobs: Vec<EntityInfo>,
}
impl horizon_mods::WorldView for Machines {
    fn block(&self, _dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let found = self.blocks.iter().find(|(p, _)| *p == (x, y, z));
        Some(found.map_or_else(|| "minecraft:air".to_owned(), |(_, s)| s.clone()))
    }
    fn biome(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        Some("minecraft:plains".into())
    }
    fn has_dimension(&self, dimension: &str) -> bool {
        dimension == "minecraft:overworld"
    }
    fn check_state(&self, _state: &str) -> Result<(), String> {
        Ok(())
    }
    fn redstone(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<u8> {
        Some(self.redstone)
    }
    fn max_stack(&self, item: &str) -> Option<u32> {
        (item != "minecraft:nothing").then_some(64)
    }
    fn container(&self, _dimension: &str, x: i32, y: i32, z: i32) -> Option<Vec<Option<HostStack>>> {
        self.containers.iter().find(|(p, _)| *p == (x, y, z)).map(|(_, s)| s.clone())
    }
    fn set_container(&mut self, _dimension: &str, x: i32, y: i32, z: i32, slots: Vec<Option<HostStack>>) -> bool {
        match self.containers.iter_mut().find(|(p, _)| *p == (x, y, z)) {
            Some((_, at)) if at.len() == slots.len() => {
                *at = slots;
                true
            }
            _ => false,
        }
    }
    fn mobs(&self, _dimension: &str) -> Vec<EntityInfo> {
        self.mobs.clone()
    }
}
fn with_machines(mods: &mut Mods, f: impl FnOnce(&mut Machines)) {
    let view: Box<dyn std::any::Any> = mods.take_world().expect("a world lent");
    let mut view = view.downcast::<Machines>().expect("the machines");
    f(&mut view);
    mods.lend_world(view);
}
fn stack(item: &str, count: u32) -> HostStack {
    HostStack {
        item: item.into(),
        count,
        ..HostStack::default()
    }
}

// API level 10: a machine with a storage and a window, timers, redstone,
// vanilla containers, stacks that keep what the mod can't see, inventories,
// placing, item use, neighbours and entities
#[test]
fn iron_press_runs_a_machine() {
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
    let dim = "minecraft:overworld";
    let (press, above, below) = ((1, 64, 2), (1, 65, 2), (1, 63, 2));
    let at = BlockPos { x: 1, y: 64, z: 2 };
    // An ingot carrying an enchantment-like component and another mod's bytes.
    let marked = HostStack {
        components: vec![10, 0, 0],
        data: [("other_mod".to_string(), vec![9u8])].into_iter().collect(),
        ..stack("minecraft:iron_ingot", 1)
    };
    let mut slots = vec![Some(marked.clone()), Some(stack("minecraft:iron_ingot", 2))];
    slots.resize(27, None);
    let mut mods = Mods::load(&[mods_dir().join("iron_press.hmod")], |_| true, |_| false);
    assert_eq!(mods.entries()[0].status, Status::Running);
    mods.set_snapshot(Snapshot {
        players: vec![steve.clone()],
        inventories: vec![(
            steve.id.clone(),
            0,
            vec![Some(stack("minecraft:paper", 3)), Some(stack("minecraft:stone", 5))],
        )],
        ..Snapshot::default()
    });
    mods.lend_world(Box::new(Machines {
        blocks: vec![
            (press, "minecraft:iron_block".into()),
            (above, "minecraft:chest[facing=north]".into()),
            (below, "minecraft:hopper[facing=down]".into()),
        ],
        containers: vec![(above, slots), (below, vec![None; 5])],
        redstone: 0,
        mobs: vec![EntityInfo {
            id: "00000000-0000-0000-0000-00000000000a".into(),
            kind: "minecraft:zombie".into(),
            pos: (2.0, 64.0, 2.0),
            health: 20.0,
        }],
    }));

    // An empty hand declares the press's storage and opens its window.
    assert!(mods.use_block(&steve, at, "minecraft:iron_block", "minecraft:air"));
    let out = mods.drain();
    assert!(out.iter().any(|o| matches!(o, Output::Server(ServerAction::OpenMenu { title, pos, .. }) if title == "Iron Press" && *pos == press)), "{out:?}");
    assert_eq!(mods.storage_slots(dim, press).map(|s| s.len()), Some(9));
    assert_eq!(mods.machine_positions(dim), vec![press]);
    // Paper counts the player's items and gives a nugget with the mod's bytes.
    assert!(mods.use_block(&steve, at, "minecraft:iron_block", "minecraft:paper"));
    let out = mods.drain();
    assert!(out.iter().any(|o| matches!(o, Output::Tell { text, .. } if text == "You carry 8 items")));
    assert!(out.iter().any(|o| matches!(o, Output::Server(ServerAction::Give { stack, .. })
        if stack.item == "minecraft:iron_nugget" && stack.data.get("iron_press") == Some(&vec![7]))));

    // A second later (its timer): the first ingot comes in, unpowered it waits.
    for t in 1..=21 {
        mods.server_tick(t);
    }
    let held = mods.storage_slots(dim, press).unwrap();
    assert_eq!(held[0].as_ref(), Some(&marked), "what the mod can't see stays on the stack");
    // Powered, it presses it into nuggets and pushes them into the hopper below.
    with_machines(&mut mods, |m| m.redstone = 15);
    for t in 22..=41 {
        mods.server_tick(t);
    }
    let view: Box<dyn std::any::Any> = mods.take_world().unwrap();
    let view = view.downcast::<Machines>().unwrap();
    let hopper = &view.containers[1].1;
    let nuggets = hopper[0].as_ref().expect("nuggets below");
    assert_eq!((nuggets.item.as_str(), nuggets.count), ("minecraft:iron_nugget", 9));
    assert_eq!(nuggets.data.get("iron_press"), Some(&1u32.to_le_bytes().to_vec()));
    assert_eq!(view.containers[0].1[1].as_ref().map(|s| s.count), Some(1), "one more ingot pulled");
    mods.lend_world(view);

    // Saved with the world: the storage, its energy and the waiting timer.
    let saved = mods.block_data_json();
    assert!(saved.contains("\"storages\"") && saved.contains("\"ticks\""), "{saved}");
    let mut again = Mods::load(&[mods_dir().join("iron_press.hmod")], |_| true, |_| false);
    again.set_block_data_json(&saved);
    assert_eq!(again.storage_slots(dim, press), mods.storage_slots(dim, press));

    // A neighbour changing clanks.
    mods.drain();
    with_machines(&mut mods, |m| m.blocks.push(((2, 64, 2), "minecraft:stone".into())));
    mods.server_tick(42);
    assert!(mods.drain().iter().any(|o| matches!(o, Output::Server(ServerAction::Console { line }) if line.contains("playsound minecraft:block.anvil.land"))));

    // No TNT near a press; elsewhere it goes.
    assert!(mods.place_block(&steve, BlockPos { x: 3, y: 64, z: 2 }, "minecraft:tnt"));
    assert!(!mods.place_block(&steve, BlockPos { x: 30, y: 64, z: 2 }, "minecraft:tnt"));
    assert!(!mods.place_block(&steve, BlockPos { x: 3, y: 64, z: 2 }, "minecraft:stone"));

    // A blaze rod in the air hastens the mobs around.
    mods.drain();
    assert!(mods.use_item(&steve, &stack("minecraft:blaze_rod", 1), false));
    let out = mods.drain();
    assert!(out.iter().any(|o| matches!(o, Output::Server(ServerAction::Console { line })
        if line == "effect give 00000000-0000-0000-0000-00000000000a minecraft:speed 10 1")), "{out:?}");
    assert!(out.iter().any(|o| matches!(o, Output::Server(ServerAction::Console { line }) if line.contains("particle minecraft:flame"))));
    assert!(!mods.use_item(&steve, &stack("minecraft:stick", 1), false));

    // game-events: explosions near a press fizzle; a kill is told to the killer.
    assert!(mods.wants_event("explosion") && !mods.wants_event("entity-spawned"));
    assert!(mods.explosion(dim, (2.0, 64.0, 2.0), 4.0));
    assert!(!mods.explosion(dim, (40.0, 64.0, 2.0), 4.0));
    mods.drain();
    let zombie = EntityInfo {
        id: "00000000-0000-0000-0000-00000000000a".into(),
        kind: "minecraft:zombie".into(),
        pos: (2.0, 64.0, 2.0),
        health: 0.0,
    };
    mods.entity_died(dim, &zombie, Some(&steve.id));
    assert!(mods.drain().iter().any(|o| matches!(o, Output::Tell { text, .. } if text == "You defeated minecraft:zombie")));

    // net: a client mod's ping is answered on the mod's own channel.
    mods.server_net_received(&steve, "iron_press/ping", b"?");
    let out = mods.drain();
    assert!(out.iter().any(|o| matches!(o, Output::Server(ServerAction::Net { player: Some(p), channel, data })
        if *p == steve.id && channel == "iron_press/pong" && *data == 1u32.to_le_bytes().to_vec())), "{out:?}");
}

// `world.set-data`: a mod's bytes stay on the block, survive saving the world, and go with the block
#[test]
fn block_data_lives_and_dies_with_the_block() {
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
    let at = BlockPos { x: 1, y: 64, z: 2 };
    let door = |open: bool| FewBlocks(vec![((1, 64, 2), format!("minecraft:oak_door[open={open}]"))]);
    let read = |mods: &mut Mods| -> String {
        assert!(mods.use_block(&steve, at, "minecraft:oak_door", "minecraft:stick"));
        mods.drain()
            .into_iter()
            .find_map(|o| match o {
                Output::Tell { text, .. } => Some(text),
                _ => None,
            })
            .expect("a line")
    };
    let mut mods = Mods::load(&[mods_dir().join("block_guard.hmod")], |_| true, |_| false);
    mods.set_snapshot(Snapshot {
        players: vec![steve.clone()],
        ..Snapshot::default()
    });
    mods.lend_world(Box::new(door(false)));
    assert!(read(&mut mods).ends_with("(read 1 times)"));
    assert!(read(&mut mods).ends_with("(read 2 times)"));
    // A changed property is the same block: the count stays.
    mods.lend_world(Box::new(door(true)));
    assert!(read(&mut mods).ends_with("(read 3 times)"));
    // Saved with the world and back in a fresh game.
    let saved = mods.block_data_json();
    assert!(saved.contains("block_guard"));
    let mut again = Mods::load(&[mods_dir().join("block_guard.hmod")], |_| true, |_| false);
    again.set_snapshot(Snapshot {
        players: vec![steve.clone()],
        ..Snapshot::default()
    });
    again.set_block_data_json(&saved);
    again.lend_world(Box::new(door(true)));
    assert!(read(&mut again).ends_with("(read 4 times)"));
    // Another block there now: the data went with the door.
    again.lend_world(Box::new(FewBlocks(vec![((1, 64, 2), "minecraft:stone".into())])));
    assert!(read(&mut again).ends_with("(read 1 times)"));
}

// `/hmod reload`: the mods that are on start again from their files, all of them or one by id
#[test]
fn hmod_reload_restarts_mods() {
    let mut mods = Mods::load(
        &[mods_dir().join("horizon_mods_test_ui_guest.hmod"), mods_dir().join("block_guard.hmod")],
        |id| id == "test_ui_guest",
        |_| false,
    );
    assert!(mods.command("ui_replace on"));
    assert_eq!(mods.replacement_owner("minecraft:inventory"), Some(0));
    // Only the mods that are on: block_guard stays off and isn't listed.
    let done = mods.reload(None, |_| false);
    assert_eq!(done, vec![("test_ui_guest".to_string(), Ok(()))]);
    assert_eq!(mods.entries()[0].status, Status::Running);
    assert_eq!(mods.entries()[1].status, Status::Off);
    // It started from init again: what it held in memory is gone.
    assert_eq!(mods.replacement_owner("minecraft:inventory"), None);
    assert_eq!(mods.reload(Some("test_ui_guest"), |_| false).len(), 1);
    assert!(mods.reload(Some("no_such_mod"), |_| false).is_empty());
}

// `block-events`: the server asks a mod before it breaks or uses a block, and the mod's answer decides it
#[test]
fn block_events_refuse_and_take() {
    let mut mods = Mods::load(&[mods_dir().join("block_guard.hmod")], |_| true, |_| false);
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
    let at = BlockPos { x: 1, y: 64, z: 2 };
    // A guarded block stays, and the player is told why; stone breaks as usual.
    assert!(mods.break_block(&steve, at, "minecraft:diamond_block"));
    assert!(mods.drain().iter().any(|o| matches!(o, Output::Tell { text, .. } if text == "This block is guarded")));
    assert!(!mods.break_block(&steve, at, "minecraft:stone"));
    // A stick reads the block instead of using it; anything else is vanilla's.
    assert!(mods.use_block(&steve, at, "minecraft:oak_door[open=false]", "minecraft:stick"));
    assert!(mods
        .drain()
        .iter()
        .any(|o| matches!(o, Output::Tell { text, .. } if text.starts_with("1 64 2: minecraft:oak_door[open=false]"))));
    assert!(!mods.use_block(&steve, at, "minecraft:oak_door[open=false]", "minecraft:air"));
    // A server mod that didn't register `blocks` is not asked.
    let mut beacons = Mods::load(&[mods_dir().join("beacons.hmod")], |_| true, |_| false);
    assert!(!beacons.break_block(&steve, at, "minecraft:diamond_block"));
}

// `ui.replace-screen`: the mod's screen opens in the inventory's place, not over it, until it gives it back
#[test]
fn ui_replaces_the_inventory() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    assert_eq!(mods.replacement_owner("minecraft:inventory"), None);
    assert!(mods.command("ui_replace on"));
    assert_eq!(mods.replacement_owner("minecraft:inventory"), Some(0));
    // Replacing opens nothing yet: the game shows it when the player opens the inventory.
    assert!(mods.screen(0).is_none());
    assert!(mods.show_replacement(0, "minecraft:inventory"));
    assert_eq!(mods.screen(0).expect("the replacement").title, "Guest's inventory");
    assert!(!mods.show_replacement(0, "minecraft:crafting_table"));
    // A screen this Horizon can't replace is the call's error, and changes nothing.
    assert!(mods.command("ui_replace bad"));
    assert!(mods
        .drain()
        .iter()
        .any(|o| matches!(o, Output::Feedback { text, error: true } if text.contains("can't be replaced"))));
    assert_eq!(mods.replacement_owner("minecraft:pause"), None);
    // Given back: the game opens its own inventory again.
    assert!(mods.command("ui_replace off"));
    assert_eq!(mods.replacement_owner("minecraft:inventory"), None);
}

#[test]
fn render_shapes_and_shakes() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    mods.set_snapshot(Snapshot {
        gui_size: (427.0, 240.0),
        ..Snapshot::default()
    });
    assert_eq!(mods.hud_shapes().count(), 0);
    assert!(mods.command("render_hud"));
    let shapes: Vec<_> = mods.hud_shapes().cloned().collect();
    assert_eq!(shapes.len(), 5);
    // The guest read `gui-size`; text is clipped to 256 characters.
    let Shape::Text(t) = &shapes[4] else {
        panic!("{:?}", shapes[4])
    };
    assert!(t.text.starts_with("gui 427x240 "), "{}", t.text);
    assert_eq!(t.text.chars().count(), 256);
    assert!(mods.command("render_clear"));
    assert_eq!(mods.hud_shapes().count(), 0);
    // One jolt, clamped; zero and NaN shake nothing.
    mods.drain();
    assert!(mods.command("render_shake"));
    assert_eq!(
        mods.drain(),
        [Output::Shake {
            strength: 1.0,
            seconds: 10.0
        }]
    );
}

#[test]
fn input_claims_keys_and_events() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let chat = |mods: &mut Mods| -> Vec<String> {
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    assert!(!mods.claims_any() && !mods.has_keys());
    assert!(mods.command("input_claim"));
    let said = chat(&mut mods);
    assert!(
        said[0].starts_with("claimed; \"R\" is not a key"),
        "{said:?}"
    );
    assert!(mods.claims("minecraft:stick") && !mods.claims("minecraft:stone"));
    let keys: Vec<_> = mods
        .mod_keys()
        .map(|(m, k)| (m.to_owned(), k.key.clone()))
        .collect();
    assert_eq!(
        keys,
        [("Test UI Guest".to_owned(), "key.keyboard.r".to_owned())]
    );
    // The game feeds the events; the mod reads them in order with `poll`.
    let stick = HeldItem {
        id: "minecraft:stick".into(),
        count: 3,
        slot: 2,
    };
    mods.use_start(&stick);
    mods.key("key.keyboard.r", true);
    mods.key("key.keyboard.t", true);
    mods.key("key.keyboard.r", false);
    mods.use_stop(&stick, 12);
    assert!(mods.command("input_poll"));
    assert_eq!(
        chat(&mut mods),
        [
            "use-start minecraft:stick 0",
            "key-down wheel",
            "key-up wheel",
            "use-stop minecraft:stick 12"
        ]
    );
    // `held` reads the snapshot.
    mods.set_snapshot(Snapshot {
        held: Some(stick),
        ..Snapshot::default()
    });
    assert!(mods.command("input_held"));
    assert_eq!(chat(&mut mods), ["holding minecraft:stick x3 in 2"]);
    // A screen opening lets held keys up, once; the release that follows
    // is not sent again.
    mods.key("key.keyboard.r", true);
    mods.release_keys();
    mods.release_keys();
    mods.key("key.keyboard.r", false);
    assert!(mods.command("input_poll"));
    assert_eq!(chat(&mut mods), ["key-down wheel", "key-up wheel"]);
}

#[test]
fn the_players_key_choice_wins() {
    let chat = |mods: &mut Mods| -> Vec<String> {
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    assert!(mods.command("input_claim"));
    // Its keys alone give it a page in Options > Mods, where the game lists them.
    assert!(mods.settings_widgets(0).is_empty() && mods.entries()[0].has_ui_settings());
    // Rebinding while held lets the old key go.
    mods.key("key.keyboard.r", true);
    assert!(mods.rebind(0, "wheel", "key.mouse.4"));
    assert_eq!(mods.keys_of(0)[0].key, "key.mouse.4");
    mods.key("key.keyboard.r", false);
    mods.key("key.mouse.4", true);
    assert!(!mods.rebind(0, "wheel", "R") && !mods.rebind(0, "nope", "key.keyboard.g"));
    chat(&mut mods);
    assert!(mods.command("input_poll"));
    assert_eq!(
        chat(&mut mods),
        ["key-down wheel", "key-up wheel", "key-down wheel"]
    );
    // A choice saved earlier replaces the default, registered before or after.
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    mods.set_bindings(|_| vec![("wheel", "key.keyboard.g"), ("bad", "G")]);
    assert!(mods.command("input_claim"));
    assert_eq!(mods.keys_of(0)[0].key, "key.keyboard.g");
    mods.set_bindings(|_| vec![("wheel", "key.keyboard.h")]);
    assert_eq!(mods.keys_of(0)[0].key, "key.keyboard.h");
}

#[test]
fn input_wheel_is_the_mods_while_claimed() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let chat = |mods: &mut Mods| -> Vec<String> {
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    // Unclaimed, the hotbar keeps the wheel and the mod hears nothing.
    assert!(!mods.wheel_claimed() && !mods.wheel(1.0));
    assert!(mods.command("input_wheel"));
    assert!(mods.wheel_claimed());
    assert!(mods.wheel(1.0) && mods.wheel(-2.0));
    assert!(mods.command("input_poll"));
    assert_eq!(chat(&mut mods), ["wheel 1", "wheel -2"]);
    assert!(mods.command("input_wheel off"));
    assert!(!mods.wheel(1.0));
}

#[test]
fn render_zoom_is_clamped_and_the_strongest_counts() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let zoom = |mods: &mut Mods, args: &str| {
        assert!(mods.command(&format!("zoom {args}")));
        mods.zoom().map(|z| (z.factor, z.smooth, z.turn))
    };
    assert!(mods.zoom().is_none());
    assert_eq!(zoom(&mut mods, "4 1 0.25"), Some((4.0, true, 0.25)));
    // Over 10 and a turn under 0.05 are clamped.
    assert_eq!(zoom(&mut mods, "50 0 0"), Some((10.0, false, 0.05)));
    // A NaN turn keeps the player's sensitivity.
    assert_eq!(zoom(&mut mods, "2 0 x"), Some((2.0, false, 1.0)));
    // 1, under 1 and NaN end the zoom.
    assert_eq!(zoom(&mut mods, "1 0 1"), None);
    assert_eq!(zoom(&mut mods, "0.5 0 1"), None);
    assert_eq!(zoom(&mut mods, "x 0 1"), None);
}

#[test]
fn render_minimap_is_clamped_and_hidden() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let map = |mods: &mut Mods, args: &str| {
        assert!(mods.command(&format!("minimap {args}")));
        mods.minimap()
            .map(|m| (m.corner, m.size, m.blocks_per_pixel, m.rotate, m.round))
    };
    assert!(mods.minimap().is_none());
    let corner = horizon_mods::RenderCorner::BottomLeft;
    assert_eq!(
        map(&mut mods, "96 2 1 0"),
        Some((corner, 96.0, 2.0, true, false))
    );
    // The side and the scale are clamped; NaN takes the smallest.
    assert_eq!(
        map(&mut mods, "999 0.1 0 1"),
        Some((corner, 256.0, 0.5, false, true))
    );
    assert_eq!(
        map(&mut mods, "x 9 0 0"),
        Some((corner, 16.0, 4.0, false, false))
    );
    assert_eq!(map(&mut mods, "off"), None);
}

#[test]
fn render_far_terrain_is_clamped_and_ended() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let far = |mods: &mut Mods, args: &str| {
        assert!(mods.command(&format!("far_terrain {args}")));
        mods.far_terrain().map(|f| (f.distance, f.reduced))
    };
    assert!(mods.far_terrain().is_none());
    assert_eq!(far(&mut mods, "256 1"), Some((256, true)));
    // The distance is clamped to LOD Distance's 64 to 1024 chunks.
    assert_eq!(far(&mut mods, "8 0"), Some((64, false)));
    assert_eq!(far(&mut mods, "99999 0"), Some((1024, false)));
    assert_eq!(far(&mut mods, "off"), None);
}

/// `game.raycast`: the view's hit the game worked out, up to the distance
/// asked for; the game works it out only while a mod imports the call.
#[test]
fn game_raycast_sees_the_view_hit_within_its_distance() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    assert!(mods.wants_raycast());
    assert!(!load("coords_hud.hmod").wants_raycast());
    let said = |mods: &mut Mods, args: &str| -> Vec<String> {
        assert!(mods.command(&format!("raycast {args}")));
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    };
    assert_eq!(said(&mut mods, "100"), ["no hit"]);
    mods.set_snapshot(Snapshot {
        view_hit: Some((
            40.5,
            Hit {
                block_x: 12,
                block_y: 63,
                block_z: -44,
                x: 12.5,
                y: 64.0,
                z: -43.75,
                block: "minecraft:grass_block".into(),
            },
        )),
        ..player()
    });
    let hit = "hit minecraft:grass_block at 12 63 -44 (12.5 64.0 -43.8)";
    assert_eq!(said(&mut mods, "100"), [hit]);
    assert_eq!(said(&mut mods, "40"), ["no hit"]);
    // NaN asks for nothing; past 1024 is 1024.
    assert_eq!(said(&mut mods, "x"), ["no hit"]);
    assert_eq!(said(&mut mods, "1e9"), [hit]);
}

#[test]
fn quiet_commands_come_out_as_one_batch_capped() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let quiet = |mods: &mut Mods, command: &str| -> Vec<(String, Vec<String>)> {
        assert!(mods.command(command));
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::QuietCommands { mod_id, lines } => Some((mod_id, lines)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        quiet(&mut mods, "quiet setblock 0 0 0 stone;say hi"),
        [(
            "test_ui_guest".to_string(),
            vec!["setblock 0 0 0 stone".to_string(), "say hi".to_string()]
        )]
    );
    let batches = quiet(&mut mods, "quiet_many 300");
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].1.len(), 256);
    assert_eq!(batches[0].1[255], "setblock 255 300 0 stone");
}

#[test]
fn corner_panels_text_width_and_readings() {
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    // A font of 5-pixel glyphs, 3-pixel spaces.
    let mut widths = vec![5.0f32; 0x250];
    widths[' ' as usize] = 3.0;
    mods.set_snapshot(Snapshot {
        frame: (60, 41, 52),
        mappings: vec![("key.forward", "W".into(), true)],
        cps: (7, 2),
        font_widths: widths.into(),
        ..Snapshot::default()
    });
    assert!(mods.command("corner"));
    let panels: Vec<_> = mods.corner_panels().cloned().collect();
    assert_eq!(panels.len(), 1);
    // "60 fps": five glyphs and a space, plus the box's 4.
    assert_eq!(
        (panels[0].width, panels[0].height),
        (5.0 * 5.0 + 3.0 + 4.0, 12.0)
    );
    assert_eq!(panels[0].shapes.len(), 2);
    assert!(matches!(
        panels[0].corner,
        horizon_mods::RenderCorner::TopRight
    ));
    assert!(matches!(
        &mods.settings_widgets(0)[0],
        horizon_mods::UiWidget::Button(b) if b.id == 3 && b.label == "Corner: Top Right"
    ));
    mods.drain();
    assert!(mods.command("readings"));
    assert_eq!(
        mods.drain(),
        [Output::Chat {
            name: "Test UI Guest".into(),
            text: "60 41 52; forward W true; cps 7 2; other true".into(),
        }]
    );
}

#[test]
fn content_only_mods_load_without_an_engine() {
    let manifest = r#"
id = "ruby"
name = "Ruby"
version = "0.1.0"
api = "0.2"
side = "both"
capabilities = ["registries"]
"#;
    let block: &[u8] = br#"{"template": "minecraft:iron_block"}"#;
    let dir = mods_dir().join("content");
    std::fs::create_dir_all(&dir).unwrap();
    let ok = dir.join("ruby.hmod");
    let files: &[(&str, &[u8])] = &[("data/ruby/horizon/block/ruby_block.json", block)];
    std::fs::write(&ok, Package::write_files(manifest, None, files).unwrap()).unwrap();
    let bad = dir.join("bad.hmod");
    let files: &[(&str, &[u8])] = &[("data/minecraft/recipe/stone.json", b"{}")];
    let bad_manifest = manifest.replace("\"ruby\"", "\"bad\"");
    std::fs::write(
        &bad,
        Package::write_files(&bad_manifest, None, files).unwrap(),
    )
    .unwrap();
    let mods = Mods::load(&[ok, bad], |_| true, |_| false);
    assert_eq!(mods.entries()[0].status, Status::Running);
    assert!(matches!(&mods.entries()[1].status, Status::Failed(e) if e.contains("own namespace")));
    // Content has no hooks: the game's per-tick mod work stays skipped.
    assert!(!mods.is_running());
}

/// `cargo horizon-mod test` on the examples: their own `tests/*.toml` pass.
#[test]
fn the_examples_test_scripts_pass() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-mod-sdk/examples");
    let mut ran = 0;
    let examples_and_files = [
        ("coords-hud", "coords_hud"),
        ("welcome", "welcome"),
        ("beacons", "beacons"),
    ];
    for (example, hmod) in examples_and_files {
        for entry in std::fs::read_dir(examples.join(example).join("tests")).unwrap() {
            let path = entry.unwrap().path();
            let script = Script::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let mut game = TestGame::load(&mods_dir().join(format!("{hmod}.hmod"))).unwrap();
            script
                .run(&mut game)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            ran += 1;
        }
    }
    assert_eq!(ran, 5);
}

/// Capability `world` without a world lent (the registry's trial run):
/// `get-block` finds nothing, `set-blocks` checks its count and queues.
#[test]
fn a_server_mod_without_a_world_reads_nothing() {
    let mut game = TestGame::load(&mods_dir().join("beacons.hmod")).unwrap();
    game.join("Alex");
    let alex = game.players[0].clone();
    assert!(game.player_command(&alex, "ground"));
    let out = game.drain();
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::Reply { text, error: true, .. }
            if text == "That block isn't loaded")),
        "{out:?}"
    );
}

#[test]
fn a_test_script_says_which_step_failed_and_why() {
    let script = Script::parse(
        r#"
        [[step]]
        ticks = 3
        command = "hello there"
        expect.chat = ["hello there after 2 ticks"]
        "#,
    )
    .unwrap();
    let mut game = TestGame::load(&mods_dir().join("horizon_mods_test_guest.hmod")).unwrap();
    assert_eq!(
        script.run(&mut game).unwrap_err(),
        "step 1 (3 ticks, /hello there): chat\n    expected: [\"hello there after 2 ticks\"]\n    \
         got:      [\"hello there after 3 ticks\"]"
    );
    assert!(
        Script::parse("[[step]]\ntick = 1").is_err(),
        "a typo is an error"
    );

    let mut game = TestGame::load(&mods_dir().join("welcome.hmod")).unwrap();
    let script = "[[step]]\njoin = \"Alex\"\n\n[[step]]\nas = \"Steve\"\ncommand = \"mined\"";
    assert_eq!(
        Script::parse(script).unwrap().run(&mut game).unwrap_err(),
        "step 2 (/mined): Steve is not on the server"
    );
}

#[test]
fn a_test_script_reloads_and_expects_a_stop() {
    let script = Script::parse(
        r#"
        [[step]]
        command = "save kept"
        expect.feedback = "saved"

        [[step]]
        reload = true
        command = "load"
        expect.chat = ["kept"]

        [[step]]
        command = "panic"
        expect.stopped = "panic"

        [[step]]
        reload = true
        command = "nope"
        "#,
    )
    .unwrap();
    let mut game = TestGame::load(&mods_dir().join("horizon_mods_test_guest.hmod")).unwrap();
    let dir = game.hmod().parent().unwrap().to_path_buf();
    assert_eq!(
        script.run(&mut game).unwrap_err(),
        "step 4 (reload, /nope): the mod owns no command /nope"
    );
    drop(game);
    assert!(
        !dir.exists(),
        "the scratch copy and its storage are removed"
    );
}

fn alex() -> ServerPlayer {
    ServerPlayer {
        id: "00000000-0000-4000-8000-000000000001".into(),
        name: "Alex".into(),
        x: 0.5,
        y: 64.0,
        z: 0.5,
        dimension: "minecraft:the_nether".into(),
        game_mode: GameMode::Survival,
        health: 20.0,
    }
}

#[test]
fn a_server_mod_runs_only_where_the_world_does() {
    // A fresh folder: what the mod stored in an earlier run is beside it.
    let dir = mods_dir().join("server");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("welcome.hmod");
    std::fs::copy(mods_dir().join("welcome.hmod"), &path).unwrap();
    let paths = std::slice::from_ref(&path);
    let window = Mods::load_for(Runs::Client, paths, |_| true, |_| false);
    assert_eq!(window.entries()[0].status, Status::Elsewhere);
    assert!(!window.is_running());

    let mut mods = Mods::load_for(Runs::Server, paths, |_| true, |c| c == "give");
    assert!(mods.runs_server());
    let commands: Vec<_> = mods.server_commands().map(|c| c.name.as_str()).collect();
    assert_eq!(commands, ["online", "mined"]);
    // `/mined [player]`: its argument travels with it, for the command tree.
    let mined = mods.server_commands().nth(1).unwrap();
    assert_eq!(mined.arguments[0].kind, horizon_mods::ArgumentKind::Player);
    assert!(mined.arguments[0].optional);
    assert_eq!(mods.commands().count(), 0, "not the window's chat commands");
    let alex = alex();
    mods.set_snapshot(Snapshot {
        players: vec![alex.clone()],
        ..Snapshot::default()
    });
    mods.player_joined(&alex);
    let greeted = [
        Output::Tell {
            name: "Welcome".into(),
            player: alex.id.clone(),
            text: "Welcome, Alex! 0 other player(s) online.".into(),
        },
        Output::Broadcast {
            name: "Welcome".into(),
            text: "Alex is here for the first time".into(),
        },
    ];
    assert_eq!(mods.drain(), greeted);
    let stone = BlockPos { x: 1, y: 63, z: 0 };
    mods.block_broken(&alex, stone, "minecraft:stone");
    assert!(mods.server_command(&alex, "mined"));
    assert!(mods.server_command(&alex, "online"));
    let reply = |text: &str| Output::Reply {
        player: alex.id.clone(),
        text: text.into(),
        error: false,
    };
    assert_eq!(
        mods.drain(),
        [reply("You broke 1 block(s)"), reply("Alex (the_nether)")]
    );
    assert!(!mods.server_command(&alex, "where"));
    assert!(
        !mods.command("online"),
        "a server command isn't the window's"
    );
}

/// Capability `recipes`: the guest reads tables as big as vanilla's (1,500
/// items, 1,600 recipes of nine five-item slots) within one command's fuel.
#[test]
fn recipes_reads_the_tables() {
    use horizon_mods::{Recipe, RecipeItem, RecipeKind, RecipeStack, Recipes};
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let stack = |item| RecipeStack {
        item,
        count: 1,
        components: String::new(),
    };
    let items = (0..1500)
        .map(|i| RecipeItem {
            id: format!("minecraft:item_{i}"),
            name: format!("Item {i}"),
            tags: vec!["planks".into(), "logs".into()],
        })
        .collect();
    let mut recipes: Vec<Recipe> = (0..1600)
        .map(|i| Recipe {
            id: format!("minecraft:recipe_{i}"),
            kind: RecipeKind::CraftingShaped,
            category: "crafting_misc".into(),
            group: String::new(),
            width: 3,
            height: 3,
            slots: (0..9)
                .map(|k| (0..5).map(|j| stack((i + k + j) % 1500)).collect())
                .collect(),
            results: vec![RecipeStack {
                item: 7,
                count: 4,
                components: String::new(),
            }],
            cooking_time: 0,
            experience: 0.0,
            book_id: i as i32,
        })
        .collect();
    recipes[1].kind = RecipeKind::Brewing;
    recipes[1].results[0].components =
        "[minecraft:potion_contents={potion:\"minecraft:swiftness\"}]".into();
    let mods_recipes = Some(std::sync::Arc::new(Recipes { items, recipes }));
    mods.set_snapshot(Snapshot {
        recipes: mods_recipes.clone(),
        known_recipes: vec![1, 5, 9].into(),
        ..Snapshot::default()
    });
    let said = |mods: &mut Mods, command: &str| {
        assert!(mods.command(command));
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        said(&mut mods, "recipes"),
        ["1500 items, 1600 recipes, 3 known, 1 brewing"]
    );
    assert_eq!(
        said(&mut mods, "recipes minecraft:recipe_1"),
        [
            "minecraft:recipe_1 Kind::Brewing 3x3 0t slots 9, the first 1 minecraft:item_1 -> \
          4 minecraft:item_7[minecraft:potion_contents={potion:\"minecraft:swiftness\"}] known true"
        ]
    );
    assert!(
        mods.entries()
            .iter()
            .all(|e| e.status == horizon_mods::Status::Running),
        "it ran out of fuel"
    );
    // Actions: refused without a crafting screen, for an unknown recipe,
    // and outside creative mode.
    assert_eq!(
        said(&mut mods, "recipe_place minecraft:recipe_5"),
        ["place minecraft:recipe_5: no screen that takes recipes is open"]
    );
    let mut snap = Snapshot {
        recipes: mods_recipes.clone(),
        known_recipes: vec![1, 5, 9].into(),
        placing: true,
        ..Snapshot::default()
    };
    mods.set_snapshot(snap.clone());
    assert_eq!(
        said(&mut mods, "recipe_place minecraft:recipe_6"),
        ["place minecraft:recipe_6: recipe 6 is not in the player's recipe book"]
    );
    assert!(mods.command("recipe_place minecraft:recipe_5 all"));
    assert_eq!(
        mods.drain()[0],
        Output::PlaceRecipe {
            book_id: 5,
            all: true
        }
    );
    assert_eq!(
        said(&mut mods, "recipe_give minecraft:item_7 3"),
        ["give minecraft:item_7: only in creative mode"]
    );
    snap.creative = true;
    mods.set_snapshot(snap);
    assert!(mods.command("recipe_give minecraft:item_7 3"));
    assert_eq!(
        mods.drain()[0],
        Output::Give {
            item: "minecraft:item_7".into(),
            count: 3,
            components: String::new()
        }
    );
    // Without the game's tables (no mod read them yet): empty, not a trap.
    mods.set_snapshot(Snapshot::default());
    assert_eq!(
        said(&mut mods, "recipes"),
        ["0 items, 0 recipes, 0 known, 0 brewing"]
    );
}

#[test]
fn browser_lists_items_and_hears_the_player() {
    use horizon_mods::{BrowserClick, BrowserEvent, BrowserStack};
    let mut mods = load("horizon_mods_test_ui_guest.hmod");
    let said = |mods: &mut Mods, command: &str| {
        assert!(mods.command(command));
        mods.drain()
            .into_iter()
            .filter_map(|o| match o {
                Output::Chat { text, .. } => Some(text),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert!(mods.browser_items().is_none());
    let potion = "[minecraft:potion_contents={potion:\"minecraft:swiftness\"}]";
    assert_eq!(
        said(
            &mut mods,
            &format!("browser minecraft:stone minecraft:potion{potion}")
        ),
        ["panel of 2"]
    );
    let (owner, version, items) = mods.browser_items().unwrap();
    assert_eq!(
        (items[0].item.as_str(), items[1].components.as_str()),
        ("minecraft:stone", potion)
    );
    mods.browser_event(
        owner,
        BrowserEvent::Clicked(BrowserClick {
            index: 1,
            right: true,
        }),
    );
    mods.browser_event(owner, BrowserEvent::Search("oak".into()));
    let log = BrowserStack {
        item: "minecraft:oak_log".into(),
        count: 3,
        components: String::new(),
    };
    // R is the guest's `recipe`; X is none of its keys.
    mods.browser_key("key.keyboard.r", &log);
    mods.browser_key("key.keyboard.x", &log);
    assert_eq!(
        said(&mut mods, "browser_poll"),
        [
            "3 events",
            "clicked 1 right",
            "search \"oak\"",
            "key recipe on minecraft:oak_log x3"
        ]
    );
    // Each `set-items` is a new version; an empty list hides the panel.
    said(&mut mods, "browser minecraft:dirt");
    assert_ne!(mods.browser_items().unwrap().1, version);
    assert_eq!(said(&mut mods, "browser"), ["panel of 0"]);
    assert!(mods.browser_items().is_none());
    // Recipes in the panel's place: the guest shows those making planks.
    use horizon_mods::{BrowserLayout, Recipe, RecipeItem, RecipeKind, RecipeStack, Recipes};
    let item = |id: &str| RecipeItem {
        id: id.into(),
        name: id.into(),
        tags: Vec::new(),
    };
    let stack = |item| RecipeStack {
        item,
        count: 1,
        components: String::new(),
    };
    mods.set_snapshot(Snapshot {
        recipes: Some(std::sync::Arc::new(Recipes {
            items: vec![item("minecraft:oak_log"), item("minecraft:oak_planks")],
            recipes: vec![Recipe {
                id: "minecraft:oak_planks".into(),
                kind: RecipeKind::CraftingShapeless,
                category: "building".into(),
                group: "planks".into(),
                width: 0,
                height: 0,
                slots: vec![vec![stack(0)]],
                results: vec![RecipeStack {
                    count: 4,
                    ..stack(1)
                }],
                cooking_time: 0,
                experience: 0.0,
                book_id: 1,
            }],
        })),
        ..Snapshot::default()
    });
    said(&mut mods, "browser minecraft:dirt");
    assert_eq!(
        said(&mut mods, "browser_recipes minecraft:oak_planks"),
        ["recipes shown: 1"]
    );
    let page = mods.browser_recipes(owner).unwrap();
    assert_eq!(page.title, "Recipes for minecraft:oak_planks");
    assert_eq!(
        (page.tabs[0].item.as_str(), page.tab),
        ("minecraft:crafting_table", 0)
    );
    let r = &page.recipes[0];
    assert_eq!(
        (r.layout, r.width, r.height, r.action),
        (BrowserLayout::Grid, 1, 1, Some(false))
    );
    assert_eq!(
        (r.slots[0][0].item.as_str(), r.results[0].count),
        ("minecraft:oak_log", 4)
    );
    assert_eq!(said(&mut mods, "browser_recipes"), ["recipes shown: 0"]);
    assert!(mods.browser_recipes(owner).is_none());
    // At most 256 events wait for a poll: the oldest go.
    for _ in 0..300 {
        mods.browser_event(owner, BrowserEvent::Page(1));
    }
    assert_eq!(said(&mut mods, "browser_poll")[0], "256 events");
}
