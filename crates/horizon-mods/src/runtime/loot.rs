//! `loot` and `recipes-server` for server mods: the game's loot tables and
//! recipes, read from the world it lends the call
//! ([`WorldView`](super::WorldView)). Giving and dropping wait for the
//! tick's end as `players.give` and `entities.drop-item` do; filling a
//! container writes the lent block entity, as `storages.set-item` does.
use super::server_api::{HostStack, from_guest, to_guest};
use super::server_bindings::horizon::mods::{entities, loot, players, recipes_server};
use super::{State, check_game_id};
use std::sync::Arc;

/// A recipe as `recipes-server` lists it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecipeFacts {
    /// `minecraft:oak_planks`.
    pub id: String,
    /// The recipe file's type, `minecraft:crafting_shaped`.
    pub kind: String,
    /// The items it makes.
    pub results: Vec<String>,
    /// Every item that fits one of its slots.
    pub ingredients: Vec<String>,
}

/// What [`WorldView::loot`](super::WorldView::loot) rolls.
#[derive(Clone, Debug, PartialEq)]
pub enum LootRoll {
    Table(String),
    Block {
        state: String,
        tool: Option<HostStack>,
    },
    Entity {
        kind: String,
        by_player: bool,
    },
}

/// The most stacks a roll hands a mod.
pub const LOOT_STACKS: usize = 256;

impl State {
    fn seed(seed: Option<u64>) -> u64 {
        use std::hash::{BuildHasher, Hasher};
        seed.unwrap_or_else(|| {
            std::collections::hash_map::RandomState::new()
                .build_hasher()
                .finish()
        })
    }
    fn roll_loot(&self, roll: LootRoll, seed: Option<u64>) -> Result<Vec<HostStack>, String> {
        let view = self
            .world
            .lock()
            .map_err(|_| "the world is busy".to_string())?;
        let view = view.as_ref().ok_or("the game lent no world to this call")?;
        let mut out = view.loot(&roll, Self::seed(seed))?;
        out.truncate(LOOT_STACKS);
        Ok(out)
    }
    fn roll_out(
        &self,
        roll: LootRoll,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        Ok(self
            .roll_loot(roll, seed)?
            .iter()
            .map(|s| to_guest(s, &self.id))
            .collect())
    }
    fn recipe_list(&self) -> Option<Arc<Vec<RecipeFacts>>> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.recipes()
    }
    fn recipe_ids(&self, keep: impl Fn(&RecipeFacts) -> bool) -> Vec<String> {
        self.recipe_list().map_or_else(Vec::new, |all| {
            all.iter()
                .filter(|r| keep(r))
                .map(|r| r.id.clone())
                .collect()
        })
    }
    fn cooking(&self, stack: players::ItemStack) -> Option<(String, HostStack, u32, f32)> {
        let stack = from_guest(stack, &self.id).ok()?;
        let view = self.world.lock().ok()?;
        view.as_ref()?.match_cooking("minecraft:smelting", &stack)
    }
}

