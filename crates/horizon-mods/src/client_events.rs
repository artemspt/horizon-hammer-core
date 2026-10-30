//! Client bus events for render, input, screen, key and HUD parity.

use crate::event_bus::{Event, ModBusEvent};

macro_rules! client_event {
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

client_event!(RenderHudEvent { tick: u64 });
client_event!(RenderWorldEvent { tick: u64 });
client_event!(ScreenOpenEvent { screen: String });
client_event!(ScreenCloseEvent { screen: String });
client_event!(KeyPressEvent { key: String });
client_event!(MouseClickEvent { button: i32 });
client_event!(HudOverlayEvent { overlay: String });
client_event!(ModelLoadEvent { model: String });
client_event!(TextureAtlasEvent { atlas: String });

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_events_cancel() {
        let mut e = KeyPressEvent {
            key: "a".into(),
            cancelled: false,
        };
        e.set_cancelled(true);
        assert!(e.is_cancelled());
    }
}
