use horizon_mod_sdk::server::{Registration, ServerMod, worldgen};

struct TestWorldgen;

impl ServerMod for TestWorldgen {
    fn init() -> Result<(Self, Registration), String> {
        worldgen::set_surface_rule("minecraft:plains", "minecraft:diamond_block")?;
        Ok((TestWorldgen, Registration::default()))
    }
}

horizon_mod_sdk::export_server_mod!(TestWorldgen);
