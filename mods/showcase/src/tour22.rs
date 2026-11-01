//! `/showcase api extra`, `extracheck`, then `extraafter`: every function of
//! API level 22, in the air beside the player. The first clears a space and
//! builds; the second reads it and changes it; the third sees the changes,
//! which land at the tick's end.
use horizon_mod_sdk::server::{
    Player, admin, crowd, fx, noise, patterns, players, shapes, stock, storages, timers, world,
};

type Pos = (i32, i32, i32);
const STONE: &str = "minecraft:stone";
const IRON: &str = "minecraft:iron_block";
const GOLD: &str = "minecraft:gold_block";

fn base(player: &Player) -> Result<Pos, String> {
    let bytes = world::get_saved("tour22_base").ok_or("no space: /showcase api extra first")?;
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let n: Vec<i32> = text.split(' ').filter_map(|s| s.parse().ok()).collect();
    let [x, y, z] = n[..] else {
        return Err(format!("{player:?} saved no base"));
    };
    Ok((x, y, z))
}
fn at(b: Pos, x: i32, y: i32, z: i32) -> Pos {
    (b.0 + x, b.1 + y, b.2 + z)
}
fn press() -> Result<(), String> {
    patterns::define(
        "press",
        &[
            vec!["iii".to_owned(), "i@i".to_owned(), "iii".to_owned()],
            vec!["   ".to_owned(), " g ".to_owned(), "   ".to_owned()],
        ],
        &[
            patterns::PatternKey {
                symbol: "i".to_owned(),
                blocks: vec![IRON.to_owned()],
            },
            patterns::PatternKey {
                symbol: "@".to_owned(),
                blocks: vec![GOLD.to_owned()],
            },
            patterns::PatternKey {
                symbol: "g".to_owned(),
                blocks: vec!["minecraft:glass".to_owned()],
            },
        ],
    )
}

pub fn set(player: &Player) -> Result<String, String> {
    let dim = player.dimension.as_str();
    let b = (
        player.x.floor() as i32 + 3,
        player.y.floor() as i32 + 12,
        player.z.floor() as i32 + 3,
    );
    world::set_saved("tour22_base", format!("{} {} {}", b.0, b.1, b.2).as_bytes())?;
    let (x, y, z) = b;
    world::fill(dim, x, y, z, x + 15, y + 9, z + 15, "minecraft:air")?;
    shapes::line(dim, b, at(b, 3, 0, 0), STONE)?;
    shapes::disc(dim, at(b, 3, 2, 3), 1, "minecraft:oak_planks")?;
    shapes::sphere(dim, at(b, 8, 2, 3), 1, "minecraft:glass")?;
    shapes::hollow_sphere(dim, at(b, 3, 6, 8), 2, "minecraft:bricks")?;
    shapes::cylinder(dim, at(b, 8, 0, 8), 1, 3, IRON)?;
    shapes::hollow_box(dim, at(b, 0, 0, 10), at(b, 3, 3, 13), "minecraft:sandstone")?;
    shapes::walls(dim, at(b, 5, 0, 12), at(b, 7, 2, 14), "minecraft:dirt")?;
    press()?;
    let built = patterns::build(dim, at(b, 12, 1, 3), "press", patterns::Facing::East)?;
    Ok(format!(
        "api: built, press {built} cells, now /showcase api extracheck"
    ))
}

struct Checks {
    ok: u32,
    bad: Vec<&'static str>,
}
impl Checks {
    fn check(&mut self, name: &'static str, passed: bool) {
        if passed {
            self.ok += 1
        } else {
            self.bad.push(name)
        }
    }
    fn report(self, level: &str) -> String {
        if self.bad.is_empty() {
            format!("{level}: all {} answered right", self.ok)
        } else {
            format!("{level}: {} right, wrong: {}", self.ok, self.bad.join(", "))
        }
    }
}

