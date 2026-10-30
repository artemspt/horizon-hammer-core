//! Registries for moddable game objects, a NeoForge-like foundation.
//!
//! A `Registry<T>` owns one kind of object (blocks, items, entities, ...)
//! keyed by a [`ResourceLocation`]. Mods never mutate it directly during a
//! tick: they hand [`DeferredRegister<T>`] suppliers to it during the
//! registration stage, and the registry fires [`RegisterEvent`] / freezes
//! at the end of that stage so lookups made later see the final set.
//!
//! This module gives the *primitives* and the *manager*; linking it to the
//! game's block/item/entity tables is later work (Phase 2 of the roadmap).

use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use crate::event_bus::Event;

/// `namespace:path`, the key every registry entry and mod object uses.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceLocation {
    namespace: String,
    path: String,
}

impl ResourceLocation {
    pub fn new(namespace: impl Into<String>, path: impl Into<String>) -> Result<Self, String> {
        let namespace = namespace.into();
        let path = path.into();
        if namespace.is_empty() || path.is_empty() {
            return Err("registry name may not be empty".into());
        }
        if !namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("bad registry namespace: {namespace}"));
        }
        if !path.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '/'
        }) {
            return Err(format!("bad registry path: {path}"));
        }
        Ok(Self { namespace, path })
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let (namespace, path) = text
            .split_once(':')
            .ok_or_else(|| format!("registry name needs a colon: {text}"))?;
        Self::new(namespace, path)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for ResourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

#[derive(Debug, Clone)]
pub struct RegistryError {
    pub message: String,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RegistryError {}

/// One registry of one type of object. `T` is the registered value (a block
/// definition, an item, ...). It must be sendable/syncable so the registry
/// can live behind a lock across threads.
pub struct Registry<T> {
    name: ResourceLocation,
    entries: BTreeMap<ResourceLocation, T>,
    frozen: bool,
}

impl<T> Registry<T> {
    pub fn new(name: ResourceLocation) -> Self {
        Self {
            name,
            entries: BTreeMap::new(),
            frozen: false,
        }
    }

    pub fn name(&self) -> &ResourceLocation {
        &self.name
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    pub fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn unfreeze(&mut self) {
        self.frozen = false;
    }

    pub fn contains(&self, id: &ResourceLocation) -> bool {
        self.entries.contains_key(id)
    }

    pub fn get(&self, id: &ResourceLocation) -> Option<&T> {
        self.entries.get(id)
    }

    pub fn register(&mut self, id: ResourceLocation, value: T) -> Result<(), RegistryError> {
        if self.frozen {
            return Err(RegistryError {
                message: format!("registry {} is frozen", self.name),
            });
        }
        if self.entries.contains_key(&id) {
            return Err(RegistryError {
                message: format!("{} is already registered in {}", id, self.name),
            });
        }
        self.entries.insert(id, value);
        Ok(())
    }

    pub fn remove(&mut self, id: &ResourceLocation) -> Option<T> {
        if self.frozen {
            return None;
        }
        self.entries.remove(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn ids(&self) -> Vec<ResourceLocation> {
        self.entries.keys().cloned().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ResourceLocation, &T)> {
        self.entries.iter()
    }
}

/// Accumulates suppliers during a mod's registration and applies them to a
/// `Registry<T>` in one go, NeoForge-style `DeferredRegister`.
pub struct DeferredRegister<T> {
    registry: ResourceLocation,
    namespace: String,
    pending: Mutex<Vec<(String, Box<dyn Fn() -> T + Send + Sync>)>>,
}

impl<T: Send + Sync + 'static> DeferredRegister<T> {
    pub fn new(registry: ResourceLocation, namespace: impl Into<String>) -> Self {
        Self {
            registry,
            namespace: namespace.into(),
            pending: Mutex::new(Vec::new()),
        }
    }

    pub fn registry(&self) -> &ResourceLocation {
        &self.registry
    }

    pub fn register(
        &self,
        name: impl Into<String>,
        supplier: impl Fn() -> T + Send + Sync + 'static,
    ) -> Result<ResourceLocation, String> {
        let path = name.into();
        let id = ResourceLocation::new(self.namespace.clone(), path.clone())?;
        let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        pending.push((path, Box::new(supplier)));
        Ok(id)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Runs every accumulated supplier against `registry`.
    pub fn apply(&self, registry: &mut Registry<T>) -> Result<usize, RegistryError> {
        let pending = {
            let mut lock = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut *lock)
        };
        let mut count = 0;
        for (path, supplier) in pending {
            let id = ResourceLocation::new(self.namespace.clone(), path)
                .map_err(|e| RegistryError { message: e })?;
            registry.register(id, supplier())?;
            count += 1;
        }
        Ok(count)
    }
}

/// Holder used by [`RegistryManager`] to store registries of different
/// entry types behind one erased pointer.
pub struct RegistryHolder<T> {
    pub registry: Arc<RwLock<Registry<T>>>,
}

impl<T: Send + Sync + 'static> RegistryHolder<T> {
    pub fn new(registry: Registry<T>) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
        }
    }
}

