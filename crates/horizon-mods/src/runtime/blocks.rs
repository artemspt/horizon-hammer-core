//! API level 24: `blocks` and `chunks`, and what `players` reads of a body.
//! Types come from the game's tables, positions from the world it lends the
//! call; a body is copied into the snapshot only while a mod reads one.
use super::server_api::{HostStack, PlayerChange, ServerAction};
use super::server_bindings::horizon::mods::{blocks, chunks};
use super::{BLOCK_DATA_BYTES, State, WorldView, from_hex, to_hex};

/// What the game knows of a block state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockFacts {
    pub hardness: f32,
    pub emission: u8,
    pub solid: bool,
    pub replaceable: bool,
    pub opaque: bool,
    pub collision: bool,
    pub requires_tool: bool,
    /// The fluid's id and amount; `None` for none.
    pub fluid: Option<(String, u8)>,
}

/// A player's body, for `players`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerState {
    pub id: String,
    pub food: u32,
    pub saturation: f32,
    pub air: i32,
    pub max_health: f32,
    pub xp_level: u32,
    pub xp_progress: f32,
    pub xp_total: u32,
    pub velocity: (f64, f64, f64),
    pub sprinting: bool,
    pub flying: bool,
    pub on_ground: bool,
    pub sleeping: bool,
    pub swimming: bool,
    pub gliding: bool,
    pub using: bool,
    pub permission: u8,
    pub ping: i32,
    pub ender: Vec<Option<HostStack>>,
    /// Dimension and block.
    pub spawn: Option<(String, (i32, i32, i32))>,
    /// The advancements with every criterion.
    pub advancements: Vec<String>,
}

/// The reads only a [`PlayerState`] answers: a mod importing one makes the
/// game build them for each call.
pub const PLAYER_STATE_READS: [&str; 17] = [
    "players.food",
    "players.air",
    "players.max-health",
    "players.xp",
    "players.velocity",
    "players.is-sprinting",
    "players.is-flying",
    "players.is-on-ground",
    "players.is-sleeping",
    "players.is-swimming",
    "players.is-gliding",
    "players.is-using",
    "players.is-op",
    "players.ping",
    "players.ender-chest",
    "players.spawn-point",
    "players.has-advancement",
];

/// A chunk's data key.
fn chunk_key(dimension: &str, x: i32, z: i32) -> String {
    format!("{dimension} {x} {z}")
}

/// `execute in` for commands that act on the level in place otherwise.
fn in_dimension(dimension: &str, line: &str) -> String {
    format!("execute in {dimension} run {line}")
}

/// Points into a level, as vanilla's `getXpNeededForNextLevel`.
pub(super) fn xp_for_level(level: u32) -> u32 {
    match level {
        30.. => 112u32.saturating_add((level - 30).saturating_mul(9)),
        15.. => 37 + (level - 15) * 5,
        _ => 7 + level * 2,
    }
}

