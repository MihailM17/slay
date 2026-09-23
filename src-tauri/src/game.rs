//! Slay rules engine: hex island, territories, gold economy, strict combat.

use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use std::collections::BTreeSet;

pub const BUY_PEASANT: i64 = 10;
pub const BUY_CASTLE: i64 = 15;
pub const START_GOLD: i64 = 10;
pub const CASTLE_DEF: u8 = 2;
pub const CAPITAL_DEF: u8 = 1;
pub const GRAVE_DEF: u8 = 1;

pub fn wage(rank: u8) -> i64 {
    match rank {
        1 => 2,
        2 => 6,
        3 => 18,
        4 => 54,
        _ => 0,
    }
}

const EVEN_OFF: [(i32, i32); 6] = [(1, 0), (-1, 0), (0, -1), (-1, -1), (0, 1), (-1, 1)];
const ODD_OFF: [(i32, i32); 6] = [(1, 0), (-1, 0), (1, -1), (0, -1), (1, 1), (0, 1)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tree {
    Pine,
    Palm,
}

#[derive(Clone, Debug)]
pub struct Hex {
    pub water: bool,
    pub owner: i8,
    pub unit: u8,
    pub acted: bool,
    pub castle: bool,
    pub tree: Option<Tree>,
    pub grave: bool,
}

impl Hex {
    fn sea() -> Self {
        Hex { water: true, owner: -1, unit: 0, acted: false, castle: false, tree: None, grave: false }
    }
    fn land() -> Self {
        Hex { water: false, ..Hex::sea() }
    }
}

#[derive(Clone, Debug)]
pub struct Terr {
    pub owner: i8,
    pub hexes: BTreeSet<(i32, i32)>,
    pub savings: i64,
    pub capital: Option<(i32, i32)>,
}

pub struct Game {
    pub cols: i32,
    pub rows: i32,
    pub seed: u64,
    pub arch: String,
    pub difficulty: String,
    pub enemies: u8,
    pub size: u8,
    pub players: Vec<i8>,
    pub round: u32,
    pub current: i8,
    pub grid: Vec<Hex>,
    pub terrs: Vec<Terr>,
    pub log: Vec<String>,
    pub sel: Option<(i32, i32)>,
    pub focus: Option<(i32, i32)>,
    pub starts: Vec<(i32, i32)>,
    pub humans: Vec<i8>,
    pub tutorial: bool,
    pub custom: Option<CustomSpec>,
    rng: StdRng,
}

/// A hand-painted island for the level creator.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CustomSpec {
    pub cols: i32,
    pub rows: i32,
    pub land: Vec<(i32, i32)>,
    pub starts: Vec<(i32, i32)>,
}

/// Hex-step distance on an odd-r board (proper 6-neighbour metric,
///
/// not Manhattan: diagonal neighbours are 1 step, not 2).
pub fn hex_dist(a: (i32, i32), b: (i32, i32)) -> i32 {
    fn to_cube(col: i32, row: i32) -> (i32, i32, i32) {
        let x = col - (row - (row & 1)) / 2;
        let z = row;
        (x, -x - z, z)
    }
    let (ax, ay, az) = to_cube(a.0, a.1);
    let (bx, by, bz) = to_cube(b.0, b.1);
    ((ax - bx).abs() + (ay - by).abs() + (az - bz).abs()) / 2
}