impl loot::Host for State {
    fn roll(
        &mut self,
        table: String,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        check_game_id("loot table", &table)?;
        self.roll_out(LootRoll::Table(table), seed)
    }
    fn table_exists(&mut self, table: String) -> bool {
        let Ok(view) = self.world.lock() else {
            return false;
        };
        view.as_ref().is_some_and(|v| v.loot_table_exists(&table))
    }
    fn block_drops(
        &mut self,
        state: String,
        tool: Option<players::ItemStack>,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        let tool = tool.map(|t| from_guest(t, &self.id)).transpose()?;
        self.roll_out(LootRoll::Block { state, tool }, seed)
    }
    fn entity_drops(
        &mut self,
        kind: String,
        by_player: bool,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        check_game_id("entity type", &kind)?;
        self.roll_out(LootRoll::Entity { kind, by_player }, seed)
    }
    fn give(
        &mut self,
        player: String,
        table: String,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        check_game_id("loot table", &table)?;
        self.online_player(&player)?;
        let stacks = self.roll_out(LootRoll::Table(table), seed)?;
        for s in &stacks {
            self.player_give(player.clone(), s.clone())?;
        }
        Ok(stacks)
    }
    fn drop(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        table: String,
        seed: Option<u64>,
    ) -> Result<Vec<players::ItemStack>, String> {
        check_game_id("loot table", &table)?;
        let stacks = self.roll_out(LootRoll::Table(table), seed)?;
        for s in &stacks {
            entities::Host::drop_item(self, dimension.clone(), x, y, z, s.clone())?;
        }
        Ok(stacks)
    }
    fn fill_container(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        table: String,
        seed: Option<u64>,
    ) -> Result<u32, String> {
        check_game_id("loot table", &table)?;
        let seed = Self::seed(seed);
        let stacks = self.roll_loot(LootRoll::Table(table), Some(seed))?;
        let mut view = self
            .world
            .lock()
            .map_err(|_| "the world is busy".to_string())?;
        let view = view.as_mut().ok_or("the game lent no world to this call")?;
        let slots = view
            .container(&dimension, x, y, z)
            .ok_or("no container there")?;
        let mut order: Vec<usize> = (0..slots.len()).collect();
        // `LootTable.fill`'s shuffle of the free slots, from the roll's seed.
        let mut r = seed | 1;
        for i in (1..order.len()).rev() {
            r ^= r << 13;
            r ^= r >> 7;
            r ^= r << 17;
            order.swap(i, (r % (i as u64 + 1)) as usize);
        }
        let mut filled = vec![None; slots.len()];
        let mut placed = 0;
        for (stack, &slot) in stacks.into_iter().zip(&order) {
            filled[slot] = Some(stack);
            placed += 1;
        }
        if !view.set_container(&dimension, x, y, z, filled) {
            return Err("no container there".into());
        }
        Ok(placed)
    }
}

impl recipes_server::Host for State {
    fn all(&mut self) -> Vec<String> {
        self.recipe_ids(|_| true)
    }
    fn by_type(&mut self, kind: String) -> Vec<String> {
        self.recipe_ids(|r| r.kind == kind)
    }
    fn by_result(&mut self, item: String) -> Vec<String> {
        self.recipe_ids(|r| r.results.contains(&item))
    }
    fn by_ingredient(&mut self, item: String) -> Vec<String> {
        self.recipe_ids(|r| r.ingredients.contains(&item))
    }
    fn get(&mut self, id: String) -> Option<String> {
        check_game_id("recipe", &id).ok()?;
        if id.split(['/', ':']).any(|part| part == ".." || part == ".") {
            return None;
        }
        let view = self.world.lock().ok()?;
        view.as_ref()?.recipe_json(&id)
    }
    fn match_crafting(
        &mut self,
        grid: Vec<Option<players::ItemStack>>,
    ) -> Option<(String, players::ItemStack)> {
        if grid.len() != 4 && grid.len() != 9 {
            return None;
        }
        let grid: Vec<Option<HostStack>> = grid
            .into_iter()
            .map(|s| s.and_then(|s| from_guest(s, &self.id).ok()))
            .collect();
        let view = self.world.lock().ok()?;
        let (id, made) = view.as_ref()?.match_crafting(&grid)?;
        Some((id, to_guest(&made, &self.id)))
    }
    fn match_smelting(
        &mut self,
        stack: players::ItemStack,
    ) -> Option<(String, players::ItemStack)> {
        let (id, made, _, _) = self.cooking(stack)?;
        Some((id, to_guest(&made, &self.id)))
    }
    fn match_stonecutting(
        &mut self,
        stack: players::ItemStack,
    ) -> Vec<(String, players::ItemStack)> {
        let Ok(stack) = from_guest(stack, &self.id) else {
            return Vec::new();
        };
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        let Some(view) = view.as_ref() else {
            return Vec::new();
        };
        view.match_stonecutting(&stack)
            .into_iter()
            .map(|(id, made)| (id, to_guest(&made, &self.id)))
            .collect()
    }
    fn smelting_time(&mut self, stack: players::ItemStack) -> Option<u32> {
        Some(self.cooking(stack)?.2)
    }
    fn smelting_xp(&mut self, stack: players::ItemStack) -> Option<f32> {
        Some(self.cooking(stack)?.3)
    }
    fn of_own_type(&mut self, kind: String) -> Vec<(String, String)> {
        // A mod reads the types of its own namespace, not vanilla's or another mod's.
        if check_game_id("recipe type", &kind).is_err()
            || kind.split_once(':').map(|(ns, _)| ns) != Some(self.id.as_str())
        {
            return Vec::new();
        }
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref()
            .map_or_else(Vec::new, |v| v.recipes_of_type(&kind))
    }
}
