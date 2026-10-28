//! `game-events`: what happens to entities and the world, for the server
//! mods whose manifest lists the event, in order of their `priority`.
use super::server_api::{EntityInfo, HostStack, to_guest};
use super::{FUEL_EVENT, Mods, Running, ServerPlayer, State, events_bindings};
use events_bindings::exports::horizon::mods::game_events::Guest;
use super::server_bindings::horizon::mods::entities::Entity;

fn guest_entity(e: &EntityInfo) -> Entity {
    Entity {
        id: e.id.clone(),
        kind: e.kind.clone(),
        x: e.pos.0,
        y: e.pos.1,
        z: e.pos.2,
        health: e.health,
    }
}

impl Mods {
    /// The running mods that hear `event`, highest `priority` first.
    fn hearing(&self, event: &str) -> Vec<usize> {
        let mut order: Vec<(i32, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.run.as_ref().is_some_and(|r| r.events.is_some()))
            .filter_map(|(i, e)| {
                let m = e.manifest.as_ref()?;
                m.events.iter().any(|n| n == event).then_some((m.priority, i))
            })
            .collect();
        order.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        order.into_iter().map(|(_, i)| i).collect()
    }
    /// Whether a running mod hears `event`: the game finds none otherwise.
    pub fn wants_event(&self, event: &str) -> bool {
        !self.hearing(event).is_empty()
    }

    fn ask_events<R>(
        &mut self,
        i: usize,
        f: impl FnOnce(&Guest, &mut wasmtime::Store<State>) -> wasmtime::Result<R>,
    ) -> Option<R> {
        self.call(i, FUEL_EVENT, |run| {
            let Running { events, store, .. } = run;
            match events {
                Some(e) => f(e.horizon_mods_game_events(), store),
                None => Err(wasmtime::Error::msg("no game-events")),
            }
        })
    }
    /// Asks in order until one answers true.
    fn cancelled(
        &mut self,
        event: &str,
        mut f: impl FnMut(&Guest, &mut wasmtime::Store<State>, &str) -> wasmtime::Result<bool>,
    ) -> bool {
        for i in self.hearing(event) {
            let id = self.entries[i].manifest.as_ref().map(|m| m.id.clone()).unwrap_or_default();
            if self.ask_events(i, |g, s| f(g, s, &id)).unwrap_or(false) {
                return true;
            }
        }
        false
    }

    /// A mob appeared: true when a mod removes it.
    pub fn entity_spawned(&mut self, dimension: &str, entity: &EntityInfo) -> bool {
        let e = guest_entity(entity);
        self.cancelled("entity-spawned", |g, s, _| g.call_entity_spawned(s, dimension, &e))
    }
    /// A mob lost `amount` health and lives: true when a mod gives it back.
    pub fn entity_hurt(&mut self, dimension: &str, entity: &EntityInfo, amount: f32) -> bool {
        let e = guest_entity(entity);
        self.cancelled("entity-hurt", |g, s, _| g.call_entity_hurt(s, dimension, &e, amount))
    }
    /// A mob died, last hurt by `killer` (a player's id) if anyone.
    pub fn entity_died(&mut self, dimension: &str, entity: &EntityInfo, killer: Option<&str>) {
        let e = guest_entity(entity);
        for i in self.hearing("entity-died") {
            self.ask_events(i, |g, s| g.call_entity_died(s, dimension, &e, killer));
        }
    }
    /// A player lost `amount` health.
    pub fn player_hurt(&mut self, player: &ServerPlayer, amount: f32) {
        for i in self.hearing("player-hurt") {
            self.ask_events(i, |g, s| g.call_player_hurt(s, player, amount));
        }
    }
    /// A player picked up `stack`.
    pub fn item_picked_up(&mut self, player: &ServerPlayer, stack: &HostStack) {
        for i in self.hearing("item-picked-up") {
            let id = self.entries[i].manifest.as_ref().map(|m| m.id.clone()).unwrap_or_default();
            let guest = to_guest(stack, &id);
            self.ask_events(i, |g, s| g.call_item_picked_up(s, player, &guest));
        }
    }
    /// An explosion is about to go off: true when a mod stops it.
    pub fn explosion(&mut self, dimension: &str, at: (f64, f64, f64), power: f32) -> bool {
        self.cancelled("explosion", |g, s, _| g.call_explosion(s, dimension, at.0, at.1, at.2, power))
    }
}
