//! API level 22: what a mod would otherwise write itself, or cross the
//! boundary thousands of times for. `shapes`, `patterns` (multiblocks),
//! `noise`, `timers`, `crowd`, `fx`, `stock`, `admin`. None needs the game's
//! help: they read through the lent [`WorldView`](super::WorldView) and
//! write as `world`, `storages` and the console do.
use super::server_bindings::horizon::mods::{
    admin, crowd, entities, fx, noise, patterns, players, shapes, stock, storages, timers,
    world as mod_world,
};
use super::{
    BLOCKS_PER_CALL, SCAN_CELLS, State, WorldView, block_id, box_cells, check_game_id,
    is_air_block, quoted, state_is,
};
use std::collections::{BTreeMap, HashSet, VecDeque};

type Pos = (i32, i32, i32);
type Change = mod_world::BlockChange;

/// The widest radius a shape reads or builds: its box is about [`SCAN_CELLS`].
const RADIUS: u32 = 20;
/// How long a ray or a `clear-between` goes.
const REACH: f64 = 256.0;
const CLOCKS_PER_MOD: usize = 4096;
const PATTERNS_PER_MOD: usize = 256;
/// Blocks a `find-in-box` looks at, however many anchors the box holds.
const PATTERN_READS: usize = 1 << 20;
const FX_POINTS: usize = 256;

/// A mod's `timers`, in server ticks ([`BlockData::now`](super::BlockData)).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ModClocks {
    /// Name → when due, and every how many ticks after (0: once).
    timers: BTreeMap<String, (u64, u64)>,
    /// `owner\0key` → when it ends.
    cooldowns: BTreeMap<String, u64>,
}

impl ModClocks {
    /// A world just opened on another clock: everything keeps its distance.
    pub(super) fn shift(&mut self, by: i128) {
        let moved = |at: &mut u64| *at = (i128::from(*at) + by).max(0) as u64;
        self.timers.values_mut().for_each(|t| moved(&mut t.0));
        self.cooldowns.values_mut().for_each(moved);
    }
    fn full(&self) -> bool {
        self.timers.len() + self.cooldowns.len() >= CLOCKS_PER_MOD
    }
}

/// A `patterns.define`d structure.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pattern {
    /// From the anchor, facing north, and the symbol there.
    cells: Vec<(Pos, char)>,
    keys: BTreeMap<char, Vec<String>>,
    size: (u32, u32, u32),
}

impl Pattern {
    fn takes(&self, symbol: char, state: &str) -> bool {
        self.keys
            .get(&symbol)
            .is_some_and(|k| k.iter().any(|b| state_is(state, b)))
    }
    /// Complete with the anchor at `at`; each block read costs `budget`.
    fn complete(
        &self,
        view: &dyn WorldView,
        dimension: &str,
        at: Pos,
        facing: patterns::Facing,
        budget: &mut usize,
    ) -> bool {
        self.cells.iter().all(|&(cell, symbol)| {
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            let (x, y, z) = placed(at, cell, facing);
            view.block(dimension, x, y, z)
                .is_some_and(|s| self.takes(symbol, &s))
        })
    }
}

const FACINGS: [patterns::Facing; 4] = [
    patterns::Facing::North,
    patterns::Facing::East,
    patterns::Facing::South,
    patterns::Facing::West,
];

fn placed(at: Pos, (dx, dy, dz): Pos, facing: patterns::Facing) -> Pos {
    let (dx, dz) = match facing {
        patterns::Facing::North => (dx, dz),
        patterns::Facing::East => (-dz, dx),
        patterns::Facing::South => (-dx, -dz),
        patterns::Facing::West => (dz, -dx),
    };
    (at.0 + dx, at.1 + dy, at.2 + dz)
}

/// The cells of a ball, or of its one-block shell.
fn ball(center: Pos, radius: u32, hollow: bool) -> Result<Vec<Pos>, String> {
    check_radius(radius)?;
    let r = radius as i32;
    let outer = (f64::from(radius) + 0.25).powi(2);
    let inner = (f64::from(radius) - 0.75).max(0.0).powi(2);
    let mut cells = Vec::new();
    for x in -r..=r {
        for y in -r..=r {
            for z in -r..=r {
                let d = f64::from(x * x + y * y + z * z);
                if d <= outer && (!hollow || d >= inner) {
                    cells.push((center.0 + x, center.1 + y, center.2 + z));
                }
            }
        }
    }
    Ok(cells)
}
/// The cells of a flat circle.
fn round(center: Pos, radius: u32) -> Result<Vec<Pos>, String> {
    check_radius(radius)?;
    let r = radius as i32;
    let outer = (f64::from(radius) + 0.25).powi(2);
    let mut cells = Vec::new();
    for x in -r..=r {
        for z in -r..=r {
            if f64::from(x * x + z * z) <= outer {
                cells.push((center.0 + x, center.1, center.2 + z));
            }
        }
    }
    Ok(cells)
}
fn check_radius(radius: u32) -> Result<(), String> {
    if radius > RADIUS {
        return Err(format!("a radius of at most {RADIUS}"));
    }
    Ok(())
}
fn corner(a: Pos, b: Pos, pick: fn(i32, i32) -> i32) -> Pos {
    (pick(a.0, b.0), pick(a.1, b.1), pick(a.2, b.2))
}

/// A state's properties turned; see `shapes.rotated-state`.
fn turned_state(state: &str, turns: u8, mirror: bool) -> String {
    let Some((id, rest)) = state.split_once('[') else {
        return state.to_owned();
    };
    let Some(props) = rest.strip_suffix(']') else {
        return state.to_owned();
    };
    const SIDES: [&str; 4] = ["north", "east", "south", "west"];
    let turn = |side: &str| {
        let Some(i) = SIDES.iter().position(|s| *s == side) else {
            return side.to_owned();
        };
        let i = if mirror && i % 2 == 1 { (i + 2) % 4 } else { i };
        SIDES[(i + usize::from(turns % 4)) % 4].to_owned()
    };
    let props: Vec<String> = props
        .split(',')
        .map(|p| {
            let Some((name, value)) = p.split_once('=') else {
                return p.to_owned();
            };
            let value = match name {
                "facing" => turn(value),
                "axis" if turns % 2 == 1 => match value {
                    "x" => "z".to_owned(),
                    "z" => "x".to_owned(),
                    v => v.to_owned(),
                },
                "rotation" => value.parse::<u32>().map_or_else(
                    |_| value.to_owned(),
                    |r| {
                        let r = if mirror { (16 - r) % 16 } else { r };
                        ((r + 4 * u32::from(turns % 4)) % 16).to_string()
                    },
                ),
                _ => value.to_owned(),
            };
            let name = if SIDES.contains(&name) {
                turn(name)
            } else {
                name.to_owned()
            };
            format!("{name}={value}")
        })
        .collect();
    format!("{id}[{}]", props.join(","))
}

/// The cells a ray crosses, from its start, and the distance to each.
pub(super) fn ray_cells(from: [f64; 3], dir: [f64; 3], reach: f64) -> Vec<(Pos, f64)> {
    let len = dir.iter().map(|d| d * d).sum::<f64>().sqrt();
    if !len.is_finite() || len == 0.0 || !from.iter().all(|c| c.abs() < 3.0e7) {
        return Vec::new();
    }
    let dir = dir.map(|d| d / len);
    let reach = reach.clamp(0.0, REACH);
    let mut cell = from.map(|c| c.floor() as i32);
    let mut step = [0; 3];
    let mut next = [f64::INFINITY; 3];
    let mut delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if dir[i] != 0.0 {
            step[i] = if dir[i] > 0.0 { 1 } else { -1 };
            delta[i] = (1.0 / dir[i]).abs();
            let edge = f64::from(cell[i]) + if dir[i] > 0.0 { 1.0 } else { 0.0 };
            next[i] = (edge - from[i]) / dir[i];
        }
    }
    let mut out = vec![((cell[0], cell[1], cell[2]), 0.0)];
    loop {
        let i = (0..3).fold(0, |b, i| if next[i] < next[b] { i } else { b });
        if next[i] > reach {
            return out;
        }
        cell[i] += step[i];
        out.push(((cell[0], cell[1], cell[2]), next[i]));
        next[i] += delta[i];
    }
}

