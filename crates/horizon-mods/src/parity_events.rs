//! Additional runtime/config/payload/attachment events for NeoForge parity.

use crate::event_bus::{Event, ModBusEvent};

macro_rules! parity_event {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name { $( pub $field : $ty, )* pub cancelled: bool, }
        impl Event for $name {
            fn is_cancellable(&self) -> bool { true }
            fn is_cancelled(&self) -> bool { self.cancelled }
            fn set_cancelled(&mut self, value: bool) { self.cancelled = value; }
        }
        impl ModBusEvent for $name {}
    };
}

parity_event!(ConfigReloadEvent { config: String });
parity_event!(AttachmentSyncEvent {
    target: String,
    attachment: String
});
parity_event!(CapabilityProviderRegisteredEvent { capability: String });
parity_event!(CustomPayloadEvent {
    channel: String,
    bytes: usize
});
parity_event!(LootTableLoadEvent { table: String });
parity_event!(RecipeLoadEvent { recipe: String });
parity_event!(RecipeConditionEvent { condition: String });
parity_event!(DataPackReloadedEvent { success: bool });
parity_event!(ResourceOverlayEvent { namespace: String });
parity_event!(WorldgenFeatureEvent {
    dimension: String,
    feature: String
});
parity_event!(CustomMenuEvent { menu_type: String });
parity_event!(FluidCapabilityEvent {
    item: String,
    fluid: String
});
parity_event!(EnchantmentCostEvent {
    item: String,
    level: i32
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parity_events_cancel() {
        let mut e = LootTableLoadEvent {
            table: "t".into(),
            cancelled: false,
        };
        e.set_cancelled(true);
        assert!(e.is_cancelled());
    }
}
