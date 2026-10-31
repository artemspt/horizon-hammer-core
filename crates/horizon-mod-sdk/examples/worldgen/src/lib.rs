//! Example Horizon server mod for `worldgen` (`docs/mods.md`): the mod sets
//! the surface block of a biome, and the game uses it when it builds the
//! terrain. The rule lives as long as the mod runs; a reload sets it again.
use horizon_mod_sdk::server::{Registration, ServerMod, worldgen};
use horizon_mod_sdk::storage;

struct Worldgen;

impl ServerMod for Worldgen {
    fn init() -> Result<(Self, Registration), String> {
        let surface = storage::config("surface").ok_or("no surface in the manifest")?;
        worldgen::set_surface_rule("minecraft:plains", &surface)?;
        worldgen::add_ore(&worldgen::Ore {
            biome: "*".into(),
            block: "minecraft:emerald_ore".into(),
            replaces: vec!["minecraft:stone".into(), "minecraft:deepslate".into()],
            min_y: -32,
            max_y: 64,
            size: 6,
            per_chunk: 4,
        })?;
        Ok((Worldgen, Registration::default()))
    }
}

horizon_mod_sdk::export_server_mod!(Worldgen);
