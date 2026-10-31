//! Dimensions mods declare (`data/<id>/horizon/dimension/*.json`): a level of
//! its own over a vanilla dimension type, with a generator: vanilla's noise of
//! a dimension, flat layers, or the mod's code (`logic-events.generate-chunk`).
use crate::{chunk::Chunk, streaming::ChunkGenerator};
use horizon_mods::content::DimensionGenerator;
use std::sync::{Mutex, RwLock, mpsc};
use std::time::Duration;

/// A mod's dimension.
pub struct ModDimension {
    pub id: &'static str,
    /// `minecraft:overworld`, `minecraft:the_nether` or `minecraft:the_end`.
    pub base: &'static str,
    pub generator: DimensionGenerator,
}
static DIMENSIONS: RwLock<Vec<&'static ModDimension>> = RwLock::new(Vec::new());

pub fn set(dimensions: Vec<ModDimension>) {
    *DIMENSIONS.write().expect("mod dimensions poisoned") = dimensions
        .into_iter()
        .take(255)
        .map(|d| &*Box::leak(Box::new(d)))
        .collect();
}
pub fn all() -> Vec<&'static ModDimension> {
    DIMENSIONS.read().expect("mod dimensions poisoned").clone()
}
pub fn get(index: u8) -> Option<&'static ModDimension> {
    all().get(usize::from(index)).copied()
}
pub fn index_of(id: &str) -> Option<u8> {
    all().iter().position(|d| d.id == id).map(|i| i as u8)
}
/// The vanilla dimension type behind a level's name.
pub fn base_of(id: &str) -> &str {
    all().iter().find(|d| d.id == id).map_or(id, |d| d.base)
}

/// What a mod's code says a chunk holds: `blocks` index `palette`, y then z
/// then x, from `min_y` up.
#[derive(Clone, Debug)]
pub struct ChunkFill {
    pub palette: Vec<String>,
    pub min_y: i32,
    pub blocks: Vec<u16>,
    pub biome: Option<String>,
}
/// A terrain worker asking for a chunk: mods run on the game's thread.
pub struct ChunkAsk {
    pub dimension: &'static str,
    pub x: i32,
    pub z: i32,
    pub reply: mpsc::Sender<Option<ChunkFill>>,
}
static ASKS: Mutex<Vec<ChunkAsk>> = Mutex::new(Vec::new());
/// The chunks waiting for a mod, at most `limit`.
pub fn take_asks(limit: usize) -> Vec<ChunkAsk> {
    let mut asks = ASKS.lock().expect("chunk asks poisoned");
    let n = asks.len().min(limit);
    asks.drain(..n).collect()
}
/// How long a worker waits for the game's thread.
const ANSWER_TIME: Duration = Duration::from_secs(20);

/// A state from `ns:block[a=b,c=d]`.
fn state_of(registry: &horizon_world::registry::BlockRegistry, name: &str) -> Option<u32> {
    let (block, props) = match name.split_once('[') {
        Some((b, rest)) => (b, rest.strip_suffix(']')?),
        None => (name, ""),
    };
    let default = registry.default_state(block)?;
    if props.is_empty() {
        return Some(default.0);
    }
    let mut state = registry.state(default)?.clone();
    for pair in props.split(',') {
        let (k, v) = pair.split_once('=')?;
        state.properties.insert(k.to_owned(), v.to_owned());
    }
    registry.id(&state).map(|s| s.0)
}

/// A dimension whose chunks a mod's code fills.
pub struct CodeTerrain {
    pub dimension: &'static str,
    pub registry: &'static horizon_world::registry::BlockRegistry,
    pub biomes: std::sync::Arc<crate::biome::Biomes>,
}
impl CodeTerrain {
    fn fill(&self, chunk: &mut Chunk, fill: ChunkFill) {
        let states: Vec<u32> = fill
            .palette
            .iter()
            .map(|name| state_of(self.registry, name).unwrap_or(0))
            .collect();
        if let Some(biome) = fill.biome.as_deref().map(|b| self.biomes.id(b)) {
            for s in &mut chunk.sections {
                std::sync::Arc::make_mut(s).biomes = horizon_core::paletted::PalettedContainer::new(
                    crate::chunk::BIOME_STRATEGY,
                    u32::from(biome),
                );
            }
        }
        for (i, &at) in fill.blocks.iter().enumerate() {
            let state = states.get(usize::from(at)).copied().unwrap_or(0);
            if state == 0 {
                continue;
            }
            let (x, z) = ((i % 16) as i32, (i / 16 % 16) as i32);
            let y = fill.min_y + (i / 256) as i32;
            if (-64..320).contains(&y) {
                chunk.set(x, y, z, state);
            }
        }
        for s in &mut chunk.sections {
            std::sync::Arc::make_mut(s).compact();
        }
    }
}
impl ChunkGenerator for CodeTerrain {
    type Worker = ();
    fn worker(&self) {}
    fn proto(&self, _: &mut (), cx: i32, cz: i32) -> crate::streaming::Proto {
        let (reply, answer) = mpsc::channel();
        ASKS.lock().expect("chunk asks poisoned").push(ChunkAsk {
            dimension: self.dimension,
            x: cx,
            z: cz,
            reply,
        });
        let mut chunk = Chunk::empty(cx, cz);
        match answer.recv_timeout(ANSWER_TIME) {
            Ok(Some(fill)) => self.fill(&mut chunk, fill),
            Ok(None) => {}
            Err(_) => log::warn!(
                "Horizon: no mod answered for chunk {cx} {cz} of {}",
                self.dimension
            ),
        }
        Box::new(chunk)
    }
    fn finish(&self, proto: crate::streaming::Proto) -> Chunk {
        *proto.downcast::<Chunk>().expect("code terrain proto")
    }
}
