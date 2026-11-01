//! Write Horizon mods in Rust (`docs/mods.md`).
//!
//! A mod is a type implementing [`Mod`], exported with [`export_mod!`] from
//! a `cdylib` crate built for `wasm32-unknown-unknown`;
//! `cargo horizon-mod build` turns it into the component the game loads.
//!
//! ```ignore
//! use horizon_mod_sdk::{Mod, Registration, chat};
//!
//! struct Hello;
//! impl Mod for Hello {
//!     fn init() -> Result<(Self, Registration), String> {
//!         Ok((Hello, Registration::new().command("hello")))
//!     }
//!     fn command(&mut self, _name: &str, _args: &str) -> Result<Option<String>, String> {
//!         chat::show("Hello from a mod!");
//!         Ok(None)
//!     }
//! }
//! horizon_mod_sdk::export_mod!(Hello);
//! ```
//!
//! Every interface but `log` is a capability: list the ones the mod uses in
//! `horizon-mod.toml` (`game-read`, `chat`, `hud`, `storage`, `command`, `ui`;
//! a server mod `storage` and `players`). Server mods: [`server`].
//!
//! Widgets (capability `ui`): a mod shows a screen it owns with
//! [`ui::open_screen`] and a page in Options > Mods with
//! [`ui::set_settings`]; what the player does with them comes back as
//! [`Mod::ui_select`], [`Mod::ui_closed`] and [`Mod::ui_settings_opened`].

wit_bindgen::generate!({
    path: "wit",
    world: "horizon-ui-mod",
    pub_export_macro: true,
    export_macro_name: "__export_horizon_mod",
    default_bindings_module: "horizon_mod_sdk",
});

pub use exports::horizon::mods::client_net_events::Guest as __ClientNetGuest;
pub use exports::horizon::mods::events::{Guest as __Guest, Registration};
pub use exports::horizon::mods::ui_events::Guest as __UiEventsGuest;
pub use horizon::mods::{
    browser, chat, client_net, command, game, hud, input, log, recipes, render, storage, ui,
};

/// The game's calls into a mod. Only the hooks [`Registration`] asks for
/// are called; the others may stay as their defaults.
pub trait Mod: Sized + 'static {
    /// Once, after loading: the mod's state and the hooks it wants. An error
    /// keeps the mod off and is shown to the player.
    fn init() -> Result<(Self, Registration), String>;
    /// Every game tick, when [`Registration::tick`] is set.
    fn tick(&mut self, _tick: u64) {}
    /// Every rendered frame, when [`Registration::frame`] is set.
    fn frame(&mut self, _seconds: f32) {}
    /// `/name args` for a command the mod registered. `Ok(Some(text))` is
    /// shown as feedback, `Err(text)` in red.
    fn command(&mut self, _name: &str, _args: &str) -> Result<Option<String>, String> {
        Err("This command does nothing".into())
    }
    /// A chat line the player received, when [`Registration::chat`] is set.
    fn chat(&mut self, _text: &str) {}
    /// A row of one of the mod's `list` widgets was picked (capability
    /// `ui`): the widget's `id` and the row.
    fn ui_select(&mut self, _id: u32, _row: u32) {}
    /// The player left the mod's screen (Esc or Done; capability `ui`).
    fn ui_closed(&mut self) {}
    /// The player opened the mod's settings page (capability `ui`): call
    /// [`ui::set_settings`] for fresh widgets.
    fn ui_settings_opened(&mut self) {}
    /// A message from the server's mods on `channel` (capability `net`).
    fn net_received(&mut self, _channel: &str, _data: &[u8]) {}
}

impl Registration {
    /// No commands and no hooks.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            tick: false,
            frame: false,
            chat: false,
        }
    }
    /// Adds `/name`.
    pub fn command(mut self, name: &str) -> Self {
        self.commands.push(name.into());
        self
    }
    pub fn on_tick(mut self) -> Self {
        self.tick = true;
        self
    }
    pub fn on_frame(mut self) -> Self {
        self.frame = true;
        self
    }
    pub fn on_chat(mut self) -> Self {
        self.chat = true;
        self
    }
}
impl Default for Registration {
    fn default() -> Self {
        Self::new()
    }
}

