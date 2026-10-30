//! Tag manager core for NeoForge-like tag entries.

use crate::definitions::TagDefinition;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct TagManager {
    tags: BTreeMap<String, TagDefinition>,
    entries: BTreeMap<String, BTreeSet<String>>,
}

impl TagManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, tag: TagDefinition) {
        self.tags.insert(tag.id.to_string(), tag.clone());
        let mut set = BTreeSet::new();
        for e in tag.entries {
            set.insert(e);
        }
        self.entries.insert(tag.id.to_string(), set);
    }
    pub fn contains_tag(&self, id: &str) -> bool {
        self.tags.contains_key(id)
    }
    pub fn entries(&self, id: &str) -> Option<Vec<String>> {
        self.entries.get(id).map(|s| s.iter().cloned().collect())
    }
    pub fn add_entry(&mut self, tag: &str, entry: String) {
        self.entries
            .entry(tag.to_string())
            .or_default()
            .insert(entry);
    }
    pub fn has_entry(&self, tag: &str, entry: &str) -> bool {
        self.entries
            .get(tag)
            .map(|s| s.contains(entry))
            .unwrap_or(false)
    }
    pub fn count(&self) -> usize {
        self.tags.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn tags_register_and_resolve_entries() {
        let mut m = TagManager::new();
        let mut t = TagDefinition::new(ResourceLocation::parse("mod:stone_tools").unwrap(), "item");
        t.entries.push("minecraft:iron_pickaxe".into());
        m.register(t);
        assert!(m.contains_tag("mod:stone_tools"));
        assert!(m.has_entry("mod:stone_tools", "minecraft:iron_pickaxe"));
        m.add_entry("mod:stone_tools", "minecraft:diamond_pickaxe".into());
        assert_eq!(m.entries("mod:stone_tools").unwrap().len(), 2);
    }
}
