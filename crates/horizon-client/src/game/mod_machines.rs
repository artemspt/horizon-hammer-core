//! Server mods' items, storages' windows, vanilla containers, mobs and
//! redstone (`docs/mods.md`, API level 10) in the game that runs them: stacks
//! between the game's `ItemStack` and the mods' `HostStack`, what server
//! mods ask of players' inventories, the windows of their storages, and what
//! the world lends their calls beyond blocks.
use super::{Dim, Game};
use horizon_client::containers::BlockEntity;
use horizon_client::items::ItemStack;
use horizon_client::lan::menu::{MOD_MENU_KEY_Y, Slots};
use horizon_client::text::Text;
use horizon_mods::{EntityInfo, HostStack, ServerAction};
use horizon_world::nbt::{self, JavaString, Limits, Tag};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

const CUSTOM_DATA: &str = "minecraft:custom_data";
/// Where mods' bytes sit in `minecraft:custom_data`: mod id → bytes.
const MODS_KEY: &str = "horizon_mods";
/// The placeholder container a storage's window edits lives under a key no
/// block has (`MOD_MENU_KEY_Y`: the reach checked is to the storage itself,
/// in `sync_mod_menus`); its z counts down from -1.
const MENU_KEY_X: i32 = i32::MAX - 17;

/// A storage's window open for a player.
pub(super) struct ModMenu {
    conn: u32,
    key: (i32, i32, i32),
    /// The level holding the placeholder: the player's.
    level: String,
    dimension: String,
    pos: (i32, i32, i32),
    /// The slots as last copied either way.
    last: Vec<Option<HostStack>>,
}

/// `UUID.toString()`.
pub(super) fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}
pub(super) fn uuid_words(w: [i32; 4]) -> u128 {
    w.iter()
        .fold(0u128, |u, &x| (u << 32) | u128::from(x as u32))
}

/// A game stack as server mods see it: their bytes out of
/// `minecraft:custom_data`, the rest as the game's own encoding.
pub(super) fn host_stack(stack: &ItemStack) -> HostStack {
    let mut s = stack.clone();
    let mut data = BTreeMap::new();
    if let Some(d) = s.extra.as_mut()
        && let Some(i) = d.other.iter().position(|(k, _)| k == CUSTOM_DATA)
        && let Tag::Compound(c) = &mut d.other[i].1
    {
        if let Some(Tag::Compound(mods)) = c.remove(&JavaString::from(MODS_KEY)) {
            for (id, bytes) in mods {
                if let Tag::ByteArray(b) = bytes {
                    data.insert(id.to_string_lossy(), b);
                }
            }
        }
        if c.is_empty() {
            d.other.remove(i);
        }
    }
    s.tidy();
    let count = s.count;
    let components = if s.extra.is_none() && s.damage == 0 {
        Vec::new()
    } else {
        s.count = 1;
        nbt::write_network(&horizon_items::item_nbt::item_to_nbt(&s), Limits::default())
            .unwrap_or_default()
    };
    HostStack {
        item: s.item.to_string(),
        count,
        components,
        data,
    }
}

/// A mod's stack as the game's: none for no count or components it can't read.
pub(super) fn game_stack(h: &HostStack) -> Option<ItemStack> {
    if h.count == 0 {
        return None;
    }
    let mut s = if h.components.is_empty() {
        ItemStack {
            item: Arc::from(h.item.as_str()),
            count: 1,
            damage: 0,
            extra: None,
        }
    } else {
        let (tag, _) = nbt::read_network(&h.components, Limits::default()).ok()?;
        horizon_items::item_nbt::item_from_nbt(&tag)?
    };
    s.item = Arc::from(h.item.as_str());
    s.count = h.count;
    if !h.data.is_empty() {
        let mods: nbt::Compound = h
            .data
            .iter()
            .map(|(id, b)| (JavaString::from(id.as_str()), Tag::ByteArray(b.clone())))
            .collect();
        let d = s.data_mut();
        match d.other.iter_mut().find(|(k, _)| k == CUSTOM_DATA) {
            Some((_, Tag::Compound(c))) => {
                c.insert(JavaString::from(MODS_KEY), Tag::Compound(mods));
            }
            _ => d.other.push((
                CUSTOM_DATA.into(),
                Tag::compound([(MODS_KEY, Tag::Compound(mods))]),
            )),
        }
    }
    Some(s)
}

/// 36 inventory slots, armor feet to head (the game keeps head first), the off hand.
fn inventory_slots(
    inventory: &[Option<ItemStack>],
    armor: &[Option<ItemStack>; 4],
    offhand: &Option<ItemStack>,
) -> Vec<Option<HostStack>> {
    let mut out: Vec<Option<HostStack>> = inventory
        .iter()
        .take(36)
        .map(|s| s.as_ref().map(host_stack))
        .collect();
    out.resize(36, None);
    out.extend(armor.iter().rev().map(|s| s.as_ref().map(host_stack)));
    out.push(offhand.as_ref().map(host_stack));
    out
}