fn check_shapes(c: &mut Checks, dim: &str, b: Pos) -> Result<(), String> {
    let line = shapes::flood(dim, b, &[STONE.to_owned()], 64);
    c.check("line, flood", line.len() == 4 && line[0] == b);
    let plank = "minecraft:oak_planks".to_owned();
    c.check(
        "disc, flood-diagonal",
        shapes::flood_diagonal(dim, at(b, 3, 2, 3), &[plank], 64).len() == 5,
    );
    let ball = shapes::histogram(dim, at(b, 7, 1, 2), at(b, 9, 3, 4))?;
    c.check(
        "sphere, histogram",
        ball.iter().any(|h| h.0 == "minecraft:glass" && h.1 == 7),
    );
    c.check(
        "hollow-sphere, rotated-state",
        world::block_id(dim, b.0 + 3, b.1 + 6, b.2 + 8).as_deref() == Some("minecraft:air")
            && world::block_id(dim, b.0 + 5, b.1 + 6, b.2 + 8).as_deref()
                == Some("minecraft:bricks")
            && shapes::rotated_state("a:b[facing=north]", 1, false) == "a:b[facing=east]",
    );
    c.check(
        "cylinder, column",
        shapes::column(dim, b.0 + 8, b.2 + 8, b.1, b.1 + 2)
            .is_some_and(|s| s.iter().all(|s| s == IRON)),
    );
    let sand = shapes::histogram(dim, at(b, 0, 0, 10), at(b, 3, 3, 13))?;
    c.check(
        "hollow-box",
        sand.iter().any(|h| h.0 == "minecraft:air" && h.1 == 8),
    );
    let dirt = shapes::histogram(dim, at(b, 5, 0, 12), at(b, 7, 2, 14))?;
    c.check(
        "walls",
        dirt.iter().any(|h| h.0 == "minecraft:dirt" && h.1 == 24),
    );
    let ray = shapes::raycast(
        dim,
        f64::from(b.0) - 3.5,
        f64::from(b.1) + 0.5,
        f64::from(b.2) + 0.5,
        1.0,
        0.0,
        0.0,
        16.0,
    );
    c.check(
        "raycast",
        ray.is_some_and(|r| r.pos == b && r.before == at(b, -1, 0, 0) && r.state == STONE),
    );
    let (px, py, pz) = (f64::from(b.0), f64::from(b.1) + 0.5, f64::from(b.2) + 0.5);
    c.check(
        "clear-between",
        shapes::clear_between(dim, px - 3.0, py + 1.0, pz, px + 3.0, py + 1.0, pz)
            && !shapes::clear_between(dim, px - 3.0, py, pz, px + 8.0, py, pz),
    );
    c.check(
        "nearest-block",
        shapes::nearest_block(dim, at(b, 8, 5, 3), 8, "minecraft:glass") == Some(at(b, 8, 3, 3)),
    );
    c.check(
        "surface-below, ceiling-above",
        shapes::surface_below(dim, b.0, b.1 + 5, b.2) == Some(b.1)
            && shapes::ceiling_above(dim, b.0, b.1 - 3, b.2).is_some(),
    );
    let around = shapes::neighbors(dim, b.0 + 1, b.1, b.2);
    c.check(
        "neighbors",
        around.len() == 6 && around[4].as_deref() == Some(STONE),
    );
    c.check(
        "is-box-clear",
        shapes::is_box_clear(dim, at(b, 10, 5, 10), at(b, 15, 9, 15))?
            && !shapes::is_box_clear(dim, b, at(b, 1, 1, 1))?,
    );
    let found = shapes::find_any(dim, b, at(b, 9, 3, 4), &[STONE.to_owned()], 2)?;
    c.check("find-any", found.len() == 2);
    let copied = shapes::copy_box(dim, b, at(b, 3, 0, 0))?;
    c.check("copy-box", copied.len() == 4);
    shapes::stamp(dim, at(b, 12, 0, 12), 1, false, &copied)?;
    c.check(
        "replace-in-box",
        shapes::replace_in_box(dim, b, at(b, 3, 0, 0), STONE, "minecraft:cobblestone")? == 4,
    );
    c.check(
        "replace-in-sphere",
        shapes::replace_in_sphere(
            dim,
            at(b, 8, 2, 3),
            1,
            "minecraft:glass",
            "minecraft:tinted_glass",
        )? == 7,
    );
    Ok(())
}

