//! Fluid interaction and registration support, a NeoForge-like core slice.

use crate::definitions::FluidDefinition;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FluidInteractionRule {
    pub first: String,
    pub second: String,
    pub result: String,
}

impl FluidInteractionRule {
    pub fn new(
        first: impl Into<String>,
        second: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        Self {
            first: first.into(),
            second: second.into(),
            result: result.into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FluidInteractionResolver {
    rules: Vec<FluidInteractionRule>,
}

impl FluidInteractionResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults() -> Self {
        let mut r = Self::new();
        r.add(FluidInteractionRule::new(
            "minecraft:lava",
            "minecraft:water",
            "minecraft:obsidian",
        ));
        r
    }

    pub fn add(&mut self, rule: FluidInteractionRule) {
        self.rules.push(rule);
    }

    pub fn resolve(&self, a: &str, b: &str) -> Option<&str> {
        for r in &self.rules {
            if (r.first == a && r.second == b) || (r.first == b && r.second == a) {
                return Some(r.result.as_str());
            }
        }
        None
    }

    pub fn rules(&self) -> &[FluidInteractionRule] {
        &self.rules
    }
    pub fn len(&self) -> usize {
        self.rules.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FluidRegistration {
    pub definition: FluidDefinition,
    pub states: Vec<String>,
    pub bucket_item: Option<String>,
}

impl FluidRegistration {
    pub fn new(definition: FluidDefinition) -> Self {
        let id = definition.id.to_string();
        Self {
            definition,
            states: vec![format!("{id}_still"), format!("{id}_flowing")],
            bucket_item: Some(format!("{id}_bucket")),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct FluidRegistryManager {
    fluids: BTreeMap<String, FluidRegistration>,
    buckets: BTreeMap<String, String>,
}

impl FluidRegistryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, registration: FluidRegistration) {
        let id = registration.definition.id.to_string();
        if let Some(bucket) = &registration.bucket_item {
            self.buckets.insert(bucket.clone(), id.clone());
        }
        self.fluids.insert(id, registration);
    }

    pub fn fluid(&self, id: &str) -> Option<&FluidRegistration> {
        self.fluids.get(id)
    }
    pub fn fluids(&self) -> Vec<&FluidRegistration> {
        self.fluids.values().collect()
    }
    pub fn bucket(&self, item: &str) -> Option<&str> {
        self.buckets.get(item).map(String::as_str)
    }
    pub fn count(&self) -> usize {
        self.fluids.len()
    }
    pub fn contains(&self, id: &str) -> bool {
        self.fluids.contains_key(id)
    }

    pub fn properties(&self, id: &str) -> Option<(i32, i32, i32, i32)> {
        self.fluid(id).map(|f| {
            (
                f.definition.density,
                f.definition.viscosity,
                f.definition.temperature,
                f.definition.luminance,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;

    #[test]
    fn defaults_resolve() {
        let r = FluidInteractionResolver::with_defaults();
        assert_eq!(
            r.resolve("minecraft:lava", "minecraft:water"),
            Some("minecraft:obsidian")
        );
        assert_eq!(
            r.resolve("minecraft:water", "minecraft:lava"),
            Some("minecraft:obsidian")
        );
        assert_eq!(r.resolve("minecraft:water", "minecraft:water"), None);
    }

    #[test]
    fn fluid_registry_registers_and_queries() {
        let mut m = FluidRegistryManager::new();
        let def = FluidDefinition::new(ResourceLocation::parse("mod:oil").unwrap(), "Oil");
        m.register(FluidRegistration::new(def));
        assert!(m.contains("mod:oil"));
        assert_eq!(m.count(), 1);
        assert!(m.fluid("mod:oil").unwrap().states.len() >= 2);
        assert!(m.properties("mod:oil").is_some());
    }
}
