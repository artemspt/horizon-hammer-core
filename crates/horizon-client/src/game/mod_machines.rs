//! Server mods' items, storages' windows, vanilla containers, mobs and
//! redstone (`docs/mods.md`, API level 10) in the game that runs them: stacks
//! between the game's `ItemStack` and the mods' `HostStack`, what server
//! mods ask of players' inventories, the windows of their storages, and what
//! the world lends their calls beyond blocks.
use super::{Dim, Game};
use horizon_client::containers::BlockEntity;
use horizon_client::items::ItemStack;
use horizon_client::lan::menu::{Slots, VEHICLE_KEY_Y};
use horizon_client::text::Text;
use horizon_mods::{EntityInfo, HostStack, ServerAction};
use horizon_world::nbt::{self, JavaString, Limits, Tag};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const CUSTOM_DATA: &str = "minecraft:custom_data";
/// Where mods' bytes sit in `minecraft:custom_data`: mod id → bytes.
const MODS_KEY: &str = "horizon_mods";
/// The placeholder container a storage's window edits lives under a key no
/// block has (as a vehicle's does, so no reach check); its z counts down
/// from -1 so it never meets a vehicle's entity id.
const MENU_KEY_X: i32 = i32::MAX - 17;

/// A storage's window open for a player.
pub(super) struct ModMenu {
    conn: u32,
    key: (i32, i32, i32),
    /// The level holding the placeholder: the player's.
    level: String,
    dimension: String,
    pos: (i32, i32, i32),
    /// The slots as last copied either way.
    last: Vec<Option<HostStack>>,
}

/// `UUID.toString()`.
fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}
fn uuid_words(w: [i32; 4]) -> u128 {
    w.iter().fold(0u128, |u, &x| (u << 32) | u128::from(x as u32))
}

/// A game stack as server mods see it: their bytes out of
/// `minecraft:custom_data`, the rest as the game's own encoding.
pub(super) fn host_stack(stack: &ItemStack) -> HostStack {
    let mut s = stack.clone();
    let mut data = BTreeMap::new();
    if let Some(d) = s.extra.as_mut()
        && let Some(i) = d.other.iter().position(|(k, _)| k == CUSTOM_DATA)
        && let Tag::Compound(c) = &mut d.other[i].1
    {
        if let Some(Tag::Compound(mods)) = c.remove(&JavaString::from(MODS_KEY)) {
            for (id, bytes) in mods {
                if let Tag::ByteArray(b) = bytes {
                    data.insert(id.to_string_lossy(), b);
                }
            }
        }
        if c.is_empty() {
            d.other.remove(i);
        }
    }
    s.tidy();
    let count = s.count;
    let components = if s.extra.is_none() && s.damage == 0 {
        Vec::new()
    } else {
        s.count = 1;
        nbt::write_network(&horizon_items::item_nbt::item_to_nbt(&s), Limits::default()).unwrap_or_default()
    };
    HostStack {
        item: s.item.to_string(),
        count,
        components,
        data,
    }
}

/// A mod's stack as the game's: none for no count or components it can't read.
pub(super) fn game_stack(h: &HostStack) -> Option<ItemStack> {
    if h.count == 0 {
        return None;
    }
    let mut s = if h.components.is_empty() {
        ItemStack {
            item: Arc::from(h.item.as_str()),
            count: 1,
            damage: 0,
            extra: None,
        }
    } else {
        let (tag, _) = nbt::read_network(&h.components, Limits::default()).ok()?;
        horizon_items::item_nbt::item_from_nbt(&tag)?
    };
    s.item = Arc::from(h.item.as_str());
    s.count = h.count;
    if !h.data.is_empty() {
        let mods: nbt::Compound = h
            .data
            .iter()
            .map(|(id, b)| (JavaString::from(id.as_str()), Tag::ByteArray(b.clone())))
            .collect();
        let d = s.data_mut();
        match d.other.iter_mut().find(|(k, _)| k == CUSTOM_DATA) {
            Some((_, Tag::Compound(c))) => {
                c.insert(JavaString::from(MODS_KEY), Tag::Compound(mods));
            }
            _ => d
                .other
                .push((CUSTOM_DATA.into(), Tag::compound([(MODS_KEY, Tag::Compound(mods))]))),
        }
    }
    Some(s)
}

