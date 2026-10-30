//! Runtime game-bus events for NeoForge parity: player, entity, level,
//! inventory, block/container, interaction, command, crafting, item use, ticks.
//!
//! These are contracts that the game runtime calls when wired later.

use crate::event_bus::{Event, GameBusEvent};

macro_rules! simple_game_event {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name { $( pub $field : $ty, )* pub cancelled: bool, }
        impl Event for $name {
            fn is_cancellable(&self) -> bool { true }
            fn is_cancelled(&self) -> bool { self.cancelled }
            fn set_cancelled(&mut self, value: bool) { self.cancelled = value; }
        }
        impl GameBusEvent for $name {}
    };
}

simple_game_event!(PlayerTickEvent {
    player: String,
    tick: u64
});
simple_game_event!(PlayerDeathEvent {
    player: String,
    killer: String
});
simple_game_event!(PlayerRespawnEvent { player: String });
simple_game_event!(EntityTickEvent {
    entity: String,
    tick: u64
});
simple_game_event!(EntityJoinLevelEvent {
    entity: String,
    dimension: String
});
simple_game_event!(EntityLeaveLevelEvent { entity: String });
simple_game_event!(LevelTickEvent {
    dimension: String,
    tick: u64
});
simple_game_event!(BlockBrokenEvent {
    player: String,
    block: String,
    x: i32,
    y: i32,
    z: i32
});
simple_game_event!(BlockPlacedEvent {
    player: String,
    block: String,
    x: i32,
    y: i32,
    z: i32
});
simple_game_event!(BlockUseEvent {
    player: String,
    block: String,
    x: i32,
    y: i32,
    z: i32
});
simple_game_event!(ItemUseEvent {
    player: String,
    item: String
});
simple_game_event!(ItemUseOnBlockEvent {
    player: String,
    item: String,
    block: String,
    x: i32,
    y: i32,
    z: i32
});
simple_game_event!(InventoryChangedEvent {
    player: String,
    slot: i32
});
simple_game_event!(ContainerOpenGameEvent {
    player: String,
    menu_type: String
});
simple_game_event!(ContainerCloseGameEvent {
    player: String,
    menu_type: String
});
simple_game_event!(CraftingEvent {
    player: String,
    output: String
});
simple_game_event!(CommandExecuteEvent {
    command: String,
    sender: String
});
simple_game_event!(LevelLoadEvent { dimension: String });
simple_game_event!(LevelUnloadEvent { dimension: String });
simple_game_event!(ExplosionEvent {
    dimension: String,
    x: f64,
    y: f64,
    z: f64
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_events_cancel() {
        let mut e = PlayerTickEvent {
            player: "p".into(),
            tick: 1,
            cancelled: false,
        };
        assert!(!e.is_cancelled());
        e.set_cancelled(true);
        assert!(e.is_cancelled());
    }
}
