//! Server mods' items, storages on blocks, block timers and entities
//! (API level 10): the stacks they hand around, what the server keeps for
//! them beside their bytes on blocks, and the host side of `storages`,
//! `entities` and the item half of `players`.
use super::{FUEL_EVENT, Mods, Output, Running, State, block_id, block_key, server_bindings};
use server_bindings::horizon::mods::{entities, players, storages};
use std::collections::BTreeMap;

/// A stack as the server keeps it for mods: the item, its count, the game's
/// own components (opaque here, the game's encoding) and every mod's bytes.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HostStack {
    pub item: String,
    pub count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<u8>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub data: BTreeMap<String, Vec<u8>>,
}
impl HostStack {
    /// The same item with the same components and bytes: they stack.
    pub fn stacks_with(&self, other: &HostStack) -> bool {
        self.item == other.item && self.components == other.components && self.data == other.data
    }
}

/// A tank of a storage: its fluid and amount (mB), and its capacity.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tank {
    pub fluid: Option<(String, u32)>,
    pub capacity: u32,
}

/// What a mod declared on a block (`storages.declare`).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Storage {
    pub owner: String,
    /// The block it was declared on, without properties.
    pub block: String,
    pub slots: Vec<Option<HostStack>>,
    pub energy: u64,
    pub energy_capacity: u64,
    pub tanks: Vec<Tank>,
    /// `storages.set-menu-data`: a furnace window's bars.
    #[serde(default)]
    pub menu_data: Vec<i16>,
    /// `storages.set-slot-rule`, by slot.
    #[serde(default)]
    pub slot_rules: std::collections::BTreeMap<u32, SlotRule>,
}

/// What a window's player may do with a slot.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SlotRule {
    pub take_only: bool,
    pub items: Vec<String>,
}

/// What a mod changes on a mob.
#[derive(Clone, Debug, PartialEq)]
pub enum MobChange {
    Ai(bool),
    Name(Option<String>),
    Velocity(f64, f64, f64),
    MoveTo(f64, f64, f64, f64),
    Target(Option<String>),
    Heal(f32),
    Health(f32),
    /// `entities.set-baby`.
    Baby(bool),
    /// `entities.set-owner`: a player's UUID, none untames.
    Owner(Option<String>),
    /// `entities.set-fire` (0: `extinguish`).
    Fire(u32),
    Flag(MobFlag, bool),
    /// `entities.remove`: gone with no death.
    Remove,
    AddVelocity(f64, f64, f64),
    /// Strength, then the x and z it is pushed from.
    Knockback(f64, f64, f64),
}

/// A mob's on-off flags `entities` sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobFlag {
    Glowing,
    Invisible,
    Invulnerable,
    Silent,
    NoGravity,
    Persistent,
}

/// `players.style-stack`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackLook {
    pub name: Option<String>,
    pub lore: Vec<String>,
    pub model: Option<String>,
    pub glint: Option<bool>,
}

/// `world.set-redstone-output` and `set-comparator-output` on a block.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RedstoneOutput {
    /// The mod that set them: they stop when it is not running.
    #[serde(default)]
    pub owner: String,
    /// The block they were set on, without properties.
    pub block: String,
    pub signal: u8,
    pub comparator: u8,
}

/// A `world.schedule-tick` waiting.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScheduledTick {
    pub owner: String,
    pub dimension: String,
    pub pos: (i32, i32, i32),
    pub block: String,
    pub due: u64,
}

/// A mob as `entities.near` reports it; the game lists them per level.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityInfo {
    pub id: String,
    pub kind: String,
    pub pos: (f64, f64, f64),
    pub health: f32,
}

/// What a server mod asks the game to do at the tick's end (the game owns
/// players' inventories, windows and the command dispatcher).
#[derive(Clone, Debug, PartialEq)]
pub enum ServerAction {
    Give {
        player: String,
        stack: HostStack,
    },
    SetSlot {
        player: String,
        slot: u32,
        stack: Option<HostStack>,
    },
    Take {
        player: String,
        item: String,
        count: u32,
    },
    /// A vanilla command line, run as the server console.
    Console {
        line: String,
    },
    OpenMenu {
        player: String,
        dimension: String,
        pos: (i32, i32, i32),
        title: String,
        /// The window: `minecraft:furnace`, ...
        kind: String,
    },
    /// `server-net.send` (one player) or `broadcast` (none: everyone).
    Net {
        player: Option<String>,
        channel: String,
        data: Vec<u8>,
    },
    /// `entities.set-ai`, `set-name`, `set-velocity`.
    Mob {
        id: String,
        change: MobChange,
    },
    /// `entities.spawn` of a mod's own type: the console doesn't know it.
    SpawnMob {
        dimension: String,
        kind: String,
        pos: (f64, f64, f64),
        /// `entities.spawn-with-reason`: the UUID it gets and why it came.
        uuid: Option<String>,
        reason: Option<String>,
    },
    /// `entities.set-pickup-delay` on a dropped item.
    PickupDelay {
        id: String,
        ticks: i32,
    },
    /// `entities.xp-orb`.
    XpOrb {
        dimension: String,
        pos: (f64, f64, f64),
        amount: u32,
    },
    /// `entities.drop-item`.
    Undying {
        player: String,
        on: bool,
    },
    DropItem {
        dimension: String,
        pos: (i32, i32, i32),
        stack: HostStack,
    },
    /// `world.explode`.
    Explode {
        dimension: String,
        pos: (f64, f64, f64),
        power: f32,
    },
    /// `worldgen.add-ore` by the server mod `id`.
    Ore {
        id: String,
        ore: crate::OreDefinition,
    },
    /// A world generation rule registered by the server mod `id`.
    Worldgen {
        id: String,
        biome: String,
        surface: String,
    },
}

/// Slots in a player's inventory as `players.inventory` numbers them.
pub const INVENTORY_SLOTS: u32 = 41;
const STACK_DATA_BYTES: usize = 1024;
const MAX_SLOTS: u32 = 54;
const MAX_TANKS: usize = 16;
const STORAGES_PER_MOD: usize = 65536;
const TICKS_PER_MOD: usize = 65536;
const MAX_DELAY: u32 = 72_000;
/// Changes a call may ask for (gives, slots, commands, windows).
const ACTIONS_PER_CALL: usize = 4096;
/// Scheduled ticks run in one server tick; the rest wait for the next.
const TIMERS_PER_TICK: usize = 4096;
/// Ticks until a timer whose chunk is not loaded looks again.
const UNLOADED_RETRY: u64 = 100;

/// The `components` a guest sees: the game's components and the other
/// mods' bytes, which it hands back unchanged.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Opaque {
    #[serde(default)]
    c: String,
    #[serde(default)]
    d: BTreeMap<String, String>,
}

fn hex(bytes: &[u8]) -> String {
    super::to_hex(bytes)
}

/// `stack` as mod `me` sees it.
pub(in crate::runtime) fn to_guest(stack: &HostStack, me: &str) -> players::ItemStack {
    let others: BTreeMap<String, String> = stack
        .data
        .iter()
        .filter(|(id, _)| id.as_str() != me)
        .map(|(id, d)| (id.clone(), hex(d)))
        .collect();
    let components = if stack.components.is_empty() && others.is_empty() {
        Vec::new()
    } else {
        serde_json::to_vec(&Opaque {
            c: hex(&stack.components),
            d: others,
        })
        .unwrap_or_default()
    };
    players::ItemStack {
        item: stack.item.clone(),
        count: stack.count,
        data: stack.data.get(me).cloned().unwrap_or_default(),
        components,
    }
}

/// A stack mod `me` handed in.
pub(super) fn from_guest(stack: players::ItemStack, me: &str) -> Result<HostStack, String> {
    if stack.data.len() > STACK_DATA_BYTES {
        return Err(format!(
            "{} bytes on a stack: at most {STACK_DATA_BYTES}",
            stack.data.len()
        ));
    }
    let opaque: Opaque = if stack.components.is_empty() {
        Opaque::default()
    } else {
        serde_json::from_slice(&stack.components)
            .map_err(|_| "components: not a stack's, hand them back unchanged".to_string())?
    };
    let mut data: BTreeMap<String, Vec<u8>> = opaque
        .d
        .into_iter()
        .filter(|(id, _)| id.as_str() != me)
        .map(|(id, h)| (id, super::from_hex(&h)))
        .collect();
    if !stack.data.is_empty() {
        data.insert(me.to_owned(), stack.data);
    }
    Ok(HostStack {
        item: stack.item,
        count: stack.count,
        components: super::from_hex(&opaque.c),
        data,
    })
}

fn guest_fluid(t: &Tank) -> Option<storages::FluidStack> {
    t.fluid
        .as_ref()
        .map(|(fluid, amount)| storages::FluidStack {
            fluid: fluid.clone(),
            amount: *amount,
        })
}