/// Manages the set of registries a game/mod environment knows about.
pub struct RegisteredRegistry {
    ops: Arc<dyn RegistryAccess>,
    any: Arc<dyn Any + Send + Sync>,
}

pub struct RegistryManager {
    registries: RwLock<BTreeMap<ResourceLocation, RegisteredRegistry>>,
}

impl Default for RegistryManager {
    fn default() -> Self {
        Self {
            registries: RwLock::new(BTreeMap::new()),
        }
    }
}

impl RegistryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<T: Send + Sync + 'static>(
        &self,
        name: ResourceLocation,
        registry: Registry<T>,
    ) -> Result<(), RegistryError> {
        let mut registries = self.registries.write().unwrap_or_else(|e| e.into_inner());
        if registries.contains_key(&name) {
            return Err(RegistryError {
                message: format!("registry {} is already known", name),
            });
        }
        let holder = Arc::new(RegistryHolder::new(registry));
        registries.insert(
            name,
            RegisteredRegistry {
                ops: holder.clone(),
                any: holder,
            },
        );
        Ok(())
    }

    pub fn contains(&self, name: &ResourceLocation) -> bool {
        self.registries
            .read()
            .map(|r| r.contains_key(name))
            .unwrap_or(false)
    }

    pub fn names(&self) -> Vec<ResourceLocation> {
        self.registries
            .read()
            .map(|r| r.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Freeze every known registry after a registration pass.
    pub fn freeze_all(&self) {
        if let Ok(guard) = self.registries.read() {
            for item in guard.values() {
                item.ops.freeze();
            }
        }
    }

    /// Are all known registries frozen?
    pub fn all_frozen(&self) -> bool {
        self.registries
            .read()
            .map(|g| g.values().all(|i| i.ops.is_frozen()))
            .unwrap_or(true)
    }

    /// Total number of entries across all registries where the count is available.
    pub fn entry_count(&self) -> usize {
        self.registries
            .read()
            .map(|g| g.values().map(|i| i.ops.len()).sum())
            .unwrap_or(0)
    }

    pub fn get_typed<T: Send + Sync + 'static>(
        &self,
        name: &ResourceLocation,
    ) -> Option<Arc<RwLock<Registry<T>>>> {
        let registries = self.registries.read().unwrap_or_else(|e| e.into_inner());
        let holder = registries.get(name)?.any.clone();
        let downcast = holder.downcast::<RegistryHolder<T>>().ok()?;
        Some(downcast.registry.clone())
    }
}

/// Conventional registry names, matching NeoForge-ish grouping.
pub struct Registries;

impl Registries {
    pub fn blocks() -> ResourceLocation {
        ResourceLocation::parse("horizon:blocks").expect("valid")
    }

    pub fn items() -> ResourceLocation {
        ResourceLocation::parse("horizon:items").expect("valid")
    }

    pub fn entities() -> ResourceLocation {
        ResourceLocation::parse("horizon:entities").expect("valid")
    }

    pub fn fluids() -> ResourceLocation {
        ResourceLocation::parse("horizon:fluids").expect("valid")
    }

    pub fn block_entity_types() -> ResourceLocation {
        ResourceLocation::parse("horizon:block-entity-types").expect("valid")
    }

    pub fn biomes() -> ResourceLocation {
        ResourceLocation::parse("horizon:biomes").expect("valid")
    }

    pub fn dimensions() -> ResourceLocation {
        ResourceLocation::parse("horizon:dimensions").expect("valid")
    }

    pub fn features() -> ResourceLocation {
        ResourceLocation::parse("horizon:features").expect("valid")
    }

