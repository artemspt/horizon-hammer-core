//! Example Horizon server mod for `block-events` (`docs/mods.md`): the
//! server asks it before it acts on a block. Nobody breaks a diamond block,
//! and a right click with a stick tells the player what the block is
//! instead of using it (a door stays shut, a chest closed), and counts how
//! often each block was read: a number kept on the block (`world.set-data`),
//! saved with the world and gone with the block.
use horizon_mod_sdk::server::{BlockPos, Player, Registration, ServerMod, players, world};

struct BlockGuard;

/// The block nobody may break.
const GUARDED: &str = "minecraft:diamond_block";

impl ServerMod for BlockGuard {
    fn init() -> Result<(Self, Registration), String> {
        Ok((BlockGuard, Registration::new().on_blocks()))
    }

    fn break_block(&mut self, player: &Player, _pos: BlockPos, state: &str) -> bool {
        let guarded = state == GUARDED;
        if guarded {
            players::tell(&player.id, "This block is guarded");
        }
        guarded
    }

    fn use_block(&mut self, player: &Player, pos: BlockPos, state: &str, item: &str) -> bool {
        if item != "minecraft:stick" {
            return false;
        }
        let (dim, x, y, z) = (player.dimension.as_str(), pos.x, pos.y, pos.z);
        let reads = world::get_data(dim, x, y, z)
            .and_then(|d| d.try_into().ok())
            .map_or(0, u32::from_le_bytes)
            + 1;
        let _ = world::set_data(dim, x, y, z, &reads.to_le_bytes());
        players::tell(&player.id, &format!("{x} {y} {z}: {state} (read {reads} times)"));
        true
    }
}
horizon_mod_sdk::export_server_mod!(BlockGuard);
