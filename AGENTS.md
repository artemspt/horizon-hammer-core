# Horizon Hammer Core - Modding Core Development Log

> **Extending the mod API?** Read `crates/horizon-mods/AGENTS.md` first; the list is `docs/mod-api-plan.md`.

> **FORK NOTICE:** В этом форке мы работаем **ТОЛЬКО** над модовым ядром.  
> Зона работы: `crates/horizon-mods/`, `crates/horizon-mod-sdk/`, `crates/cargo-horizon-mod/`, `mods/test_core/`.  
> Всё остальное (рендер, мир, геймплей, сеть, лаунчер и т.п.) трогать не нужно, если это не требуется исключительно для интеграции модового ядра.

This file tracks the development of the NeoForge-like modding core for Horizon.

## Current Version
**Core Version:** `0.1.0-dev`  
**Last Updated:** 2026-10-30

## Project Context
Horizon is a Minecraft: Java Edition 26.3 rewrite in Rust (wgpu). We are building a NeoForge-like modding API on top of the existing WASM-based mod system.

## Current State
The project has a working WASM mod system in `crates/horizon-mods/` with:
- WASM sandbox using wasmtime component model
- Capability-based permissions
- Client/Server mod separation
- Content mods (assets/data)
- Basic hooks (tick, frame, chat, command)

## Changelog

### 2026-10-31 - Items and dimensions (API level 19)

#### Added
- **WIT `items`** (38, capability `world`): `all`, `exists-in-tag`, `name`, `max-damage`, `is-damageable`, `is-food`,
  `food-values`, `is-block-item`, `block-of`, `fuel-ticks`, `rarity`, `components`, `get-component`, `set-component`,
  `remove-component`, `get-damage`, `set-damage`, `repair`, `get-custom-name`, `get-lore`, `same-item`, `can-stack`,
  `split`, `grow`, `enchantments`, `enchantment-level`, `set-enchantment`, `remove-enchantment`, `has-glint`,
  `attribute-modifiers`, `add-attribute-modifier`, `tool-speed`, `is-correct-tool`, `dyed-color`, `set-dyed-color`,
  `potion-contents`, `set-potion-contents`, `container-contents`
- **WIT `dimensions`** (7, capability `world`): `list`, `exists`, `loaded`, `base`, `min-y`, `max-y`, `players-in`
- **Runtime** - `runtime/items.rs`: `ItemFacts`, `StackFacts`, `StackEdit`, `DimensionFacts` and `WorldView` reads for
  them; ids and SNBT checked before they reach the game
- **Game** - `mod_machines.rs` `Lent` holds the world's resources (`Arc<Resources>`) and reads and edits stacks with
  `horizon_items` (`encode_components`, `command_components`, `decode_components`); `server_mods.rs` lists the dimensions
- **`/showcase api items`** - every function of the level, checked against vanilla 26.3
- Not run in the game yet: no game resources on this machine. Built, and the runtime tested

### 2026-10-31 - Events asked before they happen (`before-events`)

#### Added
- **WIT `before-events`** (world `before-events-host`, exported by `horizon-server-block-mod`): `entity-spawn`,
  `entity-hurt`, `player-hurt`, `player-death`, `explosion-blocks`, `item-pickup`, `item-toss`, `mob-drop`, `xp-drop`,
  `container-open`; manifest names `before-<name>` in `manifest::EVENTS`
- **Runtime** - `runtime/before.rs`: asked by `priority`, a refusal stops, a changed value goes on; `Mods::wants_before`
- **Game** - `horizon_client::mod_gate`: `Mob::damage_with` and `PlayerMotion::hurt_caused_by` ask through it, and
  the totem's death protection; `game/mod_before.rs` `Game::mod_gated` hands the mods to it around the creature, effect,
  thunder, bolt, trident, rocket, falling block and TNT phases, a guest's tick and a guest's attack, and `mods_now` finds
  them meanwhile (`hears_events`, `lend_world`, `entity-dying` and `explosion` go through it). Spawns are asked where mobs
  join (`spawn_mob_for`, natural spawning, spawners, mob events, raids, iron golems, wardens); pickups, drops of the held
  stack and container windows of guests; a dead mob's loot and experience; an explosion's blocks
- **SDK** - `ServerMod::before_*`; **example** `core-tour` cancels and changes; **test** `before_events_change_and_refuse_as_they_happen`
- **`/showcase before on|off|check`** - a rule on each hook, and how often the game asked it
- Not run in the game yet: the machine it was written on had no game resources. Built, and the runtime tested