/// What a server mod's call may read beyond blocks, moved out of the game
/// for the call and back after it.
#[derive(Default)]
pub(super) struct Lent {
    pub(super) item_tags: Arc<HashMap<String, std::collections::HashSet<String>>>,
    pub(super) block_tags: super::mods::TagMap,
    pub(super) entity_tags: super::mods::TagMap,
    pub(super) redstone: HashMap<(String, i32, i32, i32), u8>,
    pub(super) max_stacks: Arc<HashMap<String, u32>>,
    /// Each level's block entities (vanilla containers).
    pub(super) containers: Vec<(&'static str, horizon_client::block_entities::BlockEntities)>,
    pub(super) mobs: Vec<(&'static str, Vec<EntityInfo>)>,
    /// Item types, block facts and fuel, for `items`.
    pub(super) res: Option<Arc<super::Resources>>,
    /// The resources' folder: recipe files for `recipes-server`.
    pub(super) root: std::path::PathBuf,
    /// `recipes-server`'s list, made once.
    pub(super) recipes: Option<Arc<Vec<horizon_mods::RecipeFacts>>>,
}
/// `players.style-stack`.
pub(super) fn style_stack(
    stack: &HostStack,
    look: &horizon_mods::StackLook,
) -> Result<HostStack, String> {
    let mut s = game_stack(stack).ok_or("an empty stack has no look")?;
    let d = s.data_mut();
    if let Some(name) = &look.name {
        d.custom_name = Some(name.clone());
    }
    if !look.lore.is_empty() {
        let lines = look
            .lore
            .iter()
            .map(|l| Tag::String(l.as_str().into()))
            .collect();
        d.set_other("minecraft:lore", Some(Tag::List(lines)));
    }
    if let Some(model) = &look.model {
        d.set_other(
            "minecraft:item_model",
            Some(Tag::String(model.as_str().into())),
        );
    }
    if let Some(glint) = look.glint {
        d.set_other(
            "minecraft:enchantment_glint_override",
            Some(Tag::Byte(i8::from(glint))),
        );
    }
    let mut out = host_stack(&s);
    out.data = stack.data.clone();
    Ok(out)
}

impl Lent {
    pub(super) fn redstone(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<u8> {
        self.redstone.get(&(dimension.to_owned(), x, y, z)).copied()
    }
    pub(super) fn max_stack(&self, item: &str) -> Option<u32> {
        self.max_stacks.get(item).copied()
    }
    fn entity(&self, dimension: &str, pos: (i32, i32, i32)) -> Option<&BlockEntity> {
        let (_, map) = self.containers.iter().find(|(d, _)| *d == dimension)?;
        map.get(&pos)
    }
    pub(super) fn container(
        &self,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<Vec<Option<HostStack>>> {
        let slots = self.entity(dimension, (x, y, z))?.slots();
        (!slots.is_empty()).then(|| slots.iter().map(|s| s.as_ref().map(host_stack)).collect())
    }
    pub(super) fn set_container(
        &mut self,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
        slots: Vec<Option<HostStack>>,
    ) -> bool {
        let Some((_, map)) = self.containers.iter_mut().find(|(d, _)| *d == dimension) else {
            return false;
        };
        let Some(e) = map.get_mut(&(x, y, z)) else {
            return false;
        };
        let to = e.slots_mut();
        if to.is_empty() || to.len() != slots.len() {
            return false;
        }
        for (at, s) in to.iter_mut().zip(slots) {
            *at = s.as_ref().and_then(game_stack);
        }
        true
    }
    pub(super) fn mobs(&self, dimension: &str) -> Vec<EntityInfo> {
        self.mobs
            .iter()
            .find(|(d, _)| *d == dimension)
            .map(|(_, m)| m.clone())
            .unwrap_or_default()
    }
}

/// `items`: what the game knows of item types and stacks, from the world's
/// resources the call holds.
impl Lent {
    fn res(&self) -> Option<&super::Resources> {
        self.res.as_deref()
    }
    pub(super) fn item_facts(&self, item: &str) -> Option<horizon_mods::ItemFacts> {
        let res = self.res()?;
        let items = res.items();
        let info = items.get(item)?;
        let stack = ItemStack::new(item, 1);
        let name = horizon_client::item_tooltip::name_line(&stack, &|key| {
            let text = horizon_client::lang::get(key);
            (text != key).then_some(text)
        });
        Some(horizon_mods::ItemFacts {
            name,
            max_stack: info.max_stack,
            max_damage: info.max_damage,
            food: info.food.map(|(n, s)| (n, s, info.can_always_eat)),
            block: info.block.map(|b| res.facts().get(b).name.to_string()),
            fuel_ticks: res.providers().burn_time(items, &stack, false).max(0) as u32,
            rarity: items
                .enchantments
                .props(item)
                .map_or_else(|| "common".to_owned(), |p| p.rarity.clone()),
        })
    }
    pub(super) fn item_ids(&self) -> Vec<String> {
        let Some(res) = self.res() else {
            return Vec::new();
        };
        let items = res.items();
        items.registry_order.iter().map(|i| i.to_string()).collect()
    }
    pub(super) fn items_in_tag(&self, tag: &str) -> Option<Vec<String>> {
        let items = self.res()?.items();
        let members = items.item_tags.get(tag)?;
        Some(
            items
                .registry_order
                .iter()
                .filter(|i| members.contains(&***i))
                .map(|i| i.to_string())
                .collect(),
        )
    }
    pub(super) fn stack_facts(&self, stack: &HostStack) -> Option<horizon_mods::StackFacts> {
        use horizon_items::item_data::{canon_tag, encode_components};
        let s = game_stack(stack)?;
        let items = self.res()?.items();
        let tag = encode_components(&s);
        let each = match &tag {
            Some(Tag::Compound(c)) => c
                .iter()
                .map(|(k, v)| (k.to_string_lossy(), canon_tag(v)))
                .collect(),
            _ => Vec::new(),
        };
        let data = s.data();
        let text = |t: &Tag| match t {
            Tag::String(s) => Some(s.to_string_lossy()),
            Tag::Compound(c) => match c.get(&JavaString::from("text")) {
                Some(Tag::String(s)) => Some(s.to_string_lossy()),
                _ => None,
            },
            _ => None,
        };
        let lore = match data.and_then(|d| d.other("minecraft:lore")) {
            Some(Tag::List(lines)) => lines.iter().filter_map(text).collect(),
            _ => Vec::new(),
        };
        let mut enchantments: Vec<(String, u32)> = s
            .enchantments()
            .iter()
            .chain(s.stored_enchantments())
            .map(|(e, l)| (e.to_string(), *l))
            .collect();
        enchantments.dedup();
        let glint = items.enchantments.props(&s.item).is_some_and(|p| p.glint);
        let potion = s.potion().map(|p| {
            let effects = horizon_client_core::potions::potion(p).map_or_else(Vec::new, |def| {
                def.effects
                    .iter()
                    .map(|(e, d, a)| (format!("minecraft:{e}"), *d, (*a).clamp(0, 255) as u8))
                    .collect()
            });
            (p.to_owned(), effects)
        });
        let contents = data.map_or_else(Vec::new, |d| {
            d.container
                .iter()
                .map(|(_, s)| host_stack(s))
                .chain(d.bundle_contents.iter().map(host_stack))
                .collect()
        });
        Some(horizon_mods::StackFacts {
            components: tag.as_ref().map_or_else(|| "{}".to_owned(), canon_tag),
            each,
            damage: s.damage,
            custom_name: s.custom_name().map(str::to_owned),
            lore,
            enchantments,
            glint: s.has_foil(glint),
            modifiers: stack_modifiers(&s, items),
            dyed: data
                .and_then(|d| d.dyed_color)
                .map(|c| c as u32 & 0xFF_FFFF),
            potion,
            contents,
        })
    }
    pub(super) fn edit_stack(
        &self,
        stack: &HostStack,
        edit: &horizon_mods::StackEdit,
    ) -> Result<HostStack, String> {
        use horizon_items::item_data::{command_components, decode_components, encode_components};
        use horizon_mods::StackEdit as E;
        let mut s = game_stack(stack).ok_or("a stack the game can't read")?;
        let items = self.res().ok_or("the game lent no items")?.items();
        let info = items.get(&s.item).ok_or("an item the game doesn't have")?;
        match edit {
            E::SetComponent(name, _) | E::RemoveComponent(name) => {
                let mut all = match encode_components(&s) {
                    Some(Tag::Compound(c)) => c,
                    _ => nbt::Compound::new(),
                };
                match edit {
                    E::SetComponent(_, value) => {
                        let one = command_components(&format!("[{name}={value}]"))?;
                        let Tag::Compound(one) = one else {
                            return Err(format!("{name}: not a component"));
                        };
                        for (k, v) in one {
                            all.insert(k, v);
                        }
                    }
                    _ => {
                        all.remove(&JavaString::from(name.as_str()));
                    }
                }
                let (damage, extra) = decode_components(Some(&Tag::Compound(all)));
                s.damage = damage;
                s.extra = extra;
            }
            E::Damage(d) => {
                if info.max_damage == 0 {
                    return Err(format!("{} doesn't wear", s.item));
                }
                s.damage = (*d).min(info.max_damage);
            }
            E::Repair(n) => {
                if info.max_damage == 0 {
                    return Err(format!("{} doesn't wear", s.item));
                }
                s.damage = s.damage.saturating_sub(*n);
            }
            E::Enchant(id, level) => {
                if items.enchantments.get(id).is_none() {
                    return Err(format!("no enchantment {id}"));
                }
                let book = &*s.item == "minecraft:enchanted_book";
                let d = s.data_mut();
                let list = if book {
                    &mut d.stored_enchantments
                } else {
                    &mut d.enchantments
                };
                horizon_items::item_data::set_level(list, id, *level);
            }
            E::Disenchant(id) => {
                let d = s.data_mut();
                d.enchantments.retain(|(e, _)| &**e != id.as_str());
                d.stored_enchantments.retain(|(e, _)| &**e != id.as_str());
            }
            E::AddModifier(m) => {
                let mut list: Vec<Tag> = stack_modifiers(&s, items)
                    .into_iter()
                    .filter(|o| o.id != m.id)
                    .chain(std::iter::once(m.clone()))
                    .map(|m| {
                        Tag::compound([
                            ("type", Tag::String(m.attribute.as_str().into())),
                            ("id", Tag::String(m.id.as_str().into())),
                            ("amount", Tag::Double(m.amount.to_bits())),
                            ("operation", Tag::String(m.operation.as_str().into())),
                            ("slot", Tag::String(m.slot.as_str().into())),
                        ])
                    })
                    .collect();
                list.shrink_to_fit();
                s.data_mut()
                    .set_other("minecraft:attribute_modifiers", Some(Tag::List(list)));
            }
            E::Dye(color) => s.data_mut().dyed_color = color.map(|c| c as i32),
            E::Potion(potion) => {
                if let Some(p) = potion
                    && horizon_client_core::potions::potion(p).is_none()
                {
                    return Err(format!("no potion {p}"));
                }
                s.data_mut().potion = potion.as_deref().map(std::sync::Arc::from);
            }
        }
        s.tidy();
        Ok(host_stack(&s))
    }
    pub(super) fn tool(&self, tool: &HostStack, block: &str) -> Option<(f32, bool)> {
        let s = game_stack(tool)?;
        let items = self.res()?.items();
        items.get(&s.item)?;
        Some((
            items.mining_speed(Some(&s), block),
            items.correct_for_drops(Some(&s), block),
        ))
    }
}

/// A recipe's id as mods name it: vanilla's files go by bare name.
fn recipe_id(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("minecraft:{name}")
    }
}

/// `recipes-server`'s list: the crafting, cooking and stonecutting recipes
/// the world loaded, made once.
pub(super) fn server_recipe_facts(res: &super::Resources) -> Vec<horizon_mods::RecipeFacts> {
    use horizon_client::crafting::Recipe;
    fn items(
        i: &horizon_client::crafting::Ingredient,
    ) -> std::iter::Cloned<std::slice::Iter<'_, String>> {
        i.1.iter().cloned()
    }
    let mut out = Vec::new();
    for (name, recipe) in &res.recipes().crafting {
        let (kind, results, ingredients): (&str, Vec<String>, Vec<String>) = match recipe {
            Recipe::Shaped { cells, result, .. } => (
                "minecraft:crafting_shaped",
                result.item.iter().map(|i| i.to_string()).collect(),
                cells.iter().flatten().flat_map(items).collect(),
            ),
            Recipe::Shapeless {
                ingredients,
                result,
            } => (
                "minecraft:crafting_shapeless",
                result.item.iter().map(|i| i.to_string()).collect(),
                ingredients.iter().flat_map(items).collect(),
            ),
            Recipe::Special(_) => ("minecraft:crafting_special", Vec::new(), Vec::new()),
        };
        let mut ingredients = ingredients;
        ingredients.sort();
        ingredients.dedup();
        out.push(horizon_mods::RecipeFacts {
            id: recipe_id(name),
            kind: kind.to_owned(),
            results,
            ingredients,
        });
    }
    for c in &res.recipes().cooking {
        out.push(horizon_mods::RecipeFacts {
            id: recipe_id(&c.id),
            kind: c.kind.clone(),
            results: vec![c.result.item.to_string()],
            ingredients: c.ingredient.1.clone(),
        });
    }
    for s in &res.stations().recipes.stonecutting {
        out.push(horizon_mods::RecipeFacts {
            id: recipe_id(&s.id),
            kind: "minecraft:stonecutting".to_owned(),
            results: vec![s.result.item.to_string()],
            ingredients: s.input.1.clone(),
        });
    }
    out
}

/// `loot` and `recipes-server`: rolls and recipe lookups on the world's
/// resources.
impl Lent {
    pub(super) fn loot(
        &self,
        registry: &horizon_world::registry::BlockRegistry,
        roll: &horizon_mods::LootRoll,
        seed: u64,
    ) -> Result<Vec<HostStack>, String> {
        use horizon_mods::LootRoll as L;
        let res = self.res().ok_or("the game lent no loot")?;
        let items = res.items();
        let mut random = horizon_core::random::LegacyRandom::new(seed as i64);
        let drops = match roll {
            L::Table(table) => {
                if !items.has_loot_table(table) {
                    return Err(format!("no loot table {table}"));
                }
                items.loot_table(table, false, None, &mut random)
            }
            L::Block { state, tool } => {
                let state = crate::game::render_control::parse_state(registry, state)
                    .map_err(|e| e.to_string())?;
                let tool = tool.as_ref().and_then(game_stack);
                items.block_drops(registry, state, tool.as_ref(), &mut random)
            }
            L::Entity { kind, by_player } => {
                let (ns, path) = kind.split_once(':').unwrap_or(("minecraft", kind));
                let table = format!("{ns}:entities/{path}");
                if !items.has_loot_table(&table) {
                    return Err(format!("{kind} has no loot table"));
                }
                items.loot_table(&table, *by_player, None, &mut random)
            }
        };
        Ok(drops
            .iter()
            .take(horizon_mods::LOOT_STACKS)
            .map(host_stack)
            .collect())
    }
    pub(super) fn loot_table_exists(&self, table: &str) -> bool {
        self.res().is_some_and(|r| r.items().has_loot_table(table))
    }
    pub(super) fn recipes(&self) -> Option<Arc<Vec<horizon_mods::RecipeFacts>>> {
        self.recipes.clone()
    }
    pub(super) fn recipe_json(&self, id: &str) -> Option<String> {
        let (ns, path) = id.split_once(':')?;
        let file = self.root.join(format!("data/{ns}/recipe/{path}.json"));
        // A recipe file is small: a mod is handed no more than this.
        let meta = std::fs::metadata(&file).ok()?;
        if meta.len() > 64 * 1024 {
            return None;
        }
        std::fs::read_to_string(file).ok()
    }
    pub(super) fn match_crafting(&self, grid: &[Option<HostStack>]) -> Option<(String, HostStack)> {
        let size = match grid.len() {
            4 => 2,
            9 => 3,
            _ => return None,
        };
        let cells: Vec<Option<ItemStack>> = grid
            .iter()
            .map(|s| s.as_ref().and_then(game_stack))
            .collect();
        let (id, made) = self.res()?.recipes().find(&cells, size)?;
        Some((recipe_id(id), host_stack(&made)))
    }
    pub(super) fn match_cooking(
        &self,
        kind: &str,
        stack: &HostStack,
    ) -> Option<(String, HostStack, u32, f32)> {
        let stack = game_stack(stack)?;
        let c = self.res()?.recipes().cook(kind, &stack)?;
        Some((
            recipe_id(&c.id),
            host_stack(&c.result),
            c.time,
            c.experience,
        ))
    }
    pub(super) fn match_stonecutting(&self, stack: &HostStack) -> Vec<(String, HostStack)> {
        let (Some(stack), Some(res)) = (game_stack(stack), self.res()) else {
            return Vec::new();
        };
        res.stations()
            .recipes
            .stonecutter(&stack)
            .into_iter()
            .map(|r| (recipe_id(&r.id), host_stack(&r.result)))
            .collect()
    }
    pub(super) fn recipes_of_type(&self, kind: &str) -> Vec<(String, String)> {
        // Bounds on what one call reads off the disk.
        const FILES: usize = 4096;
        const BYTES: u64 = 4 << 20;
        let mut out = Vec::new();
        let mut read = 0;
        for ns in horizon_items::content_namespaces::namespaces(&self.root) {
            let Ok(dir) = std::fs::read_dir(self.root.join(format!("data/{ns}/recipe"))) else {
                continue;
            };
            let mut files: Vec<_> = dir
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .take(FILES)
                .collect();
            files.sort();
            for file in files {
                let Ok(text) = std::fs::read_to_string(&file) else {
                    continue;
                };
                read += text.len() as u64;
                if read > BYTES {
                    return out;
                }
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                if v["type"].as_str() == Some(kind)
                    && let Some(stem) = file.file_stem().and_then(|s| s.to_str())
                {
                    out.push((format!("{ns}:{stem}"), text));
                }
            }
        }
        out
    }
}

/// A stack's attribute modifiers: its own component, else its item's.
fn stack_modifiers(
    s: &ItemStack,
    items: &horizon_client::items::Items,
) -> Vec<horizon_mods::ModifierFacts> {
    let text = |c: &nbt::Compound, k: &str| match c.get(&JavaString::from(k)) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    };
    if let Some(Tag::List(list)) = s
        .data()
        .and_then(|d| d.other("minecraft:attribute_modifiers"))
    {
        return list
            .iter()
            .filter_map(|t| match t {
                Tag::Compound(c) => Some(horizon_mods::ModifierFacts {
                    attribute: text(c, "type"),
                    id: text(c, "id"),
                    amount: match c.get(&JavaString::from("amount")) {
                        Some(Tag::Double(v)) => f64::from_bits(*v),
                        Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
                        _ => 0.0,
                    },
                    operation: text(c, "operation"),
                    slot: match text(c, "slot") {
                        s if s.is_empty() => "any".to_owned(),
                        s => s,
                    },
                }),
                _ => None,
            })
            .collect();
    }
    items.get(&s.item).map_or_else(Vec::new, |info| {
        info.modifiers
            .iter()
            .map(
                |(attribute, id, amount, operation, slot)| horizon_mods::ModifierFacts {
                    attribute: attribute.clone(),
                    id: id.clone(),
                    amount: *amount,
                    operation: operation.clone(),
                    slot: slot.clone(),
                },
            )
            .collect()
    })
}

impl Game<'_> {
    /// The level `dim`'s world: the one in place or a parked one.
    fn level_world(&mut self, dim: &str) -> Option<&mut horizon_client::world::World> {
        if self.dim.id() == dim {
            Some(&mut self.world)
        } else {
            self.levels.parked_world(Dim::from_id(dim))
        }
    }