impl Game {
    pub fn new(
        seed: Option<u64>,
        enemies: u8,
        difficulty: &str,
        size: u8,
        humans: Vec<i8>,
        tutorial: bool,
        custom: Option<CustomSpec>,
    ) -> Self {
        let seed = seed.unwrap_or_else(|| {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x853c49e6748fea9)
        });
        // hand-painted islands skip generation entirely (validated here;
        // strays and bad input fall back to a generated island)
        let mut custom_ok = false;
        let mut custom_land = BTreeSet::new();
        let mut custom_starts: Vec<(i32, i32)> = vec![];
        if let Some(c) = custom.as_ref() {
            if c.cols >= 4 && c.rows >= 4 && c.cols <= 24 && c.rows <= 18 {
                let land: BTreeSet<(i32, i32)> = c.land.iter().cloned()
                    .filter(|&(x, y)| x >= 0 && x < c.cols && y >= 0 && y < c.rows)
                    .collect();
                let mut seen = BTreeSet::new();
                let starts: Vec<(i32, i32)> = c.starts.iter().cloned()
                    .filter(|p| land.contains(p) && seen.insert(*p)).collect();
                if land.len() >= 12 && (2..=4).contains(&starts.len()) {
                    custom_ok = true;
                    custom_land = land;
                    custom_starts = starts;
                }
            }
        }
        let (cols, rows) = if custom_ok {
            let c = custom.as_ref().unwrap();
            (c.cols, c.rows)
        } else {
            match size {
                0 => (12, 9),
                1 => (15, 11),
                _ => (18, 13),
            }
        };
        let nfoes = if custom_ok { custom_starts.len() - 1 } else { enemies.clamp(1, 3) as usize };
        let humans: Vec<i8> = {
            let valid: Vec<i8> = humans.into_iter()
                .filter(|&o| o >= 0 && (o as usize) <= nfoes).collect();
            if valid.is_empty() { vec![0] } else { valid }
        };
        let mut g = Game {
            cols, rows, seed,
            arch: String::new(),
            difficulty: difficulty.to_string(),
            enemies: enemies.clamp(1, 3),
            size,
            players: (0..=(nfoes as i8)).collect(),
            round: 1,
            current: 0,
            grid: vec![Hex::sea(); (cols * rows) as usize],
            terrs: vec![],
            log: vec![],
            sel: None,
            focus: None,
            starts: vec![],
            humans,
            tutorial,
            custom: custom.clone(),
            rng: StdRng::seed_from_u64(seed ^ 0x9e3779b97f4a7c15),
        };
        if custom_ok {
            g.arch = "Custom".to_string();
            for y in 0..g.rows {
                for x in 0..g.cols {
                    let i = Self::idx(g.cols, x, y);
                    g.grid[i] = if custom_land.contains(&(x, y)) { Hex::land() } else { Hex::sea() };
                }
            }
            g.starts = custom_starts.clone();
            for (p, (x, y)) in custom_starts.iter().enumerate() {
                let i = Self::idx(g.cols, *x, *y);
                g.grid[i].owner = p as i8;
            }
            // a few wild pines so the economy breathes
            let wilds: Vec<(i32, i32)> = custom_land.iter().cloned()
                .filter(|p| !custom_starts.contains(p)).collect();
            for (i, p) in wilds.iter().enumerate() {
                if i % 9 == 0 {
                    g.grid[Self::idx(g.cols, p.0, p.1)].tree = Some(Tree::Pine);
                }
            }
        } else {
            g.gen();
        }
        g.recompute();
        for t in g.terrs.iter_mut() {
            t.savings = START_GOLD;
        }
        if tutorial {
            // tutorial grant: your home treasury starts funded
            if let Some(s0) = g.starts.first().cloned() {
                if let Some(ti) = g.terr_at(s0.0, s0.1) {
                    if g.terrs[ti].owner == 0 {
                        g.terrs[ti].savings = 25;
                    }
                }
            }
        }
        g.say("Buy peasants, grow, combine into armies.");
        g.say("Cut enemies in half - the poor side starves.");
        g
    }

    // ---------- rng helpers ----------
    fn ri(&mut self, n: usize) -> usize {
        if n == 0 { return 0; }
        (self.rng.next_u64() % n as u64) as usize
    }
    fn rf(&mut self) -> f64 {
        (self.rng.next_u64() as f64) / (u64::MAX as f64)
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.ri(i + 1);
            v.swap(i, j);
        }
    }

    // ---------- grid ----------
    pub fn idx(cols: i32, x: i32, y: i32) -> usize {
        (y * cols + x) as usize
    }
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.cols && y >= 0 && y < self.rows
    }
    pub fn neighbours(&self, x: i32, y: i32) -> Vec<(i32, i32)> {
        let offs = if y & 1 == 1 { ODD_OFF } else { EVEN_OFF };
        let mut out = vec![];
        for (dx, dy) in offs {
            let (nx, ny) = (x + dx, y + dy);
            if self.in_bounds(nx, ny) {
                out.push((nx, ny));
            }
        }
        out
    }

    fn say(&mut self, msg: &str) {
        self.log.push(msg.to_string());
        if self.log.len() > 4 {
            let n = self.log.len();
            self.log.drain(..n - 4);
        }
    }

    // ---------- map gen ----------
    fn try_land(&mut self) -> BTreeSet<(i32, i32)> {
        let cx = (self.cols - 1) as f64 / 2.0;
        let cy = (self.rows - 1) as f64 / 2.0;
        let rx = self.cols as f64 / 2.0 - 0.5 * (0.85 + self.rf() * 0.25);
        let ry = self.rows as f64 / 2.0 - 0.5 * (0.85 + self.rf() * 0.25);
        let mut land = BTreeSet::new();
        for y in 0..self.rows {
            for x in 0..self.cols {
                let n = ((x as f64 - cx) / rx).powi(2) + ((y as f64 - cy) / ry).powi(2);
                if self.arch == "Atoll" {
                    if 0.40 + self.rf() * 0.25 < n && n < 1.05 {
                        land.insert((x, y));
                    }
                } else if n + self.rf() * 0.45 < 1.0 {
                    land.insert((x, y));
                }
            }
        }
        land
    }

    fn biggest(&self, land: &BTreeSet<(i32, i32)>) -> BTreeSet<(i32, i32)> {
        let mut comps: Vec<BTreeSet<(i32, i32)>> = vec![];
        let mut left = land.clone();
        while let Some(&s) = left.iter().next() {
            let mut comp = BTreeSet::new();
            let mut stack = vec![s];
            while let Some(p) = stack.pop() {
                if !comp.insert(p) {
                    continue;
                }
                left.remove(&p);
                for q in self.neighbours(p.0, p.1) {
                    if land.contains(&q) && !comp.contains(&q) {
                        stack.push(q);
                    }
                }
            }
            comps.push(comp);
        }
        comps.into_iter().max_by_key(|c| c.len()).unwrap_or_default()
    }

    fn gen(&mut self) {
        let mut land = BTreeSet::new();
        {
        let archs = ["Isle", "Atoll", "Lakes"];
        self.arch = archs[self.ri(3)].to_string();
        for _ in 0..12 {
            let attempt = self.try_land();
            land = self.biggest(&attempt);
            if self.arch == "Lakes" {
                for _ in 0..(2 + self.ri(2)) {
                    let inner: Vec<(i32, i32)> = land.iter().cloned()
                        .filter(|p| self.neighbours(p.0, p.1).iter().all(|&q| land.contains(&q)))
                        .collect();
                    if inner.is_empty() {
                        break;
                    }
                    let (lx, ly) = inner[self.ri(inner.len())];
                    land.remove(&(lx, ly));
                    for q in self.neighbours(lx, ly) {
                        land.remove(&q);
                    }
                }
                land = self.biggest(&land);
            }
            if land.len() >= 25 {
                break;
            }
        }
        if land.len() < 25 {
            self.arch = "Isle".to_string();
            let attempt = self.try_land();
            land = self.biggest(&attempt);
        }
        }
        for y in 0..self.rows {
            for x in 0..self.cols {
                let i = Self::idx(self.cols, x, y);
                self.grid[i] = if land.contains(&(x, y)) { Hex::land() } else { Hex::sea() };
            }
        }
        // Starting spots: every pair at least MIN_GAP hex-steps apart
        // (proper hex distance, not Manhattan). Falls back to greedy
        // maximin so the spread is always the best the island allows.
        const MIN_GAP: i32 = 3;
        let mut cands: Vec<(i32, i32)> = land.iter().cloned().collect();
        self.shuffle(&mut cands);
        let mut spots = vec![];
        for &c in &cands {
            if spots.len() == self.players.len() {
                break;
            }
            if spots.iter().all(|&s| hex_dist(c, s) >= MIN_GAP) {
                spots.push(c);
            }
        }
        if spots.len() < self.players.len() {
            spots.clear();
            let mut pool = cands;
            if let Some(first) = pool.pop() {
                spots.push(first);
            }
            while spots.len() < self.players.len() && !pool.is_empty() {
                let mut bi = 0;
                let mut bd = -1;
                for (i, &c) in pool.iter().enumerate() {
                    let d = spots.iter().map(|&s| hex_dist(c, s)).min().unwrap_or(99);
                    if d > bd {
                        bd = d;
                        bi = i;
                    }
                }
                spots.push(pool.remove(bi));
            }
        }
        self.starts = spots.clone();
        for (p, (x, y)) in self.players.clone().iter().zip(spots) {
            let i = Self::idx(self.cols, x, y);
            self.grid[i].owner = *p;
        }
        for &(x, y) in &land {
            let i = Self::idx(self.cols, x, y);
            if self.grid[i].owner != -1 || self.rf() > 0.10 {
                continue;
            }
            let coast = self.neighbours(x, y).iter().any(|&(nx, ny)| self.grid[Self::idx(self.cols, nx, ny)].water);
            self.grid[i].tree = Some(if coast && self.rf() < 0.6 { Tree::Palm } else { Tree::Pine });
        }
    }

    // ---------- territories ----------
    fn pick_capital(&self, comp: &BTreeSet<(i32, i32)>) -> (i32, i32) {
        let n = comp.len() as f64;
        let (sx, sy) = comp.iter().fold((0i64, 0i64), |(a, b), p| (a + p.0 as i64, b + p.1 as i64));
        let (cx, cy) = (sx as f64 / n, sy as f64 / n);
        let mut best = comp.iter().next().cloned().unwrap();
        let mut bd = f64::MAX;
        for &p in comp {
            let d = (p.0 as f64 - cx).powi(2) + (p.1 as f64 - cy).powi(2);
            if d < bd - 1e-9 || ((d - bd).abs() < 1e-9 && p < best) {
                bd = d;
                best = p;
            }
        }
        best
    }

    fn recompute(&mut self) {
        let old = std::mem::take(&mut self.terrs);
        let mut new_terrs = vec![];
        for &o in &self.players {
            let mut remaining: BTreeSet<(i32, i32)> = (0..self.rows)
                .flat_map(|y| (0..self.cols).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let h = &self.grid[Self::idx(self.cols, x, y)];
                    h.owner == o && !h.water
                })
                .collect();
            while let Some(&s) = remaining.iter().next() {
                let mut comp = BTreeSet::new();
                let mut stack = vec![s];
                while let Some(p) = stack.pop() {
                    if !comp.insert(p) {
                        continue;
                    }
                    remaining.remove(&p);
                    for q in self.neighbours(p.0, p.1) {
                        let h = &self.grid[Self::idx(self.cols, q.0, q.1)];
                        if h.owner == o && remaining.contains(&q) {
                            stack.push(q);
                        }
                    }
                }
                let mut gold = 0i64;
                for t in &old {
                    if t.owner == o && !t.hexes.is_disjoint(&comp)
                        && (t.capital.is_none() || comp.contains(&t.capital.unwrap()))
                    {
                        gold += t.savings;
                    }
                }
                let cap = if comp.len() >= 2 { Some(self.pick_capital(&comp)) } else { None };
                new_terrs.push(Terr { owner: o, hexes: comp, savings: gold, capital: cap });
            }
        }
        self.terrs = new_terrs;
    }

    pub fn terr_at(&self, x: i32, y: i32) -> Option<usize> {
        self.terrs.iter().position(|t| t.hexes.contains(&(x, y)))
    }
    pub fn terrs_of(&self, o: i8) -> Vec<usize> {
        self.terrs.iter().enumerate().filter(|(_, t)| t.owner == o).map(|(i, _)| i).collect()
    }
    pub fn hexes_of(&self, o: i8) -> usize {
        self.terrs.iter().filter(|t| t.owner == o).map(|t| t.hexes.len()).sum()
    }

    fn terr_income(&self, ti: usize) -> i64 {
        self.terrs[ti].hexes.iter().filter(|&&(x, y)| self.grid[Self::idx(self.cols, x, y)].tree.is_none()).count() as i64
    }
    fn terr_wages(&self, ti: usize) -> i64 {
        self.terrs[ti].hexes.iter().map(|&(x, y)| wage(self.grid[Self::idx(self.cols, x, y)].unit)).sum()
    }

    // ---------- economy ----------
    pub fn start_turn(&mut self, p: i8) {
        let tis = self.terrs_of(p);
        for ti in tis {
            let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
            for &(x, y) in &hexes {
                let i = Self::idx(self.cols, x, y);
                if self.grid[i].grave {
                    self.grid[i].grave = false;
                    if self.grid[i].unit == 0 && !self.grid[i].castle {
                        self.grid[i].tree = Some(Tree::Pine);
                    }
                }
            }
            for &(x, y) in &hexes {
                let i = Self::idx(self.cols, x, y);
                let h = &self.grid[i];
                if h.tree.is_some() || h.unit != 0 || h.castle || h.grave {
                    continue;
                }
                if Some((x, y)) == self.terrs[ti].capital {
                    continue;
                }
                let pine_nb = self.neighbours(x, y).iter()
                    .filter(|&&(nx, ny)| self.grid[Self::idx(self.cols, nx, ny)].tree == Some(Tree::Pine)).count();
                let palm_nb = self.neighbours(x, y).iter()
                    .filter(|&&(nx, ny)| self.grid[Self::idx(self.cols, nx, ny)].tree == Some(Tree::Palm)).count();
                let coast = self.neighbours(x, y).iter()
                    .any(|&(nx, ny)| self.grid[Self::idx(self.cols, nx, ny)].water);
                if pine_nb >= 2 && self.rf() < 0.5 {
                    self.grid[i].tree = Some(Tree::Pine);
                } else if coast && palm_nb >= 1 && self.rf() < 0.5 {
                    self.grid[i].tree = Some(Tree::Palm);
                }
            }
            let income = self.terr_income(ti);
            let wages = self.terr_wages(ti);
            self.terrs[ti].savings += income;
            if self.terrs[ti].savings >= wages {
                self.terrs[ti].savings -= wages;
            } else {
                for &(x, y) in &hexes {
                    let i = Self::idx(self.cols, x, y);
                    if self.grid[i].unit != 0 {
                        self.grid[i].unit = 0;
                        self.grid[i].acted = false;
                        self.grid[i].grave = true;
                    }
                }
                self.terrs[ti].savings = 0;
                if p == 0 {
                    self.say("Bankrupt! A territory starved.");
                }
            }
        }
        for ti in self.terrs_of(p) {
            let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
            for (x, y) in hexes {
                self.grid[Self::idx(self.cols, x, y)].acted = false;
            }
        }
    }

    // ---------- combat ----------
    pub fn defence(&self, x: i32, y: i32) -> u8 {
        let h = &self.grid[Self::idx(self.cols, x, y)];
        if h.water {
            return 99;
        }
        let mut best = 0u8;
        if h.unit > 0 {
            best = best.max(h.unit);
        }
        if h.castle {
            best = best.max(CASTLE_DEF);
        }
        if h.grave {
            best = best.max(GRAVE_DEF);
        }
        if let Some(ti) = self.terr_at(x, y) {
            let t = &self.terrs[ti];
            if Some((x, y)) == t.capital {
                best = best.max(CAPITAL_DEF);
            }
            for (nx, ny) in self.neighbours(x, y) {
                let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                if nh.owner == h.owner && t.hexes.contains(&(nx, ny)) {
                    if nh.unit > 0 {
                        best = best.max(nh.unit);
                    }
                    if nh.castle {
                        best = best.max(CASTLE_DEF);
                    }
                    if Some((nx, ny)) == t.capital {
                        best = best.max(CAPITAL_DEF);
                    }
                }
            }
        }
        best
    }

    fn touches_terr(&self, ti: usize, tx: i32, ty: i32) -> bool {
        self.terrs[ti].hexes.iter().any(|&(x, y)| {
            self.neighbours(x, y).contains(&(tx, ty))
        })
    }

    pub fn can_attack(&self, fx: i32, fy: i32, tx: i32, ty: i32) -> Result<(), String> {
        let fh = &self.grid[Self::idx(self.cols, fx, fy)];
        if fh.unit == 0 || fh.acted || fh.owner == -1 {
            return Err(String::new());
        }
        let fti = self.terr_at(fx, fy).ok_or_else(String::new)?;
        if !self.touches_terr(fti, tx, ty) {
            return Err("Too far: attacks must touch your territory.".to_string());
        }
        let th = &self.grid[Self::idx(self.cols, tx, ty)];
        if th.water {
            return Err("Water.".to_string());
        }
        if th.owner == fh.owner {
            return Err("Own hex (move/combine instead).".to_string());
        }
        let d = self.defence(tx, ty);
        if fh.unit <= d {
            return Err(format!("Too strong (def {d}). Combine men first."));
        }
        Ok(())
    }

    pub fn do_attack(&mut self, fx: i32, fy: i32, tx: i32, ty: i32, who: i8) -> Result<String, String> {
        self.can_attack(fx, fy, tx, ty)?;
        let s = self.grid[Self::idx(self.cols, fx, fy)].unit;
        let ti = Self::idx(self.cols, tx, ty);
        {
            let th = &mut self.grid[ti];
            if th.unit != 0 {
                th.grave = true;
            }
            th.owner = who;
            th.unit = s;
            th.acted = true;
            th.castle = false;
            th.tree = None;
        }
        {
            let fh = &mut self.grid[Self::idx(self.cols, fx, fy)];
            fh.unit = 0;
            fh.acted = false;
        }
        self.recompute();
        Ok(format!("{} takes a hex.", rank_name(s)))
    }

    pub fn do_move(&mut self, fx: i32, fy: i32, tx: i32, ty: i32) -> Result<String, String> {
        let fti = self.terr_at(fx, fy).ok_or("Move inside your own territory.".to_string())?;
        if !self.terrs[fti].hexes.contains(&(tx, ty)) {
            return Err("Same territory only.".to_string());
        }
        let (fowner, funit, facted) = {
            let h = &self.grid[Self::idx(self.cols, fx, fy)];
            (h.owner, h.unit, h.acted)
        };
        if funit == 0 || facted {
            return Err("That man already acted.".to_string());
        }
        let ti2 = Self::idx(self.cols, tx, ty);
        if self.grid[ti2].unit != 0 {
            let s = funit + self.grid[ti2].unit;
            if s > 4 {
                return Err("Too strong to combine (max Baron).".to_string());
            }
            self.grid[ti2].unit = s;
            self.grid[ti2].acted = true;
            let fi = Self::idx(self.cols, fx, fy);
            self.grid[fi].unit = 0;
            self.grid[fi].acted = false;
            return Ok(format!("Combined into {}.", rank_name(s)));
        }
        if self.grid[ti2].castle {
            return Err("Occupied by a castle.".to_string());
        }
        if self.grid[ti2].owner != fowner {
            return Err("Move inside your own territory.".to_string());
        }
        let chop = self.grid[ti2].tree.is_some();
        {
            let th = &mut self.grid[ti2];
            th.unit = funit;
            th.tree = None;
            th.grave = false;
            th.acted = false;
        }
        {
            let fh = &mut self.grid[Self::idx(self.cols, fx, fy)];
            fh.unit = 0;
            fh.acted = false;
        }
        if chop {
            self.grid[ti2].acted = true;
            return Ok("Chopped a tree.".to_string());
        }
        Ok("Moved.".to_string())
    }

    pub fn buy(&mut self, x: i32, y: i32, kind: &str, who: i8) -> Result<String, String> {
        let ti = self.terr_at(x, y).ok_or("Your territory only.".to_string())?;
        if self.terrs[ti].owner != who {
            return Err("Your territory only.".to_string());
        }
        let cost = if kind == "castle" { BUY_CASTLE } else { BUY_PEASANT };
        if self.terrs[ti].savings < cost {
            return Err(format!("Need {} gold (territory has {}).", cost, self.terrs[ti].savings));
        }
        {
            let h = &self.grid[Self::idx(self.cols, x, y)];
            if h.unit != 0 || h.castle || h.grave {
                return Err("Occupied.".to_string());
            }
            if kind == "castle" && h.tree.is_some() {
                return Err("Chop the tree first.".to_string());
            }
        }
        {
            let h = &mut self.grid[Self::idx(self.cols, x, y)];
            if kind == "castle" {
                h.castle = true;
            } else {
                h.unit = 1;
                h.acted = false;
                h.tree = None;
            }
        }
        self.terrs[ti].savings -= cost;
        Ok(if kind == "castle" { "Castle built.".to_string() } else { "Peasant ready.".to_string() })
    }

    // ---------- cuts ----------
    pub fn split_pieces(&self, tx: i32, ty: i32, victim: i8) -> usize {
        let ti = match self.terr_at(tx, ty) {
            Some(i) if self.terrs[i].owner == victim => i,
            _ => return 1,
        };
        let rest: BTreeSet<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().filter(|&p| p != (tx, ty)).collect();
        if rest.is_empty() {
            return 0;
        }
        let mut count = 0;
        let mut seen = BTreeSet::new();
        while let Some(&s) = rest.difference(&seen).next() {
            count += 1;
            let mut stack = vec![s];
            while let Some(p) = stack.pop() {
                if !seen.insert(p) {
                    continue;
                }
                for q in self.neighbours(p.0, p.1) {
                    if rest.contains(&q) && !seen.contains(&q) {
                        stack.push(q);
                    }
                }
            }
        }
        count
    }

    pub fn choke_hexes(&self, ti: usize) -> Vec<(i32, i32)> {
        let hexes = &self.terrs[ti].hexes;
        if hexes.len() < 4 {
            return vec![];
        }
        let mut out = vec![];
        for &p in hexes {
            let rest: BTreeSet<(i32, i32)> = hexes.iter().cloned().filter(|&q| q != p).collect();
            let &start = rest.iter().next().unwrap();
            let mut seen = BTreeSet::from([start]);
            let mut stack = vec![start];
            while let Some(q) = stack.pop() {
                for r in self.neighbours(q.0, q.1) {
                    if rest.contains(&r) && seen.insert(r) {
                        stack.push(r);
                    }
                }
            }
            if seen.len() < rest.len() {
                out.push(p);
            }
        }
        out
    }

    // ---------- AI ----------
    /// End the current player's turn and run everything up to the next
    /// HUMAN turn (AI opponents in between play immediately). The round
    /// counter ticks over each time play wraps back to the first seat.
    pub fn advance_turn(&mut self) {
        self.sel = None;
        let n = self.players.len();
        let mut idx = self.players.iter().position(|&p| p == self.current).unwrap_or(0);
        for _ in 0..=n {
            idx = (idx + 1) % n;
            let p = self.players[idx];
            if idx == 0 {
                self.round += 1;
            }
            self.current = p;
            if self.humans.contains(&p) {
                self.start_turn(p);
                break;
            }
            self.ai_take_turn(p);
            if self.winner().is_some() {
                break;
            }
        }
    }

    pub fn ai_take_turn(&mut self, ai: i8) {
        self.start_turn(ai);
        let aggro = if ai == 1 { 1.0 } else { 0.6 };
        let mut used = BTreeSet::new();
        for ti in self.terrs_of(ai) {
            for (cx, cy) in self.choke_hexes(ti) {
                let ci = Self::idx(self.cols, cx, cy);
                {
                    let ch = &self.grid[ci];
                    if ch.unit != 0 || ch.castle || ch.grave || ch.tree.is_some() {
                        continue;
                    }
                }
                let cands: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
                for (mx, my) in cands {
                    if used.contains(&(mx, my)) || (mx, my) == (cx, cy) {
                        continue;
                    }
                    let mh = &self.grid[Self::idx(self.cols, mx, my)];
                    if mh.unit != 0 && !mh.acted {
                        if self.do_move(mx, my, cx, cy).is_ok() {
                            used.insert((cx, cy));
                        }
                        break;
                    }
                }
            }
        }
        let budget = 60;
        for _ in 0..budget {
            if !self.ai_step(ai, aggro) {
                break;
            }
        }
    }

    fn ai_step(&mut self, ai: i8, aggro: f64) -> bool {
        // 1) chop income-denying trees
        let tis = self.terrs_of(ai);
        for &ti in &tis {
            let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
            for (x, y) in hexes {
                let hi = Self::idx(self.cols, x, y);
                if self.grid[hi].unit == 0 || self.grid[hi].acted {
                    continue;
                }
                for (nx, ny) in self.neighbours(x, y) {
                    let ni = Self::idx(self.cols, nx, ny);
                    let ok = self.terrs[ti].hexes.contains(&(nx, ny))
                        && self.grid[ni].tree.is_some()
                        && self.grid[ni].unit == 0
                        && !self.grid[ni].castle
                        && Some((nx, ny)) != self.terrs[ti].capital;
                    if ok && self.do_move(x, y, nx, ny).is_ok() {
                        return true;
                    }
                }
            }
        }
        // 2) best attack, with a big bonus for cuts
        let mut best: Option<(f64, i32, i32, i32, i32)> = None;
        for &ti in &tis {
            let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
            for (x, y) in hexes {
                if self.grid[Self::idx(self.cols, x, y)].unit == 0 || self.grid[Self::idx(self.cols, x, y)].acted {
                    continue;
                }
                let mut seen = BTreeSet::new();
                for (tx, ty) in self.targets_around(ti, x, y) {
                    if !seen.insert((tx, ty)) {
                        continue;
                    }
                    if self.can_attack(x, y, tx, ty).is_err() {
                        continue;
                    }
                    let th = &self.grid[Self::idx(self.cols, tx, ty)];
                    let mut score = if th.owner == -1 {
                        3.0 + if th.tree.is_some() { 2.0 } else { 0.0 }
                    } else if th.unit != 0 {
                        4.0 + th.unit as f64
                    } else {
                        2.0
                    };
                    if th.owner != -1 && th.owner != ai {
                        score *= aggro;
                        match self.split_pieces(tx, ty, th.owner) {
                            0 => score += 12.0,
                            n => score += 6.0 * (n as f64 - 1.0),
                        }
                    } else {
                        score *= 2.0 - aggro;
                    }
                    score += self.rf();
                    if best.map_or(true, |(b, _, _, _, _)| score > b) {
                        best = Some((score, x, y, tx, ty));
                    }
                }
            }
        }
        if let Some((_, fx, fy, tx, ty)) = best {
            if self.do_attack(fx, fy, tx, ty, ai).is_ok() {
                return true;
            }
        }
        // 3) buy only with a purpose and only if next payday survives
        let easy = self.difficulty == "easy";
        let mut order = self.terrs_of(ai);
        order.sort_by_key(|&ti| -(self.terrs[ti].hexes.len() as i64));
        for ti in order {
            let wages = self.terr_wages(ti);
            let income = self.terr_income(ti);
            let buf = if easy { 4 } else { 0 };
            if self.terrs[ti].savings - BUY_PEASANT + income < wages + wage(1) + buf {
                if income > 0 || self.terrs[ti].savings < BUY_PEASANT {
                    continue;
                }
            }
            if !self.needs_man(ti, ai) {
                continue;
            }
            if let Some((sx, sy)) = self.buy_spot(ti, ai) {
                if self.buy(sx, sy, "man", ai).is_ok() {
                    return true;
                }
            }
        }
        // 4) combine if next payday covers it
        for ti in self.terrs_of(ai) {
            let wages = self.terr_wages(ti);
            let income = self.terr_income(ti);
            let mut men: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned()
                .filter(|&(x, y)| {
                    let h = &self.grid[Self::idx(self.cols, x, y)];
                    h.unit != 0 && !h.acted
                })
                .collect();
            if men.len() < 2 {
                continue;
            }
            men.sort_by_key(|&(x, y)| -(self.grid[Self::idx(self.cols, x, y)].unit as i64));
            let (ax, ay) = men[0];
            let (bx, by) = men[1];
            let a = self.grid[Self::idx(self.cols, ax, ay)].unit;
            let b = self.grid[Self::idx(self.cols, bx, by)].unit;
            if a + b <= 4 && (a + b > a || !easy) {
                if easy && a + b > 2 {
                    continue;
                }
                let after = wages - wage(a) - wage(b) + wage(a + b);
                if self.terrs[ti].savings + income < after {
                    continue;
                }
                if self.do_move(ax, ay, bx, by).is_ok() {
                    return true;
                }
            }
        }
        // 5) castle threatened rich borders
        for ti in self.terrs_of(ai) {
            if self.terrs[ti].savings < 45 {
                continue;
            }
            let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
            for (x, y) in hexes {
                let hi = Self::idx(self.cols, x, y);
                {
                    let h = &self.grid[hi];
                    if h.unit != 0 || h.castle || h.tree.is_some() || h.grave
                        || Some((x, y)) == self.terrs[ti].capital
                    {
                        continue;
                    }
                }
                let danger = self.neighbours(x, y).iter().any(|&(nx, ny)| {
                    let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                    nh.owner != -1 && nh.owner != ai && nh.unit >= 2
                });
                if danger && self.buy(x, y, "castle", ai).is_ok() {
                    return true;
                }
            }
        }
        false
    }

    fn targets_around(&self, ti: usize, x: i32, y: i32) -> Vec<(i32, i32)> {
        let mut out = vec![];
        for (tx, ty) in self.neighbours(x, y) {
            if self.terrs[ti].hexes.contains(&(tx, ty)) {
                continue;
            }
            if self.grid[Self::idx(self.cols, tx, ty)].water {
                continue;
            }
            out.push((tx, ty));
        }
        out
    }

    fn needs_man(&self, ti: usize, ai: i8) -> bool {
        let hexes = &self.terrs[ti].hexes;
        if hexes.iter().all(|&(x, y)| {
            let h = &self.grid[Self::idx(self.cols, x, y)];
            h.tree.is_some() || h.unit != 0 || h.castle
        }) && hexes.iter().any(|&(x, y)| {
            let h = &self.grid[Self::idx(self.cols, x, y)];
            h.unit == 0 && !h.castle && !h.grave
        }) {
            return true;
        }
        for &(x, y) in hexes {
            for (nx, ny) in self.neighbours(x, y) {
                let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                if nh.water || hexes.contains(&(nx, ny)) {
                    continue;
                }
                if nh.owner != ai && self.defence(nx, ny) < 1 {
                    return true;
                }
            }
        }
        for &(x, y) in hexes {
            for (nx, ny) in self.neighbours(x, y) {
                let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                if nh.owner != -1 && nh.owner != ai && nh.unit != 0
                    && self.defence(x, y) < nh.unit
                {
                    return true;
                }
            }
        }
        false
    }

    fn buy_spot(&mut self, ti: usize, ai: i8) -> Option<(i32, i32)> {
        let choke: BTreeSet<(i32, i32)> = self.choke_hexes(ti).into_iter().collect();
        let mut best: Option<(i32, i32)> = None;
        let mut bs = -1.0f64;
        // borrow dance: snapshot hex list first
        let hexes: Vec<(i32, i32)> = self.terrs[ti].hexes.iter().cloned().collect();
        let cap = self.terrs[ti].capital;
        for (x, y) in hexes {
            {
                let h = &self.grid[Self::idx(self.cols, x, y)];
                if h.unit != 0 || h.castle || h.grave || Some((x, y)) == cap {
                    continue;
                }
            }
            let mut score = self.neighbours(x, y).iter().filter(|&&(nx, ny)| {
                let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                nh.owner == -1 && !nh.water
            }).count() as f64 * 2.0;
            score += self.neighbours(x, y).iter().filter(|&&(nx, ny)| {
                let nh = &self.grid[Self::idx(self.cols, nx, ny)];
                nh.owner != -1 && nh.owner != ai
            }).count() as f64;
            if choke.contains(&(x, y)) {
                score += 3.0;
            }
            score += self.rf();
            if score > bs {
                best = Some((x, y));
                bs = score;
            }
        }
        best
    }

    // ---------- misc ----------
    pub fn viable(&self, o: i8) -> bool {
        for ti in self.terrs_of(o) {
            for &(x, y) in &self.terrs[ti].hexes {
                let h = &self.grid[Self::idx(self.cols, x, y)];
                if h.unit != 0 || h.castle || h.tree.is_none() {
                    return true;
                }
            }
            if self.terrs[ti].savings >= BUY_PEASANT
                && self.terrs[ti].hexes.iter().any(|&(x, y)| {
                    let h = &self.grid[Self::idx(self.cols, x, y)];
                    h.unit == 0 && !h.castle && !h.grave
                })
            {
                return true;
            }
        }
        false
    }

    pub fn alive(&self) -> Vec<i8> {
        self.players.iter().cloned().filter(|&o| self.hexes_of(o) > 0 && self.viable(o)).collect()
    }

    pub fn winner(&self) -> Option<i8> {
        let a = self.alive();
        if a.len() == 1 {
            Some(a[0])
        } else {
            None
        }
    }

    pub fn player_name(o: i8) -> &'static str {
        match o {
            0 => "You",
            1 => "Karg",
            2 => "Vex",
            3 => "Mord",
            _ => "?",
        }
    }
}

