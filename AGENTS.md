# Horizon Hammer Core - Modding Core Development Log

> **FORK NOTICE:** В этом форке мы работаем **ТОЛЬКО** над модовым ядром.  
> Зона работы: `crates/horizon-mods/`, `crates/horizon-mod-sdk/`, `crates/cargo-horizon-mod/`, `mods/test_core/`.  
> Всё остальное (рендер, мир, геймплей, сеть, лаунчер и т.п.) трогать не нужно, если это не требуется исключительно для интеграции модового ядра.

This file tracks the development of the NeoForge-like modding core for Horizon.

## Current Version
**Core Version:** `0.1.0-dev`  
**Last Updated:** 2025-10-28

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

### Phase 2: Registries (Next)
- [ ] Registry system (blocks, items, entities)
- [ ] DeferredRegister API
- [ ] Registry events
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
