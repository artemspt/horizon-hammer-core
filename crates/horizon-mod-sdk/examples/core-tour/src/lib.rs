//! Example Horizon server mod for what NeoForge mods lean on (`docs/mods.md`, API level 15): data kept on a player
//! through death (attachments), its own data file (a reload listener's input), messages between mods and an optional
//! link to another (`ModList.isLoaded`), tags, player events, and items dropped on the ground. It writes what it saw to
//! `world.set-saved`, under `report`.
use horizon_mod_sdk::server::{
    Args, Entity, ItemStack, Player, Registration, ServerMod, entities, players, world,
};
use horizon_mod_sdk::storage;

struct CoreTour;

impl ServerMod for CoreTour {
    fn init() -> Result<(Self, Registration), String> {
        Ok((
            CoreTour,
            Registration::new().on_tick().on_blocks().command("tour"),
        ))
    }

    /// A stick calms a mob: its AI stops, and the click is the mod's.
    fn use_entity(&mut self, player: &Player, entity: &Entity, item: &str) -> bool {
        // Paper: walks off wearing a helmet, then goes for the player.
        if item == "minecraft:paper" {
            let helmet = ItemStack::new("minecraft:diamond_helmet", 1);
            let _ = entities::set_equipment(&entity.id, "armor.head", &helmet);
            let _ = entities::move_to(&entity.id, player.x + 8.0, player.y, player.z, 1.0);
            let _ = entities::set_target(&entity.id, Some(&player.id));
            return true;
        }
        if item != "minecraft:stick" {
            return false;
        }
        let _ = entities::set_ai(&entity.id, false);
        let _ = entities::set_data(&entity.id, "calm", b"1");
        true
    }

    /// A feather can't hurt anything.
    fn attack_entity(&mut self, _player: &Player, _entity: &Entity, item: &str) -> bool {
        item == "minecraft:feather"
    }

    fn command(
        &mut self,
        _player: &Player,
        _name: &str,
        _args: &Args,
    ) -> Result<Option<String>, String> {
        Ok(world::get_saved("report").map(|r| String::from_utf8_lossy(&r).into_owned()))
    }

    /// A calmed mob doesn't die.
    fn entity_dying(&mut self, _dimension: &str, entity: &Entity, _killer: Option<&str>) -> bool {
        entities::get_data(&entity.id, "calm").is_some()
    }

    /// A book in the air: a styled stack, and twice the health.
    fn use_item(&mut self, player: &Player, stack: &ItemStack, _off_hand: bool) -> bool {
        if stack.item != "minecraft:book" {
            return false;
        }
        let look = players::ItemLook {
            name: Some("Tour Guide".to_owned()),
            lore: vec!["Seen by every client".to_owned()],
            model: None,
            glint: Some(true),
        };
        if let Ok(styled) = players::style_stack(&ItemStack::new("minecraft:paper", 1), &look) {
            let _ = players::give(&player.id, &styled);
        }
        let _ = entities::set_attribute(&player.id, "minecraft:max_health", 40.0);
        let _ = entities::effect(&player.id, "minecraft:speed", 30, 1);
        let _ = entities::hurt(&player.id, 2.0);
        let _ = entities::teleport(&player.id, &player.dimension, player.x, player.y + 1.0, player.z);
        true
    }

    fn tick(&mut self, _tick: u64) {
        let mut report = Vec::new();
        for event in players::poll_events() {
            if matches!(event.kind.as_str(), "tossed" | "used-item" | "broke-item" | "targeted") {
                let _ = players::tell(&event.player, &format!("{} {}", event.kind, event.detail));
            }
            if event.kind == "died" {
                let deaths = players::get_data(&event.player, "deaths").map_or(0, |d| d.len());
                let _ = players::set_data(&event.player, "deaths", &vec![1; deaths + 1]);
                let dim = "minecraft:overworld";
                let _ = entities::drop_item(dim, 0, 64, 0, &ItemStack::new("minecraft:bone", 1));
                report.push(format!("deaths {}", deaths + 1));
            }
        }
        if let Some(text) = storage::read_resource("data/core_tour/greeting.txt") {
            report.push(String::from_utf8_lossy(&text).trim().to_owned());
        }
        report.push(format!("self {}", world::mod_loaded("core_tour")));
        report.push(format!("other {}", world::mod_loaded("no_such_mod")));
        let _ = world::mod_send("core_tour", "ping", b"hi");
        for (from, channel, data) in world::mod_receive() {
            report.push(format!("got {channel} from {from}: {}", data.len()));
        }
        report.push(format!(
            "tag {}",
            world::item_has_tag("minecraft:oak_log", "minecraft:logs")
        ));
        report.push(format!(
            "blocktag {} {}",
            world::block_has_tag("minecraft:oak_log", "minecraft:logs"),
            world::entity_has_tag("minecraft:zombie", "minecraft:undead"),
        ));
        let _ = world::set_saved("report", report.join("|").as_bytes());
    }
}
horizon_mod_sdk::export_server_mod!(CoreTour);