    /// What happened to players since last tick: found by comparing them,
    /// with the recipes [`Game::mod_crafted`] heard of.
    pub(super) fn mod_player_events(
        &mut self,
        players: &[horizon_mods::ServerPlayer],
    ) -> Vec<(String, String, String)> {
        let mut out = std::mem::take(&mut self.mods.crafted);
        for p in players {
            let Some(was) = self.mods.players.iter().find(|q| q.id == p.id) else {
                continue;
            };
            let mut event = |kind: &str, detail: &str| {
                out.push((p.id.clone(), kind.to_owned(), detail.to_owned()));
            };
            if was.health > 0.0 && p.health <= 0.0 {
                event("died", "");
            } else if was.health <= 0.0 && p.health > 0.0 {
                event("respawned", "");
            }
            if was.dimension != p.dimension {
                event("changed-dimension", &p.dimension);
            }
        }
        self.mod_new_targets(&mut out);
        out
    }

    /// A guest took the results of these recipes.
    pub(super) fn mod_crafted(&mut self, uuid: u128, recipes: &[String]) {
        if self.mods.server {
            let id = uuid_string(uuid);
            self.mods.crafted.extend(
                recipes
                    .iter()
                    .map(|r| (id.clone(), "crafted".to_owned(), r.clone())),
            );
        }
    }

    pub(super) fn mod_acting_id(&self) -> String {
        let [a, b, c, d] = self.acting_uuid().map(|w| w as u32);
        uuid_string(
            (u128::from(a) << 96) | (u128::from(b) << 64) | (u128::from(c) << 32) | u128::from(d),
        )
    }

    /// The acting player threw, finished using or broke `item`.
    pub(super) fn mod_item_event(&mut self, kind: &str, item: &str) {
        if self.mods.server {
            let id = self.mod_acting_id();
            self.mods
                .crafted
                .push((id, kind.to_owned(), item.to_owned()));
        }
    }

    /// `players.set-undying` on the acting player.
    pub(super) fn mod_undying(&self) -> bool {
        !self.mods.undying.is_empty() && self.mods.undying.contains(&self.mod_acting_id())
    }