/// `log::write` at info level.
pub fn info(message: &str) {
    log::write(log::Level::Info, message);
}
/// `log::write` at warn level.
pub fn warn(message: &str) {
    log::write(log::Level::Warn, message);
}

/// Exports a [`Mod`] as the component's `events` interface. The state lives
/// in a thread local: a mod instance is single-threaded.
#[macro_export]
macro_rules! export_mod {
    ($t:ty) => {
        const _: () = {
            use ::std::cell::RefCell;
            ::std::thread_local! {
                static STATE: RefCell<Option<$t>> = const { RefCell::new(None) };
            }
            fn with(f: impl FnOnce(&mut $t)) {
                STATE.with(|s| {
                    if let Some(m) = s.borrow_mut().as_mut() {
                        f(m)
                    }
                })
            }
            struct Export;
            impl $crate::__Guest for Export {
                fn init() -> Result<$crate::Registration, String> {
                    let (m, registration) = <$t as $crate::Mod>::init()?;
                    STATE.with(|s| *s.borrow_mut() = Some(m));
                    Ok(registration)
                }
                fn tick(tick: u64) {
                    with(|m| $crate::Mod::tick(m, tick))
                }
                fn frame(seconds: f32) {
                    with(|m| $crate::Mod::frame(m, seconds))
                }
                fn command(name: String, args: String) -> Result<Option<String>, String> {
                    let mut out = Err("The mod is not running".to_string());
                    with(|m| out = $crate::Mod::command(m, &name, &args));
                    out
                }
                fn chat(text: String) {
                    with(|m| $crate::Mod::chat(m, &text))
                }
            }
            impl $crate::__UiEventsGuest for Export {
                fn select(id: u32, row: u32) {
                    with(|m| $crate::Mod::ui_select(m, id, row))
                }
                fn closed() {
                    with(|m| $crate::Mod::ui_closed(m))
                }
                fn settings_opened() {
                    with(|m| $crate::Mod::ui_settings_opened(m))
                }
            }
            impl $crate::__ClientNetGuest for Export {
                fn received(channel: String, data: Vec<u8>) {
                    with(|m| $crate::Mod::net_received(m, &channel, &data))
                }
            }
            // Only WebAssembly has the component's exports: a host build (cargo test or
            // clippy on the workspace) checks the mod without them, which macOS
            // couldn't link (its exported-symbols list cuts names at the `#`).
            #[cfg(target_arch = "wasm32")]
            $crate::__export_horizon_mod!(Export);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = (
                <Export as $crate::__Guest>::init,
                <Export as $crate::__UiEventsGuest>::select,
            );
        };
    };
}

/// Server mods (manifest `side = "server"`, `api = "0.3"`, experimental): a
/// type implementing [`ServerMod`](server::ServerMod), exported with
/// [`export_server_mod!`]. It runs where the world is simulated, and reads
/// and messages the players with capability `players`.
pub mod server {
    wit_bindgen::generate!({
        path: "wit",
        world: "horizon-server-block-mod",
        pub_export_macro: true,
        export_macro_name: "__export_horizon_server_mod",
        default_bindings_module: "horizon_mod_sdk::server",
        with: {
            "horizon:mods/log@0.1.0": crate::log,
            "horizon:mods/storage@0.1.0": crate::storage,
        },
    });

    pub use exports::horizon::mods::before_events::Guest as __BeforeGuest;
    pub use exports::horizon::mods::block_events::Guest as __BlockGuest;
    pub use exports::horizon::mods::entity_events::Guest as __EntityGuest;
    pub use exports::horizon::mods::game_events::Guest as __EventsGuest;
    pub use exports::horizon::mods::logic_events::{ChunkFill, Guest as __LogicGuest};
    pub use exports::horizon::mods::server_events::{
        Argument, ArgumentKind, ArgumentValue, BlockPos, Guest as __Guest, Registration,
        ServerCommand,
    };
    pub use exports::horizon::mods::server_net_events::Guest as __ServerNetGuest;
    pub use horizon::mods::entities::{self, Entity};
    pub use horizon::mods::players::{self, GameMode, ItemStack, Player};
    pub use horizon::mods::server_net;
    pub use horizon::mods::storages::{self, FluidStack, StorageShape};
    pub use horizon::mods::world::{self, BlockChange};
    pub use horizon::mods::worldgen::{self};
    pub use horizon::mods::{
        admin, crowd, dimensions, fx, items, loot, noise, patterns, recipes_server, shapes, stock,
        timers,
    };

