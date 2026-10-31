//! First Legacy on Horizon. Knowledge closes crafting (`world.set-locked-results`); a note of the First, read by the
//! research station, opens it. The station is a 3x3x3 cube round its core: casing, one input gate, one output gate, one
//! lava bay and a thermal converter against the bay. A first generator burns fuel into FE for its neighbours.
//!
//! Differences from the NeoForge mod, each a limit of the mod API today: a note is read by a right click in the air,
//! not in a tooltip; the output gate gives the note back once nothing else waits (no sneaking in `players`); notes don't
//! appear in vanilla loot (no loot modifiers), so `/firstlegacy note` hands them out; recipes aren't hidden from the
//! recipe book, only from the grid.
use horizon_mod_sdk::server::{
    Args, Argument, BlockPos, ItemStack, Player, Registration, ServerMod, StorageShape, entities,
    players, storages, world,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const WORK_TICKS: u32 = 200;
/// A station's job moves in steps of this many ticks.
const STEP: u32 = 20;
/// Lava burnt at a time, and ticks of work per mB.
const LAVA_MB: u32 = 50;
const TICKS_PER_MB: u32 = 6;
const BAY_CAPACITY: u32 = 4000;
const LAVA: &str = "minecraft:lava";
const FE_PER_TICK: u64 = 20;
const GENERATOR_CAPACITY: u64 = 20_000;
const TRANSFER: u64 = 200;

const CORE: &str = "firstlegacy:research_station";
const CASING: &str = "firstlegacy:station_casing";
const INPUT: &str = "firstlegacy:station_input_gate";
const OUTPUT: &str = "firstlegacy:station_output_gate";
const BAY: &str = "firstlegacy:station_lava_bay";
const CONVERTER: &str = "firstlegacy:thermal_converter";
const GENERATOR: &str = "firstlegacy:first_generator";
const NOTE: &str = "firstlegacy:note";

const KNOWLEDGE: [(&str, &str); 5] = [
    (
        "firstlegacy:first_infuser",
        include_str!("knowledge/first_infuser.json"),
    ),
    (
        "firstlegacy:first_processing",
        include_str!("knowledge/first_processing.json"),
    ),
    (
        "firstlegacy:first_energy",
        include_str!("knowledge/first_energy.json"),
    ),
    (
        "firstlegacy:first_logistics",
        include_str!("knowledge/first_logistics.json"),
    ),
    (
        "firstlegacy:first_forge",
        include_str!("knowledge/first_forge.json"),
    ),
];

#[derive(Deserialize)]
struct Knowledge {
    name: String,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    unlocks: Vec<String>,
    #[serde(default)]
    parts: Vec<Part>,
}
#[derive(Deserialize)]
struct Part {
    text: String,
    #[serde(default)]
    hint: String,
}

/// What this world has learned, shared by everyone on it.
#[derive(Default, Deserialize, Serialize)]
struct Learned {
    known: BTreeSet<String>,
}

/// The bytes on a note: which knowledge, which parts are written down.
#[derive(Deserialize, Serialize)]
struct NoteData {
    knowledge: String,
    parts: Vec<usize>,
}

/// The core's own bytes.
#[derive(Default, Deserialize, Serialize)]
struct Station {
    note: Option<String>,
    parts: Vec<usize>,
    reading: bool,
    progress: u32,
    user: String,
    heat: u32,
}

/// The generator's own bytes.
#[derive(Default, Deserialize, Serialize)]
struct Generator {
    burn: u32,
}

struct FirstLegacy {
    all: BTreeMap<String, Knowledge>,
    learned: Learned,
}

fn say(player: &str, text: &str) {
    let _ = players::tell(player, text);
}
fn data<T: for<'a> Deserialize<'a> + Default>(dim: &str, p: BlockPos) -> T {
    world::get_data(dim, p.x, p.y, p.z)
        .and_then(|d| serde_json::from_slice(&d).ok())
        .unwrap_or_default()
}
fn set_data<T: Serialize>(dim: &str, p: BlockPos, v: &T) {
    if let Ok(bytes) = serde_json::to_vec(v) {
        let _ = world::set_data(dim, p.x, p.y, p.z, &bytes);
    }
}
fn block(dim: &str, p: BlockPos) -> String {
    let state = world::get_block(dim, p.x, p.y, p.z).unwrap_or_default();
    state.split('[').next().unwrap_or_default().to_owned()
}
fn offset(p: BlockPos, x: i32, y: i32, z: i32) -> BlockPos {
    BlockPos {
        x: p.x + x,
        y: p.y + y,
        z: p.z + z,
    }
}
fn shell(core: BlockPos) -> Vec<BlockPos> {
    let mut cells = Vec::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if (x, y, z) != (0, 0, 0) {
                    cells.push(offset(core, x, y, z));
                }
            }
        }
    }
    cells
}
fn neighbours(p: BlockPos) -> [BlockPos; 6] {
    [
        offset(p, 1, 0, 0),
        offset(p, -1, 0, 0),
        offset(p, 0, 1, 0),
        offset(p, 0, -1, 0),
        offset(p, 0, 0, 1),
        offset(p, 0, 0, -1),
    ]
}
fn manhattan(a: BlockPos, b: BlockPos) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs() + (a.z - b.z).abs()
}