impl State {
    pub(super) fn player_state(&self, player: &str) -> Option<&PlayerState> {
        self.snapshot.player_states.iter().find(|p| p.id == player)
    }
    pub(super) fn player_flag(&self, player: &str, flag: fn(&PlayerState) -> bool) -> bool {
        self.player_state(player).is_some_and(flag)
    }
    /// A change to a player who is online.
    pub(super) fn change_player(
        &mut self,
        player: String,
        change: PlayerChange,
    ) -> Result<(), String> {
        self.check_online(&player)?;
        self.act(ServerAction::Player { player, change })
    }
    pub(super) fn check_online(&self, player: &str) -> Result<(), String> {
        if self.snapshot.players.iter().any(|p| p.id == player) {
            Ok(())
        } else {
            Err(format!("{player} is not online"))
        }
    }
    fn view<R>(&self, f: impl FnOnce(&dyn WorldView) -> R) -> Option<R> {
        let view = self.world.lock().ok()?;
        Some(f(view.as_ref()?.as_ref()))
    }
    fn facts(&self, state: &str) -> Option<BlockFacts> {
        self.view(|v| v.block_facts(state)).flatten()
    }
    fn light_at(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<(u8, u8)> {
        self.view(|v| v.light(dimension, x, y, z)).flatten()
    }
    fn known_dimension(&self, dimension: &str) -> Result<(), String> {
        if self.view(|v| v.has_dimension(dimension)) == Some(true) {
            Ok(())
        } else {
            Err(format!("no dimension {dimension}"))
        }
    }
}

impl blocks::Host for State {
    fn all(&mut self) -> Vec<String> {
        self.view(|v| v.block_ids()).unwrap_or_default()
    }
    fn properties(&mut self, block: String) -> Vec<(String, Vec<String>)> {
        self.view(|v| v.block_properties(&block))
            .unwrap_or_default()
    }
    fn default_state(&mut self, block: String) -> Option<String> {
        self.view(|v| v.default_state(&block)).flatten()
    }
    fn hardness(&mut self, state: String) -> Option<f32> {
        Some(self.facts(&state)?.hardness)
    }
    fn light_emission(&mut self, state: String) -> Option<u8> {
        Some(self.facts(&state)?.emission)
    }
    fn light_at(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
        let (sky, block) = State::light_at(self, &dimension, x, y, z)?;
        Some(sky.max(block))
    }
    fn sky_light_at(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
        Some(State::light_at(self, &dimension, x, y, z)?.0)
    }
    fn block_light_at(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
        Some(State::light_at(self, &dimension, x, y, z)?.1)
    }
    fn can_see_sky(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        State::light_at(self, &dimension, x, y, z).is_some_and(|(sky, _)| sky == 15)
    }
    fn is_solid(&mut self, state: String) -> bool {
        self.facts(&state).is_some_and(|f| f.solid)
    }
    fn is_replaceable(&mut self, state: String) -> bool {
        self.facts(&state).is_some_and(|f| f.replaceable)
    }
    fn is_opaque(&mut self, state: String) -> bool {
        self.facts(&state).is_some_and(|f| f.opaque)
    }
    fn has_collision(&mut self, state: String) -> bool {
        self.facts(&state).is_some_and(|f| f.collision)
    }
    fn requires_tool(&mut self, state: String) -> bool {
        self.facts(&state).is_some_and(|f| f.requires_tool)
    }
    fn is_waterlogged(&mut self, state: String) -> bool {
        state.contains("waterlogged=true")
    }
    fn fluid_at(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<(String, u8)> {
        let state = self.view(|v| v.block(&dimension, x, y, z)).flatten()?;
        self.facts(&state)?.fluid
    }
    fn has_block_entity(&mut self, dimension: String, x: i32, y: i32, z: i32) -> bool {
        self.view(|v| v.block_entity_nbt(&dimension, x, y, z).is_some()) == Some(true)
    }
    fn get_nbt(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        self.view(|v| v.block_entity_nbt(&dimension, x, y, z))
            .flatten()
    }
    fn merge_nbt(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        snbt: String,
    ) -> Result<(), String> {
        self.known_dimension(&dimension)?;
        self.console(in_dimension(
            &dimension,
            &format!("data merge block {x} {y} {z} {snbt}"),
        ))
    }
}

impl chunks::Host for State {
    fn state_id(&mut self, state: String) -> Option<u32> {
        self.view(|v| v.state_id(&state)).flatten()
    }
    fn state_of_id(&mut self, id: u32) -> Option<String> {
        self.view(|v| v.state_of_id(id)).flatten()
    }
    fn read_section(&mut self, dimension: String, cx: i32, sy: i32, cz: i32) -> Option<Vec<u32>> {
        self.view(|v| v.section(&dimension, cx, sy, cz)).flatten()
    }
    fn write_section(
        &mut self,
        dimension: String,
        cx: i32,
        sy: i32,
        cz: i32,
        ids: Vec<u32>,
    ) -> Result<(), String> {
        if ids.len() != 4096 {
            return Err(format!("{} numbers: a section has 4096", ids.len()));
        }
        let changes = self
            .view(|v| {
                let old = v
                    .section(&dimension, cx, sy, cz)
                    .ok_or_else(|| format!("no section {cx} {sy} {cz} in {dimension}"))?;
                let mut changes = Vec::new();
                for (i, (&new, &was)) in ids.iter().zip(&old).enumerate() {
                    if new == was {
                        continue;
                    }
                    let state = v
                        .state_of_id(new)
                        .ok_or_else(|| format!("{new}: no state has this number"))?;
                    let i = i as i32;
                    changes.push(super::mod_world::BlockChange {
                        x: cx.wrapping_mul(16).wrapping_add(i & 15),
                        y: sy.wrapping_mul(16).wrapping_add(i >> 8),
                        z: cz.wrapping_mul(16).wrapping_add((i >> 4) & 15),
                        state,
                    });
                }
                Ok::<_, String>(changes)
            })
            .ok_or("no world to write to")??;
        super::mod_world::Host::set_blocks(self, dimension, changes)
    }
    fn heightmap(&mut self, dimension: String, cx: i32, cz: i32) -> Option<Vec<i32>> {
        self.view(|v| v.heightmap(&dimension, cx, cz)).flatten()
    }
    fn loaded(&mut self, dimension: String) -> Vec<(i32, i32)> {
        self.view(|v| v.loaded_chunks(&dimension))
            .unwrap_or_default()
    }
    fn force_load(&mut self, dimension: String, cx: i32, cz: i32) -> Result<(), String> {
        self.known_dimension(&dimension)?;
        let (x, z) = (cx.wrapping_mul(16), cz.wrapping_mul(16));
        self.console(in_dimension(&dimension, &format!("forceload add {x} {z}")))
    }
    fn force_unload(&mut self, dimension: String, cx: i32, cz: i32) -> Result<(), String> {
        self.known_dimension(&dimension)?;
        let (x, z) = (cx.wrapping_mul(16), cz.wrapping_mul(16));
        self.console(in_dimension(
            &dimension,
            &format!("forceload remove {x} {z}"),
        ))
    }
    fn get_data(&mut self, dimension: String, cx: i32, cz: i32) -> Option<Vec<u8>> {
        let data = self.block_data.lock().ok()?;
        let hex = data
            .chunk_data
            .get(&self.id)?
            .get(&chunk_key(&dimension, cx, cz))?;
        Some(from_hex(hex))
    }
    fn set_data(
        &mut self,
        dimension: String,
        cx: i32,
        cz: i32,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        if bytes.len() > BLOCK_DATA_BYTES {
            return Err(format!("{} bytes: at most {BLOCK_DATA_BYTES}", bytes.len()));
        }
        self.known_dimension(&dimension)?;
        let mut data = self
            .block_data
            .lock()
            .map_err(|_| "block data unavailable".to_string())?;
        let mine = data.chunk_data.entry(self.id.clone()).or_default();
        let key = chunk_key(&dimension, cx, cz);
        if bytes.is_empty() {
            mine.remove(&key);
            return Ok(());
        }
        if mine.len() >= CHUNKS_PER_MOD && !mine.contains_key(&key) {
            return Err(format!("at most {CHUNKS_PER_MOD} chunks of data"));
        }
        mine.insert(key, to_hex(&bytes));
        Ok(())
    }
    fn data_chunks(&mut self, dimension: String) -> Vec<(i32, i32)> {
        let Ok(data) = self.block_data.lock() else {
            return Vec::new();
        };
        let prefix = format!("{dimension} ");
        data.chunk_data
            .get(&self.id)
            .into_iter()
            .flat_map(|m| m.keys())
            .filter_map(|k| {
                let (x, z) = k.strip_prefix(&prefix)?.split_once(' ')?;
                Some((x.parse().ok()?, z.parse().ok()?))
            })
            .collect()
    }
}

const CHUNKS_PER_MOD: usize = 65536;

/// `players` at API level 24, called from its `Host` (one impl per interface).
impl State {
    pub(super) fn tell_json_line(&self, target: &str, json: &str) -> Result<String, String> {
        serde_json::from_str::<serde_json::Value>(json).map_err(|e| format!("not JSON: {e}"))?;
        Ok(format!("tellraw {target} {json}"))
    }
    pub(super) fn ray_block(&self, player: &str, reach: f64) -> Option<(i32, i32, i32)> {
        let p = self.snapshot.players.iter().find(|p| p.id == player)?;
        let &(_, yaw, pitch) = self.snapshot.looks.iter().find(|l| l.0 == player)?;
        let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
        let dir = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        ];
        let cells = super::extra::ray_cells([p.x, p.y + 1.62, p.z], dir, reach);
        self.view(|v| {
            cells.iter().map(|c| c.0).find(|&(x, y, z)| {
                v.block(&p.dimension, x, y, z)
                    .is_some_and(|s| !super::is_air_block(super::block_id(&s)))
            })
        })
        .flatten()
    }
}