    /// Mobs that just took a player as their target.
    fn mod_new_targets(&mut self, out: &mut Vec<(String, String, String)>) {
        let local = self.player_uuid_string();
        let targeting: Vec<u32> = self
            .mobs
            .iter()
            .filter(|m| m.ai.target == Some(horizon_client::mobs::Who::Player))
            .map(|m| m.id)
            .collect();
        for id in &targeting {
            if self.mods.mob_targets.contains(id) {
                continue;
            }
            let Some(mob) = self
                .mobs
                .iter()
                .find(|m| m.id == *id)
                .and_then(|m| self.mob_info(m))
            else {
                continue;
            };
            let bound = self.lan.as_ref().and_then(|h| {
                let conn = h.mob_players.get(id)?;
                h.hub.guest(*conn).map(|g| uuid_string(g.uuid))
            });
            out.push((
                bound.unwrap_or_else(|| local.clone()),
                "targeted".to_owned(),
                mob.id,
            ));
        }
        self.mods.mob_targets = targeting.into_iter().collect();
    }

    /// Where the players look, for `players.look`.
    pub(super) fn mod_looks(&self, host: Option<&super::lan::LanHost>) -> Vec<(String, f32, f32)> {
        let mut out = Vec::new();
        if let Some(p) = &self.player
            && self.remote.is_none()
        {
            out.push((self.player_uuid_string(), p.yaw, p.pitch));
        }
        let guests = host
            .or(self.lan.as_ref())
            .into_iter()
            .flat_map(|h| &h.hub.guests);
        out.extend(
            guests
                .filter(|g| g.placed)
                .map(|g| (uuid_string(g.uuid), g.yaw, g.pitch)),
        );
        out
    }

    /// The ids of the players holding sneak, for `players.sneaking`.
    pub(super) fn mod_sneaking(&self, host: Option<&super::lan::LanHost>) -> Vec<String> {
        let mut out = Vec::new();
        if self.options.execution_role.is_client()
            && self.remote.is_none()
            && self.player.is_some()
            && self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        {
            out.push(self.player_uuid_string());
        }
        let guests = host
            .or(self.lan.as_ref())
            .into_iter()
            .flat_map(|h| &h.hub.guests);
        out.extend(
            guests
                .filter(|g| g.placed && g.sneaking)
                .map(|g| uuid_string(g.uuid)),
        );
        out
    }

    /// Each player's held slot and inventory, for `players.inventory`.
    pub(super) fn mod_inventories(
        &self,
        host: Option<&super::lan::LanHost>,
    ) -> Vec<(String, u32, Vec<Option<HostStack>>)> {
        let mut out = Vec::new();
        if self.options.execution_role.is_client() && self.remote.is_none() && self.player.is_some()
        {
            let slots = inventory_slots(&self.inventory, &self.armor, &self.offhand);
            out.push((self.player_uuid_string(), self.selected as u32, slots));
        }
        for g in host
            .or(self.lan.as_ref())
            .into_iter()
            .flat_map(|h| &h.hub.guests)
            .filter(|g| g.placed)
        {
            let slots = inventory_slots(&g.inventory, &g.armor, &g.offhand);
            out.push((uuid_string(g.uuid), g.selected as u32, slots));
        }
        out
    }

    /// Before the world is lent: the redstone power at the mods' blocks of
    /// the level in place, the items' stack sizes, the levels' block
    /// entities and mobs. [`Game::return_lent`] puts them back.
    pub(super) fn lend_more(&mut self) -> Lent {
        let dim = self.dim.id();
        let mut redstone = HashMap::new();
        for (x, y, z) in self.mods_now(|m| m.machine_positions(dim)) {
            let pos = horizon_core::position::BlockPos::new(x, y, z);
            let power =
                self.with_level(|b, level, ticks| b.best_neighbor_signal(level, ticks, pos));
            redstone.insert((dim.to_owned(), x, y, z), power.clamp(0, 15) as u8);
        }
        let max_stacks = self
            .mods
            .max_stacks
            .get_or_insert_with(|| {
                Arc::new(
                    self.res
                        .items()
                        .items
                        .iter()
                        .map(|(id, info)| (id.to_string(), info.max_stack))
                        .collect(),
                )
            })
            .clone();
        let recipes = (self.res.rest.get().is_some()).then(|| {
            self.mods
                .server_recipes
                .get_or_insert_with(|| Arc::new(server_recipe_facts(&self.res)))
                .clone()
        });
        let mut containers = vec![(dim, std::mem::take(&mut self.world.block_entities))];
        for (d, world) in self.levels.parked_worlds() {
            containers.push((d.id(), std::mem::take(&mut world.block_entities)));
        }
        let uuids = &self.persist.uuids;
        let mobs: Vec<EntityInfo> = self
            .mobs
            .iter()
            .filter(|m| m.death_time == 0)
            .filter_map(|m| {
                let uuid = uuids.get(&m.id)?;
                Some(EntityInfo {
                    id: uuid_string(uuid_words(*uuid)),
                    kind: m.kind.id.clone(),
                    pos: (m.position.x, m.position.y, m.position.z),
                    health: m.health,
                })
            })
            .collect();
        let item_tags = self
            .mods
            .item_tags
            .get_or_insert_with(|| Arc::new(self.res.items().item_tags.clone()))
            .clone();
        redstone.extend(
            self.mods
                .parked_redstone
                .iter()
                .map(|(k, v)| (k.clone(), *v)),
        );
        let root = horizon_client::mobs::loaded_root();
        let tags = |kind: &str| {
            Arc::new(root.as_deref().map_or_else(Default::default, |root| {
                horizon_client::items::load_tags(root, kind).unwrap_or_default()
            }))
        };
        let block_tags = self
            .mods
            .block_tags
            .get_or_insert_with(|| tags("block"))
            .clone();
        let entity_tags = self
            .mods
            .entity_tags
            .get_or_insert_with(|| tags("entity_type"))
            .clone();
        Lent {
            res: Some(self.res.clone()),
            root: self.options.resources.clone(),
            recipes,
            item_tags,
            block_tags,
            entity_tags,
            redstone,
            max_stacks,
            containers,
            mobs: std::iter::once((dim, mobs))
                .chain(
                    self.mods
                        .parked_mobs
                        .iter()
                        .filter(|(d, _)| *d != dim)
                        .cloned(),
                )
                .collect(),
        }
    }
    /// The block entities [`Game::lend_more`] took, back in their levels.
    pub(super) fn return_lent(&mut self, lent: Lent) {
        for (d, map) in lent.containers {
            if let Some(world) = self.level_world(d) {
                world.block_entities = map;
            }
        }
    }

    /// The changes server mods asked for, in order (`players.give`, ...).
    pub(super) fn apply_mod_actions(&mut self) {
        for action in std::mem::take(&mut self.mods.actions) {
            match action {
                ServerAction::Give { player, stack } => {
                    if let Some(s) = game_stack(&stack) {
                        self.on_mod_player(&player, |g| g.give_or_drop(s));
                    }
                }
                ServerAction::SetSlot {
                    player,
                    slot,
                    stack,
                } => {
                    let s = stack.as_ref().and_then(game_stack);
                    self.on_mod_player(&player, |g| match slot {
                        0..=35 => {
                            if let Some(at) = g.inventory.get_mut(slot as usize) {
                                *at = s;
                            }
                        }
                        36..=39 => g.armor[(39 - slot) as usize] = s,
                        40 => g.offhand = s,
                        _ => {}
                    });
                }
                ServerAction::Take {
                    player,
                    item,
                    count,
                } => {
                    self.on_mod_player(&player, |g| {
                        let mut left = count;
                        let slots = g
                            .inventory
                            .iter_mut()
                            .chain(g.armor.iter_mut())
                            .chain(std::iter::once(&mut g.offhand));
                        for at in slots {
                            if left == 0 {
                                break;
                            }
                            if let Some(s) = at
                                && *s.item == *item
                            {
                                let n = s.count.min(left);
                                s.count -= n;
                                left -= n;
                                if s.count == 0 {
                                    *at = None;
                                }
                            }
                        }
                    });
                }
                ServerAction::Console { line } => {
                    if let Some(rest) = line.strip_prefix("effect give ") {
                        let mut parts = rest.split_whitespace();
                        let target = parts.next();
                        let effect = parts.next();
                        let seconds = parts
                            .next()
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(1);
                        let amplifier =
                            parts.next().and_then(|s| s.parse::<u8>().ok()).unwrap_or(0);
                        if let (Some(target), Some(effect)) = (target, effect) {
                            self.mods.mods.mob_effect_applied(
                                target.to_owned(),
                                effect.to_owned(),
                                seconds,
                                amplifier,
                            );
                        }
                    }
                    self.mod_console(&line);
                }
                ServerAction::OpenMenu {
                    player,
                    dimension,
                    pos,
                    title,
                    kind,
                } => {
                    self.mods.mods.container_opened(&player, &title);
                    self.open_mod_menu(&player, dimension, pos, &title, &kind);
                }
                ServerAction::Net {
                    player,
                    channel,
                    data,
                } => self.mod_net_send(player.as_deref(), &channel, data),
                ServerAction::DropItem { ref dimension, .. }
                | ServerAction::SpawnMob { ref dimension, .. }
                | ServerAction::Explode { ref dimension, .. }
                    if dimension != self.dim.id() =>
                {
                    self.mods.waiting.push((dimension.clone(), action));
                }
                ServerAction::DropItem { pos, stack, .. } => {
                    if let Some(s) = game_stack(&stack) {
                        self.pop_resource([pos.0, pos.1, pos.2], s);
                    }
                }
                ServerAction::SpawnMob { kind, pos, .. } => {
                    if self.spawn_mob(&kind, [pos.0, pos.1, pos.2], true).is_none() {
                        log::warn!("Horizon: a mod spawned an unknown mob {kind}");
                    }
                }
                ServerAction::Explode { pos, power, .. } => {
                    if let Err(e) = self.explode([pos.0, pos.1, pos.2], power) {
                        log::warn!("Horizon: a mod's explosion: {e}");
                    }
                }
                ServerAction::Undying { player, on } => {
                    if on {
                        self.mods.undying.insert(player);
                    } else {
                        self.mods.undying.remove(&player);
                    }
                }
                ServerAction::Mob { id, change } => {
                    if !self.mod_change_mob(&id, &change) {
                        // In a level not in place: each takes its turn.
                        for dim in self.levels.parked_dims() {
                            self.mods.waiting.push((
                                dim.id().to_owned(),
                                ServerAction::Mob {
                                    id: id.clone(),
                                    change: change.clone(),
                                },
                            ));
                        }
                    }
                }
                ServerAction::Ore { id, ore } => {
                    self.mods.mods.worldgen_mut().add_ore(&id, ore);
                    #[cfg(feature = "mods")]
                    horizon_client::mod_worldgen::publish(&self.mods.mods);
                }
                ServerAction::Worldgen { id, biome, surface } => {
                    let rule = horizon_mods::ResourceLocation::parse(&format!("{id}:worldgen"))
                        .expect("a mod id is a registry namespace");
                    self.mods
                        .mods
                        .worldgen_mut()
                        .set_surface_rule(rule, biome, surface);
                    #[cfg(feature = "mods")]
                    horizon_client::mod_worldgen::publish(&self.mods.mods);
                }
            }
        }
    }

