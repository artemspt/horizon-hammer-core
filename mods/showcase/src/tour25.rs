//! `/showcase api l25`, then `l25check`: `item-events`. A stick in the main hand
//! goes into a short use, then an iron pickaxe and a sword are for breaking and
//! hitting by hand (or by `horizon-ctl`); the hooks count what they hear.
use horizon_mod_sdk::server::{Entity, ItemStack, Player, players};
use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    static HEARD: RefCell<BTreeMap<String, u32>> = const { RefCell::new(BTreeMap::new()) };
}

pub fn heard(what: &str) {
    HEARD.with(|h| *h.borrow_mut().entry(what.to_owned()).or_default() += 1);
}
fn count(what: &str) -> u32 {
    HEARD.with(|h| h.borrow().get(what).copied().unwrap_or(0))
}

pub fn set(player: &Player) -> Result<String, String> {
    HEARD.with(|h| h.borrow_mut().clear());
    let me = player.id.as_str();
    players::set_slot(me, players::selected_slot(me).unwrap_or(0), Some(&stick()))?;
    players::start_using(me, 10)?;
    Ok("a stick in use for 10 ticks".to_owned())
}
fn stick() -> ItemStack {
    ItemStack {
        item: "minecraft:stick".to_owned(),
        count: 1,
        data: Vec::new(),
        components: Vec::new(),
    }
}

pub fn stop(player: &Player) -> Result<String, String> {
    players::start_using(&player.id, 1000)?;
    Ok("a long use; /showcase api l25stop2 ends it".to_owned())
}
pub fn stop2(player: &Player) -> Result<String, String> {
    players::stop_using(&player.id)?;
    Ok("ended".to_owned())
}

pub fn check() -> Result<String, String> {
    let line: Vec<String> =
        HEARD.with(|h| h.borrow().iter().map(|(k, v)| format!("{k}={v}")).collect());
    let _ = count;
    Ok(format!("heard: {}", line.join(" ")))
}

pub fn use_tick(_: &Player, item: &ItemStack, _: u32) {
    heard(&format!("use-tick:{}", item.item));
}
pub fn hit(_: &Player, item: &ItemStack, target: &Entity) {
    heard(&format!("hit:{}:{}", item.item, target.kind));
}
