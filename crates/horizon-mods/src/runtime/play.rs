//! `item-events` and `block-hooks`: what players do with chosen items, and
//! what happens at chosen blocks. They only tell, so the game queues them as
//! they happen and hands them over at the tick's end, with the world lent.
//! Asked of the mods whose manifest lists the event and names the item in
//! `hook_items` or the block in `hook_blocks`.
use super::server_api::{EntityInfo, HostStack, to_guest};
use super::{
    FUEL_EVENT, Mods, Running, ServerPlayer, block_hook_bindings, block_id, item_bindings,
};

/// What a player did with an item.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemEvent {
    UseTick(u32),
    UseFinished,
    UseStopped(u32),
    InventoryTick(u32),
    HeldTick,
    Mine {
        dimension: String,
        pos: (i32, i32, i32),
        state: String,
    },
    Hit(EntityInfo),
}

impl ItemEvent {
    fn name(&self) -> &'static str {
        match self {
            Self::UseTick(_) => "item-use-tick",
            Self::UseFinished => "item-use-finished",
            Self::UseStopped(_) => "item-use-stopped",
            Self::InventoryTick(_) => "item-inventory-tick",
            Self::HeldTick => "item-held-tick",
            Self::Mine { .. } => "item-mine-block",
            Self::Hit(_) => "item-hit-entity",
        }
    }
}

/// What happened at a block.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockHook {
    RandomTick,
    EntityInside(String),
    SteppedOn(String),
    FellOn(String, f32),
    Exploded,
}

impl BlockHook {
    pub fn name(&self) -> &'static str {
        match self {
            Self::RandomTick => "block-random-tick",
            Self::EntityInside(_) => "block-entity-inside",
            Self::SteppedOn(_) => "block-stepped-on",
            Self::FellOn(..) => "block-fell-on",
            Self::Exploded => "block-exploded",
        }
    }
}

impl Mods {
    /// The running mods listing `event`, with their `hook_items` or
    /// `hook_blocks` naming `id`, highest `priority` first; `blocks` says which.
    fn hooked(&self, event: &str, id: &str, blocks: bool) -> Vec<(usize, String)> {
        let mut order: Vec<(i32, usize, String)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.run.is_some())
            .filter_map(|(i, e)| {
                let m = e.manifest.as_ref()?;
                let list = if blocks {
                    &m.hook_blocks
                } else {
                    &m.hook_items
                };
                (m.events.iter().any(|n| n == event) && list.iter().any(|n| n == id))
                    .then(|| (m.priority, i, m.id.clone()))
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        order.into_iter().map(|(_, i, id)| (i, id)).collect()
    }
    /// Whether a running mod names items or blocks to hear of: the game looks
    /// for nothing otherwise.
    pub fn has_item_hooks(&self) -> bool {
        self.entries.iter().any(|e| {
            e.run.is_some()
                && e.manifest
                    .as_ref()
                    .is_some_and(|m| !m.hook_items.is_empty())
        })
    }
    pub fn has_block_hooks(&self, event: &str) -> bool {
        self.entries.iter().any(|e| {
            e.run.is_some()
                && e.manifest.as_ref().is_some_and(|m| {
                    !m.hook_blocks.is_empty() && m.events.iter().any(|n| n == event)
                })
        })
    }
    /// Whether `event` is heard for this block, from a state or an id.
    pub fn hooks_block(&self, event: &str, state: &str) -> bool {
        !self.hooked(event, block_id(state), true).is_empty()
    }

    /// The lines running mods add under `item`'s tooltip.
    pub fn item_tooltip(&self, item: &str) -> Vec<String> {
        let Ok(data) = self.block_data.lock() else {
            return Vec::new();
        };
        let Some(of_item) = data.tooltips.get(item) else {
            return Vec::new();
        };
        of_item
            .iter()
            .filter(|(id, _)| {
                self.entries
                    .iter()
                    .any(|e| e.run.is_some() && e.manifest.as_ref().is_some_and(|m| m.id == **id))
            })
            .flat_map(|(_, lines)| lines.iter().cloned())
            .collect()
    }

    /// Hands a player's item event to the mods naming the item.
    pub fn item_event(&mut self, player: &ServerPlayer, stack: &HostStack, event: &ItemEvent) {
        for (i, id) in self.hooked(event.name(), &stack.item, false) {
            let item = to_guest(stack, &id);
            self.call(i, FUEL_EVENT, |run| {
                let Running {
                    item_events, store, ..
                } = run;
                let Some(h) = item_events else {
                    return Ok(());
                };
                let g = h.horizon_mods_item_events();
                match event {
                    ItemEvent::UseTick(t) => g.call_use_tick(store, player, &item, *t),
                    ItemEvent::UseFinished => g.call_use_finished(store, player, &item),
                    ItemEvent::UseStopped(t) => g.call_use_stopped(store, player, &item, *t),
                    ItemEvent::InventoryTick(s) => g.call_inventory_tick(store, player, &item, *s),
                    ItemEvent::HeldTick => g.call_held_tick(store, player, &item),
                    ItemEvent::Mine {
                        dimension,
                        pos: (x, y, z),
                        state,
                    } => g.call_mine_block(store, player, &item, dimension, *x, *y, *z, state),
                    ItemEvent::Hit(target) => g.call_hit_entity(
                        store,
                        player,
                        &item,
                        &super::events::guest_entity(target),
                    ),
                }
            });
        }
    }
    /// A player took a crafted result: every mod listing `item-crafted` hears.
    pub fn item_crafted(&mut self, player: &ServerPlayer, recipe: &str) {
        for i in 0..self.entries.len() {
            let listed = self.entries[i]
                .manifest
                .as_ref()
                .is_some_and(|m| m.events.iter().any(|n| n == "item-crafted"));
            if listed {
                self.call(i, FUEL_EVENT, |run| {
                    let Running {
                        item_events, store, ..
                    } = run;
                    match item_events {
                        Some(h) => h
                            .horizon_mods_item_events()
                            .call_crafted(store, player, recipe),
                        None => Ok(()),
                    }
                });
            }
        }
    }
    /// Hands a block's event to the mods naming the block.
    pub fn block_hook(
        &mut self,
        dimension: &str,
        (x, y, z): (i32, i32, i32),
        state: &str,
        hook: &BlockHook,
    ) {
        for (i, _) in self.hooked(hook.name(), block_id(state), true) {
            self.call(i, FUEL_EVENT, |run| {
                let Running {
                    block_hooks, store, ..
                } = run;
                let Some(h) = block_hooks else {
                    return Ok(());
                };
                let g = h.horizon_mods_block_hooks();
                match hook {
                    BlockHook::RandomTick => g.call_random_tick(store, dimension, x, y, z, state),
                    BlockHook::EntityInside(e) => {
                        g.call_entity_inside(store, dimension, x, y, z, state, e)
                    }
                    BlockHook::SteppedOn(e) => {
                        g.call_stepped_on(store, dimension, x, y, z, state, e)
                    }
                    BlockHook::FellOn(e, d) => {
                        g.call_fell_on(store, dimension, x, y, z, state, e, *d)
                    }
                    BlockHook::Exploded => g.call_exploded(store, dimension, x, y, z, state),
                }
            });
        }
    }
}

// The names `Running` holds them under.
pub(super) use block_hook_bindings::BlockHooksHost;
pub(super) use item_bindings::ItemEventsHost;
