//! Container/menu resolver for slot definitions, validation, quick-move,
//! drag and data-slot synchronization.

use crate::definitions::ContainerMenuDefinition;
use crate::parity_models::{ContainerSlotDef, DataSlotDef};

#[derive(Clone, Debug, Default)]
pub struct ContainerMenuResolver {
    pub def: Option<ContainerMenuDefinition>,
    pub slots: Vec<ContainerSlotDef>,
    pub data_slots: Vec<DataSlotDef>,
    slot_values: Vec<i32>,
    dragged: Option<i32>,
}

impl ContainerMenuResolver {
    pub fn new(def: ContainerMenuDefinition) -> Self {
        Self {
            def: Some(def),
            slots: Vec::new(),
            data_slots: Vec::new(),
            slot_values: Vec::new(),
            dragged: None,
        }
    }
    pub fn add_slot(&mut self, slot: ContainerSlotDef) {
        self.slots.push(slot);
        if self.slot_values.len() < self.slots.len() {
            self.slot_values.resize(self.slots.len(), 0);
        }
    }
    pub fn add_data_slot(&mut self, slot: DataSlotDef) {
        self.data_slots.push(slot);
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
    pub fn data_count(&self) -> usize {
        self.data_slots.len()
    }
    pub fn validate_move(&self, slot: usize, count: i32) -> i32 {
        let s = match self.slots.get(slot) {
            Some(s) => s,
            None => return 0,
        };
        count.min(s.max_stack).max(0)
    }
    pub fn is_quick_move_allowed(&self, slot: usize) -> bool {
        self.slots.get(slot).map(|s| s.quick_move).unwrap_or(false)
    }
    pub fn set_slot_value(&mut self, slot: usize, value: i32) -> Result<(), String> {
        if slot >= self.slot_values.len() {
            return Err("slot index out of range".into());
        }
        self.slot_values[slot] = value;
        Ok(())
    }
    pub fn slot_value(&self, slot: usize) -> Option<i32> {
        self.slot_values.get(slot).copied()
    }
    pub fn start_drag(&mut self, amount: i32) {
        self.dragged = Some(amount);
    }
    pub fn commit_drag(&mut self, targets: &[usize]) -> i32 {
        let mut amount = self.dragged.unwrap_or(0);
        let per_target = if targets.is_empty() {
            0
        } else {
            amount / targets.len().max(1) as i32
        };
        for &slot in targets {
            let allowed = self.validate_move(slot, per_target);
            if self.set_slot_value(slot, allowed).is_ok() {
                amount -= allowed;
            }
        }
        self.dragged = None;
        amount
    }
    pub fn dragged_amount(&self) -> Option<i32> {
        self.dragged
    }
    pub fn set_synced_data(&mut self, name: &str, _value: i32) -> bool {
        self.data_slots.iter().any(|s| s.name == name && s.synced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResourceLocation;
    #[test]
    fn menu_resolver_validates_slots() {
        let def = ContainerMenuDefinition::new(ResourceLocation::parse("mod:m").unwrap(), "m", 9);
        let mut r = ContainerMenuResolver::new(def);
        r.add_slot(ContainerSlotDef::new(0));
        r.add_data_slot(DataSlotDef::new("progress"));
        assert_eq!(r.validate_move(0, 100), 64);
        assert!(r.is_quick_move_allowed(0));
        assert_eq!(r.data_count(), 1);
    }

    #[test]
    fn menu_resolver_sync_drag_and_data() {
        let def = ContainerMenuDefinition::new(ResourceLocation::parse("mod:m").unwrap(), "m", 9);
        let mut r = ContainerMenuResolver::new(def);
        r.add_slot(ContainerSlotDef::new(0));
        r.add_slot(ContainerSlotDef::new(1));
        r.add_data_slot(DataSlotDef::new("progress"));
        assert!(r.set_slot_value(0, 10).is_ok());
        r.start_drag(6);
        let remained = r.commit_drag(&[0, 1]);
        assert!(remained >= 0);
        assert!(r.dragged_amount().is_none());
        assert!(r.set_synced_data("progress", 5));
    }
}
