//! Everything the mod core does, in one mod you can walk through: `/showcase demo` builds a working stand.
//!
//! - Blocks, items, a mob type and recipes from `data/`; an ore from `worldgen`; settings and a data file of its own.
//! - The reactor: a furnace window with bars and slot rules, energy, a tank, timers, redstone out and a comparator,
//!   energy shared through touching reactors, particles and sounds. It refuses to break while charged.
//! - The guard: its own type, named and helmeted by code, walks and attacks when told, survives one death.
//! - Events of every kind, data on players, mobs and the world, messages between mods, tags, and a HUD on the client
//!   (`client.rs`, this mod's other half) fed over the mod's own network channel.
mod before;
mod client;

use horizon_mod_sdk::server::{
    Args, Argument, BlockChange, BlockPos, Entity, ItemStack, Player, Registration, ServerMod,
    StorageShape, connected, entities, move_energy, players, server_net, storages, world, worldgen,
};
use horizon_mod_sdk::storage;

const REACTOR: &str = "showcase:reactor";
const GUARD: &str = "showcase:guard";
const CORE: &str = "showcase:core";
const BOLT: &str = "showcase:bolt";
const BOOMERANG: &str = "showcase:boomerang";
const RIFT: &str = "showcase:rift";
const CELL: &str = "showcase:charged_cell";
const WAND: &str = "showcase:wand";
const BLADE: &str = "showcase:sun_blade";
const COAL: &str = "minecraft:coal";
const INGOT: &str = "minecraft:iron_ingot";
const CAPACITY: u64 = 64_000;
const CELL_COST: u64 = 800;
const STEP: u32 = 20;

struct Showcase {
    fuel_value: u64,
    /// Things seen, for `/showcase status` and the HUD.
    spawned: u32,
    picked_up: u32,
    explosions_stopped: u32,
    heartbeats: u32,
    /// Reactors the demo placed: blocks land at the tick's end, so they start on the next.
    to_start: Vec<(String, BlockPos)>,
    /// The reactor whose window each player opened last: its gauge is theirs.
    open: std::collections::HashMap<String, (String, BlockPos)>,
    /// Boomerangs in flight: who threw each, how long ago.
    boomerangs: std::collections::HashMap<String, (String, u32)>,
    /// `/showcase before`: the `before-events` hooks.
    before: before::Before,
}

fn say(player: &Player, text: &str) {
    let _ = players::tell(&player.id, text);
}

/// The window's gauge: this reactor's charge, not any other's.
fn send_charge(player: &Player, dim: &str, p: BlockPos) {
    let (energy, capacity) = storages::energy(dim, p.x, p.y, p.z).unwrap_or((0, 1));
    let charge = energy as f32 / capacity.max(1) as f32;
    let _ = server_net::send(&player.id, "showcase/charge", charge.to_string().as_bytes());
}