/// Moves `stack` into `slots` as a hopper does; how many went in.
fn insert_into(
    slots: &mut [Option<HostStack>],
    stack: &HostStack,
    max: u32,
    simulate: bool,
) -> u32 {
    let mut left = stack.count;
    let mut moved: Vec<(usize, u32)> = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        if left == 0 {
            break;
        }
        if let Some(s) = slot
            && s.stacks_with(stack)
            && s.count < max
        {
            let n = (max - s.count).min(left);
            moved.push((i, n));
            left -= n;
        }
    }
    for (i, slot) in slots.iter().enumerate() {
        if left == 0 {
            break;
        }
        if slot.is_none() {
            let n = max.min(left);
            moved.push((i, n));
            left -= n;
        }
    }
    if !simulate {
        for (i, n) in moved {
            match &mut slots[i] {
                Some(s) => s.count += n,
                none => {
                    *none = Some(HostStack {
                        count: n,
                        ..stack.clone()
                    })
                }
            }
        }
    }
    stack.count - left
}

/// Takes up to `count` from the first slot holding `item` (none: any).
fn extract_from(
    slots: &mut [Option<HostStack>],
    item: Option<&str>,
    count: u32,
    simulate: bool,
) -> Option<HostStack> {
    let i = slots.iter().position(|s| {
        s.as_ref()
            .is_some_and(|s| s.count > 0 && item.is_none_or(|item| s.item == item))
    })?;
    let s = slots[i].as_mut()?;
    let n = s.count.min(count);
    if n == 0 {
        return None;
    }
    let out = HostStack {
        count: n,
        ..s.clone()
    };
    if !simulate {
        s.count -= n;
        if s.count == 0 {
            slots[i] = None;
        }
    }
    Some(out)
}

fn insert_fluid(tanks: &mut [Tank], fluid: &str, amount: u32, simulate: bool) -> u32 {
    let mut left = amount;
    for pass in 0..2 {
        for t in tanks.iter_mut() {
            if left == 0 {
                break;
            }
            let held = match &t.fluid {
                Some((f, a)) if pass == 0 && f == fluid => *a,
                None if pass == 1 => 0,
                _ => continue,
            };
            let n = (t.capacity.saturating_sub(held)).min(left);
            if n == 0 {
                continue;
            }
            left -= n;
            if !simulate {
                t.fluid = Some((fluid.to_owned(), held + n));
            }
        }
    }
    amount - left
}

fn extract_fluid(
    tanks: &mut [Tank],
    fluid: Option<&str>,
    amount: u32,
    simulate: bool,
) -> Option<(String, u32)> {
    let t = tanks.iter_mut().find(|t| {
        t.fluid
            .as_ref()
            .is_some_and(|(f, a)| *a > 0 && fluid.is_none_or(|w| w == f))
    })?;
    let (f, a) = t.fluid.clone()?;
    let n = a.min(amount);
    if n == 0 {
        return None;
    }
    if !simulate {
        t.fluid = (a > n).then(|| (f.clone(), a - n));
    }
    Some((f, n))
}

fn check_id(kind: &str, id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 256
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-.:/".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!("{kind} {id:?}: not an id"))
    }
}
fn check_uuid(id: &str) -> Result<(), String> {
    let ok = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(format!("{id:?}: not a UUID"))
    }
}

impl State {
    /// `entities.*` changing a mob by UUID, at the tick's end.
    fn change_mob(&mut self, id: String, change: MobChange) -> Result<(), String> {
        check_uuid(&id)?;
        self.act(ServerAction::Mob { id, change })
    }
    /// Queues a change for the game; Err past the per-call limit.
    pub(super) fn act(&mut self, action: ServerAction) -> Result<(), String> {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Server(_)))
            .count();
        if queued >= ACTIONS_PER_CALL {
            return Err(format!("at most {ACTIONS_PER_CALL} changes per call"));
        }
        self.out.push(Output::Server(action));
        Ok(())
    }
    fn check_dimension(&self, dimension: &str) -> Result<(), String> {
        match self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            Some(view) if !view.has_dimension(dimension) => {
                Err(format!("no dimension {dimension}"))
            }
            _ => Ok(()),
        }
    }
    /// A stack's size limit; Err for an item the game does not know. With
    /// no world lent, 64.
    fn max_stack(&self, item: &str) -> Result<u32, String> {
        match self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            Some(view) => view
                .max_stack(item)
                .ok_or_else(|| format!("no item {item}")),
            None => Ok(64),
        }
    }
    /// The online player with this id; Err when not online.
    pub(super) fn online_player(&self, id: &str) -> Result<players::Player, String> {
        self.snapshot
            .players
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| format!("no player {id} online"))
    }
    pub(super) fn check_dim(&self, dimension: &str) -> Result<(), String> {
        self.check_dimension(dimension)
    }
    pub(super) fn console(&mut self, line: String) -> Result<(), String> {
        if line.contains(['\n', '\r']) {
            return Err("one line only".into());
        }
        // The game's selectors find a player by name, not by UUID.
        let line = self
            .snapshot
            .players
            .iter()
            .fold(line, |l, p| l.replace(&p.id, &p.name));
        self.act(ServerAction::Console { line })
    }

    /// The storage at a position, dropped when its block is gone; `f` runs
    /// on it with the data locked.
    fn with_storage<R>(
        &mut self,
        dimension: &str,
        (x, y, z): (i32, i32, i32),
        f: impl FnOnce(&mut super::Storage) -> R,
    ) -> Option<R> {
        let now = self.block_at(dimension, x, y, z)?;
        let key = block_key(dimension, x, y, z);
        let mut all = self.block_data.lock().ok()?;
        let s = all.storages.get_mut(&key)?;
        if s.block != now {
            all.storages.remove(&key);
            return None;
        }
        Some(f(s))
    }
    fn with_own_storage<R>(
        &mut self,
        dimension: &str,
        pos: (i32, i32, i32),
        f: impl FnOnce(&mut super::Storage) -> R,
    ) -> Result<R, String> {
        let me = self.id.clone();
        let mut f = Some(f);
        match self.with_storage(dimension, pos, |s| {
            (s.owner == me).then(|| (f.take().unwrap())(s))
        }) {
            Some(Some(r)) => Ok(r),
            Some(None) => Err("another mod's storage".into()),
            None => Err(format!("no storage at {} {} {}", pos.0, pos.1, pos.2)),
        }
    }
    /// A vanilla container's slots there.
    fn container(
        &self,
        dimension: &str,
        (x, y, z): (i32, i32, i32),
    ) -> Option<Vec<Option<HostStack>>> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.container(dimension, x, y, z)
    }
    fn set_container(
        &self,
        dimension: &str,
        (x, y, z): (i32, i32, i32),
        slots: Vec<Option<HostStack>>,
    ) -> bool {
        let Ok(mut view) = self.world.lock() else {
            return false;
        };
        view.as_mut()
            .is_some_and(|v| v.set_container(dimension, x, y, z, slots))
    }
}

