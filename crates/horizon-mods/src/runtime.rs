//! Loading and calling mods with wasmtime's component model.
use crate::{
    RegistryManager,
    capabilities::{AttachmentManager, CapabilityProviderRegistry},
    config::{ConfigManager, ModConfig},
    container::{ModContainer, ModDependency, ModState},
    definitions::{
        BlockStatePropertyDefinition, ContainerMenuDefinition, DimensionDefinition,
        EnchantmentDefinition, FluidDefinition, LootModifierDefinition, MobEffectDefinition,
        ParticleDefinition, SoundDefinition, StructureDefinition, SurfaceRuleDefinition,
        TagDefinition,
    },
    dependency::DependencyResolver,
    event_bus::{EventBus, ModConfigEvent, ModLoadEvent},
    manifest::{Manifest, Package, Side},
    payloads::PayloadRegistry,
};
use server_bindings::{
    HorizonServerMod,
    exports::horizon::mods::server_events::Registration as ServerRegistration,
    horizon::mods::{players, world as mod_world, worldgen},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap,
    component::{Component, HasSelf, Linker},
};

mod events;
mod net;
mod server_api;
pub use net::{NET_BYTES, NET_PREFIX};
pub use server_api::{
    EntityInfo, HostStack, INVENTORY_SLOTS, MobChange, ScheduledTick, ServerAction, SlotRule,
    StackLook, Storage, Tank,
};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "horizon-mod",
    });
}
/// The server mods' world: `log` and `storage` are the client world's.
mod server_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "horizon-server-mod",
        additional_derives: [PartialEq],
        with: {
            "horizon:mods/log": super::bindings::horizon::mods::log,
            "horizon:mods/storage": super::bindings::horizon::mods::storage,
        },
    });
}
/// `block-events` beyond `horizon-server-mod`: a second world, so server mods
/// built before it (no export) still instantiate and are simply not asked.
mod block_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "block-host",
        with: {
            "horizon:mods/players": super::server_bindings::horizon::mods::players,
        },
    });
}
use block_bindings::BlockHost;
/// `game-events` beyond `horizon-server-mod`, as `block-events` is.
mod events_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "events-host",
        with: {
            "horizon:mods/players": super::server_bindings::horizon::mods::players,
            "horizon:mods/entities": super::server_bindings::horizon::mods::entities,
        },
    });
}
use events_bindings::EventsHost;
/// `entity-events`, as `block-events` is.
mod entity_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "entity-events-host",
        with: {
            "horizon:mods/players": super::server_bindings::horizon::mods::players,
            "horizon:mods/entities": super::server_bindings::horizon::mods::entities,
        },
    });
}
use entity_bindings::EntityEventsHost;
/// `logic-events`, as `entity-events` is.
mod logic_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "logic-events-host",
        with: {
            "horizon:mods/players": super::server_bindings::horizon::mods::players,
            "horizon:mods/entities": super::server_bindings::horizon::mods::entities,
        },
    });
}
use logic_bindings::LogicEventsHost;
pub use logic_bindings::exports::horizon::mods::logic_events::ChunkFill;
/// Capability `net`'s exports, each a world of its own as `ui-events` is.
mod client_net_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "client-net-host",
    });
}
mod server_net_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "server-net-host",
        with: {
            "horizon:mods/players": super::server_bindings::horizon::mods::players,
        },
    });
}
/// Capability `ui`'s world beyond `horizon-mod` (docs/mods.md): a second
/// world, so mods built before it (no `ui-events` export) still instantiate
/// as `horizon-mod` and simply hear nothing of their widgets.
mod ui_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "ui-host",
    });
}
pub use bindings::horizon::mods::{
    game::{Clock, Hit, Player},
    hud::{Corner, Line},
};
use bindings::{
    HorizonMod,
    exports::horizon::mods::events::Registration,
    horizon::mods::{chat, command, game, hud, log as mod_log, storage},
};
pub use server_bindings::{
    exports::horizon::mods::server_events::{
        Argument, ArgumentKind, ArgumentValue, BlockPos, ServerCommand,
    },
    horizon::mods::players::{GameMode, Player as ServerPlayer},
    horizon::mods::world::BlockChange,
};
type ClientEvents = bindings::exports::horizon::mods::events::Guest;
type ServerEvents = server_bindings::exports::horizon::mods::server_events::Guest;
/// The widget model a `ui` screen or settings page shows (WIT `interface
/// ui`), for the game to draw with its menus' own widgets.
pub use ui_bindings::horizon::mods::ui::{
    ButtonWidget, ItemWidget, ListWidget, Screen as UiScreen, TextWidget, Widget as UiWidget,
};
use ui_bindings::{UiHost, horizon::mods::ui};
/// Capability `render`'s imports (docs/mods.md): the game draws the shapes.
mod render_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "render-host",
    });
}
use render_bindings::horizon::mods::input;
/// Capability `input`'s records (WIT `interface input`).
pub use render_bindings::horizon::mods::input::{Event as InputEvent, HeldItem, ItemUse};
/// What capability `render` asks the game to draw (WIT `interface render`).
pub use render_bindings::horizon::mods::render::{
    Corner as RenderCorner, FarTerrain as ModFarTerrain, FillShape, FrameRate, ItemShape,
    MenuLayout, Minimap as ModMinimap, Point as RenderPoint, QuadShape, Rect as RenderRect, Shape,
    SpriteShape, TextShape, Zoom as ModZoom,
};
use render_bindings::{RenderHost, horizon::mods::render};
/// Capability `recipes`'s imports (docs/mods.md): the game's recipes and items.
mod recipes_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "recipes-host",
    });
}
/// Capability `ui`'s recipe viewer panel (WIT `interface browser`).
pub use recipes_bindings::horizon::mods::browser::{
    Click as BrowserClick, Event as BrowserEvent, ItemKey as BrowserItemKey,
    Layout as BrowserLayout, RecipeView as BrowserRecipe, SlotClick as BrowserSlotClick,
    Stack as BrowserStack,
};
/// What capability `recipes` reads (WIT `interface recipes`).
pub use recipes_bindings::horizon::mods::recipes::{
    Item as RecipeItem, Kind as RecipeKind, Recipe, Stack as RecipeStack,
};
use recipes_bindings::{RecipesHost, horizon::mods::browser, horizon::mods::recipes};

/// A mod's linear memory cap.
const MEMORY: usize = 64 << 20;
/// Fuel (about one unit per WebAssembly instruction) per call: a bound on
/// runaway loops, far above the hooks' time budgets.
const FUEL_INIT: u64 = 2_000_000_000;
const FUEL_TICK: u64 = 50_000_000;
const FUEL_FRAME: u64 = 10_000_000;
const FUEL_EVENT: u64 = 100_000_000;
/// Blocks a mod may set per `world.set-blocks` call.
pub const BLOCKS_PER_CALL: usize = 4096;
/// Chat lines a mod may show per call, HUD lines and their length.
const CHAT_PER_CALL: usize = 8;
/// `command.run-quiet` lines one call of a mod may queue (a quiet payload's
/// cap, `horizon_protocol::quiet::MAX_LINES`).
const QUIET_PER_CALL: usize = 256;
/// How far `game.raycast` sees, in blocks.
pub const RAYCAST_MAX: f32 = 1024.0;
const HUD_LINES: usize = 16;
const LINE_CHARS: usize = 256;
/// Total bytes a mod's `storage` may hold on disk, across all its keys.
const STORAGE_QUOTA: u64 = 4 << 20;
/// Distinct keys a mod's `storage` may hold: `STORAGE_QUOTA` alone doesn't
/// bound a flood of zero- or near-zero-byte files (each still a file name,
/// a directory entry and an inode).
const STORAGE_MAX_KEYS: usize = 64;
/// Bytes `storage::set` may write across one hook call: fuel bounds a
/// mod's CPU time per call, not the time a synchronous disk write takes, so
/// a cheap-in-fuel loop replacing one key with large payloads needs its own
/// cap, separate from the on-disk quota it would otherwise stay under.
const STORAGE_WRITE_BUDGET: u64 = 1 << 20;

/// What mods may read of the game, copied once per tick by the game.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub player: Option<Player>,
    pub clock: Option<Clock>,
    /// The screen in GUI pixels (capability `render`'s `gui-size`).
    pub gui_size: (f32, f32),
    /// The main hand's stack (capability `input`'s `held`).
    pub held: Option<HeldItem>,
    /// The game's frame counter: fps, min, 1% low (`render.frame-stats`).
    pub frame: (u32, u32, u32),
    /// The movement and click mappings `input.mapping` answers for:
    /// (mapping, the bound key's label, held).
    pub mappings: Vec<(&'static str, String, bool)>,
    /// Attack and use clicks in the last second.
    pub cps: (u32, u32),
    /// The HUD font's advance per character from U+0000
    /// (`render.text-width`); empty: 6 each.
    pub font_widths: Arc<[f32]>,
    /// The players on the server, for server mods.
    pub players: Vec<ServerPlayer>,
    /// Each player's id, held hotbar slot and 41 inventory slots
    /// (`players.inventory`), for server mods.
    pub inventories: Vec<(String, u32, Vec<Option<HostStack>>)>,
    /// The ids of the players holding sneak.
    pub sneaking: Vec<String>,
    /// Where players look: id, yaw, pitch.
    pub looks: Vec<(String, f32, f32)>,
    /// This tick's `players.poll-events`: player, kind, detail.
    pub player_events: Vec<(String, String, String)>,
    /// The running mods' ids, for `world.mod-loaded`.
    pub running: Vec<String>,
    /// The first block along the player's view within [`RAYCAST_MAX`] and
    /// its distance from the eye, for `game.raycast`; the game works it out
    /// only while a mod calls it ([`Mods::wants_raycast`]).
    pub view_hit: Option<(f32, Hit)>,
    /// Every item and recipe, for capability `recipes`; the game makes them
    /// once, and only while a mod reads them ([`Mods::wants_recipes`]).
    pub recipes: Option<Arc<Recipes>>,
    /// The `book-id`s of the recipes the player's book holds.
    pub known_recipes: Arc<[i32]>,
    /// The open screen takes recipes (`recipes.place`).
    pub placing: bool,
    /// The player is in creative mode (`recipes.give`).
    pub creative: bool,
}

/// Capability `recipes`'s tables: `items` in the registry's order, which
/// each [`RecipeStack::item`] indexes.
#[derive(Clone, Debug, Default)]
pub struct Recipes {
    pub items: Vec<RecipeItem>,
    pub recipes: Vec<Recipe>,
}

/// Which mods a game starts: the window's game the client mods, the game
/// simulating the world the server mods, single player in the frame both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Runs {
    Client,
    Server,
    Both,
}
impl Runs {
    fn starts(self, m: &Manifest) -> bool {
        match self {
            Runs::Both => true,
            Runs::Client => m.side != Side::Server,
            Runs::Server => m.side == Side::Server || m.two_halves(),
        }
    }
}

/// What mods asked the game to show since the last [`Mods::drain`].
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// `chat.show`: a line after the mod's name.
    Chat { name: String, text: String },
    /// A mod command's feedback, red when `error`.
    Feedback { text: String, error: bool },
    /// `command.run`: a vanilla command line, without its slash, to run as
    /// the player.
    Command { line: String },
    /// `command.run-quiet`: vanilla command lines to run as the player with
    /// nothing in the chat (`horizon_protocol::quiet` on a server).
    QuietCommands { mod_id: String, lines: Vec<String> },
    /// The mod was turned off: it trapped, overran or failed to start.
    Stopped { name: String, reason: String },
    /// `players.tell`: a line after the mod's name, to the player with this id.
    Tell {
        name: String,
        player: String,
        text: String,
    },
    /// `players.broadcast`: a line after the mod's name, to every player.
    Broadcast { name: String, text: String },
    /// A server mod command's feedback, to the player who typed it.
    Reply {
        player: String,
        text: String,
        error: bool,
    },
    /// `world.set-blocks`: changes to make in `dimension`, in order, each
    /// state checked by the [`WorldView`] lent for the call.
    SetBlocks {
        name: String,
        dimension: String,
        blocks: Vec<BlockChange>,
    },
    /// `ui.open-screen`: the mod's entry in `entries()`; what it shows is
    /// [`Mods::screen`].
    OpenScreen { owner: usize },
    /// `ui.close-screen`: the mod closed its own screen (no `closed` back).
    CloseScreen { owner: usize },
    /// `render.shake`: strength 0 to 1, fading out over `seconds` (≤ 10).
    Shake { strength: f32, seconds: f32 },
    /// `recipes.place`: the recipe book's click on this display id, all of
    /// it when `all`.
    PlaceRecipe { book_id: i32, all: bool },
    /// `recipes.give`: a stack for a creative player.
    Give {
        item: String,
        count: u32,
        components: String,
    },
    /// A server mod's change to players, windows or entities, made by the
    /// game at the tick's end.
    Server(ServerAction),
    /// `client-net.send`: a message to the server's mods.
    NetToServer { channel: String, data: Vec<u8> },
}

