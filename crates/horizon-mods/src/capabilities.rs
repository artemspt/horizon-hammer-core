//! Capability provider registry and attachment query core, NeoForge parity.

use crate::parity_models::{CapabilityProvider, DataAttachment};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct CapabilityProviderRegistry {
    providers: Vec<CapabilityProvider>,
}

impl CapabilityProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, provider: CapabilityProvider) {
        self.providers.push(provider);
    }
    pub fn providers_for(&self, capability: &str, target_kind: &str) -> Vec<&CapabilityProvider> {
        let mut out: Vec<&CapabilityProvider> = self
            .providers
            .iter()
            .filter(|p| p.capability == capability && p.target_kind == target_kind)
            .collect();
        out.sort_by_key(|p| std::cmp::Reverse(p.priority));
        out
    }
    pub fn len(&self) -> usize {
        self.providers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[derive(Clone, Debug, Default)]
pub struct AttachmentManager {
    values: BTreeMap<String, String>,
    synced: BTreeMap<String, bool>,
}

impl AttachmentManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, id: &str, value: impl Into<String>, synced: bool) {
        self.values.insert(id.to_string(), value.into());
        self.synced.insert(id.to_string(), synced);
    }
    pub fn get(&self, id: &str) -> Option<&str> {
        self.values.get(id).map(String::as_str)
    }
    pub fn is_synced(&self, id: &str) -> bool {
        self.synced.get(id).copied().unwrap_or(false)
    }
    pub fn keys(&self) -> Vec<String> {
        self.values.keys().cloned().collect()
    }

    pub fn to_json(&self) -> String {
        let mut map = BTreeMap::new();
        for (k, v) in &self.values {
            map.insert(k.clone(), v.clone());
        }
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn sync_payload(&self) -> String {
        self.to_json()
    }

    pub fn synced_keys(&self) -> Vec<String> {
        self.keys()
            .into_iter()
            .filter(|k| self.is_synced(k))
            .collect()
    }

    pub fn apply_synced(&mut self, text: &str) -> Result<usize, String> {
        let parsed: BTreeMap<String, String> =
            serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut count = 0;
        for (k, v) in parsed {
            if self.values.contains_key(&k) {
                if let Some(flag) = self.synced.get(&k).copied()
                    && flag
                {
                    self.values.insert(k, v);
                    count += 1;
                }
            } else {
                self.values.insert(k, v);
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn from_json(&mut self, text: &str) {
        if let Ok(parsed) = serde_json::from_str::<BTreeMap<String, String>>(text) {
            for (k, v) in parsed {
                self.values.insert(k, v);
            }
        }
    }
}

impl DataAttachment {
    pub fn key(target: &str, id: &str) -> String {
        format!("{target}:{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capability_and_attachment_work() {
        let mut reg = CapabilityProviderRegistry::new();
        reg.register(CapabilityProvider::new(
            crate::ResourceLocation::parse("mod:prov").unwrap(),
            "fluid",
            "block",
        ));
        assert_eq!(reg.providers_for("fluid", "block").len(), 1);
        let mut a = AttachmentManager::new();
        a.set("entity:1:foo", "bar", true);
        assert_eq!(a.get("entity:1:foo"), Some("bar"));
        assert!(a.is_synced("entity:1:foo"));
    }
}
