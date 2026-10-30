//! Enchantment resolution utility: eligibility, cost progression, deterministic selection.

use crate::definitions::EnchantmentDefinition;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct EnchantmentRollContext {
    pub player_level: i32,
    pub luck: f32,
    pub allowed_item_tags: HashSet<String>,
    pub is_treasure: bool,
    pub seed: u64,
}

impl Default for EnchantmentRollContext {
    fn default() -> Self {
        Self {
            player_level: 1,
            luck: 0.0,
            allowed_item_tags: HashSet::new(),
            is_treasure: false,
            seed: 0x9E3779B97F4A7C15,
        }
    }
}

pub struct EnchantmentResolver {
    enchantments: Vec<EnchantmentDefinition>,
    max_pick: usize,
}

impl Default for EnchantmentResolver {
    fn default() -> Self {
        Self {
            enchantments: Vec::new(),
            max_pick: 3,
        }
    }
}

impl EnchantmentResolver {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, e: EnchantmentDefinition) {
        self.enchantments.push(e);
    }
    pub fn all(&self) -> &[EnchantmentDefinition] {
        &self.enchantments
    }

    pub fn eligible(&self, ctx: &EnchantmentRollContext, e: &EnchantmentDefinition) -> bool {
        if e.treasure && !ctx.is_treasure {
            return false;
        }
        e.discoverable || ctx.player_level > 0
    }

    pub fn cost(&self, e: &EnchantmentDefinition, level: i32) -> i32 {
        let lvl = level.clamp(1, e.max_level);
        (e.min_cost + (lvl - 1) * (e.max_cost - e.min_cost).max(1) / e.max_level.max(1)).max(1)
    }

    pub fn pick(&self, ctx: EnchantmentRollContext) -> Vec<(ResourceLocation, i32)> {
        let budget = ctx.player_level.max(1);
        let mut candidates: Vec<(EnchantmentDefinition, i32)> = self
            .enchantments
            .iter()
            .filter(|e| self.eligible(&ctx, e))
            .filter_map(|e| {
                let try_level = e.max_level;
                let c = self.cost(e, try_level);
                if c <= budget {
                    Some((e.clone(), c))
                } else {
                    None
                }
            })
            .collect();
        candidates.sort_by_key(|(_, c)| *c);

        let mut rng = DeterministicRng::new(ctx.seed);
        let mut chosen = Vec::new();
        let mut spent = 0i32;
        while chosen.len() < self.max_pick && !candidates.is_empty() {
            let total: i64 = candidates.iter().map(|(_, c)| (*c) as i64).sum();
            if total <= 0 {
                break;
            }
            let tick = (rng.next() % (total as u64)) as i64;
            let mut cursor = 0i64;
            let mut idx = None;
            for (i, (_, c)) in candidates.iter().enumerate() {
                cursor += *c as i64;
                if tick < cursor {
                    idx = Some(i);
                    break;
                }
            }
            let idx = match idx {
                Some(i) => i,
                None => break,
            };
            let (e, c) = candidates.remove(idx);
            if spent + c > budget {
                continue;
            }
            let lvl = if c > budget { 1 } else { e.max_level };
            let id = e.id.clone();
            spent += c;
            chosen.push((id, lvl));
        }
        chosen
    }

    pub fn from_definitions(defs: Vec<EnchantmentDefinition>) -> Self {
        let mut s = Self::default();
        for d in defs {
            s.add(d);
        }
        s
    }
}

use crate::ResourceLocation;

struct DeterministicRng {
    s: u64,
}
impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            s: if seed == 0 { 1 } else { seed },
        }
    }
    fn next(&mut self) -> u64 {
        self.s = self.s.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EnchantmentDefinition;

    #[test]
    fn cost_progression() {
        let r = EnchantmentResolver::new();
        let e = EnchantmentDefinition {
            id: ResourceLocation::parse("mod:e").unwrap(),
            max_level: 5,
            min_cost: 10,
            max_cost: 50,
            ..Default::default()
        };
        assert!(r.cost(&e, 5) >= r.cost(&e, 1));
    }

    #[test]
    fn deterministic_pick() {
        let mut r = EnchantmentResolver::new();
        r.add(EnchantmentDefinition {
            id: ResourceLocation::parse("mod:a").unwrap(),
            max_level: 3,
            min_cost: 3,
            max_cost: 20,
            ..Default::default()
        });
        r.add(EnchantmentDefinition {
            id: ResourceLocation::parse("mod:b").unwrap(),
            max_level: 2,
            min_cost: 2,
            max_cost: 10,
            ..Default::default()
        });
        let ctx = EnchantmentRollContext {
            player_level: 20,
            seed: 1234,
            ..Default::default()
        };
        let a = r.pick(ctx.clone());
        let b = r.pick(ctx);
        assert_eq!(a, b);
    }
}