### 2026-10-31 - 75 smaller functions (API level 18)

#### Added
- **WIT `players`** (25): `by-name`, `by-id`, `is-online`, `count`, `near`, `held-item`, `offhand-item`, `armor`, `count-item`,
  `has-item`, `free-slots`, `find-item`, `set-game-mode`, `clear-inventory`, `show-title`, `action-bar`, `set-title-times`,
  `add-experience`, `add-levels`, `set-spawn`, `kick`, `play-sound`, `stop-sounds`, `grant-advancement`, `revoke-advancement`
- **WIT `world`** (28): `get-blocks`, `is-loaded`, `is-air`, `block-id`, `block-property`, `set-block`, `set-block-property`,
  `break-block`, `fill`, `find-blocks`, `count-blocks`, `height-at`, `game-time`, `day-time`, `set-time`, `set-weather`,
  `set-gamerule`, `set-difficulty`, `set-world-spawn`, `lightning`, `explode`, `has-data`, `saved-keys`, `running-mods`,
  `is-valid-state`, `item-exists`, `max-stack-size`, `random-u32`
- **WIT `entities`** (10): `get`, `exists`, `of-kind`, `nearest`, `count-near`, `clear-effects`, `remove-effect`, `heal`, `set-health`, `swing`
- **WIT `storages`** (12): `has-storage`, `is-empty`, `is-full`, `free-slots`, `count-item`, `clear`, `positions`, `free-energy`,
  `fluid-amount`, `transfer-item`, `transfer-energy`, `transfer-fluid`
- **`/showcase api set|check|act|boom|clear|kick`** calls them all and says which answered wrong

#### Fixed
- A server thread's mods had no clock: `Snapshot.clock` needed a player of the window's own

### 2026-10-31 - Block properties, world fluids, effects, dimensions, goals, entities by code

#### Added
- **`states` in `horizon/block/*.json`** (`mod_content.rs`: copies of each template state, `Rename.combos`)
- **`horizon/fluid/*.json`** - `FluidState.family` keeps `kind`'s behaviour and flows apart; `fluid::mod_fluids`,
  `BucketContent::ModFluid`, mesher sprites and tint
- **`horizon/effect/*.json`** - `potions::mod_effect`; players and mobs; `minecraft:mob_effect` ids after vanilla's
- **`horizon/dimension/*.json`** - `Dim::Mod`, `mod_dimensions.rs`: flat, noise, `CodeTerrain` (workers ask the game's thread)
- **WIT `logic-events`** - `generate-chunk`, `goal-can-use`, `goal-tick`, `goal-stop`, `entity-tick`, `effect-tick`, asked of the
  mod owning the name; `entity/*.json` `goals`, `ticks`, `projectile.through_blocks`; `/showcase dial|plasma|shock|boomerang|go`

### 2026-10-31 - First runs in the game; item components, mob control, slot rules (API level 17)

#### Added
- **Item `components`** in `data/<id>/horizon/item/*.json`: vanilla components over the template's
- **WIT `entities.move-to`, `set-target`, `set-equipment`**, **`storages.set-slot-rule`**, **`players.set-undying`**,
  **`entity-events.entity-dying`**; `use-entity`/`attack-entity` also for players and objects; `poll-events` kinds
  `tossed`, `used-item`, `broke-item`, `targeted`, `block-gone`
- Spawn and hurt events in every level; wires turn towards a block with a mod's redstone output

- **`players.set-undying`**, not seen in the game yet
- **Mods `mods/showcase` and `mods/showcase_hud`**: the core in one walk-through, `/showcase demo` builds a stand

#### Fixed (found by running `horizon-ctl` against the game)
- Output of block, item and entity hooks was lost when the hub was out of `self.lan` (`tell` never arrived)
- `NoAI` was only saved: the mob tick ignored it, so `set-ai` did nothing
- Mobs had no UUID until asked, and mods couldn't see fresh ones (`mod_name_mobs`)
- `horizon/entity/` was refused by the packer and not copied to the content root; `/summon` didn't know mod mobs
- `entities.*` on a player: selectors don't take a player's UUID, and commands act on the player in place
- Mod projectiles flew with the world lent, so every block read as air; `spawn` dropped their `Motion`
- `players.look`, `sneaking`, `inventory` were empty in a guest's commands: the hub was out of `self.lan`
- `storages.set-slot-rule` now refuses at the click (`Menu.mod_rules`), not a moment later
- Clearing far terrain joined its worker on the main thread (`clear_lod`), a probable freeze

### 2026-10-31 - One mod with two halves, a mob's own look, display mobs, block shapes (API level 17)

#### Added
- **`side = "both"` on api 0.3** - `Manifest::two_halves`, `client.wasm` beside `mod.wasm`, SDK `export_both!`,
  `cargo horizon-mod` compiles the crate twice with `--cfg horizon_side`
- **`data/<id>/horizon/entity/*.json`** `texture`, `model` (`mobs.rs` `bake_mod_model`), `display`; clients hear the
  real type on `horizon_mods:mob_type` (`lan/entity.rs`, `game/remote.rs`)
- **WIT `entities.set-rotation`, `players.look`, `render.set-models`, `render.set-menu-layouts`, `render.open-canvas`,
  `entity-events.projectile-hit`**; block `properties.shape`, `shape_from` (`mod_content.rs`); mob type `sounds`,
  `projectile`; `data/<id>/horizon/model/*.json`; mods' `entity/` textures in the atlas (`hud.rs`)
- **Creative tabs** for mods, with pages (`creative.rs` `add_mod_tabs`, `turn_page`); `mods/build-all.sh`
- **Example** `mods/showcase`: one mod, a guard with its own model, a core turning over the reactor,
  `/showcase_panel`, a pedestal

### 2026-10-31 - Mob types, clicks on mobs, a stack's look, every level (API level 16)

#### Added
- **`data/<id>/horizon/entity/*.json`** - a mod's mob type from a vanilla template (`mobs.rs` `load_mod_mobs`,
  `mod_mob_base`: clients are told the template)