/// Adds up a list of (item, count), so an item named twice counts twice.
fn wanted(items: Vec<(String, u32)>) -> BTreeMap<String, u32> {
    let mut all = BTreeMap::new();
    for (item, count) in items {
        let n: &mut u32 = all.entry(item).or_default();
        *n = n.saturating_add(count);
    }
    all
}

impl State {
    fn reading<R>(&self, f: impl FnOnce(&dyn WorldView) -> R) -> Option<R> {
        let view = self.world.lock().ok()?;
        Some(f(&**view.as_ref()?))
    }
    fn place(&mut self, dimension: String, cells: Vec<Pos>, state: String) -> Result<(), String> {
        if cells.len() > BLOCKS_PER_CALL {
            return Err(format!(
                "{} blocks: at most {BLOCKS_PER_CALL} per call",
                cells.len()
            ));
        }
        let blocks = cells
            .into_iter()
            .map(|(x, y, z)| Change {
                x,
                y,
                z,
                state: state.clone(),
            })
            .collect();
        mod_world::Host::set_blocks(self, dimension, blocks)
    }
    fn replacing(
        &mut self,
        dimension: String,
        cells: Vec<Pos>,
        block: &str,
        state: String,
    ) -> Result<u32, String> {
        let hits: Vec<Pos> = self
            .reading(|v| {
                cells
                    .into_iter()
                    .filter(|&(x, y, z)| {
                        v.block(&dimension, x, y, z)
                            .is_some_and(|s| state_is(&s, block))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let n = hits.len() as u32;
        self.place(dimension, hits, state)?;
        Ok(n)
    }
    fn flooding(
        &self,
        dimension: &str,
        start: Pos,
        blocks: &[String],
        limit: u32,
        steps: &[Pos],
    ) -> Vec<Pos> {
        let limit = limit.min(BLOCKS_PER_CALL as u32) as usize;
        self.reading(|v| {
            let member = |(x, y, z): Pos| {
                v.block(dimension, x, y, z)
                    .is_some_and(|s| blocks.iter().any(|b| state_is(&s, b)))
            };
            let mut out = Vec::new();
            if limit == 0 || !member(start) {
                return out;
            }
            let mut seen = HashSet::from([start]);
            let mut queue = VecDeque::from([start]);
            while let Some(at) = queue.pop_front() {
                out.push(at);
                if out.len() + queue.len() >= limit {
                    out.extend(queue);
                    break;
                }
                for s in steps {
                    let n = (at.0 + s.0, at.1 + s.1, at.2 + s.2);
                    if seen.insert(n) && member(n) {
                        queue.push_back(n);
                    }
                }
            }
            out.truncate(limit);
            out
        })
        .unwrap_or_default()
    }
    fn pattern(&self, name: &str) -> Option<Pattern> {
        let all = self.block_data.lock().ok()?;
        all.patterns.get(&self.id)?.get(name).cloned()
    }
    fn clocks<R>(&self, f: impl FnOnce(&mut ModClocks, u64) -> R) -> Option<R> {
        let mut all = self.block_data.lock().ok()?;
        let now = all.now;
        Some(f(all.clocks.entry(self.id.clone()).or_default(), now))
    }
    fn set_timer(&mut self, name: String, ticks: u64, period: u64) -> Result<(), String> {
        if name.len() > 256 {
            return Err("a name of at most 256 bytes".into());
        }
        self.clocks(|c, now| {
            if !c.timers.contains_key(&name) && c.full() {
                return Err(format!("at most {CLOCKS_PER_MOD} timers and cooldowns"));
            }
            c.timers.insert(name, (now.saturating_add(ticks), period));
            Ok(())
        })
        .unwrap_or_else(|| Err("timers unavailable".into()))
    }
    fn near_players(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
    ) -> Vec<players::Player> {
        players::Host::near(self, dimension, x, y, z, radius)
    }
    /// The player's inventory holds this many of each item.
    fn has_all(&mut self, player: &str, items: &BTreeMap<String, u32>) -> bool {
        let slots = self.player_inventory(player.to_owned());
        items.iter().all(|(item, count)| {
            let held: u64 = slots
                .iter()
                .flatten()
                .filter(|s| &s.item == item)
                .map(|s| u64::from(s.count))
                .sum();
            held >= u64::from(*count)
        })
    }
    fn fx_points(
        &mut self,
        dimension: String,
        particle: String,
        points: impl Iterator<Item = [f64; 3]>,
    ) -> Result<(), String> {
        for [x, y, z] in points {
            if ![x, y, z].iter().all(|c| c.abs() < 3.0e7) {
                return Err("a point out of the world".into());
            }
            entities::Host::particle(
                self,
                dimension.clone(),
                particle.clone(),
                x,
                y,
                z,
                0.0,
                0.0,
                0.0,
                0.0,
                1,
            )?;
        }
        Ok(())
    }
    fn stock_has(
        &mut self,
        dimension: &str,
        (x, y, z): Pos,
        items: &BTreeMap<String, u32>,
    ) -> bool {
        items.iter().all(|(item, count)| {
            storages::Host::count_item(self, dimension.to_owned(), x, y, z, item.clone()) >= *count
        })
    }
}

impl shapes::Host for State {
    fn line(&mut self, dimension: String, a: Pos, b: Pos, state: String) -> Result<(), String> {
        let steps = [
            i64::from(b.0) - i64::from(a.0),
            i64::from(b.1) - i64::from(a.1),
            i64::from(b.2) - i64::from(a.2),
        ]
        .iter()
        .map(|d| d.abs())
        .max()
        .unwrap_or(0);
        if steps >= BLOCKS_PER_CALL as i64 {
            return Err(format!(
                "{steps} blocks: at most {BLOCKS_PER_CALL} per call"
            ));
        }
        let along = |p: i32, q: i32, i: i64| {
            let t = if steps == 0 {
                0.0
            } else {
                i as f64 / steps as f64
            };
            (f64::from(p) + (f64::from(q) - f64::from(p)) * t).round() as i32
        };
        let cells = (0..=steps)
            .map(|i| (along(a.0, b.0, i), along(a.1, b.1, i), along(a.2, b.2, i)))
            .collect();
        self.place(dimension, cells, state)
    }
    fn sphere(
        &mut self,
        dimension: String,
        center: Pos,
        radius: u32,
        state: String,
    ) -> Result<(), String> {
        let cells = ball(center, radius, false)?;
        self.place(dimension, cells, state)
    }
    fn hollow_sphere(
        &mut self,
        dimension: String,
        center: Pos,
        radius: u32,
        state: String,
    ) -> Result<(), String> {
        let cells = ball(center, radius, true)?;
        self.place(dimension, cells, state)
    }
    fn cylinder(
        &mut self,
        dimension: String,
        base: Pos,
        radius: u32,
        height: u32,
        state: String,
    ) -> Result<(), String> {
        let layer = round(base, radius)?;
        if (layer.len() as u64) * u64::from(height) > BLOCKS_PER_CALL as u64 {
            return Err(format!("at most {BLOCKS_PER_CALL} blocks per call"));
        }
        let cells = (0..height as i32)
            .flat_map(|h| layer.iter().map(move |c| (c.0, c.1 + h, c.2)))
            .collect();
        self.place(dimension, cells, state)
    }
    fn disc(
        &mut self,
        dimension: String,
        center: Pos,
        radius: u32,
        state: String,
    ) -> Result<(), String> {
        let cells = round(center, radius)?;
        self.place(dimension, cells, state)
    }
    fn hollow_box(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        state: String,
    ) -> Result<(), String> {
        let (lo, hi) = (corner(a, b, i32::min), corner(a, b, i32::max));
        let cells = box_cells(a, b, SCAN_CELLS)?
            .filter(|c| {
                c.0 == lo.0
                    || c.0 == hi.0
                    || c.1 == lo.1
                    || c.1 == hi.1
                    || c.2 == lo.2
                    || c.2 == hi.2
            })
            .collect();
        self.place(dimension, cells, state)
    }
    fn walls(&mut self, dimension: String, a: Pos, b: Pos, state: String) -> Result<(), String> {
        let (lo, hi) = (corner(a, b, i32::min), corner(a, b, i32::max));
        let cells = box_cells(a, b, SCAN_CELLS)?
            .filter(|c| c.0 == lo.0 || c.0 == hi.0 || c.2 == lo.2 || c.2 == hi.2)
            .collect();
        self.place(dimension, cells, state)
    }
    fn replace_in_box(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        block: String,
        state: String,
    ) -> Result<u32, String> {
        let cells = box_cells(a, b, SCAN_CELLS)?.collect();
        self.replacing(dimension, cells, &block, state)
    }
    fn replace_in_sphere(
        &mut self,
        dimension: String,
        center: Pos,
        radius: u32,
        block: String,
        state: String,
    ) -> Result<u32, String> {
        let cells = ball(center, radius, false)?;
        self.replacing(dimension, cells, &block, state)
    }
    fn stamp(
        &mut self,
        dimension: String,
        origin: Pos,
        turns: u8,
        mirror: bool,
        cells: Vec<Change>,
    ) -> Result<(), String> {
        let blocks = cells
            .into_iter()
            .map(|c| {
                let x = if mirror { c.x.saturating_neg() } else { c.x };
                let z = c.z;
                let (x, z) = match turns % 4 {
                    0 => (x, z),
                    1 => (z.saturating_neg(), x),
                    2 => (x.saturating_neg(), z.saturating_neg()),
                    _ => (z, x.saturating_neg()),
                };
                Change {
                    x: origin.0.saturating_add(x),
                    y: origin.1.saturating_add(c.y),
                    z: origin.2.saturating_add(z),
                    state: turned_state(&c.state, turns, mirror),
                }
            })
            .collect();
        mod_world::Host::set_blocks(self, dimension, blocks)
    }
    fn rotated_state(&mut self, state: String, turns: u8, mirror: bool) -> String {
        turned_state(&state, turns, mirror)
    }
    fn copy_box(&mut self, dimension: String, a: Pos, b: Pos) -> Result<Vec<Change>, String> {
        let lo = corner(a, b, i32::min);
        let cells = box_cells(a, b, SCAN_CELLS)?;
        let mut out = Vec::new();
        self.reading(|v| {
            for (x, y, z) in cells {
                let Some(state) = v.block(&dimension, x, y, z) else {
                    continue;
                };
                if is_air_block(block_id(&state)) {
                    continue;
                }
                if out.len() == BLOCKS_PER_CALL {
                    return Err(format!("more than {BLOCKS_PER_CALL} blocks"));
                }
                out.push(Change {
                    x: x - lo.0,
                    y: y - lo.1,
                    z: z - lo.2,
                    state,
                });
            }
            Ok(())
        })
        .unwrap_or(Ok(()))?;
        Ok(out)
    }
    fn flood(
        &mut self,
        dimension: String,
        start: Pos,
        blocks: Vec<String>,
        limit: u32,
    ) -> Vec<Pos> {
        const FACES: [Pos; 6] = [
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
            (-1, 0, 0),
            (1, 0, 0),
        ];
        self.flooding(&dimension, start, &blocks, limit, &FACES)
    }
    fn flood_diagonal(
        &mut self,
        dimension: String,
        start: Pos,
        blocks: Vec<String>,
        limit: u32,
    ) -> Vec<Pos> {
        let mut around = Vec::with_capacity(26);
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    if (x, y, z) != (0, 0, 0) {
                        around.push((x, y, z));
                    }
                }
            }
        }
        self.flooding(&dimension, start, &blocks, limit, &around)
    }
    fn raycast(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        dx: f64,
        dy: f64,
        dz: f64,
        reach: f64,
    ) -> Option<shapes::RayHit> {
        let cells = ray_cells([x, y, z], [dx, dy, dz], reach);
        self.reading(|v| {
            let mut before = cells.first()?.0;
            for &(pos, distance) in &cells {
                let state = v.block(&dimension, pos.0, pos.1, pos.2)?;
                if !is_air_block(block_id(&state)) {
                    return Some(shapes::RayHit {
                        pos,
                        before,
                        state,
                        distance,
                    });
                }
                before = pos;
            }
            None
        })?
    }
    fn clear_between(
        &mut self,
        dimension: String,
        ax: f64,
        ay: f64,
        az: f64,
        bx: f64,
        by: f64,
        bz: f64,
    ) -> bool {
        let dir = [bx - ax, by - ay, bz - az];
        let reach = dir.iter().map(|d| d * d).sum::<f64>().sqrt();
        if reach > REACH {
            return false;
        }
        let end = (bx.floor() as i32, by.floor() as i32, bz.floor() as i32);
        let cells = ray_cells([ax, ay, az], dir, reach);
        self.reading(|v| {
            cells.iter().skip(1).filter(|c| c.0 != end).all(|&(p, _)| {
                v.block(&dimension, p.0, p.1, p.2)
                    .is_some_and(|s| is_air_block(block_id(&s)))
            })
        })
        .unwrap_or(false)
    }
    fn nearest_block(
        &mut self,
        dimension: String,
        center: Pos,
        radius: u32,
        block: String,
    ) -> Option<Pos> {
        let mut cells = ball(center, radius, false).ok()?;
        let d2 = |c: &Pos| {
            let (x, y, z) = (c.0 - center.0, c.1 - center.1, c.2 - center.2);
            x * x + y * y + z * z
        };
        cells.sort_by_key(d2);
        self.reading(|v| {
            cells.into_iter().find(|&(x, y, z)| {
                v.block(&dimension, x, y, z)
                    .is_some_and(|s| state_is(&s, &block))
            })
        })?
    }
    fn surface_below(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<i32> {
        self.reading(|v| {
            (y.saturating_sub(4095)..=y).rev().find(|&h| {
                v.block(&dimension, x, h, z)
                    .is_some_and(|s| !is_air_block(block_id(&s)))
            })
        })?
    }
    fn ceiling_above(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<i32> {
        self.reading(|v| {
            (y..=y.saturating_add(4095)).find(|&h| {
                v.block(&dimension, x, h, z)
                    .is_some_and(|s| !is_air_block(block_id(&s)))
            })
        })?
    }
    fn column(
        &mut self,
        dimension: String,
        x: i32,
        z: i32,
        y1: i32,
        y2: i32,
    ) -> Option<Vec<String>> {
        let (lo, hi) = (y1.min(y2), y1.max(y2));
        if i64::from(hi) - i64::from(lo) >= BLOCKS_PER_CALL as i64 {
            return None;
        }
        self.reading(|v| (lo..=hi).map(|y| v.block(&dimension, x, y, z)).collect())?
    }
    fn neighbors(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Vec<Option<String>> {
        let around = [
            (0, -1, 0),
            (0, 1, 0),
            (0, 0, -1),
            (0, 0, 1),
            (-1, 0, 0),
            (1, 0, 0),
        ];
        self.reading(|v| {
            around
                .iter()
                .map(|s| v.block(&dimension, x + s.0, y + s.1, z + s.2))
                .collect()
        })
        .unwrap_or_else(|| vec![None; 6])
    }
    fn histogram(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
    ) -> Result<Vec<(String, u32)>, String> {
        let cells = box_cells(a, b, SCAN_CELLS)?;
        let counts = self.reading(|v| {
            let mut counts = BTreeMap::<String, u32>::new();
            for (x, y, z) in cells {
                if let Some(state) = v.block(&dimension, x, y, z) {
                    *counts.entry(block_id(&state).to_owned()).or_default() += 1;
                }
            }
            counts
        });
        let mut out: Vec<_> = counts.unwrap_or_default().into_iter().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(out)
    }
    fn is_box_clear(&mut self, dimension: String, a: Pos, b: Pos) -> Result<bool, String> {
        let mut cells = box_cells(a, b, SCAN_CELLS)?;
        Ok(self
            .reading(|v| {
                cells.all(|(x, y, z)| {
                    v.block(&dimension, x, y, z)
                        .is_some_and(|s| is_air_block(block_id(&s)))
                })
            })
            .unwrap_or(false))
    }
    fn find_any(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        blocks: Vec<String>,
        limit: u32,
    ) -> Result<Vec<Pos>, String> {
        let cells = box_cells(a, b, SCAN_CELLS)?;
        Ok(self
            .reading(|v| {
                cells
                    .filter(|&(x, y, z)| {
                        v.block(&dimension, x, y, z)
                            .is_some_and(|s| blocks.iter().any(|b| state_is(&s, b)))
                    })
                    .take(limit.min(BLOCKS_PER_CALL as u32) as usize)
                    .collect()
            })
            .unwrap_or_default())
    }
}

impl patterns::Host for State {
    fn define(
        &mut self,
        name: String,
        layers: Vec<Vec<String>>,
        keys: Vec<patterns::PatternKey>,
    ) -> Result<(), String> {
        let mut keyed = BTreeMap::new();
        for key in keys {
            let mut symbol = key.symbol.chars();
            let (Some(c), None) = (symbol.next(), symbol.next()) else {
                return Err(format!("{:?}: a symbol is one character", key.symbol));
            };
            if key.blocks.is_empty() || key.blocks.len() > 64 {
                return Err(format!("{c}: 1 to 64 blocks"));
            }
            keyed.insert(c, key.blocks);
        }
        let mut written = Vec::new();
        let mut anchor = None;
        let (mut width, mut depth) = (0, 0);
        for (y, layer) in layers.iter().enumerate() {
            depth = depth.max(layer.len());
            for (z, row) in layer.iter().enumerate() {
                width = width.max(row.chars().count());
                for (x, c) in row.chars().enumerate() {
                    if c == ' ' {
                        continue;
                    }
                    if !keyed.contains_key(&c) {
                        return Err(format!("{c}: no key"));
                    }
                    if written.len() == BLOCKS_PER_CALL {
                        return Err(format!("at most {BLOCKS_PER_CALL} cells"));
                    }
                    let at = (x as i32, y as i32, z as i32);
                    if c == '@' && anchor.replace(at).is_some() {
                        return Err("one anchor @ only".into());
                    }
                    written.push((at, c));
                }
            }
        }
        let anchor = anchor.ok_or("no anchor @")?;
        let cells = written
            .into_iter()
            .map(|(c, s)| ((c.0 - anchor.0, c.1 - anchor.1, c.2 - anchor.2), s))
            .collect();
        let mut all = self
            .block_data
            .lock()
            .map_err(|_| "patterns unavailable".to_string())?;
        let mine = all.patterns.entry(self.id.clone()).or_default();
        if !mine.contains_key(&name) && mine.len() >= PATTERNS_PER_MOD {
            return Err(format!("at most {PATTERNS_PER_MOD} patterns"));
        }
        mine.insert(
            name,
            Pattern {
                cells,
                keys: keyed,
                size: (width as u32, layers.len() as u32, depth as u32),
            },
        );
        Ok(())
    }
    fn remove(&mut self, name: String) -> bool {
        self.block_data.lock().is_ok_and(|mut all| {
            all.patterns
                .get_mut(&self.id)
                .is_some_and(|m| m.remove(&name).is_some())
        })
    }
    fn names(&mut self) -> Vec<String> {
        self.block_data
            .lock()
            .ok()
            .and_then(|all| Some(all.patterns.get(&self.id)?.keys().cloned().collect()))
            .unwrap_or_default()
    }
    fn size(&mut self, name: String) -> Option<(u32, u32, u32)> {
        Some(self.pattern(&name)?.size)
    }
    fn match_at(&mut self, dimension: String, pos: Pos, name: String) -> Option<patterns::Facing> {
        let pattern = self.pattern(&name)?;
        self.reading(|v| {
            FACINGS
                .into_iter()
                .find(|f| pattern.complete(v, &dimension, pos, *f, &mut { PATTERN_READS }))
        })?
    }
    fn match_facing(
        &mut self,
        dimension: String,
        pos: Pos,
        name: String,
        facing: patterns::Facing,
    ) -> bool {
        let Some(pattern) = self.pattern(&name) else {
            return false;
        };
        self.reading(|v| pattern.complete(v, &dimension, pos, facing, &mut { PATTERN_READS }))
            .unwrap_or(false)
    }
    fn find_in_box(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        name: String,
        limit: u32,
    ) -> Result<Vec<(Pos, patterns::Facing)>, String> {
        let pattern = self
            .pattern(&name)
            .ok_or_else(|| format!("no pattern {name}"))?;
        let cells = box_cells(a, b, SCAN_CELLS)?;
        let limit = limit.min(BLOCKS_PER_CALL as u32) as usize;
        Ok(self
            .reading(|v| {
                let mut budget = PATTERN_READS;
                let mut out = Vec::new();
                for at in cells {
                    if out.len() >= limit || budget == 0 {
                        break;
                    }
                    let anchor = v.block(&dimension, at.0, at.1, at.2);
                    if !anchor.is_some_and(|s| pattern.takes('@', &s)) {
                        continue;
                    }
                    if let Some(f) = FACINGS
                        .into_iter()
                        .find(|f| pattern.complete(v, &dimension, at, *f, &mut budget))
                    {
                        out.push((at, f));
                    }
                }
                out
            })
            .unwrap_or_default())
    }
    fn cells_of(
        &mut self,
        pos: Pos,
        name: String,
        facing: patterns::Facing,
        symbol: String,
    ) -> Vec<Pos> {
        let Some(pattern) = self.pattern(&name) else {
            return Vec::new();
        };
        pattern
            .cells
            .iter()
            .filter(|(_, s)| symbol.chars().eq([*s]))
            .map(|&(c, _)| placed(pos, c, facing))
            .collect()
    }
    fn missing(
        &mut self,
        dimension: String,
        pos: Pos,
        name: String,
        facing: patterns::Facing,
    ) -> Vec<Change> {
        let Some(pattern) = self.pattern(&name) else {
            return Vec::new();
        };
        self.reading(|v| {
            pattern
                .cells
                .iter()
                .filter_map(|&(cell, symbol)| {
                    let (x, y, z) = placed(pos, cell, facing);
                    if v.block(&dimension, x, y, z)
                        .is_some_and(|s| pattern.takes(symbol, &s))
                    {
                        return None;
                    }
                    Some(Change {
                        x,
                        y,
                        z,
                        state: pattern.keys.get(&symbol)?.first()?.clone(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
    }
    fn build(
        &mut self,
        dimension: String,
        pos: Pos,
        name: String,
        facing: patterns::Facing,
    ) -> Result<u32, String> {
        if self.pattern(&name).is_none() {
            return Err(format!("no pattern {name}"));
        }
        let wrong = patterns::Host::missing(self, dimension.clone(), pos, name, facing);
        let n = wrong.len() as u32;
        mod_world::Host::set_blocks(self, dimension, wrong)?;
        Ok(n)
    }
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
fn cell_hash(seed: u64, x: i64, y: i64, z: i64) -> u64 {
    mix(mix(mix(seed ^ x as u64) ^ y as u64) ^ z as u64)
}
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
fn perlin2(seed: u64, x: f64, z: f64) -> f64 {
    const GRADS: [(f64, f64); 8] = [
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (0.707, 0.707),
        (-0.707, 0.707),
        (0.707, -0.707),
        (-0.707, -0.707),
    ];
    let (fx, fz) = (x.floor(), z.floor());
    let (cx, cz) = (fx as i64, fz as i64);
    let (tx, tz) = (x - fx, z - fz);
    let dot = |ox: i64, oz: i64| {
        let g = GRADS[(cell_hash(seed, cx + ox, 0, cz + oz) & 7) as usize];
        g.0 * (tx - ox as f64) + g.1 * (tz - oz as f64)
    };
    let (u, v) = (fade(tx), fade(tz));
    1.4 * lerp(
        lerp(dot(0, 0), dot(1, 0), u),
        lerp(dot(0, 1), dot(1, 1), u),
        v,
    )
}
fn perlin3(seed: u64, x: f64, y: f64, z: f64) -> f64 {
    const GRADS: [(f64, f64, f64); 12] = [
        (1.0, 1.0, 0.0),
        (-1.0, 1.0, 0.0),
        (1.0, -1.0, 0.0),
        (-1.0, -1.0, 0.0),
        (1.0, 0.0, 1.0),
        (-1.0, 0.0, 1.0),
        (1.0, 0.0, -1.0),
        (-1.0, 0.0, -1.0),
        (0.0, 1.0, 1.0),
        (0.0, -1.0, 1.0),
        (0.0, 1.0, -1.0),
        (0.0, -1.0, -1.0),
    ];
    let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
    let (cx, cy, cz) = (fx as i64, fy as i64, fz as i64);
    let (tx, ty, tz) = (x - fx, y - fy, z - fz);
    let dot = |ox: i64, oy: i64, oz: i64| {
        let g = GRADS[(cell_hash(seed, cx + ox, cy + oy, cz + oz) % 12) as usize];
        g.0 * (tx - ox as f64) + g.1 * (ty - oy as f64) + g.2 * (tz - oz as f64)
    };
    let (u, v, w) = (fade(tx), fade(ty), fade(tz));
    let layer = |oy: i64| {
        lerp(
            lerp(dot(0, oy, 0), dot(1, oy, 0), u),
            lerp(dot(0, oy, 1), dot(1, oy, 1), u),
            w,
        )
    };
    lerp(layer(0), layer(1), v)
}
/// Octaves of `one`, each twice as fine and half as strong, in one's range.
fn octaves(octaves: u8, seed: u64, mut one: impl FnMut(u64, f64) -> f64) -> f64 {
    let (mut sum, mut amp, mut total, mut scale) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..octaves.clamp(1, 16) {
        sum += amp * one(mix(seed ^ u64::from(o)), scale);
        total += amp;
        amp *= 0.5;
        scale *= 2.0;
    }
    sum / total
}
fn fbm2(seed: u64, x: f64, z: f64, n: u8) -> f64 {
    octaves(n, seed, |s, f| perlin2(s, x * f, z * f))
}
fn fbm3(seed: u64, x: f64, y: f64, z: f64, n: u8) -> f64 {
    octaves(n, seed, |s, f| perlin3(s, x * f, y * f, z * f))
}

impl noise::Host for State {
    fn perlin2(&mut self, seed: u64, x: f64, z: f64) -> f64 {
        perlin2(seed, x, z)
    }
    fn perlin3(&mut self, seed: u64, x: f64, y: f64, z: f64) -> f64 {
        perlin3(seed, x, y, z)
    }
    fn fbm2(&mut self, seed: u64, x: f64, z: f64, octaves: u8) -> f64 {
        fbm2(seed, x, z, octaves)
    }
    fn fbm3(&mut self, seed: u64, x: f64, y: f64, z: f64, octaves: u8) -> f64 {
        fbm3(seed, x, y, z, octaves)
    }
    fn ridged2(&mut self, seed: u64, x: f64, z: f64, n: u8) -> f64 {
        octaves(n, seed, |s, f| {
            1.0 - perlin2(s, x * f, z * f).abs().min(1.0)
        })
    }
    fn voronoi2(&mut self, seed: u64, x: f64, z: f64) -> (f64, u64) {
        let (cx, cz) = (x.floor() as i64, z.floor() as i64);
        let mut best = (f64::INFINITY, 0);
        for ox in -1..=1 {
            for oz in -1..=1 {
                let h = cell_hash(seed, cx + ox, 1, cz + oz);
                let px = (cx + ox) as f64 + (h & 0xffff) as f64 / 65536.0;
                let pz = (cz + oz) as f64 + ((h >> 16) & 0xffff) as f64 / 65536.0;
                let d = (px - x).hypot(pz - z);
                if d < best.0 {
                    best = (d, h);
                }
            }
        }
        best
    }
    fn fill2(
        &mut self,
        seed: u64,
        x: f64,
        z: f64,
        step: f64,
        width: u32,
        depth: u32,
        octaves: u8,
    ) -> Vec<f32> {
        if u64::from(width) * u64::from(depth) > 1 << 16 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity((width * depth) as usize);
        for d in 0..depth {
            for w in 0..width {
                let (px, pz) = (x + f64::from(w) * step, z + f64::from(d) * step);
                out.push(fbm2(seed, px, pz, octaves) as f32);
            }
        }
        out
    }
    fn fill3(
        &mut self,
        seed: u64,
        x: f64,
        y: f64,
        z: f64,
        step: f64,
        width: u32,
        height: u32,
        depth: u32,
        octaves: u8,
    ) -> Vec<f32> {
        if u128::from(width) * u128::from(height) * u128::from(depth) > 1 << 17 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity((width * height * depth) as usize);
        for h in 0..height {
            for d in 0..depth {
                for w in 0..width {
                    let py = y + f64::from(h) * step;
                    let (px, pz) = (x + f64::from(w) * step, z + f64::from(d) * step);
                    out.push(fbm3(seed, px, py, pz, octaves) as f32);
                }
            }
        }
        out
    }
    fn hash_pos(&mut self, seed: u64, x: i32, y: i32, z: i32) -> u64 {
        cell_hash(seed, x.into(), y.into(), z.into())
    }
    fn chunk_seed(&mut self, seed: u64, chunk_x: i32, chunk_z: i32) -> u64 {
        cell_hash(seed, chunk_x.into(), i64::MIN, chunk_z.into())
    }
}

fn cooldown_key(owner: &str, key: &str) -> String {
    format!("{owner}\0{key}")
}

impl timers::Host for State {
    fn after(&mut self, name: String, ticks: u64) -> Result<(), String> {
        self.set_timer(name, ticks, 0)
    }
    fn every(&mut self, name: String, ticks: u64) -> Result<(), String> {
        let ticks = ticks.max(1);
        self.set_timer(name, ticks, ticks)
    }
    fn cancel(&mut self, name: String) -> bool {
        self.clocks(|c, _| c.timers.remove(&name).is_some())
            .unwrap_or(false)
    }
    fn cancel_all(&mut self) {
        self.clocks(|c, _| c.timers.clear());
    }
    fn pending(&mut self) -> Vec<String> {
        self.clocks(|c, _| c.timers.keys().cloned().collect())
            .unwrap_or_default()
    }
    fn remaining(&mut self, name: String) -> Option<u64> {
        self.clocks(|c, now| Some(c.timers.get(&name)?.0.saturating_sub(now)))?
    }
    fn poll(&mut self) -> Vec<String> {
        self.clocks(|c, now| {
            let mut due: Vec<(u64, String)> = c
                .timers
                .iter()
                .filter(|(_, t)| t.0 <= now)
                .map(|(n, t)| (t.0, n.clone()))
                .collect();
            due.sort();
            for (_, name) in &due {
                match c.timers.get_mut(name) {
                    Some(t) if t.1 > 0 => t.0 = now.saturating_add(t.1),
                    _ => {
                        c.timers.remove(name);
                    }
                }
            }
            due.into_iter().map(|d| d.1).collect()
        })
        .unwrap_or_default()
    }
    fn cooldown_set(&mut self, owner: String, key: String, ticks: u64) -> Result<(), String> {
        if owner.len() + key.len() > 256 {
            return Err("an owner and key of at most 256 bytes".into());
        }
        let at = cooldown_key(&owner, &key);
        self.clocks(|c, now| {
            c.cooldowns.retain(|_, end| *end > now);
            if !c.cooldowns.contains_key(&at) && c.full() {
                return Err(format!("at most {CLOCKS_PER_MOD} timers and cooldowns"));
            }
            c.cooldowns.insert(at, now.saturating_add(ticks));
            Ok(())
        })
        .unwrap_or_else(|| Err("timers unavailable".into()))
    }
    fn cooldown_ready(&mut self, owner: String, key: String) -> bool {
        timers::Host::cooldown_left(self, owner, key) == 0
    }
    fn cooldown_left(&mut self, owner: String, key: String) -> u64 {
        let at = cooldown_key(&owner, &key);
        self.clocks(|c, now| {
            c.cooldowns
                .get(&at)
                .map_or(0, |end| end.saturating_sub(now))
        })
        .unwrap_or(0)
    }
    fn cooldown_clear(&mut self, owner: String, key: String) {
        let at = cooldown_key(&owner, &key);
        self.clocks(|c, _| c.cooldowns.remove(&at));
    }
    fn cooldown_clear_owner(&mut self, owner: String) {
        let prefix = format!("{owner}\0");
        self.clocks(|c, _| c.cooldowns.retain(|k, _| !k.starts_with(&prefix)));
    }
}

impl crowd::Host for State {
    fn names(&mut self) -> Vec<String> {
        self.snapshot
            .players
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }
    fn in_dimension(&mut self, dimension: String) -> Vec<players::Player> {
        let all = self.snapshot.players.iter();
        all.filter(|p| p.dimension == dimension).cloned().collect()
    }
    fn in_box(&mut self, dimension: String, a: Pos, b: Pos) -> Vec<players::Player> {
        let (lo, hi) = (corner(a, b, i32::min), corner(a, b, i32::max));
        let inside = |v: f64, l: i32, h: i32| v >= f64::from(l) && v < f64::from(h) + 1.0;
        let all = self.snapshot.players.iter();
        all.filter(|p| {
            p.dimension == dimension
                && inside(p.x, lo.0, hi.0)
                && inside(p.y, lo.1, hi.1)
                && inside(p.z, lo.2, hi.2)
        })
        .cloned()
        .collect()
    }
    fn nearest(&mut self, dimension: String, x: f64, y: f64, z: f64) -> Option<players::Player> {
        self.near_players(dimension, x, y, z, f64::INFINITY)
            .into_iter()
            .next()
    }
    fn sneaking(&mut self) -> Vec<players::Player> {
        let all = self.snapshot.players.iter();
        all.filter(|p| self.snapshot.sneaking.contains(&p.id))
            .cloned()
            .collect()
    }
    fn holding(&mut self, item: String) -> Vec<players::Player> {
        let all = self.snapshot.players.clone();
        all.into_iter()
            .filter(|p| {
                players::Host::held_item(self, p.id.clone()).is_some_and(|s| s.item == item)
            })
            .collect()
    }
    fn having(&mut self, item: String, least: u32) -> Vec<players::Player> {
        let wanted = BTreeMap::from([(item, least)]);
        let all = self.snapshot.players.clone();
        all.into_iter()
            .filter(|p| self.has_all(&p.id, &wanted))
            .collect()
    }
    fn tell_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        text: String,
    ) -> u32 {
        let near = self.near_players(dimension, x, y, z, radius);
        near.into_iter()
            .filter(|p| players::Host::tell(self, p.id.clone(), text.clone()))
            .count() as u32
    }
    fn tell_dimension(&mut self, dimension: String, text: String) -> u32 {
        crowd::Host::in_dimension(self, dimension)
            .into_iter()
            .filter(|p| players::Host::tell(self, p.id.clone(), text.clone()))
            .count() as u32
    }
    fn tell_others(&mut self, player: String, text: String) -> u32 {
        let all = self.snapshot.players.clone();
        all.into_iter()
            .filter(|p| p.id != player)
            .filter(|p| players::Host::tell(self, p.id.clone(), text.clone()))
            .count() as u32
    }
    fn title_all(&mut self, title: String, subtitle: Option<String>) -> Result<u32, String> {
        let all = self.snapshot.players.clone();
        for p in &all {
            players::Host::show_title(self, p.id.clone(), title.clone(), subtitle.clone())?;
        }
        Ok(all.len() as u32)
    }
    fn title_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        title: String,
        subtitle: Option<String>,
    ) -> Result<u32, String> {
        let near = self.near_players(dimension, x, y, z, radius);
        for p in &near {
            players::Host::show_title(self, p.id.clone(), title.clone(), subtitle.clone())?;
        }
        Ok(near.len() as u32)
    }
    fn action_bar_all(&mut self, text: String) -> Result<u32, String> {
        let all = self.snapshot.players.clone();
        for p in &all {
            self.console(format!("title {} actionbar {}", p.name, quoted(&text)))?;
        }
        Ok(all.len() as u32)
    }
    fn action_bar_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        text: String,
    ) -> Result<u32, String> {
        let near = self.near_players(dimension, x, y, z, radius);
        for p in &near {
            self.console(format!("title {} actionbar {}", p.name, quoted(&text)))?;
        }
        Ok(near.len() as u32)
    }
    fn sound_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        sound: String,
        volume: f32,
        pitch: f32,
    ) -> Result<u32, String> {
        let near = self.near_players(dimension, x, y, z, radius);
        for p in &near {
            players::Host::play_sound(self, p.id.clone(), sound.clone(), volume, pitch)?;
        }
        Ok(near.len() as u32)
    }
    fn effect_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        effect: String,
        seconds: u32,
        amplifier: u8,
    ) -> Result<u32, String> {
        check_game_id("effect", &effect)?;
        let seconds = seconds.clamp(1, 1_000_000);
        let near = self.near_players(dimension, x, y, z, radius);
        for p in &near {
            self.console(format!(
                "effect give {} {effect} {seconds} {amplifier}",
                p.name
            ))?;
        }
        Ok(near.len() as u32)
    }
    fn give_near(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        stack: players::ItemStack,
    ) -> Result<u32, String> {
        let near = self.near_players(dimension, x, y, z, radius);
        for p in &near {
            self.player_give(p.id.clone(), stack.clone())?;
        }
        Ok(near.len() as u32)
    }
    fn has_many(&mut self, player: String, items: Vec<(String, u32)>) -> bool {
        self.has_all(&player, &wanted(items))
    }
    fn take_many(&mut self, player: String, items: Vec<(String, u32)>) -> bool {
        let items = wanted(items);
        if !self.has_all(&player, &items) {
            return false;
        }
        for (item, count) in items {
            self.player_take(player.clone(), item, count);
        }
        true
    }
    fn slots_of(&mut self, player: String, item: String) -> Vec<u32> {
        let slots = self.player_inventory(player);
        (0..slots.len() as u32)
            .filter(|&i| slots[i as usize].as_ref().is_some_and(|s| s.item == item))
            .collect()
    }
}

fn fx_count(count: u32) -> usize {
    (count as usize).min(FX_POINTS)
}

impl fx::Host for State {
    fn line(
        &mut self,
        dimension: String,
        particle: String,
        ax: f64,
        ay: f64,
        az: f64,
        bx: f64,
        by: f64,
        bz: f64,
        count: u32,
    ) -> Result<(), String> {
        let n = fx_count(count);
        let at = |i: usize| i as f64 / (n.max(2) - 1) as f64;
        let points = (0..n).map(|i| {
            [
                lerp(ax, bx, at(i)),
                lerp(ay, by, at(i)),
                lerp(az, bz, at(i)),
            ]
        });
        self.fx_points(dimension, particle, points)
    }
    fn circle(
        &mut self,
        dimension: String,
        particle: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        count: u32,
    ) -> Result<(), String> {
        let n = fx_count(count);
        let points = (0..n).map(|i| {
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            [x + radius * a.cos(), y, z + radius * a.sin()]
        });
        self.fx_points(dimension, particle, points)
    }
    fn sphere(
        &mut self,
        dimension: String,
        particle: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        count: u32,
    ) -> Result<(), String> {
        let n = fx_count(count);
        // A Fibonacci sphere: even, without a grid's poles.
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let points = (0..n).map(|i| {
            let h = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
            let r = (1.0 - h * h).sqrt();
            let a = golden * i as f64;
            [
                x + radius * r * a.cos(),
                y + radius * h,
                z + radius * r * a.sin(),
            ]
        });
        self.fx_points(dimension, particle, points)
    }
    fn helix(
        &mut self,
        dimension: String,
        particle: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        height: f64,
        turns: f64,
        count: u32,
    ) -> Result<(), String> {
        let n = fx_count(count);
        let points = (0..n).map(|i| {
            let t = i as f64 / (n.max(2) - 1) as f64;
            let a = std::f64::consts::TAU * turns * t;
            [x + radius * a.cos(), y + height * t, z + radius * a.sin()]
        });
        self.fx_points(dimension, particle, points)
    }
    fn box_edges(
        &mut self,
        dimension: String,
        particle: String,
        ax: f64,
        ay: f64,
        az: f64,
        bx: f64,
        by: f64,
        bz: f64,
        count: u32,
    ) -> Result<(), String> {
        let per_edge = (fx_count(count) / 12).max(1);
        let corner = |m: u8| {
            [
                if m & 1 == 0 { ax } else { bx },
                if m & 2 == 0 { ay } else { by },
                if m & 4 == 0 { az } else { bz },
            ]
        };
        let mut points = Vec::with_capacity(per_edge * 12);
        for m in 0u8..8 {
            for bit in [1u8, 2, 4] {
                if m & bit == 0 {
                    let (p, q) = (corner(m), corner(m | bit));
                    for i in 0..per_edge {
                        let t = (i as f64 + 0.5) / per_edge as f64;
                        points.push([
                            lerp(p[0], q[0], t),
                            lerp(p[1], q[1], t),
                            lerp(p[2], q[2], t),
                        ]);
                    }
                }
            }
        }
        self.fx_points(dimension, particle, points.into_iter())
    }
    fn cone(
        &mut self,
        dimension: String,
        particle: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f64,
        height: f64,
        count: u32,
    ) -> Result<(), String> {
        let n = fx_count(count);
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let points = (0..n).map(|i| {
            let t = ((i as f64 + 0.5) / n as f64).sqrt();
            let a = golden * i as f64;
            [
                x + radius * t * a.cos(),
                y + height * t,
                z + radius * t * a.sin(),
            ]
        });
        self.fx_points(dimension, particle, points)
    }
}

impl stock::Host for State {
    fn slots_of(&mut self, dimension: String, (x, y, z): Pos, item: String) -> Vec<u32> {
        let slots = storages::Host::items(self, dimension, x, y, z).unwrap_or_default();
        (0..slots.len() as u32)
            .filter(|&i| slots[i as usize].as_ref().is_some_and(|s| s.item == item))
            .collect()
    }
    fn total_items(&mut self, dimension: String, (x, y, z): Pos) -> u32 {
        let slots = storages::Host::items(self, dimension, x, y, z).unwrap_or_default();
        slots.iter().flatten().map(|s| s.count).sum()
    }
    fn comparator_level(&mut self, dimension: String, (x, y, z): Pos) -> u8 {
        // AbstractContainerMenu.getRedstoneSignalFromContainer.
        let slots = storages::Host::items(self, dimension, x, y, z).unwrap_or_default();
        if slots.is_empty() {
            return 0;
        }
        let (mut sum, mut any) = (0.0f32, false);
        for s in slots.iter().flatten() {
            let max = mod_world::Host::max_stack_size(self, s.item.clone()).unwrap_or(64);
            sum += s.count as f32 / max.max(1) as f32;
            any = true;
        }
        if !any {
            return 0;
        }
        (1.0 + (sum / slots.len() as f32 * 14.0).floor()).min(15.0) as u8
    }
    fn energy_fraction(&mut self, dimension: String, (x, y, z): Pos) -> Option<f64> {
        let (stored, capacity) = storages::Host::energy(self, dimension, x, y, z)?;
        (capacity > 0).then(|| stored as f64 / capacity as f64)
    }
    fn fluid_fraction(&mut self, dimension: String, (x, y, z): Pos, tank: u32) -> Option<f64> {
        let shape = storages::Host::shape(self, dimension.clone(), x, y, z)?;
        let capacity = *shape.tanks.get(tank as usize)?;
        let tanks = storages::Host::fluids(self, dimension, x, y, z)?;
        let held = tanks.get(tank as usize)?.as_ref().map_or(0, |f| f.amount);
        (capacity > 0).then(|| f64::from(held) / f64::from(capacity))
    }
    fn swap_slots(
        &mut self,
        dimension: String,
        (x, y, z): Pos,
        a: u32,
        b: u32,
    ) -> Result<(), String> {
        let mut slots = storages::Host::items(self, dimension.clone(), x, y, z)
            .ok_or_else(|| format!("nothing holds items at {x} {y} {z}"))?;
        if a as usize >= slots.len() || b as usize >= slots.len() {
            return Err("no such slot".into());
        }
        let first = slots[a as usize].take();
        let second = slots[b as usize].take();
        storages::Host::set_item(self, dimension.clone(), x, y, z, a, second)?;
        storages::Host::set_item(self, dimension, x, y, z, b, first)
    }
    fn move_slot(
        &mut self,
        dimension: String,
        (x, y, z): Pos,
        a: u32,
        b: u32,
    ) -> Result<u32, String> {
        let slots = storages::Host::items(self, dimension.clone(), x, y, z)
            .ok_or_else(|| format!("nothing holds items at {x} {y} {z}"))?;
        let (Some(from), Some(to)) = (slots.get(a as usize), slots.get(b as usize)) else {
            return Err("no such slot".into());
        };
        let Some(mut from) = from.clone().filter(|_| a != b) else {
            return Ok(0);
        };
        let max = mod_world::Host::max_stack_size(self, from.item.clone()).unwrap_or(64);
        let mut to = match to.clone() {
            None => players::ItemStack {
                count: 0,
                ..from.clone()
            },
            Some(t)
                if t.item == from.item
                    && t.data == from.data
                    && t.components == from.components =>
            {
                t
            }
            Some(_) => return Ok(0),
        };
        let moved = from.count.min(max.saturating_sub(to.count));
        if moved == 0 {
            return Ok(0);
        }
        to.count += moved;
        from.count -= moved;
        let rest = (from.count > 0).then_some(from);
        storages::Host::set_item(self, dimension.clone(), x, y, z, b, Some(to))?;
        storages::Host::set_item(self, dimension, x, y, z, a, rest)?;
        Ok(moved)
    }
    fn has_many(&mut self, dimension: String, pos: Pos, items: Vec<(String, u32)>) -> bool {
        self.stock_has(&dimension, pos, &wanted(items))
    }
    fn take_many(&mut self, dimension: String, pos: Pos, items: Vec<(String, u32)>) -> bool {
        let items = wanted(items);
        if !self.stock_has(&dimension, pos, &items) {
            return false;
        }
        let (x, y, z) = pos;
        for (item, mut left) in items {
            // One call takes from one slot; `stock_has` said enough are there.
            while left > 0 {
                let got = storages::Host::extract_item(
                    self,
                    dimension.clone(),
                    x,
                    y,
                    z,
                    Some(item.clone()),
                    left,
                    false,
                );
                match got {
                    Some(s) if s.count > 0 => left = left.saturating_sub(s.count),
                    _ => break,
                }
            }
        }
        true
    }
    fn insert_many(
        &mut self,
        dimension: String,
        (x, y, z): Pos,
        stacks: Vec<players::ItemStack>,
    ) -> Vec<players::ItemStack> {
        stacks
            .into_iter()
            .filter_map(|mut s| {
                let put =
                    storages::Host::insert_item(self, dimension.clone(), x, y, z, s.clone(), false);
                s.count -= put.min(s.count);
                (s.count > 0).then_some(s)
            })
            .collect()
    }
    fn extract_all(
        &mut self,
        dimension: String,
        (x, y, z): Pos,
        item: String,
    ) -> Vec<players::ItemStack> {
        let mut out = Vec::new();
        while out.len() < 256 {
            let got = storages::Host::extract_item(
                self,
                dimension.clone(),
                x,
                y,
                z,
                Some(item.clone()),
                u32::MAX,
                false,
            );
            match got {
                Some(s) if s.count > 0 => out.push(s),
                _ => break,
            }
        }
        out
    }
    fn transfer_matching(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        items: Vec<String>,
        max: u32,
    ) -> u32 {
        if a == b {
            return 0;
        }
        let slots =
            storages::Host::items(self, dimension.clone(), a.0, a.1, a.2).unwrap_or_default();
        let mut left = max;
        for stack in slots.into_iter().flatten() {
            if left == 0 {
                break;
            }
            if !items.is_empty() && !items.contains(&stack.item) {
                continue;
            }
            let trial = players::ItemStack {
                count: stack.count.min(left),
                ..stack.clone()
            };
            let fits =
                storages::Host::insert_item(self, dimension.clone(), b.0, b.1, b.2, trial, true);
            if fits == 0 {
                continue;
            }
            let Some(got) = storages::Host::extract_item(
                self,
                dimension.clone(),
                a.0,
                a.1,
                a.2,
                Some(stack.item.clone()),
                fits,
                false,
            ) else {
                continue;
            };
            let count = got.count;
            let put = storages::Host::insert_item(
                self,
                dimension.clone(),
                b.0,
                b.1,
                b.2,
                got.clone(),
                false,
            );
            if put < count {
                // It took a stack of the same item that didn't fit: back it goes.
                let back = players::ItemStack {
                    count: count - put,
                    ..got
                };
                storages::Host::insert_item(self, dimension.clone(), a.0, a.1, a.2, back, false);
            }
            left = left.saturating_sub(put);
        }
        max - left
    }
}

impl admin::Host for State {
    fn clone_box(
        &mut self,
        dimension: String,
        a: Pos,
        b: Pos,
        to: Pos,
        masked: bool,
        moved: bool,
    ) -> Result<(), String> {
        self.check_dim(&dimension)?;
        let mask = if masked { "masked" } else { "replace" };
        let mode = if moved { "move" } else { "normal" };
        self.console(format!(
            "execute in {dimension} run clone {} {} {} {} {} {} {} {} {} {mask} {mode}",
            a.0, a.1, a.2, b.0, b.1, b.2, to.0, to.1, to.2
        ))
    }
    fn default_game_mode(&mut self, mode: String) -> Result<(), String> {
        if !matches!(
            mode.as_str(),
            "survival" | "creative" | "adventure" | "spectator"
        ) {
            return Err(format!("{mode:?}: no such game mode"));
        }
        self.console(format!("defaultgamemode {mode}"))
    }
    fn spectate(&mut self, player: String, target: Option<String>) -> Result<(), String> {
        let name = self.online_player(&player)?.name;
        match target {
            Some(target) => {
                check_game_id("entity", &target)?;
                self.console(format!("spectate {target} {name}"))
            }
            None => self.console(format!("execute as {name} run spectate")),
        }
    }
    fn spread_players(
        &mut self,
        x: f64,
        z: f64,
        spacing: f32,
        range: f32,
        players: Vec<String>,
    ) -> Result<(), String> {
        if !(x.is_finite() && z.is_finite() && spacing.is_finite() && range.is_finite()) {
            return Err("finite numbers".into());
        }
        let mut names = Vec::with_capacity(players.len());
        for p in &players {
            names.push(self.online_player(p)?.name);
        }
        if names.is_empty() {
            return Ok(());
        }
        self.console(format!(
            "spreadplayers {x} {z} {spacing} {range} false {}",
            names.join(" ")
        ))
    }
    fn run_function(&mut self, id: String) -> Result<(), String> {
        check_game_id("function", &id)?;
        self.console(format!("function {id}"))
    }
    fn enchant_held(
        &mut self,
        player: String,
        enchantment: String,
        level: u8,
    ) -> Result<(), String> {
        let name = self.online_player(&player)?.name;
        check_game_id("enchantment", &enchantment)?;
        self.console(format!("enchant {name} {enchantment} {level}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_stable_and_in_range() {
        for i in 0..2000 {
            let (x, z) = (f64::from(i) * 0.37, f64::from(i) * -0.91);
            let n = fbm2(7, x, z, 4);
            assert!(n.abs() <= 1.2, "{n}");
            assert_eq!(n, fbm2(7, x, z, 4));
            assert!(perlin3(7, x, z, x + z).abs() <= 1.2);
        }
        assert_ne!(fbm2(7, 0.5, 0.5, 1), fbm2(8, 0.5, 0.5, 1));
        // A whole number falls on the lattice, where a gradient gives nothing.
        assert_eq!(perlin2(7, 3.0, 4.0), 0.0);
    }

    #[test]
    fn states_turn_with_their_cells() {
        let turn = turned_state;
        assert_eq!(
            turn("a:b[facing=north,lit=true]", 1, false),
            "a:b[facing=east,lit=true]"
        );
        assert_eq!(turn("a:b[facing=east]", 0, true), "a:b[facing=west]");
        assert_eq!(turn("a:log[axis=x]", 1, false), "a:log[axis=z]");
        assert_eq!(turn("a:log[axis=y]", 1, false), "a:log[axis=y]");
        assert_eq!(turn("a:sign[rotation=4]", 3, false), "a:sign[rotation=0]");
        assert_eq!(
            turn("a:fence[north=true,east=false]", 1, false),
            "a:fence[east=true,south=false]"
        );
        assert_eq!(turn("a:stone", 2, true), "a:stone");
    }

    #[test]
    fn a_pattern_faces_all_four_ways() {
        use patterns::Facing::*;
        assert_eq!(placed((10, 0, 10), (0, 1, -1), North), (10, 1, 9));
        assert_eq!(placed((10, 0, 10), (0, 1, -1), East), (11, 1, 10));
        assert_eq!(placed((10, 0, 10), (0, 1, -1), South), (10, 1, 11));
        assert_eq!(placed((10, 0, 10), (0, 1, -1), West), (9, 1, 10));
    }

    #[test]
    fn rays_walk_cell_by_cell() {
        let cells = ray_cells([0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 3.0);
        let at: Vec<Pos> = cells.iter().map(|c| c.0).collect();
        assert_eq!(at, [(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0)]);
        assert!(ray_cells([0.0; 3], [0.0; 3], 3.0).is_empty());
        assert!(ray_cells([f64::NAN; 3], [1.0, 0.0, 0.0], 3.0).is_empty());
    }

    #[test]
    fn shapes_have_the_cells_they_say() {
        assert_eq!(ball((0, 0, 0), 0, false).unwrap(), [(0, 0, 0)]);
        assert_eq!(ball((0, 0, 0), 1, false).unwrap().len(), 7);
        assert!(ball((0, 0, 0), RADIUS + 1, false).is_err());
        let hollow = ball((0, 0, 0), 5, true).unwrap();
        assert!(!hollow.contains(&(0, 0, 0)) && hollow.contains(&(5, 0, 0)));
    }

    #[test]
    fn a_shifted_clock_keeps_its_distance() {
        let mut c = ModClocks::default();
        c.timers.insert("a".into(), (150, 0));
        c.cooldowns.insert("p\0k".into(), 120);
        c.shift(-100);
        assert_eq!(c.timers["a"].0, 50);
        assert_eq!(c.cooldowns["p\0k"], 20);
    }
}