/// Calls every function of API level 18 and says which answered wrong.
fn api_tour(player: &Player, step: &str, (x, y, z): (i32, i32, i32)) -> Result<String, String> {
    let dim = player.dimension.as_str();
    let me = player.id.as_str();
    let (px, py, pz) = (player.x, player.y, player.z);
    if step == "set" {
        world::fill(dim, x, y, z, x + 2, y, z + 2, "minecraft:stone")?;
        world::set_block(dim, x, y + 1, z, "minecraft:oak_log[axis=y]")?;
        world::set_block(dim, x + 1, y + 1, z, "minecraft:stone")?;
        world::set_block(dim, x + 2, y + 1, z, "minecraft:stone")?;
        entities::spawn(dim, "minecraft:zombie", px + 3.0, py, pz, "")?;
        players::set_slot(me, 0, Some(&ItemStack { item: "minecraft:apple".into(), count: 5, data: Vec::new(), components: Vec::new() }))?;
        world::set_saved("api_tour", &[1])?;
        return Ok("api: built, now /showcase api check".into());
    }
    let mut bad = Vec::new();
    let mut ok = 0;
    let mut check = |name: &str, passed: bool| {
        if passed { ok += 1 } else { bad.push(name.to_owned()) }
    };
    if step == "check" {
        check("by-name", players::by_name(&player.name).is_some());
        check("by-id", players::by_id(me).is_some());
        check("is-online", players::is_online(me));
        check("count", players::count() >= 1);
        check("near", players::near(dim, px, py, pz, 4.0).iter().any(|p| p.id == me));
        check("held-item", players::held_item(me).is_some_and(|s| s.item == "minecraft:apple"));
        check("offhand-item", players::offhand_item(me).is_none());
        check("armor", players::armor(me).len() == 4);
        check("count-item", players::count_item(me, "minecraft:apple") >= 5);
        check("has-item", players::has_item(me, "minecraft:apple", 5) && !players::has_item(me, "minecraft:apple", 9999));
        check("free-slots", players::free_slots(me) < 36);
        check("find-item", players::find_item(me, "minecraft:apple").is_some());

        let log = (x, y + 1, z);
        check("get-blocks", world::get_blocks(dim, &[log, (x + 1, y, z)])?.len() == 2);
        check("is-loaded", world::is_loaded(dim, x, z));
        check("is-air", world::is_air(dim, x, y + 3, z) && !world::is_air(dim, x, y, z));
        check("block-id", world::block_id(dim, log.0, log.1, log.2).as_deref() == Some("minecraft:oak_log"));
        check("block-property", world::block_property(dim, log.0, log.1, log.2, "axis").as_deref() == Some("y"));
        check("find-blocks", world::find_blocks(dim, x, y, z, x + 2, y, z + 2, "minecraft:stone", 4)?.len() == 4);
        check("count-blocks", world::count_blocks(dim, x, y, z, x + 2, y, z + 2, "minecraft:stone")? == 9);
        check("height-at", world::height_at(dim, x, z).is_some_and(|h| h >= y + 1));
        check("game-time", world::game_time().is_some());
        check("day-time", world::day_time().is_some());
        check("has-data", !world::has_data(dim, x, y, z));
        check("saved-keys", world::saved_keys().iter().any(|k| k == "api_tour"));
        check("running-mods", world::running_mods().iter().any(|m| m == "showcase"));
        check("is-valid-state", world::is_valid_state("minecraft:stone") && !world::is_valid_state("minecraft:no_such"));
        check("item-exists", world::item_exists("minecraft:apple") && !world::item_exists("minecraft:no_such"));
        check("max-stack-size", world::max_stack_size("minecraft:apple") == Some(64));
        check("random-u32", world::random_u32() != world::random_u32());

        let zombie = entities::of_kind(dim, "minecraft:zombie", px, py, pz, 32.0);
        check("of-kind", !zombie.is_empty());
        check("nearest", entities::nearest(dim, px, py, pz, 32.0, Some("minecraft:zombie")).is_some());
        check("count-near", entities::count_near(dim, px, py, pz, 32.0) >= 1);
        if let Some(z) = zombie.first() {
            check("get", entities::get(dim, &z.id).is_some());
            check("exists", entities::exists(dim, &z.id) && !entities::exists(dim, me));
            entities::set_health(&z.id, 5.0)?;
            entities::heal(&z.id, 2.0)?;
            entities::swing(&z.id)?;
            entities::effect(&z.id, "minecraft:speed", 30, 0)?;
            entities::remove_effect(&z.id, "minecraft:speed")?;
            entities::clear_effects(&z.id)?;
        }

        let (a, b) = ((x, y, z + 1), (x + 1, y, z + 1));
        let shape = StorageShape { slots: 3, energy_capacity: 100, tanks: vec![1000] };
        storages::declare(dim, a.0, a.1, a.2, &shape)?;
        storages::declare(dim, b.0, b.1, b.2, &shape)?;
        let apple = ItemStack { item: "minecraft:apple".into(), count: 4, data: Vec::new(), components: Vec::new() };
        check("has-storage", storages::has_storage(dim, a.0, a.1, a.2) && !storages::has_storage(dim, x, y + 3, z));
        check("is-empty", storages::is_empty(dim, a.0, a.1, a.2));
        storages::set_item(dim, a.0, a.1, a.2, 0, Some(&apple))?;
        storages::set_energy(dim, a.0, a.1, a.2, 60)?;
        storages::set_fluid(dim, a.0, a.1, a.2, 0, Some(&storages::FluidStack { fluid: "minecraft:water".into(), amount: 500 }))?;
        check("is-full", !storages::is_full(dim, a.0, a.1, a.2));
        check("free-slots", storages::free_slots(dim, a.0, a.1, a.2) == 2);
        check("count-item", storages::count_item(dim, a.0, a.1, a.2, "minecraft:apple") == 4);
        check("positions", storages::positions(dim).contains(&a));
        check("free-energy", storages::free_energy(dim, a.0, a.1, a.2) == 40);
        check("fluid-amount", storages::fluid_amount(dim, a.0, a.1, a.2, "minecraft:water") == 500);
        check("transfer-item", storages::transfer_item(dim, a, b, None, 3) == 3 && storages::count_item(dim, b.0, b.1, b.2, "minecraft:apple") == 3);
        check("transfer-energy", storages::transfer_energy(dim, a, b, 50) == 50 && storages::free_energy(dim, b.0, b.1, b.2) == 50);
        check("transfer-fluid", storages::transfer_fluid(dim, a, b, None, 200) == 200 && storages::fluid_amount(dim, b.0, b.1, b.2, "minecraft:water") == 200);
        storages::clear(dim, b.0, b.1, b.2)?;
        check("clear", storages::is_empty(dim, b.0, b.1, b.2));
        storages::remove(dim, a.0, a.1, a.2);
        storages::remove(dim, b.0, b.1, b.2);

        world::set_block_property(dim, x, y + 1, z, "axis", "x")?;
        world::break_block(dim, x + 1, y + 1, z, true)?;
        world::break_block(dim, x + 2, y + 1, z, false)?;
    } else if step == "act" {
        players::set_title_times(me, 5, 40, 5)?;
        players::show_title(me, "Level 18", Some("seventy-five new functions"))?;
        players::action_bar(me, "the action bar")?;
        players::add_experience(me, 7)?;
        players::add_levels(me, 2)?;
        players::set_spawn(me, dim, px as i32, py as i32, pz as i32)?;
        players::play_sound(me, "minecraft:block.anvil.use", 1.0, 1.0)?;
        players::stop_sounds(me)?;
        players::grant_advancement(me, "minecraft:story/root")?;
        players::revoke_advancement(me, "minecraft:story/root")?;
        players::set_game_mode(me, players::GameMode::Adventure)?;
        world::set_time(6000)?;
        world::set_weather("rain", 60)?;
        world::set_gamerule("keep_inventory", "true")?;
        world::set_difficulty("hard")?;
        world::set_world_spawn(px as i32, py as i32, pz as i32)?;
    } else if step == "boom" {
        world::lightning(dim, px + 6.0, py, pz)?;
        world::explode(dim, f64::from(x), f64::from(y) + 1.0, f64::from(z), 2.0)?;
        players::set_game_mode(me, players::GameMode::Creative)?;
    } else if step == "clear" {
        players::clear_inventory(me)?;
    } else if step == "kick" {
        players::kick(me, "the end of the tour")?;
    } else {
        return Ok("/showcase api set, check, act, boom, clear or kick".into());
    }
    Ok(if bad.is_empty() {
        format!("api {step}: ok ({ok} answers right)")
    } else {
        format!("api {step}: {ok} right, WRONG: {}", bad.join(", "))
    })
}

/// The closest of the mod's positions to a mob, with its distance.
fn nearest_reactor(dim: &str, mob: &Entity) -> Option<(BlockPos, i64)> {
    world::data_positions(dim)
        .into_iter()
        .map(|(x, y, z)| {
            let d = (f64::from(x) - mob.x).abs() + (f64::from(z) - mob.z).abs();
            (BlockPos { x, y, z }, d as i64)
        })
        .min_by_key(|r| r.1)
}

