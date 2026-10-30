//! Resource overlay manager used by mods-as-resource-packs parity.

use crate::parity_models::ResourceOverlayDef;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct OverlayManager {
    overlays: BTreeMap<String, ResourceOverlayDef>,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, def: ResourceOverlayDef) {
        self.overlays.insert(def.mod_id.clone(), def);
    }
    pub fn names(&self) -> Vec<String> {
        self.overlays.keys().cloned().collect()
    }
    pub fn get(&self, k: &str) -> Option<&ResourceOverlayDef> {
        self.overlays.get(k)
    }
    pub fn count(&self) -> usize {
        self.overlays.len()
    }
    pub fn json(&self) -> String {
        serde_json::to_string(&self.overlays).unwrap_or_else(|_| "{}".into())
    }
    pub fn from_json(&mut self, text: &str) {
        if let Ok(parsed) = serde_json::from_str::<BTreeMap<String, ResourceOverlayDef>>(text) {
            self.overlays = parsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overlay_manager_roundtrip() {
        let mut m = OverlayManager::new();
        m.add(ResourceOverlayDef::new("moda", "namespace"));
        assert_eq!(m.count(), 1);
        let j = m.json();
        let mut reloaded = OverlayManager::new();
        reloaded.from_json(&j);
        assert_eq!(reloaded.count(), 1);
    }
}
