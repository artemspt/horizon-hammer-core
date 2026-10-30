# Mod core parity plan (NeoForge -> Horizon mod kernel)
Status: in-progress (P0 lifecycle/registry wiring in `feature/screens-blocks`)
> Tracker note: statuses below are current implementation status after P0. They are not full NeoForge parity.

## 1. Mod loading / entrypoint (FML)
- Mod entrypoint lifecycle: ConstructMod, CommonSetup, ClientSetup, ServerSetup, post-init
- Dist-aware registration: Dist.CLIENT / Dist.DEDICATED_SERVER and client-only side loading
- Manifest validation: IDs, namespaces, api levels, capabilities, side
- Dependency declarations and version resolution from manifest
- Load ordering, cycle detection, graceful failure and status reporting
- Stability baseline: 0.1 frozen compatibility mod and stability CI

## 2. Registry systems (NeoForge-level)
- Typed registries for blocks/items/fluids/entities/block_entities/menu_types/particles/sounds/mob_effects/enchantments/dimensions/features/structures/loot_modifiers/recipes
- DeferredHolder entries and Holder/Vanilla lookup link
- NewRegistryEvent and custom registries
- RegisterEvent typed integration with loader stages
- Tag integration and naming conventions
- Registry validation: duplicate IDs, namespace ownership, missing references

## 3. Event systems
- Game bus events: player/entity/level/inventory/block/container/interaction/command/crafting/item use/tick lifecycle
- Client bus events: render/input/screen/key/HUD overlays
- EventBusSubscriber declarative registration helper
- IModBusEvent marker / automatic bus detection
- RegisterCapabilitiesEvent, NewRegistryEvent, ModConfigEvent full support
- Network sync for events where needed
- Priority/cancel already partly implemented

## 4. Capabilities / data attachments
- NeoForge capability provider model (fluid/item/energy/block handlers)
- Data attachments: DataAttachmentType for entities/items/blocks/players
- Capability provider registration and query helpers
- Attachment sync over network
- Provider ordering and priority
- CapabilityHooks equivalents built into core
- Custom caps registration event
- Sync lifecycle and null handling

## 5. Items & blocks data behavior
- Programmatic item properties (durability/food/attributes/repair/creative tab)
- Block entity registration and sync
- Block state property definitions and validators
- Use interactions beyond current use-block/use-item
- Inventory and slot API, shift-click/quick-move/drag
- Item cooldowns first-class API
- Creative tabs registration/sorting
- Item tooltips/model overrides API
- Tool/armor material registrations
- Durability/attributes integration hooks

## 6. World generation & dimensions
- Biome modifiers with add/remove features/spawns/weather/climate
- Full feature/ore registration pipeline
- Structure template pools and jigsaw modifier API
- Surface rule modifications
- Dimension creation/modification registry and presets
- Biome and rule tests
- NBT template loading hooks and structure modifications
- Chunk loading/regeneration accessibility
- World creation events and data pack hooks

## 7. Fluids
- Fluid registration with source/flowing states
- Bucket integration
- Fluid tanks/capabilities
- Fluid interactions/custom cauldrons
- Render properties: color/tint/overlay/texture
- Behavioral properties: temperature/viscosity/luminosity
- World generation interactions (oceans/lava lakes)
- Fluid events (currently interaction/bucket fill)

## 8. Menus / containers / UI
- MenuType registration and sync
- Slot definitions/validators and shift-click/quick move/drag
- Data slots/synced properties
- Container component/packet hooks
- Screen events open/close
- Container menu linking to block entity/player inventory
- HUD overlays full layering API
- UI screen events beyond current replacement

## 9. Networking
- Custom payload registration (handshake, decode/encode)
- Packet protocol validation/version compatibility
- Server/client play payloads
- Network entity/item spawning integration
- Data attachment sync
- Client/server capability sync
- Thread-safe main thread callback dispatch
- Header message bounds/quota (already partially)

## 10. Properties / recipes / loot
- Recipe serializers/conditions/composites
- Custom ingredient types
- Loot modifiers/chest loot/table modification
- Block drop/global loot adjustments
- Recipe manager reload hooks and data maps
- Advancements/criteria/triggers
- Recipe browser API already partly done

## 11. Data packs / resources
- Resource manager events before/after reload
- Datapack registry resync
- Custom data file loaders and validation
- Pack dependencies/version metadata
- Dynamic model registration and load hooks
- Texture atlasing / tint color maps
- Model loader events
- Shader-level hooks are mostly already present separately

## 12. Configs
- Typed ForgeConfigSpec equivalent (server/client/common)
- TOML load/save with defaults generation
- Automatic config screen
- Network sync server configs
- Runtime config reload
- Annotation/validation-driven constraints

## 13. Client-side capabilities & rendering
- Client setup bus integration
- Render type registration
- Block/item model loader events
- Particle providers/render registration
- Custom sound instance playback registration (currently IDs only)
- Overlay renderer events/HUD placement/order
- Text/font size and splash screen hook
- Color map/tint index
- Rendering layers/shader API for mods
- Optional custom virtual overlay helpers

