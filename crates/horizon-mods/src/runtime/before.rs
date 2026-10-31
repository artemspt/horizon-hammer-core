//! `before-events`: asked before something happens, of the server mods whose
//! manifest lists `before-<name>`, in order of their `priority`. A refusal
//! ends the asking; a changed value goes on to the next mod.
use super::server_api::{EntityInfo, HostStack, from_guest, to_guest};
use super::{FUEL_EVENT, Mods, Running, ServerPlayer, State, before_bindings};
use before_bindings::exports::horizon::mods::before_events::Guest;

/// The most blocks `explosion-blocks` hands a mod.
const EXPLOSION_BLOCKS: usize = 4096;
/// The most stacks a mod may make a block or a mob drop.
const DROPS: usize = 64;

impl Mods {
    /// The running mods with a `before-events` export that list `event`
    /// (`before-entity-hurt`), highest `priority` first.
    fn hearing_before(&self, event: &str) -> Vec<(usize, String)> {
        let mut order: Vec<(i32, usize, String)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.run.as_ref().is_some_and(|r| r.before.is_some()))
            .filter_map(|(i, e)| {
                let m = e.manifest.as_ref()?;
                m.events
                    .iter()
                    .any(|n| n == event)
                    .then(|| (m.priority, i, m.id.clone()))
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        order.into_iter().map(|(_, i, id)| (i, id)).collect()
    }
    fn ask_before<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&Guest, &mut wasmtime::Store<State>) -> wasmtime::Result<R>,
    ) -> Option<R> {
        self.call(i, FUEL_EVENT, |run| {
            let Running { before, store, .. } = run;
            match before {
                Some(b) => f(b.horizon_mods_before_events(), store),
                None => Err(wasmtime::Error::msg("no before-events")),
            }
        })
    }
    /// Asks in order until one answers true.
    fn refused(
        &mut self,
        event: &str,
        mut f: impl FnMut(&Guest, &mut wasmtime::Store<State>, &str) -> wasmtime::Result<bool>,
    ) -> bool {
        for (i, id) in self.hearing_before(event) {
            if self.ask_before(i, |g, s| f(g, s, &id)).unwrap_or(false) {
                return true;
            }
        }
        false
    }
    /// An amount handed along: `None` when no mod changed it.
    fn amount(
        &mut self,
        event: &str,
        amount: f32,
        mut f: impl FnMut(&Guest, &mut wasmtime::Store<State>, f32) -> wasmtime::Result<f32>,
    ) -> Option<f32> {
        let mut now = amount;
        for (i, _) in self.hearing_before(event) {
            if let Some(next) = self.ask_before(i, |g, s| f(g, s, now))
                && next.is_finite()
            {
                now = next;
            }
            if now <= 0.0 {
                break;
            }
        }
        (now != amount).then_some(now)
    }
    /// Stacks handed along, each mod seeing its own bytes on them.
    fn stacks(
        &mut self,
        event: &str,
        drops: &[HostStack],
        mut f: impl FnMut(
            &Guest,
            &mut wasmtime::Store<State>,
            &[super::server_bindings::horizon::mods::players::ItemStack],
        ) -> wasmtime::Result<
            Option<Vec<super::server_bindings::horizon::mods::players::ItemStack>>,
        >,
    ) -> Option<Vec<HostStack>> {
        let mut now: Option<Vec<HostStack>> = None;
        for (i, id) in self.hearing_before(event) {
            let shown: Vec<_> = now
                .as_deref()
                .unwrap_or(drops)
                .iter()
                .map(|s| to_guest(s, &id))
                .collect();
            let Some(Some(answer)) = self.ask_before(i, |g, s| f(g, s, &shown)) else {
                continue;
            };
            let mut next = Vec::new();
            for stack in answer.into_iter().take(DROPS) {
                match from_guest(stack, &id) {
                    Ok(s) if s.count > 0 => next.push(s),
                    Ok(_) => {}
                    Err(e) => log::warn!("Horizon: mod {id}'s {event}: {e}"),
                }
            }
            now = Some(next);
        }
        now
    }

    pub fn wants_before(&self, event: &str) -> bool {
        !self.hearing_before(event).is_empty()
    }

    /// `before-entity-spawn`: true when a mod refuses the mob.
    pub fn before_entity_spawn(
        &mut self,
        dimension: &str,
        entity: &EntityInfo,
        reason: &str,
    ) -> bool {
        let e = super::events::guest_entity(entity);
        self.refused("before-entity-spawn", |g, s, _| {
            g.call_entity_spawn(s, dimension, &e, reason)
        })
    }
    /// `before-entity-hurt`: what the mob loses instead, if a mod changed it.
    pub fn before_entity_hurt(
        &mut self,
        dimension: &str,
        entity: &EntityInfo,
        damage_type: &str,
        attacker: Option<&str>,
        amount: f32,
    ) -> Option<f32> {
        let e = super::events::guest_entity(entity);
        self.amount("before-entity-hurt", amount, |g, s, now| {
            g.call_entity_hurt(s, dimension, &e, damage_type, attacker, now)
        })
    }
    /// `before-player-hurt`: as [`Mods::before_entity_hurt`].
    pub fn before_player_hurt(
        &mut self,
        player: &ServerPlayer,
        damage_type: &str,
        attacker: Option<&str>,
        amount: f32,
    ) -> Option<f32> {
        self.amount("before-player-hurt", amount, |g, s, now| {
            g.call_player_hurt(s, player, damage_type, attacker, now)
        })
    }
    /// `before-player-death`: true when a mod keeps the player alive.
    pub fn before_player_death(&mut self, player: &ServerPlayer, damage_type: &str) -> bool {
        self.refused("before-player-death", |g, s, _| {
            g.call_player_death(s, player, damage_type)
        })
    }
    /// `before-explosion-blocks`: the blocks the mods spare.
    pub fn before_explosion_blocks(
        &mut self,
        dimension: &str,
        (x, y, z): (f64, f64, f64),
        blocks: &[[i32; 3]],
    ) -> Vec<[i32; 3]> {
        let mut breaking: Vec<(i32, i32, i32)> = blocks
            .iter()
            .take(EXPLOSION_BLOCKS)
            .map(|p| (p[0], p[1], p[2]))
            .collect();
        let mut spared = Vec::new();
        for (i, _) in self.hearing_before("before-explosion-blocks") {
            if breaking.is_empty() {
                break;
            }
            let Some(stay) = self.ask_before(i, |g, s| {
                g.call_explosion_blocks(s, dimension, x, y, z, &breaking)
            }) else {
                continue;
            };
            let stay: std::collections::HashSet<_> = stay.into_iter().collect();
            breaking.retain(|p| {
                let kept = stay.contains(p);
                if kept {
                    spared.push([p.0, p.1, p.2]);
                }
                !kept
            });
        }
        spared
    }
    /// `before-item-pickup`: true when a mod leaves the stack on the ground.
    pub fn before_item_pickup(&mut self, player: &ServerPlayer, stack: &HostStack) -> bool {
        self.refused("before-item-pickup", |g, s, id| {
            g.call_item_pickup(s, player, &to_guest(stack, id))
        })
    }
    /// `before-item-toss`: true when a mod keeps the stack in the inventory.
    pub fn before_item_toss(&mut self, player: &ServerPlayer, stack: &HostStack) -> bool {
        self.refused("before-item-toss", |g, s, id| {
            g.call_item_toss(s, player, &to_guest(stack, id))
        })
    }
    /// `before-mob-drop`: what drops instead, if a mod changed it.
    pub fn before_mob_drop(
        &mut self,
        dimension: &str,
        entity: &EntityInfo,
        killer: Option<&str>,
        drops: &[HostStack],
    ) -> Option<Vec<HostStack>> {
        let e = super::events::guest_entity(entity);
        self.stacks("before-mob-drop", drops, |g, s, now| {
            g.call_mob_drop(s, dimension, &e, killer, now)
        })
    }
    /// `before-xp-drop`: the experience that drops.
    pub fn before_xp_drop(
        &mut self,
        dimension: &str,
        (x, y, z): (f64, f64, f64),
        source: &str,
        amount: u32,
    ) -> u32 {
        let mut now = amount;
        for (i, _) in self.hearing_before("before-xp-drop") {
            if let Some(next) =
                self.ask_before(i, |g, s| g.call_xp_drop(s, dimension, x, y, z, source, now))
            {
                now = next;
            }
        }
        now
    }
    /// `before-container-open`: true when a mod keeps the window shut.
    pub fn before_container_open(
        &mut self,
        player: &ServerPlayer,
        [x, y, z]: [i32; 3],
        block: &str,
    ) -> bool {
        self.refused("before-container-open", |g, s, _| {
            g.call_container_open(s, player, x, y, z, block)
        })
    }
}