    /// The blocks joined to `start` through its six sides for which `member`
    /// says yes, `start` first, up to `limit`: a cable network, a pipe run, a
    /// multiblock. Each block looked at is one host call: keep `limit` honest,
    /// and rebuild from `neighbor_changed` rather than each tick.
    pub fn connected(
        start: BlockPos,
        limit: usize,
        mut member: impl FnMut(BlockPos) -> bool,
    ) -> Vec<BlockPos> {
        let key = |p: BlockPos| (p.x, p.y, p.z);
        let mut seen = std::collections::BTreeSet::from([key(start)]);
        let mut found = vec![start];
        let mut next = 0;
        while next < found.len() && found.len() < limit {
            let p = found[next];
            next += 1;
            for (dx, dy, dz) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                let q = BlockPos {
                    x: p.x + dx,
                    y: p.y + dy,
                    z: p.z + dz,
                };
                if found.len() < limit && seen.insert(key(q)) && member(q) {
                    found.push(q);
                }
            }
        }
        found
    }

    /// Moves up to `amount` FE from the storage at `from` into the one at
    /// `to`: what actually moved. Nothing is lost when `to` is full.
    pub fn move_energy(dimension: &str, from: BlockPos, to: BlockPos, amount: u64) -> u64 {
        let have = storages::extract_energy(dimension, from.x, from.y, from.z, amount, true);
        let moved = storages::insert_energy(dimension, to.x, to.y, to.z, have, false);
        storages::extract_energy(dimension, from.x, from.y, from.z, moved, false);
        moved
    }

    impl ItemStack {
        /// A plain stack of `count` `item`s, with no bytes of the mod's.
        pub fn new(item: &str, count: u32) -> Self {
            Self {
                item: item.to_owned(),
                count,
                data: Vec::new(),
                components: Vec::new(),
            }
        }
        /// The same stack carrying the mod's own bytes.
        pub fn with_data(mut self, data: Vec<u8>) -> Self {
            self.data = data;
            self
        }
    }

    /// The server's calls into a mod. Only the hooks [`Registration`] asks
    /// for are called; the others may stay as their defaults.
    pub trait ServerMod: Sized + 'static {
        /// Once, after loading: the mod's state and the hooks it wants. An
        /// error keeps the mod off and is logged.
        fn init() -> Result<(Self, Registration), String>;
        /// Every server tick, when [`Registration::tick`] is set.
        fn tick(&mut self, _tick: u64) {}
        /// When [`Registration::joins`] is set.
        fn player_joined(&mut self, _player: &Player) {}
        fn player_left(&mut self, _player: &Player) {}
        /// A chat message a player sent, when [`Registration::chat`] is set.
        fn chat(&mut self, _player: &Player, _text: &str) {}
        /// `/name args` typed by `player`, parsed as the command's
        /// arguments say ([`Registration::command_with`]). `Ok(Some(text))`
        /// is their feedback, `Err(text)` their error in red.
        fn command(
            &mut self,
            _player: &Player,
            _name: &str,
            _args: &Args,
        ) -> Result<Option<String>, String> {
            Err("This command does nothing".into())
        }
        /// When [`Registration::blocks`] is set: `state` is what was there.
        fn block_broken(&mut self, _player: &Player, _pos: BlockPos, _state: &str) {}
        fn block_placed(&mut self, _player: &Player, _pos: BlockPos, _state: &str) {}
        /// Before `player` starts breaking `state` at `pos`, when
        /// [`Registration::blocks`] is set: true refuses it and the block
        /// stays (the player's game puts it back).
        fn break_block(&mut self, _player: &Player, _pos: BlockPos, _state: &str) -> bool {
            false
        }
        /// Before vanilla acts on `player`'s right click on `state` at `pos`
        /// holding `item` (`minecraft:air`: empty hand), when
        /// [`Registration::blocks`] is set: true means the mod handled it, and
        /// vanilla neither uses the block nor uses or places the item.
        fn use_block(
            &mut self,
            _player: &Player,
            _pos: BlockPos,
            _state: &str,
            _item: &str,
        ) -> bool {
            false
        }
        /// Before `player` places `item` at `pos` (the block it would go in),
        /// when [`Registration::blocks`] is set: true refuses it.
        fn place_block(&mut self, _player: &Player, _pos: BlockPos, _item: &str) -> bool {
            false
        }
        /// `player` right-clicks the air holding `stack`, when
        /// [`Registration::blocks`] is set: true means the mod handled it.
        fn use_item(&mut self, _player: &Player, _stack: &ItemStack, _off_hand: bool) -> bool {
            false
        }
        /// The tick [`world::schedule_tick`] asked for, the block still
        /// `state`, when [`Registration::blocks`] is set.
        fn block_tick(&mut self, _dimension: &str, _pos: BlockPos, _state: &str) {}
        /// A neighbour (`from`) of a block the mod keeps data or a storage
        /// on changed, when [`Registration::blocks`] is set.
        fn neighbor_changed(
            &mut self,
            _dimension: &str,
            _pos: BlockPos,
            _state: &str,
            _from: BlockPos,
        ) {
        }

        /// `game-events`, for the events the manifest lists (`events =
        /// ["entity-hurt", ...]`), in order of `priority`. A mob appeared:
        /// true removes it.
        fn entity_spawned(&mut self, _dimension: &str, _entity: &Entity) -> bool {
            false
        }
        /// A mob lost `amount` health and lives: true gives it back.
        fn entity_hurt(&mut self, _dimension: &str, _entity: &Entity, _amount: f32) -> bool {
            false
        }
        /// A mob died; `killer` is the player who last hurt it.
        fn entity_died(&mut self, _dimension: &str, _entity: &Entity, _killer: Option<&str>) {}
        /// A player lost `amount` health.
        fn player_hurt(&mut self, _player: &Player, _amount: f32) {}
        /// A player picked up `stack`.
        fn item_picked_up(&mut self, _player: &Player, _stack: &ItemStack) {}
        /// A right click on a mob, as it happens: true takes it from vanilla.
        fn use_entity(&mut self, _player: &Player, _entity: &Entity, _item: &str) -> bool {
            false
        }
        /// A `projectile` mob type hit a block or `target`: true keeps it flying.
        fn projectile_hit(
            &mut self,
            _dimension: &str,
            _projectile: &Entity,
            _target: Option<&str>,
            _at: (f64, f64, f64),
        ) -> bool {
            false
        }
        /// A chunk of a dimension declared with `"generator": {"type": "code"}`.
        fn generate_chunk(&mut self, _dimension: &str, _x: i32, _z: i32) -> Option<ChunkFill> {
            None
        }
        /// Whether a mob may start `goal`, one of its type's `goals`.
        fn goal_can_use(&mut self, _goal: &str, _mob: &Entity) -> bool {
            false
        }
        /// A running goal's tick: false ends it.
        fn goal_tick(&mut self, _goal: &str, _mob: &Entity) -> bool {
            false
        }
        /// A goal ended or was replaced.
        fn goal_stop(&mut self, _goal: &str, _mob: &Entity) {}
        /// Each tick of a mob whose type says `"ticks": true`.
        fn entity_tick(&mut self, _dimension: &str, _entity: &Entity) {}
        /// Each tick of a mob or player under this mod's effect.
        fn effect_tick(&mut self, _effect: &str, _target: &str, _amplifier: u8) {}
        /// An attack on a mob, as it happens: true stops it.
        fn entity_dying(
            &mut self,
            _dimension: &str,
            _entity: &Entity,
            _killer: Option<&str>,
        ) -> bool {
            false
        }
        fn attack_entity(&mut self, _player: &Player, _entity: &Entity, _item: &str) -> bool {
            false
        }
        /// An explosion of `power` is about to go off: true stops it.
        fn explosion(&mut self, _dimension: &str, _x: f64, _y: f64, _z: f64, _power: f32) -> bool {
            false
        }
        /// A message from `player`'s client mods on `channel` (capability `net`).
        fn net_received(&mut self, _player: &Player, _channel: &str, _data: &[u8]) {}

        /// `before-events`, for the events the manifest lists with `before-`
        /// in front (`events = ["before-entity-hurt", ...]`), asked as they
        /// happen. A mob is about to appear (`reason`: `natural`, `spawner`,
        /// `breeding`...): true refuses it.
        fn before_entity_spawn(
            &mut self,
            _dimension: &str,
            _entity: &Entity,
            _reason: &str,
        ) -> bool {
            false
        }
        /// A mob is about to lose `amount` to `damage_type`: what it loses
        /// instead (0 or less refuses the hit).
        fn before_entity_hurt(
            &mut self,
            _dimension: &str,
            _entity: &Entity,
            _damage_type: &str,
            _attacker: Option<&str>,
            amount: f32,
        ) -> f32 {
            amount
        }
        /// The same for a player.
        fn before_player_hurt(
            &mut self,
            _player: &Player,
            _damage_type: &str,
            _attacker: Option<&str>,
            amount: f32,
        ) -> f32 {
            amount
        }
        /// A player is about to die: true keeps them alive on 1 health.
        fn before_player_death(&mut self, _player: &Player, _damage_type: &str) -> bool {
            false
        }
        /// The blocks an explosion is about to break: the ones that stay.
        fn before_explosion_blocks(
            &mut self,
            _dimension: &str,
            _at: (f64, f64, f64),
            _blocks: &[BlockPos],
        ) -> Vec<BlockPos> {
            Vec::new()
        }
        /// A player is about to pick up `stack`: true leaves it on the ground.
        fn before_item_pickup(&mut self, _player: &Player, _stack: &ItemStack) -> bool {
            false
        }
        /// A player is about to drop `stack`: true keeps it.
        fn before_item_toss(&mut self, _player: &Player, _stack: &ItemStack) -> bool {
            false
        }
        /// What a dead mob is about to drop: `None` leaves it.
        fn before_mob_drop(
            &mut self,
            _dimension: &str,
            _entity: &Entity,
            _killer: Option<&str>,
            _drops: &[ItemStack],
        ) -> Option<Vec<ItemStack>> {
            None
        }
        /// Experience about to drop: what drops.
        fn before_xp_drop(
            &mut self,
            _dimension: &str,
            _at: (f64, f64, f64),
            _source: &str,
            amount: u32,
        ) -> u32 {
            amount
        }
        /// A player is about to open `block`'s window: true refuses it.
        fn before_container_open(
            &mut self,
            _player: &Player,
            _pos: BlockPos,
            _block: &str,
        ) -> bool {
            false
        }
    }

    impl Registration {
        /// No commands and no hooks.
        pub fn new() -> Self {
            Self {
                commands: Vec::new(),
                tick: false,
                joins: false,
                chat: false,
                blocks: false,
            }
        }
        /// Adds `/name`, taking free text ([`Args::text`]): one optional
        /// `text` argument named `args`.
        pub fn command(self, name: &str) -> Self {
            self.command_with(name, [Argument::text("args").optional()])
        }
        /// Adds `/name` with typed arguments (none: nothing after the name):
        /// players' chat boxes complete and check them, and the server
        /// parses them into [`Args`].
        pub fn command_with(
            mut self,
            name: &str,
            arguments: impl IntoIterator<Item = Argument>,
        ) -> Self {
            self.commands.push(ServerCommand {
                name: name.into(),
                arguments: arguments.into_iter().collect(),
            });
            self
        }
        pub fn on_tick(mut self) -> Self {
            self.tick = true;
            self
        }
        pub fn on_joins(mut self) -> Self {
            self.joins = true;
            self
        }
        pub fn on_chat(mut self) -> Self {
            self.chat = true;
            self
        }
        pub fn on_blocks(mut self) -> Self {
            self.blocks = true;
            self
        }
    }
    impl Default for Registration {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Argument {
        fn of(name: &str, kind: ArgumentKind) -> Self {
            Self {
                name: name.into(),
                kind,
                optional: false,
            }
        }
        /// One word: letters, digits and `_ - . +`.
        pub fn word(name: &str) -> Self {
            Self::of(name, ArgumentKind::Word)
        }
        /// The rest of the line: only last.
        pub fn text(name: &str) -> Self {
            Self::of(name, ArgumentKind::Text)
        }
        /// A whole number, at least `min` and at most `max` when given.
        pub fn integer(name: &str, min: Option<i32>, max: Option<i32>) -> Self {
            Self::of(name, ArgumentKind::Integer((min, max)))
        }
        pub fn number(name: &str) -> Self {
            Self::of(name, ArgumentKind::Number)
        }
        pub fn boolean(name: &str) -> Self {
            Self::of(name, ArgumentKind::Boolean)
        }
        /// One online player: a name, `@s` or `@p`.
        pub fn player(name: &str) -> Self {
            Self::of(name, ArgumentKind::Player)
        }
        /// `x y z`, each one also `~` or `~n`.
        pub fn block_pos(name: &str) -> Self {
            Self::of(name, ArgumentKind::BlockPos)
        }
        /// May be left out; optional arguments come after the required ones.
        pub fn optional(mut self) -> Self {
            self.optional = true;
            self
        }
    }

    /// A command's arguments: the line as typed, and the values the server
    /// parsed from it, by argument name.
    #[derive(Clone, Debug, Default)]
    pub struct Args {
        text: String,
        values: Vec<(String, ArgumentValue)>,
    }
    impl Args {
        pub fn new(text: String, values: Vec<(String, ArgumentValue)>) -> Self {
            Self { text, values }
        }
        /// The line after the command's name, as typed.
        pub fn text(&self) -> &str {
            &self.text
        }
        /// The value of argument `name`; `None` when it was left out.
        pub fn get(&self, name: &str) -> Option<&ArgumentValue> {
            self.values.iter().find(|(n, _)| n == name).map(|(_, v)| v)
        }
        pub fn integer(&self, name: &str) -> Option<i32> {
            match self.get(name)? {
                ArgumentValue::Integer(n) => Some(*n),
                _ => None,
            }
        }
        pub fn number(&self, name: &str) -> Option<f64> {
            match self.get(name)? {
                ArgumentValue::Number(n) => Some(*n),
                _ => None,
            }
        }
        /// A `word` or `text` argument.
        pub fn string(&self, name: &str) -> Option<&str> {
            match self.get(name)? {
                ArgumentValue::Word(s) | ArgumentValue::Text(s) => Some(s),
                _ => None,
            }
        }
        pub fn boolean(&self, name: &str) -> Option<bool> {
            match self.get(name)? {
                ArgumentValue::Boolean(b) => Some(*b),
                _ => None,
            }
        }
        pub fn player(&self, name: &str) -> Option<&Player> {
            match self.get(name)? {
                ArgumentValue::Player(p) => Some(p),
                _ => None,
            }
        }
        pub fn block_pos(&self, name: &str) -> Option<BlockPos> {
            match self.get(name)? {
                ArgumentValue::BlockPos(p) => Some(*p),
                _ => None,
            }
        }
    }
}

