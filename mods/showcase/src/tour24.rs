//! `/showcase api l24`, `l24check`, then `l24after`: every function of API level
//! 24, in the air beside the player. Changes land at the tick's end, so each
//! step reads what the one before it asked for.
use horizon_mod_sdk::server::{
    ItemStack, Player, StorageShape, blocks, chunks, players, storages, world,
};

type Pos = (i32, i32, i32);
const STONE: &str = "minecraft:stone";
const GLOW: &str = "minecraft:glowstone";
const APPLE: &str = "minecraft:apple";

struct Checks {
    ok: u32,
    bad: Vec<String>,
}
impl Checks {
    fn check(&mut self, name: &str, passed: bool) {
        if passed {
            self.ok += 1;
        } else {
            self.bad.push(name.to_owned());
        }
    }
    fn report(self, what: &str) -> String {
        if self.bad.is_empty() {
            format!("{what}: {} checks, all right", self.ok)
        } else {
            format!("{what}: {} right, wrong: {}", self.ok, self.bad.join(", "))
        }
    }
}

fn stack(item: &str, count: u32) -> ItemStack {
    ItemStack {
        item: item.to_owned(),
        count,
        data: Vec::new(),
        components: Vec::new(),
    }
}
fn base() -> Result<Pos, String> {
    let bytes = world::get_saved("tour24_base").ok_or("no space: /showcase api l24 first")?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let n: Vec<i32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    match n[..] {
        [x, y, z] => Ok((x, y, z)),
        _ => Err("saved no base".to_owned()),
    }
}

