//! `items` and `dimensions` for server mods: what the game knows of item
//! types, stacks and dimensions, read from the world it lends the call
//! ([`WorldView`](super::WorldView)). A stack goes to the game in its own
//! encoding and comes back as a new one: the mod's bytes ride along.
use super::server_api::{HostStack, from_guest, to_guest};
use super::server_bindings::horizon::mods::{dimensions, items, players};
use super::{ServerPlayer, State};

/// What the game knows of an item type.
#[derive(Clone, Debug, Default)]
pub struct ItemFacts {
    /// In the server's language.
    pub name: String,
    pub max_stack: u32,
    pub max_damage: u32,
    /// Nutrition, saturation, eaten when full.
    pub food: Option<(u32, f32, bool)>,
    /// The block it places.
    pub block: Option<String>,
    pub fuel_ticks: u32,
    pub rarity: String,
}

/// An attribute modifier, as `items.modifier` has it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModifierFacts {
    pub attribute: String,
    pub id: String,
    pub amount: f64,
    pub operation: String,
    pub slot: String,
}

/// What a stack carries beyond its item and count.
#[derive(Clone, Debug, Default)]
pub struct StackFacts {
    /// Every component as SNBT, `{}` for none.
    pub components: String,
    /// Each component's SNBT, by name.
    pub each: Vec<(String, String)>,
    pub damage: u32,
    pub custom_name: Option<String>,
    pub lore: Vec<String>,
    pub enchantments: Vec<(String, u32)>,
    pub glint: bool,
    pub modifiers: Vec<ModifierFacts>,
    pub dyed: Option<u32>,
    /// The potion and its effects: (effect, duration, amplifier).
    pub potion: Option<(String, Vec<(String, i32, u8)>)>,
    pub contents: Vec<HostStack>,
}

/// A change `items` asks the game to make to a stack.
#[derive(Clone, Debug, PartialEq)]
pub enum StackEdit {
    SetComponent(String, String),
    RemoveComponent(String),
    Damage(u32),
    Repair(u32),
    Enchant(String, u32),
    Disenchant(String),
    AddModifier(ModifierFacts),
    Dye(Option<u32>),
    Potion(Option<String>),
}

/// A dimension the server has.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionFacts {
    pub id: String,
    /// The vanilla dimension it's made over (itself for a vanilla one).
    pub base: String,
    pub min_y: i32,
    pub max_y: i32,
    pub loaded: bool,
}

/// The longest SNBT a mod may hand in, and a component's name.
const SNBT: usize = 32 * 1024;
const NAME: usize = 128;
const OPERATIONS: [&str; 3] = ["add_value", "add_multiplied_base", "add_multiplied_total"];
const SLOTS: [&str; 10] = [
    "any", "mainhand", "offhand", "hand", "head", "chest", "legs", "feet", "armor", "body",
];

/// `minecraft:name`: a namespaced id of the characters ids take.
fn check_id(what: &str, id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= NAME
        && id.split_once(':').is_some_and(|(ns, path)| {
            !ns.is_empty()
                && !path.is_empty()
                && ns
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
                && path
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'))
        });
    if ok {
        Ok(())
    } else {
        Err(format!("{what} {id:?} is not a namespaced id"))
    }
}

impl State {
    fn item_facts(&self, item: &str) -> Option<ItemFacts> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.item_facts(item)
    }
    fn stack_in(&self, stack: players::ItemStack) -> Result<HostStack, String> {
        from_guest(stack, &self.id)
    }
    fn stack_out(&self, stack: &HostStack) -> players::ItemStack {
        to_guest(stack, &self.id)
    }
    fn stack_facts(&self, stack: players::ItemStack) -> Option<StackFacts> {
        let host = self.stack_in(stack).ok()?;
        let view = self.world.lock().ok()?;
        view.as_ref()?.stack_facts(&host)
    }
    fn edit(
        &self,
        stack: players::ItemStack,
        edit: StackEdit,
    ) -> Result<players::ItemStack, String> {
        let host = self.stack_in(stack)?;
        if host.count == 0 {
            return Err("an empty stack".into());
        }
        let view = self
            .world
            .lock()
            .map_err(|_| "the world is busy".to_string())?;
        let view = view.as_ref().ok_or("the game lent no world to this call")?;
        let mut out = view.edit_stack(&host, &edit)?;
        // The mods' bytes are the stack's, whatever the game kept.
        out.data = host.data;
        Ok(self.stack_out(&out))
    }
    fn dimension_facts(&self) -> Vec<DimensionFacts> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref().map_or_else(Vec::new, |v| v.dimensions())
    }
    fn dimension(&self, id: &str) -> Option<DimensionFacts> {
        self.dimension_facts().into_iter().find(|d| d.id == id)
    }
}

