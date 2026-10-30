//! Typed definitions for NeoForge-like object registries, Phase 2.
//!
//! These are the *data shapes* mods (and the game) agree on when they
//! register fluids, effects, menus, loot, structures and so on. They intentionally
//! do not pull in `horizon-world`/`horizon-items`; the game bridges them
//! later.

use crate::ResourceLocation;

#[derive(Clone, Debug, PartialEq)]
pub struct FluidDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub density: i32,
    pub viscosity: i32,
    pub temperature: i32,
    pub luminance: i32,
    pub source_texture: String,
    pub flowing_texture: String,
    pub bucket: Option<String>,
    pub can_convert_to_source: bool,
}

impl FluidDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            density: 1000,
            viscosity: 1000,
            temperature: 300,
            luminance: 0,
            source_texture: String::new(),
            flowing_texture: String::new(),
            bucket: None,
            can_convert_to_source: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MobEffectDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub beneficial: bool,
    pub instant: bool,
    pub color: u32,
    pub max_amplifier: i32,
}

impl MobEffectDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>, beneficial: bool) -> Self {
        Self {
            id,
            name: name.into(),
            beneficial,
            instant: false,
            color: 0,
            max_amplifier: 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnchantmentDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub max_level: i32,
    pub min_cost: i32,
    pub max_cost: i32,
    pub treasure: bool,
    pub curse: bool,
    pub tradeable: bool,
    pub discoverable: bool,
    pub weight: i32,
}

impl EnchantmentDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            max_level: 1,
            min_cost: 1,
            max_cost: 1,
            treasure: false,
            curse: false,
            tradeable: true,
            discoverable: true,
            weight: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub texture: String,
    pub max_age_ticks: i32,
}

impl ParticleDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            texture: String::new(),
            max_age_ticks: 20,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub subtitle: Option<String>,
    pub replace: bool,
}

impl SoundDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            subtitle: None,
            replace: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContainerMenuDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub size: i32,
}

impl ContainerMenuDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>, size: i32) -> Self {
        Self {
            id,
            name: name.into(),
            size,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LootModifierDefinition {
    pub id: ResourceLocation,
    pub loot_table: String,
    pub conditions: Vec<String>,
    pub added_entries: Vec<String>,
    pub removed_entries: Vec<String>,
    pub bonus_rolls: i32,
}

impl LootModifierDefinition {
    pub fn new(id: ResourceLocation, loot_table: impl Into<String>) -> Self {
        Self {
            id,
            loot_table: loot_table.into(),
            conditions: Vec::new(),
            added_entries: Vec::new(),
            removed_entries: Vec::new(),
            bonus_rolls: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StructureDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub piece_type: String,
    pub terrain_adaptation: String,
}

impl StructureDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            piece_type: String::new(),
            terrain_adaptation: "none".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceRuleDefinition {
    pub id: ResourceLocation,
    pub biome: String,
    pub block: String,
    pub min_depth: i32,
    pub max_depth: i32,
}

impl SurfaceRuleDefinition {
    pub fn new(id: ResourceLocation, biome: impl Into<String>, block: impl Into<String>) -> Self {
        Self {
            id,
            biome: biome.into(),
            block: block.into(),
            min_depth: 0,
            max_depth: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DimensionDefinition {
    pub id: ResourceLocation,
    pub name: String,
    pub ambient_light: f32,
    pub has_ceiling: bool,
    pub has_skylight: bool,
    pub natural: bool,
}

impl DimensionDefinition {
    pub fn new(id: ResourceLocation, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            ambient_light: 0.0,
            has_ceiling: false,
            has_skylight: true,
            natural: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockStatePropertyDefinition {
    pub id: ResourceLocation,
    pub block: String,
    pub property: String,
    pub values: Vec<String>,
    pub default: String,
}

impl BlockStatePropertyDefinition {
    pub fn new(
        id: ResourceLocation,
        block: impl Into<String>,
        property: impl Into<String>,
    ) -> Self {
        Self {
            id,
            block: block.into(),
            property: property.into(),
            values: Vec::new(),
            default: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TagDefinition {
    pub id: ResourceLocation,
    pub kind: String,
    pub entries: Vec<String>,
    pub replace: bool,
}

impl TagDefinition {
    pub fn new(id: ResourceLocation, kind: impl Into<String>) -> Self {
        Self {
            id,
            kind: kind.into(),
            entries: Vec::new(),
            replace: false,
        }
    }
}

impl Default for EnchantmentDefinition {
    fn default() -> Self {
        Self {
            id: ResourceLocation::parse("horizon:default").unwrap(),
            name: String::new(),
            max_level: 1,
            min_cost: 1,
            max_cost: 1,
            treasure: false,
            curse: false,
            tradeable: true,
            discoverable: true,
            weight: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_definitions_construct() {
        let id = ResourceLocation::parse("mod:foo").unwrap();
        assert!(MobEffectDefinition::new(id.clone(), "Bar", true).beneficial);
        assert_eq!(ContainerMenuDefinition::new(id.clone(), "M", 9).size, 9);
        assert!(DimensionDefinition::new(id.clone(), "D").has_skylight);
        let fd = FluidDefinition::new(id.clone(), "water");
        assert_eq!(fd.name, "water");
        let bd = BlockStatePropertyDefinition::new(id, "minecraft:stone", "facing");
        assert!(bd.values.is_empty());
    }
}