fn check_patterns(c: &mut Checks, dim: &str, b: Pos) -> Result<(), String> {
    let anchor = at(b, 12, 1, 3);
    let east = patterns::Facing::East;
    c.check(
        "define, names, size",
        patterns::names().contains(&"press".to_owned())
            && patterns::size("press") == Some((3, 2, 3)),
    );
    c.check(
        "build, match-facing",
        patterns::match_facing(dim, anchor, "press", east),
    );
    c.check(
        "match-at",
        patterns::match_at(dim, anchor, "press").is_some(),
    );
    c.check(
        "missing",
        patterns::missing(dim, anchor, "press", east).is_empty()
            && patterns::missing(dim, at(b, 12, 6, 12), "press", east).len() == 10,
    );
    let found = patterns::find_in_box(dim, at(b, 10, 0, 0), at(b, 15, 3, 6), "press", 4)?;
    c.check("find-in-box", found.len() == 1 && found[0].0 == anchor);
    c.check(
        "cells-of",
        patterns::cells_of(anchor, "press", east, "g") == [at(anchor, 0, 1, 0)]
            && patterns::cells_of(anchor, "press", east, "i").len() == 8,
    );
    patterns::define(
        "gone",
        &[vec!["@".to_owned()]],
        &[patterns::PatternKey {
            symbol: "@".to_owned(),
            blocks: vec![GOLD.to_owned()],
        }],
    )?;
    c.check(
        "remove",
        patterns::remove("gone") && !patterns::remove("gone"),
    );
    Ok(())
}

fn check_noise(c: &mut Checks) {
    let near = |v: f64| v.abs() <= 1.2;
    c.check(
        "perlin2, perlin3",
        near(noise::perlin2(1, 0.3, 0.7)) && near(noise::perlin3(1, 0.3, 0.7, 0.9)),
    );
    c.check(
        "fbm2, fbm3, stable",
        noise::fbm2(1, 0.3, 0.7, 3) == noise::fbm2(1, 0.3, 0.7, 3)
            && near(noise::fbm3(1, 0.3, 0.7, 0.9, 3)),
    );
    c.check("ridged2", noise::ridged2(1, 0.3, 0.7, 3) >= 0.0);
    c.check("voronoi2", noise::voronoi2(1, 0.3, 0.7).0 < 2.0);
    c.check(
        "fill2",
        noise::fill2(1, 0.0, 0.0, 0.1, 16, 16, 2).len() == 256,
    );
    c.check(
        "fill3",
        noise::fill3(1, 0.0, 0.0, 0.0, 0.1, 16, 384, 16, 1).len() == 98304,
    );
    c.check(
        "hash-pos, chunk-seed",
        noise::hash_pos(1, 1, 2, 3) != noise::hash_pos(1, 1, 2, 4)
            && noise::chunk_seed(1, 1, 2) != noise::chunk_seed(2, 1, 2),
    );
}

fn check_crowd(c: &mut Checks, player: &Player, dim: &str) -> Result<(), String> {
    let me = player.id.as_str();
    let (x, y, z) = (player.x, player.y, player.z);
    let pos = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
    players::give(me, &players::ItemStack::new("minecraft:apple", 3))?;
    c.check("names", crowd::names().contains(&player.name));
    c.check(
        "in-dimension",
        crowd::in_dimension(dim).iter().any(|p| p.id == me),
    );
    c.check(
        "in-box",
        crowd::in_box(dim, at(pos, -1, -1, -1), at(pos, 1, 1, 1)).len() == 1,
    );
    c.check(
        "nearest",
        crowd::nearest(dim, x, y, z).is_some_and(|p| p.id == me),
    );
    c.check("sneaking", crowd::sneaking().is_empty());
    let held = players::held_item(me).map(|s| s.item);
    c.check(
        "holding",
        held.is_none_or(|i| crowd::holding(&i).iter().any(|p| p.id == me)),
    );
    c.check(
        "tell-near",
        crowd::tell_near(dim, x, y, z, 8.0, "tell-near") == 1,
    );
    c.check(
        "tell-dimension",
        crowd::tell_dimension(dim, "tell-dimension") == 1,
    );
    c.check("tell-others", crowd::tell_others(me, "nobody") == 0);
    c.check("title-all", crowd::title_all("Level 22", None)? == 1);
    c.check(
        "title-near",
        crowd::title_near(dim, x, y, z, 8.0, "Level 22", Some("near"))? == 1,
    );
    c.check("action-bar-all", crowd::action_bar_all("all")? == 1);
    c.check(
        "action-bar-near",
        crowd::action_bar_near(dim, x, y, z, 8.0, "near")? == 1,
    );
    c.check(
        "sound-near",
        crowd::sound_near(
            dim,
            x,
            y,
            z,
            8.0,
            "minecraft:block.note_block.pling",
            1.0,
            1.0,
        )? == 1,
    );
    c.check(
        "effect-near",
        crowd::effect_near(dim, x, y, z, 8.0, "minecraft:glowing", 30, 0)? == 1,
    );
    c.check(
        "give-near",
        crowd::give_near(
            dim,
            x,
            y,
            z,
            8.0,
            &players::ItemStack::new("minecraft:carrot", 2),
        )? == 1,
    );
    Ok(())
}

