//! Attribute modifier resolver and stack rules.
use crate::parity_models::AttributeModifierDef;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedAttribute {
    pub attribute: String,
    pub base: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Default)]
pub struct AttributeManager {
    base: BTreeMap<String, f64>,
    modifiers: BTreeMap<String, Vec<AttributeModifierDef>>,
}

impl AttributeManager {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_base(&mut self, attr: impl Into<String>, base: f64) {
        self.base.insert(attr.into(), base);
    }
    pub fn add_modifier(&mut self, modifier: AttributeModifierDef) {
        self.modifiers
            .entry(modifier.attribute.clone())
            .or_default()
            .push(modifier);
    }
    pub fn compute(&self, attribute: &str) -> ComputedAttribute {
        let base = self.base.get(attribute).copied().unwrap_or(0.0);
        let mut total = base;
        if let Some(mods) = self.modifiers.get(attribute) {
            // NeoForge ordering: add values, multiply base, multiply total.
            let mut addend = 0.0f64;
            let mut base_multiplier = 1.0f64;
            let mut total_multiplier = 1.0f64;
            let mut final_addend = 0.0f64;
            for m in mods {
                match m.operation.as_str() {
                    "add" => addend += m.amount,
                    "add_value" => addend += m.amount,
                    "multiply_base" => base_multiplier *= 1.0 + m.amount,
                    "multiply_total" => total_multiplier *= 1.0 + m.amount,
                    "final_add" => final_addend += m.amount,
                    _ => addend += m.amount,
                }
            }
            total = (base + addend) * base_multiplier * total_multiplier + final_addend;
        }
        ComputedAttribute {
            attribute: attribute.to_string(),
            base,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attribute_compute_works() {
        let mut m = AttributeManager::new();
        m.set_base("attack", 2.0);
        let mut add =
            AttributeModifierDef::new(crate::ResourceLocation::parse("mod:s").unwrap(), "attack");
        add.amount = 1.0;
        m.add_modifier(add);
        let mut multiply =
            AttributeModifierDef::new(crate::ResourceLocation::parse("mod:m").unwrap(), "attack");
        multiply.operation = "multiply_total".into();
        multiply.amount = 0.5;
        m.add_modifier(multiply);
        // Add values, then multiply the total: (2 + 1) * 1.5.
        let c = m.compute("attack");
        assert_eq!(c.total, 4.5);
    }
}
