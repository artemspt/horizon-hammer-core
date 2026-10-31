//! Server mods asked in the middle of the simulation (`before-events`), where
//! the game's state is borrowed and only the thing at hand can be seen: a mob
//! or a player taking damage inside its own tick. The game installs a
//! [`Gate`] around such a phase; with none installed (no mod listening, a
//! client) each ask is one check and answers nothing.
use crate::mobs::{Damage, Mob};
use std::any::Any;
use std::cell::{Cell, RefCell};

/// The game's side: asks its server mods.
pub trait Gate: Any {
    /// `mob` is about to lose `amount` health to `source`: `None` leaves it,
    /// `Some(amount)` is what it loses instead (0 or less: nothing at all).
    fn entity_hurt(&mut self, mob: &Mob, amount: f32, source: &Damage) -> Option<f32>;
    /// The same for the [`actor`] (the player in place when there is none),
    /// standing at `at` with `health`; `attacker` is the mob that dealt it.
    fn player_hurt(
        &mut self,
        actor: Option<u128>,
        damage_type: &str,
        attacker: Option<u32>,
        amount: f32,
        at: [f64; 3],
        health: f32,
    ) -> Option<f32>;
    /// The [`actor`] is about to die of `damage_type`: true keeps them alive.
    fn player_death(&mut self, actor: Option<u128>, damage_type: &str) -> bool;
    fn as_any(&mut self) -> &mut dyn Any;
}

thread_local! {
    static GATE: RefCell<Option<Box<dyn Gate>>> = const { RefCell::new(None) };
    /// The UUID of the guest whose turn it is, if one's is.
    static ACTOR: Cell<Option<u128>> = const { Cell::new(None) };
    /// The mob behind the next damage to a player, for its `attacker`.
    static ATTACKER: Cell<Option<u32>> = const { Cell::new(None) };
}

/// Puts `gate` in place for this thread; [`close`] gives it back.
pub fn open(gate: Box<dyn Gate>) {
    GATE.with(|g| *g.borrow_mut() = Some(gate));
}
pub fn close() -> Option<Box<dyn Gate>> {
    GATE.with(|g| g.borrow_mut().take())
}
pub fn is_open() -> bool {
    GATE.with(|g| g.borrow().is_some())
}

/// Runs `f` on the gate in place, taken out meanwhile: an ask made while the
/// gate is busy (a mod's call can't reach back here, but be safe) answers
/// nothing.
pub fn with<R>(f: impl FnOnce(&mut dyn Gate) -> R) -> Option<R> {
    let mut gate = GATE.with(|g| g.borrow_mut().take())?;
    let out = f(gate.as_mut());
    GATE.with(|g| *g.borrow_mut() = Some(gate));
    Some(out)
}

/// The guest acting now, as their turn begins and ends.
pub fn set_actor(uuid: Option<u128>) {
    ACTOR.with(|a| a.set(uuid));
}
/// The mob behind the damage the next [`player_hurt`] asks about.
pub fn set_attacker(mob: Option<u32>) {
    ATTACKER.with(|a| a.set(mob));
}

/// `LivingEntity.hurtServer` past the checks that refuse a hit outright.
pub fn entity_hurt(mob: &Mob, amount: f32, source: &Damage) -> Option<f32> {
    with(|g| g.entity_hurt(mob, amount, source)).flatten()
}
/// `LivingEntity.checkTotemDeathProtection` for a player with no totem.
pub fn player_death(damage_type: &str) -> bool {
    let actor = ACTOR.with(|a| a.get());
    with(|g| g.player_death(actor, damage_type)).unwrap_or(false)
}
/// `ServerPlayer.hurtServer`, before armor.
pub fn player_hurt(damage_type: &str, amount: f32, at: [f64; 3], health: f32) -> Option<f32> {
    let attacker = ATTACKER.with(|a| a.take());
    let actor = ACTOR.with(|a| a.get());
    with(|g| g.player_hurt(actor, damage_type, attacker, amount, at, health)).flatten()
}
