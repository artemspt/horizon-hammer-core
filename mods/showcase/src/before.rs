//! `/showcase before on|off|check`: every `before-events` hook, each with a
//! rule a player can see, and how often the game asked it.
use horizon_mod_sdk::server::{BlockPos, Entity, ItemStack, Player, world};
use std::collections::BTreeMap;

/// What the hooks do while on, as `/showcase before check` tells it.
pub const RULES: &str = "cows take no damage, bats don't spawn, falls hurt half, \
    you survive one death, oak planks resist explosions, rotten flesh stays on the ground, \
    diamonds can't be dropped, zombies drop an emerald, mobs drop twice the experience, \
    barrels stay shut";

#[derive(Default)]
pub struct Before {
    pub on: bool,
    /// One death survived since `on`.
    spared: bool,
    /// How often each hook was asked, and how often its rule acted.
    asked: BTreeMap<&'static str, (u32, u32)>,
}

impl Before {
    fn count(&mut self, hook: &'static str, acted: bool) -> bool {
        let n = self.asked.entry(hook).or_default();
        n.0 += 1;
        let acted = acted && self.on;
        if acted {
            n.1 += 1;
        }
        acted
    }
    pub fn switch(&mut self, on: bool) {
        self.on = on;
        self.spared = false;
        self.asked.clear();
    }
    /// Each hook: asked / acted.
    pub fn report(&self) -> String {
        if self.asked.is_empty() {
            return "No hook asked yet".into();
        }
        self.asked
            .iter()
            .map(|(hook, (asked, acted))| format!("{hook} {asked}/{acted}"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn entity_spawn(&mut self, entity: &Entity) -> bool {
        self.count("entity-spawn", entity.kind == "minecraft:bat")
    }
    pub fn entity_hurt(&mut self, entity: &Entity, amount: f32) -> f32 {
        if self.count("entity-hurt", entity.kind == "minecraft:cow") {
            0.0
        } else {
            amount
        }
    }
    pub fn player_hurt(&mut self, damage_type: &str, amount: f32) -> f32 {
        if self.count("player-hurt", damage_type == "minecraft:fall") {
            amount / 2.0
        } else {
            amount
        }
    }
    pub fn player_death(&mut self) -> bool {
        let spare = self.count("player-death", !self.spared);
        if spare {
            self.spared = true;
        }
        spare
    }
    pub fn explosion_blocks(&mut self, dimension: &str, blocks: &[BlockPos]) -> Vec<BlockPos> {
        let planks: Vec<BlockPos> = blocks
            .iter()
            .filter(|p| {
                world::block_id(dimension, p.x, p.y, p.z).as_deref() == Some("minecraft:oak_planks")
            })
            .copied()
            .collect();
        if self.count("explosion-blocks", !planks.is_empty()) {
            planks
        } else {
            Vec::new()
        }
    }
    pub fn item_pickup(&mut self, stack: &ItemStack) -> bool {
        self.count("item-pickup", stack.item == "minecraft:rotten_flesh")
    }
    pub fn item_toss(&mut self, stack: &ItemStack) -> bool {
        self.count("item-toss", stack.item == "minecraft:diamond")
    }
    pub fn mob_drop(&mut self, entity: &Entity, drops: &[ItemStack]) -> Option<Vec<ItemStack>> {
        if !self.count("mob-drop", entity.kind == "minecraft:zombie") {
            return None;
        }
        let mut drops = drops.to_vec();
        drops.push(ItemStack::new("minecraft:emerald", 1));
        Some(drops)
    }
    pub fn xp_drop(&mut self, amount: u32) -> u32 {
        if self.count("xp-drop", amount > 0) {
            amount * 2
        } else {
            amount
        }
    }
    pub fn container_open(&mut self, player: &Player, block: &str) -> bool {
        let shut = self.count("container-open", block == "minecraft:barrel");
        if shut {
            super::say(player, "The barrel is shut (/showcase before off)");
        }
        shut
    }
}
