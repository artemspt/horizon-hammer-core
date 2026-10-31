//! Mod API levels: which of the WIT contract's functions a Horizon has
//! (`docs/api/stability.md`). Every function the game offers mods has the
//! level it arrived at; a Horizon serves every level up to its [`LEVEL`].
//! A mod needs the level of the newest function it imports, so a mod that
//! calls a function this Horizon doesn't have is refused with "update
//! Horizon" instead of a linking error, and the registry lists each
//! version's level so the launcher can tell before it installs.

/// The newest level this Horizon serves.
pub const LEVEL: u32 = 19;

/// Every function the game offers mods, as `interface.function`, with its
/// level. Level 0 is the contract of Horizon 0.2.67 to 0.2.69. A new
/// function takes the next level and raises [`LEVEL`] (the `api_reference`
/// test fails until it is listed here).
pub const FUNCTIONS: [(&str, u32); 243] = [
    ("log.write", 0),
    ("game.get-player", 0),
    ("game.get-clock", 0),
    ("chat.show", 0),
    ("hud.set-panel", 0),
    ("hud.clear", 0),
    ("storage.get", 0),
    ("storage.set", 0),
    ("storage.delete", 0),
    ("command.run", 0),
    ("ui.open-screen", 0),
    ("ui.close-screen", 0),
    ("ui.set-settings", 0),
    ("input.claim-items", 0),
    ("input.register-key", 0),
    ("input.held", 0),
    ("input.poll", 0),
    ("input.claim-wheel", 0),
    ("input.mapping", 0),
    ("input.clicks-per-second", 0),
    ("render.set-hud", 0),
    ("render.gui-size", 0),
    ("render.shake", 0),
    ("render.set-corner", 0),
    ("render.clear-corner", 0),
    ("render.text-width", 0),
    ("render.frame-stats", 0),
    ("render.set-zoom", 0),
    ("render.set-minimap", 1),
    ("players.online", 1),
    ("players.tell", 1),
    ("players.broadcast", 1),
    ("game.raycast", 2),
    ("command.run-quiet", 3),
    ("world.get-block", 4),
    ("world.get-biome", 4),
    ("world.set-blocks", 4),
    ("recipes.items", 5),
    ("recipes.all", 5),
    ("recipes.known", 5),
    ("browser.set-items", 6),
    ("browser.poll", 6),
    ("browser.set-recipes", 6),
    ("recipes.place", 6),
    ("recipes.give", 6),
    ("render.set-far-terrain", 7),
    ("ui.replace-screen", 8),
    ("world.get-data", 9),
    ("world.set-data", 9),
    ("world.data-positions", 9),
    ("players.inventory", 10),
    ("players.selected-slot", 10),
    ("players.give", 10),
    ("players.set-slot", 10),
    ("players.take", 10),
    ("world.schedule-tick", 10),
    ("world.redstone-power", 10),
    ("storages.declare", 10),
    ("storages.remove", 10),
    ("storages.shape", 10),
    ("storages.items", 10),
    ("storages.set-item", 10),
    ("storages.insert-item", 10),
    ("storages.extract-item", 10),
    ("storages.energy", 10),
    ("storages.insert-energy", 10),
    ("storages.extract-energy", 10),
    ("storages.set-energy", 10),
    ("storages.fluids", 10),
    ("storages.insert-fluid", 10),
    ("storages.extract-fluid", 10),
    ("storages.set-fluid", 10),
    ("storages.open-menu", 10),
    ("entities.near", 10),
    ("entities.spawn", 10),
    ("entities.hurt", 10),
    ("entities.kill", 10),
    ("entities.teleport", 10),
    ("entities.effect", 10),
    ("entities.particle", 10),
    ("entities.sound", 10),
    ("client-net.send", 11),
    ("server-net.send", 11),
    ("server-net.broadcast", 11),
    ("worldgen.set-surface-rule", 12),
    ("world.set-redstone-output", 13),
    ("world.set-comparator-output", 13),
    ("storages.set-menu-data", 13),
    ("world.set-locked-results", 13),
    ("world.get-saved", 13),
    ("world.set-saved", 13),
    ("players.sneaking", 14),
    ("storages.open-menu-as", 14),
    ("worldgen.add-ore", 14),
    ("storage.config", 14),
    ("storage.read-resource", 15),
    ("players.get-data", 15),
    ("players.set-data", 15),
    ("players.poll-events", 15),
    ("world.mod-loaded", 15),
    ("world.mod-send", 15),
    ("world.mod-receive", 15),
    ("world.item-has-tag", 15),
    ("entities.drop-item", 15),
    ("entities.set-attribute", 15),
    ("players.style-stack", 16),
    ("entities.get-data", 16),
    ("entities.set-data", 16),
    ("entities.set-ai", 16),
    ("entities.set-name", 16),
    ("entities.set-velocity", 16),
    ("world.block-has-tag", 16),
    ("world.entity-has-tag", 16),
    ("storages.set-slot-rule", 17),
    ("entities.move-to", 17),
    ("entities.set-target", 17),
    ("entities.set-equipment", 17),
    ("players.set-undying", 17),
    ("entities.set-rotation", 17),
    ("render.set-models", 17),
    ("render.set-menu-layouts", 17),
    ("players.look", 17),
    ("render.open-canvas", 17),
    ("players.by-name", 18),
    ("players.by-id", 18),
    ("players.is-online", 18),
    ("players.count", 18),
    ("players.near", 18),
    ("players.held-item", 18),
    ("players.offhand-item", 18),
    ("players.armor", 18),
    ("players.count-item", 18),
    ("players.has-item", 18),
    ("players.free-slots", 18),
    ("players.find-item", 18),
    ("players.set-game-mode", 18),
    ("players.clear-inventory", 18),
    ("players.show-title", 18),
    ("players.action-bar", 18),
    ("players.set-title-times", 18),
    ("players.add-experience", 18),
    ("players.add-levels", 18),
    ("players.set-spawn", 18),
    ("players.kick", 18),
    ("players.play-sound", 18),
    ("players.stop-sounds", 18),
    ("players.grant-advancement", 18),
    ("players.revoke-advancement", 18),
    ("world.get-blocks", 18),
    ("world.is-loaded", 18),
    ("world.is-air", 18),
    ("world.block-id", 18),
    ("world.block-property", 18),
    ("world.set-block", 18),
    ("world.set-block-property", 18),
    ("world.break-block", 18),
    ("world.fill", 18),
    ("world.find-blocks", 18),
    ("world.count-blocks", 18),
    ("world.height-at", 18),
    ("world.game-time", 18),
    ("world.day-time", 18),
    ("world.set-time", 18),
    ("world.set-weather", 18),
    ("world.set-gamerule", 18),
    ("world.set-difficulty", 18),
    ("world.set-world-spawn", 18),
    ("world.lightning", 18),
    ("world.explode", 18),
    ("world.has-data", 18),
    ("world.saved-keys", 18),
    ("world.running-mods", 18),
    ("world.is-valid-state", 18),
    ("world.item-exists", 18),
    ("world.max-stack-size", 18),
    ("world.random-u32", 18),
    ("entities.get", 18),
    ("entities.exists", 18),
    ("entities.of-kind", 18),
    ("entities.nearest", 18),
    ("entities.count-near", 18),
    ("entities.clear-effects", 18),
    ("entities.remove-effect", 18),
    ("entities.heal", 18),
    ("entities.set-health", 18),
    ("entities.swing", 18),
    ("storages.has-storage", 18),
    ("storages.is-empty", 18),
    ("storages.is-full", 18),
    ("storages.free-slots", 18),
    ("storages.count-item", 18),
    ("storages.clear", 18),
    ("storages.positions", 18),
    ("storages.free-energy", 18),
    ("storages.fluid-amount", 18),
    ("storages.transfer-item", 18),
    ("storages.transfer-energy", 18),
    ("storages.transfer-fluid", 18),
    ("items.all", 19),
    ("items.exists-in-tag", 19),
    ("items.name", 19),
    ("items.max-damage", 19),
    ("items.is-damageable", 19),
    ("items.is-food", 19),
    ("items.food-values", 19),
    ("items.is-block-item", 19),
    ("items.block-of", 19),
    ("items.fuel-ticks", 19),
    ("items.rarity", 19),
    ("items.components", 19),
    ("items.get-component", 19),
    ("items.set-component", 19),
    ("items.remove-component", 19),
    ("items.get-damage", 19),
    ("items.set-damage", 19),
    ("items.repair", 19),
    ("items.get-custom-name", 19),
    ("items.get-lore", 19),
    ("items.same-item", 19),
    ("items.can-stack", 19),
    ("items.split", 19),
    ("items.grow", 19),
    ("items.enchantments", 19),
    ("items.enchantment-level", 19),
    ("items.set-enchantment", 19),
    ("items.remove-enchantment", 19),
    ("items.has-glint", 19),
    ("items.attribute-modifiers", 19),
    ("items.add-attribute-modifier", 19),
    ("items.tool-speed", 19),
    ("items.is-correct-tool", 19),
    ("items.dyed-color", 19),
    ("items.set-dyed-color", 19),
    ("items.potion-contents", 19),
    ("items.set-potion-contents", 19),
    ("items.container-contents", 19),
    ("dimensions.list", 19),
    ("dimensions.exists", 19),
    ("dimensions.loaded", 19),
    ("dimensions.base", 19),
    ("dimensions.min-y", 19),
    ("dimensions.max-y", 19),
    ("dimensions.players-in", 19),
];