impl items::Host for State {
    fn all(&mut self) -> Vec<String> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref().map_or_else(Vec::new, |v| v.item_ids())
    }
    fn exists_in_tag(&mut self, tag: String) -> Option<Vec<String>> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.items_in_tag(tag.trim_start_matches('#'))
    }
    fn name(&mut self, item: String) -> Option<String> {
        Some(self.item_facts(&item)?.name)
    }
    fn max_damage(&mut self, item: String) -> Option<u32> {
        Some(self.item_facts(&item)?.max_damage)
    }
    fn is_damageable(&mut self, item: String) -> bool {
        self.item_facts(&item).is_some_and(|i| i.max_damage > 0)
    }
    fn is_food(&mut self, item: String) -> bool {
        self.item_facts(&item).is_some_and(|i| i.food.is_some())
    }
    fn food_values(&mut self, item: String) -> Option<items::Food> {
        let (nutrition, saturation, can_always_eat) = self.item_facts(&item)?.food?;
        Some(items::Food {
            nutrition,
            saturation,
            can_always_eat,
        })
    }
    fn is_block_item(&mut self, item: String) -> bool {
        self.item_facts(&item).is_some_and(|i| i.block.is_some())
    }
    fn block_of(&mut self, item: String) -> Option<String> {
        self.item_facts(&item)?.block
    }
    fn fuel_ticks(&mut self, item: String) -> u32 {
        self.item_facts(&item).map_or(0, |i| i.fuel_ticks)
    }
    fn rarity(&mut self, item: String) -> Option<String> {
        Some(self.item_facts(&item)?.rarity)
    }

    fn components(&mut self, stack: players::ItemStack) -> Result<String, String> {
        self.stack_facts(stack)
            .map(|f| f.components)
            .ok_or_else(|| "the game lent no world to this call".into())
    }
    fn get_component(&mut self, stack: players::ItemStack, name: String) -> Option<String> {
        self.stack_facts(stack)?
            .each
            .into_iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v)
    }
    fn set_component(
        &mut self,
        stack: players::ItemStack,
        name: String,
        value: String,
    ) -> Result<players::ItemStack, String> {
        check_id("component", &name)?;
        if value.len() > SNBT {
            return Err(format!("a component's value takes at most {SNBT} bytes"));
        }
        self.edit(stack, StackEdit::SetComponent(name, value))
    }
    fn remove_component(
        &mut self,
        stack: players::ItemStack,
        name: String,
    ) -> Result<players::ItemStack, String> {
        check_id("component", &name)?;
        self.edit(stack, StackEdit::RemoveComponent(name))
    }
    fn get_damage(&mut self, stack: players::ItemStack) -> u32 {
        self.stack_facts(stack).map_or(0, |f| f.damage)
    }
    fn set_damage(
        &mut self,
        stack: players::ItemStack,
        damage: u32,
    ) -> Result<players::ItemStack, String> {
        self.edit(stack, StackEdit::Damage(damage))
    }
    fn repair(
        &mut self,
        stack: players::ItemStack,
        amount: u32,
    ) -> Result<players::ItemStack, String> {
        self.edit(stack, StackEdit::Repair(amount))
    }
    fn get_custom_name(&mut self, stack: players::ItemStack) -> Option<String> {
        self.stack_facts(stack)?.custom_name
    }
    fn get_lore(&mut self, stack: players::ItemStack) -> Vec<String> {
        self.stack_facts(stack).map_or_else(Vec::new, |f| f.lore)
    }
    fn same_item(&mut self, a: players::ItemStack, b: players::ItemStack) -> bool {
        a.item == b.item
    }
    fn can_stack(&mut self, a: players::ItemStack, b: players::ItemStack) -> bool {
        let (Ok(a), Ok(b)) = (self.stack_in(a), self.stack_in(b)) else {
            return false;
        };
        let max = self.item_facts(&a.item).map_or(0, |i| i.max_stack);
        a.stacks_with(&b) && u64::from(a.count) + u64::from(b.count) <= u64::from(max)
    }
    fn split(
        &mut self,
        stack: players::ItemStack,
        count: u32,
    ) -> (players::ItemStack, players::ItemStack) {
        let taken = count.min(stack.count);
        let mut part = stack.clone();
        part.count = taken;
        let mut rest = stack;
        rest.count -= taken;
        (part, rest)
    }
    fn grow(&mut self, stack: players::ItemStack, by: i32) -> players::ItemStack {
        let max = self
            .item_facts(&stack.item)
            .map_or(stack.count, |i| i.max_stack);
        let mut out = stack;
        out.count = (i64::from(out.count) + i64::from(by)).clamp(0, i64::from(max)) as u32;
        out
    }
    fn enchantments(&mut self, stack: players::ItemStack) -> Vec<(String, u32)> {
        self.stack_facts(stack)
            .map_or_else(Vec::new, |f| f.enchantments)
    }
    fn enchantment_level(&mut self, stack: players::ItemStack, enchantment: String) -> u32 {
        self.stack_facts(stack).map_or(0, |f| {
            f.enchantments
                .iter()
                .find(|(e, _)| *e == enchantment)
                .map_or(0, |e| e.1)
        })
    }
    fn set_enchantment(
        &mut self,
        stack: players::ItemStack,
        enchantment: String,
        level: u32,
    ) -> Result<players::ItemStack, String> {
        check_id("enchantment", &enchantment)?;
        if !(1..=255).contains(&level) {
            return Err("an enchantment's level is 1 to 255".into());
        }
        self.edit(stack, StackEdit::Enchant(enchantment, level))
    }
    fn remove_enchantment(
        &mut self,
        stack: players::ItemStack,
        enchantment: String,
    ) -> Result<players::ItemStack, String> {
        check_id("enchantment", &enchantment)?;
        self.edit(stack, StackEdit::Disenchant(enchantment))
    }
    fn has_glint(&mut self, stack: players::ItemStack) -> bool {
        self.stack_facts(stack).is_some_and(|f| f.glint)
    }
    fn attribute_modifiers(&mut self, stack: players::ItemStack) -> Vec<items::Modifier> {
        self.stack_facts(stack)
            .map_or_else(Vec::new, |f| f.modifiers)
            .into_iter()
            .map(|m| items::Modifier {
                attribute: m.attribute,
                id: m.id,
                amount: m.amount,
                operation: m.operation,
                slot: m.slot,
            })
            .collect()
    }
    fn add_attribute_modifier(
        &mut self,
        stack: players::ItemStack,
        modifier: items::Modifier,
    ) -> Result<players::ItemStack, String> {
        check_id("attribute", &modifier.attribute)?;
        check_id("modifier id", &modifier.id)?;
        if !OPERATIONS.contains(&modifier.operation.as_str()) {
            return Err(format!("operation is one of {}", OPERATIONS.join(", ")));
        }
        if !SLOTS.contains(&modifier.slot.as_str()) {
            return Err(format!("slot is one of {}", SLOTS.join(", ")));
        }
        if !modifier.amount.is_finite() {
            return Err("amount is a number".into());
        }
        self.edit(
            stack,
            StackEdit::AddModifier(ModifierFacts {
                attribute: modifier.attribute,
                id: modifier.id,
                amount: modifier.amount,
                operation: modifier.operation,
                slot: modifier.slot,
            }),
        )
    }
    fn tool_speed(&mut self, tool: players::ItemStack, block: String) -> f32 {
        let Ok(tool) = self.stack_in(tool) else {
            return 1.0;
        };
        let Ok(view) = self.world.lock() else {
            return 1.0;
        };
        view.as_ref()
            .and_then(|v| v.tool(&tool, &block))
            .map_or(1.0, |t| t.0)
    }
    fn is_correct_tool(&mut self, tool: players::ItemStack, block: String) -> bool {
        let Ok(tool) = self.stack_in(tool) else {
            return false;
        };
        let Ok(view) = self.world.lock() else {
            return false;
        };
        view.as_ref()
            .and_then(|v| v.tool(&tool, &block))
            .is_some_and(|t| t.1)
    }
    fn dyed_color(&mut self, stack: players::ItemStack) -> Option<u32> {
        self.stack_facts(stack)?.dyed
    }
    fn set_dyed_color(
        &mut self,
        stack: players::ItemStack,
        color: Option<u32>,
    ) -> Result<players::ItemStack, String> {
        if color.is_some_and(|c| c > 0xFF_FFFF) {
            return Err("a dye colour is RGB, at most 0xFFFFFF".into());
        }
        self.edit(stack, StackEdit::Dye(color))
    }
    fn potion_contents(
        &mut self,
        stack: players::ItemStack,
    ) -> Option<(String, Vec<items::PotionEffect>)> {
        let (potion, effects) = self.stack_facts(stack)?.potion?;
        let effects = effects
            .into_iter()
            .map(|(effect, duration, amplifier)| items::PotionEffect {
                effect,
                duration,
                amplifier,
            })
            .collect();
        Some((potion, effects))
    }
    fn set_potion_contents(
        &mut self,
        stack: players::ItemStack,
        potion: Option<String>,
    ) -> Result<players::ItemStack, String> {
        if let Some(p) = &potion {
            check_id("potion", p)?;
        }
        self.edit(stack, StackEdit::Potion(potion))
    }
    fn container_contents(&mut self, stack: players::ItemStack) -> Vec<players::ItemStack> {
        let contents = self
            .stack_facts(stack)
            .map_or_else(Vec::new, |f| f.contents);
        contents.iter().map(|s| self.stack_out(s)).collect()
    }
}

impl dimensions::Host for State {
    fn list(&mut self) -> Vec<String> {
        self.dimension_facts().into_iter().map(|d| d.id).collect()
    }
    fn exists(&mut self, dimension: String) -> bool {
        self.dimension(&dimension).is_some()
    }
    fn loaded(&mut self, dimension: String) -> bool {
        self.dimension(&dimension).is_some_and(|d| d.loaded)
    }
    fn base(&mut self, dimension: String) -> Option<String> {
        Some(self.dimension(&dimension)?.base)
    }
    fn min_y(&mut self, dimension: String) -> Option<i32> {
        Some(self.dimension(&dimension)?.min_y)
    }
    fn max_y(&mut self, dimension: String) -> Option<i32> {
        Some(self.dimension(&dimension)?.max_y)
    }
    fn players_in(&mut self, dimension: String) -> Vec<ServerPlayer> {
        self.snapshot
            .players
            .iter()
            .filter(|p| p.dimension == dimension)
            .cloned()
            .collect()
    }
}
