//! What an atlas tree is worth for a plan: the atlas builder's maths (search in `atlas_search.rs`, asking in `atlas_opt.rs`).
//!
//! - `data/atlas_effects.json`: what each node changes, as channels with a number (tools/build_atlas_effects.mjs, from the node texts).
//! - `data/atlas_model.json`: where income comes from and which channels each source pays more for. Income of a source =
//!   unit x how many x product of (1 + weight x channel), so things multiply or overlap by themselves. Reasons in ATLAS_KNOWLEDGE.md.
//! - `data/atlas_shares.json`: what people farming each mechanic take (tools/build_atlas_shares.mjs). It carries the nodes the
//!   maths has no number for, and keeps the rest honest.
//!
//! Only comparisons mean anything here: never show the numbers as chaos per hour.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

const EFFECTS: &str = include_str!("../data/atlas_effects.json");
const MODEL: &str = include_str!("../data/atlas_model.json");
const SHARES: &str = include_str!("../data/atlas_shares.json");

/// How much what experts take counts next to the maths (tests: 6 does best on real trees, 2-20 is nearly flat).
const LAMBDA: f64 = 6.0;
/// Celestial Presence: which altars you get, so nothing without altars.
const CELESTIAL: u32 = 27261;

#[derive(Debug, Deserialize)]
struct Effect {
    c: String,
    v: f64,
    per: Option<String>,
    #[serde(default)]
    need: Vec<String>,
}
#[derive(Debug, Deserialize)]
struct Opt { id: u32, e: Vec<Effect> }
#[derive(Debug, Deserialize)]
struct NodeFx { e: Vec<Effect>, #[serde(default)] o: Vec<Opt> }
#[derive(Debug, Deserialize)]
struct Effects { nodes: HashMap<u32, NodeFx> }

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MapStats { pub mods: f64, pub pack: f64, pub drops: f64, pub rarity: f64, pub currency: f64 }

#[derive(Debug, Deserialize)]
struct Spawn { base: f64, ch: String, mode: String, cap: Option<f64> }
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    label: String,
    unit: f64,
    #[serde(default)]
    min: f64,
    mech: Option<String>,
    thread: Option<String>,
    side: Option<String>,
    #[serde(default)]
    count: Option<f64>,
    count_of: Option<String>,
    spawn: Option<Spawn>,
    maphas: Option<String>,
    needs: Option<String>,
    #[serde(default)]
    always: bool,
    #[serde(default, rename = "thread_steer")]
    thread_steer: bool,
    factors: Vec<(String, f64)>,
    #[serde(default)]
    conf: String,
}
#[derive(Debug, Deserialize)]
struct Juice { base: f64, uplift: f64 }
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Goal {
    pub label: String,
    pub pick: Vec<String>,
    #[serde(default)]
    pub prior: Vec<String>,
    #[serde(default)]
    pub boost: HashMap<String, f64>,
    #[serde(default)]
    pub terror: bool,
    #[serde(default)]
    pub maven: bool,
    #[serde(default)]
    pub rush: bool,
    #[serde(default)]
    pub lock: Vec<u32>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Mode { pub when: String, pub node: u32, pub option: u32, pub label: String }

#[derive(Debug, Deserialize)]
pub struct Model {
    per: HashMap<String, f64>,
    pub maps: HashMap<String, MapStats>,
    globals: HashMap<String, f64>,
    pub thread_prices: BTreeMap<String, f64>,
    juice: BTreeMap<String, Juice>,
    danger: BTreeMap<String, f64>,
    pub goals: BTreeMap<String, Goal>,
    mechanics: HashMap<String, Vec<String>>,
    shares_as: HashMap<String, String>,
    wants: HashMap<String, Vec<String>>,
    /// How much a source's farm lives off the map's own monsters (1) or only wants more of itself (0.1).
    juice_use: HashMap<String, f64>,
    pub primaries: Vec<String>,
    pub cheap_threads: Vec<String>,
    pub modes: Vec<Mode>,
    pub clash: Vec<(u32, u32, String)>,
    pub never: HashMap<u32, String>,
    pub defaults: HashMap<u32, u32>,
    pub option_items: HashMap<u32, HashMap<u32, String>>,
    pub tips: Vec<(Vec<String>, String)>,
    sources: BTreeMap<String, Source>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MechShare { pub w: f64, pub n: HashMap<u32, f64> }
/// What people take: share of all trees, and of the trees that farm each mechanic.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Shares {
    #[serde(default)]
    pub family: HashMap<String, Vec<u32>>,
    #[serde(default)]
    pub general: Vec<String>,
    pub all: HashMap<u32, f64>,
    pub mechs: HashMap<String, MechShare>,
}

fn effects() -> &'static Effects { static D: OnceLock<Effects> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(EFFECTS).expect("atlas_effects.json")) }
pub fn model() -> &'static Model { static D: OnceLock<Model> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(MODEL).expect("atlas_model.json")) }
pub fn shares() -> &'static Shares { static D: OnceLock<Shares> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(SHARES).expect("atlas_shares.json")) }
/// What each node is about ("legion", "rare monsters", "eater"…).
pub fn family() -> &'static HashMap<u32, &'static str> {
    static D: OnceLock<HashMap<u32, &'static str>> = OnceLock::new();
    D.get_or_init(|| shares().family.iter().flat_map(|(f, ids)| ids.iter().map(move |i| (*i, f.as_str()))).collect())
}
/// Families that help whatever you farm (pack size, rares, altars…), not a mechanic of their own.
fn general(f: &str) -> bool { shares().general.iter().any(|g| g == f) }
fn glob(k: &str) -> f64 { model().globals.get(k).copied().unwrap_or(0.0) }

/// The mechanic as atlas_shares.json knows it.
pub fn shares_key(m: &str) -> &str { model().shares_as.get(m).map(|s| s.as_str()).unwrap_or(m) }
/// The income sources an asked-for mechanic means.
pub fn sources_of(m: &str) -> Vec<String> { model().mechanics.get(m).cloned().unwrap_or_else(|| vec![m.to_string()]) }

/// The plan a tree is built for.
#[derive(Debug, Clone)]
pub struct Setup {
    /// Income sources / mechanics that count fully.
    pub pick: HashSet<String>,
    /// Mechanics to look up what people take.
    pub mechs: Vec<String>,
    /// "pack size", "threads"…: nodes for them count extra.
    pub wants: Vec<String>,
    pub threads: Vec<(String, u32)>,
    /// What every map has anyway (bought or rolled): "mirage", "delirium"…
    pub map_has: Vec<String>,
    pub map: MapStats,
    /// "eater", "exarch" or none.
    pub eldritch: Option<String>,
    pub maven: bool,
    pub terror: bool,
    pub rush: bool,
    /// 0 weak to 1 strong: how much a dangerous node costs you.
    pub strength: f64,
    pub boost: HashMap<String, f64>,
    /// Nodes always in, with the option when it's a choice.
    pub lock: Vec<(u32, Option<u32>)>,
    pub block: HashSet<u32>,
    /// Today's Thread prices ("Legion" -> chaos); the shipped ones otherwise.
    pub prices: BTreeMap<String, f64>,
}

// Amounts many sources share.
const DER: [&str; 18] = ["E", "pack", "drops", "rarity", "magic", "rare", "rmods", "mix", "altar", "juice", "hard", "thrB", "mapB", "threadmods", "shrineN", "bossN", "altarboss", "evolve"];
const E: usize = 0; const PACK: usize = 1; const DROPS: usize = 2; const RARITY: usize = 3; const MAGIC: usize = 4; const RARE: usize = 5; const RMODS: usize = 6; const MIX: usize = 7;
const ALTAR: usize = 8; const JUICE: usize = 9; const HARD: usize = 10; const THRB: usize = 11; const MAPB: usize = 12; const THREADMODS: usize = 13; const SHRINES: usize = 14; const BOSSES: usize = 15;
const ALTARBOSS: usize = 16; const EVOLVE: usize = 17;
/// Said in plain words; "" = not worth saying by itself.
const DER_SAID: [&str; 18] = ["", "more monsters", "more drops that follow the map's bonuses", "better drops", "more magic monsters", "more rare monsters", "rares with an extra modifier", "",
    "stronger altars", "a better chance of a bonus mechanic", "tougher, richer packs", "", "", "rares drop more Threads", "more shrines", "more bosses", "altars that pay on bosses", ""];

/// One way to take a node: its option (0 = none) and what it adds.
#[derive(Debug, Clone)]
pub struct Alt { pub id: u32, pub d: Vec<(usize, f64)> }

enum How { Fixed(f64), Surpass(f64, usize), Inc(f64, usize), Chance(f64, usize, f64), Count(usize) }
struct Src {
    key: &'static str,
    label: &'static str,
    unit: f64,
    weight: f64,
    min: f64,
    how: How,
    side: Option<bool>,
    thread: Option<&'static str>,
    steer: bool,
    der: Vec<(usize, f64)>,
    ch: Vec<(usize, f64)>,
    quick: bool,
    guess: bool,
}

/// One source in a tree: label, income, how many.
#[derive(Debug, Clone)]
pub struct SrcOut { pub key: &'static str, pub label: &'static str, pub income: f64, pub n: f64, pub guess: bool }
#[derive(Debug, Clone, Default)]
pub struct Detail { pub src: Vec<SrcOut>, pub total: f64, pub q: [f64; 18] }

#[derive(Default)]
struct Chans { ix: HashMap<String, usize> }
impl Chans { fn get(&mut self, n: &str) -> usize { if let Some(i) = self.ix.get(n) { return *i; } let i = self.ix.len(); self.ix.insert(n.to_string(), i); i } }

struct C { per_kind: usize, per_mod: usize, eff: usize, pre: usize, suf: usize, xmods: usize, pack: usize, hard: usize, eld: usize, rarity: usize, magic: usize, mpack: usize, mpk: usize, rare: usize, rdup: usize,
    rmir: usize, rmods: usize, a_eater: usize, a_exarch: usize, a_up: usize, a_extra: usize, pick_p: usize, pick_b: usize, sh: usize, shapp: usize, shbuff: usize, shextra: usize, maven: usize, mode_u: usize,
    mode_h: usize, t_found: usize, m_found: usize, m_per: usize, t_lim: usize, keep: usize, per_rm: usize, evolve: usize, prior: usize }

/// A plan ready to score trees: every node's effects resolved for it.
pub struct Problem {
    pub ids: Vec<u32>,
    pub alts: Vec<Vec<Alt>>,
    /// What people take, per node (already in the alts as a channel; kept for saying why).
    pub setup: Setup,
    pub k: usize,
    c: C,
    src: Vec<Src>,
    juice: Vec<(usize, f64, f64, bool)>,
    danger: Vec<(usize, f64)>,
    steer: Vec<(usize, usize, f64)>,
    avg_price: f64,
    thr: Vec<(&'static str, f64, f64)>,
}

fn leak(s: &str) -> &'static str {
    // Labels and keys come from the shipped model: each once.
    static D: OnceLock<std::sync::Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut g = D.get_or_init(Default::default).lock().unwrap();
    if let Some(x) = g.get(s) { return x; }
    let x: &'static str = Box::leak(s.to_string().into_boxed_str());
    g.insert(x);
    x
}

fn need_met(n: &str, s: &Setup) -> bool {
    match n.split_once(':') {
        Some(("eldritch", side)) => s.eldritch.as_deref() == Some(side),
        Some(("influence", _)) | Some(("map", _)) => false,
        Some(("maven", _)) => s.maven,
        _ => true,
    }
}

impl Problem {
    pub fn new(setup: Setup, shares: &Shares) -> Problem {
        let m = model();
        let mut ch = Chans::default();
        let side = setup.eldritch.clone().unwrap_or_else(|| "none".into());
        let mut ids: Vec<u32> = crate::atlas::graph().ids();
        ids.sort();
        let prior_ch = ch.get("prior");
        let prior = prior(&setup, shares, &ids);
        let wants: HashSet<&str> = setup.wants.iter().flat_map(|w| m.wants.get(w).into_iter().flatten()).map(|s| s.as_str()).collect();

        let deltas = |list: &[Effect], ch: &mut Chans| -> Vec<(usize, f64)> {
            let mut out: Vec<(usize, f64)> = vec![];
            for e in list {
                if !e.need.iter().all(|n| need_met(n, &setup)) { continue; }
                // Four voidstones: maps are tier 16 already.
                if e.c.starts_with("altars.") && setup.eldritch.is_none() { continue; }
                if e.c == "mapdrops.tier_up" || e.c == "mapdrops.portal" || e.c.starts_with("labyrinth.") { continue; }
                let mut v = e.v;
                if e.c == "mapdrops.found_per_mod" { v *= setup.map.mods; }
                if let Some(p) = &e.per { v *= m.per.get(p).copied().unwrap_or(1.0); }
                let name = match e.need.iter().find_map(|n| n.strip_prefix("eldritch:")) { Some(s) => format!("{}@{s}", e.c), None => e.c.clone() };
                if wants.contains(e.c.as_str()) { out.push((ch.get("wanted"), 1.0)); }
                out.push((ch.get(&name), v));
            }
            out
        };
        let mut alts: Vec<Vec<Alt>> = ids.iter().map(|id| match effects().nodes.get(id) {
            None => vec![Alt { id: 0, d: vec![] }],
            Some(n) => {
                let base = deltas(&n.e, &mut ch);
                if n.o.is_empty() { vec![Alt { id: 0, d: base }] }
                else { n.o.iter().map(|o| { let mut d = base.clone(); d.extend(deltas(&o.e, &mut ch)); Alt { id: o.id, d } }).collect() }
            }
        }).collect();
        for (i, a) in alts.iter_mut().enumerate() {
            // Nodes of a side that isn't in play get nothing from popularity either.
            let dead = effects().nodes.get(&ids[i]).is_some_and(|n| !n.e.is_empty() && n.e.iter().all(|e| !e.need.iter().all(|x| need_met(x, &setup))));
            let dead = dead || (ids[i] == CELESTIAL && setup.eldritch.is_none());
            if dead || prior[i] <= 0.0 { continue; }
            for o in a.iter_mut() { o.d.push((prior_ch, prior[i])); }
        }
        let c = C { per_kind: ch.get("explicit.per_kind"), per_mod: ch.get("explicit.per_mod"), eff: ch.get("explicit.effect"), pre: ch.get("explicit.effect_prefix"), suf: ch.get("explicit.effect_suffix"),
            xmods: ch.get("map.extra_map_mods"), pack: ch.get("pack.pack"), hard: ch.get("pack.hard_packs"), eld: ch.get(&format!("eldritch.pack@{side}")), rarity: ch.get("rarity.rarity"), magic: ch.get("magics.magic"),
            mpack: ch.get("magics.magic_pack"), mpk: ch.get("magics.magic_packs"), rare: ch.get("rares.rare"), rdup: ch.get("rares.dup_rares"), rmir: ch.get("rares.mirrored_rares"), rmods: ch.get("rares.rare_mods"),
            a_eater: ch.get("altars.chance@eater"), a_exarch: ch.get("altars.chance@exarch"), a_up: ch.get(&format!("altars.upside@{side}")), a_extra: ch.get(&format!("altars.extra_upside@{side}")),
            pick_p: ch.get("altars.pick_players"), pick_b: ch.get("altars.pick_bosses"), sh: ch.get("shrines.spawn"), shapp: ch.get("shrines.apparition"), shbuff: ch.get("shrines.buff"), shextra: ch.get("shrines.extra_effect"),
            maven: ch.get("maven.extra"), mode_u: ch.get("breach.mode@unstable"), mode_h: ch.get("breach.mode@hive"), t_found: ch.get("threads.found"), m_found: ch.get("mapdrops.found"), m_per: ch.get("mapdrops.found_per_mod"),
            t_lim: ch.get("threads.thread_limit"), keep: ch.get("threads.thread_keep"), per_rm: ch.get("threads.per_mod"), evolve: ch.get("shrines.evolving"), prior: prior_ch };
        let price = |t: &str| -> f64 { let k = cap(t); setup.prices.get(&k).or(m.thread_prices.get(&k)).copied().unwrap_or(0.35) };
        let thr: Vec<(&'static str, f64, f64)> = setup.threads.iter().map(|(t, n)| (leak(t), *n as f64, price(t))).collect();
        let juiced = setup.pick.iter().filter_map(|p| m.juice_use.get(p)).copied().fold(0.0f64, f64::max).max(if setup.pick.is_empty() { 1.0 } else { 0.1 });
        let src = m.sources.iter().filter_map(|(key, s)| {
            let mech = s.mech.as_deref().unwrap_or(key);
            match s.needs.as_deref() { Some("maven") if !setup.maven => return None, Some("rush") if !setup.rush => return None, _ => {} }
            if let Some(h) = &s.maphas { if !setup.map_has.contains(h) && !setup.pick.contains(mech) { return None; } }
            if let Some(t) = &s.thread { if !thr.iter().any(|x| x.0 == t) { return None; } }
            let picked = setup.pick.contains(mech) || setup.pick.contains(key);
            let weight = if picked { 1.0 } else if key == "monsters" { 0.8 * juiced } else if s.always && mech == "bosses" { 0.3 } else if ["threads", "maps", "gold", "cards"].contains(&key.as_str()) { 0.6 } else if s.always { 0.6 } else { 0.1 };
            let how = if let Some(sp) = &s.spawn {
                let i = ch.get(&sp.ch);
                match sp.mode.as_str() { "surpass" => How::Surpass(sp.base, i), "inc" => How::Inc(sp.base, i), _ => How::Chance(sp.base, i, sp.cap.unwrap_or(1.0)) }
            } else if let Some(q) = &s.count_of { How::Count(DER.iter().position(|d| d == q).expect("known amount")) } else { How::Fixed(s.count.unwrap_or(1.0)) };
            let (mut der, mut chans) = (vec![], vec![]);
            for (f, w) in &s.factors {
                if *w == 0.0 { continue; }
                match DER.iter().position(|d| d == f) { Some(i) => der.push((i, *w)), None => chans.push((ch.get(f), *w)) }
            }
            Some(Src { key: leak(key), label: leak(&s.label), unit: s.unit * setup.boost.get(key).copied().unwrap_or(1.0), weight, min: s.min, how,
                side: s.side.as_deref().map(|x| x == "unstable"), thread: s.thread.as_deref().map(leak), steer: s.thread_steer, der, ch: chans, quick: picked || s.always, guess: s.conf == "guess" })
        }).collect();
        let juice = m.juice.iter().map(|(k, j)| (ch.get(&format!("{k}.spawn")), j.base, j.uplift, setup.map_has.contains(k))).collect();
        let danger = m.danger.iter().map(|(k, w)| (ch.get(k), *w)).collect();
        let steer: Vec<(usize, usize, f64)> = m.thread_prices.keys().map(|t| (ch.get(&format!("steer:+{t}")), ch.get(&format!("steer:-{t}")), price(t))).collect();
        let avg_price = steer.iter().map(|s| s.2).sum::<f64>() / steer.len().max(1) as f64;
        let wanted = ch.get("wanted");
        debug_assert!(wanted < ch.ix.len());
        Problem { ids, alts, setup, k: ch.ix.len(), c, src, juice, danger, steer, avg_price, thr }.with_wanted(wanted)
    }

    /// "Also spend points on": counted with what people take, so it never drowns the maths.
    fn with_wanted(mut self, wanted: usize) -> Problem {
        let g = crate::atlas::graph();
        for (i, alts) in self.alts.iter_mut().enumerate() {
            let bonus = if matches!(g.kind(self.ids[i]), 1 | 2) { 2.0 } else { 0.6 } / 150.0;
            for a in alts {
                let wants = a.d.iter().any(|(k, _)| *k == wanted);
                a.d.retain(|(k, _)| *k != wanted);
                if wants { a.d.push((self.c.prior, bonus)); }
            }
        }
        self
    }

    /// How good a tree with these channel sums is (log scale).
    pub fn eval(&self, x: &[f64]) -> f64 { self.worth(x, None) }

    pub fn detail(&self, x: &[f64]) -> Detail { let mut d = Detail::default(); self.worth(x, Some(&mut d)); d }

    fn worth(&self, x: &[f64], mut det: Option<&mut Detail>) -> f64 {
        let (c, s) = (&self.c, &self.setup);
        let m = &s.map;
        let mut kinds = self.thr.len() as f64;
        let mut only: Option<&str> = None;
        if x[c.t_lim] > 0.0 {
            kinds = kinds.min(1.0);
            only = self.thr.iter().find(|t| s.pick.contains(t.0)).map(|t| t.0);
        }
        let mods = m.mods + x[c.xmods];
        let eff = x[c.eff] + x[c.pre] * glob_c().prefix + x[c.suf] * (1.0 - glob_c().prefix) + x[c.per_kind] * kinds + x[c.per_mod] * mods;
        let g = glob_c();
        let mm = (1.0 + x[c.xmods] * g.extra_mod / m.mods.max(1.0)) * (1.0 + eff);
        let mut q = [0.0f64; 18];
        q[E] = 1.0 + eff;
        let side = if s.eldritch.is_some() { g.influenced } else { 0.0 };
        q[PACK] = (1.0 + m.pack / 100.0 * mm + x[c.pack] + x[c.eld] * side) / (1.0 + m.pack / 100.0);
        let drops0 = 1.0 + m.drops / 100.0;
        q[DROPS] = (1.0 + m.drops / 100.0 * mm) / drops0;
        // "Increased Threads / Maps found" adds to Atlas Drops instead of multiplying it.
        q[THRB] = (1.0 + m.drops / 100.0 * mm + x[c.t_found]) / drops0;
        q[MAPB] = (1.0 + m.drops / 100.0 * mm + x[c.m_found] + x[c.m_per]) / drops0;
        q[RARITY] = ((1.0 + m.rarity / 100.0 * mm + x[c.rarity]) / (1.0 + m.rarity / 100.0)).powf(g.rarity_power) * (1.0 + m.currency / 100.0 * mm) / (1.0 + m.currency / 100.0);
        q[MAGIC] = (1.0 + x[c.magic] + x[c.mpk] * 0.03) * (1.0 + x[c.mpack]);
        q[RARE] = 1.0 + x[c.rare] + (x[c.rdup] + x[c.rmir] * 2.0) / g.rare_base * g.added_rare;
        q[RMODS] = 1.0 + g.per_rare_mod * x[c.rmods];
        q[MIX] = g.normal + g.magic * q[MAGIC] + g.rare * q[RARE] * q[RMODS];
        q[HARD] = 1.0 + g.hard * x[c.hard];
        // Rushing to the boss passes most altars.
        let altars = if s.rush { 0.25 } else { 1.0 } * (1.0 + x[c.a_eater] + x[c.a_exarch] / g.altars) * (1.0 + x[c.a_up]) * (1.0 + x[c.a_extra]);
        q[ALTAR] = if s.eldritch.is_some() { 1.0 + g.altar * altars * if x[c.pick_p] > 0.0 { g.altar_players } else { 1.0 } } else { 1.0 };
        q[ALTARBOSS] = if x[c.pick_b] > 0.0 && s.eldritch.is_some() { altars } else { 0.0 };
        let mut j = 1.0;
        for (k, base, up, has) in &self.juice { j *= 1.0 + if *has { 1.0 } else { (base * (1.0 + x[*k])).min(1.0) } * up; }
        q[JUICE] = j;
        q[THREADMODS] = 1.0 + x[c.per_rm] * (g.rare_mods + x[c.rmods]);
        q[SHRINES] = 0.6 + x[c.sh] + x[c.shapp] + if s.terror { 8.0 } else { 0.0 };
        q[BOSSES] = 1.0 + if s.maven { x[c.maven] } else { 0.0 } + if s.terror { q[SHRINES] } else { 0.0 };
        q[EVOLVE] = x[c.evolve] * (q[SHRINES] - if s.terror { 8.0 } else { 0.0 }).clamp(0.0, 1.0);
        let unstable = if x[c.mode_u] > 0.0 { 1.0 } else if x[c.mode_h] > 0.0 { 0.0 } else { 0.5 };

        let (mut v, mut t) = (0.0, g.base_min * if s.rush { 0.5 } else { 1.0 });
        for sr in &self.src {
            let mut n = match sr.how {
                How::Fixed(n) => n,
                How::Surpass(b, i) => b + x[i],
                How::Inc(b, i) => (b * (1.0 + x[i])).min(1.0),
                How::Chance(b, i, cap) => (b + x[i]).min(cap),
                How::Count(d) => q[d],
            };
            if let Some(th) = sr.thread {
                if only.is_some_and(|o| o != th) { continue; }
                let have = self.thr.iter().find(|y| y.0 == th).map(|y| y.1).unwrap_or(0.0);
                n *= 0.65 + 0.07 * have.min(8.0);
                if only.is_some() { n *= 1.02; }
            }
            if let Some(u) = sr.side { n *= if u { unstable } else { 1.0 - unstable }; }
            if n <= 0.0 { continue; }
            let mut inc = sr.unit * n;
            if s.rush { match sr.how { How::Count(_) => {} How::Fixed(_) => inc *= 0.3, _ => inc *= if sr.quick { 0.7 } else { 0.2 } } }
            for &(d, w) in &sr.der { inc *= if d == ALTARBOSS || d == SHRINES { 1.0 + w * q[d] } else { 1.0 + w * (q[d] - 1.0) }; }
            for &(i, w) in &sr.ch { inc *= (1.0 + w * x[i]).max(0.05); }
            if sr.steer {
                let (mut sw, mut sp) = (0.0, 0.0);
                for &(plus, minus, p) in &self.steer { let w = if x[minus] > 0.0 { 0.0 } else { 1.0 + x[plus] }; sw += w; sp += w * p; }
                if sw > 0.0 { inc *= sp / sw / self.avg_price; }
            }
            t += sr.min * n * if sr.weight >= 1.0 { 1.0 } else { 0.3 };
            v += inc * sr.weight;
            if let Some(d) = det.as_deref_mut() { d.src.push(SrcOut { key: sr.key, label: sr.label, income: inc * sr.weight, n, guess: sr.guess }); }
        }
        // Threads you spend: Subtle Manipulations is worth its share of the bill.
        let keep = x[c.keep].min(1.0);
        let bill: f64 = self.thr.iter().filter(|y| only.is_none_or(|o| o == y.0)).map(|y| y.1 * y.2).sum();
        v = (v - bill * (1.0 - keep)).max(1.0);
        let danger: f64 = self.danger.iter().map(|(k, w)| w * x[*k]).sum();
        t /= 1.0 + g.shrine_speed * q[SHRINES].min(g.shrine_cap) * (1.0 + x[c.shbuff] + x[c.shextra] * 0.5);
        let u = v.ln() - danger * (1.0 - s.strength) - t.ln();
        if let Some(d) = det { d.total = v; d.q = q; }
        u + LAMBDA * x[c.prior]
    }

    /// The channel what-people-take sits in (to tell the maths' gain apart).
    pub fn prior_channel(&self) -> usize { self.c.prior }
    pub fn lambda() -> f64 { LAMBDA }
}

/// What changed between a tree with a node and without it, in plain words ("more monsters, better drops").
pub fn said(with: &Detail, without: &Detail) -> Vec<&'static str> {
    let mut v: Vec<(f64, &'static str)> = (0..18).filter(|i| !DER_SAID[*i].is_empty() && without.q[*i] != 0.0)
        .map(|i| (with.q[i] / without.q[i] - 1.0, DER_SAID[i])).filter(|(d, _)| *d > 0.003).collect();
    if with.q[ALTARBOSS] > 0.0 && without.q[ALTARBOSS] == 0.0 { v.push((1.0, DER_SAID[ALTARBOSS])); }
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v.into_iter().map(|x| x.1).collect()
}

pub fn cap(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }

struct Globals { prefix: f64, extra_mod: f64, influenced: f64, rarity_power: f64, rare_base: f64, added_rare: f64, per_rare_mod: f64, normal: f64, magic: f64, rare: f64, hard: f64, altars: f64, altar: f64,
    altar_players: f64, rare_mods: f64, base_min: f64, shrine_speed: f64, shrine_cap: f64 }
fn glob_c() -> &'static Globals {
    static D: OnceLock<Globals> = OnceLock::new();
    D.get_or_init(|| Globals { prefix: 0.6, extra_mod: glob("extraModQuality"), influenced: glob("influencedShare"), rarity_power: glob("rarityPower"), rare_base: glob("rareBase"), added_rare: glob("addedRareWeight"),
        per_rare_mod: glob("lootPerRareMod"), normal: glob("shareNormal"), magic: glob("shareMagic"), rare: glob("shareRare"), hard: glob("hardPacks"), altars: glob("altarsBase"), altar: glob("altarBase"),
        altar_players: glob("altarPlayerPick"), rare_mods: glob("rareMods"), base_min: 3.0, shrine_speed: glob("shrineSpeed"), shrine_cap: glob("shrineCap") })
}

/// What people take, per node, for these mechanics: 150 points of the best sum to about 1.
fn prior(s: &Setup, sh: &Shares, ids: &[u32]) -> Vec<f64> {
    let fam = family();
    let g = crate::atlas::graph();
    let m = model();
    let primaries: HashSet<&str> = m.primaries.iter().map(|p| p.as_str()).collect();
    let empty = MechShare::default();
    let mut v: Vec<f64> = ids.iter().map(|id| {
        if m.never.contains_key(id) { return 0.0; }
        let notable = matches!(g.kind(*id), 1 | 2);
        let f = fam.get(id).copied().unwrap_or("other");
        let all = sh.all.get(id).copied().unwrap_or(0.0);
        let of = |k: &str| { let c = sh.mechs.get(k).unwrap_or(&empty); (c.n.get(id).copied().unwrap_or(0.0) * c.w, c.w) };
        // A mechanic that isn't in the map at all.
        if primaries.contains(f) && !s.threads.iter().any(|t| t.0 == f) { return 0.0; }
        if s.map_has.iter().any(|h| h == f) { return 0.0; }
        let p = if s.mechs.iter().any(|k| k == f) {
            let (a, w) = of(f);
            (a + if notable { 0.5 } else { 0.35 }) / (w + 1.0)
        } else {
            let p = if s.mechs.is_empty() { all } else {
                s.mechs.iter().map(|k| { let (a, w) = of(k); (a + 0.3 * all) / (w + 0.3) }).sum::<f64>() / s.mechs.len() as f64
            };
            p * if general(f) { 1.0 } else { 0.3 }
        };
        p * if notable { 4.0 } else { 1.0 }
    }).collect();
    let mut sorted = v.clone();
    sorted.sort_by(|a, b| b.total_cmp(a));
    let sum: f64 = sorted.iter().take(150).sum::<f64>().max(1e-9);
    v.iter_mut().for_each(|x| *x /= sum);
    v
}