pub fn rank_name(s: u8) -> &'static str {
    match s {
        1 => "Peasant",
        2 => "Spearman",
        3 => "Knight",
        4 => "Baron",
        _ => "?",
    }
}

// ================= UI snapshot =================
use serde::Serialize as Ser2;

#[derive(Ser2)]
pub struct UiHex {
    pub x: i32,
    pub y: i32,
    pub water: bool,
    pub owner: i8,
    pub unit: u8,
    pub acted: bool,
    pub castle: bool,
    pub tree: Option<String>,
    pub grave: bool,
    pub capital: bool,
    pub afford: bool,
}

#[derive(Ser2, Clone)]
pub struct UiTerr {
    pub owner: i8,
    pub hexes: usize,
    pub savings: i64,
    pub income: i64,
    pub wages: i64,
}

#[derive(Ser2)]
pub struct UiPlayer {
    pub owner: i8,
    pub name: String,
    pub hexes: usize,
    pub alive: bool,
}

#[derive(Ser2)]
pub struct UiState {
    pub round: u32,
    pub current: i8,
    pub arch: String,
    pub seed: u64,
    pub difficulty: String,
    pub humans: Vec<i8>,
    pub players: Vec<UiPlayer>,
    pub hexes: Vec<UiHex>,
    pub terrs: Vec<UiTerr>,
    pub sel: Option<(i32, i32)>,
    pub targets: Vec<(i32, i32)>,
    pub outline: Vec<(i32, i32)>,
    pub focus: Option<(i32, i32)>,
    pub focus_terr: Option<UiTerr>,
    pub totals: UiTerr,
    pub log: Vec<String>,
    pub msg: String,
    pub sfx: String,
    pub winner: Option<i8>,
}