/// What is wrong with the cube, in words for the player; empty when it works.
fn problems(dim: &str, core: BlockPos) -> Vec<String> {
    let mut out = Vec::new();
    let count = |name: &str, what: &str, out: &mut Vec<String>| {
        let n = shell(core)
            .iter()
            .filter(|c| block(dim, **c) == name)
            .count();
        if n == 0 {
            out.push(format!("нет: {what}"));
        } else if n > 1 {
            out.push(format!("лишний: {what}"));
        }
    };
    count(INPUT, "шлюз ввода", &mut out);
    count(OUTPUT, "шлюз вывода", &mut out);
    count(BAY, "отсек лавы", &mut out);
    count(CONVERTER, "термопреобразователь", &mut out);
    let strangers = shell(core)
        .iter()
        .filter(|c| ![CASING, INPUT, OUTPUT, BAY, CONVERTER].contains(&block(dim, **c).as_str()))
        .count();
    if strangers > 0 {
        out.push(format!("в корпусе чужих блоков: {strangers}"));
    }
    let find = |name: &str| shell(core).into_iter().find(|c| block(dim, *c) == name);
    if let (Some(b), Some(c)) = (find(BAY), find(CONVERTER))
        && manhattan(b, c) != 1
    {
        out.push("термопреобразователь должен стоять вплотную к отсеку лавы".to_owned());
    }
    out
}

/// The core a gate belongs to.
fn core_of(dim: &str, gate: BlockPos) -> Option<BlockPos> {
    shell(gate).into_iter().find(|c| block(dim, *c) == CORE)
}

/// A lit core or a dark one, and the bay's level, as block states.
fn set_state(dim: &str, p: BlockPos, state: &str) {
    let _ = world::set_blocks(
        dim,
        &[world::BlockChange {
            x: p.x,
            y: p.y,
            z: p.z,
            state: state.to_owned(),
        }],
    );
}
fn set_bay_fill(dim: &str, bay: BlockPos) {
    let amount = storages::fluids(dim, bay.x, bay.y, bay.z)
        .and_then(|t| t.into_iter().next().flatten())
        .map_or(0, |f| f.amount);
    set_state(
        dim,
        bay,
        &format!("{BAY}[charges={}]", amount.div_ceil(1000)),
    );
}
fn bay_declared(dim: &str, bay: BlockPos) {
    if storages::shape(dim, bay.x, bay.y, bay.z).is_none() {
        let shape = StorageShape {
            slots: 0,
            energy_capacity: 0,
            tanks: vec![BAY_CAPACITY],
        };
        let _ = storages::declare(dim, bay.x, bay.y, bay.z, &shape);
    }
}

