//! Event buses for NeoForge-like modding.
//!
//! There are two buses a mod sees:
//! - the **mod bus**: per-mod construction/registration/setup (`FML...SetupEvent`,
//!   `RegisterEvent`, capabilities), fired while mods load;
//! - the **game bus**: runtime gameplay events (ticks, chat, entity hurt, ...).
//!
//! Both share this implementation. Handlers run in priority order (highest
//! first); an event that [`Event::is_cancellable`] reports true may be
//! cancelled by an earlier handler, which stops the rest.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

/// Priority an event handler runs at. Higher runs first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct EventPriority(pub i32);

impl EventPriority {
    pub const HIGHEST: EventPriority = EventPriority(1000);
    pub const HIGH: EventPriority = EventPriority(500);
    pub const NORMAL: EventPriority = EventPriority(0);
    pub const LOW: EventPriority = EventPriority(-500);
    pub const LOWEST: EventPriority = EventPriority(-1000);
}

/// Base trait for all events. `post` hands the handler a `&mut` event, so
/// cancellation (and any mutable state the event carries) lives in the event
/// itself.
pub trait Event: Any + Send + Sync {
    fn is_cancellable(&self) -> bool {
        false
    }

    fn is_cancelled(&self) -> bool {
        false
    }

    fn set_cancelled(&mut self, _value: bool) {}

    fn priority(&self) -> EventPriority {
        EventPriority::NORMAL
    }
}

/// Marker for events posted on the mod (startup) bus.
pub trait ModBusEvent: Event {}

/// Marker for events posted on the game (runtime) bus.
pub trait GameBusEvent: Event {}

/// A new registry has been registered.
#[derive(Debug, Clone)]
pub struct NewRegistryEvent {
    pub name: String,
}

impl Event for NewRegistryEvent {}
impl ModBusEvent for NewRegistryEvent {}

/// A registry is frozen after all registration passes.
#[derive(Debug, Clone)]
pub struct RegistryFrozenEvent {
    pub name: String,
}

impl Event for RegistryFrozenEvent {}
impl ModBusEvent for RegistryFrozenEvent {}

/// Generic client-side event marker base.
#[derive(Debug, Clone)]
pub struct ClientEvent {
    pub name: String,
}

impl Event for ClientEvent {}
impl ModBusEvent for ClientEvent {}

/// Generic capability provider query.
#[derive(Debug, Clone)]
pub struct CapabilityQueryEvent {
    pub cape: String,
    pub target: String,
    pub handled: bool,
}

impl Event for CapabilityQueryEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.handled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.handled = value;
    }
}

type EventHandler = Arc<dyn Fn(&mut dyn Any) + Send + Sync>;

#[derive(Clone)]
struct RegisteredHandler {
    priority: EventPriority,
    handler: EventHandler,
}

#[derive(Default)]
pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<RegisteredHandler>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handler at normal priority.
    pub fn register<E, F>(&self, handler: F)
    where
        E: Event,
        F: Fn(&mut E) + Send + Sync + 'static,
    {
        self.register_at(EventPriority::NORMAL.0, handler);
    }

    /// Register a handler at an explicit priority (higher runs first).
    pub fn register_at<E, F>(&self, priority: i32, handler: F)
    where
        E: Event,
        F: Fn(&mut E) + Send + Sync + 'static,
    {
        let handler: EventHandler = Arc::new(move |event: &mut dyn Any| {
            if let Some(e) = event.downcast_mut::<E>() {
                handler(e);
            }
        });
        let mut handlers = self.handlers.write().unwrap();
        let list = handlers.entry(TypeId::of::<E>()).or_default();
        list.push(RegisteredHandler {
            priority: EventPriority(priority),
            handler,
        });
        list.sort_by_key(|h| std::cmp::Reverse(h.priority));
    }

    /// Post an event to all handlers in priority order. Stops early when a
    /// cancellable event has been cancelled.
    pub fn post<E: Event>(&self, event: &mut E) {
        let handlers = {
            let guard = self.handlers.read().unwrap();
            guard.get(&TypeId::of::<E>()).cloned().unwrap_or_default()
        };
        for registered in handlers {
            (registered.handler)(event);
            if event.is_cancellable() && event.is_cancelled() {
                break;
            }
        }
    }

    pub fn handler_count<E: Event>(&self) -> usize {
        let handlers = self.handlers.read().unwrap();
        handlers.get(&TypeId::of::<E>()).map_or(0, |v| v.len())
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        let handlers = self.handlers.read().unwrap();
        f.debug_struct("EventBus")
            .field("event_types", &handlers.len())
            .finish()
    }
}

// ---- Built-in engine events -------------------------------------------------

#[derive(Debug, Clone)]
pub struct TickEvent {
    pub tick: u64,
}

impl Event for TickEvent {}