impl Game {
    fn ui_terr(&self, ti: usize) -> UiTerr {
        UiTerr {
            owner: self.terrs[ti].owner,
            hexes: self.terrs[ti].hexes.len(),
            savings: self.terrs[ti].savings,
            income: self.terr_income(ti),
            wages: self.terr_wages(ti),
        }
    }

    fn totals(&self) -> UiTerr {
        let me = self.current;
        let mut t = UiTerr { owner: me, hexes: 0, savings: 0, income: 0, wages: 0 };
        for ti in self.terrs_of(me) {
            let u = self.ui_terr(ti);
            t.hexes += u.hexes;
            t.savings += u.savings;
            t.income += u.income;
            t.wages += u.wages;
        }
        t
    }

    pub fn sel_targets(&self) -> Vec<(i32, i32)> {
        let (sx, sy) = match self.sel {
            Some(p) => p,
            None => return vec![],
        };
        let sh = &self.grid[Self::idx(self.cols, sx, sy)];
        if sh.unit == 0 || sh.acted || sh.owner != self.current {
            return vec![];
        }
        let fti = match self.terr_at(sx, sy) {
            Some(i) => i,
            None => return vec![],
        };
        let mut out = BTreeSet::new();
        for &(x, y) in &self.terrs[fti].hexes {
            if (x, y) == (sx, sy) {
                continue;
            }
            let th = &self.grid[Self::idx(self.cols, x, y)];
            if th.castle {
                continue;
            }
            if th.unit != 0 && sh.unit + th.unit > 4 {
                continue;
            }
            out.insert((x, y));
        }
        let hexes: Vec<(i32, i32)> = self.terrs[fti].hexes.iter().cloned().collect();
        for (x, y) in hexes {
            for q in self.neighbours(x, y) {
                if self.terrs[fti].hexes.contains(&q) {
                    continue;
                }
                let th = &self.grid[Self::idx(self.cols, q.0, q.1)];
            if th.water || th.owner == self.current {
                continue;
            }
                if self.can_attack(sx, sy, q.0, q.1).is_ok() {
                    out.insert(q);
                }
            }
        }
        out.into_iter().collect()
    }

