//! Recipe resolver / conditions / composites support.

use crate::parity_models::{RecipeConditionDef, RecipeSerializerDef};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeComposite {
    And,
    Or,
    Alternate,
}

#[derive(Clone, Debug, Default)]
pub struct RecipeResolver {
    serializers: Vec<RecipeSerializerDef>,
    conditions: Vec<RecipeConditionDef>,
}

impl RecipeResolver {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_serializer(&mut self, s: RecipeSerializerDef) {
        self.serializers.push(s);
    }
    pub fn add_condition(&mut self, c: RecipeConditionDef) {
        self.conditions.push(c);
    }
    pub fn serializers(&self) -> &[RecipeSerializerDef] {
        &self.serializers
    }
    pub fn conditions(&self) -> &[RecipeConditionDef] {
        &self.conditions
    }
    pub fn matches(&self, id: &str) -> bool {
        self.conditions.iter().any(|c| c.id == id)
    }
    pub fn evaluate_composite(&self, composite: RecipeComposite, ids: &[&str]) -> bool {
        match composite {
            RecipeComposite::And => ids.iter().all(|id| self.matches(id)),
            RecipeComposite::Or => ids.iter().any(|id| self.matches(id)),
            RecipeComposite::Alternate => ids.len() <= 1 || ids.iter().any(|id| self.matches(id)),
        }
    }
    pub fn clear(&mut self) {
        self.serializers.clear();
        self.conditions.clear();
    }
    pub fn has_serializer(&self, id: &str) -> bool {
        self.serializers.iter().any(|s| s.id.to_string() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn recipe_resolver_works() {
        let mut r = RecipeResolver::new();
        r.add_serializer(RecipeSerializerDef::new(
            ResourceLocation::parse("mod:s").unwrap(),
        ));
        r.add_condition(RecipeConditionDef::new("has_gold"));
        assert!(r.matches("has_gold"));
        assert_eq!(r.serializers().len(), 1);
        assert!(r.evaluate_composite(RecipeComposite::And, &["has_gold"]));
        assert!(!r.evaluate_composite(RecipeComposite::And, &["has_gold", "missing"]));
        assert!(r.evaluate_composite(RecipeComposite::Or, &["missing", "has_gold"]));
    }
}