/// The game's world as capability `world` reads it during a call: the
/// game lends one with [`Mods::lend_world`] and takes it back after.
pub trait WorldView: std::any::Any + Send {
    /// The block state at a position, `minecraft:oak_log[axis=y]`; `None`
    /// where no chunk of that dimension is loaded.
    fn block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String>;
    /// The biome at a position, `minecraft:plains`; `None` as for blocks.
    fn biome(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String>;
    /// Whether `dimension` is one the server has.
    fn has_dimension(&self, dimension: &str) -> bool;
    /// `Err` (why) for a state string that names no block state.
    fn check_state(&self, state: &str) -> Result<(), String>;
    /// The redstone power reaching a block (`world.redstone-power`): the
    /// game works it out for [`Mods::machine_positions`] before lending.
    fn redstone(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<u8> {
        None
    }
    /// Whether an item is in an item tag.
    fn item_has_tag(&self, _item: &str, _tag: &str) -> bool {
        false
    }
    /// Whether a block is in a block tag.
    fn block_has_tag(&self, _block: &str, _tag: &str) -> bool {
        false
    }
    /// Whether an entity type is in an entity type tag.
    fn entity_has_tag(&self, _kind: &str, _tag: &str) -> bool {
        false
    }
    /// A stack with this look; the game alone knows its encoding.
    fn style_stack(
        &self,
        _stack: &HostStack,
        _look: &server_api::StackLook,
    ) -> Result<HostStack, String> {
        Err("no game to style a stack with".into())
    }
    /// An item's stack size; none for an item the game does not know.
    fn max_stack(&self, _item: &str) -> Option<u32> {
        Some(64)
    }
    /// A vanilla container's slots at a position (chest, barrel, hopper...).
    fn container(
        &self,
        _dimension: &str,
        _x: i32,
        _y: i32,
        _z: i32,
    ) -> Option<Vec<Option<HostStack>>> {
        None
    }
    /// Stores a vanilla container's slots (as many as it has): false when
    /// none is there.
    fn set_container(
        &mut self,
        _dimension: &str,
        _x: i32,
        _y: i32,
        _z: i32,
        _slots: Vec<Option<HostStack>>,
    ) -> bool {
        false
    }
    /// The living mobs of a dimension (`entities.near`).
    fn mobs(&self, _dimension: &str) -> Vec<EntityInfo> {
        Vec::new()
    }
}

/// The view lent for the current calls, shared by every mod's store.
type WorldSlot = Arc<Mutex<Option<Box<dyn WorldView>>>>;

/// A mod's text panel, drawn every frame until it changes.
#[derive(Clone, Debug)]
pub struct Panel {
    pub corner: Corner,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Installed and turned off by the player.
    Off,
    Running,
    /// Turned on, and run by another game: a server mod in the window while
    /// the world runs on its server thread.
    Elsewhere,
    /// Could not load or start, or stopped while running.
    Failed(String),
}

/// One installed mod, for the Mods page.
pub struct Entry {
    pub path: PathBuf,
    /// None when the manifest could not be read (`status` says why).
    pub manifest: Option<Manifest>,
    /// NeoForge-style mod container
    pub container: Option<ModContainer>,
    pub status: Status,
    /// The mod's average time per call of its busiest hook.
    pub cost: Duration,
    run: Option<Running>,
    modified: Option<SystemTime>,
}
impl Entry {
    /// The running mod has a settings page (capability `ui` set widgets
    /// with `ui.set-settings`, or `input` registered keys the player may
    /// bind there): its row in Options > Mods gains a button.
    pub fn has_ui_settings(&self) -> bool {
        self.run.as_ref().is_some_and(|r| {
            let d = r.store.data();
            !d.ui.settings.is_empty() || !d.input.keys.is_empty()
        })
    }
}

struct Running {
    store: Store<State>,
    /// The mod's `ui-events` export; none for a mod built before `ui`, or a
    /// server mod.
    ui: Option<UiHost>,
    /// A server mod's `block-events` export; none for one built before it.
    blocks: Option<BlockHost>,
    /// A server mod's `game-events` export; none for one built before it.
    events: Option<EventsHost>,
    /// A server mod's `entity-events` export; none for one built before it.
    entity_events: Option<EntityEventsHost>,
    /// A server mod's `logic-events` export; none for one built before it.
    logic_events: Option<LogicEventsHost>,
    /// Capability `net`'s exports, on each side.
    net_client: Option<client_net_bindings::ClientNetHost>,
    net_server: Option<server_net_bindings::ServerNetHost>,
    hooks: Hooks,
    /// It imports `game.raycast`.
    raycasts: bool,
    /// It imports capability `recipes`.
    reads_recipes: bool,
}

/// A running mod's exports and the hooks its `init` asked for.
enum Hooks {
    Client(HorizonMod, Registration),
    Server(HorizonServerMod, ServerRegistration),
}
impl Hooks {
    fn client(&self) -> Option<&Registration> {
        match self {
            Hooks::Client(_, r) => Some(r),
            Hooks::Server(..) => None,
        }
    }
    fn server(&self) -> Option<&ServerRegistration> {
        match self {
            Hooks::Server(_, r) => Some(r),
            Hooks::Client(..) => None,
        }
    }
    fn commands(&self) -> Vec<&str> {
        match self {
            Hooks::Client(_, r) => r.commands.iter().map(String::as_str).collect(),
            Hooks::Server(_, r) => r.commands.iter().map(|c| c.name.as_str()).collect(),
        }
    }
}

/// What capability `ui` pushed since the hooks last ran.
#[derive(Default)]
struct UiState {
    /// The screen `ui.open-screen` last pushed, until `ui.close-screen`.
    screen: Option<UiScreen>,
    /// `ui.open-canvas`: what `screen` draws instead of widgets.
    canvas: Option<(Vec<Shape>, Vec<render::CanvasArea>)>,
    /// The widgets `ui.set-settings` last pushed (the mod's page in
    /// Options > Mods).
    settings: Vec<UiWidget>,
    /// The calls since the last hook ended, for [`Output`].
    open: bool,
    close: bool,
    /// `ui.replace-screen`: vanilla screen id → the mod's screen shown in
    /// its place.
    replacements: std::collections::BTreeMap<String, UiScreen>,
}

/// Capability `ui`'s panel beside container screens (`browser`).
#[derive(Default)]
struct BrowserState {
    items: Vec<BrowserStack>,
    /// Which `set-items` or `set-recipes` call the panel is from, unique
    /// across mods and reloads, so the game converts them once.
    version: u64,
    /// `set-recipes`' title and recipes, shown instead of the items.
    recipes: Option<RecipePage>,
    events: std::collections::VecDeque<BrowserEvent>,
}
/// What `browser.set-recipes` shows: a title, tabs (`tab` the selected
/// one) and the recipes.
#[derive(Clone, Debug)]
pub struct RecipePage {
    pub title: String,
    pub tabs: Vec<BrowserStack>,
    pub tab: u32,
    pub recipes: Vec<BrowserRecipe>,
}
static BROWSER_VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
/// Items a panel may hold, and events kept for `browser.poll`.
const BROWSER_ITEMS: usize = 8192;
/// `recipes.place` and `give` calls a hook call may make.
const RECIPE_ACTIONS: usize = 36;
/// Recipes `set-recipes` may show, slots in one, and stacks a slot cycles.
const BROWSER_RECIPES: usize = 256;
const RECIPE_SLOTS: usize = 9;
const RECIPE_TABS: usize = 16;
const SLOT_STACKS: usize = 64;
const BROWSER_EVENTS: usize = 256;
impl BrowserState {
    fn push(&mut self, event: BrowserEvent) {
        if self.events.len() == BROWSER_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

struct State {
    /// `storage.resource`: where the mod's package is.
    package: (PathBuf, Manifest),
    /// `storage.config`: the settings as read when the mod started.
    config: std::collections::BTreeMap<String, String>,
    name: String,
    id: String,
    snapshot: Arc<Snapshot>,
    out: Vec<Output>,
    panel: Option<Panel>,
    ui: UiState,
    /// Capability `render`'s HUD shapes, drawn until replaced.
    shapes: Vec<Shape>,
    /// `render.set-models`.
    models: Vec<render::PlacedModel>,
    /// `render.set-menu-layouts`.
    menu_layouts: Vec<render::MenuLayout>,
    /// Capability `render`'s corner panel.
    corner: Option<CornerPanel>,
    /// Capability `render`'s zoom, when the mod zooms.
    zoom: Option<ModZoom>,
    /// Capability `render`'s minimap, when the mod shows one.
    minimap: Option<ModMinimap>,
    /// Capability `render`'s far terrain, when the mod draws one.
    far_terrain: Option<ModFarTerrain>,
    input: InputState,
    browser: BrowserState,
    /// Capability `storage`'s folder; made on first `set`.
    data_dir: PathBuf,
    /// Bytes `storage::set` may still write before this hook call ends
    /// (reset per call in [`Mods::call`]): fuel bounds a mod's CPU time but
    /// not disk I/O time, so a loop of small-fuel, large-payload `set`s
    /// needs its own budget.
    storage_budget: u64,
    limits: StoreLimits,
    world: WorldSlot,
    /// Every server mod's bytes on blocks, shared (each reads its own).
    block_data: BlockDataSlot,
    /// Mob effect state for integration.
    mob_effects: crate::MobEffectManager,
}

/// Server mods' bytes on blocks (`world.get-data`, `world.set-data`): mod id
/// → `"dimension x y z"` → the block they were set on (its id, without
/// properties) and the bytes as hex. The game saves it with the world.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockData {
    pub mods:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, (String, String)>>,
    /// `storages.declare`: `"dimension x y z"` → the storage.
    #[serde(default)]
    pub storages: std::collections::BTreeMap<String, Storage>,
    /// `world.schedule-tick`s waiting.
    #[serde(default)]
    pub ticks: Vec<ScheduledTick>,
    /// `world.set-redstone-output`: `"dimension x y z"` → what the block gives.
    #[serde(default)]
    pub outputs: std::collections::BTreeMap<String, server_api::RedstoneOutput>,
    /// `world.set-locked-results`: mod id → items. Not saved: a removed mod's
    /// locks go with it, a running one's are set again.
    #[serde(skip)]
    pub locked: std::collections::BTreeMap<String, Vec<String>>,
    /// `players.set-data`: player → mod id → key → bytes in hex.
    #[serde(default)]
    pub player_data: std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    >,
    /// `world.mod-send`: target mod id → (sender, channel, data). Not saved.
    #[serde(skip)]
    pub inbox: std::collections::BTreeMap<String, Vec<(String, String, Vec<u8>)>>,
    /// `world.set-saved`: mod id → key → bytes in hex.
    #[serde(default)]
    pub saved: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// The server tick of the last [`Mods::server_tick`], which `ticks` count from.
    #[serde(default)]
    pub now: u64,
    /// `now` follows this server's clock (false right after loading).
    #[serde(skip)]
    synced: bool,
    #[serde(skip)]
    neighbors: server_api::NeighborCache,
}
type BlockDataSlot = Arc<Mutex<BlockData>>;
const BLOCK_DATA_BYTES: usize = 4096;
const BLOCK_DATA_POSITIONS: usize = 65536;

fn block_key(dimension: &str, x: i32, y: i32, z: i32) -> String {
    format!("{dimension} {x} {y} {z}")
}
/// `minecraft:furnace[lit=true]` → `minecraft:furnace`.
fn block_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn from_hex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// The installed mods and the engine running them. `Mods::default()` is
/// the state with none: no engine, and every hook a no-op.
#[derive(Default)]
pub struct Mods {
    /// This game runs a `side = "both"` mod's client half, not its server's.
    client_half: bool,
    engine: Option<(Engine, Linker<State>)>,
    entries: Vec<Entry>,
    snapshot: Arc<Snapshot>,
    out: Vec<Output>,
    world: WorldSlot,
    /// Server mods' bytes on blocks, saved with the world by the game.
    block_data: BlockDataSlot,
    /// Moves on whenever a mod starts or stops: the game resends the server
    /// mods' commands to players when it changes.
    generation: u64,
    /// NeoForge-style event bus
    event_bus: Arc<EventBus>,
    /// Mod (startup) event bus, separate from the game bus, NeoForge-style.
    mod_event_bus: Arc<EventBus>,
    /// Loading stage progression, NeoForge `FML` lifecycle.
    lifecycle: crate::LifecycleManager,
    /// Object registries, NeoForge-style `RegistryManager`.
    registries: RegistryManager,
    /// Config manager, ForgeConfigSpec-style core slice.
    configs: ConfigManager,
    /// Runtime mob effect tracker.
    mob_effects: crate::MobEffectManager,
    /// Capability providers.
    capability_providers: CapabilityProviderRegistry,
    /// Data attachments.
    attachments: AttachmentManager,
    /// Custom payload registry.
    payloads: PayloadRegistry,
    /// Resource overlays.
    overlays: crate::OverlayManager,
    /// Worldgen modifiers.
    worldgen: crate::WorldgenModifierManager,
    /// Fluid registrations/buckets.
    fluid_registry: crate::FluidRegistryManager,
    /// Fluid interactions.
    fluid_interactions: crate::FluidInteractionResolver,
    /// Attribute modifier manager.
    attributes: crate::AttributeManager,
    /// AI goal manager.
    ai_goals: crate::AiGoalManager,
    /// Recipe condition/composite resolver.
    recipe_resolver: crate::RecipeResolver,
    /// Block state property resolver.
    block_state_properties: crate::BlockStatePropertyResolver,
    /// InterMod compatibility messages.
    intermod: crate::InterModComms,
    /// Tag entry manager.
    tag_manager: crate::TagManager,
    /// Container menu sync resolvers keyed by menu type.
    menu_resolvers: std::collections::BTreeMap<String, crate::ContainerMenuResolver>,
}

impl Mods {
    /// Get the event bus
    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    /// Mod/startup event bus (registration, setup, capabilities).
    pub fn mod_event_bus(&self) -> &Arc<EventBus> {
        &self.mod_event_bus
    }

    /// Current lifecycle loading stage.
    pub fn lifecycle_stage(&self) -> crate::LoadingStage {
        self.lifecycle.current_stage()
    }

    /// Advance to a lifecycle stage and emit an event on the mod event bus.
    pub fn advance_lifecycle(&mut self, stage: crate::LoadingStage) {
        match stage {
            crate::LoadingStage::Construct => {
                for entry in &self.entries {
                    let id = entry
                        .manifest
                        .as_ref()
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                    let mut event = crate::event_bus::ModConstructEvent { mod_id: id };
                    self.mod_event_bus.post(&mut event);
                }
            }
            crate::LoadingStage::CommonSetup => {
                let mut event = crate::event_bus::CommonSetupEvent;
                self.mod_event_bus.post(&mut event);
            }
            crate::LoadingStage::SidedSetup => {
                let mut event = crate::event_bus::SidedSetupEvent { client: true };
                self.mod_event_bus.post(&mut event);
            }
            crate::LoadingStage::LoadComplete => {
                let mut event = crate::event_bus::ModLoadCompleteEvent;
                self.mod_event_bus.post(&mut event);
                for entry in &self.entries {
                    let id = entry.manifest.as_ref().map(|m| m.id.clone());
                    let Some(id) = id else {
                        continue;
                    };
                    if entry.status == crate::Status::Running {
                        self.mod_event_bus
                            .post(&mut crate::event_bus::ModLoadEvent { mod_id: id });
                    }
                }
            }
            crate::LoadingStage::PostLoad => {}
        }
        self.lifecycle.advance(stage);
    }

    /// Run the conventional FML-like startup sequence through `PostLoad`.
    pub fn run_lifecycle(&mut self) {
        for stage in [
            crate::LoadingStage::Construct,
            crate::LoadingStage::CommonSetup,
            crate::LoadingStage::SidedSetup,
            crate::LoadingStage::LoadComplete,
            crate::LoadingStage::PostLoad,
        ] {
            self.advance_lifecycle(stage);
        }
    }

    /// Run a NeoForge-like registration pass: announce new registries, freeze
    /// those already known, and announce the freeze.
    pub fn run_registration(&mut self) {
        let mut new_event = crate::event_bus::NewRegistryEvent {
            name: "horizon:registries".to_string(),
        };
        self.mod_event_bus.post(&mut new_event);
        self.registries.freeze_all();
        let mut frozen_event = crate::event_bus::RegistryFrozenEvent {
            name: "horizon:registries".to_string(),
        };
        self.mod_event_bus.post(&mut frozen_event);
    }

    /// Object registry manager, NeoForge-style.
    pub fn registries(&self) -> &RegistryManager {
        &self.registries
    }

    /// Config manager.
    pub fn configs(&self) -> &ConfigManager {
        &self.configs
    }

    pub fn mob_effects(&self) -> &crate::MobEffectManager {
        &self.mob_effects
    }

    pub fn mob_effect_applied(&mut self, id: String, effect: String, seconds: u32, amplifier: u8) {
        let eff_id = crate::ResourceLocation::parse(&effect)
            .unwrap_or_else(|_| crate::ResourceLocation::parse("minecraft:effect").unwrap());
        let def = crate::MobEffectDefinition::new(eff_id, &effect, true);
        self.mob_effects
            .apply(&id, &def, i32::from(amplifier), (seconds as i32) * 20);
        let mut e = crate::event_bus::PotionApplyEvent {
            entity: id.clone(),
            effect: effect.clone(),
            amplifier: i32::from(amplifier),
            duration: (seconds as i32) * 20,
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn add_config(
        &mut self,
        id: String,
        spec: crate::parity_models::ConfigSpecDef,
        path: std::path::PathBuf,
    ) -> Result<(), String> {
        let mut cfg = ModConfig::new(spec, path);
        cfg.load()?;
        self.configs.add(id.clone(), cfg);
        let mut event = ModConfigEvent {
            mod_id: id,
            phase: crate::event_bus::ConfigPhase::Loading,
        };
        self.mod_event_bus.post(&mut event);
        Ok(())
    }

    pub fn config_reload(&mut self, id: &str) -> Result<(), String> {
        if let Some(cfg) = self.configs.get_mut(id) {
            cfg.reload()?;
        } else {
            return Err("unknown config".into());
        }
        let mut event = ModConfigEvent {
            mod_id: id.to_string(),
            phase: crate::event_bus::ConfigPhase::Reloading,
        };
        self.mod_event_bus.post(&mut event);
        Ok(())
    }

    pub fn config_save_all(&mut self) -> Result<(), String> {
        self.configs.save_all()
    }

    pub fn register_capability_provider(
        &mut self,
        provider: crate::parity_models::CapabilityProvider,
    ) {
        let kind = provider.capability.clone();
        self.capability_providers.register(provider.clone());
        let mut event = crate::parity_events::CapabilityProviderRegisteredEvent {
            capability: kind,
            cancelled: false,
        };
        self.mod_event_bus.post(&mut event);
    }

    pub fn set_attachment(&mut self, id: &str, value: impl Into<String>, synced: bool) {
        self.attachments.set(id, value, synced);
        let mut event = crate::parity_events::AttachmentSyncEvent {
            target: id.split(':').next().unwrap_or("unknown").to_string(),
            attachment: id.to_string(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut event);
    }

    pub fn register_payload(&mut self, def: crate::parity_models::CustomPayloadDef) {
        let channel = def.id.clone();
        self.payloads.register(def);
        let mut event = crate::parity_events::CustomPayloadEvent {
            channel,
            bytes: 0,
            cancelled: false,
        };
        self.mod_event_bus.post(&mut event);
    }

    pub fn receive_payload(&mut self, channel: &str, data: &[u8]) -> Result<(), String> {
        self.payloads.validate(channel, data.len())?;
        let mut event = crate::parity_events::CustomPayloadEvent {
            channel: channel.to_string(),
            bytes: data.len(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut event);
        Ok(())
    }

    pub fn recipe_loaded(&mut self, recipe: impl Into<String>) {
        let mut e = crate::parity_events::RecipeLoadEvent {
            recipe: recipe.into(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn loot_table_loaded(&mut self, table: impl Into<String>) {
        let mut e = crate::parity_events::LootTableLoadEvent {
            table: table.into(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn resource_overlay_added(&mut self, ns: impl Into<String>) {
        let mut e = crate::parity_events::ResourceOverlayEvent {
            namespace: ns.into(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn enchantment_level_set(
        &mut self,
        item: impl Into<String>,
        table: impl Into<String>,
        slot: i32,
        level: i32,
    ) {
        let mut e = crate::event_bus::EnchantmentLevelSetEvent {
            item: item.into(),
            table: table.into(),
            slot,
            level,
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn container_opened(&mut self, player: impl Into<String>, menu_type: impl Into<String>) {
        let mut e = crate::event_bus::ContainerOpenEvent {
            player: player.into(),
            menu_type: menu_type.into(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn loot_modified(&mut self, loot_table: impl Into<String>) {
        let mut e = crate::parity_events::LootTableLoadEvent {
            table: loot_table.into(),
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn resolve_loot(
        &self,
        resolver: &crate::LootResolver,
        table_id: &str,
        ctx: crate::LootContext,
    ) -> Result<Vec<String>, String> {
        let drops = resolver.resolve_table(table_id, ctx)?;
        Ok(drops
            .into_iter()
            .map(|d| format!("{}:{}", d.item, d.count))
            .collect())
    }

    pub fn surface_rule_evaluated(
        &mut self,
        biome: impl Into<String>,
        block: impl Into<String>,
        depth: i32,
    ) {
        let biome = biome.into();
        let block = block.into();
        let mut e = crate::event_bus::SurfaceRuleEvent {
            biome,
            block: block.clone(),
            depth,
            result_block: block,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn register_menu_type(&mut self, name: impl Into<String>, id: String) {
        let mut event = crate::parity_events::CustomMenuEvent {
            menu_type: id,
            cancelled: false,
        };
        // Keep name in trace only.
        let _ = name;
        self.mod_event_bus.post(&mut event);
    }

    pub fn capability_providers(&self) -> &CapabilityProviderRegistry {
        &self.capability_providers
    }

    pub fn attachments(&self) -> &AttachmentManager {
        &self.attachments
    }

    pub fn payloads(&self) -> &PayloadRegistry {
        &self.payloads
    }

    pub fn overlays(&self) -> &crate::OverlayManager {
        &self.overlays
    }

    pub fn resource_overlay_loaded(&mut self, namespace: impl Into<String>) {
        let namespace = namespace.into();
        self.overlays
            .add(crate::parity_models::ResourceOverlayDef::new(
                "mod-runtime",
                namespace.clone(),
            ));
        let mut e = crate::parity_events::ResourceOverlayEvent {
            namespace,
            cancelled: false,
        };
        self.mod_event_bus.post(&mut e);
    }

    pub fn worldgen(&self) -> &crate::WorldgenModifierManager {
        &self.worldgen
    }

    /// A server mod's `worldgen.set-surface-rule` lands here.
    pub fn worldgen_mut(&mut self) -> &mut crate::WorldgenModifierManager {
        &mut self.worldgen
    }

    pub fn fluid_interactions(&self) -> &crate::FluidInteractionResolver {
        &self.fluid_interactions
    }

    pub fn intermod(&self) -> &crate::InterModComms {
        &self.intermod
    }

    pub fn intermod_mut(&mut self) -> &mut crate::InterModComms {
        &mut self.intermod
    }

    pub fn fluid_registry(&self) -> &crate::FluidRegistryManager {
        &self.fluid_registry
    }

    pub fn fluid_registry_mut(&mut self) -> &mut crate::FluidRegistryManager {
        &mut self.fluid_registry
    }

    pub fn attributes(&self) -> &crate::AttributeManager {
        &self.attributes
    }

    pub fn attributes_mut(&mut self) -> &mut crate::AttributeManager {
        &mut self.attributes
    }

    pub fn ai_goals(&self) -> &crate::AiGoalManager {
        &self.ai_goals
    }

    pub fn ai_goals_mut(&mut self) -> &mut crate::AiGoalManager {
        &mut self.ai_goals
    }

    pub fn recipe_resolver(&self) -> &crate::RecipeResolver {
        &self.recipe_resolver
    }

    pub fn recipe_resolver_mut(&mut self) -> &mut crate::RecipeResolver {
        &mut self.recipe_resolver
    }

    /// Wiring-level helper: tell worldgen manager a level has loaded.
    pub fn wiring_level_loaded(&mut self, dimension: impl Into<String>) -> usize {
        self.worldgen.handle_level_load(&crate::LevelLoadEvent {
            dimension: dimension.into(),
            cancelled: false,
        })
    }

    /// Wiring-level helper: resolve an attribute with current modifiers.
    pub fn wiring_attribute(&self, attribute: &str) -> crate::ComputedAttribute {
        self.attributes.compute(attribute)
    }

    /// Wiring-level helper: top AI goal for an entity.
    pub fn wiring_top_goal(&self, entity: &str) -> Option<crate::AIGoalDef> {
        self.ai_goals.top_goal(entity)
    }

    /// Wiring-level helper: test a recipe composite.
    pub fn wiring_recipe_allowed(&self, composite: crate::RecipeComposite, ids: &[&str]) -> bool {
        self.recipe_resolver.evaluate_composite(composite, ids)
    }

    /// Wiring-level helper: resolve a fluid interaction.
    pub fn wiring_fluid_interaction(&self, a: &str, b: &str) -> Option<&str> {
        self.fluid_interactions.resolve(a, b)
    }

    pub fn register_menu_resolver(
        &mut self,
        menu_type: impl Into<String>,
        resolver: crate::ContainerMenuResolver,
    ) {
        self.menu_resolvers.insert(menu_type.into(), resolver);
        let mut event = crate::event_bus::RegisterEvent {
            registry: "menu-types".into(),
        };
        self.mod_event_bus.post(&mut event);
    }

    pub fn validate_container_move(&self, menu_type: &str, slot: usize, count: i32) -> i32 {
        self.menu_resolvers
            .get(menu_type)
            .map(|r| r.validate_move(slot, count))
            .unwrap_or(count)
    }

    pub fn commit_container_drag(&mut self, menu_type: &str, targets: &[usize]) -> i32 {
        self.menu_resolvers
            .get_mut(menu_type)
            .map(|r| r.commit_drag(targets))
            .unwrap_or(0)
    }

    pub fn container_sync_value(&mut self, menu_type: &str, name: &str, value: i32) -> bool {
        self.menu_resolvers
            .get_mut(menu_type)
            .map(|r| r.set_synced_data(name, value))
            .unwrap_or(false)
    }

    pub fn has_active_mob_effect(&self, entity: &str, effect: &str) -> bool {
        self.mob_effects.active(entity, effect).is_some()
    }

    pub fn worldgen_apply_features(&self, biome: &str) -> Vec<String> {
        self.worldgen.features_for(biome)
    }

    pub fn wiring_mob_effect_active(&self, entity: &str, effect: &str) -> bool {
        self.mob_effects.active(entity, effect).is_some()
    }

    pub fn wiring_container_validated_move(&self, menu_type: &str, slot: usize, count: i32) -> i32 {
        self.validate_container_move(menu_type, slot, count)
    }

    pub fn wiring_worldgen_apply_features(&self, biome: &str) -> Vec<String> {
        self.worldgen_apply_features(biome)
    }

    pub fn config_sync_payload(&mut self, mod_id: String, config_text: String) -> Vec<u8> {
        let mut payload = Vec::with_capacity(16 + config_text.len());
        payload.extend_from_slice(b"horizon_net:");
        payload.extend_from_slice(mod_id.as_bytes());
        payload.push(b':');
        let remaining = 32 * 1024 - payload.len();
        payload.extend_from_slice(&config_text.as_bytes()[..config_text.len().min(remaining)]);
        let mut event = ModConfigEvent {
            mod_id,
            phase: crate::event_bus::ConfigPhase::Reloading,
        };
        self.mod_event_bus.post(&mut event);
        payload
    }

    /// Post an event to the bus
    pub fn post_event<E: crate::event_bus::Event>(&self, event: &mut E) {
        self.event_bus.post(event);
    }

    /// Server mods' bytes on blocks, as JSON, for the game to save with the world.
    pub fn block_data_json(&self) -> String {
        let data = self
            .block_data
            .lock()
            .map(|d| d.clone())
            .unwrap_or_default();
        serde_json::to_string(&data).unwrap_or_default()
    }
    /// Puts back the bytes [`Mods::block_data_json`] saved (an unreadable or
    /// empty text: none), when a world opens.
    pub fn set_block_data_json(&mut self, json: &str) {
        let data = serde_json::from_str(json).unwrap_or_default();
        if let Ok(mut slot) = self.block_data.lock() {
            *slot = data;
        }
    }
    /// Changes whenever the running mods (so their commands) change.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Whether a running mod may use capability `world`: only then is a
    /// view worth lending.
    pub fn wants_world(&self) -> bool {
        self.entries.iter().any(|e| {
            e.status == Status::Running
                && e.manifest
                    .as_ref()
                    .is_some_and(|m| m.capabilities.contains(&crate::Capability::World))
        })
    }
    /// Lends the game's world to capability `world` for the calls until
    /// [`Mods::take_world`]; with none lent, `get-block` finds nothing and
    /// `set-blocks` checks only its count.
    pub fn lend_world(&mut self, view: Box<dyn WorldView>) {
        if let Ok(mut slot) = self.world.lock() {
            *slot = Some(view);
        }
    }
    /// The view [`Mods::lend_world`] lent, back to the game.
    pub fn take_world(&mut self) -> Option<Box<dyn WorldView>> {
        self.world.lock().ok()?.take()
    }

    /// Reads every mod at `paths` and starts those `enabled` says are on,
    /// client and server mods alike. A command that `reserved` (vanilla's
    /// commands) or an earlier mod already uses is refused.
    pub fn load(
        paths: &[PathBuf],
        enabled: impl Fn(&str) -> bool,
        reserved: impl Fn(&str) -> bool,
    ) -> Mods {
        Mods::load_for(Runs::Both, paths, enabled, reserved)
    }
    /// [`Mods::load`], starting only the mods `runs` says this game runs; the
    /// others that are on are [`Status::Elsewhere`].
    pub fn load_for(
        runs: Runs,
        paths: &[PathBuf],
        enabled: impl Fn(&str) -> bool,
        reserved: impl Fn(&str) -> bool,
    ) -> Mods {
        let mut mods = Mods {
            client_half: matches!(runs, Runs::Client),
            ..Mods::default()
        };

        // Dependency-aware ordering (best effort): only reorder when manifests
        // declare dependencies, so existing content-only and failure tests keep
        // their expected order.
        let has_dependencies = paths.iter().any(|path| {
            Package::read(path)
                .map(|package| !package.manifest.dependencies.is_empty())
                .unwrap_or(false)
        });
        let ordered: Vec<PathBuf> = if has_dependencies {
            let mut resolver = DependencyResolver::new();
            let mut available: Vec<(PathBuf, String)> = Vec::new();
            let mut failed: Vec<PathBuf> = Vec::new();
            for path in paths {
                match Package::read(path) {
                    Ok(package) => {
                        resolver.add_container(ModContainer::from_manifest(
                            package.manifest.clone(),
                            path.clone(),
                        ));
                        available.push((path.clone(), package.manifest.id.clone()));
                    }
                    Err(_) => failed.push(path.clone()),
                }
            }
            let sorted_ids = resolver.resolve().unwrap_or_default();
            let mut sorted: Vec<PathBuf> = if sorted_ids.is_empty() {
                available.iter().map(|(p, _)| p.clone()).collect()
            } else {
                let mut sorted_available = available.clone();
                sorted_available.sort_by_key(|(_, id)| {
                    sorted_ids
                        .iter()
                        .position(|x| x == id)
                        .unwrap_or(usize::MAX)
                });
                sorted_available.into_iter().map(|(p, _)| p).collect()
            };
            sorted.extend(failed);
            sorted
        } else {
            paths.to_vec()
        };

        for path_ref in &ordered {
            let path: PathBuf = path_ref.clone();
            let modified = modified(&path);
            let mut entry = Entry {
                path: path.clone(),
                manifest: None,
                container: None,
                status: Status::Off,
                cost: Duration::ZERO,
                run: None,
                modified,
            };
            match Package::read(&path) {
                Err(e) => entry.status = Status::Failed(e),
                Ok(package) => {
                    let on = enabled(&package.manifest.id);
                    entry.manifest = Some(package.manifest.clone());
                    // Create mod container
                    let mut container =
                        ModContainer::from_manifest(package.manifest.clone(), path.clone());
                    container.set_state(ModState::Validated);
                    container.dependencies = package
                        .manifest
                        .dependencies
                        .iter()
                        .map(|dep| ModDependency {
                            mod_id: dep.clone(),
                            version_range: "*".into(),
                            required: true,
                            ordering: crate::container::DependencyOrdering::After,
                        })
                        .collect();
                    entry.container = Some(container);
                    if on && !runs.starts(&package.manifest) {
                        entry.status = Status::Elsewhere;
                        if let Some(c) = entry.container.as_mut() {
                            c.set_state(ModState::Disabled);
                        }
                    } else if on {
                        // Content is read before the world loads; a mod whose
                        // content is refused stays off, with the reason.
                        let content = crate::Content::read(&path, &package.manifest);
                        entry.status =
                            match content.and_then(|_| mods.start(&path, &package, &reserved)) {
                                Ok(run) => {
                                    entry.run = run;
                                    if let Some(c) = entry.container.as_mut() {
                                        c.set_state(ModState::Loaded);
                                    }
                                    Status::Running
                                }
                                Err(e) => {
                                    if let Some(c) = entry.container.as_mut() {
                                        c.set_state(ModState::Errored(e.clone()));
                                    }
                                    Status::Failed(e)
                                }
                            };
                    }
                }
            }
            if let (Status::Failed(reason), Some(m)) = (&entry.status, &entry.manifest) {
                log::warn!("Horizon: mod {} did not start: {reason}", m.id);
            }
            mods.entries.push(entry);
        }
        // NeoForge-style mod bus: construction then load events.
        for entry in &mods.entries {
            if entry.status == Status::Running
                && let Some(c) = &entry.container
            {
                let mut constructed = crate::event_bus::ModConstructEvent {
                    mod_id: c.id.clone(),
                };
                mods.mod_event_bus.post(&mut constructed);

                let mut loaded = ModLoadEvent {
                    mod_id: c.id.clone(),
                };
                mods.event_bus.post(&mut loaded);
            }
        }
        for registry in [
            "horizon:blocks",
            "horizon:items",
            "horizon:entities",
            "horizon:fluids",
        ] {
            let mut event = crate::event_bus::RegisterEvent {
                registry: registry.to_string(),
            };
            mods.mod_event_bus.post(&mut event);
        }
        let mut common = crate::event_bus::CommonSetupEvent;
        mods.mod_event_bus.post(&mut common);
        let mut capabilities = crate::event_bus::RegisterCapabilitiesEvent;
        mods.mod_event_bus.post(&mut capabilities);
        let mut client = crate::event_bus::SidedSetupEvent { client: true };
        mods.mod_event_bus.post(&mut client);
        let mut loaded = crate::event_bus::ModLoadCompleteEvent;
        mods.mod_event_bus.post(&mut loaded);

        // Core object registries for parity: available from startup.
        macro_rules! reg {
            ($name:expr, $ty:ty) => {
                let _ = mods
                    .registries
                    .add($name, crate::Registry::<$ty>::new($name));
            };
        }
        reg!(crate::Registries::fluids(), FluidDefinition);
        reg!(crate::Registries::mob_effects(), MobEffectDefinition);
        reg!(crate::Registries::enchantments(), EnchantmentDefinition);
        reg!(crate::Registries::particles(), ParticleDefinition);
        reg!(crate::Registries::sounds(), SoundDefinition);
        reg!(crate::Registries::menu_types(), ContainerMenuDefinition);
        reg!(crate::Registries::loot_modifiers(), LootModifierDefinition);
        reg!(crate::Registries::structures(), StructureDefinition);
        reg!(crate::Registries::surface_rules(), SurfaceRuleDefinition);
        reg!(crate::Registries::dimensions(), DimensionDefinition);
        reg!(
            crate::Registries::block_states(),
            BlockStatePropertyDefinition
        );
        reg!(crate::Registries::tags(), TagDefinition);

        let mut new_registry = crate::event_bus::NewRegistryEvent {
            name: "core".into(),
        };
        mods.mod_event_bus.post(&mut new_registry);

        mods.fluid_interactions = crate::FluidInteractionResolver::with_defaults();
        mods.fluid_registry = crate::FluidRegistryManager::new();
        mods.attributes = crate::AttributeManager::new();
        mods.ai_goals = crate::AiGoalManager::new();
        mods.recipe_resolver = crate::RecipeResolver::new();
        mods.intermod = crate::InterModComms::new();
        mods.block_state_properties = crate::BlockStatePropertyResolver::new();
        mods.tag_manager = crate::TagManager::new();
        mods.menu_resolvers = std::collections::BTreeMap::new();
        mods.worldgen = crate::WorldgenModifierManager::new();

        mods.advance_lifecycle(crate::LoadingStage::Construct);
        mods.run_registration();
        mods.advance_lifecycle(crate::LoadingStage::CommonSetup);
        mods.advance_lifecycle(crate::LoadingStage::SidedSetup);
        mods.advance_lifecycle(crate::LoadingStage::LoadComplete);
        mods.advance_lifecycle(crate::LoadingStage::PostLoad);

        if !mods.registries.contains(&crate::Registries::menu_types()) {
            let _ = mods.registries.add(
                crate::Registries::menu_types(),
                crate::Registry::<crate::ContainerMenuDefinition>::new(
                    crate::Registries::menu_types(),
                ),
            );
        }

        mods
    }

    fn engine(&mut self) -> Result<&(Engine, Linker<State>), String> {
        if self.engine.is_none() {
            let mut config = Config::new();
            config.wasm_component_model(true).consume_fuel(true);
            let engine = Engine::new(&config).map_err(|e| e.to_string())?;
            let mut linker = Linker::new(&engine);
            HorizonMod::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            server_bindings::horizon::mods::storages::add_to_linker::<State, HasSelf<State>>(
                &mut linker,
                |s| s,
            )
            .map_err(|e| e.to_string())?;
            server_bindings::horizon::mods::entities::add_to_linker::<State, HasSelf<State>>(
                &mut linker,
                |s| s,
            )
            .map_err(|e| e.to_string())?;
            server_bindings::horizon::mods::server_net::add_to_linker::<State, HasSelf<State>>(
                &mut linker,
                |s| s,
            )
            .map_err(|e| e.to_string())?;
            players::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            mod_world::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            worldgen::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            UiHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RenderHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RecipesHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            self.engine = Some((engine, linker));
        }
        Ok(self.engine.as_ref().expect("made above"))
    }

    /// Starts a mod's code; `Ok(None)` for a content-only mod, which has
    /// nothing to run (the game reads its content before the world loads).
    /// `path` is the mod's own `.hmod` or folder, for capability `storage`'s
    /// per-mod directory.
    fn start(
        &mut self,
        path: &Path,
        package: &Package,
        reserved: &dyn Fn(&str) -> bool,
    ) -> Result<Option<Running>, String> {
        let m = &package.manifest;
        // In-frame, where one game is both, a two-halved mod runs its server half.
        let client_half = m.two_halves() && self.client_half;
        let code = if client_half {
            &package.client_wasm
        } else {
            &package.wasm
        };
        let Some(wasm) = code else {
            return Ok(None);
        };
        // Two running mods with the same id would share one `data_dir`
        // (capability `storage`): the first loaded keeps the id.
        if self
            .entries
            .iter()
            .any(|e| e.run.is_some() && e.manifest.as_ref().is_some_and(|other| other.id == m.id))
        {
            return Err(format!("another running mod already uses id {}", m.id));
        }
        let server = m.side == Side::Server || (m.two_halves() && !client_half);
        let taken: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .filter(|r| r.hooks.server().is_some() == server)
            .flat_map(|r| r.hooks.commands())
            .map(str::to_owned)
            .collect();
        let snapshot = self.snapshot.clone();
        let world = self.world.clone();
        let block_data = self.block_data.clone();
        let (engine, linker) = self.engine()?;
        let component =
            Component::new(engine, wasm).map_err(|e| format!("not a component: {e}"))?;
        let ty = component.component_type();
        // An interface imported for its types alone (`use entities.{entity}`
        // in an export) calls nothing, so it needs no capability.
        use wasmtime::component::types::ComponentItem;
        let calls = |item: &ComponentItem| match item {
            ComponentItem::ComponentInstance(i) => i
                .exports(engine)
                .any(|(_, e)| matches!(e.ty, ComponentItem::ComponentFunc(_))),
            _ => true,
        };
        let imports: Vec<String> = ty
            .imports(engine)
            .filter(|(_, item)| calls(&item.ty))
            .map(|(name, _)| name.to_string())
            .collect();
        m.check_imports(imports.iter().map(String::as_str))?;
        // A function this Horizon doesn't have: say so, not wasmtime's linking error.
        let functions = crate::api_level::imports(wasm)?;
        let (level, function) = crate::api_level::needed(functions.iter().map(String::as_str));
        if let Some(why) = crate::api_level::too_new(level, function) {
            return Err(why);
        }
        let data_dir = path
            .parent()
            .unwrap_or(Path::new("."))
            .join(crate::manifest::STORAGE_DIR)
            .join(&m.id);
        let state = State {
            name: m.name.clone(),
            id: m.id.clone(),
            snapshot,
            out: Vec::new(),
            panel: None,
            ui: UiState::default(),
            shapes: Vec::new(),
            models: Vec::new(),
            menu_layouts: Vec::new(),
            corner: None,
            zoom: None,
            minimap: None,
            far_terrain: None,
            input: InputState::default(),
            browser: BrowserState::default(),
            config: crate::manifest::load_config(path, m),
            package: (path.to_path_buf(), m.clone()),
            data_dir,
            storage_budget: STORAGE_WRITE_BUDGET,
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY)
                .instances(16)
                .build(),
            world,
            block_data,
            mob_effects: crate::MobEffectManager::new(),
        };
        let mut store = Store::new(engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(FUEL_INIT).map_err(|e| e.to_string())?;
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|e| reason(&e))?;
        let net_client = client_net_bindings::ClientNetHost::new(&mut store, &instance).ok();
        let net_server = server_net_bindings::ServerNetHost::new(&mut store, &instance).ok();
        let (mut hooks, ui, blocks, events) = if server {
            let exports = HorizonServerMod::new(&mut store, &instance).map_err(|e| reason(&e))?;
            // `block-events` is a server mod built since it; older ones are not asked.
            let blocks = BlockHost::new(&mut store, &instance).ok();
            let r = exports
                .horizon_mods_server_events()
                .call_init(&mut store)
                .map_err(|e| reason(&e))??;
            let events = EventsHost::new(&mut store, &instance).ok();
            (Hooks::Server(exports, r), None, blocks, events)
        } else {
            let exports = HorizonMod::new(&mut store, &instance).map_err(|e| reason(&e))?;
            // `ui-events` is a mod built since `ui`'s; older mods have no export
            // and stay callable through `events` alone.
            let ui = UiHost::new(&mut store, &instance).ok();
            let r = exports
                .horizon_mods_events()
                .call_init(&mut store)
                .map_err(|e| reason(&e))??;
            (Hooks::Client(exports, r), ui, None, None)
        };
        let free = |c: &str| {
            let free = !reserved(c) && !taken.iter().any(|t| t == c);
            if !free {
                log::warn!("Horizon: mod {} cannot take /{c}: already a command", m.id);
            }
            free
        };
        match &mut hooks {
            Hooks::Client(_, r) => r.commands.retain(|c| free(c)),
            Hooks::Server(_, r) => {
                r.commands.retain(|c| free(&c.name));
                for c in &mut r.commands {
                    if let Err(e) = check_arguments(&c.arguments) {
                        log::warn!(
                            "Horizon: mod {}'s /{}: {e}; it takes free text",
                            m.id,
                            c.name
                        );
                        c.arguments.clear();
                    }
                }
            }
        }
        self.generation += 1;
        self.out.append(&mut store.data_mut().out);
        let entity_events = EntityEventsHost::new(&mut store, &instance).ok();
        let logic_events = LogicEventsHost::new(&mut store, &instance).ok();
        Ok(Some(Running {
            entity_events,
            logic_events,
            events,
            net_client,
            net_server,
            store,
            ui,
            blocks,
            hooks,
            raycasts: functions.iter().any(|f| f == "game.raycast"),
            reads_recipes: functions.iter().any(|f| f.starts_with("recipes.")),
        }))
    }

    /// True when at least one mod runs: the game skips its mod work otherwise.
    pub fn is_running(&self) -> bool {
        self.entries.iter().any(|e| e.run.is_some())
    }
    /// True when a server mod runs: the server skips its mod work otherwise.
    pub fn runs_server(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.hooks.server().is_some()))
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// What mods read until the next snapshot.
    pub fn set_snapshot(&mut self, mut snapshot: Snapshot) {
        snapshot.running = self
            .entries
            .iter()
            .filter(|e| e.run.is_some())
            .filter_map(|e| e.manifest.as_ref().map(|m| m.id.clone()))
            .collect();
        self.snapshot = Arc::new(snapshot);
    }

    /// Calls `f` on every running mod whose hooks `wants`, with `fuel`.
    fn each(
        &mut self,
        fuel: u64,
        wants: impl Fn(&Hooks) -> bool,
        mut f: impl FnMut(&mut Running) -> wasmtime::Result<()>,
    ) {
        for i in 0..self.entries.len() {
            if self.entries[i]
                .run
                .as_ref()
                .is_some_and(|r| wants(&r.hooks))
            {
                self.call(i, fuel, &mut f);
            }
        }
    }
    /// [`Mods::each`] over the client mods.
    fn each_client(
        &mut self,
        fuel: u64,
        wants: impl Fn(&Registration) -> bool,
        mut f: impl FnMut(&ClientEvents, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        self.each(
            fuel,
            |h| h.client().is_some_and(&wants),
            |run| match &run.hooks {
                Hooks::Client(m, _) => f(m.horizon_mods_events(), &mut run.store),
                Hooks::Server(..) => Ok(()),
            },
        );
    }
    /// [`Mods::each`] over the server mods.
    fn each_server(
        &mut self,
        fuel: u64,
        wants: impl Fn(&ServerRegistration) -> bool,
        mut f: impl FnMut(&ServerEvents, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        self.each(
            fuel,
            |h| h.server().is_some_and(&wants),
            |run| match &run.hooks {
                Hooks::Server(m, _) => f(m.horizon_mods_server_events(), &mut run.store),
                Hooks::Client(..) => Ok(()),
            },
        );
    }

    /// One call into mod `i`; a trap or an overrun turns the mod off.
    fn call<R>(
        &mut self,
        i: usize,
        fuel: u64,
        f: impl FnOnce(&mut Running) -> wasmtime::Result<R>,
    ) -> Option<R> {
        let entry = &mut self.entries[i];
        let run = entry.run.as_mut()?;
        run.store.data_mut().snapshot = self.snapshot.clone();
        run.store.data_mut().storage_budget = STORAGE_WRITE_BUDGET;
        let started = Instant::now();
        let result = run.store.set_fuel(fuel).and_then(|()| f(run));
        // An average over about the last 64 calls.
        entry.cost = (entry.cost * 63 + started.elapsed()) / 64;
        let data = run.store.data_mut();
        let (open, close) = (
            std::mem::take(&mut data.ui.open),
            std::mem::take(&mut data.ui.close),
        );
        self.out.append(&mut data.out);
        if open {
            self.out.push(Output::OpenScreen { owner: i });
        }
        if close {
            self.out.push(Output::CloseScreen { owner: i });
        }
        match result {
            Ok(r) => Some(r),
            Err(e) => {
                let reason = reason(&e);
                log::warn!("Horizon: mod {} stopped: {reason}", run.store.data().id);
                let name = run.store.data().name.clone();
                self.out.push(Output::Stopped {
                    name,
                    reason: reason.clone(),
                });
                entry.status = Status::Failed(reason);
                entry.run = None;
                self.generation += 1;
                None
            }
        }
    }

    /// A game tick.
    pub fn tick(&mut self, tick: u64) {
        let mut tick_event = crate::event_bus::TickEvent { tick };
        self.event_bus.post(&mut tick_event);
        self.each_client(FUEL_TICK, |h| h.tick, |m, s| m.call_tick(s, tick));
    }
    /// A rendered frame, `seconds` after the last.
    pub fn frame(&mut self, seconds: f32) {
        self.each_client(FUEL_FRAME, |h| h.frame, |m, s| m.call_frame(s, seconds));
    }
    /// A chat line the player received, as plain text.
    pub fn chat(&mut self, text: &str) {
        self.each_client(FUEL_EVENT, |h| h.chat, |m, s| m.call_chat(s, text));
    }
    /// Whether a running client mod owns `/name`.
    pub fn has_command(&self, name: &str) -> bool {
        self.command_owner(false, name).is_some()
    }
    fn command_owner(&self, server: bool, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.run.as_ref().is_some_and(|r| {
                r.hooks.server().is_some() == server && r.hooks.commands().contains(&name)
            })
        })
    }
    /// Runs `line` (without its slash) if a client mod owns its command;
    /// false when none does.
    pub fn command(&mut self, line: &str) -> bool {
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let Some(i) = self.command_owner(false, name) else {
            return false;
        };
        let result = self.call(i, FUEL_EVENT, |run| match &run.hooks {
            Hooks::Client(m, _) => m
                .horizon_mods_events()
                .call_command(&mut run.store, name, args),
            Hooks::Server(..) => Ok(Ok(None)),
        });
        match result {
            Some(Ok(Some(text))) => self.out.push(Output::Feedback { text, error: false }),
            Some(Err(text)) => self.out.push(Output::Feedback { text, error: true }),
            Some(Ok(None)) | None => {}
        }
        true
    }
    /// The commands running client mods own, for suggestions.
    pub fn commands(&self) -> impl Iterator<Item = &str> {
        self.owned_commands(false)
    }
    fn owned_commands(&self, server: bool) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .filter(move |r| r.hooks.server().is_some() == server)
            .flat_map(|r| r.hooks.commands())
    }

    /// A server tick.
    pub fn server_tick(&mut self, tick: u64) {
        self.block_hooks(tick);
        self.each_server(FUEL_TICK, |h| h.tick, |m, s| m.call_tick(s, tick));
    }
    /// `player` joined the server (they are in the snapshot's players).
    pub fn player_joined(&mut self, player: &ServerPlayer) {
        self.each_server(
            FUEL_EVENT,
            |h| h.joins,
            |m, s| m.call_player_joined(s, player),
        );
    }
    /// `player` left the server (they are no longer in the snapshot's players).
    pub fn player_left(&mut self, player: &ServerPlayer) {
        self.each_server(
            FUEL_EVENT,
            |h| h.joins,
            |m, s| m.call_player_left(s, player),
        );
    }
    /// A chat message `player` sent, as plain text.
    pub fn player_chat(&mut self, player: &ServerPlayer, text: &str) {
        self.each_server(FUEL_EVENT, |h| h.chat, |m, s| m.call_chat(s, player, text));
    }
    /// `player` broke `state` at `pos`.
    pub fn block_broken(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str) {
        self.each_server(
            FUEL_EVENT,
            |h| h.blocks,
            |m, s| m.call_block_broken(s, player, pos, state),
        );
    }
    /// Running server mods asked about blocks: registration `blocks` and the
    /// `block-events` export.
    fn asks_blocks(&self, i: usize) -> bool {
        self.entries[i]
            .run
            .as_ref()
            .is_some_and(|r| r.blocks.is_some() && r.hooks.server().is_some_and(|h| h.blocks))
    }
    /// `player` starts breaking `state` at `pos` (`block-events.break-block`):
    /// true when a server mod refuses it, and the block stays.
    pub fn break_block(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str) -> bool {
        let mut refused = false;
        for i in 0..self.entries.len() {
            if !self.asks_blocks(i) {
                continue;
            }
            refused |= self
                .call(i, FUEL_EVENT, |run| {
                    let Running { blocks, store, .. } = run;
                    match blocks {
                        Some(b) => b
                            .horizon_mods_block_events()
                            .call_break_block(store, player, pos.x, pos.y, pos.z, state),
                        None => Ok(false),
                    }
                })
                .unwrap_or(false);
        }
        refused
    }
    /// `player` right-clicks `state` at `pos` holding `item`
    /// (`block-events.use-block`): true when a server mod handled it, and
    /// vanilla does nothing there. The first mod that takes it ends the asking.
    pub fn use_block(
        &mut self,
        player: &ServerPlayer,
        pos: BlockPos,
        state: &str,
        item: &str,
    ) -> bool {
        for i in 0..self.entries.len() {
            if !self.asks_blocks(i) {
                continue;
            }
            let taken = self
                .call(i, FUEL_EVENT, |run| {
                    let Running { blocks, store, .. } = run;
                    match blocks {
                        Some(b) => b
                            .horizon_mods_block_events()
                            .call_use_block(store, player, pos.x, pos.y, pos.z, state, item),
                        None => Ok(false),
                    }
                })
                .unwrap_or(false);
            if taken {
                return true;
            }
        }
        false
    }
    /// `player` placed `state` at `pos`.
    pub fn block_placed(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str) {
        self.each_server(
            FUEL_EVENT,
            |h| h.blocks,
            |m, s| m.call_block_placed(s, player, pos, state),
        );
    }
    /// The commands running server mods own and their arguments, for the
    /// server's command tree.
    pub fn server_commands(&self) -> impl Iterator<Item = &ServerCommand> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.hooks.server())
            .flat_map(|r| r.commands.iter())
    }
    /// Runs `line` (without its slash), typed by `player`, if a server mod
    /// owns its command; its feedback is an [`Output::Reply`]. False when
    /// none does.
    pub fn server_command(&mut self, player: &ServerPlayer, line: &str) -> bool {
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let Some(i) = self.command_owner(true, name) else {
            return false;
        };
        let reply = |text, error| Output::Reply {
            player: player.id.clone(),
            text,
            error,
        };
        let arguments = self.entries[i]
            .run
            .as_ref()
            .and_then(|r| r.hooks.server())
            .and_then(|r| r.commands.iter().find(|c| c.name == name))
            .map(|c| c.arguments.as_slice())
            .unwrap_or_default();
        let values = match crate::commands::parse(arguments, args, player, &self.snapshot.players) {
            Ok(values) => values,
            Err(e) => {
                self.out.push(reply(e, true));
                return true;
            }
        };
        let result = self.call(i, FUEL_EVENT, |run| match &run.hooks {
            Hooks::Server(m, _) => m.horizon_mods_server_events().call_command(
                &mut run.store,
                player,
                name,
                args,
                &values,
            ),
            Hooks::Client(..) => Ok(Ok(None)),
        });
        match result {
            Some(Ok(Some(text))) => self.out.push(reply(text, false)),
            Some(Err(text)) => self.out.push(reply(text, true)),
            Some(Ok(None)) | None => {}
        }
        true
    }
    /// The running mods' HUD panels.
    pub fn panels(&self) -> impl Iterator<Item = &Panel> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().panel.as_ref())
    }
    /// The running mods' corner panels (capability `render`), in load order.
    pub fn corner_panels(&self) -> impl Iterator<Item = &CornerPanel> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().corner.as_ref())
    }
    /// Some running mod calls `game.raycast`: the snapshot needs a
    /// [`Snapshot::view_hit`].
    pub fn wants_raycast(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.raycasts))
    }
    /// The panel beside container screens (`browser.set-items`): the first
    /// running mod with items or recipes, its index, the panel's version and
    /// the items.
    pub fn browser_items(&self) -> Option<(usize, u64, &[BrowserStack])> {
        self.entries.iter().enumerate().find_map(|(i, e)| {
            let b = &e.run.as_ref()?.store.data().browser;
            (!b.items.is_empty() || b.recipes.is_some()).then_some((
                i,
                b.version,
                b.items.as_slice(),
            ))
        })
    }
    /// What mod `owner`'s panel shows instead of its items
    /// (`browser.set-recipes`).
    pub fn browser_recipes(&self, owner: usize) -> Option<&RecipePage> {
        let b = &self.entries.get(owner)?.run.as_ref()?.store.data().browser;
        b.recipes.as_ref()
    }
    /// The player did `event` on mod `owner`'s panel.
    pub fn browser_event(&mut self, owner: usize, event: BrowserEvent) {
        if let Some(run) = self.entries.get_mut(owner).and_then(|e| e.run.as_mut()) {
            run.store.data_mut().browser.push(event);
        }
    }
    /// The key `key` went down with the pointer on `stack` in a container
    /// screen: each mod with a key on it hears its own name for it.
    pub fn browser_key(&mut self, key: &str, stack: &BrowserStack) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let data = run.store.data_mut();
            let names: Vec<String> = data
                .input
                .keys
                .iter()
                .filter(|k| k.key == key)
                .map(|k| k.name.clone())
                .collect();
            for key in names {
                data.browser.push(BrowserEvent::Key(BrowserItemKey {
                    key,
                    stack: stack.clone(),
                }));
            }
        }
    }
    /// Some running mod reads capability `recipes`: the snapshot needs
    /// [`Snapshot::recipes`] and [`Snapshot::known_recipes`].
    pub fn wants_recipes(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.reads_recipes))
    }
    /// Some running mod has a `frame` hook: the game refreshes the snapshot
    /// every frame for it.
    pub fn wants_frames(&self) -> bool {
        self.entries.iter().any(|e| {
            e.run
                .as_ref()
                .is_some_and(|r| r.hooks.client().is_some_and(|h| h.frame))
        })
    }
    /// Some running mod claimed items (capability `input`).
    pub fn claims_any(&self) -> bool {
        self.inputs().any(|i| !i.claimed.is_empty())
    }
    /// Some running mod claimed the item `id`.
    pub fn claims(&self, id: &str) -> bool {
        self.inputs().any(|i| i.claimed.iter().any(|c| c == id))
    }
    /// Some running mod registered a key.
    pub fn has_keys(&self) -> bool {
        self.inputs().any(|i| !i.keys.is_empty())
    }
    /// The mods' keys: (mod name, key name, label, key), for Controls.
    pub fn mod_keys(&self) -> impl Iterator<Item = (&str, &ModKey)> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| {
                let d = r.store.data();
                d.input.keys.iter().map(move |k| (d.name.as_str(), k))
            })
    }
    /// Mod `i`'s keys, as it registered them (with the player's choices).
    pub fn keys_of(&self, i: usize) -> &[ModKey] {
        self.entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .map_or(&[], |r| r.store.data().input.keys.as_slice())
    }
    /// The keys the player chose (`ui`'s key widget), by mod id: each
    /// running mod's `name → key`. They replace the defaults registered
    /// so far and later ones. Keys that aren't keys are skipped.
    pub fn set_bindings<'a>(&mut self, chosen: impl Fn(&str) -> Vec<(&'a str, &'a str)>) {
        for e in &mut self.entries {
            let (Some(m), Some(run)) = (&e.manifest, e.run.as_mut()) else {
                continue;
            };
            let input = &mut run.store.data_mut().input;
            input.bindings = chosen(&m.id)
                .into_iter()
                .filter(|(_, key)| key_ok(key))
                .map(|(n, k)| (n.to_owned(), k.to_owned()))
                .collect();
            for (name, key) in input.bindings.clone() {
                input.rebind(&name, key);
            }
        }
    }
    /// The player binds mod `i`'s key `name` to `key` (`key.keyboard.x`,
    /// `key.mouse.4`, `key.keyboard.unknown` for none). False when the mod
    /// has no such key or `key` isn't one.
    pub fn rebind(&mut self, i: usize, name: &str, key: &str) -> bool {
        let Some(run) = self.entries.get_mut(i).and_then(|e| e.run.as_mut()) else {
            return false;
        };
        let input = &mut run.store.data_mut().input;
        if !key_ok(key) || !input.keys.iter().any(|k| k.name == name) {
            return false;
        }
        input.bindings.retain(|(n, _)| n != name);
        input.bindings.push((name.to_owned(), key.to_owned()));
        input.rebind(name, key.to_owned());
        true
    }
    /// The use button went down with `item`: `use-start` to each mod that
    /// claimed it.
    pub fn use_start(&mut self, item: &HeldItem) {
        let event = InputEvent::UseStart(ItemUse {
            item: item.clone(),
            ticks: 0,
        });
        self.push_input(|i| i.claimed.contains(&item.id), &event);
    }
    /// The claimed use ended after `ticks`.
    pub fn use_stop(&mut self, item: &HeldItem, ticks: u32) {
        let event = InputEvent::UseStop(ItemUse {
            item: item.clone(),
            ticks,
        });
        self.push_input(|i| i.claimed.contains(&item.id), &event);
    }
    /// The key `key` (`key.keyboard.r`) went down or up: each mod with a
    /// key on it hears its own name for it.
    pub fn key(&mut self, key: &str, down: bool) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            let names: Vec<String> = input
                .keys
                .iter()
                .filter(|k| k.key == key)
                .map(|k| k.name.clone())
                .collect();
            for name in names {
                let held = input.down.iter().position(|n| *n == name);
                match (down, held) {
                    (true, None) => input.down.push(name.clone()),
                    (false, Some(i)) => {
                        input.down.swap_remove(i);
                    }
                    // Already let go when a screen opened.
                    (false, None) => continue,
                    (true, Some(_)) => {}
                }
                input.push(if down {
                    InputEvent::KeyDown(name)
                } else {
                    InputEvent::KeyUp(name)
                });
            }
        }
    }
    /// A screen opened, or the game stopped playing: every mod key held
    /// lets go (`key-up`), so a key needs a new press afterwards.
    pub fn release_keys(&mut self) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            for name in std::mem::take(&mut input.down) {
                input.push(InputEvent::KeyUp(name));
            }
        }
    }
    /// Some running mod holds the mouse wheel.
    pub fn wheel_claimed(&self) -> bool {
        self.inputs().any(|i| i.wheel)
    }
    /// Wheel steps while playing: `wheel` to the mods holding it; false
    /// when none does (the hotbar takes them).
    pub fn wheel(&mut self, steps: f32) -> bool {
        let event = InputEvent::Wheel(steps);
        let claimed = self.wheel_claimed();
        self.push_input(|i| i.wheel, &event);
        claimed
    }
    /// The strongest zoom a running mod asks for (capability `render`).
    pub fn zoom(&self) -> Option<ModZoom> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().zoom)
            .max_by(|a, b| a.factor.total_cmp(&b.factor))
    }
    /// The minimap the first running mod shows (capability `render`).
    pub fn minimap(&self) -> Option<ModMinimap> {
        self.entries
            .iter()
            .find_map(|e| e.run.as_ref()?.store.data().minimap)
    }
    /// The far terrain the first running mod draws (capability `render`).
    pub fn far_terrain(&self) -> Option<ModFarTerrain> {
        self.entries
            .iter()
            .find_map(|e| e.run.as_ref()?.store.data().far_terrain)
    }
    fn inputs(&self) -> impl Iterator<Item = &InputState> {
        self.entries
            .iter()
            .filter_map(|e| Some(&e.run.as_ref()?.store.data().input))
    }
    fn push_input(&mut self, to: impl Fn(&InputState) -> bool, event: &InputEvent) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            if to(input) {
                input.push(event.clone());
            }
        }
    }
    /// The running mods' HUD shapes (capability `render`), each mod's in
    /// its own order, mods in load order.
    /// The layout a mod gave the windows titled `title`.
    pub fn menu_layout(&self, title: &str) -> Option<&render::MenuLayout> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| &r.store.data().menu_layouts)
            .find(|l| l.title == title)
    }
    /// What every mod has placed with `render.set-models`.
    pub fn world_models(&self) -> impl Iterator<Item = &render::PlacedModel> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| &r.store.data().models)
    }
    /// The canvas mod `i`'s screen is, with its clickable areas.
    pub fn canvas(&self, i: usize) -> Option<&(Vec<Shape>, Vec<render::CanvasArea>)> {
        self.entries
            .get(i)?
            .run
            .as_ref()?
            .store
            .data()
            .ui
            .canvas
            .as_ref()
    }
    pub fn hud_shapes(&self) -> impl Iterator<Item = &Shape> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| &r.store.data().shapes)
    }
    /// The screen mod `i` last pushed with `ui.open-screen`, while it is up.
    /// The running mod that replaced the vanilla screen `vanilla`
    /// (`ui.replace-screen`), first in load order: the game opens its screen
    /// instead, with [`Mods::show_replacement`].
    pub fn replacement_owner(&self, vanilla: &str) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.status == Status::Running
                && e.run
                    .as_ref()
                    .is_some_and(|r| r.store.data().ui.replacements.contains_key(vanilla))
        })
    }
    /// Makes mod `i`'s replacement for `vanilla` its open screen
    /// ([`Mods::screen`]): false when it has none.
    pub fn show_replacement(&mut self, i: usize, vanilla: &str) -> bool {
        let Some(run) = self.entries.get_mut(i).and_then(|e| e.run.as_mut()) else {
            return false;
        };
        let ui = &mut run.store.data_mut().ui;
        match ui.replacements.get(vanilla) {
            Some(screen) => {
                ui.screen = Some(screen.clone());
                true
            }
            None => false,
        }
    }
    pub fn screen(&self, i: usize) -> Option<&UiScreen> {
        self.entries
            .get(i)?
            .run
            .as_ref()?
            .store
            .data()
            .ui
            .screen
            .as_ref()
    }
    /// The widgets `ui.set-settings` pushed: mod `i`'s page in
    /// Options > Mods. Empty while it has none.
    pub fn settings_widgets(&self, i: usize) -> &[UiWidget] {
        self.entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .map_or(&[], |r| &r.store.data().ui.settings)
    }
    /// A `list` widget's row was picked on a surface of mod `i`.
    pub fn ui_select(&mut self, i: usize, id: u32, row: u32) {
        self.ui_event(i, |ui, s| {
            ui.horizon_mods_ui_events().call_select(s, id, row)
        });
    }
    /// The player left mod `i`'s screen (Esc or Done).
    pub fn ui_closed(&mut self, i: usize) {
        self.ui_event(i, |ui, s| ui.horizon_mods_ui_events().call_closed(s));
    }
    /// The player opened mod `i`'s settings page.
    pub fn ui_settings_opened(&mut self, i: usize) {
        self.ui_event(i, |ui, s| {
            ui.horizon_mods_ui_events().call_settings_opened(s)
        });
    }
    /// A `ui-events` call on mod `i`; no export, no call (mods built
    /// before `ui` have none).
    fn ui_event(
        &mut self,
        i: usize,
        f: impl FnOnce(&UiHost, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        let has_ui = self
            .entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .is_some_and(|r| r.ui.is_some());
        if has_ui {
            self.call(i, FUEL_EVENT, |run| {
                f(run.ui.as_ref().expect("checked above"), &mut run.store)
            });
        }
    }
    /// What mods asked to show since the last call.
    pub fn drain(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.out)
    }

    /// Restarts the mods whose file changed since they loaded (mod
    /// development: rebuild, and the game picks it up). True when any did.
    pub fn reload_changed(&mut self, reserved: impl Fn(&str) -> bool) -> bool {
        let mut any = false;
        for i in 0..self.entries.len() {
            let entry = &self.entries[i];
            let was_on = !matches!(entry.status, Status::Off | Status::Elsewhere);
            if modified(&entry.path) == entry.modified || !was_on {
                continue;
            }
            any = true;
            let _ = self.restart(i, &reserved);
        }
        any
    }

    /// `/hmod reload`: restarts the mods that are on (every one, or only the
    /// one with id `only`) from their files, changed or not, between two
    /// calls into them: the world goes on, the mods start from `init` again
    /// (what they keep is in `storage`). Each one's id and whether it runs
    /// again (why not).
    pub fn reload(
        &mut self,
        only: Option<&str>,
        reserved: impl Fn(&str) -> bool,
    ) -> Vec<(String, Result<(), String>)> {
        let mut done = Vec::new();
        for i in 0..self.entries.len() {
            let entry = &self.entries[i];
            let id = entry.manifest.as_ref().map(|m| m.id.clone());
            let was_on = !matches!(entry.status, Status::Off | Status::Elsewhere);
            if !was_on || only.is_some_and(|o| id.as_deref() != Some(o)) {
                continue;
            }
            let result = self.restart(i, &reserved);
            let id = self.entries[i]
                .manifest
                .as_ref()
                .map_or_else(|| id.unwrap_or_else(|| "?".into()), |m| m.id.clone());
            done.push((id, result));
        }
        done
    }

    /// Starts mod `i` again from its file: Err (why) when it doesn't run.
    fn restart(&mut self, i: usize, reserved: &impl Fn(&str) -> bool) -> Result<(), String> {
        self.generation += 1;
        self.entries[i].modified = modified(&self.entries[i].path);
        self.entries[i].run = None;
        let path = self.entries[i].path.clone();
        let started = Package::read(&path).and_then(|p| {
            self.entries[i].manifest = Some(p.manifest.clone());
            self.start(&path, &p, reserved)
        });
        let id = self.entries[i]
            .manifest
            .as_ref()
            .map_or_else(|| "?".to_owned(), |m| m.id.clone());
        // The rules the last version of this mod set are gone with it; its
        // `init` sets the ones it keeps, and they reach the game at `drain`.
        self.worldgen.remove_mod(&id);
        if let Ok(mut all) = self.block_data.lock() {
            all.locked.remove(&id);
        }
        match started {
            Ok(run) => {
                log::info!("Horizon: mod {id} reloaded");
                let entry = &mut self.entries[i];
                entry.run = run;
                entry.status = Status::Running;
                Ok(())
            }
            Err(e) => {
                log::warn!("Horizon: mod {id} did not reload: {e}");
                self.entries[i].status = Status::Failed(e.clone());
                Err(e)
            }
        }
    }
}

