//! `entities` at API level 21: a mob's state, read from what the game lends
//! the call. Every level's mobs answer where they are; the level in place's
//! answer the rest ([`MobState`]), made only while a running mod imports one
//! of those reads ([`Mods::reads_mobs`](super::Mods::reads_mobs)).
use super::server_api::{EntityInfo, HostStack};
use super::{ServerPlayer, State};

/// A mob of the level in place, as the game has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MobState {
    pub id: String,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity: (f64, f64, f64),
    pub max_health: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub burning: bool,
    pub baby: bool,
    pub tamed: bool,
    pub owner: Option<String>,
    pub custom_name: Option<String>,
    /// (effect, ticks left, amplifier).
    pub effects: Vec<(String, i32, u8)>,
    /// (attribute, now, base).
    pub attributes: Vec<(String, f64, f64)>,
    /// (slot, item).
    pub equipment: Vec<(String, String)>,
    pub target: Option<String>,
    pub passengers: Vec<String>,
    pub vehicle: Option<String>,
    pub tags: Vec<String>,
}

/// An item on the ground, for `entities.items-near`.
#[derive(Clone, Debug, PartialEq)]
pub struct DroppedItem {
    pub id: String,
    pub pos: (f64, f64, f64),
    pub stack: HostStack,
}

/// The reads only a [`MobState`] answers: a mod importing one makes the
/// game build them for each call.
pub const MOB_STATE_READS: [&str; 22] = [
    "entities.rotation",
    "entities.velocity",
    "entities.max-health",
    "entities.is-on-ground",
    "entities.is-in-water",
    "entities.is-in-lava",
    "entities.is-burning",
    "entities.is-baby",
    "entities.is-tamed",
    "entities.owner",
    "entities.custom-name",
    "entities.effects",
    "entities.has-effect",
    "entities.attribute",
    "entities.base-attribute",
    "entities.equipment",
    "entities.target",
    "entities.passengers",
    "entities.vehicle",
    "entities.tags",
    "entities.has-tag",
    "entities.look-at",
];

/// A tag `/tag` takes: `[A-Za-z0-9_.+-]`, up to 64.
pub(super) fn check_tag(tag: &str) -> Result<(), String> {
    let ok = !tag.is_empty()
        && tag.len() <= 64
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_.+-".contains(c));
    if ok {
        Ok(())
    } else {
        Err(format!("tag {tag:?}: [A-Za-z0-9_.+-], up to 64"))
    }
}

/// SNBT for `/data merge`: one compound on one line.
pub(super) fn check_snbt(nbt: &str) -> Result<(), String> {
    let t = nbt.trim();
    if t.len() > 8192 || !t.starts_with('{') || !t.ends_with('}') || t.contains(['\n', '\r']) {
        return Err("nbt: one SNBT compound on one line, up to 8192 bytes".into());
    }
    Ok(())
}

/// A spawn reason as `before-events.entity-spawn` hears it.
pub(super) fn check_reason(reason: &str) -> Result<(), String> {
    let ok = !reason.is_empty()
        && reason.len() <= 32
        && reason
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(format!("reason {reason:?}: [a-z0-9_], up to 32"))
    }
}

impl State {
    /// A mob of any level, and its level.
    pub(super) fn find_mob(&self, id: &str) -> Option<(String, EntityInfo)> {
        let view = self.world.lock().ok()?;
        let view = view.as_ref()?;
        view.dimensions().into_iter().find_map(|d| {
            let e = view.mobs(&d.id).into_iter().find(|e| e.id == id)?;
            Some((d.id, e))
        })
    }
    pub(super) fn find_player(&self, id: &str) -> Option<&ServerPlayer> {
        self.snapshot.players.iter().find(|p| p.id == id)
    }
    pub(super) fn mob_state(&self, id: &str) -> Option<MobState> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.mob_state(id)
    }
    pub(super) fn mobs_of(&self, dimension: &str) -> Vec<EntityInfo> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref().map_or_else(Vec::new, |v| v.mobs(dimension))
    }
    pub(super) fn dropped_near(&self, dimension: &str) -> Vec<DroppedItem> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref()
            .map_or_else(Vec::new, |v| v.dropped_items(dimension))
    }
    /// Where an entity or player is: its dimension and feet.
    pub(super) fn whereabouts(&self, id: &str) -> Option<(String, (f64, f64, f64))> {
        if let Some(p) = self.find_player(id) {
            return Some((p.dimension.clone(), (p.x, p.y, p.z)));
        }
        let (dim, e) = self.find_mob(id)?;
        Some((dim, e.pos))
    }
}