    /// A mob of the level in place, by its UUID: false when it isn't here.
    fn mod_change_mob(&mut self, id: &str, change: &horizon_mods::MobChange) -> bool {
        use horizon_mods::MobChange;
        let uuids = &self.persist.uuids;
        // A target that names no mob here is a player.
        let other: HashMap<String, u32> = match change {
            MobChange::Target(_) => uuids
                .iter()
                .map(|(mob, u)| (uuid_string(uuid_words(*u)), *mob))
                .collect(),
            _ => HashMap::new(),
        };
        let Some(m) = self.mobs.iter_mut().find(|m| {
            uuids
                .get(&m.id)
                .is_some_and(|u| uuid_string(uuid_words(*u)) == id)
        }) else {
            return false;
        };
        match change {
            MobChange::Ai(on) => m.no_ai = !on,
            MobChange::Name(name) => m.custom_name = name.clone(),
            MobChange::MoveTo(x, y, z, speed) => m.mod_move = Some(([*x, *y, *z], *speed)),
            MobChange::Target(target) => {
                let who = target.as_ref().map(|t| {
                    other
                        .get(t)
                        .map_or(horizon_client::mobs::Who::Player, |id| {
                            horizon_client::mobs::Who::Mob(*id)
                        })
                });
                m.ai.target = who;
            }
            MobChange::Heal(amount) => m.health = (m.health + amount).min(m.kind.max_health),
            MobChange::Health(health) => m.health = health.min(m.kind.max_health),
            MobChange::Velocity(x, y, z) => {
                m.velocity = horizon_core::geometry::Vec3::new(*x, *y, *z);
                m.needs_sync = true;
            }
        }
        true
    }

    /// Each level not in place takes a turn when its mods' machines need it:
    /// what waits for it, its redstone both ways, its mobs for `entities.near`.
    pub(super) fn tick_parked_mod_levels(&mut self) {
        let here = self.dim;
        let mut waiting = std::mem::take(&mut self.mods.waiting);
        self.mods.parked_redstone.clear();
        self.mods.parked_mobs.clear();
        for dim in self.levels.parked_dims() {
            let id = dim.id();
            let machines = self.mods.mods.machine_positions(id);
            let mine: Vec<ServerAction> = {
                let (mine, rest) = waiting.into_iter().partition(|(d, _)| d == id);
                waiting = rest;
                mine.into_iter().map(|(_, a)| a).collect()
            };
            // A level costs a swap: only when it holds something a mod needs.
            let outputs = !self.mods.mods.redstone_outputs(id).is_empty();
            let wanted = !machines.is_empty()
                || !mine.is_empty()
                || outputs
                || self.hears_events("entity-spawned")
                || self.hears_events("entity-hurt")
                || self
                    .mods
                    .mods
                    .any_running_with(horizon_mods::Capability::Entities);
            if !wanted {
                continue;
            }
            let Some(back) = self.enter_command_level(dim) else {
                continue;
            };
            self.mod_name_mobs();
            let mobs: Vec<EntityInfo> = self
                .mobs
                .iter()
                .filter(|m| m.death_time == 0)
                .filter_map(|m| self.mob_info(m))
                .collect();
            self.mods.parked_mobs.push((id, mobs));
            self.lend_world();
            self.mod_mob_events();
            self.return_world();
            for action in mine {
                match action {
                    ServerAction::Mob { id, change } => {
                        self.mod_change_mob(&id, &change);
                    }
                    ServerAction::DropItem { pos, stack, .. } => {
                        if let Some(s) = game_stack(&stack) {
                            self.pop_resource([pos.0, pos.1, pos.2], s);
                        }
                    }
                    ServerAction::SpawnMob { kind, pos, .. } => {
                        self.spawn_mob(&kind, [pos.0, pos.1, pos.2], true);
                    }
                    ServerAction::Explode { pos, power, .. } => {
                        if let Err(e) = self.explode([pos.0, pos.1, pos.2], power) {
                            log::warn!("Horizon: a mod's explosion: {e}");
                        }
                    }
                    _ => {}
                }
            }
            self.sync_mod_redstone();
            self.spill_gone_storages();
            for (x, y, z) in machines {
                let pos = horizon_core::position::BlockPos::new(x, y, z);
                let power =
                    self.with_level(|b, level, ticks| b.best_neighbor_signal(level, ticks, pos));
                self.mods
                    .parked_redstone
                    .insert((id.to_owned(), x, y, z), power.clamp(0, 15) as u8);
            }
            self.enter_command_level(back);
        }
        debug_assert!(self.dim == here);
        self.mods.waiting = waiting;
    }

    /// A machine's block is gone, however it went (explosion, piston, another
    /// mod): what its storage held drops there, as a chest's does.
    pub(super) fn spill_gone_storages(&mut self) {
        let dim = self.dim.id();
        let (world, res) = (&self.world, &self.res);
        let gone = self.mods.mods.take_gone_storages(dim, |(x, y, z)| {
            world
                .chunks
                .contains_key(&(x >> 4, z >> 4))
                .then(|| res.facts().get(world.block(x, y, z)).name.to_string())
        });
        for (pos, stacks) in gone {
            self.mods.crafted.push((
                String::new(),
                "block-gone".to_owned(),
                format!("{dim} {} {} {}", pos.0, pos.1, pos.2),
            ));
            for stack in stacks {
                if let Some(s) = game_stack(&stack) {
                    self.pop_resource([pos.0, pos.1, pos.2], s);
                }
            }
        }
    }

    /// What server mods have closed, to the crafting grids when it changes.
    pub(in crate::game) fn sync_mod_locked(&mut self) {
        let locked = self.mods.mods.locked_results();
        if locked != self.mods.locked {
            horizon_client::crafting::set_locked_results(locked.iter().cloned().collect());
            self.mods.locked = locked;
        }
    }

    /// What server mods' blocks give, into the level in place; what changed
    /// tells its neighbours (`world.set-redstone-output`).
    pub(super) fn sync_mod_redstone(&mut self) {
        use horizon_core::position::BlockPos;
        use horizon_world::level::LevelAccess;
        let dim = self.dim.id();
        let outputs = self.mods.mods.redstone_outputs(dim);
        let mut gone = Vec::new();
        let registry = self.pack.assets.registry;
        self.with_level(|b, level, ticks| {
            let mut wanted = HashMap::new();
            for (pos, block, signal, comparator) in outputs {
                let at = BlockPos::new(pos.0, pos.1, pos.2);
                let here = registry.state(level.block_state(at));
                if here.is_some_and(|s| s.name != block) {
                    gone.push(pos);
                    continue;
                }
                wanted.insert(at, (i32::from(signal), i32::from(comparator)));
            }
            if wanted != ticks.mod_outputs {
                let changed: Vec<BlockPos> = wanted
                    .keys()
                    .chain(ticks.mod_outputs.keys())
                    .copied()
                    .filter(|pos| wanted.get(pos) != ticks.mod_outputs.get(pos))
                    .collect();
                ticks.mod_outputs = wanted;
                for pos in changed {
                    // Neighbours, and wires beside it turn towards it or away.
                    b.notify_written(
                        level,
                        ticks,
                        pos,
                        horizon_world::level::flags::UPDATE_NEIGHBORS,
                    );
                    b.update_neighbour_for_output_signal(level, ticks, pos);
                }
            }
        });
        for pos in gone {
            self.mods.mods.remove_redstone_output(dim, pos);
        }
    }