fn is_reactor(dim: &str, p: BlockPos) -> bool {
    world::get_block(dim, p.x, p.y, p.z).is_some_and(|s| s.starts_with(REACTOR))
}

/// A reactor's storage and its first timer, if it has none yet.
fn start_reactor(dim: &str, p: BlockPos) -> Result<(), String> {
    let (x, y, z) = (p.x, p.y, p.z);
    if storages::shape(dim, x, y, z).is_some() {
        return Ok(());
    }
    let shape = StorageShape {
        slots: 3,
        energy_capacity: CAPACITY,
        tanks: vec![4000],
    };
    storages::declare(dim, x, y, z, &shape)?;
    let rule = |take_only, item: &str| storages::SlotRule {
        take_only,
        items: if item.is_empty() {
            Vec::new()
        } else {
            vec![item.to_owned()]
        },
    };
    storages::set_slot_rule(dim, x, y, z, 0, Some(&rule(false, INGOT)))?;
    storages::set_slot_rule(dim, x, y, z, 1, Some(&rule(false, COAL)))?;
    storages::set_slot_rule(dim, x, y, z, 2, Some(&rule(true, "")))?;
    world::schedule_tick(dim, x, y, z, STEP)
}

impl Showcase {
    /// One step of a reactor: burn, press, share, show.
    fn work(&self, dim: &str, p: BlockPos) {
        let (x, y, z) = (p.x, p.y, p.z);
        let Some((mut energy, capacity)) = storages::energy(dim, x, y, z) else {
            return;
        };
        // Redstone switches it off.
        let powered = world::redstone_power(dim, x, y, z).unwrap_or(0) > 0;
        let mut burning = false;
        if !powered && energy + self.fuel_value <= capacity {
            let burnt = storages::extract_item(dim, x, y, z, Some(COAL), 1, false);
            if burnt.is_some_and(|s| s.count > 0) {
                energy = storages::insert_energy(dim, x, y, z, self.fuel_value, false) + energy;
                burning = true;
            }
        }
        let mut pressing = false;
        if !powered && energy >= CELL_COST {
            let cell = cell_stack(energy);
            let fits = storages::items(dim, x, y, z).is_some_and(|slots| {
                slots.get(2).is_some_and(|s| {
                    s.as_ref()
                        .is_none_or(|s| s.item == CELL && s.data == cell.data && s.count < 64)
                })
            });
            if fits
                && storages::extract_item(dim, x, y, z, Some(INGOT), 1, false)
                    .is_some_and(|s| s.count > 0)
            {
                let mut out = cell;
                let held =
                    storages::items(dim, x, y, z).and_then(|slots| slots.get(2).cloned().flatten());
                out.count = held.map_or(1, |s| s.count + 1);
                let _ = storages::set_item(dim, x, y, z, 2, Some(&out));
                storages::extract_energy(dim, x, y, z, CELL_COST, false);
                let _ = storages::insert_fluid(
                    dim,
                    x,
                    y,
                    z,
                    &storages::FluidStack {
                        fluid: "minecraft:water".into(),
                        amount: 100,
                    },
                    false,
                );
                pressing = true;
            }
        }
        // The core over it: a display mob that turns with the charge.
        let (px, py, pz) = (f64::from(p.x) + 0.5, f64::from(p.y) + 1.0, f64::from(p.z) + 0.5);
        let core = entities::near(dim, px, py, pz, 1.0).into_iter().find(|e| e.kind == CORE);
        match core {
            Some(core) => {
                let turn = world::get_saved("turn").map_or(0, |d| d.first().copied().unwrap_or(0));
                let _ = world::set_saved("turn", &[turn.wrapping_add(16)]);
                let _ = entities::set_rotation(&core.id, f32::from(turn) * 360.0 / 256.0, 0.0);
            }
            None => {
                let _ = entities::spawn(dim, CORE, px, py, pz, "");
            }
        }
        // Touching reactors even out.
        for other in connected(p, 16, |q| is_reactor(dim, q)).into_iter().skip(1) {
            let theirs = storages::energy(dim, other.x, other.y, other.z).map_or(0, |e| e.0);
            let mine = storages::energy(dim, x, y, z).map_or(0, |e| e.0);
            if mine > theirs + 1 {
                move_energy(dim, p, other, (mine - theirs) / 2);
            }
        }
        let energy = storages::energy(dim, x, y, z).map_or(0, |e| e.0);
        let share = energy * 15 / capacity.max(1);
        let _ = world::set_redstone_output(dim, x, y, z, u8::from(energy > 0) * 15);
        let _ = world::set_comparator_output(dim, x, y, z, share as u8);
        let flame = if burning { 200 } else { 0 };
        let arrow = if pressing { 200 } else { 0 };
        let _ = storages::set_menu_data(dim, x, y, z, &[flame, 200, arrow, 200]);
        let _ = world::set_data(dim, x, y, z, &energy.to_le_bytes());
        let (cx, cy, cz) = (f64::from(x) + 0.5, f64::from(y) + 1.1, f64::from(z) + 0.5);
        if burning {
            let _ = entities::particle(dim, "minecraft:flame", cx, cy, cz, 0.2, 0.1, 0.2, 0.01, 8);
            let _ = entities::sound(
                dim,
                "minecraft:block.blastfurnace.fire_crackle",
                cx,
                cy,
                cz,
                1.0,
                1.0,
            );
        }
        if pressing {
            let _ = entities::sound(dim, "minecraft:block.anvil.use", cx, cy, cz, 0.4, 1.4);
        }
    }

    fn status(&self, dim: &str) -> String {
        let reactors = world::data_positions(dim);
        let energy: u64 = reactors
            .iter()
            .filter_map(|p| storages::energy(dim, p.0, p.1, p.2))
            .map(|e| e.0)
            .sum();
        format!(
            "reactors {} | FE {energy} | mobs seen {} | pickups {} | explosions stopped {} | beats {}",
            reactors.len(),
            self.spawned,
            self.picked_up,
            self.explosions_stopped,
            self.heartbeats
        )
    }