## 14. Entities & AI
- Mob effect registration/application
- Particle registration
- Entity render registration compatibility hooks (concept only)
- AI goal/behavior registration (optional)
- Attribute modifiers API
- Interaction/hitbox adjustments
- Entity spawning via mod registry link (already entity commands)

## 15. Enchantments
- Enchantment definitions (max level/applicability/treasure/curse)
- Loot table modification / cost hooks
- Registry linking to recipe/crafting
- Holder lookups (typed)

## 16. Mod container / dependency management
- Real dependency version resolution including manifest dependency schema
- Load ordering and cycle failure handling
- Side distribution metadata (server-only/client-only)
- Feature compatibility checks and update Horizon messages
- Mod resource metadata and resource overlays (mods as resource packs)
- Status tracking and error reporting (partly done)

## 17. Registry-linked events
- RegisterEvent with holder lookup
- NewRegistryEvent
- Post-registration freeze hooks
- Enforce no mutation after freeze except special cases

## 18. Synchronization
- Capability/provider sync
- Data attachment sync
- Dynamic registry sync multiplayer
- Custom menu sync
- State sync primitive

## 19. Testing & compatibility
- Game tests for mods
- 0.1 baseline compatibility mod tested in CI
- API level marks and breaking change check
- Sandbox fuzz/test (wasm fuel/time budgets)

## 20. Compatibility boundary (non-applicable as WASM)
- Access transformers / mixins replaced by event hooks
- Java bytecode transformations not needed
- Native loader replaced by WASM sandbox
- Update checker optional external concern

Note: Sections 12-20 bullets above mirror the user's stated NeoForge parity list.
Checklist will be updated during implementation; no completion claims yet.

## P0 progress (implemented in the mod kernel)
- Lifecycle wiring: `Mods` now owns `LifecycleManager`, emits `ModConstructEvent`, `CommonSetupEvent`,
  `SidedSetupEvent`, `ModLoadCompleteEvent` / `ModLoadEvent` through `run_lifecycle` / `advance_lifecycle`.
- Lifecycle events are marked as `ModBusEvent`s.
- Registry wiring: `RegistryManager` uses a type-erased `RegistryAccess` so `freeze_all` can freeze
  registries of any registered value type; `run_registration` announces `NewRegistryEvent` /
  `RegistryFrozenEvent`.
- Capability provider lists are sorted by provider priority.
- Minor compile warnings removed (`loot.rs`, `mob_effects.rs`).
- `fluid_machines.rs` must be committed with this work (previously untracked).

## P1 progress (deepened core contracts)
- Worldgen: `WorldgenModifierManager` now models biome modifiers, feature add/remove,
  surface rules and level-load availability; feature resolution applies modifiers.
- Fluids: `FluidRegistryManager` registers fluid definitions, states, and bucket items;
  interaction resolver remains available through the same module.
- Menus/containers: `ContainerMenuResolver` now tracks synced data slots, slot values,
  drag amounts, quick-move checks and commit logic.
- Entities/AI: `AttributeManager` now applies NeoForge-style add/multiply-base/multiply-total
  ordering; `AiGoalManager` supports goal removal and membership checks.
- Recipes/loot: `RecipeResolver` now evaluates composite conditions (`and`/`or`/`alternate`)
  and keeps a remaining-value based loot drop selection intact.
- Synchronization: `AttachmentManager` now exposes synced keys and applies synced payloads.
- No game/engine files were changed; P1 breadth remains core-side until the corresponding
  wiring phase.

## P2 status and promotion notes
- Script mods: not started. Requires an interpreter-based mod entrypoint and deep sandboxing integration.
- Shader API: not started as a mod authoring API. A separate shader pack pipeline exists outside the mod WIT contract.
- Promotion of `ui`, `render`, `input`, `browser`, `recipes`, server mods, and content mods from experimental to preview needs owner approval and evidence (stable generated reference, guide section, examples, one release line without breaks). The core now has tests and docs, so these can be proposed as promotion candidates.
- No runtime feature was promoted automatically in this pass.

## Separate wiring stage progress
- Worldgen surface rules are wired end to end (`worldgen.set-surface-rule`, API level 12): the game puts a
  mod's rule in `WorldgenModifierManager`, publishes a snapshot for the terrain worker (`horizon-client`
  `mod_worldgen`) and the chunk pass applies it. A reload drops the rules of the version it replaces.
- `Mods::load` now creates and advances `LifecycleManager` through Construct, CommonSetup,
  SidedSetup, LoadComplete and PostLoad.
- It also runs the registration pass and exposes `run_registration()` plus the constructed
  manager instances.
- P1 managers are now runtime fields and available through getters/mutable accessors and
  wiring helper methods:
  `worldgen`, `fluid_registry`, `attributes`, `ai_goals`, `recipe_resolver`.
- Existing fields (`fluid_interactions`, `attachments`, etc.) remain accessible, and the
  runtime now emits mod lifecycle events at concrete stages.
