//! Server mods' items, storages on blocks, block timers and entities
//! (API level 10): the stacks they hand around, what the server keeps for
//! them beside their bytes on blocks, and the host side of `storages`,
//! `entities` and the item half of `players`.
use super::{
    FUEL_EVENT, Mods, Output, Running, State, block_id, block_key, server_bindings,
};
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
    Give { player: String, stack: HostStack },
    SetSlot { player: String, slot: u32, stack: Option<HostStack> },
    Take { player: String, item: String, count: u32 },
    /// A vanilla command line, run as the server console.
    Console { line: String },
    OpenMenu {
        player: String,
        dimension: String,
        pos: (i32, i32, i32),
        title: String,
    },
    /// `server-net.send` (one player) or `broadcast` (none: everyone).
    Net {
        player: Option<String>,
        channel: String,
        data: Vec<u8>,
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
        return Err(format!("{} bytes on a stack: at most {STACK_DATA_BYTES}", stack.data.len()));
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
    t.fluid.as_ref().map(|(fluid, amount)| storages::FluidStack {
        fluid: fluid.clone(),
        amount: *amount,
    })
}

/// Moves `stack` into `slots` as a hopper does; how many went in.
fn insert_into(slots: &mut [Option<HostStack>], stack: &HostStack, max: u32, simulate: bool) -> u32 {
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
                none => *none = Some(HostStack { count: n, ..stack.clone() }),
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
    let out = HostStack { count: n, ..s.clone() };
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

fn extract_fluid(tanks: &mut [Tank], fluid: Option<&str>, amount: u32, simulate: bool) -> Option<(String, u32)> {
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
    if ok { Ok(()) } else { Err(format!("{kind} {id:?}: not an id")) }
}
fn check_uuid(id: &str) -> Result<(), String> {
    let ok = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if ok { Ok(()) } else { Err(format!("{id:?}: not a UUID")) }
}

impl State {
    /// Queues a change for the game; Err past the per-call limit.
    fn act(&mut self, action: ServerAction) -> Result<(), String> {
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
            Some(view) if !view.has_dimension(dimension) => Err(format!("no dimension {dimension}")),
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
    fn console(&mut self, line: String) -> Result<(), String> {
        if line.contains(['\n', '\r']) {
            return Err("one line only".into());
        }
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
        match self.with_storage(dimension, pos, |s| (s.owner == me).then(|| (f.take().unwrap())(s))) {
            Some(Some(r)) => Ok(r),
            Some(None) => Err("another mod's storage".into()),
            None => Err(format!("no storage at {} {} {}", pos.0, pos.1, pos.2)),
        }
    }
    /// A vanilla container's slots there.
    fn container(&self, dimension: &str, (x, y, z): (i32, i32, i32)) -> Option<Vec<Option<HostStack>>> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.container(dimension, x, y, z)
    }
    fn set_container(&self, dimension: &str, (x, y, z): (i32, i32, i32), slots: Vec<Option<HostStack>>) -> bool {
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
        self.snapshot.inventories.iter().find(|(id, ..)| id == player)
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
    pub(super) fn player_give(&mut self, player: String, stack: players::ItemStack) -> Result<(), String> {
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
        self.act(ServerAction::SetSlot { player, slot, stack })
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
        if n == 0 || self.act(ServerAction::Take { player, item, count: n }).is_err() {
            return 0;
        }
        n
    }
}

// ---------------------------------------------------------------- world
impl State {
    pub(super) fn schedule_tick(&mut self, dimension: String, x: i32, y: i32, z: i32, delay: u32) -> Result<(), String> {
        if !(1..=MAX_DELAY).contains(&delay) {
            return Err(format!("delay {delay}: 1 to {MAX_DELAY} ticks"));
        }
        let block = self
            .block_at(&dimension, x, y, z)
            .ok_or_else(|| format!("no chunk loaded at {x} {y} {z} in {dimension}"))?;
        let me = self.id.clone();
        let mut all = self.block_data.lock().map_err(|_| "block data unavailable".to_string())?;
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
    pub(super) fn redstone_power(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
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
        let mut all = self.block_data.lock().map_err(|_| "block data unavailable".to_string())?;
        let old = all.storages.remove(&key).filter(|s| s.block == block);
        if let Some(s) = &old
            && s.owner != me
        {
            let s = s.clone();
            all.storages.insert(key, s);
            return Err("another mod's storage is there".into());
        }
        if old.is_none() && all.storages.values().filter(|s| s.owner == me).count() >= STORAGES_PER_MOD {
            return Err(format!("at most {STORAGES_PER_MOD} storages per mod"));
        }
        let old = old.unwrap_or_default();
        let mut slots = old.slots;
        slots.resize(shape.slots as usize, None);
        let mut tanks: Vec<super::Tank> = shape
            .tanks
            .iter()
            .map(|&capacity| super::Tank { fluid: None, capacity })
            .collect();
        for (t, o) in tanks.iter_mut().zip(old.tanks) {
            t.fluid = o.fluid.map(|(f, a)| (f, a.min(t.capacity))).filter(|(_, a)| *a > 0);
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
    fn shape(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<storages::StorageShape> {
        self.with_storage(&dimension, (x, y, z), |s| storages::StorageShape {
            slots: s.slots.len() as u32,
            energy_capacity: s.energy_capacity,
            tanks: s.tanks.iter().map(|t| t.capacity).collect(),
        })
        .or_else(|| {
            self.container(&dimension, (x, y, z)).map(|c| storages::StorageShape {
                slots: c.len() as u32,
                energy_capacity: 0,
                tanks: Vec::new(),
            })
        })
    }
    fn items(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<Vec<Option<players::ItemStack>>> {
        let me = self.id.clone();
        let slots = self
            .with_storage(&dimension, (x, y, z), |s| s.slots.clone())
            .or_else(|| self.container(&dimension, (x, y, z)))?;
        Some(slots.iter().map(|s| s.as_ref().map(|s| to_guest(s, &me))).collect())
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
        self.with_own_storage(&dimension, (x, y, z), |s| match s.slots.get_mut(slot as usize) {
            Some(at) => {
                *at = stack;
                Ok(())
            }
            None => Err(format!("slot {slot}: the storage has {}", s.slots.len())),
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
        if let Some(n) = self.with_storage(&dimension, (x, y, z), |s| insert_into(&mut s.slots, &stack, max, simulate)) {
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
        let out = match self.with_storage(&dimension, (x, y, z), |s| extract_from(&mut s.slots, item, count, simulate)) {
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
    fn insert_energy(&mut self, dimension: String, x: i32, y: i32, z: i32, amount: u64, simulate: bool) -> u64 {
        self.with_storage(&dimension, (x, y, z), |s| {
            let n = (s.energy_capacity.saturating_sub(s.energy)).min(amount);
            if !simulate {
                s.energy += n;
            }
            n
        })
        .unwrap_or(0)
    }
    fn extract_energy(&mut self, dimension: String, x: i32, y: i32, z: i32, amount: u64, simulate: bool) -> u64 {
        self.with_storage(&dimension, (x, y, z), |s| {
            let n = s.energy.min(amount);
            if !simulate {
                s.energy -= n;
            }
            n
        })
        .unwrap_or(0)
    }
    fn set_energy(&mut self, dimension: String, x: i32, y: i32, z: i32, amount: u64) -> Result<(), String> {
        self.with_own_storage(&dimension, (x, y, z), |s| s.energy = amount.min(s.energy_capacity))
    }
    fn fluids(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<Vec<Option<storages::FluidStack>>> {
        self.with_storage(&dimension, (x, y, z), |s| s.tanks.iter().map(guest_fluid).collect())
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
        self.with_storage(&dimension, (x, y, z), |s| insert_fluid(&mut s.tanks, &fluid.fluid, fluid.amount, simulate))
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
        let (fluid, amount) = self
            .with_storage(&dimension, (x, y, z), |s| extract_fluid(&mut s.tanks, fluid.as_deref(), amount, simulate))??;
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
        self.with_own_storage(&dimension, (x, y, z), |s| match s.tanks.get_mut(tank as usize) {
            Some(t) => {
                t.fluid = fluid
                    .map(|f| (f.fluid, f.amount.min(t.capacity)))
                    .filter(|(_, a)| *a > 0);
                Ok(())
            }
            None => Err(format!("tank {tank}: the storage has {}", s.tanks.len())),
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
        self.online(&player)?;
        let slots = self
            .with_storage(&dimension, (x, y, z), |s| s.slots.len())
            .ok_or_else(|| format!("no storage at {x} {y} {z}"))?;
        if !(slots == 5 || (slots > 0 && slots % 9 == 0)) {
            return Err(format!("{slots} slots: a window needs 5, or rows of 9"));
        }
        self.act(ServerAction::OpenMenu {
            player,
            dimension,
            pos: (x, y, z),
            title: super::clip_to(title, 64),
        })
    }
}

// ---------------------------------------------------------------- entities
impl entities::Host for State {
    fn near(&mut self, dimension: String, x: f64, y: f64, z: f64, radius: f64) -> Vec<entities::Entity> {
        let r = radius.clamp(0.0, 128.0);
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        let Some(view) = view.as_ref() else {
            return Vec::new();
        };
        let d2 = |e: &EntityInfo| (e.pos.0 - x).powi(2) + (e.pos.1 - y).powi(2) + (e.pos.2 - z).powi(2);
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
    fn spawn(&mut self, dimension: String, kind: String, x: f64, y: f64, z: f64, nbt: String) -> Result<(), String> {
        self.check_dimension(&dimension)?;
        check_id("entity", &kind)?;
        let nbt = nbt.trim();
        self.console(format!("execute in {dimension} run summon {kind} {x} {y} {z} {nbt}").trim_end().to_owned())
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
    fn teleport(&mut self, id: String, dimension: String, x: f64, y: f64, z: f64) -> Result<(), String> {
        check_uuid(&id)?;
        self.check_dimension(&dimension)?;
        self.console(format!("execute in {dimension} run tp {id} {x} {y} {z}"))
    }
    fn effect(&mut self, id: String, effect: String, seconds: u32, amplifier: u8) -> Result<(), String> {
        check_uuid(&id)?;
        check_id("effect", &effect)?;
        self.console(format!("effect give {id} {effect} {} {amplifier}", seconds.clamp(1, 1_000_000)))
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
    fn sound(&mut self, dimension: String, sound: String, x: f64, y: f64, z: f64, volume: f32, pitch: f32) -> Result<(), String> {
        self.check_dimension(&dimension)?;
        check_id("sound", &sound)?;
        let (volume, pitch) = (volume.clamp(0.0, 16.0), pitch.clamp(0.5, 2.0));
        self.console(format!(
            "execute in {dimension} run playsound {sound} master @a {x} {y} {z} {volume} {pitch}"
        ))
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
        f: impl FnOnce(&server_bindings_block::Guest, &mut wasmtime::Store<State>) -> wasmtime::Result<R>,
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
    pub fn place_block(&mut self, player: &players::Player, pos: super::BlockPos, item: &str) -> bool {
        let mut refused = false;
        for i in 0..self.entries.len() {
            refused |= self
                .ask_blocks(i, |b, s| b.call_place_block(s, player, pos.x, pos.y, pos.z, item))
                .unwrap_or(false);
        }
        refused
    }
    /// `player` right-clicks the air holding `stack`: true when a mod took it.
    pub fn use_item(&mut self, player: &players::Player, stack: &HostStack, off_hand: bool) -> bool {
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
        let keys = all.mods.values().flat_map(|m| m.keys()).chain(all.storages.keys());
        let mut out: Vec<(i32, i32, i32)> = keys.filter_map(|k| parse_key(k, dimension)).collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// A storage's slots, for a window the game shows.
    pub fn storage_slots(&self, dimension: &str, pos: (i32, i32, i32)) -> Option<Vec<Option<HostStack>>> {
        let all = self.block_data.lock().ok()?;
        all.storages
            .get(&block_key(dimension, pos.0, pos.1, pos.2))
            .map(|s| s.slots.clone())
    }
    /// What a player did in a storage's window, back into it (same size).
    pub fn set_storage_slots(&mut self, dimension: &str, pos: (i32, i32, i32), slots: Vec<Option<HostStack>>) {
        if let Ok(mut all) = self.block_data.lock()
            && let Some(s) = all.storages.get_mut(&block_key(dimension, pos.0, pos.1, pos.2))
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
            let state = self.view_block(&t.dimension, x, y, z);
            let Some(state) = state.filter(|s| block_id(s) == t.block) else {
                continue;
            };
            if let Some(i) = self.index_of(&t.owner) {
                self.ask_blocks(i, |b, s| b.call_block_tick(s, &t.dimension, x, y, z, &state));
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
            let keys: std::collections::HashSet<&str> = watched.iter().map(|(k, _)| k.as_str()).collect();
            all.neighbors.retain(|k, _| keys.contains(k.as_str()));
            for (key, owners) in &watched {
                let Some((dim, (x, y, z))) = split_key(key) else {
                    continue;
                };
                let Some(state) = view.block(dim, x, y, z) else {
                    continue;
                };
                let around: [Option<u64>; 6] = NEIGHBORS.map(|(dx, dy, dz)| {
                    view.block(dim, x + dx, y + dy, z + dz).map(|s| hash(&s))
                });
                if let Some(before) = all.neighbors.insert(key.clone(), around) {
                    for (n, (dx, dy, dz)) in NEIGHBORS.into_iter().enumerate() {
                        if before[n].is_some() && around[n].is_some() && before[n] != around[n] {
                            for o in owners {
                                events.push((o.clone(), dim.to_owned(), (x, y, z), state.clone(), (x + dx, y + dy, z + dz)));
                            }
                        }
                    }
                }
            }
        }
        for (owner, dim, (x, y, z), state, (fx, fy, fz)) in events {
            if let Some(i) = self.index_of(&owner) {
                self.ask_blocks(i, |b, s| b.call_neighbor_changed(s, &dim, x, y, z, &state, fx, fy, fz));
            }
        }
    }
}

use super::block_bindings::exports::horizon::mods::block_events as server_bindings_block;

const NEIGHBORS: [(i32, i32, i32); 6] = [(0, -1, 0), (0, 1, 0), (0, 0, -1), (0, 0, 1), (-1, 0, 0), (1, 0, 0)];

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
    split_key(key).filter(|(d, _)| *d == dimension).map(|(_, p)| p)
}

/// Neighbour states last seen around a mod's block (hashes, in
/// [`NEIGHBORS`] order; none where no chunk was loaded).
pub(super) type NeighborCache = std::collections::HashMap<String, [Option<u64>; 6]>;

