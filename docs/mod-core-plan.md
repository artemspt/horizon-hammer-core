# Mod core plan: from API level 12 to Mekanism/AE2/Twilight Forest scale

Goal: a mod author can comfortably write a Mekanism, AE2 or Twilight Forest analogue, and our three NeoForge mods
(`artemspt/halloween-mod`: First Legacy, Harbinger, Applied Fission) run on Horizon.

Status: `[ ]` todo, `[~]` partial, `[x]` done and tested. Update this file as work lands.

## 1. Machines (Mekanism, AE2 blocks)

- [x] Redstone output from mod blocks and comparator output (`world.set-redstone-output`, `set-comparator-output`)
- [x] Machine windows with progress: a 3-slot storage opens as a furnace window, `storages.set-menu-data`
- [~] One mod with content and server code (`api = "0.3"` plus `registries`). Client code in the same mod: not yet
- [x] Block states from code: `world.set-blocks` with new properties keeps the block's data (keyed by block id)
- [x] Network graphs: `server::connected`, `server::move_energy` in the SDK
- [x] More windows: `storages.open-menu-as`

- [x] Crafting closed by progress: `world.set-locked-results`

## 2. World (Twilight Forest)

- [x] Ores: `worldgen.add-ore`
- [ ] Features and structures from a mod's `data/`
- [ ] Custom biomes from `data/`
- [ ] Custom dimensions from `data/`, teleport between them

## 3. Rendering and assets

