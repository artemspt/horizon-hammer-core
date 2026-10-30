//! Block state property resolver and validation helpers.

use crate::definitions::BlockStatePropertyDefinition;

#[derive(Clone, Debug, Default)]
pub struct BlockStatePropertyResolver {
    properties: Vec<BlockStatePropertyDefinition>,
}

impl BlockStatePropertyResolver {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, mut property: BlockStatePropertyDefinition) {
        if property.default.is_empty()
            && let Some(v) = property.values.first().cloned()
        {
            property.default = v;
        }
        self.properties.push(property);
    }
    pub fn properties_for(&self, block: &str) -> Vec<BlockStatePropertyDefinition> {
        self.properties
            .iter()
            .filter(|p| p.block == block)
            .cloned()
            .collect()
    }
    pub fn valid(&self, block: &str, property: &str, value: &str) -> bool {
        self.properties.iter().any(|p| {
            p.block == block && p.property == property && p.values.iter().any(|v| v == value)
        })
    }
    pub fn count(&self) -> usize {
        self.properties.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn block_state_property_validation() {
        let mut r = BlockStatePropertyResolver::new();
        let mut p = BlockStatePropertyDefinition::new(
            ResourceLocation::parse("mod:stone").unwrap(),
            "minecraft:stone",
            "facing",
        );
        p.values = vec!["north".into(), "south".into()];
        r.register(p);
        assert!(r.valid("minecraft:stone", "facing", "north"));
        assert!(!r.valid("minecraft:stone", "facing", "up"));
        assert_eq!(r.count(), 1);
    }
}