- **WIT `entity-events`** (`use-entity`, `attack-entity`, asked as they happen; `world entity-events-host`)
- **WIT `players.style-stack`** and lore, item model and glint in `lan/guest.rs` `network_stack`
- **WIT `entities.get-data`, `set-data`, `set-ai`, `set-name`, `set-velocity`**, **`world.block-has-tag`,
  `entity-has-tag`**
- **`game/mod_machines.rs` `tick_parked_mod_levels`** - levels not in place take a turn when a mod needs them:
  redstone both ways, mobs for `entities.near`, waiting drops and spawns. `spill_gone_storages` drops what a storage
  held when its block goes

### 2026-10-31 - What the NeoForge docs have and we didn't (API level 15)

Compared with docs.neoforged.net: attachments, reload listeners, `ModList`/IMC, tags, player events, attributes.

#### Added
- **WIT `players.get-data`, `set-data`, `poll-events`**, **`world.mod-loaded`, `mod-send`, `mod-receive`,
  `item-has-tag`**, **`storage.read-resource`**, **`entities.drop-item`, `set-attribute`**
- **Example** `core-tour`; **test** `core_tour_uses_what_neoforge_mods_lean_on`

### 2026-10-31 - Windows by name, sneaking, ores, settings (API level 14), and fixes

#### Added
- **WIT `storages.open-menu-as`, `players.sneaking`, `worldgen.add-ore`, `storage.config`**; manifest `[config]`
  (`manifest::load_config`), `vanillagen.rs` `place_mod_ores`
- **SDK** - `server::connected`, `server::move_energy`
- **Example** `worldgen` reads its surface block from its settings and adds an emerald ore
- **`docs/mod-core-known-issues.md`** - what was found and left for later

#### Fixed
- A timer whose chunk wasn't loaded when due was lost, and the machine stopped for good: it now waits
  (`a_timer_waits_for_its_chunk`)
- `world.set-locked-results` was saved with the world and outlived its mod: recipes stayed closed with the mod removed
- A redstone output kept signalling after its mod stopped: outputs have an owner
- Removed the `ModContainer::has_capability` stub, which said yes to everything

### 2026-10-30 - Redstone out, furnace windows, closed recipes, First Legacy (API level 13)

#### Added
- **WIT `world.set-redstone-output`, `set-comparator-output`** - a mod's block gives a weak signal and a comparator
  reading: `BlockData.outputs` (saved, gone with the block), `Ticks.mod_outputs` read by `redstone.rs`,
  `game/mod_machines.rs` `sync_mod_redstone` tells neighbours what changed. Level ticked last only