/// `STEP` ticks of heat from the converter, burning lava from the bay beside it.
fn draw_heat(dim: &str, core: BlockPos, station: &mut Station) -> bool {
    if station.heat < STEP {
        let Some(conv) = shell(core)
            .into_iter()
            .find(|c| block(dim, *c) == CONVERTER)
        else {
            return false;
        };
        for bay in neighbours(conv) {
            if block(dim, bay) != BAY {
                continue;
            }
            bay_declared(dim, bay);
            let got = storages::extract_fluid(dim, bay.x, bay.y, bay.z, Some(LAVA), LAVA_MB, false);
            if let Some(got) = got {
                station.heat += got.amount * TICKS_PER_MB;
                set_bay_fill(dim, bay);
                break;
            }
        }
        if station.heat < STEP {
            return false;
        }
    }
    station.heat -= STEP;
    true
}

impl FirstLegacy {
    fn save(&self) {
        if let Ok(bytes) = serde_json::to_vec(&self.learned) {
            let _ = world::set_saved("learned", &bytes);
        }
    }
    /// Closes what unknown knowledge unlocks: anything it unlocks and no other known one does.
    fn publish_locks(&self) {
        let open: BTreeSet<&String> = self
            .all
            .iter()
            .filter(|(id, _)| self.learned.known.contains(*id))
            .flat_map(|(_, k)| &k.unlocks)
            .collect();
        let locked: Vec<String> = self
            .all
            .iter()
            .filter(|(id, _)| !self.learned.known.contains(*id))
            .flat_map(|(_, k)| &k.unlocks)
            .filter(|u| !u.starts_with('#') && !open.contains(u))
            .cloned()
            .collect();
        let _ = world::set_locked_results(&locked);
    }
    fn can_learn(&self, k: &Knowledge) -> bool {
        k.requires.iter().all(|r| self.learned.known.contains(r))
    }
    /// A station finished reading the note: learned, or what it still builds on.
    fn read_note(&mut self, id: &str, user: &str) {
        let Some(k) = self.all.get(id) else {
            say(user, "Слова на записке стёрлись.");
            return;
        };
        if self.learned.known.contains(id) {
            say(user, "Записка прочитана. Это уже известно.");
            return;
        }
        if !self.can_learn(k) {
            let missing: Vec<&str> = k
                .requires
                .iter()
                .filter(|r| !self.learned.known.contains(*r))
                .map(|r| self.all.get(r).map_or(r.as_str(), |n| n.name.as_str()))
                .collect();
            say(
                user,
                &format!("Всё ясно, но сначала нужно узнать: {}.", missing.join(", ")),
            );
            return;
        }
        let name = k.name.clone();
        self.learned.known.insert(id.to_owned());
        self.save();
        self.publish_locks();
        let _ = players::broadcast(&format!("Открыто знание: «{name}»"));
    }
    fn note_stack(&self, id: &str, parts: usize) -> Option<ItemStack> {
        let k = self.all.get(id)?;
        let parts = if parts == 0 {
            k.parts.len()
        } else {
            parts.min(k.parts.len())
        };
        let bytes = serde_json::to_vec(&NoteData {
            knowledge: id.to_owned(),
            parts: (0..parts).collect(),
        })
        .ok()?;
        Some(ItemStack::new(NOTE, 1).with_data(bytes))
    }
    /// The input gate: a note goes in, if it is whole.
    fn input(&mut self, player: &Player, core: BlockPos) {
        let dim = player.dimension.as_str();
        let problems = problems(dim, core);
        if !problems.is_empty() {
            say(
                &player.id,
                &format!("Станция не собрана: {}.", problems.join("; ")),
            );
            return;
        }
        let mut st: Station = data(dim, core);
        if st.reading {
            say(
                &player.id,
                &format!("Станция занята: {}%.", st.progress * 100 / WORK_TICKS),
            );
            return;
        }
        let slot = players::selected_slot(&player.id).unwrap_or(0);
        let held = players::inventory(&player.id)
            .into_iter()
            .nth(slot as usize)
            .flatten();
        let Some(held) = held else {
            say(
                &player.id,
                match st.note {
                    Some(_) => "В станции записка. Заберите её через шлюз вывода.",
                    None => "Станция готова. Положите записку в шлюз ввода.",
                },
            );
            return;
        };
        if held.item != NOTE {
            say(
                &player.id,
                "Эта станция только читает полные чертежи. Предметы она изучать пока не умеет.",
            );
            return;
        }
        if st.note.is_some() {
            say(&player.id, "Здесь уже лежит записка.");
            return;
        }
        let note: Option<NoteData> = serde_json::from_slice(&held.data).ok();
        let Some(note) = note.filter(|n| self.all.contains_key(&n.knowledge)) else {
            say(&player.id, "Слова на записке стёрлись, читать нечего.");
            return;
        };
        let parts = self.all[&note.knowledge].parts.len();
        if (0..parts).any(|i| !note.parts.contains(&i)) {
            say(
                &player.id,
                "Это неполная записка: такой станции её не разобрать. Нужна станция уровнем выше.",
            );
            return;
        }
        let _ = players::take(&player.id, NOTE, 1);
        st.note = Some(note.knowledge);
        st.parts = note.parts;
        st.reading = true;
        st.progress = 0;
        st.user = player.id.clone();
        set_data(dim, core, &st);
        set_state(dim, core, &format!("{CORE}[lit=true]"));
        let _ = world::schedule_tick(dim, core.x, core.y, core.z, STEP);
        say(&player.id, "Станция читает записку.");
    }
    /// The output gate: the note back, once the station is done with it.
    fn output(&mut self, player: &Player, core: BlockPos) {
        let dim = player.dimension.as_str();
        let mut st: Station = data(dim, core);
        if st.reading {
            say(
                &player.id,
                &format!("Станция работает: {}%.", st.progress * 100 / WORK_TICKS),
            );
            return;
        }
        let Some(id) = st.note.take() else {
            say(&player.id, "Выход пуст.");
            return;
        };
        if let Ok(bytes) = serde_json::to_vec(&NoteData {
            knowledge: id,
            parts: std::mem::take(&mut st.parts),
        }) {
            let _ = players::give(&player.id, &ItemStack::new(NOTE, 1).with_data(bytes));
        }
        set_data(dim, core, &st);
    }
    fn station_tick(&mut self, dim: &str, core: BlockPos) {
        let mut st: Station = data(dim, core);
        if !st.reading {
            return;
        }
        let trouble = problems(dim, core);
        if trouble.is_empty() && draw_heat(dim, core, &mut st) {
            st.progress += STEP;
            let (x, y, z) = (
                f64::from(core.x) + 0.5,
                f64::from(core.y) + 0.5,
                f64::from(core.z) + 0.5,
            );
            let _ = entities::particle(
                dim,
                "minecraft:enchant",
                x,
                y + 1.5,
                z,
                0.5,
                0.5,
                0.5,
                0.0,
                8,
            );
        } else if st.progress % 100 == 0 {
            let text = if trouble.is_empty() {
                "Нет тепла: залейте лаву в отсек."
            } else {
                "Станция не собрана."
            };
            say(&st.user, text);
        }
        if st.progress >= WORK_TICKS {
            st.reading = false;
            st.progress = 0;
            let (id, user) = (st.note.clone().unwrap_or_default(), st.user.clone());
            set_data(dim, core, &st);
            set_state(dim, core, &format!("{CORE}[lit=false]"));
            self.read_note(&id, &user);
            return;
        }
        set_data(dim, core, &st);
        let _ = world::schedule_tick(dim, core.x, core.y, core.z, STEP);
    }
    /// A bucket of lava into the bay, or what it holds.
    fn bay_used(&mut self, player: &Player, bay: BlockPos, item: &str) {
        let dim = player.dimension.as_str();
        bay_declared(dim, bay);
        if item == "minecraft:lava_bucket" {
            let lava = storages::FluidStack {
                fluid: LAVA.to_owned(),
                amount: 1000,
            };
            if storages::insert_fluid(dim, bay.x, bay.y, bay.z, &lava, true) == 1000 {
                storages::insert_fluid(dim, bay.x, bay.y, bay.z, &lava, false);
                let _ = players::take(&player.id, "minecraft:lava_bucket", 1);
                let _ = players::give(&player.id, &ItemStack::new("minecraft:bucket", 1));
                set_bay_fill(dim, bay);
            }
            return;
        }
        let amount = storages::fluids(dim, bay.x, bay.y, bay.z)
            .and_then(|t| t.into_iter().next().flatten())
            .map_or(0, |f| f.amount);
        say(&player.id, &format!("Лава: {amount} из {BAY_CAPACITY} mB."));
    }
    /// Fuel into the generator, or its state.
    fn generator_used(&mut self, player: &Player, p: BlockPos, item: &str) {
        let dim = player.dimension.as_str();
        if storages::shape(dim, p.x, p.y, p.z).is_none() {
            let shape = StorageShape {
                slots: 0,
                energy_capacity: GENERATOR_CAPACITY,
                tanks: Vec::new(),
            };
            let _ = storages::declare(dim, p.x, p.y, p.z, &shape);
            let _ = world::schedule_tick(dim, p.x, p.y, p.z, 20);
        }
        let mut g: Generator = data(dim, p);
        let (stored, capacity) =
            storages::energy(dim, p.x, p.y, p.z).unwrap_or((0, GENERATOR_CAPACITY));
        if item == "minecraft:air" {
            let burn = if g.burn > 0 {
                format!(", горит ещё {} с.", g.burn.div_ceil(20))
            } else {
                ", топливо не горит.".to_owned()
            };
            say(
                &player.id,
                &format!("Первый генератор: {stored} / {capacity} FE{burn}"),
            );
            return;
        }
        if g.burn > 0 {
            say(
                &player.id,
                &format!("Ещё горит: {} с.", g.burn.div_ceil(20)),
            );
        } else if stored >= capacity {
            say(&player.id, "Буфер полон — сначала потрать энергию.");
        } else if let Some(time) = burn_time(item) {
            let _ = players::take(&player.id, item, 1);
            g.burn = time;
            set_data(dim, p, &g);
            say(
                &player.id,
                &format!("Затоплен: хватит на {} с.", time.div_ceil(20)),
            );
        } else {
            say(
                &player.id,
                "Это не топливо. Нужен уголь, дерево или другое горючее.",
            );
        }
    }
    fn generator_tick(&mut self, dim: &str, p: BlockPos) {
        let mut g: Generator = data(dim, p);
        if g.burn > 0 {
            let made = storages::insert_energy(dim, p.x, p.y, p.z, FE_PER_TICK * 20, false);
            if made > 0 {
                g.burn = g.burn.saturating_sub(20);
                set_data(dim, p, &g);
            }
        }
        for n in neighbours(p) {
            let have = storages::extract_energy(dim, p.x, p.y, p.z, TRANSFER * 20, true);
            if have == 0 {
                break;
            }
            let sent = storages::insert_energy(dim, n.x, n.y, n.z, have, false);
            storages::extract_energy(dim, p.x, p.y, p.z, sent, false);
        }
        let _ = world::schedule_tick(dim, p.x, p.y, p.z, 20);
    }
}