#[derive(Debug, Clone)]
pub struct ModLoadEvent {
    pub mod_id: String,
}

impl Event for ModLoadEvent {}

/// A mod has just been constructed: its `init` ran. Mod bus (conceptually).
#[derive(Debug, Clone)]
pub struct ModConstructEvent {
    pub mod_id: String,
}

impl Event for ModConstructEvent {}
impl ModBusEvent for ModConstructEvent {}

/// Common setup: registration and other things shared by side. Mod bus.
#[derive(Debug, Clone)]
pub struct CommonSetupEvent;

impl Event for CommonSetupEvent {}
impl ModBusEvent for CommonSetupEvent {}

/// Sided setup happens on the client or the dedicated server. Mod bus.
#[derive(Clone, Debug)]
pub struct SidedSetupEvent {
    pub client: bool,
}

impl Event for SidedSetupEvent {}
impl ModBusEvent for SidedSetupEvent {}

/// Fires at the end of the mod loading (post load). Mod bus.
#[derive(Debug, Clone)]
pub struct ModLoadCompleteEvent;

impl Event for ModLoadCompleteEvent {}
impl ModBusEvent for ModLoadCompleteEvent {}

/// A registry is entering its registration window (events fired on mod bus).
#[derive(Debug, Clone)]
pub struct RegisterEvent {
    pub registry: String,
}

impl Event for RegisterEvent {}

/// Capabilities providers may be added for this or other mods' objects.
#[derive(Debug, Clone)]
pub struct RegisterCapabilitiesEvent;

impl Event for RegisterCapabilitiesEvent {}

/// A mod's configuration is being (re)built from disk.
#[derive(Debug, Clone)]
pub struct ModConfigEvent {
    pub mod_id: String,
    pub phase: ConfigPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigPhase {
    Loading,
    Reloading,
}

impl Event for ModConfigEvent {}

// ---- World, fluid, structure and data-pack integration events ----------

/// A biome is about to be used to generate a chunk: mods may add features,
/// spawns and modifiers. Mutate `add_features` / `remove_features`.
#[derive(Debug, Clone, Default)]
pub struct BiomeLoadingEvent {
    pub biome: String,
    pub add_features: Vec<String>,
    pub remove_features: Vec<String>,
    pub spawns_changed: bool,
}

impl Event for BiomeLoadingEvent {
    fn is_cancellable(&self) -> bool {
        false
    }
}

/// A chunk is generating in dimension `dimension`: mods may place or replace
/// blocks. Cancellable to abort the whole generation step (rare).
#[derive(Debug, Clone)]
pub struct ChunkGenerationEvent {
    pub dimension: String,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub cancelled: bool,
}

impl Event for ChunkGenerationEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A structure template is being placed: allow replacing/processor overrides.
#[derive(Debug, Clone)]
pub struct StructurePlacementEvent {
    pub structure: String,
    pub dimension: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub cancelled: bool,
}

impl Event for StructurePlacementEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A feature is about to be placed (trees, ores, ...).
#[derive(Debug, Clone)]
pub struct FeaturePlacementEvent {
    pub feature: String,
    pub dimension: String,
    pub cancelled: bool,
}

impl Event for FeaturePlacementEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A fluid is about to interact/break/place a block.
#[derive(Debug, Clone)]
pub struct FluidInteractEvent {
    pub fluid: String,
    pub block: String,
    pub cancelled: bool,
}

impl Event for FluidInteractEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A bucket/item is about to be filled or emptied with a fluid.
#[derive(Debug, Clone)]
pub struct BucketFillEvent {
    pub item: String,
    pub fluid: String,
    pub filling: bool,
    pub cancelled: bool,
}

impl Event for BucketFillEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A block state is about to change during placement/break; mods can keep or
/// replace the successor state.
#[derive(Debug, Clone)]
pub struct BlockStateChangeEvent {
    pub dimension: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub old_state: String,
    pub new_state: String,
    pub cancelled: bool,
}

impl Event for BlockStateChangeEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

/// A texture/resource set is being (re)loaded: mods may add/replace assets.
#[derive(Debug, Clone)]
pub struct ResourceReloadEvent {
    pub phase: ResourceReloadPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceReloadPhase {
    Before,
    After,
}

impl Event for ResourceReloadEvent {}

/// A datapack's registries have been (re)loaded: mod data objects resync.
#[derive(Debug, Clone)]
pub struct DatapackReloadEvent {
    pub success: bool,
}

impl Event for DatapackReloadEvent {}

// ---- Selected NeoForge-parity hooks ---------------------------------------

#[derive(Debug, Clone)]
pub struct EnchantmentLevelSetEvent {
    pub item: String,
    pub table: String,
    pub slot: i32,
    pub level: i32,
    pub cancelled: bool,
}
impl Event for EnchantmentLevelSetEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

#[derive(Debug, Clone)]
pub struct PotionApplyEvent {
    pub entity: String,
    pub effect: String,
    pub amplifier: i32,
    pub duration: i32,
    pub cancelled: bool,
}
impl Event for PotionApplyEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

#[derive(Debug, Clone)]
pub struct ContainerOpenEvent {
    pub player: String,
    pub menu_type: String,
    pub cancelled: bool,
}
impl Event for ContainerOpenEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

#[derive(Debug, Clone)]
pub struct ContainerCloseEvent {
    pub player: String,
    pub menu_type: String,
}

impl Event for ContainerCloseEvent {}

#[derive(Debug, Clone)]
pub struct LootModifyEvent {
    pub loot_table: String,
    pub rolls: i32,
    pub cancelled: bool,
}
impl Event for LootModifyEvent {
    fn is_cancellable(&self) -> bool {
        true
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }
    fn set_cancelled(&mut self, value: bool) {
        self.cancelled = value;
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceRuleEvent {
    pub biome: String,
    pub block: String,
    pub depth: i32,
    pub result_block: String,
}

impl Event for SurfaceRuleEvent {}

#[derive(Debug, Clone)]
pub struct BlockStatePropertyEvent {
    pub block: String,
    pub property: String,
    pub value: String,
}

impl Event for BlockStatePropertyEvent {}

#[derive(Debug, Clone)]
pub struct TagSyncEvent {
    pub tag_type: String,
    pub count: usize,
}

impl Event for TagSyncEvent {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[derive(Debug)]
    struct TestEvent {
        value: i32,
    }
    impl Event for TestEvent {}

    #[derive(Debug)]
    struct CancelEvent {
        cancelled: bool,
    }
    impl Event for CancelEvent {
        fn is_cancellable(&self) -> bool {
            true
        }
        fn is_cancelled(&self) -> bool {
            self.cancelled
        }
        fn set_cancelled(&mut self, value: bool) {
            self.cancelled = value;
        }
    }

    #[test]
    fn test_event_bus() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicU32::new(0));
        let c = counter.clone();

        bus.register::<TestEvent, _>(move |e| {
            c.fetch_add(e.value as u32, Ordering::SeqCst);
        });

        let mut first = TestEvent { value: 5 };
        let mut second = TestEvent { value: 3 };
        bus.post(&mut first);
        bus.post(&mut second);
        assert_eq!(counter.load(Ordering::SeqCst), 8);
    }