/// A command's arguments are well formed: names given and distinct,
/// optional ones last, free text only last.
fn check_arguments(arguments: &[Argument]) -> Result<(), String> {
    for (i, a) in arguments.iter().enumerate() {
        if a.name.is_empty()
            || !a
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("argument name '{}' isn't [A-Za-z0-9_]+", a.name));
        }
        if arguments[..i].iter().any(|b| b.name == a.name) {
            return Err(format!("two arguments are named {}", a.name));
        }
        if i > 0 && arguments[i - 1].optional && !a.optional {
            return Err(format!("{} is required after an optional argument", a.name));
        }
        if a.kind == ArgumentKind::Text && i + 1 < arguments.len() {
            return Err(format!(
                "{} takes the rest of the line, so it must be last",
                a.name
            ));
        }
    }
    Ok(())
}

fn modified(path: &Path) -> Option<SystemTime> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_dir() {
        // A folder mod changes when its component does.
        return std::fs::metadata(path.join(crate::manifest::WASM))
            .ok()?
            .modified()
            .ok();
    }
    meta.modified().ok()
}

/// A trap or error, as the player reads it.
fn reason(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => "it took too long (an endless loop?)".into(),
        Some(Trap::UnreachableCodeReached) => "it crashed (a panic)".into(),
        Some(trap) => format!("it crashed: {trap}"),
        None => format!("{e:#}"),
    }
}