- [ ] Sounds and particles from a mod's `assets/`
- [ ] Assets delivered to clients on a server
- [ ] Custom entity types with models (Harbinger's monsters, Twilight Forest bosses)
- [ ] Block entity rendering (BER)

## 4. The NeoForge-style layer

Written and unit tested in `horizon-mods`, unreachable from a mod. Each is wired to WIT, SDK and game, or deleted.

- [~] Settings: manifest `[config]`, `storage.config`. Not synced to clients; `config.rs` stays unused
- [ ] Inter-mod messages (`InterModComms`)
- [ ] Custom recipe types (`recipes_resolver.rs`), loot modifiers (`loot.rs`)
- [ ] Registries: `DeferredRegister` from the SDK
- [x] `ModContainer::has_capability` removed

## 5. Ports

The ports live in `mods/`, written from each mod's behaviour and data. No NeoForge or AE2/Mekanism dependency.

- [x] First Legacy (`mods/first_legacy`): knowledge, notes, research station, first generator. Limits are listed at the top of its `lib.rs`. Not seen in the game yet
- [ ] Harbinger (30k lines): vertical slice first, Blood Moon and its monsters, then what the API allows
- [ ] Applied Fission (25k lines): vertical slice first, elements, RTG, chemical reactor. Maxim's mod: the port lives
  here, his folder in `halloween-mod` stays untouched

## Rules

- Each API addition: WIT, `api_level.rs`, runtime, game, SDK, an example or test, `docs/mods.md`, regenerated
  `docs/api/mods-reference.md`, changelog in `CLAUDE.md`.
- Check: `cargo test -p horizon-mods --features runtime,pack`, `cargo check -p horizon-client --features mods`.

## 6. From the NeoForge docs (docs.neoforged.net)

Compared page by page against the mod API.

- [x] Data attachments: `players.get-data`/`set-data`. On mobs: not yet
- [x] Reload listeners' input: `storage.read-resource`. Reloading with `/reload`: not yet
- [x] `ModList.isLoaded` and IMC: `world.mod-loaded`, `mod-send`, `mod-receive`
- [x] Tags: `world.item-has-tag`. Block and entity tags: not yet
- [x] Player events: `players.poll-events`
- [x] Attributes: `entities.set-attribute`. Mods' own attributes: not yet
- [x] Drops: `entities.drop-item`
- [ ] Global loot modifiers, custom loot functions
- [ ] Custom recipe types run by the game, conditions on data files
- [ ] Damage types, mob effects and enchantments of a mod's own
- [ ] Right click and attack on an entity (`EntityInteract`, cancellable)
- [ ] Custom entity types and renderers, block entity renderers, particles of a mod's own
- [ ] Item data components reaching clients (lore, model, food, tool): see known issues
- [ ] Command arguments for items, entities, block states
- [ ] Biome modifiers beyond surface and ores, biomes and dimensions from data
- [ ] Game tests a mod author runs: `cargo horizon-mod check` only runs a mod's hooks once

## 7. Checklist: entities, events, items, blocks and machines

### Entities
- [x] Spawn, find, hurt, kill, teleport, effects, attributes, particles, sounds
- [x] Mob types of a mod's own from a template, with its numbers
- [x] Data on a mob, name, AI off, motion, every loaded level
- [ ] Models and textures of a mod's own on players' clients (the protocol names vanilla types only)
- [ ] AI goals written by a mod (it can only turn the AI off and drive the mob each tick)
- [ ] Entities that aren't mobs: projectiles, vehicles, display entities of a mod's own
- [ ] Equipment of a mob from a mod (`/item replace` may cover it, untested)

### Events
- [x] Spawned, hurt, died, player hurt, picked up, explosion, with priority and cancel
- [x] Right click and attack on a mob, as they happen
- [x] Player died, respawned, changed dimension, crafted
- [ ] Spawn and hurt asked before they happen (found at the tick's end today)
- [ ] Clicks on players, on non-mob entities; an item thrown, used up; a mob's target chosen
- [ ] Events in a subscription bus instead of one trait (`EventBus` still unreachable)

### Items
- [x] Stack bytes, inventories, give, take, tags
- [x] Name, lore, model and shine on a stack, reaching clients
- [ ] Tools, armor, food and durability of a mod's item from data beyond its template
- [ ] Other vanilla components on a stack; tooltips computed by code

### Blocks and machines
- [x] Hooks, timers, neighbours, data, storages with items, energy, tanks
- [x] Redstone in and out and comparators, in every loaded level
- [x] Windows by name with furnace bars, states from code, tags, contents dropped when broken
- [ ] Wires turning towards a mod's block; slot rules in a furnace window
- [ ] Windows of a shape of the mod's own; block entity rendering
- [ ] Block shapes and collision beyond the template
- [ ] Break and place by explosions and pistons as hooks (neighbours see the change)

## 8. Remaining work, in order (saved 2026-10-31)

The open `[ ]` items of section 7 are the list. Reason each is open, and the order to do them:

1. Run the game with `first_legacy`, `iron-press`, `core-tour`: nothing since level 13 has been seen in it. Watch
   `tick_parked_mod_levels` (level swaps each tick), `spill_gone_storages`, mod mob types, stack looks.
2. Events: clicks on players, an item thrown or used up, spawn and hurt in every level, then before they happen.
3. Items: tools, armor, food and durability beyond the template; more components in `lan/guest.rs` `network_stack`.
4. Blocks and machines: wires turning towards a mod's block, furnace window slot rules, block shapes beyond the template,
   windows of a mod's own shape, block entity rendering.
5. Entities: AI goals from a mod, equipment, non-mob entities, then models and textures of a mod's own (protocol and
   client work: the protocol names vanilla types only).
6. The `EventBus` subscription for mods.

Not committed yet: everything from level 13 on sits in the working tree of `feature/screens-blocks`.

## 9. State after the first runs in the game (2026-10-31)

Seen working in the game: iron press (window, work, in a parked level, spill), First Legacy notes and block textures,
mod mob types, stack looks, item `components` (damage, durability, food), clicks and `entity-dying` on mobs, `tossed`,
`used-item`, `targeted`, `set-attribute`/`effect`/`hurt`/`teleport` on a player, `move-to`, wires turning to a mod's
block, a refused slot. Bugs this found and fixed: output of block/item/entity hooks lost, `NoAI` ignored, fresh mobs
without UUIDs invisible to mods, `horizon/entity/` refused and not copied, `/summon` unaware of mod mobs, player ids in
console commands.

Open:
- `entities.set-equipment` showed no helmet on a zombie: broken or the check is wrong (`data get ... equipment`).
- `set-target` on a mob, clicks on players and objects, `broke-item`, `block-gone`: written, not seen in the game.
- A locked recipe and the research station in the game; `worldgen.add-ore` on real terrain.
- Not done: custom block shapes, windows of a mod's own shape, block entity rendering, models and textures of mod
  mobs, non-mob entity types. Block shapes come from the template: several tables hold them. `players.set-undying` is new and unseen. `EventBus` for mods: skipped, manifest `events` already subscribe.

Showcase: `mods/showcase` and `mods/showcase_hud`. `/showcase demo`, then `/showcase help`. Checked only that the game
and world open with them and the commands answer; the owner tests the rest and records bugs.

## 10. Closed after section 9 (2026-10-31)

One mod with two halves; mob textures, models, own sounds; display and projectile mob types; `set-rotation`;
real block shapes (`shape`, `shape_from`); world models with moving parts (`render.set-models`); real slots in the
mod's layout (`render.set-menu-layouts`); canvas windows (`render.open-canvas`); `players.look`; creative tabs for
mods with paging. `mods/showcase` calls every function of the API.

## 11. Added on request (2026-10-31)

Own block state properties (`states`), fluids in the world (`horizon/fluid/`), effects with a type (`horizon/effect/`),
dimensions with flat, noise or code generators (`horizon/dimension/`), code goals (`goals`, `logic-events`) and entities
flown by code (`ticks`, `projectile.through_blocks`). `/showcase dial|plasma|shock|boomerang|go`.

## 12. Later

Not started, in the order they would help most:

1. Worldgen from data: a mod's own biomes, noise settings and density functions (`WORLDGEN_BUNDLES` refuses them), so a
   Twilight Forest analogue needs no code generator. Portals to a mod's dimension from data.
2. Fluids: a tag of their own for mob pathing and fishing, tank and fluid item interplay, own fog and overlay,
   `fluid_flow` treating a neighbouring other fluid as a wall when drawing the flow direction.
3. Entities: vehicles with controls of their own (today: a horse or pig template), several-part hitboxes, renderers beyond
   cube models.
4. Goals from data naming vanilla's (`melee_attack`, `stroll`), removing the template's, goal flags so a code goal holds
   movement only; goals are asked for the mob's level, without telling the mod the dimension.
5. Attributes and enchantments from code or data; recipe types of a mod's own; tooltips by code.
6. Events before they happen (spawn, hurt), a subscription bus (`EventBus`), piston and explosion block hooks.
7. Data generators; a cursor on `render.open-canvas`.

By design, for the sandbox: no Mixin, no core patching, no arbitrary shaders or code in the renderer.

## 13. Gateway: native extensions for sandboxed mods (idea, not started)

A sandboxed mod can only do what the API offers. A gateway removes that ceiling without giving up the sandbox for the rest:

- A **native extension** is a small native mod (`.hnmod`, upstream's mechanism) that changes the game freely and declares
  new functions for WebAssembly mods, such as `create:moving-structures`.
- A **WebAssembly mod** lists it (`requires = ["create_engine"]`) and calls those functions like any other API.
- Its uses: moving structures for a Create analogue, energy networks computed natively for a GregTech analogue, new API
  tried by authors before it joins the core.

Limits: an extension has full access to the player's computer, so the Mods page warns about it and the registry reviews it.
It needs the launcher to rebuild the game. Each call still crosses the boundary, so heavy loops live inside the extension.
Engine mods like Sodium stay plainly native.

Steps: one native mod offering one function to one WebAssembly mod, with `requires` checked at load; then the warning;
then a first real extension. Build on upstream's `native_hooks.rs`, and raise it with upstream (#3911): the core itself could
become a native mod with extensions, leaving almost no fork of the game.
