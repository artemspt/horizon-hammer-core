//! Thread-safe bridge between game-thread mod worldgen state and the terrain
//! worker.
//!
//! The worker cannot borrow `ModsState`; it receives an immutable snapshot
//! published by the game thread. Absent mods or absent data yields no
//! changes, so vanilla terrain stays untouched.

use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(feature = "mods")]
use std::sync::{Mutex, OnceLock};

#[cfg(feature = "mods")]
use horizon_mods::Mods;

#[derive(Clone, Default, Debug)]
pub struct WorldgenSnapshot {
    /// Biome -> preferred surface block when a mod surface rule applies.
    pub surface_blocks: BTreeMap<String, String>,
}

#[cfg(feature = "mods")]
static SNAPSHOT: OnceLock<Mutex<Option<Arc<WorldgenSnapshot>>>> = OnceLock::new();

#[cfg(feature = "mods")]
fn cell() -> &'static Mutex<Option<Arc<WorldgenSnapshot>>> {
    SNAPSHOT.get_or_init(|| Mutex::new(None))
}

/// Publishes the rules of the running server mods for the terrain worker:
/// the last rule for a biome wins, as `surface_block_for` reads them.
#[cfg(feature = "mods")]
pub fn publish(mods: &Mods) {
    let mut snapshot = WorldgenSnapshot::default();
    for rule in mods.worldgen().surface_rules() {
        if rule.min_depth <= 0 && rule.max_depth >= 0 {
            snapshot
                .surface_blocks
                .insert(rule.biome.clone(), rule.block.clone());
        }
    }
    *cell().lock().expect("worldgen snapshot poisoned") = Some(Arc::new(snapshot));
}

#[cfg(feature = "mods")]
pub fn snapshot() -> Option<Arc<WorldgenSnapshot>> {
    cell().lock().expect("worldgen snapshot poisoned").clone()
}

#[cfg(feature = "mods")]
pub fn clear() {
    *cell().lock().expect("worldgen snapshot poisoned") = None;
}

#[cfg(not(feature = "mods"))]
pub fn publish(_mods: &()) {}

#[cfg(not(feature = "mods"))]
pub fn snapshot() -> Option<Arc<WorldgenSnapshot>> {
    None
}

#[cfg(not(feature = "mods"))]
pub fn clear() {}

#[cfg(all(test, feature = "mods"))]
mod tests {
    use super::*;
    use horizon_mods::ResourceLocation;

    #[test]
    fn publish_carries_the_rules_to_the_worker() {
        clear();
        let mut mods = Mods::default();
        let id = ResourceLocation::parse("probe:worldgen").expect("valid");
        mods.worldgen_mut().set_surface_rule(
            id.clone(),
            "minecraft:plains",
            "minecraft:diamond_block",
        );
        publish(&mods);
        let published = snapshot().expect("published");
        assert_eq!(
            published
                .surface_blocks
                .get("minecraft:plains")
                .map(String::as_str),
            Some("minecraft:diamond_block")
        );

        // A reload sets the rule again; it replaces, it does not add up.
        mods.worldgen_mut()
            .set_surface_rule(id, "minecraft:plains", "minecraft:gold_block");
        publish(&mods);
        let published = snapshot().expect("published");
        assert_eq!(published.surface_blocks.len(), 1);
        assert_eq!(
            published.surface_blocks["minecraft:plains"],
            "minecraft:gold_block"
        );
        clear();
    }
}