fn clip(text: String) -> String {
    clip_to(text, LINE_CHARS)
}
fn clip_to(text: String, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => text[..at].to_string(),
        None => text,
    }
}

/// Widgets on a `ui` screen or settings page, and rows on one `list`.
const WIDGETS: usize = 64;
const LIST_ROWS: usize = 512;

fn clip_widgets(widgets: Vec<UiWidget>) -> Vec<UiWidget> {
    widgets
        .into_iter()
        .take(WIDGETS)
        .map(|w| match w {
            UiWidget::Text(mut t) => {
                t.text = clip(t.text);
                UiWidget::Text(t)
            }
            UiWidget::Button(mut b) => {
                b.label = clip(b.label);
                UiWidget::Button(b)
            }
            UiWidget::Item(mut i) => {
                i.item = clip(i.item);
                i.name = clip(i.name);
                UiWidget::Item(i)
            }
            UiWidget::List(mut l) => {
                l.rows = l.rows.into_iter().take(LIST_ROWS).map(clip).collect();
                l.selected = l.selected.filter(|&s| (s as usize) < l.rows.len());
                UiWidget::List(l)
            }
        })
        .collect()
}

impl mod_log::Host for State {
    fn write(&mut self, level: mod_log::Level, message: String) {
        let level = match level {
            mod_log::Level::Debug => log::Level::Debug,
            mod_log::Level::Info => log::Level::Info,
            mod_log::Level::Warn => log::Level::Warn,
            mod_log::Level::Error => log::Level::Error,
        };
        log::log!(level, "[mod {}] {}", self.id, clip(message));
    }
}
impl game::Host for State {
    fn get_player(&mut self) -> Option<Player> {
        self.snapshot.player.clone()
    }
    fn get_clock(&mut self) -> Option<Clock> {
        self.snapshot.clock
    }
    fn raycast(&mut self, distance: f32) -> Option<Hit> {
        let (at, hit) = self.snapshot.view_hit.as_ref()?;
        // NaN compares false: no hit.
        (*at <= distance.min(RAYCAST_MAX) && !distance.is_nan()).then(|| hit.clone())
    }
}
impl browser::Host for State {
    fn set_items(&mut self, items: Vec<BrowserStack>) {
        self.browser.items = items
            .into_iter()
            .take(BROWSER_ITEMS)
            .map(|s| BrowserStack {
                item: clip(s.item),
                components: clip_to(s.components, 4096),
                ..s
            })
            .collect();
        self.browser.version = BROWSER_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn set_recipes(
        &mut self,
        title: String,
        tabs: Vec<BrowserStack>,
        tab: u32,
        recipes: Vec<BrowserRecipe>,
    ) {
        let stack = |s: BrowserStack| BrowserStack {
            item: clip(s.item),
            components: clip_to(s.components, 4096),
            ..s
        };
        let cycle = |list: Vec<BrowserStack>| -> Vec<BrowserStack> {
            list.into_iter().take(SLOT_STACKS).map(stack).collect()
        };
        self.browser.recipes = (!recipes.is_empty()).then(|| {
            let recipes = recipes
                .into_iter()
                .take(BROWSER_RECIPES)
                .map(|r| BrowserRecipe {
                    width: r.width.clamp(1, 3),
                    height: r.height.clamp(1, 3),
                    slots: r.slots.into_iter().take(RECIPE_SLOTS).map(cycle).collect(),
                    results: cycle(r.results),
                    note: clip(r.note),
                    ..r
                })
                .collect();
            RecipePage {
                title: clip(title),
                tabs: tabs.into_iter().take(RECIPE_TABS).map(stack).collect(),
                tab,
                recipes,
            }
        });
        self.browser.version = BROWSER_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn poll(&mut self) -> Vec<BrowserEvent> {
        self.browser.events.drain(..).collect()
    }
}
impl recipes::Host for State {
    fn items(&mut self) -> Vec<RecipeItem> {
        self.snapshot
            .recipes
            .as_ref()
            .map_or_else(Vec::new, |r| r.items.clone())
    }
    fn all(&mut self) -> Vec<Recipe> {
        self.snapshot
            .recipes
            .as_ref()
            .map_or_else(Vec::new, |r| r.recipes.clone())
    }
    fn known(&mut self) -> Vec<i32> {
        self.snapshot.known_recipes.to_vec()
    }
    fn place(&mut self, book_id: i32, all: bool) -> Result<(), String> {
        self.recipe_action()?;
        if !self.snapshot.placing {
            return Err("no screen that takes recipes is open".into());
        }
        if !self.snapshot.known_recipes.contains(&book_id) {
            return Err(format!(
                "recipe {book_id} is not in the player's recipe book"
            ));
        }
        self.out.push(Output::PlaceRecipe { book_id, all });
        Ok(())
    }
    fn give(&mut self, stack: RecipeStack) -> Result<(), String> {
        self.recipe_action()?;
        if !self.snapshot.creative {
            return Err("only in creative mode".into());
        }
        let item = self
            .snapshot
            .recipes
            .as_ref()
            .and_then(|r| r.items.get(stack.item as usize))
            .ok_or_else(|| format!("no item {}", stack.item))?
            .id
            .clone();
        self.out.push(Output::Give {
            item,
            count: stack.count.clamp(1, 99),
            components: clip_to(stack.components, 4096),
        });
        Ok(())
    }
}
impl State {
    /// `recipes.place` and `give` take at most [`RECIPE_ACTIONS`] a call.
    fn recipe_action(&self) -> Result<(), String> {
        let done = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::PlaceRecipe { .. } | Output::Give { .. }))
            .count();
        if done < RECIPE_ACTIONS {
            Ok(())
        } else {
            Err(format!("at most {RECIPE_ACTIONS} places and gives a call"))
        }
    }
}
impl State {
    /// Queues a chat line unless the call already sent [`CHAT_PER_CALL`].
    fn say(&mut self, line: impl FnOnce(String) -> Output) -> bool {
        let shown = self
            .out
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    Output::Chat { .. } | Output::Tell { .. } | Output::Broadcast { .. }
                )
            })
            .count();
        if shown < CHAT_PER_CALL {
            let name = self.name.clone();
            self.out.push(line(name));
        }
        shown < CHAT_PER_CALL
    }
}
impl chat::Host for State {
    fn show(&mut self, text: String) {
        self.say(|name| Output::Chat {
            name,
            text: clip(text),
        });
    }
}
impl State {
    /// The block id at a position as the lent world shows it: none where no
    /// chunk is loaded (or no world is lent).
    fn block_at(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        let state = view.as_ref()?.block(dimension, x, y, z)?;
        Some(block_id(&state).to_owned())
    }
}
/// Cells a `find-blocks` or `count-blocks` box may have.
const SCAN_CELLS: usize = 65536;
/// A box's cells; Err past `max`.
fn box_cells(
    a: (i32, i32, i32),
    b: (i32, i32, i32),
    max: usize,
) -> Result<impl Iterator<Item = (i32, i32, i32)>, String> {
    let (x1, x2) = (a.0.min(b.0), a.0.max(b.0));
    let (y1, y2) = (a.1.min(b.1), a.1.max(b.1));
    let (z1, z2) = (a.2.min(b.2), a.2.max(b.2));
    let side = |lo: i32, hi: i32| (i64::from(hi) - i64::from(lo) + 1) as u128;
    if side(x1, x2) * side(y1, y2) * side(z1, z2) > max as u128 {
        return Err(format!("a box of at most {max} blocks"));
    }
    Ok((x1..=x2).flat_map(move |x| (y1..=y2).flat_map(move |y| (z1..=z2).map(move |z| (x, y, z)))))
}
fn is_air_block(id: &str) -> bool {
    matches!(
        id,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}
/// Whether a state is `wanted`: an id alone takes any properties.
fn state_is(state: &str, wanted: &str) -> bool {
    if wanted.contains('[') {
        state == wanted
    } else {
        block_id(state) == wanted
    }
}
/// `axis` of `minecraft:oak_log[axis=y]`.
fn state_property<'a>(state: &'a str, name: &str) -> Option<&'a str> {
    let (_, props) = state.split_once('[')?;
    props.trim_end_matches(']').split(',').find_map(|p| {
        p.split_once('=')
            .filter(|(k, _)| *k == name)
            .map(|(_, v)| v)
    })
}
/// The state with one property changed; Err for one it doesn't have.
fn with_property(state: &str, name: &str, value: &str) -> Result<String, String> {
    if state_property(state, name).is_none() {
        return Err(format!("{state} has no property {name}"));
    }
    let (id, props) = state.split_once('[').unwrap_or((state, ""));
    let props: Vec<String> = props
        .trim_end_matches(']')
        .split(',')
        .map(|p| match p.split_once('=') {
            Some((k, _)) if k == name => format!("{k}={value}"),
            _ => p.to_owned(),
        })
        .collect();
    Ok(format!("{id}[{}]", props.join(",")))
}
/// A text component for a command line: a quoted string.
fn quoted(text: &str) -> String {
    serde_json::to_string(&clip_to(text.replace(['\n', '\r'], " "), 256)).unwrap_or_default()
}
/// `minecraft:block.anvil.use`: nothing that could end a command's argument.
fn check_game_id(kind: &str, id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 256
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_:./-".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!("{kind} {id:?}: not an id"))
    }
}
impl mod_world::Host for State {
    fn get_data(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<Vec<u8>> {
        let now = self.block_at(&dimension, x, y, z)?;
        let key = block_key(&dimension, x, y, z);
        let mut data = self.block_data.lock().ok()?;
        let mine = data.mods.get_mut(&self.id)?;
        let (on, hex) = mine.get(&key)?;
        if *on != now {
            // The block it was set on is gone: so is the data.
            mine.remove(&key);
            return None;
        }
        Some(from_hex(hex))
    }
    fn set_data(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        data: Vec<u8>,
    ) -> Result<(), String> {
        if data.len() > BLOCK_DATA_BYTES {
            return Err(format!("{} bytes: at most {BLOCK_DATA_BYTES}", data.len()));
        }
        let now = self
            .block_at(&dimension, x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        let key = block_key(&dimension, x, y, z);
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let mine = all.mods.entry(self.id.clone()).or_default();
        if data.is_empty() {
            mine.remove(&key);
            return Ok(());
        }
        if now == "minecraft:air" {
            return Err(format!("no block at {x} {y} {z}: data needs one"));
        }
        if !mine.contains_key(&key) && mine.len() >= BLOCK_DATA_POSITIONS {
            return Err(format!("at most {BLOCK_DATA_POSITIONS} positions per mod"));
        }
        mine.insert(key, (now, to_hex(&data)));
        Ok(())
    }
    fn data_positions(&mut self, dimension: String) -> Vec<(i32, i32, i32)> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        let Some(mine) = all.mods.get(&self.id) else {
            return Vec::new();
        };
        mine.keys()
            .filter_map(|k| {
                let mut w = k.rsplitn(4, ' ');
                let z = w.next()?.parse().ok()?;
                let y = w.next()?.parse().ok()?;
                let x = w.next()?.parse().ok()?;
                (w.next()? == dimension).then_some((x, y, z))
            })
            .collect()
    }
    fn schedule_tick(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        delay: u32,
    ) -> Result<(), String> {
        State::schedule_tick(self, dimension, x, y, z, delay)
    }
    fn redstone_power(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
        State::redstone_power(self, dimension, x, y, z)
    }
    fn mod_loaded(&mut self, id: String) -> bool {
        self.snapshot.running.contains(&id)
    }
    fn mod_send(&mut self, target: String, channel: String, data: Vec<u8>) -> Result<(), String> {
        if !self.snapshot.running.contains(&target) {
            return Err(format!("mod {target} is not running"));
        }
        if data.len() > 1 << 16 {
            return Err(format!("{} bytes: at most 64 KiB", data.len()));
        }
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let waiting = all.inbox.entry(target).or_default();
        if waiting.len() >= 1024 {
            return Err("1024 messages are waiting already".to_string());
        }
        waiting.push((self.id.clone(), channel, data));
        Ok(())
    }
    fn mod_receive(&mut self) -> Vec<(String, String, Vec<u8>)> {
        self.block_data
            .lock()
            .ok()
            .and_then(|mut all| all.inbox.remove(&self.id))
            .unwrap_or_default()
    }
    fn block_has_tag(&mut self, block: String, tag: String) -> bool {
        self.world
            .lock()
            .is_ok_and(|v| v.as_ref().is_some_and(|v| v.block_has_tag(&block, &tag)))
    }
    fn entity_has_tag(&mut self, kind: String, tag: String) -> bool {
        self.world
            .lock()
            .is_ok_and(|v| v.as_ref().is_some_and(|v| v.entity_has_tag(&kind, &tag)))
    }
    fn item_has_tag(&mut self, item: String, tag: String) -> bool {
        let Ok(view) = self.world.lock() else {
            return false;
        };
        view.as_ref().is_some_and(|v| v.item_has_tag(&item, &tag))
    }
    fn get_saved(&mut self, key: String) -> Option<Vec<u8>> {
        let all = self.block_data.lock().ok()?;
        all.saved.get(&self.id)?.get(&key).map(|hex| from_hex(hex))
    }
    fn set_saved(&mut self, key: String, data: Vec<u8>) -> Result<(), String> {
        if data.len() > 1 << 20 {
            return Err(format!("{} bytes: at most 1 MiB", data.len()));
        }
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let mine = all.saved.entry(self.id.clone()).or_default();
        if data.is_empty() {
            mine.remove(&key);
        } else {
            if !mine.contains_key(&key) && mine.len() >= 64 {
                return Err("at most 64 keys per mod".to_string());
            }
            mine.insert(key, to_hex(&data));
        }
        Ok(())
    }
    fn set_locked_results(&mut self, items: Vec<String>) -> Result<(), String> {
        if items.len() > 4096 {
            return Err(format!("{} items: at most 4096", items.len()));
        }
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        all.locked.insert(self.id.clone(), items);
        Ok(())
    }
    fn set_redstone_output(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        signal: u8,
    ) -> Result<(), String> {
        self.set_output(&dimension, (x, y, z), |o| o.signal = signal, signal)
    }
    fn set_comparator_output(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        signal: u8,
    ) -> Result<(), String> {
        self.set_output(&dimension, (x, y, z), |o| o.comparator = signal, signal)
    }
    fn get_block(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.block(&dimension, x, y, z)
    }
    fn get_biome(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.biome(&dimension, x, y, z)
    }
    fn set_blocks(&mut self, dimension: String, blocks: Vec<BlockChange>) -> Result<(), String> {
        if blocks.len() > BLOCKS_PER_CALL {
            return Err(format!(
                "{} blocks: at most {BLOCKS_PER_CALL} per call",
                blocks.len()
            ));
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            if !view.has_dimension(&dimension) {
                return Err(format!("no dimension {dimension}"));
            }
            for b in &blocks {
                view.check_state(&b.state)?;
            }
        }
        if !blocks.is_empty() {
            self.out.push(Output::SetBlocks {
                name: self.name.clone(),
                dimension,
                blocks,
            });
        }
        Ok(())
    }
    fn get_blocks(
        &mut self,
        dimension: String,
        positions: Vec<(i32, i32, i32)>,
    ) -> Result<Vec<Option<String>>, String> {
        if positions.len() > BLOCKS_PER_CALL {
            return Err(format!("at most {BLOCKS_PER_CALL} positions per call"));
        }
        let view = self.world.lock().map_err(|_| "no world".to_string())?;
        let Some(view) = view.as_ref() else {
            return Ok(vec![None; positions.len()]);
        };
        Ok(positions
            .iter()
            .map(|&(x, y, z)| view.block(&dimension, x, y, z))
            .collect())
    }
    fn is_loaded(&mut self, dimension: String, x: i32, z: i32) -> bool {
        self.block_at(&dimension, x, 64, z).is_some()
    }
    fn is_air(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        self.block_at(&dimension, x, y, z)
            .is_some_and(|b| is_air_block(&b))
    }
    fn block_id(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        self.block_at(&dimension, x, y, z)
    }
    fn block_property(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        name: String,
    ) -> Option<String> {
        let state = mod_world::Host::get_block(self, dimension, x, y, z)?;
        state_property(&state, &name).map(str::to_owned)
    }
    fn set_block(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        state: String,
    ) -> Result<(), String> {
        mod_world::Host::set_blocks(
            self,
            dimension,
            vec![mod_world::BlockChange { x, y, z, state }],
        )
    }
    fn set_block_property(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        name: String,
        value: String,
    ) -> Result<(), String> {
        let state = mod_world::Host::get_block(self, dimension.clone(), x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        let state = with_property(&state, &name, &value)?;
        mod_world::Host::set_block(self, dimension, x, y, z, state)
    }
    fn break_block(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        drop: bool,
    ) -> Result<(), String> {
        if drop {
            self.check_dim(&dimension)?;
            self.console(format!(
                "execute in {dimension} run setblock {x} {y} {z} minecraft:air destroy"
            ))
        } else {
            mod_world::Host::set_block(self, dimension, x, y, z, "minecraft:air".into())
        }
    }
    fn fill(
        &mut self,
        dimension: String,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        state: String,
    ) -> Result<(), String> {
        let cells = box_cells((x1, y1, z1), (x2, y2, z2), BLOCKS_PER_CALL)?;
        let blocks = cells
            .map(|(x, y, z)| mod_world::BlockChange {
                x,
                y,
                z,
                state: state.clone(),
            })
            .collect();
        mod_world::Host::set_blocks(self, dimension, blocks)
    }
    fn find_blocks(
        &mut self,
        dimension: String,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        block: String,
        limit: u32,
    ) -> Result<Vec<(i32, i32, i32)>, String> {
        let cells = box_cells((x1, y1, z1), (x2, y2, z2), SCAN_CELLS)?;
        let view = self.world.lock().map_err(|_| "no world".to_string())?;
        let Some(view) = view.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(cells
            .filter(|&(x, y, z)| {
                view.block(&dimension, x, y, z)
                    .is_some_and(|s| state_is(&s, &block))
            })
            .take(limit.min(BLOCKS_PER_CALL as u32) as usize)
            .collect())
    }
    fn count_blocks(
        &mut self,
        dimension: String,
        x1: i32,
        y1: i32,
        z1: i32,
        x2: i32,
        y2: i32,
        z2: i32,
        block: String,
    ) -> Result<u32, String> {
        let cells = box_cells((x1, y1, z1), (x2, y2, z2), SCAN_CELLS)?;
        let view = self.world.lock().map_err(|_| "no world".to_string())?;
        let Some(view) = view.as_ref() else {
            return Ok(0);
        };
        Ok(cells
            .filter(|&(x, y, z)| {
                view.block(&dimension, x, y, z)
                    .is_some_and(|s| state_is(&s, &block))
            })
            .count() as u32)
    }
    fn height_at(&mut self, dimension: String, x: i32, z: i32) -> Option<i32> {
        let view = self.world.lock().ok()?;
        let view = view.as_ref()?;
        (-64..320).rev().find(|&y| {
            view.block(&dimension, x, y, z)
                .is_some_and(|s| !is_air_block(block_id(&s)))
        })
    }
    fn game_time(&mut self) -> Option<u64> {
        self.snapshot.clock.as_ref().map(|c| c.game_time)
    }
    fn day_time(&mut self) -> Option<u64> {
        self.snapshot.clock.as_ref().map(|c| c.day_time)
    }
    fn set_time(&mut self, ticks: u32) -> Result<(), String> {
        self.console(format!("time set {ticks}"))
    }
    fn set_weather(&mut self, weather: String, seconds: u32) -> Result<(), String> {
        if !matches!(weather.as_str(), "clear" | "rain" | "thunder") || seconds == 0 {
            return Err("clear, rain or thunder, for at least a second".into());
        }
        self.console(format!("weather {weather} {seconds}s"))
    }
    fn set_gamerule(&mut self, rule: String, value: String) -> Result<(), String> {
        let word = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_:.-".contains(c))
        };
        if !word(&rule) || !word(&value) {
            return Err("a rule's name and one value".into());
        }
        self.console(format!("gamerule {rule} {value}"))
    }
    fn set_difficulty(&mut self, difficulty: String) -> Result<(), String> {
        if !matches!(difficulty.as_str(), "peaceful" | "easy" | "normal" | "hard") {
            return Err("peaceful, easy, normal or hard".into());
        }
        self.console(format!("difficulty {difficulty}"))
    }
    fn set_world_spawn(&mut self, x: i32, y: i32, z: i32) -> Result<(), String> {
        self.console(format!("setworldspawn {x} {y} {z}"))
    }
    fn lightning(&mut self, dimension: String, x: f64, y: f64, z: f64) -> Result<(), String> {
        self.check_dim(&dimension)?;
        if ![x, y, z].iter().all(|v| v.is_finite()) {
            return Err("a finite position".into());
        }
        self.console(format!(
            "execute in {dimension} run summon minecraft:lightning_bolt {x} {y} {z}"
        ))
    }
    fn explode(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        power: f32,
    ) -> Result<(), String> {
        self.check_dim(&dimension)?;
        if ![x, y, z].iter().all(|v| v.is_finite()) || !(0.0..=16.0).contains(&power) {
            return Err("a finite position, and a power of 0 to 16".into());
        }
        self.act(ServerAction::Explode {
            dimension,
            pos: (x, y, z),
            power,
        })
    }
    fn has_data(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        mod_world::Host::get_data(self, dimension, x, y, z).is_some()
    }
    fn saved_keys(&mut self) -> Vec<String> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        all.saved
            .get(&self.id)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }
    fn running_mods(&mut self) -> Vec<String> {
        self.snapshot.running.clone()
    }
    fn is_valid_state(&mut self, state: String) -> bool {
        let Ok(view) = self.world.lock() else {
            return false;
        };
        view.as_ref().is_some_and(|v| v.check_state(&state).is_ok())
    }
    fn item_exists(&mut self, item: String) -> bool {
        mod_world::Host::max_stack_size(self, item).is_some()
    }
    fn max_stack_size(&mut self, item: String) -> Option<u32> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.max_stack(&item)
    }
    fn random_u32(&mut self) -> u32 {
        use std::hash::{BuildHasher, Hasher};
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish() as u32
    }
}
impl worldgen::Host for State {
    fn set_surface_rule(&mut self, biome: String, surface: String) -> Result<(), String> {
        if biome.is_empty() {
            return Err("biome must not be empty".into());
        }
        if surface.is_empty() {
            return Err("surface must not be empty".into());
        }
        self.out.push(Output::Server(ServerAction::Worldgen {
            id: self.id.clone(),
            biome,
            surface,
        }));
        Ok(())
    }
    fn add_ore(&mut self, ore: worldgen::Ore) -> Result<(), String> {
        if !(1..=64).contains(&ore.size) || !(1..=64).contains(&ore.per_chunk) {
            return Err("size and per-chunk: 1 to 64".into());
        }
        if ore.min_y > ore.max_y || ore.block.is_empty() || ore.replaces.is_empty() {
            return Err("an ore needs a block, what it replaces and min-y <= max-y".into());
        }
        self.out.push(Output::Server(ServerAction::Ore {
            id: self.id.clone(),
            ore: crate::OreDefinition {
                biome: ore.biome,
                block: ore.block,
                replaces: ore.replaces,
                min_y: ore.min_y,
                max_y: ore.max_y,
                size: ore.size,
                per_chunk: ore.per_chunk,
            },
        }));
        Ok(())
    }
}

