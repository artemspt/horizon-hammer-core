//! World-generation modifier manager, a NeoForge-like approximation layer.
//!
//! This is the core-side model. It keeps biome feature modifiers, carver
//! modifiers, spawn/feature lists and surface rules, and applies them whenever a
//! dimension is loaded. Linking to the world generator's actual feature
//! registry is intentionally deferred, but the model and tests are real.

use crate::SurfaceRuleDefinition;
use crate::game_events::LevelLoadEvent;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureAdd {
    pub biome: String,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureRemoval {
    pub biome: String,
    pub features: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BiomeModifier {
    pub id: String,
    pub biome: String,
    pub add_features: Vec<String>,
    pub remove_features: Vec<String>,
    pub spawn_modifiers: Vec<String>,
    pub climate_adjustment: Option<String>,
}

impl BiomeModifier {
    pub fn new(id: impl Into<String>, biome: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            biome: biome.into(),
            add_features: Vec::new(),
            remove_features: Vec::new(),
            spawn_modifiers: Vec::new(),
            climate_adjustment: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WorldgenModifierManager {
    modifiers: Vec<BiomeModifier>,
    surface_rules: Vec<SurfaceRuleDefinition>,
    feature_sets: BTreeMap<String, BTreeSet<String>>,
    loaded_biomes: BTreeSet<String>,
}

impl WorldgenModifierManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_modifier(&mut self, modifier: BiomeModifier) {
        self.modifiers.push(modifier);
    }
    pub fn add_surface_rule(&mut self, rule: SurfaceRuleDefinition) {
        self.surface_rules.push(rule);
    }

    /// Sets the surface block of `biome` for the mod `id`, replacing the rule
    /// that mod set before (a reload sets them again, it does not add up).
    pub fn set_surface_rule(
        &mut self,
        id: crate::ResourceLocation,
        biome: impl Into<String>,
        block: impl Into<String>,
    ) {
        let biome = biome.into();
        self.surface_rules
            .retain(|r| r.biome != biome || r.id != id);
        self.surface_rules
            .push(SurfaceRuleDefinition::new(id, biome, block));
    }

    /// Drops every rule of the mod `id` (it was reloaded or is gone).
    pub fn remove_mod(&mut self, id: &str) {
        self.surface_rules.retain(|r| r.id.namespace() != id);
    }

    pub fn modifiers_for(&self, biome: &str) -> Vec<&BiomeModifier> {
        self.modifiers.iter().filter(|m| m.biome == biome).collect()
    }

    pub fn allmodifiers(&self) -> &[BiomeModifier] {
        &self.modifiers
    }
    pub fn surface_rules(&self) -> &[SurfaceRuleDefinition] {
        &self.surface_rules
    }

    pub fn register_feature_set(&mut self, biome: impl Into<String>, features: Vec<String>) {
        self.feature_sets
            .insert(biome.into(), features.into_iter().collect());
    }

    pub fn add_features(&mut self, biome: impl Into<String>, features: Vec<String>) {
        self.feature_sets
            .entry(biome.into())
            .or_default()
            .extend(features);
    }

    pub fn remove_features(&mut self, biome: &str, features: &[String]) {
        if let Some(set) = self.feature_sets.get_mut(biome) {
            for feature in features {
                set.remove(feature);
            }
        }
    }

    pub fn features_for(&self, biome: &str) -> Vec<String> {
        let mut set = self.feature_sets.get(biome).cloned().unwrap_or_default();
        for modifier in self.modifiers_for(biome) {
            for feature in &modifier.add_features {
                set.insert(feature.clone());
            }
            for feature in &modifier.remove_features {
                set.remove(feature);
            }
        }
        set.into_iter().collect()
    }

    pub fn surface_block_for(&self, biome: &str, depth: i32) -> Option<&str> {
        self.surface_rules
            .iter()
            .rfind(|r| r.biome == biome && depth >= r.min_depth && depth <= r.max_depth)
            .map(|r| r.block.as_str())
    }

    pub fn handle_level_load(&mut self, event: &LevelLoadEvent) -> usize {
        self.loaded_biomes.insert(event.dimension.clone());
        // Worldgen modifiers are applied lazily by biome when queried, but loading
        // a level makes biomes known for diagnostics.
        self.loaded_biomes.len()
    }

    pub fn loaded_biomes_count(&self) -> usize {
        self.loaded_biomes.len()
    }
    pub fn modifier_count(&self) -> usize {
        self.modifiers.len()
    }
    pub fn feature_count(&self) -> usize {
        self.feature_sets.values().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LevelLoadEvent;

    #[test]
    fn worldgen_modifiers_add_and_remove_features() {
        let mut mm = WorldgenModifierManager::new();
        mm.register_feature_set("minecraft:plains", vec!["base:ore".into()]);
        let mut modifier = BiomeModifier::new("mod:extra", "minecraft:plains");
        modifier.add_features = vec!["mod:ore".into()];
        modifier.remove_features = vec!["base:ore".into()];
        mm.add_modifier(modifier);
        let features = mm.features_for("minecraft:plains");
        assert!(features.contains(&"mod:ore".to_string()));
        assert!(!features.contains(&"base:ore".to_string()));
        assert_eq!(mm.modifier_count(), 1);
    }

    #[test]
    fn worldgen_surface_rules_replace_per_mod() {
        let mut m = WorldgenModifierManager::new();
        let id = crate::ResourceLocation::parse("probe:worldgen").unwrap();
        m.set_surface_rule(id.clone(), "minecraft:plains", "minecraft:stone");
        m.set_surface_rule(id, "minecraft:plains", "minecraft:diamond_block");
        assert_eq!(m.surface_rules().len(), 1);
        assert_eq!(
            m.surface_block_for("minecraft:plains", 0),
            Some("minecraft:diamond_block")
        );
        m.set_surface_rule(
            crate::ResourceLocation::parse("other:worldgen").unwrap(),
            "minecraft:plains",
            "minecraft:gold_block",
        );
        assert_eq!(
            m.surface_block_for("minecraft:plains", 0),
            Some("minecraft:gold_block")
        );
        m.remove_mod("probe");
        assert_eq!(m.surface_rules().len(), 1);
        m.remove_mod("other");
        assert!(m.surface_rules().is_empty());
    }

    #[test]
    fn worldgen_surface_and_level_load() {
        let mut m = WorldgenModifierManager::new();
        m.add_surface_rule(SurfaceRuleDefinition::new(
            crate::ResourceLocation::parse("mod:rule").unwrap(),
            "minecraft:plains",
            "minecraft:stone",
        ));
        assert_eq!(
            m.surface_block_for("minecraft:plains", 0),
            Some("minecraft:stone")
        );
        let n = m.handle_level_load(&LevelLoadEvent {
            dimension: "minecraft:overworld".into(),
            cancelled: false,
        });
        assert_eq!(n, 1);
        assert_eq!(m.loaded_biomes_count(), 1);
    }
}