    /// `f` as the player with this id: the window's own in-frame, else a
    /// connection's (their inventory in place, as for their own actions).
    fn on_mod_player(&mut self, id: &str, f: impl FnOnce(&mut Self)) {
        if self.options.execution_role.is_client()
            && self.remote.is_none()
            && self.player.is_some()
            && self.player_uuid_string() == id
        {
            f(self);
            return;
        }
        let Some(mut host) = self.lan.take() else {
            return;
        };
        let conn = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && uuid_string(g.uuid) == id)
            .map(|g| g.conn);
        if let Some(conn) = conn {
            self.as_guest(&mut host, conn, f);
        }
        self.lan = Some(host);
    }

    /// `server-net`: a message to one player's client mods, or everyone's.
    fn mod_net_send(&mut self, player: Option<&str>, channel: &str, data: Vec<u8>) {
        let local = self.options.execution_role.is_client()
            && self.remote.is_none()
            && self.player.is_some();
        if local && player.is_none_or(|p| p == self.player_uuid_string()) {
            self.mod_net_received(channel, &data);
        }
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let packet = horizon_protocol::packets::play::Clientbound::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload {
                channel: format!("{}{channel}", horizon_mods::NET_PREFIX),
                data,
            },
        );
        let conns: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && player.is_none_or(|p| p == uuid_string(g.uuid)))
            .map(|g| g.conn)
            .collect();
        for conn in conns {
            host.hub.send_to(conn, &packet);
        }
    }

    /// A vanilla command as the server console, its feedback dropped.
    fn mod_console(&mut self, line: &str) {
        let capture = self.chat_capture.replace(Vec::new());
        // Commands act on the player in place: a guest's turn, as `@s`.
        let guest = self.lan.as_ref().and_then(|host| {
            host.hub.guests.iter().find_map(|g| {
                let named = line.split(' ').any(|w| w == g.name);
                named.then(|| {
                    let own: Vec<&str> = line
                        .split(' ')
                        .map(|w| if w == g.name { "@s" } else { w })
                        .collect();
                    (g.conn, own.join(" "))
                })
            })
        });
        match guest {
            Some((conn, own)) => {
                self.as_console(|g| {
                    g.for_player(super::commands::Target::Guest(conn), |g| {
                        g.run_command_quietly(&own)
                    })
                });
            }
            None => {
                self.as_console(|g| g.run_command(line));
            }
        }
        self.chat_capture = capture;
        self.admin_feedback.clear();
    }

    /// `storages.open-menu`: a chest-like window on a placeholder holding
    /// the storage's slots, copied back each tick ([`Game::sync_mod_menus`]).
    fn open_mod_menu(
        &mut self,
        player: &str,
        dimension: String,
        pos: (i32, i32, i32),
        title: &str,
        kind: &str,
    ) {
        let Some(items) = self.mods.mods.storage_slots(&dimension, pos) else {
            return;
        };
        let Some(mut host) = self.lan.take() else {
            // In-frame: no window for the window's own player yet.
            log::debug!("Horizon: a mod's window needs a server thread or a server");
            return;
        };
        let guest = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && uuid_string(g.uuid) == player)
            .map(|g| (g.conn, g.dimension.clone()));
        if let Some((conn, level)) = guest {
            self.mods.next_menu += 1;
            let key = (MENU_KEY_X, MOD_MENU_KEY_Y, -self.mods.next_menu);
            let stacks: Vec<Option<ItemStack>> = items
                .iter()
                .map(|s| s.as_ref().and_then(game_stack))
                .collect();
            if let Some(world) = self.level_world(&level) {
                world
                    .block_entities
                    .insert(key, BlockEntity::Chest(stacks.clone()));
                let title = horizon_client::lan::data::component_nbt(
                    &Text::literal(title.to_owned()).component(),
                );
                host.hub
                    .open_container(conn, kind, title, key, Slots::Block(key), stacks);
                self.mods.menus.push(ModMenu {
                    conn,
                    key,
                    level,
                    dimension,
                    pos,
                    last: items,
                });
            }
        }
        self.lan = Some(host);
    }

    /// Each tick: what players moved in storages' windows goes into the
    /// storage, what the mod changed into the window; closed ones go.
    pub(super) fn sync_mod_menus(&mut self) {
        for mut m in std::mem::take(&mut self.mods.menus) {
            // ChestBlockEntity.stillValid: the player in the storage's level, within 8 blocks of it.
            let (px, py, pz) = (
                f64::from(m.pos.0) + 0.5,
                f64::from(m.pos.1) + 0.5,
                f64::from(m.pos.2) + 0.5,
            );
            let shown_to = self.lan.as_ref().and_then(|h| {
                h.hub.guests.iter().find(|g| {
                    g.conn == m.conn && g.menu.as_ref().is_some_and(|menu| menu.pos == Some(m.key))
                })
            });
            let open = shown_to.is_some();
            let near = shown_to.is_some_and(|g| {
                let d = (g.position[0] - px).powi(2)
                    + (g.position[1] - py).powi(2)
                    + (g.position[2] - pz).powi(2);
                g.dimension == m.dimension && d <= 64.0
            });
            let bars = self.mods.mods.storage_menu_data(&m.dimension, m.pos);
            let slot_rules: std::collections::BTreeMap<usize, (bool, Vec<String>)> = self
                .mods
                .mods
                .storage_slot_rules(&m.dimension, m.pos)
                .into_iter()
                .map(|(slot, rule)| (slot as usize, (rule.take_only, rule.items)))
                .collect();
            if let Some(menu) = self.lan.as_mut().and_then(|h| {
                let g = h.hub.guests.iter_mut().find(|g| g.conn == m.conn)?;
                g.menu.as_mut().filter(|menu| menu.pos == Some(m.key))
            }) {
                menu.mod_data = bars;
                menu.mod_rules = slot_rules;
            }
            let shown: Option<Vec<Option<HostStack>>> =
                self.level_world(&m.level)
                    .and_then(|w| match w.block_entities.get(&m.key) {
                        Some(BlockEntity::Chest(items)) => {
                            Some(items.iter().map(|s| s.as_ref().map(host_stack)).collect())
                        }
                        _ => None,
                    });
            let stored = self.mods.mods.storage_slots(&m.dimension, m.pos);
            let alive = match (shown, stored) {
                (Some(shown), Some(stored)) if shown.len() == stored.len() => {
                    if shown != m.last {
                        let rules = self.mods.mods.storage_slot_rules(&m.dimension, m.pos);
                        let (shown, back) = apply_slot_rules(&m.last, shown, &rules);
                        if !back.is_empty() {
                            if let Some(w) = self.level_world(&m.level) {
                                let stacks = shown
                                    .iter()
                                    .map(|s| s.as_ref().and_then(game_stack))
                                    .collect();
                                w.block_entities.insert(m.key, BlockEntity::Chest(stacks));
                            }
                            let owner = self.lan.as_ref().and_then(|h| h.hub.guest(m.conn));
                            if let Some(id) = owner.map(|g| uuid_string(g.uuid)) {
                                self.on_mod_player(&id, |g| {
                                    for s in back.iter().filter_map(game_stack) {
                                        g.give_or_drop(s);
                                    }
                                });
                            }
                        }
                        self.mods
                            .mods
                            .set_storage_slots(&m.dimension, m.pos, shown.clone());
                        m.last = shown;
                    } else if stored != m.last {
                        let stacks: Vec<Option<ItemStack>> = stored
                            .iter()
                            .map(|s| s.as_ref().and_then(game_stack))
                            .collect();
                        if let Some(w) = self.level_world(&m.level) {
                            w.block_entities.insert(m.key, BlockEntity::Chest(stacks));
                        }
                        m.last = stored;
                    }
                    true
                }
                _ => false,
            };
            if open && alive && near {
                self.mods.menus.push(m);
                continue;
            }
            if open && let Some(host) = self.lan.as_mut() {
                host.hub.close_container(m.conn);
            }
            if let Some(w) = self.level_world(&m.level) {
                w.block_entities.remove(&m.key);
            }
        }
    }
}

/// A window's slots after its player's clicks, with what `rules` refuse taken
/// back out, and the stacks that go back to the player.
// ponytail: refused items land in the inventory, not back on the cursor.
fn apply_slot_rules(
    last: &[Option<HostStack>],
    mut shown: Vec<Option<HostStack>>,
    rules: &std::collections::BTreeMap<u32, horizon_mods::SlotRule>,
) -> (Vec<Option<HostStack>>, Vec<HostStack>) {
    let mut back = Vec::new();
    for (&slot, rule) in rules {
        let i = slot as usize;
        let (Some(was), Some(now)) = (last.get(i), shown.get_mut(i)) else {
            continue;
        };
        let Some(put) = now.clone() else { continue };
        let allowed = rule.items.is_empty() || rule.items.contains(&put.item);
        match was {
            // More of what was there.
            Some(w) if w.item == put.item && w.data == put.data => {
                if put.count > w.count && (rule.take_only || !allowed) {
                    back.push(HostStack {
                        count: put.count - w.count,
                        ..put
                    });
                    *now = Some(w.clone());
                }
            }
            // Something new; a swap leaves what was there with the player.
            _ => {
                if rule.take_only || !allowed {
                    back.push(put);
                    *now = None;
                }
            }
        }
    }
    (shown, back)
}