    fn command_as(
        &mut self,
        player: &Player,
        action: &str,
        arg: &str,
    ) -> Result<Option<String>, String> {
        let dim = player.dimension.as_str();
        let (fx, fy, fz) = (
            player.x.floor() as i32,
            player.y.floor() as i32,
            player.z.floor() as i32,
        );
        match action {
            // A block with a property of its own: a click on the dial steps `level`.
            "dial" => {
                let at = (fx + 2, fy, fz);
                let level = world::get_block(dim, at.0, at.1, at.2)
                    .filter(|s| s.starts_with("showcase:dial"))
                    .and_then(|s| {
                        let (_, v) = s.split_once("level=")?;
                        v.trim_end_matches(']').parse::<u8>().ok()
                    })
                    .map_or(0, |v| (v + 1) % 4);
                world::set_blocks(
                    dim,
                    &[BlockChange {
                        x: at.0,
                        y: at.1,
                        z: at.2,
                        state: format!("showcase:dial[level={level}]"),
                    }],
                )?;
                Ok(Some(format!("level {level}")))
            }
            // A fluid of its own: a source that flows apart from water.
            "plasma" => {
                world::set_blocks(
                    dim,
                    &[BlockChange {
                        x: fx + 2,
                        y: fy + 2,
                        z: fz,
                        state: "showcase:plasma".into(),
                    }],
                )?;
                Ok(Some("plasma above you, 2 blocks over".into()))
            }
            // An effect of its own: hurts, slows, and sparks through effect-tick.
            "shock" => {
                entities::effect(&player.id, "showcase:shock", 10, 0)?;
                Ok(None)
            }
            // Dimensions of its own: flat, vanilla's noise, and code.
            "go" => {
                let (to, y) = match arg {
                    "moon" => ("showcase:moon", 12.0),
                    "deep" => ("showcase:deep", 100.0),
                    "rift" => (RIFT, 120.0),
                    _ => ("minecraft:overworld", 100.0),
                };
                entities::teleport(&player.id, to, player.x, y, player.z)?;
                Ok(None)
            }
            // An entity kind of its own, flown by entity-tick: goes out, comes back.
            "boomerang" => {
                let (yaw, pitch) = players::look(&player.id).unwrap_or((0.0, 0.0));
                let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
                let (dx, dy, dz) = (
                    f64::from(-yaw.sin() * pitch.cos()),
                    f64::from(-pitch.sin()),
                    f64::from(yaw.cos() * pitch.cos()),
                );
                let nbt = format!("{{Motion:[{}d,{}d,{}d]}}", dx * 0.9, dy * 0.9, dz * 0.9);
                entities::spawn(
                    &player.dimension,
                    BOOMERANG,
                    player.x + dx,
                    player.y + 1.5 + dy,
                    player.z + dz,
                    &nbt,
                )?;
                Ok(None)
            }
            // A thing that isn't a mob to anyone: thrown where the player looks.
            "bolt" => {
                let (yaw, pitch) = players::look(&player.id).unwrap_or((0.0, 0.0));
                let (yaw, pitch) = (yaw.to_radians(), pitch.to_radians());
                let (dx, dy, dz) = (
                    f64::from(-yaw.sin() * pitch.cos()),
                    f64::from(-pitch.sin()),
                    f64::from(yaw.cos() * pitch.cos()),
                );
                let nbt = format!("{{Motion:[{}d,{}d,{}d]}}", dx * 1.2, dy * 1.2, dz * 1.2);
                entities::spawn(
                    &player.dimension,
                    BOLT,
                    player.x + dx,
                    player.y + 1.5 + dy,
                    player.z + dz,
                    &nbt,
                )?;
                Ok(None)
            }
            // The calls nothing else here makes, each on the nearest reactor.
            "test" => {
                let (x, y, z) = world::data_positions(dim)
                    .into_iter()
                    .min_by_key(|p| i64::from(p.0 - fx).abs() + i64::from(p.2 - fz).abs())
                    .ok_or("no reactor: /showcase demo first")?;
                let coal = ItemStack::new(COAL, 4);
                let fits = storages::insert_item(dim, x, y, z, &coal, true);
                storages::set_energy(dim, x, y, z, CAPACITY / 2)?;
                let water = storages::FluidStack { fluid: "minecraft:water".into(), amount: 1000 };
                storages::set_fluid(dim, x, y, z, 0, Some(&water))?;
                let out = storages::extract_fluid(dim, x, y, z, None, 250, false);
                players::set_slot(&player.id, 8, Some(&ItemStack::new(BLADE, 1)))?;
                let cores = entities::near(dim, f64::from(x), f64::from(y) + 1.0, f64::from(z), 3.0);
                let killed = cores.iter().filter(|e| e.kind == CORE).map(|e| entities::kill(&e.id)).count();
                Ok(Some(format!(
                    "coal fits {fits}, half charged, drew {} mB, blade in slot 9, {killed} core(s) killed (they come back)",
                    out.map_or(0, |f| f.amount)
                )))
            }
            // The same storage as vanilla's window, for comparison with the mod's layout.
            "menu" => {
                let (x, y, z) = world::data_positions(dim)
                    .into_iter()
                    .next()
                    .ok_or("no reactor: /showcase demo first")?;
                storages::open_menu(&player.id, dim, x, y, z, "Plain reactor")?;
                Ok(None)
            }
            // A storage goes while its block stays.
            "wipe" => {
                for (x, y, z) in world::data_positions(dim) {
                    storages::remove(dim, x, y, z);
                }
                Ok(Some("Storages removed".into()))
            }
            "surface" => {
                worldgen::set_surface_rule("minecraft:plains", "minecraft:moss_block")?;
                Ok(Some("New plains chunks are topped with moss".into()))
            }
            // The API's level 18 in one go: `/showcase api set`, then, a tick later, `/showcase api check`,
            // then `/showcase api act` for what shows on screen.
            "api" => api_tour(player, arg, (fx + 8, fy + 5, fz)).map(Some),
            // `before-events`: `on` turns each hook's rule on, `check` says how often each was asked.
            "before" => match arg {
                "on" | "off" => {
                    self.before.switch(arg == "on");
                    Ok(Some(match arg {
                        "on" => format!("Before-hooks on: {}", before::RULES),
                        _ => "Before-hooks off".into(),
                    }))
                }
                "check" => Ok(Some(format!(
                    "Before-hooks {} (asked/acted): {}",
                    if self.before.on { "on" } else { "off" },
                    self.before.report()
                ))),
                _ => Err("/showcase before on | off | check".into()),
            },
            "help" => Ok(Some(
                "/showcase api | before | demo | give | guard | bolt | test | menu | wipe | surface | learn | forget | undying | status | manual | tags | report | dial | plasma | shock | boomerang | go moon/deep/rift/home".into(),
            )),
            // A stand beside the player: a floor, two reactors, a lamp and a wire.
            "demo" => {
                let (x, y, z) = (fx + 3, fy, fz);
                let mut blocks = Vec::new();
                let mut put = |x, y, z, state: &str| {
                    blocks.push(BlockChange { x, y, z, state: state.to_owned() });
                };
                for dx in -1..=5 {
                    for dz in -2..=2 {
                        put(x + dx, y - 1, z + dz, "minecraft:smooth_stone");
                        for up in 0..3 {
                            put(x + dx, y + up, z + dz, "minecraft:air");
                        }
                    }
                }
                put(x, y, z, REACTOR);
                // Apart: touching reactors share their charge.
                put(x, y, z + 2, REACTOR);
                put(x + 1, y, z, "minecraft:redstone_wire");
                put(x + 2, y, z, "minecraft:redstone_lamp");
                world::set_blocks(dim, &blocks)?;
                for z in [z, z + 2] {
                    self.to_start.push((dim.to_owned(), BlockPos { x, y, z }));
                }
                let _ = players::give(&player.id, &ItemStack::new(COAL, 16));
                let _ = players::give(&player.id, &ItemStack::new(INGOT, 16));
                let _ = players::give(&player.id, &ItemStack::new(WAND, 1));
                Ok(Some("Built. Right click a reactor with an empty hand, put in coal and iron.".into()))
            }
            "give" => {
                for (item, count) in [(REACTOR, 2), (WAND, 1), (BLADE, 1), ("showcase:sun_bread", 8), ("showcase:sun_ore", 4)] {
                    players::give(&player.id, &ItemStack::new(item, count))?;
                }
                // A stack whose look comes from code.
                let look = players::ItemLook {
                    name: Some("Reactor Log".into()),
                    lore: vec![self.status(dim), format!("Written for {}", player.name)],
                    model: Some("minecraft:writable_book".into()),
                    glint: Some(true),
                };
                let log = players::style_stack(&ItemStack::new("minecraft:paper", 1), &look)?;
                players::give(&player.id, &log)?;
                Ok(None)
            }
            "guard" => {
                entities::spawn(dim, GUARD, player.x + 4.0, player.y, player.z, "")?;
                Ok(Some("A guard comes. It gets a name and a helmet as it spawns.".into()))
            }
            "learn" | "forget" => {
                let on = action == "learn";
                world::set_saved("learned", &[u8::from(on)])?;
                let locked: Vec<String> = if on { Vec::new() } else { vec![BLADE.into()] };
                world::set_locked_results(&locked)?;
                Ok(Some(format!("Sun Blade recipe {}", if on { "open" } else { "closed" })))
            }
            "undying" => {
                let on = players::get_data(&player.id, "undying").is_none();
                players::set_undying(&player.id, on)?;
                players::set_data(&player.id, "undying", if on { b"1" } else { b"" })?;
                Ok(Some(format!("Undying: {on}")))
            }
            "status" => Ok(Some(self.status(dim))),
            "manual" => Ok(storage::read_resource("data/showcase/manual.txt")
                .map(|t| String::from_utf8_lossy(&t).trim().to_owned())),
            "tags" => Ok(Some(format!(
                "coal is coal {}, reactor mineable {}, zombie undead {}, hud mod here {}, biome {}, sneaking {}",
                world::item_has_tag(COAL, "minecraft:coals"),
                world::block_has_tag(REACTOR, "minecraft:mineable/pickaxe"),
                world::entity_has_tag("minecraft:zombie", "minecraft:undead"),
                world::mod_loaded("showcase"),
                world::get_biome(dim, fx, fy, fz).unwrap_or_default(),
                players::sneaking(&player.id),
            ))),
            "report" => {
                let mine = players::inventory(&player.id).iter().flatten().map(|s| s.count).sum::<u32>();
                let slot = players::selected_slot(&player.id).unwrap_or(0);
                let visits = players::get_data(&player.id, "visits").map_or(0, |d| d.len());
                let _ = arg;
                Ok(Some(format!("items {mine}, slot {slot}, visits {visits}, players {}", players::online().len())))
            }
            other => Err(format!("No {other}: try /showcase help")),
        }
    }
}

