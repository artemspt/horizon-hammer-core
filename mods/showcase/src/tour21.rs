//! `/showcase api mobs`, `mobscheck`, then `mobsafter`: every function of
//! API level 21 (a mob's state). The first spawns a cow and a wolf and
//! drops an apple; the second reads them and changes them; the third sees
//! the changes, which land at the tick's end.
use horizon_mod_sdk::server::{ItemStack, Player, entities, world};

fn saved_id(key: &str) -> Result<String, String> {
    let bytes = world::get_saved(key).ok_or("no mob: /showcase api mobs first")?;
    String::from_utf8(bytes).map_err(|_| "a saved id that isn't text".to_string())
}

pub fn set(player: &Player) -> Result<String, String> {
    let dim = player.dimension.as_str();
    let (x, y, z) = (player.x + 2.0, player.y, player.z);
    let cow = entities::spawn_with_reason(dim, "minecraft:cow", x, y, z, "showcase_tour")?;
    let wolves = entities::spawn_group(dim, "minecraft:wolf", &[(x, y, z + 2.0)], "showcase_tour")?;
    let wolf = wolves.first().ok_or("no wolf")?;
    world::set_saved("tour_cow", cow.as_bytes())?;
    world::set_saved("tour_wolf", wolf.as_bytes())?;
    entities::drop_item(
        dim,
        x.floor() as i32,
        y.floor() as i32,
        (z - 2.0).floor() as i32,
        &ItemStack::new("minecraft:apple", 1),
    )?;
    Ok("api: a cow, a wolf and an apple, now /showcase api mobscheck".into())
}

