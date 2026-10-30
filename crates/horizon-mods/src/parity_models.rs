//! Remaining NeoForge-parity data/model contracts (mod-core side).
//!
//! These structs define the "what" for configs, capability providers,
//! attachments, custom payloads, slots, rendering resources and dependency metadata.
//! Runtime/game wiring is deliberately separate.

use crate::ResourceLocation;

#[derive(Clone, Debug, PartialEq)]
pub struct DependencySpec {
    pub mod_id: String,
    pub version_range: String,
    pub required: bool,
    pub ordering: String,
}

impl DependencySpec {
    pub fn required(mod_id: impl Into<String>, version_range: impl Into<String>) -> Self {
        Self {
            mod_id: mod_id.into(),
            version_range: version_range.into(),
            required: true,
            ordering: "after".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CapabilityProvider {
    pub id: ResourceLocation,
    pub capability: String,
    pub target_kind: String,
    pub priority: i32,
}

impl CapabilityProvider {
    pub fn new(
        id: ResourceLocation,
        capability: impl Into<String>,
        target_kind: impl Into<String>,
    ) -> Self {
        Self {
            id,
            capability: capability.into(),
            target_kind: target_kind.into(),
            priority: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataAttachment {
    pub id: ResourceLocation,
    pub target: String,
    pub default: String,
    pub synced: bool,
    pub persisted: bool,
}

impl DataAttachment {
    pub fn new(id: ResourceLocation, target: impl Into<String>) -> Self {
        Self {
            id,
            target: target.into(),
            default: String::new(),
            synced: true,
            persisted: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemPropertiesDef {
    pub id: ResourceLocation,
    pub max_stack: i32,
    pub max_damage: i32,
    pub food_level: i32,
    pub saturation: f32,
    pub creative_tab: String,
    pub repairable: bool,
}

impl ItemPropertiesDef {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            max_stack: 64,
            max_damage: 0,
            food_level: 0,
            saturation: 0.0,
            creative_tab: "misc".into(),
            repairable: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockEntityDef {
    pub id: ResourceLocation,
    pub tickable: bool,
    pub menu: Option<String>,
    pub sync_interval: i32,
}

impl BlockEntityDef {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            tickable: true,
            menu: None,
            sync_interval: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerSlotDef {
    pub index: i32,
    pub kind: String,
    pub max_stack: i32,
    pub quick_move: bool,
}

impl ContainerSlotDef {
    pub fn new(index: i32) -> Self {
        Self {
            index,
            kind: "input".into(),
            max_stack: 64,
            quick_move: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataSlotDef {
    pub name: String,
    pub synced: bool,
}

impl DataSlotDef {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            synced: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CustomPayloadDef {
    pub id: String,
    pub direction: String,
    pub version: i32,
    pub max_bytes: usize,
}

impl CustomPayloadDef {
    pub fn serverbound(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            direction: "serverbound".into(),
            version: 1,
            max_bytes: 32768,
        }
    }
    pub fn clientbound(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            direction: "clientbound".into(),
            version: 1,
            max_bytes: 32768,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigFieldDef {
    pub path: String,
    pub kind: String,
    pub default: String,
    pub range_min: Option<String>,
    pub range_max: Option<String>,
    pub sync: bool,
}

impl ConfigFieldDef {
    pub fn new(path: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: kind.into(),
            default: String::new(),
            range_min: None,
            range_max: None,
            sync: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigSpecDef {
    pub name: String,
    pub side: String,
    pub fields: Vec<ConfigFieldDef>,
    pub file: String,
}

impl ConfigSpecDef {
    pub fn new(name: impl Into<String>, side: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            side: side.into(),
            fields: Vec::new(),
            file: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderTypeInfo {
    pub id: ResourceLocation,
    pub shader_type: String,
    pub cutout: bool,
}

impl RenderTypeInfo {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            shader_type: "solid".into(),
            cutout: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleProviderDef {
    pub id: ResourceLocation,
    pub sprite: String,
    pub max_age: i32,
}

impl ParticleProviderDef {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            sprite: String::new(),
            max_age: 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundInstanceDef {
    pub id: ResourceLocation,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
}

impl SoundInstanceDef {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            volume: 1.0,
            pitch: 1.0,
            looping: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AIGoalDef {
    pub id: ResourceLocation,
    pub priority: i32,
    pub conditions: Vec<String>,
}

impl AIGoalDef {
    pub fn new(id: ResourceLocation, priority: i32) -> Self {
        Self {
            id,
            priority,
            conditions: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AttributeModifierDef {
    pub id: ResourceLocation,
    pub attribute: String,
    pub operation: String,
    pub amount: f64,
    pub slot: String,
}

impl AttributeModifierDef {
    pub fn new(id: ResourceLocation, attribute: impl Into<String>) -> Self {
        Self {
            id,
            attribute: attribute.into(),
            operation: "add".into(),
            amount: 0.0,
            slot: "mainhand".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecipeSerializerDef {
    pub id: ResourceLocation,
    pub kind: String,
}

impl RecipeSerializerDef {
    pub fn new(id: ResourceLocation) -> Self {
        Self {
            id,
            kind: "crafting".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecipeConditionDef {
    pub id: String,
    pub expression: String,
}

impl RecipeConditionDef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            expression: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResourceOverlayDef {
    pub mod_id: String,
    pub namespace: String,
    pub priority: i32,
}

impl ResourceOverlayDef {
    pub fn new(mod_id: impl Into<String>, namespace: impl Into<String>) -> Self {
        Self {
            mod_id: mod_id.into(),
            namespace: namespace.into(),
            priority: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModCompatibility {
    pub mod_id: String,
    pub ranges: String,
    pub exclude: bool,
}

impl ModCompatibility {
    pub fn supports(mod_id: impl Into<String>, ranges: impl Into<String>) -> Self {
        Self {
            mod_id: mod_id.into(),
            ranges: ranges.into(),
            exclude: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParityChecklist {
    pub name: String,
    pub items: Vec<String>,
    pub done: Vec<bool>,
}

impl ParityChecklist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
            done: Vec::new(),
        }
    }
    pub fn add(&mut self, item: impl Into<String>, done: bool) {
        self.items.push(item.into());
        self.done.push(done);
    }
    pub fn complete(&self) -> bool {
        self.items.len() == self.done.len() && self.done.iter().all(|v| *v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_models_construct() {
        let id = ResourceLocation::parse("mod:foo").unwrap();
        assert!(DependencySpec::required("foo", ">=1").required);
        assert!(CapabilityProvider::new(id.clone(), "cap", "block").priority == 0);
        assert!(DataAttachment::new(id.clone(), "entity").synced);
        assert_eq!(ItemPropertiesDef::new(id.clone()).max_stack, 64);
        assert!(BlockEntityDef::new(id.clone()).tickable);
        assert_eq!(ContainerSlotDef::new(0).max_stack, 64);
        assert!(DataSlotDef::new("num").synced);
        assert!(CustomPayloadDef::serverbound("x").max_bytes > 0);
        assert_eq!(ConfigFieldDef::new("x", "int").path, "x");
        assert_eq!(ConfigSpecDef::new("c", "common").side, "common");
        assert_eq!(RenderTypeInfo::new(id.clone()).shader_type, "solid");
        assert_eq!(ParticleProviderDef::new(id.clone()).max_age, 20);
        assert_eq!(SoundInstanceDef::new(id.clone()).volume, 1.0);
        assert_eq!(AIGoalDef::new(id.clone(), 1).priority, 1);
        assert_eq!(AttributeModifierDef::new(id.clone(), "speed").amount, 0.0);
        assert_eq!(RecipeSerializerDef::new(id.clone()).kind, "crafting");
        assert_eq!(RecipeConditionDef::new("a").id, "a");
        assert_eq!(ResourceOverlayDef::new("m", "m").priority, 0);
        assert!(!ModCompatibility::supports("a", "*").exclude);

        let mut checklist = ParityChecklist::new("demo");
        checklist.add("item", true);
        assert!(checklist.complete());
    }
}