    pub fn structures() -> ResourceLocation {
        ResourceLocation::parse("horizon:structures").expect("valid")
    }

    pub fn particles() -> ResourceLocation {
        ResourceLocation::parse("horizon:particles").expect("valid")
    }

    pub fn sounds() -> ResourceLocation {
        ResourceLocation::parse("horizon:sounds").expect("valid")
    }

    pub fn block_states() -> ResourceLocation {
        ResourceLocation::parse("horizon:block-states").expect("valid")
    }

    pub fn menu_types() -> ResourceLocation {
        ResourceLocation::parse("horizon:menu-types").expect("valid")
    }

    pub fn mob_effects() -> ResourceLocation {
        ResourceLocation::parse("horizon:mob-effects").expect("valid")
    }

    pub fn enchantments() -> ResourceLocation {
        ResourceLocation::parse("horizon:enchantments").expect("valid")
    }

    pub fn loot_modifiers() -> ResourceLocation {
        ResourceLocation::parse("horizon:loot-modifiers").expect("valid")
    }

    pub fn surface_rules() -> ResourceLocation {
        ResourceLocation::parse("horizon:surface-rules").expect("valid")
    }

    pub fn tags() -> ResourceLocation {
        ResourceLocation::parse("horizon:tags").expect("valid")
    }
}

/// Type-erased registry operations used by [`RegistryManager`] for stages such as freezing.
pub trait RegistryAccess: Send + Sync {
    fn freeze(&self);
    fn is_frozen(&self) -> bool;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Send + Sync + 'static> RegistryAccess for RegistryHolder<T> {
    fn freeze(&self) {
        if let Ok(mut guard) = self.registry.write() {
            guard.freeze();
        }
    }
    fn is_frozen(&self) -> bool {
        self.registry.read().map(|g| g.is_frozen()).unwrap_or(false)
    }
    fn len(&self) -> usize {
        self.registry.read().map(|g| g.len()).unwrap_or(0)
    }
}

/// A registry entry has just been added.
#[derive(Clone, Debug)]
pub struct RegisterEvent {
    pub registry: String,
    pub id: String,
}

impl Event for RegisterEvent {}

/// A registry is being frozen at the end of its registration stage.
#[derive(Clone, Debug)]
pub struct RegistryFreezeEvent {
    pub registry: String,
}

impl Event for RegistryFreezeEvent {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_location_parses_and_validates() {
        let id = ResourceLocation::parse("minecraft:stone").unwrap();
        assert_eq!(id.namespace(), "minecraft");
        assert_eq!(id.path(), "stone");
        assert!(ResourceLocation::parse("no-colon").is_err());
        assert!(ResourceLocation::new("bad!", "x").is_err());
    }

    #[test]
    fn registry_registers_and_freezes() {
        let mut registry = Registry::new(Registries::blocks());
        registry
            .register(ResourceLocation::parse("mod:foo").unwrap(), 7)
            .unwrap();
        assert_eq!(
            registry.get(&ResourceLocation::parse("mod:foo").unwrap()),
            Some(&7)
        );
        registry.freeze();
        assert!(registry.is_frozen());
        assert!(
            registry
                .register(ResourceLocation::parse("mod:bar").unwrap(), 8)
                .is_err()
        );
    }

    #[test]
    fn deferred_register_applies() {
        let deferred = DeferredRegister::new(Registries::items(), "mod");
        deferred.register("sword", || 42u32).unwrap();
        assert_eq!(deferred.pending_count(), 1);
        let mut registry = Registry::new(Registries::items());
        assert_eq!(deferred.apply(&mut registry).unwrap(), 1);
        assert_eq!(registry.len(), 1);
        assert_eq!(
            registry.get(&ResourceLocation::parse("mod:sword").unwrap()),
            Some(&42u32)
        );
    }

    #[test]
    fn registry_manager_downcasts() {
        let manager = RegistryManager::new();
        manager
            .add(
                Registries::blocks(),
                Registry::<i32>::new(Registries::blocks()),
            )
            .unwrap();
        assert!(manager.contains(&Registries::blocks()));
        let typed = manager
            .get_typed::<i32>(&Registries::blocks())
            .expect("typed registry");
        typed
            .write()
            .unwrap()
            .register(ResourceLocation::parse("mod:baz").unwrap(), 1)
            .unwrap();
    }
}