impl players::Host for State {
    fn online(&mut self) -> Vec<ServerPlayer> {
        self.snapshot.players.clone()
    }
    fn tell(&mut self, player: String, text: String) -> bool {
        self.snapshot.players.iter().any(|p| p.id == player)
            && self.say(|name| Output::Tell {
                name,
                player,
                text: clip(text),
            })
    }
    fn broadcast(&mut self, text: String) {
        self.say(|name| Output::Broadcast {
            name,
            text: clip(text),
        });
    }
    fn inventory(&mut self, player: String) -> Vec<Option<players::ItemStack>> {
        self.player_inventory(player)
    }
    fn get_data(&mut self, player: String, key: String) -> Option<Vec<u8>> {
        let all = self.block_data.lock().ok()?;
        let hex = all.player_data.get(&player)?.get(&self.id)?.get(&key)?;
        Some(from_hex(hex))
    }
    fn set_data(&mut self, player: String, key: String, data: Vec<u8>) -> Result<(), String> {
        if data.len() > 1 << 16 {
            return Err(format!("{} bytes: at most 65536", data.len()));
        }
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let mine = all
            .player_data
            .entry(player)
            .or_default()
            .entry(self.id.clone())
            .or_default();
        if data.is_empty() {
            mine.remove(&key);
        } else {
            if !mine.contains_key(&key) && mine.len() >= 64 {
                return Err("at most 64 keys per mod and player".to_string());
            }
            mine.insert(key, to_hex(&data));
        }
        Ok(())
    }
    fn style_stack(
        &mut self,
        stack: players::ItemStack,
        look: players::ItemLook,
    ) -> Result<players::ItemStack, String> {
        let host = server_api::from_guest(stack, &self.id)?;
        let look = server_api::StackLook {
            name: look.name,
            lore: look.lore,
            model: look.model,
            glint: look.glint,
        };
        let view = self.world.lock().map_err(|_| "no world".to_string())?;
        let view = view.as_ref().ok_or("no world lent")?;
        Ok(server_api::to_guest(
            &view.style_stack(&host, &look)?,
            &self.id,
        ))
    }
    fn set_undying(&mut self, player: String, on: bool) -> Result<(), String> {
        self.act(ServerAction::Undying { player, on })
    }
    fn poll_events(&mut self) -> Vec<players::PlayerEvent> {
        let events = self.snapshot.player_events.iter();
        events
            .map(|(player, kind, detail)| players::PlayerEvent {
                player: player.clone(),
                kind: kind.clone(),
                detail: detail.clone(),
            })
            .collect()
    }
    fn look(&mut self, player: String) -> Option<(f32, f32)> {
        let found = self.snapshot.looks.iter().find(|l| l.0 == player)?;
        Some((found.1, found.2))
    }
    fn sneaking(&mut self, player: String) -> bool {
        self.snapshot.sneaking.contains(&player)
    }
    fn selected_slot(&mut self, player: String) -> Option<u32> {
        self.player_selected(player)
    }
    fn give(&mut self, player: String, stack: players::ItemStack) -> Result<(), String> {
        self.player_give(player, stack)
    }
    fn set_slot(
        &mut self,
        player: String,
        slot: u32,
        stack: Option<players::ItemStack>,
    ) -> Result<(), String> {
        self.player_set_slot(player, slot, stack)
    }
    fn take(&mut self, player: String, item: String, count: u32) -> u32 {
        self.player_take(player, item, count)
    }
    fn by_name(&mut self, name: String) -> Option<ServerPlayer> {
        self.snapshot
            .players
            .iter()
            .find(|p| p.name == name)
            .cloned()
    }
    fn by_id(&mut self, id: String) -> Option<ServerPlayer> {
        self.snapshot.players.iter().find(|p| p.id == id).cloned()
    }
    fn is_online(&mut self, id: String) -> bool {
        self.snapshot.players.iter().any(|p| p.id == id)
    }
    fn count(&mut self) -> u32 {
        self.snapshot.players.len() as u32
    }
    fn near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<ServerPlayer> {
        let d2 = |p: &ServerPlayer| (p.x - x).powi(2) + (p.y - y).powi(2) + (p.z - z).powi(2);
        let mut near: Vec<ServerPlayer> = self
            .snapshot
            .players
            .iter()
            .filter(|p| p.dimension == dimension && d2(p) <= radius * radius)
            .cloned()
            .collect();
        near.sort_by(|a, b| d2(a).total_cmp(&d2(b)));
        near
    }
    fn held_item(&mut self, player: String) -> Option<players::ItemStack> {
        let slot = self.player_selected(player.clone())?;
        self.player_inventory(player).get(slot as usize)?.clone()
    }
    fn offhand_item(&mut self, player: String) -> Option<players::ItemStack> {
        self.player_inventory(player).get(40)?.clone()
    }
    fn armor(&mut self, player: String) -> Vec<Option<players::ItemStack>> {
        self.player_inventory(player)
            .get(36..40)
            .map(<[_]>::to_vec)
            .unwrap_or_default()
    }
    fn count_item(&mut self, player: String, item: String) -> u32 {
        let slots = self.player_inventory(player);
        slots
            .iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum()
    }
    fn has_item(&mut self, player: String, item: String, count: u32) -> bool {
        players::Host::count_item(self, player, item) >= count
    }
    fn free_slots(&mut self, player: String) -> u32 {
        let slots = self.player_inventory(player);
        slots.iter().take(36).filter(|s| s.is_none()).count() as u32
    }
    fn find_item(&mut self, player: String, item: String) -> Option<u32> {
        let slots = self.player_inventory(player);
        slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.item == item))
            .map(|i| i as u32)
    }
    fn set_game_mode(&mut self, player: String, mode: players::GameMode) -> Result<(), String> {
        let player = self.online_player(&player)?;
        let mode = match mode {
            players::GameMode::Survival => "survival",
            players::GameMode::Creative => "creative",
            players::GameMode::Adventure => "adventure",
            players::GameMode::Spectator => "spectator",
        };
        self.console(format!("gamemode {mode} {}", player.name))
    }
    fn clear_inventory(&mut self, player: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("clear {}", player.name))
    }
    fn show_title(
        &mut self,
        player: String,
        title: String,
        subtitle: Option<String>,
    ) -> Result<(), String> {
        let player = self.online_player(&player)?;
        if let Some(subtitle) = subtitle {
            self.console(format!(
                "title {} subtitle {}",
                player.name,
                quoted(&subtitle)
            ))?;
        }
        self.console(format!("title {} title {}", player.name, quoted(&title)))
    }
    fn action_bar(&mut self, player: String, text: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("title {} actionbar {}", player.name, quoted(&text)))
    }
    fn set_title_times(
        &mut self,
        player: String,
        fade_in: u32,
        stay: u32,
        fade_out: u32,
    ) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!(
            "title {} times {fade_in} {stay} {fade_out}",
            player.name
        ))
    }
    fn add_experience(&mut self, player: String, points: i32) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("experience add {} {points} points", player.name))
    }
    fn add_levels(&mut self, player: String, levels: i32) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("experience add {} {levels} levels", player.name))
    }
    fn set_spawn(
        &mut self,
        player: String,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.check_dim(&dimension)?;
        self.console(format!(
            "execute in {dimension} run spawnpoint {} {x} {y} {z}",
            player.name
        ))
    }
    fn kick(&mut self, player: String, reason: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("kick {} {}", player.name, clip_to(reason, 256)))
    }
    fn play_sound(
        &mut self,
        player: String,
        sound: String,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        let player = self.online_player(&player)?;
        check_game_id("sound", &sound)?;
        if !volume.is_finite() || !pitch.is_finite() {
            return Err("a finite volume and pitch".into());
        }
        let (name, x, y, z) = (player.name, player.x, player.y, player.z);
        self.console(format!(
            "playsound {sound} master {name} {x} {y} {z} {volume} {pitch}"
        ))
    }
    fn stop_sounds(&mut self, player: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        self.console(format!("stopsound {}", player.name))
    }
    fn grant_advancement(&mut self, player: String, advancement: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        check_game_id("advancement", &advancement)?;
        self.console(format!(
            "advancement grant {} only {advancement}",
            player.name
        ))
    }
    fn revoke_advancement(&mut self, player: String, advancement: String) -> Result<(), String> {
        let player = self.online_player(&player)?;
        check_game_id("advancement", &advancement)?;
        self.console(format!(
            "advancement revoke {} only {advancement}",
            player.name
        ))
    }
}
/// A key a mod registered (capability `input`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModKey {
    /// The mod's own name for it, echoed in `key-down`.
    pub name: String,
    /// What it does, for Controls.
    pub label: String,
    /// `key.keyboard.r`.
    pub key: String,
}