/// 36 inventory slots, armor feet to head (the game keeps head first), the off hand.
fn inventory_slots(
    inventory: &[Option<ItemStack>],
    armor: &[Option<ItemStack>; 4],
    offhand: &Option<ItemStack>,
) -> Vec<Option<HostStack>> {
    let mut out: Vec<Option<HostStack>> = inventory.iter().take(36).map(|s| s.as_ref().map(host_stack)).collect();
    out.resize(36, None);
    out.extend(armor.iter().rev().map(|s| s.as_ref().map(host_stack)));
    out.push(offhand.as_ref().map(host_stack));
    out
}

/// What a server mod's call may read beyond blocks, moved out of the game
/// for the call and back after it.
#[derive(Default)]
pub(super) struct Lent {
    pub(super) redstone: HashMap<(String, i32, i32, i32), u8>,
    pub(super) max_stacks: Arc<HashMap<String, u32>>,
    /// Each level's block entities (vanilla containers).
    pub(super) containers: Vec<(&'static str, horizon_client::block_entities::BlockEntities)>,
    pub(super) mobs: Vec<(&'static str, Vec<EntityInfo>)>,
}
impl Lent {
    pub(super) fn redstone(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<u8> {
        self.redstone.get(&(dimension.to_owned(), x, y, z)).copied()
    }
    pub(super) fn max_stack(&self, item: &str) -> Option<u32> {
        self.max_stacks.get(item).copied()
    }
    fn entity(&self, dimension: &str, pos: (i32, i32, i32)) -> Option<&BlockEntity> {
        let (_, map) = self.containers.iter().find(|(d, _)| *d == dimension)?;
        map.get(&pos)
    }
    pub(super) fn container(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<Vec<Option<HostStack>>> {
        let slots = self.entity(dimension, (x, y, z))?.slots();
        (!slots.is_empty()).then(|| slots.iter().map(|s| s.as_ref().map(host_stack)).collect())
    }
    pub(super) fn set_container(&mut self, dimension: &str, x: i32, y: i32, z: i32, slots: Vec<Option<HostStack>>) -> bool {
        let Some((_, map)) = self.containers.iter_mut().find(|(d, _)| *d == dimension) else {
            return false;
        };
        let Some(e) = map.get_mut(&(x, y, z)) else {
            return false;
        };
        let to = e.slots_mut();
        if to.is_empty() || to.len() != slots.len() {
            return false;
        }
        for (at, s) in to.iter_mut().zip(slots) {
            *at = s.as_ref().and_then(game_stack);
        }
        true
    }
    pub(super) fn mobs(&self, dimension: &str) -> Vec<EntityInfo> {
        self.mobs
            .iter()
            .find(|(d, _)| *d == dimension)
            .map(|(_, m)| m.clone())
            .unwrap_or_default()
    }
}

impl Game<'_> {
    /// The level `dim`'s world: the one in place or a parked one.
    fn level_world(&mut self, dim: &str) -> Option<&mut horizon_client::world::World> {
        if self.dim.id() == dim {
            Some(&mut self.world)
        } else {
            self.levels.parked_world(Dim::from_id(dim))
        }
    }

    /// Each player's held slot and inventory, for `players.inventory`.
    pub(super) fn mod_inventories(&self) -> Vec<(String, u32, Vec<Option<HostStack>>)> {
        let mut out = Vec::new();
        if self.options.execution_role.is_client() && self.remote.is_none() && self.player.is_some() {
            let slots = inventory_slots(&self.inventory, &self.armor, &self.offhand);
            out.push((self.player_uuid_string(), self.selected as u32, slots));
        }
        for g in self.lan.iter().flat_map(|h| &h.hub.guests).filter(|g| g.placed) {
            let slots = inventory_slots(&g.inventory, &g.armor, &g.offhand);
            out.push((uuid_string(g.uuid), g.selected as u32, slots));
        }
        out
    }

    /// Before the world is lent: the redstone power at the mods' blocks of
    /// the level in place, the items' stack sizes, the levels' block
    /// entities and mobs. [`Game::return_lent`] puts them back.
    pub(super) fn lend_more(&mut self) -> Lent {
        let dim = self.dim.id();
        let mut redstone = HashMap::new();
        for (x, y, z) in self.mods.mods.machine_positions(dim) {
            let pos = horizon_core::position::BlockPos::new(x, y, z);
            let power = self.with_level(|b, level, ticks| b.best_neighbor_signal(level, ticks, pos));
            redstone.insert((dim.to_owned(), x, y, z), power.clamp(0, 15) as u8);
        }
        let max_stacks = self
            .mods
            .max_stacks
            .get_or_insert_with(|| {
                Arc::new(
                    self.res
                        .items()
                        .items
                        .iter()
                        .map(|(id, info)| (id.to_string(), info.max_stack))
                        .collect(),
                )
            })
            .clone();
        let mut containers = vec![(dim, std::mem::take(&mut self.world.block_entities))];
        for (d, world) in self.levels.parked_worlds() {
            containers.push((d.id(), std::mem::take(&mut world.block_entities)));
        }
        let uuids = &self.persist.uuids;
        let mobs: Vec<EntityInfo> = self
            .mobs
            .iter()
            .filter(|m| m.death_time == 0)
            .filter_map(|m| {
                let uuid = uuids.get(&m.id)?;
                Some(EntityInfo {
                    id: uuid_string(uuid_words(*uuid)),
                    kind: m.kind.id.clone(),
                    pos: (m.position.x, m.position.y, m.position.z),
                    health: m.health,
                })
            })
            .collect();
        Lent {
            redstone,
            max_stacks,
            containers,
            mobs: vec![(dim, mobs)],
        }
    }
    /// The block entities [`Game::lend_more`] took, back in their levels.
    pub(super) fn return_lent(&mut self, lent: Lent) {
        for (d, map) in lent.containers {
            if let Some(world) = self.level_world(d) {
                world.block_entities = map;
            }
        }
    }

    /// The changes server mods asked for, in order (`players.give`, ...).
    pub(super) fn apply_mod_actions(&mut self) {
        for action in std::mem::take(&mut self.mods.actions) {
            match action {
                ServerAction::Give { player, stack } => {
                    if let Some(s) = game_stack(&stack) {
                        self.on_mod_player(&player, |g| g.give_or_drop(s));
                    }
                }
                ServerAction::SetSlot { player, slot, stack } => {
                    let s = stack.as_ref().and_then(game_stack);
                    self.on_mod_player(&player, |g| match slot {
                        0..=35 => {
                            if let Some(at) = g.inventory.get_mut(slot as usize) {
                                *at = s;
                            }
                        }
                        36..=39 => g.armor[(39 - slot) as usize] = s,
                        40 => g.offhand = s,
                        _ => {}
                    });
                }
                ServerAction::Take { player, item, count } => {
                    self.on_mod_player(&player, |g| {
                        let mut left = count;
                        let slots = g
                            .inventory
                            .iter_mut()
                            .chain(g.armor.iter_mut())
                            .chain(std::iter::once(&mut g.offhand));
                        for at in slots {
                            if left == 0 {
                                break;
                            }
                            if let Some(s) = at
                                && *s.item == *item
                            {
                                let n = s.count.min(left);
                                s.count -= n;
                                left -= n;
                                if s.count == 0 {
                                    *at = None;
                                }
                            }
                        }
                    });
                }
                ServerAction::Console { line } => self.mod_console(&line),
                ServerAction::OpenMenu {
                    player,
                    dimension,
                    pos,
                    title,
                } => self.open_mod_menu(&player, dimension, pos, &title),
            }
        }
    }

    /// `f` as the player with this id: the window's own in-frame, else a
    /// connection's (their inventory in place, as for their own actions).
    fn on_mod_player(&mut self, id: &str, f: impl FnOnce(&mut Self)) {
        if self.options.execution_role.is_client()
            && self.remote.is_none()
            && self.player.is_some()
            && self.player_uuid_string() == id
        {
            f(self);
            return;
        }
        let Some(mut host) = self.lan.take() else {
            return;
        };
        let conn = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && uuid_string(g.uuid) == id)
            .map(|g| g.conn);
        if let Some(conn) = conn {
            self.as_guest(&mut host, conn, f);
        }
        self.lan = Some(host);
    }

    /// A vanilla command as the server console, its feedback dropped.
    fn mod_console(&mut self, line: &str) {
        let capture = self.chat_capture.replace(Vec::new());
        self.as_console(|g| g.run_command(line));
        self.chat_capture = capture;
        self.admin_feedback.clear();
    }

    /// `storages.open-menu`: a chest-like window on a placeholder holding
    /// the storage's slots, copied back each tick ([`Game::sync_mod_menus`]).
    fn open_mod_menu(&mut self, player: &str, dimension: String, pos: (i32, i32, i32), title: &str) {
        let Some(items) = self.mods.mods.storage_slots(&dimension, pos) else {
            return;
        };
        let Some(mut host) = self.lan.take() else {
            // In-frame: no window for the window's own player yet.
            log::debug!("Horizon: a mod's window needs a server thread or a server");
            return;
        };
        let guest = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && uuid_string(g.uuid) == player)
            .map(|g| (g.conn, g.dimension.clone()));
        if let Some((conn, level)) = guest {
            self.mods.next_menu += 1;
            let key = (MENU_KEY_X, VEHICLE_KEY_Y, -self.mods.next_menu);
            let kind = if items.len() == 5 {
                "minecraft:hopper".to_owned()
            } else {
                format!("minecraft:generic_9x{}", items.len() / 9)
            };
            let stacks: Vec<Option<ItemStack>> = items.iter().map(|s| s.as_ref().and_then(game_stack)).collect();
            if let Some(world) = self.level_world(&level) {
                world.block_entities.insert(key, BlockEntity::Chest(stacks.clone()));
                let title = horizon_client::lan::data::component_nbt(&Text::literal(title.to_owned()).component());
                host.hub.open_container(conn, &kind, title, key, Slots::Block(key), stacks);
                self.mods.menus.push(ModMenu {
                    conn,
                    key,
                    level,
                    dimension,
                    pos,
                    last: items,
                });
            }
        }
        self.lan = Some(host);
    }

    /// Each tick: what players moved in storages' windows goes into the
    /// storage, what the mod changed into the window; closed ones go.
    pub(super) fn sync_mod_menus(&mut self) {
        for mut m in std::mem::take(&mut self.mods.menus) {
            let open = self.lan.as_ref().is_some_and(|h| {
                h.hub
                    .guests
                    .iter()
                    .any(|g| g.conn == m.conn && g.menu.as_ref().is_some_and(|menu| menu.pos == Some(m.key)))
            });
            let shown: Option<Vec<Option<HostStack>>> = self.level_world(&m.level).and_then(|w| {
                match w.block_entities.get(&m.key) {
                    Some(BlockEntity::Chest(items)) => Some(items.iter().map(|s| s.as_ref().map(host_stack)).collect()),
                    _ => None,
                }
            });
            let stored = self.mods.mods.storage_slots(&m.dimension, m.pos);
            let alive = match (shown, stored) {
                (Some(shown), Some(stored)) if shown.len() == stored.len() => {
                    if shown != m.last {
                        self.mods.mods.set_storage_slots(&m.dimension, m.pos, shown.clone());
                        m.last = shown;
                    } else if stored != m.last {
                        let stacks: Vec<Option<ItemStack>> = stored.iter().map(|s| s.as_ref().and_then(game_stack)).collect();
                        if let Some(w) = self.level_world(&m.level) {
                            w.block_entities.insert(m.key, BlockEntity::Chest(stacks));
                        }
                        m.last = stored;
                    }
                    true
                }
                _ => false,
            };
            if open && alive {
                self.mods.menus.push(m);
                continue;
            }
            if open && let Some(host) = self.lan.as_mut() {
                host.hub.close_container(m.conn);
            }
            if let Some(w) = self.level_world(&m.level) {
                w.block_entities.remove(&m.key);
            }
        }
    }

}
