//! Custom payload registration core, NeoForge play payload parity.

use crate::parity_models::CustomPayloadDef;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct PayloadRegistry {
    payloads: BTreeMap<String, CustomPayloadDef>,
}

impl PayloadRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, def: CustomPayloadDef) {
        self.payloads.insert(def.id.clone(), def);
    }
    pub fn get(&self, id: &str) -> Option<&CustomPayloadDef> {
        self.payloads.get(id)
    }
    pub fn len(&self) -> usize {
        self.payloads.len()
    }
    pub fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }
    pub fn ids(&self) -> Vec<String> {
        self.payloads.keys().cloned().collect()
    }
    pub fn validate(&self, id: &str, size: usize) -> Result<(), String> {
        let def = self.get(id).ok_or_else(|| "unknown payload".to_string())?;
        if size > def.max_bytes {
            return Err("payload too large".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn payload_registry_works() {
        let mut r = PayloadRegistry::new();
        r.register(CustomPayloadDef::serverbound("chan"));
        assert_eq!(r.len(), 1);
        r.validate("chan", 1).unwrap();
        assert!(r.validate("chan", 100000).is_err());
    }
}