/// What capability `input` holds for one mod.
#[derive(Default)]
struct InputState {
    claimed: Vec<String>,
    keys: Vec<ModKey>,
    events: std::collections::VecDeque<InputEvent>,
    /// The mod's keys held now, by name.
    down: Vec<String>,
    /// It holds the mouse wheel.
    wheel: bool,
    /// The keys the player chose, `name → key`: they win over
    /// `register-key`'s.
    bindings: Vec<(String, String)>,
}
/// Events kept for `poll`, items claimed and keys, per mod.
const INPUT_EVENTS: usize = 256;
const CLAIMS: usize = 64;
const KEYS: usize = 16;

/// `key.keyboard.<name>` or `key.mouse.<name>`, as options.txt writes keys;
/// the game knows which names exist and ignores a key it has none for.
fn key_ok(key: &str) -> bool {
    let rest = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse."));
    rest.is_some_and(|r| {
        (1..=32).contains(&r.len())
            && r.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.')
    })
}

impl InputState {
    /// Key `name` is `key` now; held, it lets go first.
    fn rebind(&mut self, name: &str, key: String) {
        let Some(k) = self.keys.iter_mut().find(|k| k.name == name) else {
            return;
        };
        k.key = key;
        if let Some(i) = self.down.iter().position(|n| n == name) {
            let name = self.down.swap_remove(i);
            self.push(InputEvent::KeyUp(name));
        }
    }
    fn push(&mut self, event: InputEvent) {
        if self.events.len() == INPUT_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

impl input::Host for State {
    fn claim_items(&mut self, ids: Vec<String>) {
        self.input.claimed = ids.into_iter().take(CLAIMS).map(clip).collect();
    }
    fn register_key(&mut self, name: String, label: String, key: String) -> Result<(), String> {
        if !key_ok(&key) {
            return Err(format!(
                "{key:?} is not a key as options.txt writes them (key.keyboard.r)"
            ));
        }
        let (name, label) = (clip(name), clip(label));
        let key = match self.input.bindings.iter().find(|(n, _)| *n == name) {
            Some((_, chosen)) => chosen.clone(),
            None => key,
        };
        if let Some(k) = self.input.keys.iter_mut().find(|k| k.name == name) {
            (k.label, k.key) = (label, key);
            return Ok(());
        }
        if self.input.keys.len() == KEYS {
            return Err(format!("a mod may register {KEYS} keys"));
        }
        self.input.keys.push(ModKey { name, label, key });
        Ok(())
    }
    fn held(&mut self) -> Option<HeldItem> {
        self.snapshot.held.clone()
    }
    fn poll(&mut self) -> Vec<InputEvent> {
        self.input.events.drain(..).collect()
    }
    fn mapping(&mut self, name: String) -> Option<input::MappingState> {
        let (_, label, down) = self.snapshot.mappings.iter().find(|m| m.0 == name)?;
        Some(input::MappingState {
            label: label.clone(),
            down: *down,
        })
    }
    fn clicks_per_second(&mut self) -> (u32, u32) {
        self.snapshot.cps
    }
    fn claim_wheel(&mut self, on: bool) {
        self.input.wheel = on;
    }
}

/// HUD shapes one mod may keep (capability `render`).
const SHAPES: usize = 1024;
/// The longest jolt `render.shake` makes, in seconds.
const SHAKE_SECONDS: f32 = 10.0;
/// `render.set-zoom`'s limits: `Camera.tickFov` keeps the modifier at 0.1
/// or more, and turning never stops.
const ZOOM_MAX: f32 = 10.0;
const TURN_MIN: f32 = 0.05;
/// `set-minimap`'s side in GUI pixels and its blocks per map pixel.
const MINIMAP_SIZE: (f32, f32) = (16.0, 256.0);
const MINIMAP_SCALE: (f32, f32) = (0.5, 4.0);
/// `set-far-terrain`'s distance in chunks, as the LOD Distance setting's.
const FAR_TERRAIN_DISTANCE: (u32, u32) = (64, 1024);

/// A mod's corner panel: shapes in a `width` × `height` box the game
/// places in `corner`, stacked with the other overlays.
#[derive(Clone, Debug)]
pub struct CornerPanel {
    pub corner: RenderCorner,
    pub width: f32,
    pub height: f32,
    pub shapes: Vec<Shape>,
}

fn clip_shapes(shapes: Vec<Shape>) -> Vec<Shape> {
    shapes
        .into_iter()
        .take(SHAPES)
        .map(|shape| match shape {
            Shape::Sprite(mut s) => {
                s.sprite = clip(s.sprite);
                Shape::Sprite(s)
            }
            Shape::Item(mut i) => {
                i.item = clip(i.item);
                i.scale = if i.scale.is_finite() {
                    i.scale.clamp(0.0, 16.0)
                } else {
                    1.0
                };
                Shape::Item(i)
            }
            Shape::Text(mut t) => {
                t.text = clip(t.text);
                t.color &= 0xFF_FFFF;
                Shape::Text(t)
            }
            other => other,
        })
        .collect()
}

impl render::Host for State {
    fn open_canvas(
        &mut self,
        title: String,
        shapes: Vec<Shape>,
        mut areas: Vec<render::CanvasArea>,
    ) {
        areas.truncate(1024);
        self.ui.screen = Some(UiScreen {
            title: clip(title),
            widgets: Vec::new(),
        });
        self.ui.canvas = Some((clip_shapes(shapes), areas));
        self.ui.open = true;
    }
    fn set_menu_layouts(&mut self, mut layouts: Vec<render::MenuLayout>) {
        layouts.truncate(64);
        for layout in &mut layouts {
            layout.shapes = clip_shapes(std::mem::take(&mut layout.shapes));
        }
        self.menu_layouts = layouts;
    }
    fn set_models(&mut self, mut models: Vec<render::PlacedModel>) {
        models.truncate(1024);
        self.models = models;
    }
    fn set_hud(&mut self, shapes: Vec<Shape>) {
        self.shapes = clip_shapes(shapes);
    }
    fn set_corner(&mut self, corner: RenderCorner, width: f32, height: f32, shapes: Vec<Shape>) {
        let size = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1024.0)
            } else {
                0.0
            }
        };
        self.corner = Some(CornerPanel {
            corner,
            width: size(width),
            height: size(height),
            shapes: clip_shapes(shapes),
        });
    }
    fn clear_corner(&mut self) {
        self.corner = None;
    }
    fn text_width(&mut self, text: String) -> f32 {
        let widths = &self.snapshot.font_widths;
        text.chars()
            .map(|c| widths.get(c as usize).copied().unwrap_or(6.0))
            .sum::<f32>()
            .ceil()
    }
    fn set_zoom(&mut self, zoom: ModZoom) {
        let finite = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        let factor = finite(zoom.factor, 1.0, ZOOM_MAX);
        self.zoom = (factor > 1.0).then(|| ModZoom {
            factor,
            smooth: zoom.smooth,
            turn: if zoom.turn.is_finite() {
                zoom.turn.clamp(TURN_MIN, 1.0)
            } else {
                1.0
            },
        });
    }
    fn set_minimap(&mut self, minimap: Option<ModMinimap>) {
        let finite = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        self.minimap = minimap.map(|m| ModMinimap {
            size: finite(m.size, MINIMAP_SIZE.0, MINIMAP_SIZE.1),
            blocks_per_pixel: finite(m.blocks_per_pixel, MINIMAP_SCALE.0, MINIMAP_SCALE.1),
            ..m
        });
    }
    fn set_far_terrain(&mut self, far_terrain: Option<ModFarTerrain>) {
        self.far_terrain = far_terrain.map(|f| ModFarTerrain {
            distance: f
                .distance
                .clamp(FAR_TERRAIN_DISTANCE.0, FAR_TERRAIN_DISTANCE.1),
            ..f
        });
    }
    fn frame_stats(&mut self) -> FrameRate {
        let (fps, min, low) = self.snapshot.frame;
        FrameRate { fps, min, low }
    }
    fn gui_size(&mut self) -> (f32, f32) {
        self.snapshot.gui_size
    }
    fn shake(&mut self, strength: f32, seconds: f32) {
        let ok = |v: f32, max: f32| {
            if v.is_finite() {
                v.clamp(0.0, max)
            } else {
                0.0
            }
        };
        let (strength, seconds) = (ok(strength, 1.0), ok(seconds, SHAKE_SECONDS));
        if strength > 0.0 && seconds > 0.0 {
            self.out.push(Output::Shake { strength, seconds });
        }
    }
}