// ---------------------------------------------------------------- players
impl State {
    fn inventory_of(&self, player: &str) -> Option<&(String, u32, Vec<Option<HostStack>>)> {
        self.snapshot
            .inventories
            .iter()
            .find(|(id, ..)| id == player)
    }
    pub(super) fn player_inventory(&mut self, player: String) -> Vec<Option<players::ItemStack>> {
        let me = self.id.clone();
        self.inventory_of(&player)
            .map(|(_, _, slots)| {
                slots
                    .iter()
                    .map(|s| s.as_ref().map(|s| to_guest(s, &me)))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub(super) fn player_selected(&mut self, player: String) -> Option<u32> {
        self.inventory_of(&player).map(|(_, selected, _)| *selected)
    }
    fn online(&self, player: &str) -> Result<(), String> {
        if self.snapshot.players.iter().any(|p| p.id == player) {
            Ok(())
        } else {
            Err(format!("no player {player} online"))
        }
    }
    pub(super) fn player_give(
        &mut self,
        player: String,
        stack: players::ItemStack,
    ) -> Result<(), String> {
        self.online(&player)?;
        let stack = from_guest(stack, &self.id)?;
        self.max_stack(&stack.item)?;
        if stack.count == 0 {
            return Ok(());
        }
        self.act(ServerAction::Give { player, stack })
    }
    pub(super) fn player_set_slot(
        &mut self,
        player: String,
        slot: u32,
        stack: Option<players::ItemStack>,
    ) -> Result<(), String> {
        self.online(&player)?;
        if slot >= INVENTORY_SLOTS {
            return Err(format!("slot {slot}: 0 to {}", INVENTORY_SLOTS - 1));
        }
        let stack = match stack {
            Some(s) => {
                let s = from_guest(s, &self.id)?;
                self.max_stack(&s.item)?;
                (s.count > 0).then_some(s)
            }
            None => None,
        };
        self.act(ServerAction::SetSlot {
            player,
            slot,
            stack,
        })
    }
    pub(super) fn player_take(&mut self, player: String, item: String, count: u32) -> u32 {
        let held: u32 = self
            .inventory_of(&player)
            .map(|(_, _, slots)| {
                slots
                    .iter()
                    .flatten()
                    .filter(|s| s.item == item)
                    .map(|s| s.count)
                    .sum()
            })
            .unwrap_or(0);
        let n = held.min(count);
        if n == 0
            || self
                .act(ServerAction::Take {
                    player,
                    item,
                    count: n,
                })
                .is_err()
        {
            return 0;
        }
        n
    }
}

// ---------------------------------------------------------------- world
impl State {
    pub(super) fn schedule_tick(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        delay: u32,
    ) -> Result<(), String> {
        if !(1..=MAX_DELAY).contains(&delay) {
            return Err(format!("delay {delay}: 1 to {MAX_DELAY} ticks"));
        }
        let block = self
            .block_at(&dimension, x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        let me = self.id.clone();
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        if all.ticks.iter().filter(|t| t.owner == me).count() >= TICKS_PER_MOD {
            return Err(format!("at most {TICKS_PER_MOD} waiting ticks per mod"));
        }
        let due = all.now + u64::from(delay);
        all.ticks.push(ScheduledTick {
            owner: me,
            dimension,
            pos: (x, y, z),
            block,
            due,
        });
        Ok(())
    }
    /// `world.set-redstone-output` and `set-comparator-output`.
    pub(super) fn set_output(
        &mut self,
        dimension: &str,
        (x, y, z): (i32, i32, i32),
        set: impl FnOnce(&mut RedstoneOutput),
        signal: u8,
    ) -> Result<(), String> {
        if signal > 15 {
            return Err(format!("signal {signal}: 0 to 15"));
        }
        let block = self
            .block_at(dimension, x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let key = block_key(dimension, x, y, z);
        let at = all.outputs.entry(key.clone()).or_default();
        if at.block != block {
            *at = RedstoneOutput {
                block,
                ..Default::default()
            };
        }
        at.owner = self.id.clone();
        set(at);
        if at.signal == 0 && at.comparator == 0 {
            all.outputs.remove(&key);
        }
        Ok(())
    }
    pub(super) fn redstone_power(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<u8> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.redstone(&dimension, x, y, z)
    }
}

// ---------------------------------------------------------------- storages
impl storages::Host for State {
    fn declare(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        shape: storages::StorageShape,
    ) -> Result<(), String> {
        if shape.slots > MAX_SLOTS || shape.tanks.len() > MAX_TANKS {
            return Err(format!("at most {MAX_SLOTS} slots and {MAX_TANKS} tanks"));
        }
        let block = self
            .block_at(&dimension, x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        if block == "minecraft:air" {
            return Err(format!("no block at {x} {y} {z}: a storage needs one"));
        }
        if self.container(&dimension, (x, y, z)).is_some() {
            return Err("a vanilla container is there".into());
        }
        let me = self.id.clone();
        let key = block_key(&dimension, x, y, z);
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let old = all.storages.remove(&key).filter(|s| s.block == block);
        if let Some(s) = &old
            && s.owner != me
        {
            let s = s.clone();
            all.storages.insert(key, s);
            return Err("another mod's storage is there".into());
        }
        if old.is_none()
            && all.storages.values().filter(|s| s.owner == me).count() >= STORAGES_PER_MOD
        {
            return Err(format!("at most {STORAGES_PER_MOD} storages per mod"));
        }
        let old = old.unwrap_or_default();
        let mut slots = old.slots;
        slots.resize(shape.slots as usize, None);
        let mut tanks: Vec<super::Tank> = shape
            .tanks
            .iter()
            .map(|&capacity| super::Tank {
                fluid: None,
                capacity,
            })
            .collect();
        for (t, o) in tanks.iter_mut().zip(old.tanks) {
            t.fluid = o
                .fluid
                .map(|(f, a)| (f, a.min(t.capacity)))
                .filter(|(_, a)| *a > 0);
        }
        all.storages.insert(
            key,
            super::Storage {
                owner: me,
                block,
                slots,
                energy: old.energy.min(shape.energy_capacity),
                energy_capacity: shape.energy_capacity,
                tanks,
                menu_data: old.menu_data,
                slot_rules: old.slot_rules,
            },
        );
        Ok(())
    }
    fn remove(&mut self, dimension: String, x: i32, y: i32, z: i32) {
        let key = block_key(&dimension, x, y, z);
        if let Ok(mut all) = self.block_data.lock()
            && all.storages.get(&key).is_some_and(|s| s.owner == self.id)
        {
            all.storages.remove(&key);
        }
    }
    fn shape(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<storages::StorageShape> {
        self.with_storage(&dimension, (x, y, z), |s| storages::StorageShape {
            slots: s.slots.len() as u32,
            energy_capacity: s.energy_capacity,
            tanks: s.tanks.iter().map(|t| t.capacity).collect(),
        })
        .or_else(|| {
            self.container(&dimension, (x, y, z))
                .map(|c| storages::StorageShape {
                    slots: c.len() as u32,
                    energy_capacity: 0,
                    tanks: Vec::new(),
                })
        })
    }
    fn items(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<Vec<Option<players::ItemStack>>> {
        let me = self.id.clone();
        let slots = self
            .with_storage(&dimension, (x, y, z), |s| s.slots.clone())
            .or_else(|| self.container(&dimension, (x, y, z)))?;
        Some(
            slots
                .iter()
                .map(|s| s.as_ref().map(|s| to_guest(s, &me)))
                .collect(),
        )
    }
    fn set_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
        stack: Option<players::ItemStack>,
    ) -> Result<(), String> {
        let stack = match stack {
            Some(s) => {
                let s = from_guest(s, &self.id)?;
                self.max_stack(&s.item)?;
                (s.count > 0).then_some(s)
            }
            None => None,
        };
        self.with_own_storage(&dimension, (x, y, z), |s| {
            match s.slots.get_mut(slot as usize) {
                Some(at) => {
                    *at = stack;
                    Ok(())
                }
                None => Err(format!("slot {slot}: the storage has {}", s.slots.len())),
            }
        })?
    }
    fn insert_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        stack: players::ItemStack,
        simulate: bool,
    ) -> u32 {
        let Ok(stack) = from_guest(stack, &self.id) else {
            return 0;
        };
        let Ok(max) = self.max_stack(&stack.item) else {
            return 0;
        };
        if stack.count == 0 {
            return 0;
        }
        if let Some(n) = self.with_storage(&dimension, (x, y, z), |s| {
            insert_into(&mut s.slots, &stack, max, simulate)
        }) {
            return n;
        }
        let Some(mut slots) = self.container(&dimension, (x, y, z)) else {
            return 0;
        };
        let n = insert_into(&mut slots, &stack, max, simulate);
        if !simulate && n > 0 && !self.set_container(&dimension, (x, y, z), slots) {
            return 0;
        }
        n
    }
    fn extract_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        item: Option<String>,
        count: u32,
        simulate: bool,
    ) -> Option<players::ItemStack> {
        let me = self.id.clone();
        let item = item.as_deref();
        let out = match self.with_storage(&dimension, (x, y, z), |s| {
            extract_from(&mut s.slots, item, count, simulate)
        }) {
            Some(out) => out,
            None => {
                let mut slots = self.container(&dimension, (x, y, z))?;
                let out = extract_from(&mut slots, item, count, simulate)?;
                if !simulate && !self.set_container(&dimension, (x, y, z), slots) {
                    return None;
                }
                Some(out)
            }
        }?;
        Some(to_guest(&out, &me))
    }
    fn energy(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<(u64, u64)> {
        self.with_storage(&dimension, (x, y, z), |s| (s.energy, s.energy_capacity))
            .filter(|(_, cap)| *cap > 0)
    }
    fn insert_energy(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u64,
        simulate: bool,
    ) -> u64 {
        self.with_storage(&dimension, (x, y, z), |s| {
            let n = (s.energy_capacity.saturating_sub(s.energy)).min(amount);
            if !simulate {
                s.energy += n;
            }
            n
        })
        .unwrap_or(0)
    }
    fn extract_energy(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u64,
        simulate: bool,
    ) -> u64 {
        self.with_storage(&dimension, (x, y, z), |s| {
            let n = s.energy.min(amount);
            if !simulate {
                s.energy -= n;
            }
            n
        })
        .unwrap_or(0)
    }
    fn set_energy(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u64,
    ) -> Result<(), String> {
        self.with_own_storage(&dimension, (x, y, z), |s| {
            s.energy = amount.min(s.energy_capacity)
        })
    }
    fn fluids(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<Vec<Option<storages::FluidStack>>> {
        self.with_storage(&dimension, (x, y, z), |s| {
            s.tanks.iter().map(guest_fluid).collect()
        })
        .filter(|t: &Vec<_>| !t.is_empty())
    }
    fn insert_fluid(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        fluid: storages::FluidStack,
        simulate: bool,
    ) -> u32 {
        if check_id("fluid", &fluid.fluid).is_err() {
            return 0;
        }
        self.with_storage(&dimension, (x, y, z), |s| {
            insert_fluid(&mut s.tanks, &fluid.fluid, fluid.amount, simulate)
        })
        .unwrap_or(0)
    }
    fn extract_fluid(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        fluid: Option<String>,
        amount: u32,
        simulate: bool,
    ) -> Option<storages::FluidStack> {
        let (fluid, amount) = self.with_storage(&dimension, (x, y, z), |s| {
            extract_fluid(&mut s.tanks, fluid.as_deref(), amount, simulate)
        })??;
        Some(storages::FluidStack { fluid, amount })
    }
    fn set_fluid(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        tank: u32,
        fluid: Option<storages::FluidStack>,
    ) -> Result<(), String> {
        if let Some(f) = &fluid {
            check_id("fluid", &f.fluid)?;
        }
        self.with_own_storage(&dimension, (x, y, z), |s| {
            match s.tanks.get_mut(tank as usize) {
                Some(t) => {
                    let target_fluid = fluid.clone();
                    let resolved =
                        if let (Some(f), Some((old, _))) = (&target_fluid, t.fluid.as_ref()) {
                            if old != &f.fluid {
                                crate::FluidInteractionResolver::with_defaults()
                                    .resolve(old, &f.fluid)
                                    .map(str::to_owned)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                    t.fluid = target_fluid
                        .map(|f| {
                            let name = resolved.unwrap_or(f.fluid);
                            (name, f.amount.min(t.capacity))
                        })
                        .filter(|(_, a)| *a > 0);
                    Ok(())
                }
                None => Err(format!("tank {tank}: the storage has {}", s.tanks.len())),
            }
        })?
    }
    fn set_menu_data(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        data: Vec<i16>,
    ) -> Result<(), String> {
        if data.len() != 4 {
            return Err(format!("{} values: a furnace window has 4", data.len()));
        }
        self.with_own_storage(&dimension, (x, y, z), |s| s.menu_data = data)
    }
    fn set_slot_rule(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
        rule: Option<super::server_bindings::horizon::mods::storages::SlotRule>,
    ) -> Result<(), String> {
        self.with_own_storage(&dimension, (x, y, z), |s| {
            if slot as usize >= s.slots.len() {
                return Err(format!("no slot {slot}"));
            }
            match rule {
                Some(r) => {
                    s.slot_rules.insert(
                        slot,
                        SlotRule {
                            take_only: r.take_only,
                            items: r.items,
                        },
                    );
                }
                None => {
                    s.slot_rules.remove(&slot);
                }
            }
            Ok(())
        })?
    }
    fn open_menu(
        &mut self,
        player: String,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        title: String,
    ) -> Result<(), String> {
        let slots = self
            .with_storage(&dimension, (x, y, z), |s| s.slots.len())
            .ok_or_else(|| format!("no storage at {x} {y} {z}"))?;
        let kind = match slots {
            3 => "minecraft:furnace".to_owned(),
            5 => "minecraft:hopper".to_owned(),
            n if n > 0 && n % 9 == 0 => format!("minecraft:generic_9x{}", n / 9),
            n => return Err(format!("{n} slots: a window needs 3, 5, or rows of 9")),
        };
        self.open_menu_as(player, dimension, x, y, z, title, kind)
    }
    fn open_menu_as(
        &mut self,
        player: String,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        title: String,
        kind: String,
    ) -> Result<(), String> {
        self.online(&player)?;
        let slots = self
            .with_storage(&dimension, (x, y, z), |s| s.slots.len())
            .ok_or_else(|| format!("no storage at {x} {y} {z}"))?;
        let fits = match kind.strip_prefix("minecraft:").unwrap_or_default() {
            "generic_3x3" => 9,
            "shulker_box" => 27,
            "hopper" => 5,
            "furnace" | "blast_furnace" | "smoker" => 3,
            other => match other
                .strip_prefix("generic_9x")
                .and_then(|n| n.parse::<usize>().ok())
            {
                Some(rows @ 1..=6) => rows * 9,
                _ => return Err(format!("{kind}: not a window a mod can open")),
            },
        };
        if slots != fits {
            return Err(format!("{kind} has {fits} slots, the storage {slots}"));
        }
        self.act(ServerAction::OpenMenu {
            player,
            dimension,
            pos: (x, y, z),
            title: super::clip_to(title, 64),
            kind,
        })
    }
    fn has_storage(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        self.with_storage(&dimension, (x, y, z), |_| ()).is_some()
    }
    fn is_empty(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        let slots = storages::Host::items(self, dimension.clone(), x, y, z);
        let energy = storages::Host::energy(self, dimension.clone(), x, y, z);
        let tanks = storages::Host::fluids(self, dimension, x, y, z);
        slots.is_none_or(|s| s.iter().all(Option::is_none))
            && energy.is_none_or(|(stored, _)| stored == 0)
            && tanks.is_none_or(|t| t.iter().all(Option::is_none))
    }
    fn is_full(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        let Some(slots) = storages::Host::items(self, dimension, x, y, z) else {
            return false;
        };
        slots.iter().all(|s| {
            s.as_ref()
                .is_some_and(|s| self.max_stack(&s.item).is_ok_and(|max| s.count >= max))
        })
    }
    fn free_slots(&mut self, dimension: String, x: i32, y: i32, z: i32) -> u32 {
        let slots = storages::Host::items(self, dimension, x, y, z).unwrap_or_default();
        slots.iter().filter(|s| s.is_none()).count() as u32
    }
    fn count_item(&mut self, dimension: String, x: i32, y: i32, z: i32, item: String) -> u32 {
        let slots = storages::Host::items(self, dimension, x, y, z).unwrap_or_default();
        slots
            .iter()
            .flatten()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum()
    }
    fn clear(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Result<(), String> {
        let me = self.id.clone();
        self.with_storage(&dimension, (x, y, z), |s| {
            if s.owner != me {
                return Err("another mod's storage".to_string());
            }
            s.slots.iter_mut().for_each(|slot| *slot = None);
            s.energy = 0;
            s.tanks.iter_mut().for_each(|t| t.fluid = None);
            Ok(())
        })
        .ok_or_else(|| format!("no storage at {x} {y} {z} in {dimension}"))?
    }
    fn positions(&mut self, dimension: String) -> Vec<(i32, i32, i32)> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        all.storages
            .iter()
            .filter(|(_, s)| s.owner == self.id)
            .filter_map(|(key, _)| {
                let mut parts = key.rsplitn(4, ' ');
                let z = parts.next()?.parse().ok()?;
                let y = parts.next()?.parse().ok()?;
                let x = parts.next()?.parse().ok()?;
                (parts.next()? == dimension).then_some((x, y, z))
            })
            .collect()
    }
    fn free_energy(&mut self, dimension: String, x: i32, y: i32, z: i32) -> u64 {
        storages::Host::energy(self, dimension, x, y, z)
            .map_or(0, |(stored, capacity)| capacity.saturating_sub(stored))
    }
    fn fluid_amount(&mut self, dimension: String, x: i32, y: i32, z: i32, fluid: String) -> u32 {
        let tanks = storages::Host::fluids(self, dimension, x, y, z).unwrap_or_default();
        tanks
            .iter()
            .flatten()
            .filter(|t| t.fluid == fluid)
            .map(|t| t.amount)
            .sum()
    }
    fn transfer_item(
        &mut self,
        dimension: String,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        item: Option<String>,
        count: u32,
    ) -> u32 {
        let d = dimension;
        let Some(mut stack) = storages::Host::extract_item(
            self,
            d.clone(),
            from.0,
            from.1,
            from.2,
            item,
            count,
            true,
        ) else {
            return 0;
        };
        let fits =
            storages::Host::insert_item(self, d.clone(), to.0, to.1, to.2, stack.clone(), true);
        if fits == 0 {
            return 0;
        }
        let Some(taken) = storages::Host::extract_item(
            self,
            d.clone(),
            from.0,
            from.1,
            from.2,
            Some(stack.item.clone()),
            fits,
            false,
        ) else {
            return 0;
        };
        stack = taken;
        storages::Host::insert_item(self, d, to.0, to.1, to.2, stack, false)
    }
    fn transfer_energy(
        &mut self,
        dimension: String,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        amount: u64,
    ) -> u64 {
        let d = dimension;
        let can =
            storages::Host::extract_energy(self, d.clone(), from.0, from.1, from.2, amount, true);
        let fits = storages::Host::insert_energy(self, d.clone(), to.0, to.1, to.2, can, true);
        let taken =
            storages::Host::extract_energy(self, d.clone(), from.0, from.1, from.2, fits, false);
        storages::Host::insert_energy(self, d, to.0, to.1, to.2, taken, false)
    }
    fn transfer_fluid(
        &mut self,
        dimension: String,
        from: (i32, i32, i32),
        to: (i32, i32, i32),
        fluid: Option<String>,
        amount: u32,
    ) -> u32 {
        let d = dimension;
        let Some(can) = storages::Host::extract_fluid(
            self,
            d.clone(),
            from.0,
            from.1,
            from.2,
            fluid,
            amount,
            true,
        ) else {
            return 0;
        };
        let fits =
            storages::Host::insert_fluid(self, d.clone(), to.0, to.1, to.2, can.clone(), true);
        if fits == 0 {
            return 0;
        }
        let Some(taken) = storages::Host::extract_fluid(
            self,
            d.clone(),
            from.0,
            from.1,
            from.2,
            Some(can.fluid),
            fits,
            false,
        ) else {
            return 0;
        };
        storages::Host::insert_fluid(self, d, to.0, to.1, to.2, taken, false)
    }
}

// ---------------------------------------------------------------- entities
impl entities::Host for State {
    fn near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<entities::Entity> {
        let r = radius.clamp(0.0, 128.0);
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        let Some(view) = view.as_ref() else {
            return Vec::new();
        };
        let d2 =
            |e: &EntityInfo| (e.pos.0 - x).powi(2) + (e.pos.1 - y).powi(2) + (e.pos.2 - z).powi(2);
        let mut near: Vec<(f64, EntityInfo)> = view
            .mobs(&dimension)
            .into_iter()
            .map(|e| (d2(&e), e))
            .filter(|(d, _)| *d <= r * r)
            .collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.into_iter()
            .take(256)
            .map(|(_, e)| entities::Entity {
                id: e.id,
                kind: e.kind,
                x: e.pos.0,
                y: e.pos.1,
                z: e.pos.2,
                health: e.health,
            })
            .collect()
    }
    fn spawn(
        &mut self,
        dimension: String,
        kind: String,
        x: f64,
        y: f64,
        z: f64,
        nbt: String,
    ) -> Result<(), String> {
        self.check_dimension(&dimension)?;
        check_id("entity", &kind)?;
        let nbt = nbt.trim();
        // A mod's own type: the console's `/summon` knows vanilla's.
        if !kind.starts_with("minecraft:") && nbt.is_empty() {
            return self.act(ServerAction::SpawnMob {
                uuid: None,
                reason: None,
                dimension,
                kind,
                pos: (x, y, z),
            });
        }
        self.console(
            format!("execute in {dimension} run summon {kind} {x} {y} {z} {nbt}")
                .trim_end()
                .to_owned(),
        )
    }
    fn get_data(&mut self, id: String, key: String) -> Option<Vec<u8>> {
        players::Host::get_data(self, id, key)
    }
    fn set_data(&mut self, id: String, key: String, data: Vec<u8>) -> Result<(), String> {
        check_uuid(&id)?;
        players::Host::set_data(self, id, key, data)
    }
    fn set_ai(&mut self, id: String, on: bool) -> Result<(), String> {
        check_uuid(&id)?;
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Ai(on),
        })
    }
    fn set_name(&mut self, id: String, name: Option<String>) -> Result<(), String> {
        check_uuid(&id)?;
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Name(name.map(|n| super::clip_to(n, 64))),
        })
    }
    fn set_rotation(&mut self, id: String, yaw: f32, pitch: f32) -> Result<(), String> {
        check_uuid(&id)?;
        if !yaw.is_finite() || !pitch.is_finite() {
            return Err("a finite yaw and pitch".into());
        }
        self.console(format!("rotate {id} {yaw} {pitch}"))
    }
    fn move_to(&mut self, id: String, x: f64, y: f64, z: f64, speed: f64) -> Result<(), String> {
        check_uuid(&id)?;
        if ![x, y, z, speed].iter().all(|v| v.is_finite()) || !(0.0..=8.0).contains(&speed) {
            return Err("a finite position, and a speed of 0 to 8".into());
        }
        self.act(ServerAction::Mob {
            id,
            change: MobChange::MoveTo(x, y, z, speed),
        })
    }
    fn set_target(&mut self, id: String, target: Option<String>) -> Result<(), String> {
        check_uuid(&id)?;
        if let Some(t) = &target {
            check_uuid(t)?;
        }
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Target(target),
        })
    }
    fn set_equipment(
        &mut self,
        id: String,
        slot: String,
        stack: players::ItemStack,
    ) -> Result<(), String> {
        check_uuid(&id)?;
        let clean = |s: &str| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.:/".contains(c))
        };
        if !clean(&slot) || !clean(&stack.item) || stack.item.is_empty() {
            return Err("a slot and an item id".into());
        }
        self.console(format!(
            "item replace entity {id} {slot} with {} {}",
            stack.item, stack.count
        ))
    }
    fn set_velocity(&mut self, id: String, x: f64, y: f64, z: f64) -> Result<(), String> {
        check_uuid(&id)?;
        if ![x, y, z].iter().all(|v| v.is_finite() && v.abs() <= 16.0) {
            return Err("motion: finite, at most 16 blocks per tick".into());
        }
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Velocity(x, y, z),
        })
    }
    fn drop_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        stack: players::ItemStack,
    ) -> Result<(), String> {
        let stack = from_guest(stack, &self.id)?;
        self.act(ServerAction::DropItem {
            dimension,
            pos: (x, y, z),
            stack,
        })
    }
    fn set_attribute(&mut self, id: String, attribute: String, value: f64) -> Result<(), String> {
        check_uuid(&id)?;
        if !value.is_finite() || attribute.contains(char::is_whitespace) {
            return Err("an attribute id and a finite value".into());
        }
        self.console(format!("attribute {id} {attribute} base set {value}"))
    }
    fn hurt(&mut self, id: String, amount: f32) -> Result<(), String> {
        check_uuid(&id)?;
        if !(amount.is_finite() && amount > 0.0) {
            return Err("amount: more than 0".into());
        }
        self.console(format!("damage {id} {amount}"))
    }
    fn kill(&mut self, id: String) -> Result<(), String> {
        check_uuid(&id)?;
        self.console(format!("kill {id}"))
    }
    fn teleport(
        &mut self,
        id: String,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<(), String> {
        check_uuid(&id)?;
        self.check_dimension(&dimension)?;
        self.console(format!("execute in {dimension} run tp {id} {x} {y} {z}"))
    }
    fn effect(
        &mut self,
        id: String,
        effect: String,
        seconds: u32,
        amplifier: u8,
    ) -> Result<(), String> {
        check_uuid(&id)?;
        check_id("effect", &effect)?;
        let eff_id = crate::ResourceLocation::parse(&effect)
            .unwrap_or_else(|_| crate::ResourceLocation::parse("minecraft:effect").unwrap());
        let def = crate::MobEffectDefinition::new(eff_id, &effect, true);
        self.mob_effects.apply(
            &id,
            &def,
            i32::from(amplifier),
            (seconds.clamp(1, 1_000_000) as i32) * 20,
        );
        self.console(format!(
            "effect give {id} {effect} {} {amplifier}",
            seconds.clamp(1, 1_000_000)
        ))
    }
    fn particle(
        &mut self,
        dimension: String,
        particle: String,
        x: f64,
        y: f64,
        z: f64,
        dx: f32,
        dy: f32,
        dz: f32,
        speed: f32,
        count: u32,
    ) -> Result<(), String> {
        self.check_dimension(&dimension)?;
        check_id("particle", &particle)?;
        let count = count.min(10_000);
        self.console(format!(
            "execute in {dimension} run particle {particle} {x} {y} {z} {dx} {dy} {dz} {speed} {count}"
        ))
    }
    fn sound(
        &mut self,
        dimension: String,
        sound: String,
        x: f64,
        y: f64,
        z: f64,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        self.check_dimension(&dimension)?;
        check_id("sound", &sound)?;
        let (volume, pitch) = (volume.clamp(0.0, 16.0), pitch.clamp(0.5, 2.0));
        self.console(format!(
            "execute in {dimension} run playsound {sound} master @a {x} {y} {z} {volume} {pitch}"
        ))
    }
    fn get(&mut self, dimension: String, id: String) -> Option<entities::Entity> {
        let view = self.world.lock().ok()?;
        let e = view
            .as_ref()?
            .mobs(&dimension)
            .into_iter()
            .find(|e| e.id == id)?;
        Some(entities::Entity {
            id: e.id,
            kind: e.kind,
            x: e.pos.0,
            y: e.pos.1,
            z: e.pos.2,
            health: e.health,
        })
    }
    fn exists(&mut self, dimension: String, id: String) -> bool {
        entities::Host::get(self, dimension, id).is_some()
    }
    fn of_kind(
        &mut self,
        dimension: String,
        kind: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<entities::Entity> {
        let mut near = entities::Host::near(self, dimension, x, y, z, radius);
        near.retain(|e| e.kind == kind);
        near
    }
    fn nearest(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        kind: Option<String>,
    ) -> Option<entities::Entity> {
        entities::Host::near(self, dimension, x, y, z, radius)
            .into_iter()
            .find(|e| kind.as_ref().is_none_or(|k| *k == e.kind))
    }
    fn count_near(&mut self, dimension: String, x: f64, y: f64, z: f64, radius: f64) -> u32 {
        entities::Host::near(self, dimension, x, y, z, radius).len() as u32
    }
    fn clear_effects(&mut self, id: String) -> Result<(), String> {
        check_uuid(&id)?;
        self.console(format!("effect clear {id}"))
    }
    fn remove_effect(&mut self, id: String, effect: String) -> Result<(), String> {
        check_uuid(&id)?;
        check_id("effect", &effect)?;
        self.console(format!("effect clear {id} {effect}"))
    }
    fn heal(&mut self, id: String, amount: f32) -> Result<(), String> {
        check_uuid(&id)?;
        if !(amount.is_finite() && amount > 0.0) {
            return Err("amount: more than 0".into());
        }
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Heal(amount),
        })
    }
    fn set_health(&mut self, id: String, health: f32) -> Result<(), String> {
        check_uuid(&id)?;
        if !(health.is_finite() && health > 0.0) {
            return Err("health: more than 0; `kill` ends a mob".into());
        }
        self.act(ServerAction::Mob {
            id,
            change: MobChange::Health(health),
        })
    }
    fn swing(&mut self, id: String) -> Result<(), String> {
        check_uuid(&id)?;
        self.console(format!("swing {id}"))
    }
    fn kind(&mut self, id: String) -> Option<String> {
        if self.find_player(&id).is_some() {
            return Some("minecraft:player".into());
        }
        Some(self.find_mob(&id)?.1.kind)
    }
    fn position(&mut self, id: String) -> Option<(f64, f64, f64)> {
        Some(self.whereabouts(&id)?.1)
    }
    fn rotation(&mut self, id: String) -> Option<(f32, f32)> {
        let m = self.mob_state(&id)?;
        Some((m.yaw, m.pitch))
    }
    fn velocity(&mut self, id: String) -> Option<(f64, f64, f64)> {
        Some(self.mob_state(&id)?.velocity)
    }
    fn health(&mut self, id: String) -> Option<f32> {
        if let Some(p) = self.find_player(&id) {
            return Some(p.health);
        }
        Some(self.find_mob(&id)?.1.health)
    }
    fn max_health(&mut self, id: String) -> Option<f32> {
        Some(self.mob_state(&id)?.max_health)
    }
    fn dimension(&mut self, id: String) -> Option<String> {
        Some(self.whereabouts(&id)?.0)
    }
    fn is_alive(&mut self, id: String) -> bool {
        if let Some(p) = self.find_player(&id) {
            return p.health > 0.0;
        }
        self.find_mob(&id).is_some_and(|(_, e)| e.health > 0.0)
    }
    fn is_on_ground(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.on_ground)
    }
    fn is_in_water(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.in_water)
    }
    fn is_in_lava(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.in_lava)
    }
    fn is_burning(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.burning)
    }
    fn is_baby(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.baby)
    }
    fn set_baby(&mut self, id: String, baby: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Baby(baby))
    }
    fn is_tamed(&mut self, id: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.tamed)
    }
    fn owner(&mut self, id: String) -> Option<String> {
        self.mob_state(&id)?.owner
    }
    fn set_owner(&mut self, id: String, player: Option<String>) -> Result<(), String> {
        if let Some(p) = &player {
            check_uuid(p)?;
        }
        self.change_mob(id, MobChange::Owner(player))
    }
    fn custom_name(&mut self, id: String) -> Option<String> {
        self.mob_state(&id)?.custom_name
    }
    fn effects(&mut self, id: String) -> Vec<entities::EffectState> {
        self.mob_state(&id)
            .map_or_else(Vec::new, |m| m.effects)
            .into_iter()
            .map(|(effect, duration, amplifier)| entities::EffectState {
                effect,
                duration,
                amplifier,
            })
            .collect()
    }
    fn has_effect(&mut self, id: String, effect: String) -> bool {
        self.mob_state(&id)
            .is_some_and(|m| m.effects.iter().any(|e| e.0 == effect))
    }
    fn attribute(&mut self, id: String, attribute: String) -> Option<f64> {
        let m = self.mob_state(&id)?;
        m.attributes.iter().find(|a| a.0 == attribute).map(|a| a.1)
    }
    fn base_attribute(&mut self, id: String, attribute: String) -> Option<f64> {
        let m = self.mob_state(&id)?;
        m.attributes.iter().find(|a| a.0 == attribute).map(|a| a.2)
    }
    fn equipment(&mut self, id: String) -> Vec<entities::Equipped> {
        self.mob_state(&id)
            .map_or_else(Vec::new, |m| m.equipment)
            .into_iter()
            .map(|(slot, item)| entities::Equipped { slot, item })
            .collect()
    }
    fn target(&mut self, id: String) -> Option<String> {
        self.mob_state(&id)?.target
    }
    fn passengers(&mut self, id: String) -> Vec<String> {
        self.mob_state(&id).map_or_else(Vec::new, |m| m.passengers)
    }
    fn vehicle(&mut self, id: String) -> Option<String> {
        self.mob_state(&id)?.vehicle
    }
    fn ride(&mut self, id: String, vehicle: String) -> Result<(), String> {
        check_uuid(&id)?;
        check_uuid(&vehicle)?;
        self.console(format!("ride {id} mount {vehicle}"))
    }
    fn dismount(&mut self, id: String) -> Result<(), String> {
        check_uuid(&id)?;
        self.console(format!("ride {id} dismount"))
    }
    fn remove(&mut self, id: String) -> Result<(), String> {
        self.change_mob(id, MobChange::Remove)
    }
    fn set_fire(&mut self, id: String, ticks: u32) -> Result<(), String> {
        self.change_mob(id, MobChange::Fire(ticks.min(72_000)))
    }
    fn extinguish(&mut self, id: String) -> Result<(), String> {
        self.change_mob(id, MobChange::Fire(0))
    }
    fn set_glowing(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::Glowing, on))
    }
    fn set_invisible(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::Invisible, on))
    }
    fn set_invulnerable(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::Invulnerable, on))
    }
    fn set_silent(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::Silent, on))
    }
    fn set_no_gravity(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::NoGravity, on))
    }
    fn set_persistent(&mut self, id: String, on: bool) -> Result<(), String> {
        self.change_mob(id, MobChange::Flag(MobFlag::Persistent, on))
    }
    fn set_pickup_delay(&mut self, id: String, ticks: u32) -> Result<(), String> {
        check_uuid(&id)?;
        self.act(ServerAction::PickupDelay {
            id,
            ticks: ticks.min(32_767) as i32,
        })
    }
    fn add_tag(&mut self, id: String, tag: String) -> Result<(), String> {
        check_uuid(&id)?;
        super::mobs::check_tag(&tag)?;
        self.console(format!("tag {id} add {tag}"))
    }
    fn remove_tag(&mut self, id: String, tag: String) -> Result<(), String> {
        check_uuid(&id)?;
        super::mobs::check_tag(&tag)?;
        self.console(format!("tag {id} remove {tag}"))
    }
    fn tags(&mut self, id: String) -> Vec<String> {
        self.mob_state(&id).map_or_else(Vec::new, |m| m.tags)
    }
    fn has_tag(&mut self, id: String, tag: String) -> bool {
        self.mob_state(&id).is_some_and(|m| m.tags.contains(&tag))
    }
    fn merge_nbt(&mut self, id: String, nbt: String) -> Result<(), String> {
        check_uuid(&id)?;
        super::mobs::check_snbt(&nbt)?;
        self.console(format!("data merge entity {id} {}", nbt.trim()))
    }
    fn look_at(&mut self, id: String, x: f64, y: f64, z: f64) -> Result<(), String> {
        check_uuid(&id)?;
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("a finite position".into());
        }
        let (_, at) = self.whereabouts(&id).ok_or("no such entity")?;
        // Entity.lookAt from the eyes, about 1.6 blocks up for most mobs.
        let (dx, dy, dz) = (x - at.0, y - (at.1 + 1.62), z - at.2);
        let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
        let pitch = (-dy.atan2((dx * dx + dz * dz).sqrt()).to_degrees()) as f32;
        entities::Host::set_rotation(self, id, yaw, pitch)
    }
    fn knockback(
        &mut self,
        id: String,
        strength: f64,
        from_x: f64,
        from_z: f64,
    ) -> Result<(), String> {
        if !(strength.is_finite() && from_x.is_finite() && from_z.is_finite()) {
            return Err("finite numbers".into());
        }
        self.change_mob(
            id,
            MobChange::Knockback(strength.clamp(0.0, 10.0), from_x, from_z),
        )
    }
    fn add_velocity(&mut self, id: String, x: f64, y: f64, z: f64) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("a finite motion".into());
        }
        let c = |v: f64| v.clamp(-10.0, 10.0);
        self.change_mob(id, MobChange::AddVelocity(c(x), c(y), c(z)))
    }
    fn distance(&mut self, a: String, b: String) -> Option<f64> {
        let (da, pa) = self.whereabouts(&a)?;
        let (db, pb) = self.whereabouts(&b)?;
        (da == db)
            .then(|| ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2) + (pa.2 - pb.2).powi(2)).sqrt())
    }
    fn in_box(
        &mut self,
        dimension: String,
        x1: f64,
        y1: f64,
        z1: f64,
        x2: f64,
        y2: f64,
        z2: f64,
    ) -> Vec<entities::Entity> {
        let (lo, hi) = (
            (x1.min(x2), y1.min(y2), z1.min(z2)),
            (x1.max(x2), y1.max(y2), z1.max(z2)),
        );
        self.mobs_of(&dimension)
            .into_iter()
            .filter(|e| {
                (lo.0..=hi.0).contains(&e.pos.0)
                    && (lo.1..=hi.1).contains(&e.pos.1)
                    && (lo.2..=hi.2).contains(&e.pos.2)
            })
            .take(256)
            .map(|e| super::events::guest_entity(&e))
            .collect()
    }
    fn all(&mut self, dimension: String) -> Vec<entities::Entity> {
        self.mobs_of(&dimension)
            .into_iter()
            .take(4096)
            .map(|e| super::events::guest_entity(&e))
            .collect()
    }
    fn items_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<entities::Dropped> {
        let r = radius.clamp(0.0, 64.0);
        let d2 = |p: (f64, f64, f64)| (p.0 - x).powi(2) + (p.1 - y).powi(2) + (p.2 - z).powi(2);
        let mut near: Vec<_> = self
            .dropped_near(&dimension)
            .into_iter()
            .filter(|d| d2(d.pos) <= r * r)
            .collect();
        near.sort_by(|a, b| d2(a.pos).total_cmp(&d2(b.pos)));
        near.into_iter()
            .take(256)
            .map(|d| entities::Dropped {
                id: d.id,
                x: d.pos.0,
                y: d.pos.1,
                z: d.pos.2,
                stack: to_guest(&d.stack, &self.id),
            })
            .collect()
    }
    fn xp_orb(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        amount: u32,
    ) -> Result<(), String> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("a finite position".into());
        }
        if amount == 0 {
            return Ok(());
        }
        self.act(ServerAction::XpOrb {
            dimension,
            pos: (x, y, z),
            amount: amount.min(32_767),
        })
    }
    fn spawn_with_reason(
        &mut self,
        dimension: String,
        kind: String,
        x: f64,
        y: f64,
        z: f64,
        reason: String,
    ) -> Result<String, String> {
        super::check_game_id("entity type", &kind)?;
        super::mobs::check_reason(&reason)?;
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err("a finite position".into());
        }
        let uuid = {
            use std::hash::{BuildHasher, Hasher};
            let mut words = [0u32; 4];
            for w in &mut words {
                *w = std::collections::hash_map::RandomState::new()
                    .build_hasher()
                    .finish() as u32;
            }
            // A version 4 UUID, as `UUID.randomUUID`.
            words[1] = (words[1] & 0xFFFF_0FFF) | 0x0000_4000;
            words[2] = (words[2] & 0x3FFF_FFFF) | 0x8000_0000;
            format!(
                "{:08x}-{:04x}-{:04x}-{:04x}-{:04x}{:08x}",
                words[0],
                words[1] >> 16,
                words[1] & 0xFFFF,
                words[2] >> 16,
                words[2] & 0xFFFF,
                words[3]
            )
        };
        self.act(ServerAction::SpawnMob {
            dimension,
            kind,
            pos: (x, y, z),
            uuid: Some(uuid.clone()),
            reason: Some(reason),
        })?;
        Ok(uuid)
    }
    fn spawn_group(
        &mut self,
        dimension: String,
        kind: String,
        positions: Vec<(f64, f64, f64)>,
        reason: String,
    ) -> Result<Vec<String>, String> {
        if positions.len() > 64 {
            return Err("at most 64 at once".into());
        }
        positions
            .into_iter()
            .map(|(x, y, z)| {
                entities::Host::spawn_with_reason(
                    self,
                    dimension.clone(),
                    kind.clone(),
                    x,
                    y,
                    z,
                    reason.clone(),
                )
            })
            .collect()
    }
}