/// Vanilla's fuel values for what a first generator takes.
fn burn_time(item: &str) -> Option<u32> {
    let id = item.strip_prefix("minecraft:")?;
    Some(match id {
        "coal" | "charcoal" => 1600,
        "coal_block" => 16000,
        "blaze_rod" => 2400,
        "lava_bucket" => 20000,
        "stick" => 100,
        _ if id.ends_with("_planks") || id.ends_with("_log") || id.ends_with("_wood") => 300,
        _ => return None,
    })
}

impl ServerMod for FirstLegacy {
    fn init() -> Result<(Self, Registration), String> {
        let mut all = BTreeMap::new();
        for (id, json) in KNOWLEDGE {
            let k: Knowledge = serde_json::from_str(json).map_err(|e| format!("{id}: {e}"))?;
            all.insert(id.to_owned(), k);
        }
        let id = || Argument::text("id");
        Ok((
            FirstLegacy {
                all,
                learned: Learned::default(),
            },
            Registration::new()
                .on_blocks()
                .on_tick()
                .command_with("firstlegacy", [Argument::word("action"), id().optional()]),
        ))
    }

    /// A world is lent only now: what it has learned, and what stays closed.
    fn tick(&mut self, tick: u64) {
        if tick % 100 == 0 {
            self.learned = world::get_saved("learned")
                .and_then(|d| serde_json::from_slice(&d).ok())
                .unwrap_or_default();
            self.publish_locks();
        }
    }