- **WIT `storages.set-menu-data`** - a storage of 3 slots opens as a furnace window with its flame and arrow
  (`Menu.mod_data`, `lan/containers.rs` `sync_container_data`)
- **WIT `world.get-saved`, `set-saved`** - a mod's bytes saved with the world, not per block (NeoForge's `SavedData`)
- **WIT `world.set-locked-results`** - items no crafting grid makes (`crafting.rs` `set_locked_results`)
- **Manifest** - a server mod may list `registries` and carry `assets/` and `data/`
- **Port `mods/first_legacy`** of First Legacy from `artemspt/halloween-mod`; its limits are at the top of its `lib.rs`
- **Example** `iron-press` signals when charged, a comparator reads its charge
- **Tests** - `sandbox.rs` `first_legacy_opens_knowledge_with_a_note`, `redstone_output_lives_on_its_block`
- **Plan** - `docs/mod-core-plan.md`: what Mekanism/AE2/Twilight Forest analogues and the three ports still need

### 2026-10-30 - World generation surface rules (`worldgen`, API level 12)

#### Added
- **WIT `worldgen.set-surface-rule(biome, surface)`** and the capability `worldgen` (server mods only,
  experimental: the manifest needs `experimental = ["worldgen"]`); the interface is imported by the
  `horizon-server-mod` and `horizon-server-block-mod` worlds. One surface block per biome: the game puts it at
  the top of that biome's terrain in the chunks it decorates while the rule is set; the last rule for a biome wins