fn check_fx(c: &mut Checks, player: &Player, dim: &str) -> Result<(), String> {
    let p = "minecraft:end_rod";
    let (x, y, z) = (player.x, player.y + 1.0, player.z);
    fx::line(dim, p, x, y, z, x + 3.0, y, z, 8)?;
    fx::circle(dim, p, x, y, z, 2.0, 16)?;
    fx::sphere(dim, p, x, y, z, 2.0, 16)?;
    fx::helix(dim, p, x, y, z, 1.0, 3.0, 2.0, 16)?;
    fx::box_edges(dim, p, x, y, z, x + 2.0, y + 2.0, z + 2.0, 24)?;
    fx::cone(dim, p, x, y, z, 2.0, 2.0, 16)?;
    c.check(
        "fx, too far refused",
        fx::line(dim, p, 1e9, y, z, x, y, z, 2).is_err(),
    );
    Ok(())
}

fn check_stock(c: &mut Checks, dim: &str, b: Pos) -> Result<(), String> {
    let m = at(b, 8, 2, 8);
    let n = at(b, 8, 1, 8);
    for (x, y, z) in [m, n] {
        storages::declare(
            dim,
            x,
            y,
            z,
            &storages::StorageShape {
                slots: 4,
                energy_capacity: 100,
                tanks: vec![1000],
            },
        )?;
    }
    let iron = "minecraft:iron_ingot".to_owned();
    let wood = "minecraft:oak_log".to_owned();
    let left = stock::insert_many(
        dim,
        m,
        &[
            players::ItemStack::new(&iron, 32),
            players::ItemStack::new(&wood, 64),
        ],
    );
    c.check("insert-many", left.is_empty());
    c.check("total-items", stock::total_items(dim, m) == 96);
    c.check("slots-of", stock::slots_of(dim, m, &iron) == [0]);
    // 32/64 and 64/64 over 4 slots: 1 + floor(0.375 * 14).
    c.check("comparator-level", stock::comparator_level(dim, m) == 6);
    storages::set_energy(dim, m.0, m.1, m.2, 25)?;
    c.check(
        "energy-fraction",
        stock::energy_fraction(dim, m) == Some(0.25),
    );
    storages::insert_fluid(
        dim,
        m.0,
        m.1,
        m.2,
        &storages::FluidStack {
            fluid: "minecraft:water".to_owned(),
            amount: 500,
        },
        false,
    );
    c.check(
        "fluid-fraction",
        stock::fluid_fraction(dim, m, 0) == Some(0.5),
    );
    stock::swap_slots(dim, m, 0, 2)?;
    c.check("swap-slots", stock::slots_of(dim, m, &iron) == [2]);
    c.check("move-slot", stock::move_slot(dim, m, 2, 3)? == 32);
    c.check(
        "has-many",
        stock::has_many(dim, m, &[(iron.clone(), 32), (wood.clone(), 64)])
            && !stock::has_many(dim, m, &[(iron.clone(), 33)]),
    );
    c.check(
        "take-many",
        !stock::take_many(dim, m, &[(iron.clone(), 1), (wood.clone(), 65)])
            && stock::total_items(dim, m) == 96
            && stock::take_many(dim, m, &[(iron.clone(), 2)])
            && stock::total_items(dim, m) == 94,
    );
    c.check(
        "transfer-matching",
        stock::transfer_matching(dim, m, n, std::slice::from_ref(&iron), 10) == 10
            && stock::total_items(dim, n) == 10
            && stock::total_items(dim, m) == 84,
    );
    c.check(
        "extract-all",
        stock::extract_all(dim, m, &iron)
            .iter()
            .map(|s| s.count)
            .sum::<u32>()
            == 20,
    );
    Ok(())
}