/// What the game's program carries so the launcher can read its level
/// without running it: `horizon-mod-api-level:0001;` (four digits). A
/// program from before the mark serves level 0.
pub const MARK: [u8; 27] = {
    let mut mark = *b"horizon-mod-api-level:0000;";
    let mut level = LEVEL;
    let mut i = 25;
    while level > 0 {
        mark[i] = b'0' + (level % 10) as u8;
        level /= 10;
        i -= 1;
    }
    mark
};

/// The level a program's bytes carry in their [`MARK`], if any.
pub fn read_mark(program: &[u8]) -> Option<u32> {
    let prefix = &MARK[..22];
    program
        .windows(MARK.len())
        .filter(|w| w.starts_with(prefix) && w[26] == b';')
        .find_map(|w| std::str::from_utf8(&w[22..26]).ok()?.parse().ok())
}

/// A function's level; None for one this Horizon doesn't have.
pub fn level_of(function: &str) -> Option<u32> {
    FUNCTIONS
        .iter()
        .find(|(name, _)| *name == function)
        .map(|&(_, level)| level)
}

/// The level a mod needs, from the functions it imports (as [`imports`]
/// names them): its newest function's. A function this Horizon doesn't
/// know comes from a newer one, so it needs more than [`LEVEL`]; that
/// function is returned with it, for the message.
pub fn needed<'a>(imports: impl IntoIterator<Item = &'a str>) -> (u32, Option<&'a str>) {
    let mut needed = (0, None);
    for name in imports {
        match level_of(name) {
            Some(level) if level > needed.0 => needed = (level, Some(name)),
            Some(_) => {}
            None => return (LEVEL + 1, Some(name)),
        }
    }
    needed
}