    #[test]
    fn cancellation_stops_later_handlers() {
        let bus = EventBus::new();
        let calls = Arc::new(AtomicU32::new(0));

        let calls0 = calls.clone();
        bus.register_at(100, move |_e: &mut CancelEvent| {
            calls0.fetch_add(1, Ordering::SeqCst);
        });
        let calls1 = calls.clone();
        bus.register_at(0, move |e: &mut CancelEvent| {
            calls1.fetch_add(1, Ordering::SeqCst);
            e.set_cancelled(true);
        });
        let calls2 = calls.clone();
        bus.register_at(-100, move |_e: &mut CancelEvent| {
            calls2.fetch_add(1, Ordering::SeqCst);
        });

        let mut e = CancelEvent { cancelled: false };
        bus.post(&mut e);
        assert!(e.is_cancelled());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}

/// A compatibility message between mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImcMessage {
    pub sender: String,
    pub receiver: String,
    pub key: String,
    pub payload: String,
}

/// Compatibility messages between mods, a minimal NeoForge InterModComms
/// implementation for the mod core.
#[derive(Debug, Clone, Default)]
pub struct InterModComms {
    messages: Vec<ImcMessage>,
}

impl InterModComms {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send(
        &mut self,
        sender: impl Into<String>,
        receiver: impl Into<String>,
        key: impl Into<String>,
        payload: impl Into<String>,
    ) {
        self.messages.push(ImcMessage {
            sender: sender.into(),
            receiver: receiver.into(),
            key: key.into(),
            payload: payload.into(),
        });
    }

    pub fn messages_for(&self, receiver: &str) -> Vec<ImcMessage> {
        self.messages
            .iter()
            .filter(|m| m.receiver == receiver)
            .cloned()
            .collect()
    }

    pub fn take_messages_for(&mut self, receiver: &str) -> Vec<ImcMessage> {
        let mut out = Vec::new();
        self.messages.retain(|m| {
            if m.receiver == receiver {
                out.push(m.clone());
                false
            } else {
                true
            }
        });
        out
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Declarative event-bus subscriber contract, NeoForge-style.
pub trait EventBusSubscriber {
    fn register(&self, mod_bus: &EventBus, game_bus: &EventBus);
    fn label(&self) -> &str {
        "subscriber"
    }
}