impl command::Host for State {
    fn run(&mut self, line: String) {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Command { .. }))
            .count();
        if queued < CHAT_PER_CALL {
            self.out.push(Output::Command { line: clip(line) });
        }
    }
    fn run_quiet(&mut self, lines: Vec<String>) {
        let queued: usize = self
            .out
            .iter()
            .map(|o| match o {
                Output::QuietCommands { lines, .. } => lines.len(),
                _ => 0,
            })
            .sum();
        let room = QUIET_PER_CALL.saturating_sub(queued);
        if room > 0 && !lines.is_empty() {
            self.out.push(Output::QuietCommands {
                mod_id: self.id.clone(),
                lines: lines.into_iter().take(room).map(clip).collect(),
            });
        }
    }
}
impl hud::Host for State {
    fn set_panel(&mut self, corner: Corner, lines: Vec<Line>) {
        let lines = lines
            .into_iter()
            .take(HUD_LINES)
            .map(|l| Line {
                text: clip(l.text),
                color: l.color & 0xFF_FFFF,
            })
            .collect();
        self.panel = Some(Panel { corner, lines });
    }
    fn clear(&mut self) {
        self.panel = None;
    }
}
impl ui::Host for State {
    fn open_screen(&mut self, mut screen: ui::Screen) {
        screen.title = clip(screen.title);
        screen.widgets = clip_widgets(screen.widgets);
        self.ui.screen = Some(screen);
        self.ui.canvas = None;
        self.ui.open = true;
    }
    fn close_screen(&mut self) {
        self.ui.screen = None;
        self.ui.close = true;
    }
    fn set_settings(&mut self, widgets: Vec<ui::Widget>) {
        self.ui.settings = clip_widgets(widgets);
    }
    fn replace_screen(
        &mut self,
        vanilla: String,
        replacement: Option<ui::Screen>,
    ) -> Result<(), String> {
        if !REPLACEABLE_SCREENS.contains(&vanilla.as_str()) {
            return Err(format!(
                "{vanilla} can't be replaced (this Horizon replaces {})",
                REPLACEABLE_SCREENS.join(", ")
            ));
        }
        match replacement {
            Some(mut screen) => {
                screen.title = clip(screen.title);
                screen.widgets = clip_widgets(screen.widgets);
                self.ui.replacements.insert(vanilla, screen);
            }
            None => {
                self.ui.replacements.remove(&vanilla);
            }
        }
        Ok(())
    }
}