/// One mod with both halves (`side = "both"`): `$client` a [`Mod`], `$server`
/// a [`ServerMod`](server::ServerMod). `cargo horizon-mod build` builds the
/// crate once for each and packs them together.
#[macro_export]
macro_rules! export_both {
    ($client:ty, $server:ty) => {
        #[cfg(horizon_side = "client")]
        $crate::export_mod!($client);
        #[cfg(horizon_side = "server")]
        $crate::export_server_mod!($server);
    };
}

/// Exports a [`ServerMod`](server::ServerMod) as the component's
/// `server-events` interface, as [`export_mod!`] does for a client mod.
#[macro_export]
macro_rules! export_server_mod {
    ($t:ty) => {
        const _: () = {
            use ::std::cell::RefCell;
            use $crate::server::{
                Args, ArgumentValue, BlockPos, Entity, ItemStack, Player, Registration, ServerMod,
            };
            ::std::thread_local! {
                static STATE: RefCell<Option<$t>> = const { RefCell::new(None) };
            }
            fn with(f: impl FnOnce(&mut $t)) {
                STATE.with(|s| {
                    if let Some(m) = s.borrow_mut().as_mut() {
                        f(m)
                    }
                })
            }
            struct Export;
            impl $crate::server::__Guest for Export {
                fn init() -> Result<Registration, String> {
                    let (m, registration) = <$t as ServerMod>::init()?;
                    STATE.with(|s| *s.borrow_mut() = Some(m));
                    Ok(registration)
                }
                fn tick(tick: u64) {
                    with(|m| m.tick(tick))
                }
                fn player_joined(player: Player) {
                    with(|m| m.player_joined(&player))
                }
                fn player_left(player: Player) {
                    with(|m| m.player_left(&player))
                }
                fn chat(player: Player, text: String) {
                    with(|m| m.chat(&player, &text))
                }
                fn command(
                    player: Player,
                    name: String,
                    args: String,
                    values: Vec<(String, ArgumentValue)>,
                ) -> Result<Option<String>, String> {
                    let mut out = Err("The mod is not running".to_string());
                    let args = Args::new(args, values);
                    with(|m| out = m.command(&player, &name, &args));
                    out
                }
                fn block_broken(player: Player, pos: BlockPos, state: String) {
                    with(|m| m.block_broken(&player, pos, &state))
                }
                fn block_placed(player: Player, pos: BlockPos, state: String) {
                    with(|m| m.block_placed(&player, pos, &state))
                }
            }
            impl $crate::server::__BlockGuest for Export {
                fn break_block(player: Player, x: i32, y: i32, z: i32, state: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.break_block(&player, BlockPos { x, y, z }, &state));
                    out
                }
                fn use_block(
                    player: Player,
                    x: i32,
                    y: i32,
                    z: i32,
                    state: String,
                    item: String,
                ) -> bool {
                    let mut out = false;
                    with(|m| out = m.use_block(&player, BlockPos { x, y, z }, &state, &item));
                    out
                }
                fn place_block(player: Player, x: i32, y: i32, z: i32, item: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.place_block(&player, BlockPos { x, y, z }, &item));
                    out
                }
                fn use_item(player: Player, stack: ItemStack, off_hand: bool) -> bool {
                    let mut out = false;
                    with(|m| out = m.use_item(&player, &stack, off_hand));
                    out
                }
                fn block_tick(dimension: String, x: i32, y: i32, z: i32, state: String) {
                    with(|m| m.block_tick(&dimension, BlockPos { x, y, z }, &state))
                }
                fn neighbor_changed(
                    dimension: String,
                    x: i32,
                    y: i32,
                    z: i32,
                    state: String,
                    from_x: i32,
                    from_y: i32,
                    from_z: i32,
                ) {
                    with(|m| {
                        m.neighbor_changed(
                            &dimension,
                            BlockPos { x, y, z },
                            &state,
                            BlockPos {
                                x: from_x,
                                y: from_y,
                                z: from_z,
                            },
                        )
                    })
                }
            }
            impl $crate::server::__ServerNetGuest for Export {
                fn received(player: Player, channel: String, data: Vec<u8>) {
                    with(|m| m.net_received(&player, &channel, &data))
                }
            }
            impl $crate::server::__EntityGuest for Export {
                fn use_entity(player: Player, entity: Entity, item: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.use_entity(&player, &entity, &item));
                    out
                }
                fn projectile_hit(
                    dimension: String,
                    projectile: Entity,
                    target: Option<String>,
                    x: f64,
                    y: f64,
                    z: f64,
                ) -> bool {
                    let mut out = false;
                    with(|m| {
                        out =
                            m.projectile_hit(&dimension, &projectile, target.as_deref(), (x, y, z))
                    });
                    out
                }
                fn entity_dying(dimension: String, entity: Entity, killer: Option<String>) -> bool {
                    let mut out = false;
                    with(|m| out = m.entity_dying(&dimension, &entity, killer.as_deref()));
                    out
                }
                fn attack_entity(player: Player, entity: Entity, item: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.attack_entity(&player, &entity, &item));
                    out
                }
            }
            impl $crate::server::__LogicGuest for Export {
                fn generate_chunk(
                    dimension: String,
                    x: i32,
                    z: i32,
                ) -> Option<$crate::server::ChunkFill> {
                    let mut out = None;
                    with(|m| out = m.generate_chunk(&dimension, x, z));
                    out
                }
                fn goal_can_use(goal: String, mob: Entity) -> bool {
                    let mut out = false;
                    with(|m| out = m.goal_can_use(&goal, &mob));
                    out
                }
                fn goal_tick(goal: String, mob: Entity) -> bool {
                    let mut out = false;
                    with(|m| out = m.goal_tick(&goal, &mob));
                    out
                }
                fn goal_stop(goal: String, mob: Entity) {
                    with(|m| m.goal_stop(&goal, &mob))
                }
                fn entity_tick(dimension: String, entity: Entity) {
                    with(|m| m.entity_tick(&dimension, &entity))
                }
                fn effect_tick(effect: String, target: String, amplifier: u8) {
                    with(|m| m.effect_tick(&effect, &target, amplifier))
                }
            }
            impl $crate::server::__EventsGuest for Export {
                fn entity_spawned(dimension: String, entity: Entity) -> bool {
                    let mut out = false;
                    with(|m| out = m.entity_spawned(&dimension, &entity));
                    out
                }
                fn entity_hurt(dimension: String, entity: Entity, amount: f32) -> bool {
                    let mut out = false;
                    with(|m| out = m.entity_hurt(&dimension, &entity, amount));
                    out
                }
                fn entity_died(dimension: String, entity: Entity, killer: Option<String>) {
                    with(|m| m.entity_died(&dimension, &entity, killer.as_deref()))
                }
                fn player_hurt(player: Player, amount: f32) {
                    with(|m| m.player_hurt(&player, amount))
                }
                fn item_picked_up(player: Player, stack: ItemStack) {
                    with(|m| m.item_picked_up(&player, &stack))
                }
                fn explosion(dimension: String, x: f64, y: f64, z: f64, power: f32) -> bool {
                    let mut out = false;
                    with(|m| out = m.explosion(&dimension, x, y, z, power));
                    out
                }
            }
            impl $crate::server::__BeforeGuest for Export {
                fn entity_spawn(dimension: String, entity: Entity, reason: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.before_entity_spawn(&dimension, &entity, &reason));
                    out
                }
                fn entity_hurt(
                    dimension: String,
                    entity: Entity,
                    damage_type: String,
                    attacker: Option<String>,
                    amount: f32,
                ) -> f32 {
                    let mut out = amount;
                    with(|m| {
                        out = m.before_entity_hurt(
                            &dimension,
                            &entity,
                            &damage_type,
                            attacker.as_deref(),
                            amount,
                        )
                    });
                    out
                }
                fn player_hurt(
                    player: Player,
                    damage_type: String,
                    attacker: Option<String>,
                    amount: f32,
                ) -> f32 {
                    let mut out = amount;
                    with(|m| {
                        out =
                            m.before_player_hurt(&player, &damage_type, attacker.as_deref(), amount)
                    });
                    out
                }
                fn player_death(player: Player, damage_type: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.before_player_death(&player, &damage_type));
                    out
                }
                fn explosion_blocks(
                    dimension: String,
                    x: f64,
                    y: f64,
                    z: f64,
                    blocks: Vec<(i32, i32, i32)>,
                ) -> Vec<(i32, i32, i32)> {
                    let blocks: Vec<BlockPos> = blocks
                        .into_iter()
                        .map(|(x, y, z)| BlockPos { x, y, z })
                        .collect();
                    let mut out = Vec::new();
                    with(|m| out = m.before_explosion_blocks(&dimension, (x, y, z), &blocks));
                    out.into_iter().map(|p| (p.x, p.y, p.z)).collect()
                }
                fn item_pickup(player: Player, stack: ItemStack) -> bool {
                    let mut out = false;
                    with(|m| out = m.before_item_pickup(&player, &stack));
                    out
                }
                fn item_toss(player: Player, stack: ItemStack) -> bool {
                    let mut out = false;
                    with(|m| out = m.before_item_toss(&player, &stack));
                    out
                }
                fn mob_drop(
                    dimension: String,
                    entity: Entity,
                    killer: Option<String>,
                    drops: Vec<ItemStack>,
                ) -> Option<Vec<ItemStack>> {
                    let mut out = None;
                    with(|m| {
                        out = m.before_mob_drop(&dimension, &entity, killer.as_deref(), &drops)
                    });
                    out
                }
                fn xp_drop(
                    dimension: String,
                    x: f64,
                    y: f64,
                    z: f64,
                    source: String,
                    amount: u32,
                ) -> u32 {
                    let mut out = amount;
                    with(|m| out = m.before_xp_drop(&dimension, (x, y, z), &source, amount));
                    out
                }
                fn container_open(player: Player, x: i32, y: i32, z: i32, block: String) -> bool {
                    let mut out = false;
                    with(|m| out = m.before_container_open(&player, BlockPos { x, y, z }, &block));
                    out
                }
            }
            #[cfg(target_arch = "wasm32")]
            $crate::server::__export_horizon_server_mod!(Export);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = <Export as $crate::server::__Guest>::init;
        };
    };
}
