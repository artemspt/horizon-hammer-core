//! `/showcase api items`: every function of API level 19 (`items`,
//! `dimensions`), each checked against what vanilla 26.3 has.
use horizon_mod_sdk::server::{ItemStack, Player, dimensions, items};

fn stack(item: &str, count: u32) -> ItemStack {
    ItemStack::new(item, count)
}

/// Level 19: says how many answered right, and names the others.
pub fn run(player: &Player) -> Result<String, String> {
    let me = player.id.as_str();
    let dim = player.dimension.as_str();
    let mut bad = Vec::new();
    let mut ok = 0;
    let mut check = |name: &str, passed: bool| {
        if passed {
            ok += 1
        } else {
            bad.push(name.to_owned())
        }
    };

    // Item types.
    check(
        "all",
        items::all().iter().any(|i| i == "minecraft:diamond_sword"),
    );
    check(
        "exists-in-tag",
        items::exists_in_tag("#minecraft:logs")
            .is_some_and(|l| l.iter().any(|i| i == "minecraft:oak_log"))
            && items::exists_in_tag("minecraft:no_such_tag").is_none(),
    );
    check(
        "name",
        items::name("minecraft:diamond").is_some_and(|n| !n.is_empty()),
    );
    check(
        "max-damage",
        items::max_damage("minecraft:diamond_sword") == Some(1561),
    );
    check(
        "is-damageable",
        items::is_damageable("minecraft:diamond_sword")
            && !items::is_damageable("minecraft:diamond"),
    );
    check(
        "is-food",
        items::is_food("minecraft:apple") && !items::is_food("minecraft:stone"),
    );
    check(
        "food-values",
        items::food_values("minecraft:apple")
            .is_some_and(|f| f.nutrition == 4 && (f.saturation - 2.4).abs() < 0.01),
    );
    check(
        "is-block-item",
        items::is_block_item("minecraft:oak_planks") && !items::is_block_item("minecraft:stick"),
    );
    check(
        "block-of",
        items::block_of("minecraft:oak_planks").as_deref() == Some("minecraft:oak_planks"),
    );
    check(
        "fuel-ticks",
        items::fuel_ticks("minecraft:coal") == 1600 && items::fuel_ticks("minecraft:stone") == 0,
    );
    check(
        "rarity",
        items::rarity("minecraft:dirt").as_deref() == Some("common")
            && items::rarity("minecraft:dragon_egg").as_deref() == Some("epic"),
    );

    // A sword worn, repaired, named and enchanted.
    let sword = stack("minecraft:diamond_sword", 1);
    let worn = items::set_damage(&sword, 100)?;
    check("set-damage", items::get_damage(&worn) == 100);
    let mended = items::repair(&worn, 40)?;
    check("repair", items::get_damage(&mended) == 60);
    check("get-damage", items::get_damage(&sword) == 0);
    check(
        "components",
        items::components(&mended).is_ok_and(|c| c.contains("minecraft:damage"))
            && items::components(&sword).is_ok_and(|c| c == "{}"),
    );
    check(
        "get-component",
        items::get_component(&mended, "minecraft:damage").as_deref() == Some("60"),
    );
    let named = items::set_component(&sword, "minecraft:custom_name", "\"Edge\"")?;
    check(
        "set-component",
        items::get_custom_name(&named).as_deref() == Some("Edge"),
    );
    check("get-custom-name", items::get_custom_name(&sword).is_none());
    let unworn = items::remove_component(&mended, "minecraft:damage")?;
    check("remove-component", items::get_damage(&unworn) == 0);
    let told = items::set_component(&sword, "minecraft:lore", "[\"one\",\"two\"]")?;
    check("get-lore", items::get_lore(&told) == ["one", "two"]);
    check(
        "same-item",
        items::same_item(&sword, &worn) && !items::same_item(&sword, &stack("minecraft:apple", 1)),
    );
    check(
        "can-stack",
        items::can_stack(&stack("minecraft:apple", 5), &stack("minecraft:apple", 5))
            && !items::can_stack(&sword, &sword),
    );
    let (taken, left) = items::split(&stack("minecraft:apple", 5), 2);
    check("split", taken.count == 2 && left.count == 3);
    check(
        "grow",
        items::grow(&stack("minecraft:apple", 5), 100).count == 64
            && items::grow(&stack("minecraft:apple", 5), -9).count == 0,
    );
    let sharp = items::set_enchantment(&sword, "minecraft:sharpness", 3)?;
    check(
        "set-enchantment",
        items::enchantment_level(&sharp, "minecraft:sharpness") == 3,
    );
    check(
        "enchantment-level",
        items::enchantment_level(&sword, "minecraft:sharpness") == 0,
    );
    check(
        "enchantments",
        items::enchantments(&sharp) == [("minecraft:sharpness".to_owned(), 3)],
    );
    check(
        "has-glint",
        items::has_glint(&sharp) && !items::has_glint(&sword),
    );
    let plain = items::remove_enchantment(&sharp, "minecraft:sharpness")?;
    check("remove-enchantment", items::enchantments(&plain).is_empty());
    check(
        "attribute-modifiers",
        items::attribute_modifiers(&sword)
            .iter()
            .any(|m| m.attribute == "minecraft:attack_damage"),
    );
    let edged = items::add_attribute_modifier(
        &sword,
        &items::Modifier {
            attribute: "minecraft:attack_damage".into(),
            id: "showcase:edge".into(),
            amount: 2.0,
            operation: "add_value".into(),
            slot: "mainhand".into(),
        },
    )?;
    check(
        "add-attribute-modifier",
        items::attribute_modifiers(&edged)
            .iter()
            .any(|m| m.id == "showcase:edge" && m.amount == 2.0),
    );

    // Tools, dyes, potions, containers.
    let pickaxe = stack("minecraft:diamond_pickaxe", 1);
    check(
        "tool-speed",
        items::tool_speed(&pickaxe, "minecraft:stone") == 8.0
            && items::tool_speed(&stack("minecraft:stick", 1), "minecraft:stone") == 1.0,
    );
    check(
        "is-correct-tool",
        items::is_correct_tool(&pickaxe, "minecraft:obsidian")
            && !items::is_correct_tool(&stack("minecraft:wooden_pickaxe", 1), "minecraft:obsidian"),
    );
    let helmet = stack("minecraft:leather_helmet", 1);
    check("dyed-color", items::dyed_color(&helmet).is_none());
    let blue = items::set_dyed_color(&helmet, Some(0x3366FF))?;
    check("set-dyed-color", items::dyed_color(&blue) == Some(0x3366FF));
    let swift =
        items::set_potion_contents(&stack("minecraft:potion", 1), Some("minecraft:swiftness"))?;
    check(
        "set-potion-contents",
        items::potion_contents(&swift).is_some_and(|(p, _)| p == "minecraft:swiftness"),
    );
    check(
        "potion-contents",
        items::potion_contents(&swift).is_some_and(|(_, e)| {
            e.len() == 1 && e[0].effect == "minecraft:speed" && e[0].duration == 3600
        }),
    );
    let boxed = items::set_component(
        &stack("minecraft:shulker_box", 1),
        "minecraft:container",
        "[{slot:0,item:{id:\"minecraft:apple\",count:3}}]",
    )?;
    check(
        "container-contents",
        items::container_contents(&boxed)
            .first()
            .is_some_and(|s| s.item == "minecraft:apple" && s.count == 3),
    );

    // Dimensions.
    let all = dimensions::list();
    check(
        "list",
        [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
        ]
        .iter()
        .all(|d| all.iter().any(|a| a == d)),
    );
    check(
        "exists",
        dimensions::exists(dim) && !dimensions::exists("minecraft:no_such"),
    );
    check("loaded", dimensions::loaded(dim));
    check(
        "base",
        dimensions::base("minecraft:the_nether").as_deref() == Some("minecraft:the_nether"),
    );
    check(
        "min-y",
        dimensions::min_y("minecraft:overworld") == Some(-64),
    );
    check(
        "max-y",
        dimensions::max_y("minecraft:overworld") == Some(319),
    );
    check(
        "players-in",
        dimensions::players_in(dim).iter().any(|p| p.id == me),
    );

    Ok(if bad.is_empty() {
        format!("level 19: all {ok} answered right")
    } else {
        format!("level 19: {ok} right, wrong: {}", bad.join(", "))
    })
}