/// A cell remembers how charged its reactor was.
fn cell_stack(energy: u64) -> ItemStack {
    let tier = (energy * 3 / CAPACITY).min(2) as u8 + 1;
    let look = players::ItemLook {
        name: None,
        lore: vec![format!("Tier {tier}")],
        model: None,
        glint: Some(tier == 3),
    };
    let plain = ItemStack::new(CELL, 1).with_data(vec![tier]);
    players::style_stack(&plain, &look).unwrap_or(plain)
}

impl ServerMod for Showcase {
    /// A rift: hills from sines, one biome.
    fn generate_chunk(
        &mut self,
        dimension: &str,
        cx: i32,
        cz: i32,
    ) -> Option<horizon_mod_sdk::server::ChunkFill> {
        if dimension != RIFT {
            return None;
        }
        let min_y = 0;
        let height = 100usize;
        let mut blocks = vec![0u16; 256 * height];
        for z in 0..16i32 {
            for x in 0..16i32 {
                let (wx, wz) = (f64::from(cx * 16 + x), f64::from(cz * 16 + z));
                let top = 50.0 + (wx / 13.0).sin() * 12.0 + (wz / 17.0).cos() * 12.0;
                for y in 0..height {
                    let at = y * 256 + (z as usize) * 16 + x as usize;
                    blocks[at] = match (y as f64) < top - 1.0 {
                        true => 1,
                        false if (y as f64) < top => 2,
                        false => 0,
                    };
                }
            }
        }
        Some(horizon_mod_sdk::server::ChunkFill {
            palette: vec![
                "minecraft:air".into(),
                "minecraft:deepslate".into(),
                "minecraft:crimson_nylium".into(),
            ],
            min_y,
            blocks,
            biome: Some("minecraft:the_void".into()),
        })
    }

