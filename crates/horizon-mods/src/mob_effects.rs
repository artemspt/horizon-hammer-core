//! Mob effect resolver: apply, merge, cap amplifiers, duration stacking.

use crate::definitions::MobEffectDefinition;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveEffect {
    pub effect: String,
    pub amplifier: i32,
    pub duration: i32,
}

#[derive(Clone, Debug, Default)]
pub struct MobEffectManager {
    effects: BTreeMap<String, BTreeMap<String, ActiveEffect>>, // entity -> effect id -> active
}

impl MobEffectManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn apply(
        &mut self,
        entity: &str,
        def: &MobEffectDefinition,
        amplifier: i32,
        duration: i32,
    ) -> ActiveEffect {
        let amp = amplifier.clamp(0, def.max_amplifier.max(0));
        let ent = self.effects.entry(entity.to_string()).or_default();
        let existing = ent.get(&def.id.to_string());
        let new_amp = if let Some(e) = existing {
            if e.amplifier > amp { e.amplifier } else { amp }
        } else {
            amp
        };
        let mut new_dur = duration;
        if let Some(e) = existing {
            new_dur = new_dur.max(e.duration);
        }
        let active = ActiveEffect {
            effect: def.id.to_string(),
            amplifier: new_amp,
            duration: new_dur,
        };
        ent.insert(def.id.to_string(), active.clone());
        active
    }
    pub fn remove(&mut self, entity: &str, effect: &str) -> Option<ActiveEffect> {
        self.effects.get_mut(entity).and_then(|e| e.remove(effect))
    }
    pub fn tick(&mut self, entity: &str) -> Vec<String> {
        let mut removed = Vec::new();
        if let Some(map) = self.effects.get_mut(entity) {
            for e in map.values_mut() {
                e.duration -= 1;
            }
            let expired: Vec<String> = map
                .iter()
                .filter(|(_, e)| e.duration <= 0)
                .map(|(id, _)| id.clone())
                .collect();
            for id in expired {
                map.remove(&id);
                removed.push(id);
            }
        }
        removed
    }
    pub fn active(&self, entity: &str, effect: &str) -> Option<&ActiveEffect> {
        self.effects.get(entity).and_then(|m| m.get(effect))
    }
    pub fn beneficial_active(
        &self,
        entity: &str,
        defs: &HashMap<String, MobEffectDefinition>,
    ) -> Vec<&ActiveEffect> {
        self.effects
            .get(entity)
            .map(|m| {
                m.values()
                    .filter(|a| defs.get(&a.effect).map(|d| d.beneficial).unwrap_or(false))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn count(&self) -> usize {
        self.effects.values().map(|m| m.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn apply_merge_cap_and_tick() {
        let mut m = MobEffectManager::new();
        let def = MobEffectDefinition {
            id: ResourceLocation::parse("mod:poison").unwrap(),
            name: "poison".into(),
            beneficial: false,
            instant: false,
            color: 1,
            max_amplifier: 2,
        };
        let a = m.apply("p", &def, 1, 40);
        assert_eq!(a.amplifier, 1);
        let b = m.apply("p", &def, 5, 20);
        assert_eq!(b.amplifier, 2); // capped max
        assert_eq!(b.duration, 40); // keeps higher duration
        let expired = m.tick("p");
        assert!(expired.is_empty());
        // advance 40 ticks
        for _ in 0..40 {
            m.tick("p");
        }
        assert!(m.active("p", &def.id.to_string()).is_none());
    }
}