- **Runtime** - `output::ServerAction::Worldgen { id, biome, surface }` (the mod's id, so a rule belongs to
  the mod that set it), `worldgen::Host` in `runtime.rs`, `Mods::worldgen_mut`;
  `WorldgenModifierManager::set_surface_rule` (replaces that mod's rule for the biome) and `remove_mod`;
  `Mods::restart` drops the rules of the version it replaces, so a reload sets them again instead of adding up
- **Game** - `game/mod_machines.rs` puts the rule in the manager; `mod_worldgen.rs` publishes an immutable
  snapshot for the terrain worker; `vanillagen.rs` sets the surface block when it decorates a chunk;
  `levels.rs` publishes again when a level opens
- **SDK** - `server::worldgen`; **example** `crates/horizon-mod-sdk/examples/worldgen` (plains surfaced with
  diamond blocks), built by the sandbox tests
- **Tests** - `sandbox.rs` `worldgen_rule_reaches_the_game` (the rule `init` sets reaches the game) and the
  newest-level check moved to the worldgen example; `worldgen_mods::tests::worldgen_surface_rules_replace_per_mod`;
  `horizon-client` `mod_worldgen::tests::publish_carries_the_rules_to_the_worker`
- **Docs** - `docs/mods.md` "World generation", the server interfaces table and the experimental list;
  `docs/api/stability.md`; `docs/api/mods-reference.md` regenerated

#### Fixed
- `crates/horizon-client/src/game/mod_machines.rs` compiles with the `mods` feature again (`worldgen_mut`,
  `horizon_client::mod_worldgen`), which the release builds and CI use
- `attributes::tests::attribute_compute_works` checks the modifier order instead of unused locals

### 2025-10-28 - Network for mods (capability `net`, API level 11)

#### Added
- **WIT `client-net.send`**, **`client-net-events.received`** (client mods), **`server-net.send`/`broadcast`**,
  **`server-net-events.received`** (server mods); worlds `client-net-host`, `server-net-host`; capability `net` for both
  sides (`of_import`: `client-net` and `server-net`)
- **Runtime** - `runtime/net.rs`: `NET_PREFIX` (`horizon_net:`), 32 KiB a message, 64 a call; `Output::NetToServer`,
  `ServerAction::Net`; `Mods::client_net_received`, `server_net_received`
- **Game** - the window sends serverbound custom payloads (`mods.rs`), hears clientbound ones (`remote.rs`); the server
  hears them (`lan_actions.rs` → `server_mods.rs` `guest_net_received`) and sends (`mod_machines.rs` `mod_net_send`);
  in-frame straight between the window's mods
- **SDK** - `Mod::net_received`, `ServerMod::net_received`, `client_net`, `server::server_net`
- **Example** `iron-press` answers `iron_press/ping`

### 2025-10-28 - Events with priority and cancel (`game-events`)

#### Added
- **WIT `game-events`** (a server mod's export, world `events-host`): `entity-spawned`, `entity-hurt` (cancel: removed /
  health back), `entity-died` (killer), `player-hurt`, `item-picked-up`, `explosion` (cancel: asked as it goes off)
- **Manifest** `events = [...]` (checked against `manifest::EVENTS`, server mods only) and `priority`: asked highest
  first, the first cancel ends the asking
- **Runtime** - `runtime/events.rs`; `Mods::wants_event` so the game finds nothing nobody hears. An interface imported
  for its types only needs no capability (load check and `component_imports`)
- **Game** - `mod_machines.rs` `mod_game_events` (each level's mobs' health and the players' compared at the tick's end),
  `mod_mob_died` (`creatures.rs` `mob_died`), `mod_picked_up` (`lan_actions.rs` guest pickups), `mod_explosion`
  (`game.rs` `explode`)
- **SDK** - `ServerMod::entity_spawned`, `entity_hurt`, `entity_died`, `player_hurt`, `item_picked_up`, `explosion`
- **Example** `iron-press`: explosions near a press fizzle, a killer hears of the kill

### 2025-10-28 - Machines: items, storages, windows, timers, redstone, entities (API level 10)

#### Added
- **WIT `players`**: `item-stack` (item, count, the mod's own `data`, opaque `components`); `inventory`, `selected-slot`,
  `give`, `set-slot`, `take` (made at the tick's end, in the player's own inventory: `as_guest`)
- **WIT `storages`** (capability `world`): `declare`/`remove`/`shape`; items (`items`, `set-item`, `insert-item`,
  `extract-item`), energy (FE), fluids (mB), each insert/extract with `simulate`, usable by every mod; vanilla
  containers answer the item calls; `open-menu`: a chest's window on a storage (placeholder `BlockEntity::Chest` under a
  key no block has, copied both ways each tick)
- **WIT `entities`** (new capability `entities`): `near` (mobs of the level ticked last), `spawn`, `hurt`, `kill`,
  `teleport`, `effect`, `particle`, `sound` as the server console's commands
- **WIT `world`**: `schedule-tick` (saved with the world), `redstone-power` (for mod blocks)
- **WIT `block-events`**: `place-block` (refuse), `use-item` (take a right click in the air), `block-tick`,
  `neighbor-changed` (six neighbours of every mod block compared each tick)
- **Runtime** - `runtime/server_api.rs`: `HostStack`, `Storage`, `ScheduledTick`, `ServerAction`, `EntityInfo`;
  `BlockData` keeps storages, timers and the clock; `WorldView` gains `redstone`, `max_stack`, `container`,
  `set_container`, `mobs` (defaults: none); `Output::Server`; `Mods::place_block`, `use_item`, `machine_positions`,
  `storage_slots`, `set_storage_slots`
- **Game** - `game/mod_machines.rs`: stacks both ways (mods' bytes under `minecraft:custom_data.horizon_mods`), inventories
  in the snapshot, the lent extras (redstone, stack sizes, block entities of every level, mobs), actions, windows;
  `game/server_mods.rs` `mods_refuse_place`, `mods_take_item_use`; `game/lan_actions.rs` asks them; `levels.rs`
  `parked_world(s)`
- **SDK** - `server::{ItemStack, StorageShape, FluidStack, Entity, storages, entities}`, `ServerMod::place_block`,
  `use_item`, `block_tick`, `neighbor_changed`
- **Example** `iron-press`; **test** `iron_press_runs_a_machine`; `docs/mods.md` "Machines"; API reference regenerated

### 2025-10-28 - Fixes: the inventory key and `/hmod` in default single player

#### Fixed
- **`ui.replace-screen`** now works in default single player (server thread), LAN and on servers: the inventory key
  there goes through `game/remote.rs` `remote_key`, which now asks `open_replaced_screen` first (before, only the
  `--in-frame` path in `game.rs` did)
- **`/hmod`** is in the chat's command suggestions once any mod is installed (`game/mods.rs` `hmod_name`): it no longer
  shows red as an unknown command
- `dependency.rs` - unused import

### 2025-10-28 - Data on blocks (`world.get-data` / `set-data` / `data-positions`, API level 9)

#### Added
- **WIT `world.get-data(dim, x, y, z)`, `world.set-data(dim, x, y, z, bytes)`, `world.data-positions(dim)`** (capability
  `world`, server mods) - a mod's own bytes (up to 4096) on a block position: each mod sees only its own; saved with the
  world; gone once the block is broken or replaced (the block id is checked on read, properties may change: a furnace
  lit or not). The base for block entities / machines: `data-positions` + `tick` = a loop over the mod's machines
- **`runtime.rs`** - `BlockData` (mod id → `"dim x y z"` → block id + hex bytes), shared by every mod's store;
  `Mods::block_data_json`, `Mods::set_block_data_json`
- **`api_level.rs`** - the three functions at level 9, `LEVEL = 9`
- **Game** - `game/server_mods.rs` `load_mod_block_data` (world opens, `game.rs` beside `world_root`), `save_mod_block_data`
  (`save_game`, only the game that runs server mods): `<world>/horizon-mods/block-data.json`
- **Example** `block-guard` counts how often a block was read with a stick, kept on the block
- **Tests** - `block_data_lives_and_dies_with_the_block` (count, property change, save and load, replaced block)

### 2025-10-28 - Hot reload in the game (`/hmod`)

#### Added
- **`/hmod reload [id]`** - restarts the mods that are on (all, or one) from their files, in the game, without leaving
  the world: between two calls into them, so nothing needs to freeze or save (mods are sandboxed; the world is the
  game's). A reloaded mod starts from `init` again; what it keeps across restarts belongs in `storage`
- **`/hmod list`** - the mods and their status
- The window runs it for its mods; with a server thread / LAN / dedicated server the command goes on to the server for
  the server mods (owner of an integrated server or an operator). In-frame the window does both
- **`crates/horizon-mods/src/hmod_command.rs`** - parsing; **`runtime.rs`** - `Mods::reload`, `restart` (shared with
  the dev mods' automatic reload on rebuild, `reload_changed`)
- **Game** - `game/mods.rs` `hmod_command`; `game/server_mods.rs` `guest_hmod_command`
- **Tests** - `hmod_reload_restarts_mods`, `hmod_command::tests`

Dev mods (`--dev-mod`) were already reloaded automatically about once a second after a rebuild; `/hmod reload` does it
on demand, for installed mods too.

### 2025-10-28 - Block hooks (`block-events`)

#### Added
- **WIT `interface block-events`** (exported by server mods, asked before the server acts on a block)
  - `break-block(player, x, y, z, state) -> bool` - true refuses breaking: the block stays, the player's game puts it back
  - `use-block(player, x, y, z, state, item) -> bool` - true: the mod handled the right click; vanilla neither uses the
    block nor uses or places the held item. First mod that takes it wins
  - Asked only of mods whose registration has `blocks` (`Registration::on_blocks()`)
- **Worlds**: `block-host` (the host's optional binding, like `ui-host`) and `horizon-server-block-mod` (what the SDK builds
  server mods as). Server mods built before it have no export and are simply not asked - nothing breaks
- **`runtime.rs`** - `block_bindings`, `Running.blocks`; `Mods::break_block`, `Mods::use_block`
- **SDK** - `ServerMod::break_block`, `ServerMod::use_block` (default false), wired in `export_server_mod!`
- **Game integration** (`horizon-client`): `game/server_mods.rs` `mods_refuse_break`, `mods_take_use` (in the guest's level,
  world lent for the call); `game/lan_actions.rs` asks them next to spawn protection (dig start; use item on block).
  Covers the server thread (default single player), LAN and dedicated servers; not the old `--in-frame` path
- **Example** `crates/horizon-mod-sdk/examples/block-guard`: diamond blocks can't be broken, a stick reads a block instead of using it
- **Tests** - `sandbox.rs` `block_events_refuse_and_take`; `docs/api/mods-reference.md` regenerated

### 2025-10-28 - Screen replacement (`ui.replace-screen`, API level 8)

#### Added
- **WIT `ui.replace-screen(vanilla, replacement)`** - a mod shows its own screen *in place of* a vanilla one, not over it
  - `minecraft:inventory` (the inventory key, survival and creative); `none` gives the vanilla screen back
  - Err for a screen this Horizon can't replace (`REPLACEABLE_SCREENS` in `runtime.rs`)
  - While shown it is the mod's ordinary screen: `open-screen` updates it, Esc/Done is `ui-events::closed`
- **`runtime.rs`** - `UiState.replacements`; `Mods::replacement_owner`, `Mods::show_replacement`
- **`api_level.rs`** - `("ui.replace-screen", 8)`, `LEVEL = 8`
- **Game integration** (`horizon-client`): `game/mods.rs` `open_replaced_screen`; the inventory key in `game.rs` asks it first
- **Tests** - `sandbox.rs` `ui_replaces_the_inventory` (the UI guest's `/ui_replace on|off|bad`); `docs/api/mods-reference.md` regenerated

#### Test Mod
- **`mods/test_core/`** - replaces the inventory with its "Test Core" screen; its button gives the inventory back
  (capabilities `ui`, `experimental = ["ui"]`); `horizon-mods/test_core.hmod` rebuilt with `cargo horizon-mod build`

### 2025-10-28 - NeoForge-like Core Infrastructure

#### Added
- **`crates/horizon-mods/src/container.rs`** - `ModContainer` structure
  - Centralized mod representation (id, name, version, side, capabilities, dependencies)
  - `ModState` enum (Discovered, Validated, Constructing, Setup, Loaded, Errored, Disabled)
  - `ModDependency` with ordering support
  - NeoForge-like API surface
  
- **`crates/horizon-mods/src/event_bus.rs`** - Synchronous event bus
  - Type-safe event posting/subscribing using `TypeId`
  - `Event` trait for all engine events
  - Built-in events: `TickEvent`, `ModLoadEvent`
  - Thread-safe with RwLock
  
- **`crates/horizon-mods/src/context.rs`** - `ModLoadingContext`
  - Provides access to mod container and event bus during loading
  - NeoForge-style context pattern
  
- **`crates/horizon-mods/src/lifecycle.rs`** - Loading lifecycle
  - `LoadingStage` enum (Construct, CommonSetup, SidedSetup, LoadComplete, PostLoad)
  - `LifecycleManager` for stage progression
  
- **`crates/horizon-mods/src/dependency.rs`** - Dependency resolution
  - Topological sorting for load order
  - Circular dependency detection
  - Missing dependency validation
  
- **`crates/horizon-mods/src/dist.rs`** - Distribution side
  - `Dist` enum (Client, DedicatedServer)
  - Helper methods for side checking

#### Modified
- **`crates/horizon-mods/src/lib.rs`** - Exported all new modules and types
- **`crates/horizon-mods/src/runtime.rs`** 
  - Integrated `ModContainer` creation into mod loading
  - Added `EventBus` to `Mods` struct
  - Post `TickEvent` and `ModLoadEvent` automatically
  - Added `event_bus()`, `post_event()` methods to `Mods`

#### Test Mod
- **`mods/test_core/`** - Test mod for validating new infrastructure
  - Uses `chat` and `hud` capabilities
  - Tests tick hooks
  - Will be updated as new features are added

## Architecture Overview

```
crates/horizon-mods/src/
├── container.rs      # ModContainer, ModState, ModDependency
├── event_bus.rs      # EventBus, Event trait
├── context.rs        # ModLoadingContext
├── lifecycle.rs      # LoadingStage, LifecycleManager
├── dependency.rs     # DependencyResolver
├── dist.rs           # Dist enum
├── runtime.rs        # WASM runtime, Mods manager (integrated)
├── manifest.rs       # Manifest parsing
└── lib.rs            # Public API
```

## Roadmap

### Phase 1: Core Infrastructure (In Progress)
- [x] ModContainer
- [x] EventBus
- [x] ModLoadingContext
- [x] Lifecycle stages
- [x] Dependency resolution
- [x] Dist enum
- [ ] Integration with SDK (expose to mod authors)
- [ ] More engine events

### Phase 2: Registries (In progress)
- [~] Registry primitives: ResourceLocation, Registry<T>, DeferredRegister, RegistryManager, register/freeze events
- [ ] Registry system (blocks, items, entities) wired to horizon-world/horizon-items
- [ ] DeferredRegister integrated into loading stages / SDK
- [ ] Registry events driven by runtime
- [ ] Link to horizon-world/horizon-items

### Phase 3: Advanced Features
- [ ] Menu/Container replacement API
- [ ] Custom block entities
- [ ] Fluid system API
- [ ] Network channels for mods
- [ ] Capability expansion

## Testing
Run tests: `cargo test -p horizon-mods --features runtime,pack`

All tests passing as of last update.

## Notes
- We're building on top of existing WASM system, not replacing it
- Focus on adding NeoForge-like patterns while maintaining compatibility
- Use `mods/test_core/` to test new features
