//! `/showcase api loot`, then `/showcase api lootcheck`: every function of
//! API level 20 (`loot`, `recipes-server`, the recipe book), each checked
//! against vanilla 26.3. The first places the chest the second fills: a
//! block lands at the tick's end.
use horizon_mod_sdk::server::{ItemStack, Player, loot, players, recipes_server as recipes, world};

const DUNGEON: &str = "minecraft:chests/simple_dungeon";

/// Where the chest goes: two blocks above the player.
fn chest_at(player: &Player) -> (i32, i32, i32) {
    (
        player.x.floor() as i32,
        player.y.floor() as i32 + 2,
        player.z.floor() as i32,
    )
}

pub fn set(player: &Player) -> Result<String, String> {
    let (x, y, z) = chest_at(player);
    world::set_block(&player.dimension, x, y, z, "minecraft:chest")?;
    Ok("api: a chest above you, now /showcase api lootcheck".into())
}

pub fn check(player: &Player) -> Result<String, String> {
    let me = player.id.as_str();
    let dim = player.dimension.as_str();
    let (x, y, z) = chest_at(player);
    let mut bad = Vec::new();
    let mut ok = 0;
    let mut check = |name: &str, passed: bool| {
        if passed {
            ok += 1
        } else {
            bad.push(name.to_owned())
        }
    };
    let items = |stacks: &[ItemStack]| -> Vec<(String, u32)> {
        stacks.iter().map(|s| (s.item.clone(), s.count)).collect()
    };

    // Loot.
    check(
        "table-exists",
        loot::table_exists(DUNGEON) && !loot::table_exists("minecraft:chests/no_such"),
    );
    let first = loot::roll(DUNGEON, Some(7))?;
    check(
        "roll",
        !first.is_empty()
            && items(&loot::roll(DUNGEON, Some(7))?) == items(&first)
            && loot::roll("minecraft:no_such", None).is_err(),
    );
    check(
        "block-drops",
        items(&loot::block_drops("minecraft:stone", None, Some(1))?)
            == [("minecraft:cobblestone".to_owned(), 1)]
            && loot::block_drops(
                "minecraft:stone",
                Some(&ItemStack::new("minecraft:diamond_pickaxe", 1)),
                Some(1),
            )
            .is_ok_and(|d| items(&d) == [("minecraft:cobblestone".to_owned(), 1)]),
    );
    check(
        "entity-drops",
        loot::entity_drops("minecraft:zombie", true, Some(3)).is_ok()
            && loot::entity_drops("minecraft:no_such", false, None).is_err(),
    );
    check(
        "give",
        loot::give(me, DUNGEON, Some(9)).is_ok_and(|g| !g.is_empty()),
    );
    check(
        "drop",
        loot::drop(dim, x, y + 1, z, DUNGEON, Some(11)).is_ok_and(|d| !d.is_empty()),
    );
    check(
        "fill-container",
        loot::fill_container(dim, x, y, z, DUNGEON, Some(13)).is_ok_and(|n| n > 0)
            && loot::fill_container(dim, x, y + 5, z, DUNGEON, None).is_err(),
    );

    // Recipes.
    let planks = "minecraft:oak_planks".to_owned();
    check("all", recipes::all().contains(&planks));
    check(
        "by-type",
        recipes::by_type("minecraft:crafting_shapeless").contains(&planks)
            && !recipes::by_type("minecraft:smelting").contains(&planks),
    );
    check(
        "by-result",
        recipes::by_result("minecraft:oak_planks").contains(&planks),
    );
    check(
        "by-ingredient",
        recipes::by_ingredient("minecraft:oak_log").contains(&planks),
    );
    check(
        "get",
        recipes::get("minecraft:oak_planks").is_some_and(|j| j.contains("crafting_shapeless"))
            && recipes::get("minecraft:../../x").is_none(),
    );
    let log = Some(ItemStack::new("minecraft:oak_log", 1));
    check(
        "match-crafting",
        recipes::match_crafting(&[log.clone(), None, None, None])
            .is_some_and(|(_, s)| s.item == "minecraft:oak_planks" && s.count == 4)
            && recipes::match_crafting(&[log, None]).is_none(),
    );
    let raw = ItemStack::new("minecraft:raw_iron", 1);
    check(
        "match-smelting",
        recipes::match_smelting(&raw).is_some_and(|(_, s)| s.item == "minecraft:iron_ingot"),
    );
    check(
        "match-stonecutting",
        recipes::match_stonecutting(&ItemStack::new("minecraft:stone", 1))
            .iter()
            .any(|(_, s)| s.item == "minecraft:stone_bricks"),
    );
    check("smelting-time", recipes::smelting_time(&raw) == Some(200));
    check(
        "smelting-xp",
        recipes::smelting_xp(&raw).is_some_and(|xp| (xp - 0.7).abs() < 0.01),
    );
    check(
        "of-own-type",
        recipes::of_own_type("showcase:grinding")
            .iter()
            .any(|(id, json)| id == "showcase:grind_sun_ore" && json.contains("glowstone"))
            && recipes::of_own_type("minecraft:smelting").is_empty(),
    );

    // The recipe book.
    check(
        "unlock-recipe",
        players::unlock_recipe(me, "minecraft:oak_planks").is_ok(),
    );
    check(
        "lock-recipe",
        players::lock_recipe(me, "minecraft:oak_planks").is_ok()
            && players::lock_recipe(me, "bad id").is_err(),
    );

    Ok(if bad.is_empty() {
        format!("level 20: all {ok} answered right")
    } else {
        format!("level 20: {ok} right, wrong: {}", bad.join(", "))
    })
}