    /// A guard with nobody near walks to the closest reactor.
    fn goal_can_use(&mut self, _goal: &str, mob: &Entity) -> bool {
        let dim = "minecraft:overworld";
        let alone = players::online().iter().all(|p| {
            (p.x - mob.x).abs() + (p.z - mob.z).abs() > 6.0
        });
        alone && nearest_reactor(dim, mob).is_some_and(|(_, d)| d > 3)
    }
    fn goal_tick(&mut self, _goal: &str, mob: &Entity) -> bool {
        let dim = "minecraft:overworld";
        let Some((p, d)) = nearest_reactor(dim, mob) else {
            return false;
        };
        let _ = entities::move_to(&mob.id, f64::from(p.x), f64::from(p.y), f64::from(p.z), 1.0);
        d > 3
    }

    /// Out for a while, then home to who threw it, through anything.
    fn entity_tick(&mut self, dimension: &str, entity: &Entity) {
        if entity.kind != BOOMERANG {
            return;
        }
        let owner = players::online()
            .into_iter()
            .filter(|p| p.dimension == dimension)
            .min_by_key(|p| ((p.x - entity.x).powi(2) + (p.z - entity.z).powi(2)) as i64);
        let Some(owner) = owner else {
            let _ = entities::kill(&entity.id);
            return;
        };
        let (thrower, age) = self
            .boomerangs
            .entry(entity.id.clone())
            .or_insert((owner.id.clone(), 0));
        *age += 1;
        let age = *age;
        let home = players::online().into_iter().find(|p| p.id == *thrower);
        let home = home.unwrap_or(owner);
        if age > 15 {
            let (dx, dy, dz) = (home.x - entity.x, home.y + 1.0 - entity.y, home.z - entity.z);
            let length = (dx * dx + dy * dy + dz * dz).sqrt().max(0.01);
            if length < 1.5 || age > 200 {
                let _ = entities::kill(&entity.id);
                self.boomerangs.remove(&entity.id);
                return;
            }
            let _ = entities::set_velocity(&entity.id, dx / length, dy / length, dz / length);
        }
        let _ = entities::particle(dimension, "minecraft:crit", entity.x, entity.y, entity.z, 0.0, 0.0, 0.0, 0.0, 1);
    }

    /// Shock sparks on whoever has it.
    fn effect_tick(&mut self, _effect: &str, target: &str, _amplifier: u8) {
        self.heartbeats += 1;
        if self.heartbeats % 10 == 0
            && let Some(p) = players::online().into_iter().find(|p| p.id == target)
        {
            let _ = entities::particle(&p.dimension, "minecraft:electric_spark", p.x, p.y + 1.0, p.z, 0.3, 0.5, 0.3, 0.0, 4);
        }
    }

