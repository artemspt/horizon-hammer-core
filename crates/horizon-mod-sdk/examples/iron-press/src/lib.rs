//! Example Horizon server mod for machines (`docs/mods.md`, API level 10):
//! an iron block becomes a press. A right click with an empty hand gives it
//! a storage (9 slots, energy, a tank) and opens its window. Every second
//! (`world.schedule-tick`) it pulls iron ingots from a container above,
//! charges from redstone, presses an ingot into 9 nuggets marked with how
//! many it pressed (bytes on the stack) and pushes them into a container
//! below: chests, hoppers and other mods' storages alike. A neighbour
//! changing clanks (`entities.sound`), TNT can't go near a press, a blaze
//! rod in the air hastens the mobs around, and paper counts your items.
use horizon_mod_sdk::server::{
    BlockPos, ItemStack, Player, Registration, ServerMod, StorageShape, entities, players, storages,
    world,
};

struct IronPress;

const PRESS: &str = "minecraft:iron_block";
const INGOT: &str = "minecraft:iron_ingot";
const NUGGET: &str = "minecraft:iron_nugget";
/// FE a second while powered, and per ingot pressed.
const CHARGE: u64 = 200;
const COST: u64 = 100;

fn shape() -> StorageShape {
    StorageShape {
        slots: 9,
        energy_capacity: 10_000,
        tanks: vec![4_000],
    }
}

/// How many ingots this press has pressed, kept on the block.
fn pressed(dim: &str, p: BlockPos) -> u32 {
    world::get_data(dim, p.x, p.y, p.z)
        .and_then(|d| d.try_into().ok())
        .map_or(0, u32::from_le_bytes)
}

impl IronPress {
    fn work(dim: &str, p: BlockPos) {
        let (x, y, z) = (p.x, p.y, p.z);
        // Ingots from the container above.
        if let Some(ingot) = storages::extract_item(dim, x, y + 1, z, Some(INGOT), 1, true)
            && storages::insert_item(dim, x, y, z, &ingot, true) == ingot.count
            && let Some(ingot) = storages::extract_item(dim, x, y + 1, z, Some(INGOT), 1, false)
        {
            storages::insert_item(dim, x, y, z, &ingot, false);
        }
        if world::redstone_power(dim, x, y, z).unwrap_or(0) > 0 {
            storages::insert_energy(dim, x, y, z, CHARGE, false);
        }
        let charged = storages::energy(dim, x, y, z).is_some_and(|(e, _)| e >= COST);
        if charged && storages::extract_item(dim, x, y, z, Some(INGOT), 1, false).is_some() {
            storages::extract_energy(dim, x, y, z, COST, false);
            let n = pressed(dim, p) + 1;
            let _ = world::set_data(dim, x, y, z, &n.to_le_bytes());
            let nuggets = ItemStack::new(NUGGET, 9).with_data(n.to_le_bytes().to_vec());
            storages::insert_item(dim, x, y, z, &nuggets, false);
        }
        // Nuggets into the container below.
        if let Some(out) = storages::extract_item(dim, x, y, z, Some(NUGGET), 64, true) {
            let moved = storages::insert_item(dim, x, y - 1, z, &out, false);
            if moved > 0 {
                storages::extract_item(dim, x, y, z, Some(NUGGET), moved, false);
            }
        }
    }
}

impl ServerMod for IronPress {
    fn init() -> Result<(Self, Registration), String> {
        Ok((IronPress, Registration::new().on_blocks()))
    }

    fn use_block(&mut self, player: &Player, pos: BlockPos, state: &str, item: &str) -> bool {
        let dim = player.dimension.as_str();
        if state != PRESS {
            return false;
        }
        if item == "minecraft:paper" {
            let items: u32 = players::inventory(&player.id).iter().flatten().map(|s| s.count).sum();
            players::tell(&player.id, &format!("You carry {items} items"));
            let _ = players::give(&player.id, &ItemStack::new(NUGGET, 1).with_data(vec![7]));
            return true;
        }
        if item != "minecraft:air" {
            return false;
        }
        let (x, y, z) = (pos.x, pos.y, pos.z);
        if storages::shape(dim, x, y, z).is_none() {
            if let Err(e) = storages::declare(dim, x, y, z, &shape()) {
                players::tell(&player.id, &format!("No press here: {e}"));
                return true;
            }
            let _ = world::set_data(dim, x, y, z, &0u32.to_le_bytes());
            let _ = world::schedule_tick(dim, x, y, z, 20);
        }
        if let Err(e) = storages::open_menu(&player.id, dim, x, y, z, "Iron Press") {
            players::tell(&player.id, &e);
        }
        true
    }

    fn block_tick(&mut self, dimension: &str, pos: BlockPos, _state: &str) {
        Self::work(dimension, pos);
        let _ = world::schedule_tick(dimension, pos.x, pos.y, pos.z, 20);
    }

    fn neighbor_changed(&mut self, dimension: &str, pos: BlockPos, _state: &str, _from: BlockPos) {
        let (x, y, z) = (f64::from(pos.x) + 0.5, f64::from(pos.y) + 0.5, f64::from(pos.z) + 0.5);
        let _ = entities::sound(dimension, "minecraft:block.anvil.land", x, y, z, 0.5, 1.5);
    }

    fn place_block(&mut self, player: &Player, pos: BlockPos, item: &str) -> bool {
        if item != "minecraft:tnt" {
            return false;
        }
        let near = world::data_positions(&player.dimension)
            .into_iter()
            .any(|(x, y, z)| (x - pos.x).abs() <= 4 && (y - pos.y).abs() <= 4 && (z - pos.z).abs() <= 4);
        if near {
            players::tell(&player.id, "No TNT near a press");
        }
        near
    }

    fn use_item(&mut self, player: &Player, stack: &ItemStack, _off_hand: bool) -> bool {
        if stack.item != "minecraft:blaze_rod" {
            return false;
        }
        let (dim, x, y, z) = (player.dimension.as_str(), player.x, player.y, player.z);
        for mob in entities::near(dim, x, y, z, 8.0) {
            let _ = entities::effect(&mob.id, "minecraft:speed", 10, 1);
        }
        let _ = entities::particle(dim, "minecraft:flame", x, y + 1.0, z, 0.5, 0.5, 0.5, 0.01, 20);
        true
    }
}
horizon_mod_sdk::export_server_mod!(IronPress);