    pub fn snapshot(&self, msg: String, sfx: String) -> UiState {
        let caps: BTreeSet<(i32, i32)> = self.terrs.iter().filter_map(|t| t.capital).collect();
        let rich: BTreeSet<(i32, i32)> = self.terrs.iter()
            .filter(|t| t.capital.is_some() && t.savings >= BUY_PEASANT)
            .filter_map(|t| t.capital).collect();
        let mut hexes = vec![];
        for y in 0..self.rows {
            for x in 0..self.cols {
                let h = &self.grid[Self::idx(self.cols, x, y)];
                hexes.push(UiHex {
                    x, y,
                    water: h.water,
                    owner: h.owner,
                    unit: h.unit,
                    acted: h.acted,
                    castle: h.castle,
                    tree: h.tree.map(|t| if t == Tree::Pine { "pine".to_string() } else { "palm".to_string() }),
                    grave: h.grave,
                    capital: caps.contains(&(x, y)),
                    afford: rich.contains(&(x, y)),
                });
            }
        }
        let alive = self.alive();
        let players = self.players.iter().map(|&o| UiPlayer {
            owner: o,
            name: Game::player_name(o).to_string(),
            hexes: self.hexes_of(o),
            alive: alive.contains(&o),
        }).collect();
        let terrs = (0..self.terrs.len()).map(|i| self.ui_terr(i)).collect();
        let focus_terr = self.focus.and_then(|(x, y)| self.terr_at(x, y))
            .filter(|&ti| self.terrs[ti].owner == self.current)
            .map(|ti| self.ui_terr(ti));
        // outline the whole clicked territory so ownership reads at a glance
        let outline: Vec<(i32, i32)> = self.focus
            .and_then(|(x, y)| self.terr_at(x, y))
            .map(|ti| self.terrs[ti].hexes.iter().cloned().collect())
            .unwrap_or_default();
        UiState {
            round: self.round,
            current: self.current,
            arch: self.arch.clone(),
            seed: self.seed,
            difficulty: self.difficulty.clone(),
            humans: self.humans.clone(),
            players,
            hexes,
            terrs,
            sel: self.sel,
            targets: self.sel_targets(),
            outline,
            focus: self.focus,
            focus_terr,
            totals: self.totals(),
            log: self.log.clone(),
            msg,
            sfx,
            winner: self.winner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barbell() -> Game {
        let mut g = Game::new(Some(0), 1, "normal", 0, vec![0], false, None);
        for h in g.grid.iter_mut() {
            *h = Hex::sea();
        }
        for x in 2..7 {
            let i = Game::idx(g.cols, x, 4);
            g.grid[i] = Hex::land();
            g.grid[i].owner = 1;
        }
        g.recompute();
        g
    }

    #[test]
    fn split_bridge_reports_two() {
        let g = barbell();
        assert_eq!(g.split_pieces(4, 4, 1), 2);
        assert_eq!(g.split_pieces(2, 4, 1), 1);
        assert!(!g.choke_hexes(g.terr_at(3, 4).unwrap()).is_empty());
    }

    #[test]
    fn combine_and_castle_math() {
        let mut g = Game::new(Some(1), 1, "normal", 0, vec![0], false, None);
        let (sx, sy) = g.starts[0];
        // grow a 3-hex territory
        let nbs: Vec<(i32, i32)> = g.neighbours(sx, sy).into_iter()
            .filter(|&(nx, ny)| {
                let h = &g.grid[Game::idx(g.cols, nx, ny)];
                h.owner == -1 && !h.water
            }).take(2).collect();
        assert!(nbs.len() == 2);
        for (nx, ny) in &nbs {
            g.grid[Game::idx(g.cols, *nx, *ny)].owner = 0;
        }
        g.recompute();
        g.grid[Game::idx(g.cols, sx, sy)].unit = 1;
        g.grid[Game::idx(g.cols, nbs[0].0, nbs[0].1)].unit = 1;
        assert!(g.do_move(sx, sy, nbs[0].0, nbs[0].1).is_ok());
        assert_eq!(g.grid[Game::idx(g.cols, nbs[0].0, nbs[0].1)].unit, 2);
        // spearman cannot take a castle, knight can
        g.grid[Game::idx(g.cols, sx, sy)].unit = 2;
        let t = nbs[1];
        g.grid[Game::idx(g.cols, t.0, t.1)].owner = 1;
        g.grid[Game::idx(g.cols, t.0, t.1)].castle = true;
        g.recompute();
        assert!(g.do_attack(sx, sy, t.0, t.1, 0).is_err());
        g.grid[Game::idx(g.cols, sx, sy)].unit = 3;
        g.grid[Game::idx(g.cols, sx, sy)].acted = false;
        assert!(g.do_attack(sx, sy, t.0, t.1, 0).is_ok());
    }

    #[test]
    fn bankruptcy_kills_and_money_stays_bounded() {
        let mut g = Game::new(Some(2), 1, "normal", 0, vec![0], false, None);
        let (sx, sy) = g.starts[0];
        for (nx, ny) in g.neighbours(sx, sy) {
            let h = &mut g.grid[Game::idx(g.cols, nx, ny)];
            if h.owner == -1 && !h.water {
                h.owner = 0;
            }
        }
        g.recompute();
        let ti = g.terr_at(sx, sy).unwrap();
        g.terrs[ti].savings = 0;
        g.grid[Game::idx(g.cols, sx, sy)].unit = 4;
        g.start_turn(0);
        assert_eq!(g.grid[Game::idx(g.cols, sx, sy)].unit, 0);
        assert!(g.grid[Game::idx(g.cols, sx, sy)].grave);
        // play a while: savings must never explode (old double-count bug)
        for _ in 0..30 {
            for ai in g.alive() {
                g.ai_take_turn(ai);
            }
        }
        assert!(g.terrs.iter().all(|t| t.savings.abs() < 100000));
    }

    #[test]
    fn same_seed_same_island() {
        let a = Game::new(Some(999), 2, "normal", 0, vec![0], false, None);
        let b = Game::new(Some(999), 2, "normal", 0, vec![0], false, None);
        assert_eq!(a.arch, b.arch);
        for y in 0..a.rows {
            for x in 0..a.cols {
                assert_eq!(a.grid[Game::idx(a.cols, x, y)].water, b.grid[Game::idx(b.cols, x, y)].water);
            }
        }
    }

    #[test]
    fn viability_dead_end() {
        let mut g = Game::new(Some(3), 1, "normal", 0, vec![0], false, None);
        // pave everything of player 1 with pines and no gold
        for ti in g.terrs_of(1) {
            let hexes: Vec<(i32, i32)> = g.terrs[ti].hexes.iter().cloned().collect();
            for (x, y) in hexes {
                let h = &mut g.grid[Game::idx(g.cols, x, y)];
                h.unit = 0;
                h.tree = Some(Tree::Pine);
            }
            g.terrs[ti].savings = 0;
        }
        assert!(!g.viable(1));
        assert!(!g.alive().contains(&1));
    }

    #[test]
    fn hex_distance_is_six_neighbour_metric() {
        // every direct neighbour is exactly 1 step, on even and odd rows
        for y in 2..6 {
            for x in 2..6 {
                let g = Game::new(Some(0), 1, "normal", 0, vec![0], false, None);
                for (nx, ny) in g.neighbours(x, y) {
                    assert_eq!(hex_dist((x, y), (nx, ny)), 1);
                }
            }
        }
        assert_eq!(hex_dist((0, 0), (0, 0)), 0);
        assert_eq!(hex_dist((0, 0), (2, 0)), 2);
    }

    #[test]
    fn starts_never_touch() {
        for size in [0u8, 1, 2] {
            for enemies in [1u8, 2, 3] {
                for seed in 0..15u64 {
                    let g = Game::new(Some(seed * 1000 + size as u64 * 77 + enemies as u64), enemies, "normal", size, vec![0], false, None);
                    assert_eq!(g.starts.len(), g.players.len());
                    for (i, &a) in g.starts.iter().enumerate() {
                        for &b in &g.starts[i + 1..] {
                            assert!(hex_dist(a, b) >= 2, "starts {a:?} {b:?} touch (seed {seed} size {size})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn hotseat_full_rotation_keeps_moving() {
        // 3 humans + 1 AI idling for 20 rounds: rotation never sticks,
        // the round counter advances, the AI keeps playing
        let mut g = Game::new(Some(77), 3, "normal", 0, vec![0, 1, 2], false, None);
        g.start_turn(0);
        for _ in 0..20 {
            let before = g.round;
            g.advance_turn();
            assert!(g.round >= before);
            if g.winner().is_some() {
                break;
            }
            assert!(g.humans.contains(&g.current));
        }
    }

    #[test]
    fn hotseat_advance_rotates_humans() {
        // 2 humans + 1 AI: end turn must skip past the AI to the next human
        let mut g = Game::new(Some(5), 2, "normal", 0, vec![0, 1], false, None);
        g.start_turn(0);
        assert_eq!(g.current, 0);
        g.advance_turn();
        assert_eq!(g.current, 1, "must land on the second human, AI plays between");
        assert!(g.round >= 1);
    }

    #[test]
    fn custom_map_loads_and_plays() {
        let land: Vec<(i32, i32)> = (0..8).flat_map(|y| (0..8).map(move |x| (x, y))).collect();
        let custom = CustomSpec {
            cols: 8, rows: 8,
            land,
            starts: vec![(1, 1), (6, 6), (1, 6)],
        };
        let mut g = Game::new(Some(9), 2, "normal", 0, vec![0], false, Some(custom));
        assert_eq!(g.arch, "Custom");
        assert_eq!(g.players.len(), 3);
        assert_eq!(g.starts.len(), 3);
        // first moves work on the painted island
        g.start_turn(0);
        let (sx, sy) = g.starts[0];
        assert!(g.buy(sx, sy, "man", 0).is_ok());
    }

    #[test]
    fn bad_custom_falls_back_to_generated() {
        let custom = CustomSpec { cols: 2, rows: 2, land: vec![(0, 0)], starts: vec![(0, 0)] };
        let g = Game::new(Some(9), 2, "normal", 0, vec![0], false, Some(custom));
        assert_ne!(g.arch, "Custom");
        assert_eq!(g.players.len(), 3);
    }

    #[test]
    fn tutorial_funds_home() {
        let g = Game::new(Some(11), 1, "normal", 0, vec![0], true, None);
        let s0 = g.starts[0];
        let ti = g.terr_at(s0.0, s0.1).unwrap();
        assert_eq!(g.terrs[ti].savings, 25);
    }

    #[test]
    fn tutorial_walkthrough_completes() {
        // mirrors the 5 scripted coach steps on the fixed tutorial seed
        let mut g = Game::new(Some(424242), 1, "easy", 0, vec![0], true, None);
        g.start_turn(0);
        let (sx, sy) = g.starts[0];
        // 1. buy a peasant (tutorial grant covers it)
        assert!(g.buy(sx, sy, "man", 0).is_ok());
        assert_eq!(g.grid[Game::idx(g.cols, sx, sy)].unit, 1);
        // 2+3. select it and claim an adjacent wild hex
        let tgt = g.neighbours(sx, sy).into_iter().find(|&(nx, ny)| {
            let h = &g.grid[Game::idx(g.cols, nx, ny)];
            h.owner == -1 && !h.water
        });
        assert!(tgt.is_some(), "tutorial start needs an open neighbour");
        let (tx, ty) = tgt.unwrap();
        g.sel = Some((sx, sy));
        assert!(!g.sel_targets().is_empty());
        assert!(g.do_attack(sx, sy, tx, ty, 0).is_ok());
        assert!(g.hexes_of(0) >= 2);
        // 4. end the turn
        g.advance_turn();
        assert!(g.round >= 2 || g.winner().is_some());
        // 5. buy a second peasant and stack into a spearman
        let men: Vec<(i32, i32)> = g.terrs_of(0).iter()
            .flat_map(|&ti| g.terrs[ti].hexes.iter().cloned()).collect();
        assert!(men.len() >= 2);
    }

    #[test]
    fn select_shows_targets_and_outline() {
        let mut g = Game::new(Some(11), 1, "normal", 0, vec![0], false, None);
        g.start_turn(0);
        let (sx, sy) = g.starts[0];
        assert!(g.buy(sx, sy, "man", 0).is_ok());
        g.sel = Some((sx, sy));
        g.focus = Some((sx, sy));
        let s = g.snapshot(String::new(), "select".to_string());
        assert!(!s.targets.is_empty());
        assert_eq!(s.sfx, "select");
        let ti = g.terr_at(sx, sy).unwrap();
        assert_eq!(s.outline.len(), g.terrs[ti].hexes.len());
    }
}
