//! Loot/resolver system: weighted tables, condition evaluation, modifier injection.

use crate::definitions::LootModifierDefinition;
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootEntry {
    pub item: String,
    pub weight: i32,
    pub count_min: i32,
    pub count_max: i32,
    pub conditions: Vec<String>,
}

impl LootEntry {
    pub fn new(item: impl Into<String>, weight: i32) -> Self {
        Self {
            item: item.into(),
            weight,
            count_min: 1,
            count_max: 1,
            conditions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootTable {
    pub id: String,
    pub entries: Vec<LootEntry>,
    pub rolls: i32,
    pub empty_weight: i32,
}

impl LootTable {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            entries: Vec::new(),
            rolls: 1,
            empty_weight: 1,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LootContext {
    pub conditions: HashSet<String>,
    pub luck: f32,
    pub seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootDrop {
    pub item: String,
    pub count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootModifierEntry {
    pub item: String,
    pub weight: i32,
}

#[derive(Clone, Debug, Default)]
pub struct LootResolver {
    tables: BTreeMap<String, LootTable>,
    modifiers: Vec<LootModifierDefinition>,
}

impl LootResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_table(&mut self, table: LootTable) {
        self.tables.insert(table.id.clone(), table);
    }

    pub fn table(&self, id: &str) -> Option<&LootTable> {
        self.tables.get(id)
    }

    pub fn add_modifier(&mut self, modifier: LootModifierDefinition) {
        self.modifiers.push(modifier);
    }

    pub fn evaluate(&self, table: &LootTable, ctx: LootContext) -> Vec<LootDrop> {
        let mut all_entries = table.entries.clone();
        let mut extra_rolls: i32 = 0;
        for modifier in &self.modifiers {
            if modifier.loot_table != table.id {
                continue;
            }
            let mut ok = true;
            for c in &modifier.conditions {
                if !ctx.conditions.contains(c) {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }
            extra_rolls += modifier.bonus_rolls;
            for item in &modifier.added_entries {
                all_entries.push(LootEntry {
                    item: item.clone(),
                    weight: 1,
                    count_min: 1,
                    count_max: 1,
                    conditions: Vec::new(),
                });
            }
            // Remove denied entries by item id
            if !modifier.removed_entries.is_empty() {
                let removed: BTreeSet<_> = modifier.removed_entries.iter().cloned().collect();
                all_entries.retain(|e| !removed.contains(&e.item));
            }
        }

        let mut rng = DeterministicRng::new(ctx.seed);
        let rolls = (table.rolls + extra_rolls).max(0);
        let mut drops = Vec::new();

        for _ in 0..rolls {
            let filtered: Vec<&LootEntry> = all_entries
                .iter()
                .filter(|e| e.conditions.iter().all(|c| ctx.conditions.contains(c)))
                .collect();
            let total: i64 = filtered
                .iter()
                .map(|e| weighted_value(e.weight, ctx.luck).max(0) as i64)
                .sum::<i64>()
                + table.empty_weight as i64;
            if total <= 0 {
                continue;
            }
            let tick = rng.next() % (total as u64);
            let mut cursor = table.empty_weight.max(0) as u64;
            let mut chosen: Option<&LootEntry> = None;
            // include empty entry slot
            if tick >= cursor {
                for e in &filtered {
                    let w = weighted_value(e.weight, ctx.luck).max(0) as u64;
                    cursor += w;
                    if tick < cursor {
                        chosen = Some(e);
                        break;
                    }
                }
            }
            if let Some(e) = chosen {
                let count_range = e.count_max.saturating_sub(e.count_min).max(0);
                let count =
                    e.count_min + (rng.next() as i32 % (count_range + 1)).clamp(0, count_range);
                drops.push(LootDrop {
                    item: e.item.clone(),
                    count: count.max(1),
                });
            }
        }

        // Merge duplicate items
        let mut merged: BTreeMap<String, i32> = BTreeMap::new();
        for d in drops {
            *merged.entry(d.item).or_default() += d.count;
        }
        merged
            .into_iter()
            .map(|(item, count)| LootDrop { item, count })
            .collect()
    }

    pub fn resolve_table(&self, id: &str, ctx: LootContext) -> Result<Vec<LootDrop>, String> {
        let t = self
            .tables
            .get(id)
            .ok_or_else(|| format!("missing loot table {id}"))?;
        Ok(self.evaluate(t, ctx))
    }
}

fn weighted_value(weight: i32, luck: f32) -> i32 {
    ((((weight as f32) * (1.0 + luck * 0.01)).round()) as i32).max(1)
}

struct DeterministicRng {
    state: u64,
}
impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed },
        }
    }
    fn next(&mut self) -> u64 {
        let mut z = self.state;
        z ^= z << 13;
        z ^= z >> 7;
        z ^= z << 17;
        self.state = z;
        z
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;

    #[test]
    fn resolve_weighted_table_deterministically() {
        let mut resolver = LootResolver::new();
        let mut table = LootTable::new("minecraft:chests/test");
        table.entries.push(LootEntry {
            item: "mod:diamond".into(),
            weight: 1,
            count_min: 1,
            count_max: 2,
            conditions: Vec::new(),
        });
        table.entries.push(LootEntry {
            item: "mod:coal".into(),
            weight: 3,
            count_min: 1,
            count_max: 1,
            conditions: Vec::new(),
        });
        table.rolls = 8;
        resolver.add_table(table.clone());
        let ctx = LootContext {
            seed: 42,
            ..Default::default()
        };
        let drops = resolver
            .resolve_table("minecraft:chests/test", ctx.clone())
            .unwrap();
        assert!(!drops.is_empty());
        // Determinism
        let drops2 = resolver
            .resolve_table("minecraft:chests/test", ctx.clone())
            .unwrap();
        assert_eq!(drops, drops2);
    }

    #[test]
    fn condition_filters_entry() {
        let mut resolver = LootResolver::new();
        let mut table = LootTable::new("t");
        let mut e = LootEntry::new("mod:rare", 10);
        e.conditions.push("boss_killed".into());
        table.entries.push(e);
        table.entries.push(LootEntry::new("mod:common", 1));
        table.rolls = 20;
        resolver.add_table(table);
        let drops = resolver
            .resolve_table(
                "t",
                LootContext {
                    seed: 7,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(drops.iter().all(|d| d.item == "mod:common"));
    }

    #[test]
    fn modifier_injects_and_removes() {
        let mut resolver = LootResolver::new();
        let mut table = LootTable::new("t2");
        table.entries.push(LootEntry::new("mod:bad", 10));
        table.rolls = 1;
        resolver.add_table(table);
        let id = ResourceLocation::parse("mod:m").unwrap();
        let mut m = LootModifierDefinition::new(id, "t2");
        m.added_entries.push("mod:good".into());
        m.removed_entries.push("mod:bad".into());
        m.bonus_rolls = 1;
        resolver.add_modifier(m);
        let drops = resolver
            .resolve_table(
                "t2",
                LootContext {
                    seed: 99,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!drops.iter().any(|d| d.item == "mod:bad"));
        assert!(drops.iter().any(|d| d.item == "mod:good"));
    }

    #[test]
    fn missing_table_errors() {
        let r = LootResolver::new();
        assert!(r.resolve_table("missing", LootContext::default()).is_err());
    }
}