// ---------------------------------------------------------------- the server's side
impl Mods {
    fn index_of(&self, id: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.run.is_some() && e.manifest.as_ref().is_some_and(|m| m.id == id))
    }

    /// Asks the running mods' `block-events` export: `f` for each that has it.
    fn ask_blocks<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(
            &server_bindings_block::Guest,
            &mut wasmtime::Store<State>,
        ) -> wasmtime::Result<R>,
    ) -> Option<R> {
        if !self.asks_blocks(i) {
            return None;
        }
        self.call(i, FUEL_EVENT, |run| {
            let Running { blocks, store, .. } = run;
            match blocks {
                Some(b) => f(b.horizon_mods_block_events(), store),
                None => Err(wasmtime::Error::msg("no block-events")),
            }
        })
    }

    /// `player` is about to place `item` at `pos`: true when a mod refuses.
    pub fn place_block(
        &mut self,
        player: &players::Player,
        pos: super::BlockPos,
        item: &str,
    ) -> bool {
        let mut refused = false;
        for i in 0..self.entries.len() {
            refused |= self
                .ask_blocks(i, |b, s| {
                    b.call_place_block(s, player, pos.x, pos.y, pos.z, item)
                })
                .unwrap_or(false);
        }
        refused
    }
    /// `player` right-clicks the air holding `stack`: true when a mod took it.
    pub fn use_item(
        &mut self,
        player: &players::Player,
        stack: &HostStack,
        off_hand: bool,
    ) -> bool {
        for i in 0..self.entries.len() {
            let Some(id) = self.entries[i].manifest.as_ref().map(|m| m.id.clone()) else {
                continue;
            };
            let guest = to_guest(stack, &id);
            if self
                .ask_blocks(i, |b, s| b.call_use_item(s, player, &guest, off_hand))
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }

    /// Every position in `dimension` where a server mod keeps data or a
    /// storage: the game works out their redstone power before lending.
    pub fn machine_positions(&self, dimension: &str) -> Vec<(i32, i32, i32)> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        let keys = all
            .mods
            .values()
            .flat_map(|m| m.keys())
            .chain(all.storages.keys())
            .chain(all.outputs.keys());
        let mut out: Vec<(i32, i32, i32)> = keys.filter_map(|k| parse_key(k, dimension)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Every block giving redstone in `dimension`, with what it was set on:
    /// `(pos, block, signal, comparator)`.
    pub fn redstone_outputs(&self, dimension: &str) -> Vec<((i32, i32, i32), String, u8, u8)> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        let running = |id: &str| {
            self.entries
                .iter()
                .any(|e| e.run.is_some() && e.manifest.as_ref().is_some_and(|m| m.id == id))
        };
        all.outputs
            .iter()
            .filter(|(_, o)| running(&o.owner))
            .filter_map(|(key, o)| {
                let mut w = key.rsplitn(4, ' ');
                let z = w.next()?.parse().ok()?;
                let y = w.next()?.parse().ok()?;
                let x = w.next()?.parse().ok()?;
                (w.next()? == dimension)
                    .then(|| ((x, y, z), o.block.clone(), o.signal, o.comparator))
            })
            .collect()
    }
    /// The block the output was set on is gone.
    pub fn remove_redstone_output(&mut self, dimension: &str, pos: (i32, i32, i32)) {
        if let Ok(mut all) = self.block_data.lock() {
            all.outputs
                .remove(&block_key(dimension, pos.0, pos.1, pos.2));
        }
    }
    /// Everything `world.set-locked-results` closes.
    pub fn locked_results(&self) -> std::collections::BTreeSet<String> {
        let Ok(all) = self.block_data.lock() else {
            return Default::default();
        };
        all.locked.values().flatten().cloned().collect()
    }
    /// The storages of `dimension` whose block `here` says is gone (none: its
    /// chunk isn't loaded), taken out with what they held for the game to drop.
    pub fn take_gone_storages(
        &mut self,
        dimension: &str,
        here: impl Fn((i32, i32, i32)) -> Option<String>,
    ) -> Vec<((i32, i32, i32), Vec<HostStack>)> {
        let Ok(mut all) = self.block_data.lock() else {
            return Vec::new();
        };
        let mut gone = Vec::new();
        all.storages.retain(|key, s| {
            let mut w = key.rsplitn(4, ' ');
            let pos = (|| {
                let z = w.next()?.parse().ok()?;
                let y = w.next()?.parse().ok()?;
                let x = w.next()?.parse().ok()?;
                (w.next()? == dimension).then_some((x, y, z))
            })();
            let Some(pos) = pos else {
                return true;
            };
            match here(pos) {
                Some(block) if block != s.block => {
                    gone.push((pos, s.slots.iter().flatten().cloned().collect()));
                    false
                }
                _ => true,
            }
        });
        gone
    }
    /// Whether a running mod holds this capability.
    pub fn any_running_with(&self, capability: crate::Capability) -> bool {
        self.entries.iter().any(|e| {
            e.run.is_some()
                && e.manifest
                    .as_ref()
                    .is_some_and(|m| m.capabilities.contains(&capability))
        })
    }
    /// A storage's furnace bars, for its window.
    /// A storage's slot rules, for a window the game shows.
    pub fn storage_slot_rules(
        &self,
        dimension: &str,
        pos: (i32, i32, i32),
    ) -> std::collections::BTreeMap<u32, SlotRule> {
        let Ok(all) = self.block_data.lock() else {
            return Default::default();
        };
        all.storages
            .get(&block_key(dimension, pos.0, pos.1, pos.2))
            .map(|s| s.slot_rules.clone())
            .unwrap_or_default()
    }
    pub fn storage_menu_data(&self, dimension: &str, pos: (i32, i32, i32)) -> Vec<i16> {
        let Ok(all) = self.block_data.lock() else {
            return Vec::new();
        };
        all.storages
            .get(&block_key(dimension, pos.0, pos.1, pos.2))
            .map(|s| s.menu_data.clone())
            .unwrap_or_default()
    }
    /// A storage's slots, for a window the game shows.
    pub fn storage_slots(
        &self,
        dimension: &str,
        pos: (i32, i32, i32),
    ) -> Option<Vec<Option<HostStack>>> {
        let all = self.block_data.lock().ok()?;
        all.storages
            .get(&block_key(dimension, pos.0, pos.1, pos.2))
            .map(|s| s.slots.clone())
    }
    /// What a player did in a storage's window, back into it (same size).
    pub fn set_storage_slots(
        &mut self,
        dimension: &str,
        pos: (i32, i32, i32),
        slots: Vec<Option<HostStack>>,
    ) {
        if let Ok(mut all) = self.block_data.lock()
            && let Some(s) = all
                .storages
                .get_mut(&block_key(dimension, pos.0, pos.1, pos.2))
            && s.slots.len() == slots.len()
        {
            s.slots = slots;
        }
    }

    /// Before the tick hook: the timers that are due, then the neighbour
    /// changes around the mods' blocks (the world is lent).
    pub(super) fn block_hooks(&mut self, tick: u64) {
        let due = {
            let Ok(mut all) = self.block_data.lock() else {
                return;
            };
            if !all.synced {
                // A world just opened: its timers count from its saved clock.
                let shift = tick as i128 - all.now as i128;
                for t in &mut all.ticks {
                    t.due = (t.due as i128 + shift).max(0) as u64;
                }
                all.synced = true;
            }
            all.now = tick;
            if all.ticks.is_empty() {
                Vec::new()
            } else {
                all.ticks.sort_by_key(|t| t.due);
                let n = all
                    .ticks
                    .iter()
                    .take(TIMERS_PER_TICK)
                    .take_while(|t| t.due <= tick)
                    .count();
                all.ticks.drain(..n).collect::<Vec<_>>()
            }
        };
        for t in due {
            let (x, y, z) = t.pos;
            let Some(state) = self.view_block(&t.dimension, x, y, z) else {
                // Its chunk is not loaded: the timer waits, the machine must not stop for good.
                if let Ok(mut all) = self.block_data.lock() {
                    all.ticks.push(ScheduledTick {
                        due: tick + UNLOADED_RETRY,
                        ..t
                    });
                }
                continue;
            };
            if block_id(&state) != t.block {
                continue;
            }
            if let Some(i) = self.index_of(&t.owner) {
                self.ask_blocks(i, |b, s| {
                    b.call_block_tick(s, &t.dimension, x, y, z, &state)
                });
            }
        }
        self.neighbor_hooks();
    }

    fn view_block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.block(dimension, x, y, z)
    }

    /// Compares each mod block's neighbours with last tick's.
    fn neighbor_hooks(&mut self) {
        // Owners asked about blocks, by id.
        let asking: Vec<String> = (0..self.entries.len())
            .filter(|&i| self.asks_blocks(i))
            .filter_map(|i| self.entries[i].manifest.as_ref().map(|m| m.id.clone()))
            .collect();
        if asking.is_empty() {
            return;
        }
        let watched: Vec<(String, Vec<String>)> = {
            let Ok(all) = self.block_data.lock() else {
                return;
            };
            let mut by_key: BTreeMap<&str, Vec<String>> = BTreeMap::new();
            for (id, keys) in &all.mods {
                if asking.contains(id) {
                    for k in keys.keys() {
                        by_key.entry(k).or_default().push(id.clone());
                    }
                }
            }
            for (k, s) in &all.storages {
                if asking.contains(&s.owner) {
                    let owners = by_key.entry(k).or_default();
                    if !owners.contains(&s.owner) {
                        owners.push(s.owner.clone());
                    }
                }
            }
            by_key.into_iter().map(|(k, o)| (k.to_owned(), o)).collect()
        };
        let mut events = Vec::new();
        {
            let Ok(view) = self.world.lock() else {
                return;
            };
            let Some(view) = view.as_ref() else {
                return;
            };
            let Ok(mut all) = self.block_data.lock() else {
                return;
            };
            let keys: std::collections::HashSet<&str> =
                watched.iter().map(|(k, _)| k.as_str()).collect();
            all.neighbors.retain(|k, _| keys.contains(k.as_str()));
            for (key, owners) in &watched {
                let Some((dim, (x, y, z))) = split_key(key) else {
                    continue;
                };
                let Some(state) = view.block(dim, x, y, z) else {
                    continue;
                };
                let around: [Option<u64>; 6] = NEIGHBORS
                    .map(|(dx, dy, dz)| view.block(dim, x + dx, y + dy, z + dz).map(|s| hash(&s)));
                if let Some(before) = all.neighbors.insert(key.clone(), around) {
                    for (n, (dx, dy, dz)) in NEIGHBORS.into_iter().enumerate() {
                        if before[n].is_some() && around[n].is_some() && before[n] != around[n] {
                            for o in owners {
                                events.push((
                                    o.clone(),
                                    dim.to_owned(),
                                    (x, y, z),
                                    state.clone(),
                                    (x + dx, y + dy, z + dz),
                                ));
                            }
                        }
                    }
                }
            }
        }
        for (owner, dim, (x, y, z), state, (fx, fy, fz)) in events {
            if let Some(i) = self.index_of(&owner) {
                self.ask_blocks(i, |b, s| {
                    b.call_neighbor_changed(s, &dim, x, y, z, &state, fx, fy, fz)
                });
            }
        }
    }
}

use super::block_bindings::exports::horizon::mods::block_events as server_bindings_block;

const NEIGHBORS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// `"dimension x y z"` → its parts.
fn split_key(key: &str) -> Option<(&str, (i32, i32, i32))> {
    let mut w = key.rsplitn(4, ' ');
    let z = w.next()?.parse().ok()?;
    let y = w.next()?.parse().ok()?;
    let x = w.next()?.parse().ok()?;
    Some((w.next()?, (x, y, z)))
}
fn parse_key(key: &str, dimension: &str) -> Option<(i32, i32, i32)> {
    split_key(key)
        .filter(|(d, _)| *d == dimension)
        .map(|(_, p)| p)
}

/// Neighbour states last seen around a mod's block (hashes, in
/// [`NEIGHBORS`] order; none where no chunk was loaded).
pub(super) type NeighborCache = std::collections::HashMap<String, [Option<u64>; 6]>;