    fn use_block(&mut self, player: &Player, pos: BlockPos, state: &str, item: &str) -> bool {
        let dim = player.dimension.as_str();
        match state.split('[').next().unwrap_or_default() {
            CORE => {
                let trouble = problems(dim, pos);
                let text = if trouble.is_empty() {
                    let st: Station = data(dim, pos);
                    if st.reading {
                        format!("Читает записку: {}%.", st.progress * 100 / WORK_TICKS)
                    } else {
                        "Станция готова. Положите записку в шлюз ввода.".to_owned()
                    }
                } else {
                    format!("Станция не собрана: {}.", trouble.join("; "))
                };
                say(&player.id, &text);
                true
            }
            gate @ (INPUT | OUTPUT) => {
                match core_of(dim, pos) {
                    None => say(
                        &player.id,
                        "Шлюзу не к чему подключиться: рядом нет ядра станции.",
                    ),
                    Some(core) if gate == INPUT => self.input(player, core),
                    Some(core) => self.output(player, core),
                }
                true
            }
            BAY => {
                self.bay_used(player, pos, item);
                true
            }
            GENERATOR => {
                self.generator_used(player, pos, item);
                true
            }
            _ => false,
        }
    }

    fn block_tick(&mut self, dimension: &str, pos: BlockPos, state: &str) {
        match state.split('[').next().unwrap_or_default() {
            CORE => self.station_tick(dimension, pos),
            GENERATOR => self.generator_tick(dimension, pos),
            _ => {}
        }
    }