pub fn set(player: &Player) -> Result<String, String> {
    let (dim, me) = (player.dimension.as_str(), player.id.as_str());
    let (x, y, z) = (
        player.x.floor() as i32 + 3,
        player.y.floor() as i32 + 12,
        player.z.floor() as i32 + 3,
    );
    world::set_saved("tour24_base", format!("{x} {y} {z}").as_bytes())?;
    world::fill(dim, x, y, z, x + 6, y + 3, z + 3, "minecraft:air")?;
    world::set_block(dim, x, y, z, "minecraft:chest")?;
    world::set_block(dim, x + 1, y, z, "minecraft:iron_block")?;
    world::set_block(dim, x + 2, y, z, "minecraft:gold_block")?;
    world::set_block(dim, x + 4, y, z, GLOW)?;
    world::set_block(dim, x + 6, y, z, "minecraft:water")?;

    players::set_food(me, 10, 2.0)?;
    players::set_air(me, 100)?;
    players::set_health(me, 7.0)?;
    players::set_xp(me, 5, 0.5)?;
    players::set_ender_chest_slot(me, 0, Some(&stack(APPLE, 2)))?;
    players::set_selected_slot(me, 3)?;
    players::set_may_fly(me, true)?;
    players::teleport_look(me, player.x, player.y, player.z, 10.0, 80.0)?;
    players::tell_json(me, r#"{"text":"tell-json works","color":"gold"}"#)?;
    players::broadcast_json(r#"{"text":"broadcast-json works","color":"aqua"}"#)?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    c.check(
        "tell-json refuses what isn't JSON",
        players::tell_json(me, "{").is_err(),
    );
    c.check(
        "set-food refuses too much",
        players::set_food(me, 21, 0.0).is_err(),
    );
    c.check(
        "set-health refuses 0",
        players::set_health(me, 0.0).is_err(),
    );
    c.check(
        "ender slot 27 refused",
        players::set_ender_chest_slot(me, 27, None).is_err(),
    );
    c.check("slot 9 refused", players::set_selected_slot(me, 9).is_err());
    c.check("nobody", players::set_air("nobody", 1).is_err());
    Ok(c.report("level 24 set"))
}

fn check_players(c: &mut Checks, player: &Player, me: &str) -> Result<(), String> {
    c.check(
        "food",
        players::food(me).is_some_and(|f| f.food == 10 && f.saturation <= 2.0),
    );
    c.check("air", players::air(me).is_some_and(|a| a < 300));
    c.check("max-health", players::max_health(me) == Some(20.0));
    c.check("set-health", player.health <= 8.0);
    c.check(
        "xp",
        players::xp(me).is_some_and(|x| x.level == 5 && x.progress > 0.3),
    );
    c.check(
        "ender-chest",
        players::ender_chest(me)
            .first()
            .is_some_and(|s| s.as_ref().is_some_and(|s| s.item == APPLE && s.count == 2)),
    );
    c.check(
        "position",
        players::position(me).is_some_and(|p| (p.0 - player.x).abs() < 1.0),
    );
    c.check("velocity", players::velocity(me).is_some());
    c.check("is-sprinting", !players::is_sprinting(me));
    c.check(
        "is-flying",
        !players::is_sleeping(me) && players::is_flying("nobody") == false,
    );
    c.check("is-on-ground", players::is_on_ground("nobody") == false);
    c.check("is-swimming", !players::is_swimming(me));
    c.check("is-gliding", !players::is_gliding(me));
    c.check("is-using", !players::is_using(me));
    c.check("is-op", players::is_op(me));
    c.check("ping", players::ping(me).is_some());
    c.check("spawn-point", players::spawn_point("nobody").is_none());
    c.check(
        "has-advancement",
        !players::has_advancement(me, "minecraft:story/nothing"),
    );
    c.check("raycast-block", players::raycast_block(me, 256.0).is_some());
    players::heal(me, 2.0)?;
    players::add_exhaustion(me, 0.5)?;
    players::close_window(me)?;
    players::set_velocity(me, 0.0, 0.3, 0.0)?;
    players::set_may_fly(me, false)?;
    Ok(())
}

fn check_blocks(c: &mut Checks, dim: &str, (x, y, z): Pos) -> Result<(), String> {
    c.check("all", blocks::all().iter().any(|b| b == STONE));
    c.check(
        "properties",
        blocks::properties("minecraft:oak_log")
            .iter()
            .any(|(n, v)| n == "axis" && v.len() == 3),
    );
    c.check(
        "default-state",
        blocks::default_state("minecraft:oak_log").as_deref() == Some("minecraft:oak_log[axis=y]"),
    );
    c.check("hardness", blocks::hardness(STONE) == Some(1.5));
    c.check(
        "hardness of bedrock",
        blocks::hardness("minecraft:bedrock") == Some(-1.0),
    );
    c.check("light-emission", blocks::light_emission(GLOW) == Some(15));
    c.check(
        "block-light-at",
        blocks::block_light_at(dim, x + 5, y, z) == Some(14),
    );
    c.check(
        "light-at",
        blocks::light_at(dim, x + 5, y, z).is_some_and(|l| l >= 14),
    );
    c.check(
        "sky-light-at",
        blocks::sky_light_at(dim, x + 5, y + 3, z).is_some(),
    );
    c.check("can-see-sky", blocks::can_see_sky(dim, x, y + 40, z));
    c.check(
        "is-solid",
        blocks::is_solid(STONE) && !blocks::is_solid("minecraft:air"),
    );
    c.check(
        "is-replaceable",
        blocks::is_replaceable("minecraft:air") && !blocks::is_replaceable(STONE),
    );
    c.check(
        "is-opaque",
        blocks::is_opaque(STONE) && !blocks::is_opaque("minecraft:glass"),
    );
    c.check(
        "has-collision",
        blocks::has_collision(STONE) && !blocks::has_collision("minecraft:air"),
    );
    c.check(
        "requires-tool",
        blocks::requires_tool(STONE) && !blocks::requires_tool("minecraft:dirt"),
    );
    c.check(
        "is-waterlogged",
        blocks::is_waterlogged("minecraft:oak_slab[waterlogged=true]")
            && !blocks::is_waterlogged(STONE),
    );
    c.check(
        "fluid-at",
        blocks::fluid_at(dim, x + 6, y, z).is_some_and(|(f, a)| f == "minecraft:water" && a == 8)
            && blocks::fluid_at(dim, x + 4, y, z).is_none(),
    );
    c.check(
        "has-block-entity",
        blocks::has_block_entity(dim, x, y, z) && !blocks::has_block_entity(dim, x + 4, y, z),
    );
    c.check(
        "get-nbt",
        blocks::get_nbt(dim, x, y, z).is_some_and(|n| n.contains("chest")),
    );
    blocks::merge_nbt(
        dim,
        x,
        y,
        z,
        r#"{Items:[{Slot:0b,id:"minecraft:apple",count:3}]}"#,
    )?;
    Ok(())
}

fn check_chunks(c: &mut Checks, dim: &str, (x, y, z): Pos) -> Result<(), String> {
    let (cx, sy, cz) = (x >> 4, y >> 4, z >> 4);
    let stone = chunks::state_id(STONE).ok_or("no number for stone")?;
    c.check(
        "state-of-id",
        chunks::state_of_id(stone).as_deref() == Some(STONE),
    );
    c.check(
        "state-id of nonsense",
        chunks::state_id("nonsense").is_none(),
    );
    let section = chunks::read_section(dim, cx, sy, cz).ok_or("no section")?;
    c.check("read-section", section.len() == 4096);
    let glow = (x + 4, y, z);
    let cell = |(x, y, z): Pos| (((y & 15) << 8) | ((z & 15) << 4) | (x & 15)) as usize;
    if glow.0 >> 4 == cx && glow.2 >> 4 == cz {
        c.check(
            "read-section's order",
            chunks::state_of_id(section[cell(glow)]).as_deref() == Some(GLOW),
        );
    }
    // Some air of this section: the box beside the player may cross into the next.
    let air = chunks::state_id("minecraft:air").ok_or("no number for air")?;
    let i = section
        .iter()
        .position(|&s| s == air)
        .ok_or("a section with no air")? as i32;
    let target = (
        cx * 16 + (i & 15),
        sy * 16 + (i >> 8),
        cz * 16 + ((i >> 4) & 15),
    );
    let text = format!("{} {} {}", target.0, target.1, target.2);
    world::set_saved("tour24_cell", text.as_bytes())?;
    let mut changed = section;
    changed[i as usize] = stone;
    chunks::write_section(dim, cx, sy, cz, &changed)?;
    c.check(
        "write-section wants 4096",
        chunks::write_section(dim, cx, sy, cz, &[1]).is_err(),
    );
    let heights = chunks::heightmap(dim, cx, cz).ok_or("no heightmap")?;
    c.check(
        "heightmap",
        heights.len() == 256 && heights[(z & 15) as usize * 16 + (x & 15) as usize] >= y,
    );
    c.check("loaded", chunks::loaded(dim).contains(&(cx, cz)));
    chunks::force_load(dim, cx + 40, cz)?;
    chunks::force_unload(dim, cx + 41, cz)?;
    chunks::set_data(dim, cx, cz, b"polluted")?;
    c.check(
        "get-data",
        chunks::get_data(dim, cx, cz).as_deref() == Some(b"polluted".as_slice()),
    );
    c.check("data-chunks", chunks::data_chunks(dim).contains(&(cx, cz)));
    c.check(
        "too much data",
        chunks::set_data(dim, cx, cz, &[0; 5000]).is_err(),
    );
    chunks::set_data(dim, cx, cz + 1, b"x")?;
    chunks::set_data(dim, cx, cz + 1, b"")?;
    c.check("empty removes", chunks::get_data(dim, cx, cz + 1).is_none());
    Ok(())
}

fn check_storages(c: &mut Checks, dim: &str, (x, y, z): Pos) -> Result<(), String> {
    let shape = StorageShape {
        slots: 4,
        energy_capacity: 1000,
        tanks: Vec::new(),
    };
    storages::declare(dim, x + 1, y, z, &shape)?;
    storages::declare(dim, x + 2, y, z, &shape)?;
    c.check(
        "side-config's default",
        storages::side_config(dim, x + 1, y, z, "east").as_deref() == Some("both"),
    );
    c.check(
        "a side that isn't",
        storages::set_side_config(dim, x + 1, y, z, "over", "in").is_err(),
    );
    c.check(
        "a mode that isn't",
        storages::set_side_config(dim, x + 1, y, z, "east", "up").is_err(),
    );
    storages::set_filter(dim, x + 1, y, z, &[APPLE.to_owned()])?;
    c.check("filter", storages::filter(dim, x + 1, y, z) == [APPLE]);
    c.check(
        "set-filter refuses",
        storages::insert_item(dim, x + 1, y, z, &stack("minecraft:dirt", 1), false) == 0,
    );
    c.check(
        "set-filter takes",
        storages::insert_item(dim, x + 1, y, z, &stack(APPLE, 5), false) == 5,
    );
    storages::set_filter(dim, x + 1, y, z, &[])?;
    storages::set_item(dim, x + 1, y, z, 2, Some(&stack("minecraft:bread", 1)))?;
    storages::set_item(dim, x + 1, y, z, 3, Some(&stack(APPLE, 3)))?;
    storages::compact(dim, x + 1, y, z)?;
    let items = storages::items(dim, x + 1, y, z).unwrap_or_default();
    c.check(
        "compact",
        items[0]
            .as_ref()
            .is_some_and(|s| s.item == APPLE && s.count == 8)
            && items[2].is_none(),
    );
    storages::sort(dim, x + 1, y, z)?;
    let items = storages::items(dim, x + 1, y, z).unwrap_or_default();
    c.check(
        "sort",
        items[0].as_ref().is_some_and(|s| s.item == APPLE)
            && items[1]
                .as_ref()
                .is_some_and(|s| s.item == "minecraft:bread"),
    );
    let neighbours = storages::neighbors(dim, x + 1, y, z);
    c.check(
        "neighbors",
        neighbours.len() == 2 && neighbours.contains(&"west".to_owned()),
    );
    storages::set_energy(dim, x + 1, y, z, 300)?;
    storages::set_energy(dim, x + 2, y, z, 40)?;
    c.check(
        "total-energy",
        storages::total_energy(dim, &[(x + 1, y, z), (x + 2, y, z), (x, y, z)]) == 340,
    );
    let saved = storages::save_to_stack(dim, x + 1, y, z, &stack("minecraft:iron_block", 1))?;
    c.check("save-to-stack", !saved.data.is_empty());
    storages::set_side_config(dim, x + 1, y, z, "east", "in")?;
    c.check(
        "side-config",
        storages::side_config(dim, x + 1, y, z, "east").as_deref() == Some("in"),
    );
    c.check(
        "transfer-all obeys sides",
        storages::transfer_all(dim, (x + 1, y, z), (x + 2, y, z)) == 0,
    );
    storages::set_side_config(dim, x + 1, y, z, "east", "out")?;
    c.check(
        "transfer-all",
        storages::transfer_all(dim, (x + 1, y, z), (x + 2, y, z)) == 9,
    );
    c.check(
        "emptied",
        storages::items(dim, x + 1, y, z)
            .unwrap_or_default()
            .iter()
            .all(Option::is_none),
    );
    storages::load_from_stack(dim, x + 1, y, z, &saved)?;
    c.check(
        "load-from-stack",
        storages::items(dim, x + 1, y, z)
            .unwrap_or_default()
            .first()
            .is_some_and(|s| s.as_ref().is_some_and(|s| s.count == 8)),
    );
    c.check(
        "load-from-stack wants a saved stack",
        storages::load_from_stack(dim, x + 1, y, z, &stack(APPLE, 1)).is_err(),
    );
    storages::drop_all(dim, x + 2, y, z)?;
    c.check(
        "drop-all",
        storages::shape(dim, x + 2, y, z).is_some()
            && storages::items(dim, x + 2, y, z)
                .unwrap_or_default()
                .iter()
                .all(Option::is_none),
    );
    Ok(())
}

pub fn check(player: &Player) -> Result<String, String> {
    let (dim, me) = (player.dimension.as_str(), player.id.as_str());
    let b = base()?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    check_players(&mut c, player, me)?;
    check_blocks(&mut c, dim, b)?;
    check_chunks(&mut c, dim, b)?;
    check_storages(&mut c, dim, b)?;
    Ok(c.report("level 24"))
}

pub fn after(player: &Player) -> Result<String, String> {
    let (dim, me) = (player.dimension.as_str(), player.id.as_str());
    let (x, y, z) = base()?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    let cell = world::get_saved("tour24_cell").ok_or("no cell saved")?;
    let cell = String::from_utf8(cell).map_err(|e| e.to_string())?;
    let n: Vec<i32> = cell.split(' ').filter_map(|s| s.parse().ok()).collect();
    c.check(
        "write-section landed",
        world::block_id(dim, n[0], n[1], n[2]).as_deref() == Some(STONE),
    );
    c.check(
        "merge-nbt landed",
        blocks::get_nbt(dim, x, y, z).is_some_and(|n| n.contains("minecraft:apple")),
    );
    c.check("heal landed", player.health > 8.0);
    c.check("held slot", players::selected_slot(me) == Some(3));
    chunks::force_unload(dim, (x >> 4) + 40, z >> 4)?;
    players::set_food(me, 20, 5.0)?;
    Ok(c.report("level 24 afterwards"))
}