    fn init() -> Result<(Self, Registration), String> {
        worldgen::add_ore(&worldgen::Ore {
            biome: "*".into(),
            block: "showcase:sun_ore".into(),
            replaces: vec!["minecraft:stone".into(), "minecraft:deepslate".into()],
            min_y: -32,
            max_y: 64,
            size: 5,
            per_chunk: 3,
        })?;
        let learned = world::get_saved("learned").is_some_and(|d| d == [1]);
        if !learned {
            world::set_locked_results(&[BLADE.to_owned()])?;
        }
        let fuel_value = storage::config("fuel_value")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1600);
        let state = Showcase {
            fuel_value,
            spawned: 0,
            picked_up: 0,
            explosions_stopped: 0,
            heartbeats: 0,
            to_start: Vec::new(),
            open: Default::default(),
            boomerangs: Default::default(),
            before: Default::default(),
        };
        let hooks = Registration::new()
            .on_tick()
            .on_joins()
            .on_chat()
            .on_blocks()
            .command_with(
                "showcase",
                [Argument::word("action"), Argument::text("arg").optional()],
            );
        Ok((state, hooks))
    }

    fn tick(&mut self, tick: u64) {
        for (dim, pos) in std::mem::take(&mut self.to_start) {
            let _ = start_reactor(&dim, pos);
        }
        for event in players::poll_events() {
            let text = format!("{} {}", event.kind, event.detail);
            if event.player.is_empty() {
                players::broadcast(&text);
            } else if event.kind != "targeted" {
                let _ = players::tell(&event.player, &text);
            }
            // Respawned: what they asked for lasts.
            if event.kind == "respawned" && players::get_data(&event.player, "undying").is_some() {
                let _ = players::set_undying(&event.player, true);
            }
        }
        // Its own message comes back: the channel works.
        self.heartbeats += world::mod_receive().len() as u32;
        if tick % 20 != 0 {
            return;
        }
        let _ = world::mod_send("showcase", "heartbeat", b"beat");
        for p in players::online() {
            let dim = p.dimension.clone();
            let _ = server_net::send(&p.id, "showcase/status", self.status(&dim).as_bytes());
            // Where to draw the rings: the client half draws, the game turns them.
            let rings: Vec<String> = world::data_positions(&dim)
                .iter()
                .map(|p| format!("{dim} {} {} {}", p.0, p.1, p.2))
                .collect();
            let _ = server_net::send(&p.id, "showcase/rings", rings.join("\n").as_bytes());
            if let Some((dim, pos)) = self.open.get(&p.id) {
                send_charge(&p, dim, *pos);
            }
            // Guards walk at whoever is near, and chase them when close.
            for e in entities::near(&dim, p.x, p.y, p.z, 24.0) {
                if e.kind == GUARD {
                    let _ = entities::move_to(&e.id, p.x, p.y, p.z, 1.0);
                    if (e.x - p.x).abs() + (e.z - p.z).abs() < 6.0 {
                        let _ = entities::set_target(&e.id, Some(&p.id));
                    }
                }
            }
        }
    }

    fn player_joined(&mut self, player: &Player) {
        let visits = players::get_data(&player.id, "visits").map_or(0, |d| d.len());
        let _ = players::set_data(&player.id, "visits", &vec![1; (visits + 1).min(1024)]);
        say(
            player,
            &format!("Core Showcase: visit {}. Type /showcase demo", visits + 1),
        );
    }

    fn chat(&mut self, player: &Player, text: &str) {
        if text.contains("reactor") {
            say(player, "Someone said reactor. Try /showcase manual");
        }
    }

    fn command(
        &mut self,
        player: &Player,
        _name: &str,
        args: &Args,
    ) -> Result<Option<String>, String> {
        let action = args.string("action").unwrap_or("help").to_owned();
        let arg = args.string("arg").unwrap_or_default().to_owned();
        self.command_as(player, &action, &arg)
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(REACTOR) {
            if let Err(e) = start_reactor(&player.dimension, pos) {
                say(player, &e);
            }
        }
    }

    fn place_block(&mut self, player: &Player, pos: BlockPos, item: &str) -> bool {
        // Nothing this hot deep down.
        let refuse = item == REACTOR && pos.y < -32;
        if refuse {
            say(player, "Too deep for a reactor");
        }
        refuse
    }

    fn break_block(&mut self, player: &Player, pos: BlockPos, state: &str) -> bool {
        if !state.starts_with(REACTOR) || players::sneaking(&player.id) {
            return false;
        }
        let charged =
            storages::energy(&player.dimension, pos.x, pos.y, pos.z).is_some_and(|e| e.0 > 0);
        if charged {
            say(player, "Still charged: sneak to break it");
        }
        charged
    }

    fn use_block(&mut self, player: &Player, pos: BlockPos, state: &str, item: &str) -> bool {
        if !state.starts_with(REACTOR) {
            return false;
        }
        let dim = player.dimension.as_str();
        if let Err(e) = start_reactor(dim, pos) {
            say(player, &e);
            return true;
        }
        if item == WAND {
            let (e, cap) = storages::energy(dim, pos.x, pos.y, pos.z).unwrap_or((0, 0));
            let water = storages::fluids(dim, pos.x, pos.y, pos.z)
                .and_then(|t| t.into_iter().next().flatten())
                .map_or(0, |f| f.amount);
            say(player, &format!("{e} / {cap} FE, {water} mB of water"));
            return true;
        }
        // Sneaking places what's in hand, as with any machine; otherwise the window comes first.
        if players::sneaking(&player.id) {
            return false;
        }
        self.open.insert(player.id.clone(), (dim.to_owned(), pos));
        send_charge(player, dim, pos);
        if let Err(e) = storages::open_menu_as(&player.id, dim, pos.x, pos.y, pos.z, "Reactor", "minecraft:furnace") {
            say(player, &e);
        }
        true
    }

    fn block_tick(&mut self, dimension: &str, pos: BlockPos, state: &str) {
        if state.starts_with(REACTOR) {
            self.work(dimension, pos);
            let _ = world::schedule_tick(dimension, pos.x, pos.y, pos.z, STEP);
        }
    }

    fn neighbor_changed(&mut self, dimension: &str, pos: BlockPos, state: &str, _from: BlockPos) {
        if state.starts_with(REACTOR) && storages::shape(dimension, pos.x, pos.y, pos.z).is_none() {
            let _ = start_reactor(dimension, pos);
        }
    }

    /// The wand on a reactor reads it; in the air it heals and lifts.
    fn use_item(&mut self, player: &Player, stack: &ItemStack, _off_hand: bool) -> bool {
        if stack.item != WAND {
            return false;
        }
        let _ = entities::effect(&player.id, "minecraft:regeneration", 5, 1);
        let _ = entities::set_attribute(&player.id, "minecraft:max_health", 40.0);
        let _ = entities::teleport(
            &player.id,
            &player.dimension,
            player.x,
            player.y + 2.0,
            player.z,
        );
        true
    }

    /// The wand switches a mob's mind off and on; it wears a helmet when it's on again.
    fn use_entity(&mut self, player: &Player, entity: &Entity, item: &str) -> bool {
        if item != WAND || entity.kind == "minecraft:player" {
            return false;
        }
        let calm = entities::get_data(&entity.id, "calm").is_some();
        let _ = entities::set_ai(&entity.id, calm);
        let _ = entities::set_data(&entity.id, "calm", if calm { b"" } else { b"1" });
        if calm {
            let helmet = ItemStack::new("minecraft:golden_helmet", 1);
            let _ = entities::set_equipment(&entity.id, "armor.head", &helmet);
        }
        say(player, if calm { "Awake" } else { "Calm" });
        true
    }

    /// A wand never hurts.
    fn attack_entity(&mut self, _player: &Player, _entity: &Entity, item: &str) -> bool {
        item == WAND
    }

    /// A bolt: sparks where it lands, and it hurts what it hit.
    fn projectile_hit(
        &mut self,
        dimension: &str,
        _projectile: &Entity,
        target: Option<&str>,
        (x, y, z): (f64, f64, f64),
    ) -> bool {
        let _ = entities::particle(dimension, "minecraft:flame", x, y, z, 0.2, 0.2, 0.2, 0.02, 12);
        if let Some(target) = target {
            let _ = entities::hurt(target, 4.0);
        }
        false
    }

    fn entity_spawned(&mut self, _dimension: &str, entity: &Entity) -> bool {
        self.spawned += 1;
        if entity.kind == GUARD {
            let _ = entities::set_name(&entity.id, Some("Reactor Guard"));
            let helmet = ItemStack::new("minecraft:iron_helmet", 1);
            let _ = entities::set_equipment(&entity.id, "armor.head", &helmet);
        }
        false
    }

    /// A guard shrugs off anything under 2.
    fn entity_hurt(&mut self, _dimension: &str, entity: &Entity, amount: f32) -> bool {
        entity.kind == GUARD && amount < 2.0
    }

    /// A guard dies twice: the first time it only jumps back.
    fn entity_dying(&mut self, _dimension: &str, entity: &Entity, _killer: Option<&str>) -> bool {
        if entity.kind != GUARD || entities::get_data(&entity.id, "second_life").is_some() {
            return false;
        }
        let _ = entities::set_data(&entity.id, "second_life", b"1");
        let _ = entities::set_velocity(&entity.id, 0.0, 0.8, 0.0);
        true
    }

    fn entity_died(&mut self, dimension: &str, entity: &Entity, killer: Option<&str>) {
        if entity.kind == GUARD {
            let (x, y, z) = (
                entity.x.floor() as i32,
                entity.y.floor() as i32,
                entity.z.floor() as i32,
            );
            let _ = entities::drop_item(dimension, x, y, z, &cell_stack(CAPACITY));
            if let Some(killer) = killer {
                let _ = players::tell(killer, "The guard falls, and leaves a cell");
            }
        }
    }

    fn player_hurt(&mut self, player: &Player, amount: f32) {
        if amount >= 10.0 {
            say(player, "That was a big one");
        }
    }

    fn item_picked_up(&mut self, player: &Player, stack: &ItemStack) {
        self.picked_up += 1;
        if stack.item == CELL {
            say(
                player,
                &format!(
                    "A cell of tier {}",
                    stack.data.first().copied().unwrap_or(0)
                ),
            );
        }
    }

    /// Explosions fizzle near a reactor.
    fn explosion(&mut self, dimension: &str, x: f64, y: f64, z: f64, _power: f32) -> bool {
        let near = world::data_positions(dimension).iter().any(|p| {
            (f64::from(p.0) - x).abs() + (f64::from(p.1) - y).abs() + (f64::from(p.2) - z).abs()
                < 12.0
        });
        if near {
            self.explosions_stopped += 1;
        }
        near
    }

    fn before_entity_spawn(&mut self, _dimension: &str, entity: &Entity, _reason: &str) -> bool {
        self.before.entity_spawn(entity)
    }
    fn before_entity_hurt(
        &mut self,
        _dimension: &str,
        entity: &Entity,
        _damage_type: &str,
        _attacker: Option<&str>,
        amount: f32,
    ) -> f32 {
        self.before.entity_hurt(entity, amount)
    }
    fn before_player_hurt(
        &mut self,
        _player: &Player,
        damage_type: &str,
        _attacker: Option<&str>,
        amount: f32,
    ) -> f32 {
        self.before.player_hurt(damage_type, amount)
    }
    fn before_player_death(&mut self, player: &Player, _damage_type: &str) -> bool {
        let spared = self.before.player_death();
        if spared {
            say(player, "Not this time (/showcase before)");
        }
        spared
    }
    fn before_explosion_blocks(
        &mut self,
        dimension: &str,
        _at: (f64, f64, f64),
        blocks: &[BlockPos],
    ) -> Vec<BlockPos> {
        self.before.explosion_blocks(dimension, blocks)
    }
    fn before_item_pickup(&mut self, _player: &Player, stack: &ItemStack) -> bool {
        self.before.item_pickup(stack)
    }
    fn before_item_toss(&mut self, _player: &Player, stack: &ItemStack) -> bool {
        self.before.item_toss(stack)
    }
    fn before_mob_drop(
        &mut self,
        _dimension: &str,
        entity: &Entity,
        _killer: Option<&str>,
        drops: &[ItemStack],
    ) -> Option<Vec<ItemStack>> {
        self.before.mob_drop(entity, drops)
    }
    fn before_xp_drop(
        &mut self,
        _dimension: &str,
        _at: (f64, f64, f64),
        _source: &str,
        amount: u32,
    ) -> u32 {
        self.before.xp_drop(amount)
    }
    fn before_container_open(&mut self, player: &Player, _pos: BlockPos, block: &str) -> bool {
        self.before.container_open(player, block)
    }

    fn net_received(&mut self, player: &Player, channel: &str, _data: &[u8]) {
        // The panel's reactor: the nearest one, its slots and charge, or a slot's items out.
        if channel == "showcase/slots" || channel == "showcase/take" {
            let dim = &player.dimension;
            let near = world::data_positions(dim).into_iter().min_by_key(|p| {
                let (dx, dz) = (f64::from(p.0) - player.x, f64::from(p.2) - player.z);
                (dx * dx + dz * dz) as i64
            });
            let Some((x, y, z)) = near else {
                let _ = server_net::send(&player.id, "showcase/slots", b"0 1");
                return;
            };
            if channel == "showcase/take" {
                let slot = _data.first().copied().unwrap_or(0) as usize;
                let items = storages::items(dim, x, y, z).unwrap_or_default();
                if let Some(Some(stack)) = items.get(slot)
                    && let Some(out) =
                        storages::extract_item(dim, x, y, z, Some(&stack.item), stack.count, false)
                {
                    let _ = players::give(&player.id, &out);
                }
            }
            let (energy, capacity) = storages::energy(dim, x, y, z).unwrap_or((0, 1));
            let mut reply = format!("{energy} {capacity}");
            for stack in storages::items(dim, x, y, z).unwrap_or_default() {
                let (item, count) = stack.map_or(("-".to_owned(), 0), |s| (s.item, s.count));
                reply.push_str(&format!("\n{item} {count}"));
            }
            let _ = server_net::send(&player.id, "showcase/slots", reply.as_bytes());
            return;
        }
        if channel == "showcase/ping" {
            let _ = server_net::send(
                &player.id,
                "showcase/pong",
                self.status(&player.dimension).as_bytes(),
            );
        }
    }
}

horizon_mod_sdk::export_both!(client::ShowcaseHud, Showcase);