/// Why this Horizon can't run a mod that needs `level`, or None when it can.
pub fn too_new(level: u32, function: Option<&str>) -> Option<String> {
    (level > LEVEL).then(|| {
        let uses = function.map_or(String::new(), |f| format!(" (it uses {f})"));
        format!("made for a newer Horizon{uses}: update Horizon to run it")
    })
}

/// The functions a mod's component imports from Horizon (`horizon:mods/*`),
/// as `interface.function`, in the component's order.
#[cfg(any(feature = "runtime", feature = "index"))]
pub fn imports(wasm: &[u8]) -> Result<Vec<String>, String> {
    use wasmparser::{
        Parser, Payload, Validator, WasmFeatures, component_types::ComponentEntityType,
    };
    let types = Validator::new_with_features(WasmFeatures::all())
        .validate_all(wasm)
        .map_err(|e| format!("not a component: {e}"))?;
    let types = types.as_ref();
    let mut out = Vec::new();
    for payload in Parser::new(0).parse_all(wasm) {
        let Payload::ComponentImportSection(section) = payload.map_err(|e| e.to_string())? else {
            continue;
        };
        for import in section {
            let name = import.map_err(|e| e.to_string())?.name.name;
            // `horizon:mods/render@0.1.0` → `render`.
            let Some(interface) = name.strip_prefix("horizon:mods/") else {
                continue;
            };
            let interface = interface.split('@').next().unwrap_or(interface);
            let Some(item) = types.component_item_for_import(name) else {
                continue;
            };
            if let ComponentEntityType::Instance(id) = item.ty {
                for (function, export) in &types[id].exports {
                    if matches!(export.ty, ComponentEntityType::Func(_)) {
                        out.push(format!("{interface}.{function}"));
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_are_listed_once_and_up_to_this_horizons() {
        for (i, (name, level)) in FUNCTIONS.iter().enumerate() {
            assert!(*level <= LEVEL, "{name}");
            assert!(
                FUNCTIONS[..i].iter().all(|(other, _)| other != name),
                "{name} twice"
            );
        }
        assert!(FUNCTIONS.iter().any(|&(_, level)| level == LEVEL));
    }

    #[test]
    fn the_mark_carries_the_level() {
        assert_eq!(read_mark(&MARK), Some(LEVEL));
        let mut program = b"\0\x7fELF horizon-mod-api-level: horizon-mod-api-level:".to_vec();
        program.extend_from_slice(&MARK);
        program.extend_from_slice(b"\0rest");
        assert_eq!(read_mark(&program), Some(LEVEL));
        assert_eq!(read_mark(b"horizon-mod-api-level:12;"), None);
        assert_eq!(read_mark(b"no mark"), None);
    }

    #[test]
    fn a_mod_needs_its_newest_function() {
        assert_eq!(needed(["log.write", "hud.set-panel"]), (0, None));
        assert_eq!(
            needed(["log.write", "render.set-minimap", "hud.clear"]),
            (1, Some("render.set-minimap"))
        );
        assert_eq!(
            needed(["render.set-minimap", "render.set-sky"]),
            (LEVEL + 1, Some("render.set-sky"))
        );
        assert_eq!(too_new(LEVEL, None), None);
        assert_eq!(
            too_new(LEVEL + 1, Some("render.set-sky")).as_deref(),
            Some("made for a newer Horizon (it uses render.set-sky): update Horizon to run it")
        );
    }
}