/// The vanilla screens `ui.replace-screen` can put a mod's screen in place of.
pub const REPLACEABLE_SCREENS: [&str; 1] = ["minecraft:inventory"];
impl storage::Host for State {
    fn get(&mut self, key: String) -> Option<Vec<u8>> {
        storage_key_ok(&key)
            .then(|| std::fs::read(self.data_dir.join(&key)).ok())
            .flatten()
    }
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), String> {
        if !storage_key_ok(&key) {
            return Err(format!("{key:?} is not [a-z0-9_.-]{{1,64}}"));
        }
        let len = value.len() as u64;
        if len > self.storage_budget {
            return Err("too much storage traffic in one call".into());
        }
        // Everything already on disk but the file this call replaces.
        let existing: Vec<std::fs::DirEntry> = std::fs::read_dir(&self.data_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy() != key)
            .collect();
        if existing.len() >= STORAGE_MAX_KEYS {
            return Err(format!("would hold over {STORAGE_MAX_KEYS} keys"));
        }
        let used: u64 = existing
            .iter()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        if used + len > STORAGE_QUOTA {
            return Err(format!(
                "would use over {} MiB of storage",
                STORAGE_QUOTA >> 20
            ));
        }
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(self.data_dir.join(&key), value).map_err(|e| e.to_string())?;
        self.storage_budget -= len;
        Ok(())
    }
    fn config(&mut self, key: String) -> Option<String> {
        self.config.get(&key).cloned()
    }
    fn read_resource(&mut self, path: String) -> Option<Vec<u8>> {
        crate::content::check_path(&path).ok()?;
        let (package, manifest) = &self.package;
        let content = crate::Content::read(package, manifest).ok()?;
        content.reader().ok()?.read(&path).ok()
    }
    fn delete(&mut self, key: String) {
        if storage_key_ok(&key) {
            let _ = std::fs::remove_file(self.data_dir.join(&key));
        }
    }
}

/// A storage key: one path segment of `[a-z0-9_.-]{1,64}`, not `.` or `..`.
/// No `/`, so a key can never leave the mod's own directory.
fn storage_key_ok(key: &str) -> bool {
    let chars_ok = (1..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
    chars_ok && key != "." && key != ".."
}

#[cfg(test)]
mod level_18_tests {
    use super::*;

    #[test]
    fn state_strings_and_boxes() {
        let log = "minecraft:oak_log[axis=y,waterlogged=false]";
        assert_eq!(state_property(log, "axis"), Some("y"));
        assert_eq!(state_property("minecraft:stone", "axis"), None);
        assert_eq!(
            with_property(log, "axis", "x").unwrap(),
            "minecraft:oak_log[axis=x,waterlogged=false]"
        );
        assert!(with_property("minecraft:stone", "axis", "x").is_err());
        assert!(state_is(log, "minecraft:oak_log") && state_is(log, log));
        assert!(!state_is(
            log,
            "minecraft:oak_log[axis=x,waterlogged=false]"
        ));
        assert_eq!(box_cells((1, 0, 0), (0, 1, 1), 8).unwrap().count(), 8);
        assert!(box_cells((0, 0, 0), (i32::MAX, i32::MAX, i32::MAX), 8).is_err());
        assert_eq!(quoted("a\"b\nc"), "\"a\\\"b c\"");
        assert!(check_game_id("sound", "a b").is_err());
    }
}
