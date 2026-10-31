//! `before-events` in the game that runs server mods: each hook asked where
//! the thing happens, before it does. Damage to a mob is asked inside the
//! mob's own tick, where the game is borrowed: around such phases
//! [`Game::mod_gated`] hands the mods to `horizon_client::mod_gate`, and
//! [`Game::mods_now`] finds them wherever they are meanwhile. With no mod
//! listening each hook is one check.
use super::Game;

#[cfg(not(feature = "mods"))]
impl Game<'_> {
    pub(super) fn mod_gated<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        f(self)
    }
    pub(super) fn mod_refuses_mob(
        &mut self,
        _m: &horizon_client::mobs::Mob,
        _reason: &str,
    ) -> bool {
        false
    }
    pub(super) fn mod_spare_exploded(&mut self, _center: [f64; 3], _targets: &mut Vec<[i32; 3]>) {}
    pub(super) fn mods_refuse_pickup(
        &mut self,
        _host: &mut super::lan::LanHost,
        _conn: u32,
        _stack: &horizon_client::items::ItemStack,
    ) -> bool {
        false
    }
    pub(super) fn mods_refuse_toss(
        &mut self,
        _host: &mut super::lan::LanHost,
        _conn: u32,
        _all: bool,
    ) -> bool {
        false
    }
    pub(super) fn mods_refuse_container(
        &mut self,
        _host: &mut super::lan::LanHost,
        _conn: u32,
        _pos: [i32; 3],
    ) -> bool {
        false
    }
    pub(super) fn mod_mob_drops(
        &mut self,
        _m: &horizon_client::mobs::Mob,
        _drops: &[horizon_client::mobs::drops::Drop],
        _raider: bool,
        _consumed: bool,
    ) -> Option<(Vec<horizon_client::items::ItemStack>, u32)> {
        None
    }
}