    /// A note read in the hand: its name, what is written, and hints at the rest.
    fn use_item(&mut self, player: &Player, stack: &ItemStack, _off_hand: bool) -> bool {
        if stack.item != NOTE {
            return false;
        }
        let note: Option<NoteData> = serde_json::from_slice(&stack.data).ok();
        let Some((note, k)) = note.and_then(|n| {
            let k = self.all.get(&n.knowledge)?;
            Some((n, k))
        }) else {
            say(&player.id, "Слова стёрлись.");
            return true;
        };
        say(&player.id, &k.name);
        for (i, part) in k.parts.iter().enumerate() {
            let line = if note.parts.contains(&i) {
                format!("• {}", part.text)
            } else {
                format!("• ??? {}", part.hint)
            };
            say(&player.id, &line);
        }
        true
    }

    fn command(
        &mut self,
        player: &Player,
        _name: &str,
        args: &Args,
    ) -> Result<Option<String>, String> {
        let id = args.string("id").unwrap_or_default().to_owned();
        let exists = |all: &BTreeMap<String, Knowledge>| {
            all.contains_key(&id)
                .then_some(())
                .ok_or_else(|| format!("Нет такого знания: {id}"))
        };
        match args.string("action").unwrap_or_default() {
            "learn" => {
                exists(&self.all)?;
                self.learned.known.insert(id.clone());
                self.save();
                self.publish_locks();
                Ok(Some(format!("Узнали: {}", self.all[&id].name)))
            }
            "forget" => {
                exists(&self.all)?;
                self.learned.known.remove(&id);
                self.save();
                self.publish_locks();
                Ok(Some(format!("Забыли: {}", self.all[&id].name)))
            }
            "note" => {
                exists(&self.all)?;
                let note = self.note_stack(&id, 0).ok_or("Записки нет")?;
                players::give(&player.id, &note)?;
                Ok(None)
            }
            "list" => {
                let lines: Vec<String> = self
                    .all
                    .iter()
                    .map(|(id, k)| {
                        let state = if self.learned.known.contains(id) {
                            "[узнано]"
                        } else {
                            "[закрыто]"
                        };
                        format!("{state} {id} «{}»", k.name)
                    })
                    .collect();
                Ok(Some(lines.join("\n")))
            }
            _ => Err("learn <id>, forget <id>, note <id>, list".to_owned()),
        }
    }
}
horizon_mod_sdk::export_server_mod!(FirstLegacy);