struct Checks {
    ok: u32,
    bad: Vec<String>,
}
impl Checks {
    fn check(&mut self, name: &str, passed: bool) {
        if passed {
            self.ok += 1
        } else {
            self.bad.push(name.to_owned())
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

pub fn check(player: &Player) -> Result<String, String> {
    let me = player.id.as_str();
    let dim = player.dimension.as_str();
    let cow = saved_id("tour_cow")?;
    let wolf = saved_id("tour_wolf")?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    c.check(
        "kind",
        entities::kind(&cow).as_deref() == Some("minecraft:cow")
            && entities::kind(me).as_deref() == Some("minecraft:player"),
    );
    c.check(
        "position",
        entities::position(&cow).is_some_and(|p| (p.0 - player.x).abs() < 6.0),
    );
    c.check("rotation", entities::rotation(&cow).is_some());
    c.check("velocity", entities::velocity(&cow).is_some());
    c.check("health", entities::health(&cow) == Some(10.0));
    c.check("max-health", entities::max_health(&cow) == Some(10.0));
    c.check(
        "dimension",
        entities::dimension(&cow).as_deref() == Some(dim)
            && entities::dimension(me).as_deref() == Some(dim),
    );
    c.check(
        "is-alive",
        entities::is_alive(&cow) && !entities::is_alive("00000000-0000-0000-0000-00000000dead"),
    );
    c.check("is-on-ground", entities::is_on_ground(&cow));
    c.check("is-in-water", !entities::is_in_water(&cow));
    c.check("is-in-lava", !entities::is_in_lava(&cow));
    c.check("is-burning", !entities::is_burning(&cow));
    c.check("is-baby", !entities::is_baby(&cow));
    c.check("is-tamed", !entities::is_tamed(&wolf));
    c.check("owner", entities::owner(&wolf).is_none());
    c.check("custom-name", entities::custom_name(&cow).is_none());
    c.check("effects", entities::effects(&cow).is_empty());
    c.check("has-effect", !entities::has_effect(&cow, "minecraft:speed"));
    c.check(
        "attribute",
        entities::attribute(&cow, "minecraft:max_health") == Some(10.0),
    );
    c.check(
        "base-attribute",
        entities::base_attribute(&cow, "minecraft:movement_speed")
            .is_some_and(|s| (s - 0.2).abs() < 1e-6),
    );
    c.check("equipment", entities::equipment(&cow).is_empty());
    c.check("target", entities::target(&cow).is_none());
    c.check("passengers", entities::passengers(&cow).is_empty());
    c.check("vehicle", entities::vehicle(&wolf).is_none());
    c.check("tags", entities::tags(&cow).is_empty());
    c.check("has-tag", !entities::has_tag(&cow, "showcase_tour"));
    c.check(
        "distance",
        entities::distance(&cow, me).is_some_and(|d| d < 6.0),
    );
    let (px, py, pz) = (player.x, player.y, player.z);
    c.check(
        "in-box",
        entities::in_box(
            dim,
            px - 8.0,
            py - 4.0,
            pz - 8.0,
            px + 8.0,
            py + 4.0,
            pz + 8.0,
        )
        .iter()
        .any(|e| e.id == cow),
    );
    c.check("all", entities::all(dim).iter().any(|e| e.id == wolf));
    let apple = entities::items_near(dim, px, py, pz, 8.0)
        .into_iter()
        .find(|d| d.stack.item == "minecraft:apple");
    c.check("items-near", apple.is_some());

    // Changes, seen by `/showcase api mobsafter`.
    c.check("set-baby", entities::set_baby(&cow, true).is_ok());
    c.check("set-owner", entities::set_owner(&wolf, Some(me)).is_ok());
    c.check("set-fire", entities::set_fire(&wolf, 200).is_ok());
    c.check("set-glowing", entities::set_glowing(&cow, true).is_ok());
    c.check(
        "set-invisible",
        entities::set_invisible(&wolf, false).is_ok(),
    );
    c.check(
        "set-invulnerable",
        entities::set_invulnerable(&cow, true).is_ok(),
    );
    c.check("set-silent", entities::set_silent(&cow, true).is_ok());
    c.check(
        "set-no-gravity",
        entities::set_no_gravity(&wolf, false).is_ok(),
    );
    c.check(
        "set-persistent",
        entities::set_persistent(&cow, true).is_ok(),
    );
    c.check(
        "set-pickup-delay",
        apple.is_some_and(|a| entities::set_pickup_delay(&a.id, 32767).is_ok()),
    );
    c.check(
        "add-tag",
        entities::add_tag(&cow, "showcase_tour").is_ok()
            && entities::add_tag(&cow, "bad tag").is_err(),
    );
    c.check(
        "remove-tag",
        entities::remove_tag(&cow, "no_such_tag").is_ok(),
    );
    c.check(
        "merge-nbt",
        entities::merge_nbt(&cow, "{Fire:100s}").is_ok()
            && entities::merge_nbt(&cow, "Fire:1").is_err(),
    );
    c.check("look-at", entities::look_at(&cow, px, py + 1.6, pz).is_ok());
    c.check("knockback", entities::knockback(&wolf, 0.4, px, pz).is_ok());
    c.check(
        "add-velocity",
        entities::add_velocity(&cow, 0.0, 0.3, 0.0).is_ok(),
    );
    c.check("ride", entities::ride(&wolf, &cow).is_ok());
    c.check("xp-orb", entities::xp_orb(dim, px, py, pz, 7).is_ok());
    c.check(
        "spawn-with-reason",
        entities::spawn_with_reason(dim, "minecraft:cow", px, py, pz, "Bad Reason").is_err(),
    );
    c.check(
        "spawn-group",
        entities::spawn_group(dim, "minecraft:cow", &[(px, py, pz); 65], "showcase_tour").is_err(),
    );
    Ok(c.report("level 21 reads") + ", now /showcase api mobsafter")
}

pub fn after(player: &Player) -> Result<String, String> {
    let me = player.id.as_str();
    let cow = saved_id("tour_cow")?;
    let wolf = saved_id("tour_wolf")?;
    let mut c = Checks {
        ok: 0,
        bad: Vec::new(),
    };
    c.check("set-baby", entities::is_baby(&cow));
    c.check(
        "set-owner",
        entities::is_tamed(&wolf) && entities::owner(&wolf).as_deref() == Some(me),
    );
    c.check("set-fire", entities::is_burning(&wolf));
    c.check("merge-nbt", entities::is_burning(&cow));
    c.check("add-tag", entities::has_tag(&cow, "showcase_tour"));
    c.check(
        "ride",
        entities::vehicle(&wolf).as_deref() == Some(cow.as_str())
            && entities::passengers(&cow).contains(&wolf),
    );
    c.check("extinguish", entities::extinguish(&wolf).is_ok());
    c.check("dismount", entities::dismount(&wolf).is_ok());
    c.check("remove", entities::remove(&wolf).is_ok());
    Ok(c.report("level 21 changes") + " (the wolf is gone at the tick's end)")
}