pub fn check(player: &Player) -> Result<String, String> {
    let dim = player.dimension.as_str();
    let b = base(player)?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    check_shapes(&mut c, dim, b)?;
    check_patterns(&mut c, dim, b)?;
    check_noise(&mut c);
    check_crowd(&mut c, player, dim)?;
    check_fx(&mut c, player, dim)?;
    check_stock(&mut c, dim, b)?;

    timers::cancel_all();
    timers::after("tour_once", 1)?;
    timers::every("tour_every", 1)?;
    timers::after("tour_cancelled", 100)?;
    c.check(
        "cancel",
        timers::cancel("tour_cancelled") && !timers::cancel("x"),
    );
    c.check("pending", timers::pending().len() == 2);
    c.check("remaining", timers::remaining("tour_once") == Some(1));
    timers::cooldown_set(&player.id, "tour", 1000)?;
    timers::cooldown_set(&player.id, "tour2", 1000)?;
    timers::cooldown_set("other", "tour", 1000)?;
    c.check(
        "cooldown-set, ready, left",
        !timers::cooldown_ready(&player.id, "tour")
            && timers::cooldown_left(&player.id, "tour") == 1000
            && timers::cooldown_ready(&player.id, "none"),
    );
    timers::cooldown_clear(&player.id, "tour");
    c.check("cooldown-clear", timers::cooldown_ready(&player.id, "tour"));
    timers::cooldown_clear_owner(&player.id);
    c.check(
        "cooldown-clear-owner",
        timers::cooldown_ready(&player.id, "tour2") && !timers::cooldown_ready("other", "tour"),
    );

    admin::clone_box(
        dim,
        at(b, 8, 0, 8),
        at(b, 8, 2, 8),
        at(b, 15, 7, 15),
        false,
        false,
    )?;
    admin::default_game_mode("survival")?;
    c.check(
        "default-game-mode refuses",
        admin::default_game_mode("x; y").is_err(),
    );
    admin::spectate(&player.id, None)?;
    admin::spread_players(
        player.x,
        player.z,
        1.0,
        2.0,
        std::slice::from_ref(&player.id),
    )?;
    admin::run_function("showcase:none")?;
    admin::enchant_held(&player.id, "minecraft:unbreaking", 1)?;
    Ok(c.report("level 22, now /showcase api extraafter"))
}

pub fn after(player: &Player) -> Result<String, String> {
    let dim = player.dimension.as_str();
    let me = player.id.as_str();
    let b = base(player)?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    c.check(
        "stamp turned the line",
        (0..4).all(|i| world::block_id(dim, b.0 + 12, b.1, b.2 + 12 + i).as_deref() == Some(STONE)),
    );
    c.check(
        "replace-in-box landed",
        world::block_id(dim, b.0, b.1, b.2).as_deref() == Some("minecraft:cobblestone"),
    );
    c.check(
        "replace-in-sphere landed",
        world::block_id(dim, b.0 + 8, b.1 + 2, b.2 + 3).as_deref()
            == Some("minecraft:tinted_glass"),
    );
    c.check(
        "clone-box",
        world::block_id(dim, b.0 + 15, b.1 + 8, b.2 + 15).as_deref() == Some(IRON),
    );
    let due = timers::poll();
    c.check("poll", due.contains(&"tour_once".to_owned()));
    c.check(
        "every comes again, once doesn't",
        timers::pending() == ["tour_every"],
    );
    timers::cancel_all();
    c.check("cancel-all", timers::pending().is_empty());
    let apples = vec![("minecraft:apple".to_owned(), 3)];
    c.check(
        "crowd.having",
        crowd::having("minecraft:apple", 3)
            .iter()
            .any(|p| p.id == me),
    );
    c.check("crowd.has-many", crowd::has_many(me, &apples));
    c.check(
        "crowd.slots-of",
        !crowd::slots_of(me, "minecraft:apple").is_empty(),
    );
    c.check("crowd.take-many", crowd::take_many(me, &apples));
    c.check(
        "crowd.give-near landed",
        crowd::has_many(me, &[("minecraft:carrot".to_owned(), 2)]),
    );
    c.check(
        "take-many refuses too many",
        !crowd::take_many(me, &[("minecraft:carrot".to_owned(), 100_000)]),
    );
    Ok(c.report("level 22 afterwards"))
}