/// What the game saw for `game-events` during a tick, told at its end.
pub(super) enum PendingEvent {
    Died {
        dimension: String,
        entity: EntityInfo,
        killer: Option<String>,
    },
    Picked {
        player: String,
        stack: HostStack,
    },
}

impl Game<'_> {
    /// Every mob of the level in place gets its UUID now: they are made on
    /// demand, and a mob without one can't be named to a mod.
    pub(super) fn mod_name_mobs(&mut self) {
        let ids: Vec<u32> = self.mobs.iter().map(|m| m.id).collect();
        for id in ids {
            self.uuid_for(id);
        }
    }

    /// A clicked entity that is no mob: its kind and place.
    pub(super) fn mod_object_info(&self, id: i32) -> Option<EntityInfo> {
        let (lo, hi, _) = self.object_target(id)?;
        let at = u32::try_from(id).ok()?;
        let kind = if self.boats.iter().any(|b| b.id == at) {
            "minecraft:boat"
        } else if self.carts.iter().any(|c| c.id == at) {
            "minecraft:minecart"
        } else if self.armor_stands.iter().any(|a| a.id == at) {
            "minecraft:armor_stand"
        } else if self.item_frames.iter().any(|f| f.id == at) {
            "minecraft:item_frame"
        } else if self.paintings.iter().any(|p| p.id == at) {
            "minecraft:painting"
        } else {
            "minecraft:interaction"
        };
        Some(EntityInfo {
            id: id.to_string(),
            kind: kind.to_owned(),
            pos: ((lo[0] + hi[0]) / 2.0, lo[1], (lo[2] + hi[2]) / 2.0),
            health: 0.0,
        })
    }
    pub(super) fn mob_info(&self, m: &horizon_client::mobs::Mob) -> Option<EntityInfo> {
        let uuid = self.persist.uuids.get(&m.id)?;
        Some(EntityInfo {
            id: uuid_string(uuid_words(*uuid)),
            kind: m.kind.id.clone(),
            pos: (m.position.x, m.position.y, m.position.z),
            health: m.health,
        })
    }
    fn hears_events(&mut self, event: &str) -> bool {
        self.mods.server && self.remote.is_none() && self.mods_now(|m| m.wants_event(event))
    }

    /// `entity-events.entity-dying`, before anything of the death happens:
    /// true when a mod keeps the mob alive.
    pub(super) fn mod_spares_mob(
        &mut self,
        m: &horizon_client::mobs::Mob,
        killer: Option<Option<u32>>,
    ) -> bool {
        if !self.hears_events("entity-dying") {
            return false;
        }
        self.uuid_for(m.id);
        let Some(entity) = self.mob_info(m) else {
            return false;
        };
        let killer = self.mod_killer_id(killer);
        let dim = self.dim.id();
        self.lend_world();
        let spared = self.mods_now(|m| m.entity_dying(dim, &entity, killer.as_deref()));
        self.return_world();
        spared
    }

    fn mod_killer_id(&self, killer: Option<Option<u32>>) -> Option<String> {
        killer.and_then(|by| match by {
            None => Some(self.player_uuid_string()),
            Some(conn) => self
                .lan
                .iter()
                .flat_map(|h| &h.hub.guests)
                .find(|g| g.conn == conn)
                .map(|g| uuid_string(g.uuid)),
        })
    }

    /// `LivingEntity.die`: for `entity-died` at the tick's end. `killer`:
    /// the player who last hurt it (None inside: the host).
    pub(super) fn mod_mob_died(
        &mut self,
        m: &horizon_client::mobs::Mob,
        killer: Option<Option<u32>>,
    ) {
        if self.mods.server {
            let id = self.mod_killer_id(killer);
            self.mod_note_killer(m.id, id);
        }
        if !self.hears_events("entity-died") {
            return;
        }
        self.uuid_for(m.id);
        let Some(entity) = self.mob_info(m) else {
            return;
        };
        let killer = self.mod_killer_id(killer);
        self.mods.game_events.push(PendingEvent::Died {
            dimension: self.dim.id().to_owned(),
            entity,
            killer,
        });
    }

    /// A guest picked up `count` of `stack`: for `item-picked-up`.
    pub(super) fn mod_picked_up(&mut self, uuid: u128, stack: &ItemStack, count: u32) {
        if count == 0 || !self.hears_events("item-picked-up") {
            return;
        }
        let mut stack = host_stack(stack);
        stack.count = count;
        self.mods.game_events.push(PendingEvent::Picked {
            player: uuid_string(uuid),
            stack,
        });
    }

    /// Asked as an explosion goes off: true when a server mod stops it.
    pub(super) fn mod_explosion(&mut self, center: [f64; 3], power: f32) -> bool {
        if !self.hears_events("explosion") {
            return false;
        }
        self.lend_world();
        let dim = self.dim.id();
        let stop = self.mods_now(|m| m.explosion(dim, (center[0], center[1], center[2]), power));
        self.return_world();
        self.show_mod_output();
        stop
    }

    /// The tick's end, the world lent: mobs that appeared or were hurt in
    /// the level in place, players hurt, and what was queued.
    /// The flight of the mods' `projectile` mob types, a tick of it; a level
    /// not in place holds them still.
    pub(super) fn tick_mod_projectiles(&mut self, players: &[horizon_mods::ServerPlayer]) {
        let flying: Vec<u32> = self
            .mobs
            .iter()
            .filter(|m| m.kind.projectile.is_some() && !m.removed)
            .map(|m| m.id)
            .collect();
        for id in flying {
            let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
                continue;
            };
            let [gravity, drag] = m.kind.projectile.unwrap_or_default();
            let (from, motion) = (m.position, m.velocity);
            let through = m.kind.through_blocks;
            let to = horizon_core::geometry::Vec3::new(
                from.x + motion.x,
                from.y + motion.y,
                from.z + motion.z,
            );
            // A step at most a quarter of a block: nothing is flown through.
            let steps = (motion.length() * 4.0).ceil().clamp(1.0, 64.0) as i32;
            let mut hit: Option<(Option<String>, _)> = None;
            for step in 1..=steps {
                let t = f64::from(step) / f64::from(steps);
                let at = horizon_core::geometry::Vec3::new(
                    from.x + motion.x * t,
                    from.y + motion.y * t,
                    from.z + motion.z * t,
                );
                let target = self
                    .mobs
                    .iter()
                    .find(|o| {
                        o.id != id && !o.kind.display && !o.removed && o.bounds().contains(at)
                    })
                    .map(|o| o.id);
                if let Some(target) = target {
                    self.uuid_for(target);
                    let name = self
                        .mobs
                        .iter()
                        .find(|o| o.id == target)
                        .and_then(|o| self.mob_info(o));
                    hit = Some((name.map(|n| n.id), at));
                    break;
                }
                if let Some(p) = players.iter().find(|p| {
                    let (x, y, z) = (p.x, p.y, p.z);
                    (at.x - x).abs() < 0.3 && (at.z - z).abs() < 0.3 && at.y > y && at.y < y + 1.8
                }) {
                    hit = Some((Some(p.id.clone()), at));
                    break;
                }
                let pos = [
                    at.x.floor() as i32,
                    at.y.floor() as i32,
                    at.z.floor() as i32,
                ];
                if !through && self.blocks_motion(pos) {
                    hit = Some((None, at));
                    break;
                }
            }
            let mut gone = false;
            if let Some((target, at)) = hit {
                self.uuid_for(id);
                let info = self
                    .mobs
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| self.mob_info(m));
                let dim = self.dim.id();
                self.lend_world();
                let keep = info.is_some_and(|info| {
                    self.mods
                        .mods
                        .projectile_hit(dim, &info, target.as_deref(), [at.x, at.y, at.z])
                });
                self.return_world();
                gone = !keep;
            }
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                if gone {
                    m.removed = true;
                    continue;
                }
                m.position = to;
                m.velocity = horizon_core::geometry::Vec3::new(
                    motion.x * drag,
                    motion.y * drag - gravity,
                    motion.z * drag,
                );
            }
        }
    }

    /// The chunks mods' code generates, the mobs that tick by code, their goals
    /// and the mods' effects. The world is lent.
    fn mod_logic(&mut self, players: &[horizon_mods::ServerPlayer]) {
        const CHUNKS_A_TICK: usize = 32;
        for ask in horizon_client::mod_dimensions::take_asks(CHUNKS_A_TICK) {
            let fill = self
                .mods
                .mods
                .generate_chunk(ask.dimension, ask.x, ask.z)
                .map(|f| horizon_client::mod_dimensions::ChunkFill {
                    palette: f.palette,
                    min_y: f.min_y,
                    blocks: f.blocks,
                    biome: f.biome,
                });
            let _ = ask.reply.send(fill);
        }
        let dim = self.dim.id();
        let ids: Vec<u32> = self
            .mobs
            .iter()
            .filter(|m| !m.removed)
            .filter(|m| {
                m.kind.ticks
                    || !m.kind.mod_goals.is_empty()
                    || m.extra
                        .effects
                        .iter()
                        .any(|e| e.0.contains(':') && !e.0.starts_with("minecraft:"))
            })
            .map(|m| m.id)
            .collect();
        for id in ids {
            self.uuid_for(id);
            let Some(m) = self.mobs.iter().find(|m| m.id == id) else {
                continue;
            };
            let Some(info) = self.mob_info(m) else {
                continue;
            };
            let (ticks, goals, running, age) = (
                m.kind.ticks,
                m.kind.mod_goals.clone(),
                m.extra.mod_goal,
                m.age,
            );
            let effects: Vec<(&'static str, u8)> = m
                .extra
                .effects
                .iter()
                .filter(|e| !e.0.starts_with("minecraft:"))
                .map(|e| (e.0, e.2.clamp(0, 255) as u8))
                .collect();
            if ticks {
                self.mods.mods.entity_tick(dim, &info);
            }
            for (effect, amplifier) in effects {
                self.mods.mods.effect_tick(effect, &info.id, amplifier);
            }
            if !goals.is_empty() {
                let mut now = running.map(usize::from).filter(|&i| i < goals.len());
                // GoalSelector looks for better goals now and then.
                if (age as u64 + u64::from(id)) % 5 == 0 {
                    let above = now.unwrap_or(goals.len());
                    let better =
                        (0..above).find(|&i| self.mods.mods.goal_can_use(&goals[i].1, &info));
                    if let Some(better) = better {
                        if let Some(old) = now {
                            self.mods.mods.goal_stop(&goals[old].1, &info);
                        }
                        now = Some(better);
                    }
                }
                if let Some(i) = now
                    && !self.mods.mods.goal_tick(&goals[i].1, &info)
                {
                    self.mods.mods.goal_stop(&goals[i].1, &info);
                    now = None;
                }
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    m.extra.mod_goal = now.map(|i| i as u8);
                }
            }
        }
        for p in players {
            for (effect, amplifier) in self.mod_player_effects(&p.id) {
                self.mods.mods.effect_tick(effect, &p.id, amplifier);
            }
        }
    }

    /// The mods' effects on a player.
    fn mod_player_effects(&self, id: &str) -> Vec<(&'static str, u8)> {
        let own = |list: &horizon_client::effects::PlayerEffects| -> Vec<(&'static str, u8)> {
            horizon_client_core::potions::mod_effect_ids()
                .into_iter()
                .filter_map(|e| Some((e, list.get(e)?.amplifier.clamp(0, 255) as u8)))
                .collect()
        };
        if self.player.is_some() && self.remote.is_none() && self.player_uuid_string() == id {
            return own(&self.potions.effects);
        }
        self.lan
            .iter()
            .flat_map(|h| &h.hub.guests)
            .find(|g| uuid_string(g.uuid) == id)
            .map_or(Vec::new(), |g| own(&g.effects))
    }

    pub(super) fn mod_game_events(&mut self, players: &[horizon_mods::ServerPlayer]) {
        self.mod_logic(players);
        self.mod_mob_events();
        self.mod_player_hurt_events(players);
    }

    /// Mobs of the level in place that appeared or lost health since its last
    /// look; a level not in place has its look in `tick_parked_mod_levels`.
    fn mod_mob_events(&mut self) {
        let dim = self.dim.id();
        if self.hears_events("entity-spawned") || self.hears_events("entity-hurt") {
            let now: HashMap<u32, f32> = self
                .mobs
                .iter()
                .filter(|m| m.alive() && !m.removed)
                .map(|m| (m.id, m.health))
                .collect();
            if let Some(before) = self.mods.mob_health.insert(dim.to_owned(), now.clone()) {
                let mut spawned = Vec::new();
                let mut hurt = Vec::new();
                for (id, health) in &now {
                    match before.get(id) {
                        None => spawned.push(*id),
                        Some(&was) if was > health + 1e-4 => hurt.push((*id, was)),
                        _ => {}
                    }
                }
                spawned.sort_unstable();
                hurt.sort_unstable_by_key(|h| h.0);
                for id in spawned {
                    let Some(info) = self
                        .mobs
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| self.mob_info(m))
                    else {
                        continue;
                    };
                    if self.mods.mods.entity_spawned(dim, &info)
                        && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                    {
                        m.removed = true;
                    }
                }
                for (id, was) in hurt {
                    let Some(info) = self
                        .mobs
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| self.mob_info(m))
                    else {
                        continue;
                    };
                    let amount = was - info.health;
                    if self.mods.mods.entity_hurt(dim, &info, amount)
                        && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                    {
                        m.health = was;
                        if let Some(h) = self.mods.mob_health.get_mut(dim) {
                            h.insert(id, was);
                        }
                    }
                }
            }
        } else {
            self.mods.mob_health.clear();
        }
    }

    fn mod_player_hurt_events(&mut self, players: &[horizon_mods::ServerPlayer]) {
        if self.hears_events("player-hurt") {
            for p in players {
                let was = self.mods.player_health.insert(p.id.clone(), p.health);
                if let Some(was) = was
                    && was > p.health + 1e-4
                {
                    self.mods.mods.player_hurt(p, was - p.health);
                }
            }
        } else {
            self.mods.player_health.clear();
        }
        for event in std::mem::take(&mut self.mods.game_events) {
            match event {
                PendingEvent::Died {
                    dimension,
                    entity,
                    killer,
                } => self
                    .mods
                    .mods
                    .entity_died(&dimension, &entity, killer.as_deref()),
                PendingEvent::Picked { player, stack } => {
                    if let Some(p) = players.iter().find(|p| p.id == player) {
                        self.mods.mods.item_picked_up(p, &stack);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_styled_stack_keeps_its_look_and_the_mods_bytes() {
        let stack = HostStack {
            item: "minecraft:paper".into(),
            count: 3,
            components: Vec::new(),
            data: [("my_mod".to_string(), vec![7u8])].into_iter().collect(),
        };
        let look = horizon_mods::StackLook {
            name: Some("Note".into()),
            lore: vec!["line one".into(), "line two".into()],
            model: Some("my_mod:note".into()),
            glint: Some(true),
        };
        let styled = style_stack(&stack, &look).unwrap();
        assert_eq!(styled.count, 3);
        assert_eq!(styled.data, stack.data);
        let game = game_stack(&styled).unwrap();
        let d = game.data().unwrap();
        assert_eq!(game.custom_name().as_deref(), Some("Note"));
        assert!(matches!(d.other("minecraft:lore"), Some(Tag::List(l)) if l.len() == 2));
        assert!(matches!(
            d.other("minecraft:item_model"),
            Some(Tag::String(_))
        ));
        assert!(matches!(
            d.other("minecraft:enchantment_glint_override"),
            Some(Tag::Byte(1))
        ));
        // And it goes out over the network.
        let net =
            horizon_client::lan::guest::stack_to_network(|_| 1, |_, _| None, &Some(game)).unwrap();
        let has = |id: i32| net.added.iter().any(|(k, _)| *k == id);
        use horizon_protocol::item::component::{ENCHANTMENT_GLINT_OVERRIDE, ITEM_MODEL, LORE};
        assert!(has(LORE) && has(ITEM_MODEL) && has(ENCHANTMENT_GLINT_OVERRIDE));
    }
}

#[cfg(test)]
mod slot_rule_tests {
    use super::*;
    fn stack(item: &str, count: u32) -> Option<HostStack> {
        Some(HostStack {
            item: item.to_owned(),
            count,
            ..HostStack::default()
        })
    }
    #[test]
    fn slot_rules_send_refused_items_back() {
        let rule = |take_only, items: &[&str]| horizon_mods::SlotRule {
            take_only,
            items: items.iter().map(|s| (*s).to_owned()).collect(),
        };
        let rules = [(0, rule(true, &[])), (1, rule(false, &["minecraft:coal"]))]
            .into_iter()
            .collect();
        let last = vec![stack("minecraft:iron_nugget", 5), None, None];
        // Into an output: back. Wrong fuel: back. Free slot: stays.
        let shown = vec![
            stack("minecraft:iron_nugget", 9),
            stack("minecraft:stick", 1),
            stack("minecraft:stick", 2),
        ];
        let (kept, back) = apply_slot_rules(&last, shown, &rules);
        assert_eq!(
            kept,
            vec![
                stack("minecraft:iron_nugget", 5),
                None,
                stack("minecraft:stick", 2)
            ]
        );
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].count, 4);
        // Taking from an output, the right fuel: as they are.
        let shown = vec![
            stack("minecraft:iron_nugget", 1),
            stack("minecraft:coal", 1),
            None,
        ];
        let (kept, back) = apply_slot_rules(&last, shown.clone(), &rules);
        assert_eq!(kept, shown);
        assert!(back.is_empty());
    }
}
