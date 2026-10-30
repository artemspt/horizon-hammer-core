//! Example Horizon server mod for `worldgen` (`docs/mods.md`): the mod sets
//! the surface block of a biome, and the game uses it when it builds the
//! terrain. The rule lives as long as the mod runs; a reload sets it again.
use horizon_mod_sdk::server::{Registration, ServerMod, worldgen};

struct Worldgen;

impl ServerMod for Worldgen {
    fn init() -> Result<(Self, Registration), String> {
        worldgen::set_surface_rule("minecraft:plains", "minecraft:diamond_block")?;
        Ok((Worldgen, Registration::default()))
    }
}

horizon_mod_sdk::export_server_mod!(Worldgen);