#[cfg(feature = "mods")]
mod with_mods {
    use super::super::lan::LanHost;
    use super::super::mod_machines::{game_stack, host_stack, uuid_string, uuid_words};
    use super::Game;
    use horizon_client::items::ItemStack;
    use horizon_client::mobs::{Damage, Mob, Who};
    use horizon_mods::{EntityInfo, Mods, ServerPlayer};
    use std::any::Any;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        /// The mob dying now and the player who killed it, from `mob_died`
        /// for its loot (`before-mob-drop`).
        static KILLER: RefCell<Option<(u32, Option<String>)>> = const { RefCell::new(None) };
    }

    /// The mods while a phase has them: what a mob's damage is asked of.
    struct ModGate {
        mods: Mods,
        dimension: &'static str,
        /// The mobs' UUIDs as mods see them, by entity id.
        uuids: HashMap<u32, String>,
        /// UUIDs given to mobs that had none, for the game to keep.
        fresh: Vec<(u32, [i32; 4])>,
        /// The acting player's id, for a hit `Who::Player` dealt.
        player: Option<String>,
        /// The players as the phase began, for the one a hit lands on.
        players: Vec<ServerPlayer>,
    }
    impl ModGate {
        fn uuid(&mut self, id: u32) -> String {
            if let Some(u) = self.uuids.get(&id) {
                return u.clone();
            }
            let words = horizon_client::save::random_uuid();
            let u = uuid_string(uuid_words(words));
            self.fresh.push((id, words));
            self.uuids.insert(id, u.clone());
            u
        }
    }
    impl horizon_client::mod_gate::Gate for ModGate {
        fn entity_hurt(&mut self, mob: &Mob, amount: f32, source: &Damage) -> Option<f32> {
            let by_player = source.attacker == Some(Who::Player);
            let attacker = match source.attacker {
                Some(Who::Mob(id)) => Some(self.uuid(id)),
                Some(Who::Player) => self.player.clone(),
                None => None,
            };
            let entity = EntityInfo {
                id: self.uuid(mob.id),
                kind: mob.kind.id.to_string(),
                pos: (mob.position.x, mob.position.y, mob.position.z),
                health: mob.health,
            };
            let damage_type = super::super::creatures::damage_type(source.kind, by_player);
            self.mods.before_entity_hurt(
                self.dimension,
                &entity,
                damage_type,
                attacker.as_deref(),
                amount,
            )
        }
        fn player_hurt(
            &mut self,
            actor: Option<u128>,
            damage_type: &str,
            attacker: Option<u32>,
            amount: f32,
            at: [f64; 3],
            health: f32,
        ) -> Option<f32> {
            let id = actor.map(uuid_string).or_else(|| self.player.clone())?;
            let attacker = attacker.map(|m| self.uuid(m));
            // A guest who joined during the phase: what the hit shows of them.
            let mut player = self
                .players
                .iter()
                .find(|p| p.id == id)
                .cloned()
                .unwrap_or_else(|| ServerPlayer {
                    id: id.clone(),
                    name: id.clone(),
                    x: at[0],
                    y: at[1],
                    z: at[2],
                    dimension: self.dimension.to_owned(),
                    game_mode: horizon_mods::GameMode::Survival,
                    health,
                });
            (player.x, player.y, player.z, player.health) = (at[0], at[1], at[2], health);
            self.mods
                .before_player_hurt(&player, damage_type, attacker.as_deref(), amount)
        }
        fn player_death(&mut self, actor: Option<u128>, damage_type: &str) -> bool {
            let Some(id) = actor.map(uuid_string).or_else(|| self.player.clone()) else {
                return false;
            };
            let Some(player) = self.players.iter().find(|p| p.id == id).cloned() else {
                return false;
            };
            self.mods.before_player_death(&player, damage_type)
        }
        fn as_any(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Game<'_> {
        /// Whether this game runs server mods that list `event`.
        pub(in crate::game) fn hears_before(&mut self, event: &str) -> bool {
            self.mods.server && self.remote.is_none() && self.mods_now(|m| m.wants_before(event))
        }

        /// The mods, in the game or held by the gate of the phase running.
        pub(in crate::game) fn mods_now<R>(&mut self, f: impl FnOnce(&mut Mods) -> R) -> R {
            if horizon_client::mod_gate::is_open() {
                let mut f = Some(f);
                let held = horizon_client::mod_gate::with(|g| {
                    g.as_any()
                        .downcast_mut::<ModGate>()
                        .map(|gate| (f.take().expect("asked once"))(&mut gate.mods))
                })
                .flatten();
                if let Some(out) = held {
                    return out;
                }
                if let Some(f) = f {
                    return f(&mut self.mods.mods);
                }
                unreachable!("the gate answered without asking");
            }
            f(&mut self.mods.mods)
        }

        /// Runs a phase that may hurt mobs inside their own code with the mods
        /// in the gate, when one asks `before-entity-hurt`.
        pub(in crate::game) fn mod_gated<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
            if horizon_client::mod_gate::is_open()
                || !(self.hears_before("before-entity-hurt")
                    || self.hears_before("before-player-hurt")
                    || self.hears_before("before-player-death"))
            {
                return f(self);
            }
            self.mod_name_mobs();
            let uuids = self
                .persist
                .uuids
                .iter()
                .map(|(&id, &w)| (id, uuid_string(uuid_words(w))))
                .collect();
            let player =
                (self.lan_acting.is_some() || self.player.is_some()).then(|| self.mod_acting_id());
            let players = self.server_players(None);
            horizon_client::mod_gate::open(Box::new(ModGate {
                mods: std::mem::take(&mut self.mods.mods),
                dimension: self.dim.id(),
                uuids,
                fresh: Vec::new(),
                player,
                players,
            }));
            let out = f(self);
            let gate = horizon_client::mod_gate::close().expect("the phase's gate");
            let mut gate: Box<dyn Any> = gate;
            if let Some(gate) = gate.downcast_mut::<ModGate>() {
                self.mods.mods = std::mem::take(&mut gate.mods);
                for (id, words) in gate.fresh.drain(..) {
                    self.persist.uuids.entry(id).or_insert(words);
                }
            }
            self.show_mod_output();
            out
        }

        /// `mob_died`: who killed `id`, for its loot.
        pub(in crate::game) fn mod_note_killer(&mut self, id: u32, killer: Option<String>) {
            KILLER.with(|k| *k.borrow_mut() = Some((id, killer)));
        }

        /// A mob's record as server mods see it, given a UUID if it had none.
        fn mod_mob_record(&mut self, m: &Mob) -> EntityInfo {
            let words = self.uuid_for(m.id);
            EntityInfo {
                id: uuid_string(uuid_words(words)),
                kind: m.kind.id.to_string(),
                pos: (m.position.x, m.position.y, m.position.z),
                health: m.health,
            }
        }

        /// `before-entity-spawn`, as `m` (its id given) is about to join the
        /// level in place: true when a mod refuses it.
        pub(in crate::game) fn mod_refuses_mob(&mut self, m: &Mob, reason: &str) -> bool {
            if !self.hears_before("before-entity-spawn") {
                return false;
            }
            let entity = self.mod_mob_record(m);
            let dim = self.dim.id();
            let refused = self.mods_now(|mods| mods.before_entity_spawn(dim, &entity, reason));
            if refused {
                self.persist.uuids.remove(&m.id);
            }
            refused
        }

        /// `before-explosion-blocks`: takes the blocks mods spare out of
        /// `targets`.
        pub(in crate::game) fn mod_spare_exploded(
            &mut self,
            center: [f64; 3],
            targets: &mut Vec<[i32; 3]>,
        ) {
            if targets.is_empty() || !self.hears_before("before-explosion-blocks") {
                return;
            }
            let dim = self.dim.id();
            let at = (center[0], center[1], center[2]);
            self.lend_world();
            let spared = self.mods_now(|m| m.before_explosion_blocks(dim, at, targets));
            self.return_world();
            if !spared.is_empty() {
                let spared: std::collections::HashSet<[i32; 3]> = spared.into_iter().collect();
                targets.retain(|p| !spared.contains(p));
            }
        }

        /// `before-item-pickup` for guest `conn` about to take `stack`: true
        /// leaves it on the ground.
        pub(in crate::game) fn mods_refuse_pickup(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            stack: &ItemStack,
        ) -> bool {
            if !self.hears_before("before-item-pickup") {
                return false;
            }
            let Some(player) = self.guest_server_player(host, conn) else {
                return false;
            };
            let stack = host_stack(stack);
            self.lend_world();
            let refused = self.mods_now(|m| m.before_item_pickup(&player, &stack));
            self.return_world();
            self.route_mod_output(Some(host));
            refused
        }

        /// `before-item-toss` for guest `conn` about to drop the held stack
        /// (`all` of it, or one): true keeps it, and their game is told
        /// their inventory again, since it took the stack away already.
        pub(in crate::game) fn mods_refuse_toss(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            all: bool,
        ) -> bool {
            if !self.hears_before("before-item-toss") {
                return false;
            }
            let Some(player) = self.guest_server_player(host, conn) else {
                return false;
            };
            let Some(Some(mut held)) =
                self.as_guest(host, conn, |g| g.inventory[g.selected].clone())
            else {
                return false;
            };
            if !all {
                held.count = held.count.min(1);
            }
            let stack = host_stack(&held);
            self.lend_world();
            let refused = self.mods_now(|m| m.before_item_toss(&player, &stack));
            self.return_world();
            self.route_mod_output(Some(host));
            if refused && let Some(gi) = host.hub.guests.iter().position(|g| g.conn == conn) {
                host.hub.send_inventory(gi);
            }
            refused
        }

        /// `before-container-open` for guest `conn` at `pos`: true keeps the
        /// window shut.
        pub(in crate::game) fn mods_refuse_container(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            pos: [i32; 3],
        ) -> bool {
            if !self.hears_before("before-container-open") {
                return false;
            }
            let Some(player) = self.guest_server_player(host, conn) else {
                return false;
            };
            let block = self
                .res
                .facts()
                .get(self.world.block(pos[0], pos[1], pos[2]))
                .name
                .to_string();
            self.lend_world();
            let refused = self.mods_now(|m| m.before_container_open(&player, pos, &block));
            self.return_world();
            self.route_mod_output(Some(host));
            refused
        }

        /// `before-mob-drop` and `before-xp-drop` (`mob`) for a dead mob's
        /// loot: what drops instead, when a mod listens; `consumed` means a
        /// sculk catalyst took the experience.
        pub(in crate::game) fn mod_mob_drops(
            &mut self,
            m: &Mob,
            drops: &[horizon_client::mobs::drops::Drop],
            raider: bool,
            consumed: bool,
        ) -> Option<(Vec<ItemStack>, u32)> {
            use horizon_client::mobs::drops::Drop;
            let items_heard = self.hears_before("before-mob-drop");
            if !items_heard && !self.hears_before("before-xp-drop") {
                return None;
            }
            let mut stacks = Vec::new();
            let mut xp = 0u32;
            for d in drops {
                match d {
                    Drop::Item(stack) => stacks.push(
                        if raider && stack.item == horizon_client::mobs::raider::BANNER {
                            horizon_client::mobs::raider::ominous_banner()
                        } else {
                            super::super::creatures::from_loot(stack)
                        },
                    ),
                    Drop::Orb(value) if !consumed => xp += (*value).max(0) as u32,
                    Drop::Orb(_) => {}
                }
            }
            let entity = self.mod_mob_record(m);
            let killer = KILLER.with(|k| k.borrow().as_ref().filter(|k| k.0 == m.id)?.1.clone());
            let dim = self.dim.id();
            self.lend_world();
            if items_heard {
                let shown: Vec<_> = stacks.iter().map(host_stack).collect();
                if let Some(changed) = self
                    .mods_now(|mods| mods.before_mob_drop(dim, &entity, killer.as_deref(), &shown))
                {
                    stacks = changed.iter().filter_map(game_stack).collect();
                }
            }
            if xp > 0 {
                xp = self.mods_now(|mods| mods.before_xp_drop(dim, entity.pos, "mob", xp));
            }
            self.return_world();
            Some((stacks, xp))
        }
    }
}
