//! Farm finder (24 Nov 2026): which atlas mechanics, and which of them together, look most profitable right now.
//!
//! Cross-checks, all of it shown on the cards:
//! - what each mechanic's own drops sell for and how much of them is traded (poe.ninja prices, hourly);
//! - how many players' trees go for it, and what they pair it with (poe.ninja's atlas trees, `data/atlas_meta.json`,
//!   built by `tools/build_atlas_meta.mjs`);
//! - creator trees: Threads per map, maps (`public/atlas_picks.json`, maxroll);
//! - hand-kept facts with their source: how mechanics help each other, maps that suit them (`data/farms.json`).
//!
//! The yardstick is "sales per 1% of players": drops traded, in divines, divided by the share of trees going for that
//! mechanic. It can't see drops per map, so it says where money is being made per farmer, not a promise per hour.
//! Two parts are estimates and marked so: raw currency (shared out by how maxroll ranks the mechanics for it) and
//! items sold outside the exchange (beasts: only what's listed is known).

use crate::prices::{PriceFile, PriceItem};
use crate::temple::Temples;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

const FARMS: &str = include_str!("../data/farms.json");
const META: &str = include_str!("../data/atlas_meta.json");
const PICKS: &str = include_str!("../../public/atlas_picks.json");

/// A map device holds three kinds of Thread, up to eight of each.
const MAX_THREAD_KINDS: usize = 3;
const MAX_THREADS: u32 = 8;
/// Listed items (beasts) sell slowly: this much of what's listed counts as sold.
const LISTED_COUNTS: f64 = 0.1;
/// Each way two mechanics help each other makes the pair worth this much more, up to SYNERGY_MAX.
const SYNERGY: f64 = 0.15;
const SYNERGY_MAX: f64 = 0.30;
/// Players pair two mechanics this much more often than chance: they go together.
const LIFT: f64 = 1.3;
/// A mix counts as something people run when at least this many copied the very same tree.
const COPIES: u32 = 4;

#[derive(Debug, Deserialize)]
struct Mech {
    key: String,
    label: String,
    /// poe.ninja's name for it on its atlas tree page.
    ninja: String,
    thread: Option<String>,
    pic: String,
    #[serde(default)]
    names: Vec<String>,
    #[serde(default)]
    cats: Vec<String>,
    /// Item names ending in one of these.
    #[serde(default)]
    ends: Vec<String>,
    #[serde(default)]
    not: Vec<String>,
    /// How much of its loot is raw currency, next to Legion (1).
    #[serde(default)]
    raw: f64,
    /// What it drops that no price list covers.
    unseen: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Booster { key: String, label: String, ninja: String }

#[derive(Debug, Deserialize)]
struct Raw { names: Vec<String>, everyone: f64 }

#[derive(Debug, Deserialize)]
struct Pairing { with: Vec<String>, why: String, src: String }

#[derive(Debug, Deserialize)]
struct MapFact { map: String, #[serde(rename = "for")] mechs: Vec<String>, why: String, src: String }

#[derive(Debug, Deserialize)]
struct Avoid { map: String, #[serde(rename = "for")] mech: String, why: String, src: String }

#[derive(Debug, Deserialize)]
struct Farms {
    mechanics: Vec<Mech>,
    boosters: Vec<Booster>,
    raw: Raw,
    altars: BTreeMap<String, Vec<String>>,
    synergies: Vec<Pairing>,
    maps: Vec<MapFact>,
    avoid: Vec<Avoid>,
}

#[derive(Debug, Deserialize)]
struct MetaTree { copies: u32, mechs: Vec<(String, f64)>, nodes: Vec<u32> }
#[derive(Debug, Deserialize)]
struct MetaWith { total: f64, share: HashMap<String, f64> }
#[derive(Debug, Deserialize)]
struct Meta { updated: String, total: f64, share: HashMap<String, f64>, with: HashMap<String, MetaWith>, trees: HashMap<String, Vec<MetaTree>> }

#[derive(Debug, Deserialize)]
struct Pick {
    id: String,
    name: String,
    #[serde(default)]
    author: String,
    mechanics: Vec<String>,
    #[serde(default)]
    maps: Vec<String>,
    #[serde(default)]
    threads: Vec<(String, u32)>,
    #[serde(default)]
    rolls: Vec<String>,
    /// 1 = Searing Exarch, 2 = Eater of Worlds.
    #[serde(default)]
    altar: u8,
    group: String,
}
#[derive(Debug, Deserialize)]
struct Picks { picks: Vec<Pick> }

fn farms() -> &'static Farms { static D: OnceLock<Farms> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(FARMS).expect("farms.json")) }
fn meta() -> &'static Meta { static D: OnceLock<Meta> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(META).expect("atlas_meta.json")) }
fn picks() -> &'static Picks { static D: OnceLock<Picks> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(PICKS).expect("atlas_picks.json")) }
fn mech(key: &str) -> Option<&'static Mech> { farms().mechanics.iter().find(|m| m.key == key) }

/// One of a mechanic's drops.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Drop {
    pub name: String,
    /// Picture, when it isn't the name's own (a temple room: the Chronicle).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pic: Option<String>,
    pub chaos: f64,
    /// Traded recently, in divines (listed ones count a tenth).
    pub sold_div: f64,
    /// 7 days.
    pub change: Option<f64>,
    /// Worth a divine or more: rare, so a lucky drop rather than steady income.
    pub jackpot: bool,
}

/// One mechanic on its own.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Worth {
    pub key: String,
    pub label: String,
    pub pic: String,
    /// Share of players' trees going for it (0.14 = 14%).
    pub share: f64,
    /// Divines per 1% of players: steady drops, jackpots, and the raw currency estimate.
    pub steady: f64,
    pub jackpots: f64,
    pub raw: f64,
    /// Sales-weighted 7-day price change of its drops.
    pub change: Option<f64>,
    pub drops: Vec<Drop>,
    /// What it drops that isn't counted.
    pub unseen: Option<String>,
    /// Part of the number is an estimate (raw currency, listed-only items).
    pub estimate: bool,
    /// None of its drops has a price: can't be ranked.
    pub blind: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Why { pub text: String, pub src: String }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MapTip {
    pub map: String,
    /// "Legion: wide open, so …"
    pub why: Vec<Why>,
    /// Your own average minutes in it, and how many maps that's from.
    pub yours: Option<(f64, usize)>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThreadCost { pub key: String, pub thread: String, pub count: u32, pub each: f64, pub change: Option<f64> }

/// One card: mechanics to run together.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Farm {
    pub keys: Vec<String>,
    pub title: String,
    /// 0-100, against the best one.
    pub score: f64,
    /// Divines per 1% of players (with how much of each mechanic fits in one tree, and how they help each other).
    pub value: f64,
    pub steady: f64,
    pub jackpots: f64,
    pub raw: f64,
    /// "Under-farmed" (sells well, few go for it), "Strong", "Solid" or "Crowded".
    pub verdict: String,
    pub estimate: bool,
    /// Share of players' trees with all of these (singles: that mechanic).
    pub players: Option<f64>,
    pub change: Option<f64>,
    /// Best sellers across its mechanics.
    pub drops: Vec<Drop>,
    pub threads: Vec<ThreadCost>,
    /// Chaos a map at that many Threads (before free ones from Shaped Regions).
    pub cost: f64,
    /// Why these go together.
    pub why: Vec<Why>,
    /// Also worth points in the same tree (keys for Build my own, label).
    pub also: Vec<(String, String)>,
    /// Which Eldritch altars pay Threads back, and whose.
    pub altar: Option<Why>,
    pub maps: Vec<MapTip>,
    pub avoid: Vec<Why>,
    /// A creator's tree with all of these: (maxroll id, name, author).
    pub creator: Option<(String, String, String)>,
    /// What creators roll on the map for it.
    pub rolls: Option<String>,
    pub unseen: Vec<String>,
    /// Your own maps with a tree for exactly these mechanics.
    pub own: Option<Own>,
    /// Things to know before running these together.
    pub warn: Vec<Why>,
    /// Exactly what was asked for.
    pub picked: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Count { #[default] All, /// Leave the jackpots out.
    Steady }

/// What the page asks for.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FarmAsk {
    pub count: Count,
    /// Threads may cost this much a map at most (chaos; None = any; 0 = no Threads).
    pub max_cost: Option<f64>,
    /// 0 = any number of mechanics, 1 = one on its own, 2 = two or more together.
    pub size: usize,
    /// Mechanics you want to run (he asked 24 Nov: look up any strategy or mix): the first card is exactly these, the
    /// rest add to them. Empty = the best of everything.
    pub want: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Finder {
    pub farms: Vec<Farm>,
    pub mechanics: Vec<Worth>,
    /// When the players' trees and prices are from.
    pub trees_from: String,
    pub trees: f64,
    pub prices_from: String,
    pub temples_from: String,
    /// Your trees with maps counted, best per hour first.
    pub own: Vec<OwnRow>,
    pub divine: f64,
}

/// Your map runs: (map name, seconds in it).
pub type Runs<'a> = &'a [(String, i64)];

/// One of your saved atlas trees and what your maps on it made.
#[derive(Debug, Clone, Default)]
pub struct OwnTree {
    pub name: String,
    pub nodes: Vec<u32>,
    pub result: crate::atlas::TreeResult,
    /// Its loot in divines at each map's own rate (None = divide by today's).
    pub divines: Option<f64>,
}

/// What your own maps made with a tree for a farm.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Own {
    /// Divines an hour, from the loot in your maps (hideout time between maps included).
    pub div_hour: f64,
    pub chaos_map: f64,
    pub maps: usize,
    /// Fewer than OWN_MAPS maps: luck still decides it.
    pub few: bool,
    pub trees: Vec<String>,
}

/// Your results on one saved tree, whatever it farms.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OwnRow { pub name: String, pub keys: Vec<String>, pub title: String, pub own: Own }

/// This many maps before your own number says much.
pub const OWN_MAPS: usize = 10;
/// A saved tree farms a mechanic when it puts this many points into it (or most of a small one).
const OWN_POINTS: usize = 9;

/// Which of the finder's mechanics a tree farms.
pub fn farmed(nodes: &[u32]) -> Vec<String> {
    let have = crate::atlas_opt::points_in(nodes);
    farms().mechanics.iter().filter(|m| have.iter().any(|(k, n)| *k == m.key && *n >= if m.key == "delirium" { 6 } else { OWN_POINTS }))
        .map(|m| m.key.clone()).collect()
}

/// Adds up trees' results by the time they took.
fn own_of(trees: &[&OwnTree], div: f64) -> Option<Own> {
    let (mut loot, mut divines, mut secs, mut maps) = (0.0, 0.0, 0.0, 0);
    for t in trees { loot += t.result.loot; divines += t.divines.unwrap_or(t.result.loot / div.max(1e-9)); secs += t.result.secs; maps += t.result.maps; }
    (maps > 0 && secs > 0.0 && div > 0.0).then(|| Own { div_hour: divines / (secs / 3600.0), chaos_map: loot / maps as f64, maps, few: maps < OWN_MAPS, trees: trees.iter().map(|t| t.name.clone()).collect() })
}

fn change_of(list: &[(f64, f64)]) -> Option<f64> {
    let w: f64 = list.iter().map(|x| x.0).sum();
    (w > 0.0).then(|| list.iter().map(|x| x.0 * x.1).sum::<f64>() / w)
}

fn drops_of(m: &Mech, file: &PriceFile, div: f64) -> Vec<(Drop, bool)> {
    let ours = |i: &PriceItem| !m.not.contains(&i.name)
        && (m.names.contains(&i.name) || m.cats.contains(&i.cat) || (i.cat == "Currency" && m.ends.iter().any(|e| i.name.ends_with(e.as_str()))));
    let mut out: Vec<(Drop, bool)> = file.items.iter().filter(|i| ours(i) && i.value > 0.0).filter_map(|i| {
        let (sold, listed) = match (i.volume, i.listings) {
            (Some(v), _) => (v, false),
            (None, Some(n)) => (i.value * n as f64 * LISTED_COUNTS, true),
            _ => return None,
        };
        // A week-old price that jumped tenfold on a handful of sales says nothing: the change is capped.
        Some((Drop { name: i.name.clone(), pic: None, chaos: i.value, sold_div: sold / div, change: i.change.map(|c| c.clamp(-99.0, 999.0)), jackpot: i.value >= div }, listed))
    }).collect();
    out.sort_by(|a, b| b.0.sold_div.total_cmp(&a.0.sold_div));
    out
}

/// Temples by their best rooms: what they ask over a plain temple. Listed only, like beasts.
fn temple_drops(t: &Temples, div: f64) -> Vec<(Drop, bool)> {
    let mut out: Vec<(Drop, bool)> = vec![];
    let mut dearer: Vec<&str> = vec![];
    for r in t.worth().into_iter().filter(|r| r.stats.len() == 1) {
        // A temple with two good rooms is sold once: it counts for the dearer one.
        let both: u32 = dearer.iter().filter_map(|d| t.rooms.iter().find(|p| p.stats.len() == 2 && p.stats.iter().any(|s| s == d) && p.stats.contains(&r.stats[0]))).map(|p| p.listed).sum();
        let n = r.listed.saturating_sub(both) as f64;
        out.push((Drop { name: r.label.trim_end_matches(" (Tier 3)").to_string(), pic: Some(crate::temple::CHRONICLE.into()),
            chaos: r.typical, sold_div: (r.typical - t.plain).max(0.0) * n * LISTED_COUNTS / div, change: None, jackpot: r.typical >= div }, true));
        dearer.push(&r.stats[0]);
    }
    out
}

/// Every mechanic on its own.
fn worths(file: &PriceFile, temples: &Temples, div: f64) -> Vec<Worth> {
    let f = farms();
    let pool: f64 = file.items.iter().filter(|i| f.raw.names.contains(&i.name)).filter_map(|i| i.volume).sum::<f64>() / div;
    let share = |m: &Mech| meta().share.get(&m.ninja).copied().unwrap_or(0.0);
    // Everyone's maps drop raw currency; those going for it more. Each takes part of the pool by how many run it.
    let takers: f64 = 100.0 * f.raw.everyone + f.mechanics.iter().map(|m| share(m) * 100.0 * m.raw).sum::<f64>();
    f.mechanics.iter().map(|m| {
        let s = share(m);
        let mut drops = drops_of(m, file, div);
        if m.key == "incursion" { drops.extend(temple_drops(temples, div)); drops.sort_by(|a, b| b.0.sold_div.total_cmp(&a.0.sold_div)); }
        let per = |j: bool| drops.iter().filter(|d| d.0.jackpot == j).map(|d| d.0.sold_div).sum::<f64>() / (s * 100.0).max(1.0);
        let raw = if takers > 0.0 { pool / takers * m.raw } else { 0.0 };
        Worth {
            key: m.key.clone(), label: m.label.clone(), pic: m.pic.clone(), share: s,
            steady: per(false), jackpots: per(true), raw,
            change: change_of(&drops.iter().filter_map(|d| Some((d.0.sold_div, d.0.change?))).collect::<Vec<_>>()),
            blind: drops.is_empty() && raw == 0.0,
            estimate: raw > 0.0 || drops.iter().any(|d| d.1),
            unseen: m.unseen.clone(),
            drops: drops.into_iter().map(|d| d.0).collect(),
        }
    }).collect()
}

/// Points a tree for these puts into `m`, next to trees that go for `m` most: how much of it you get in a combination.
fn depth(m: &Mech, keys: &[String]) -> f64 {
    if keys.len() == 1 { return 1.0; }
    let empty = vec![];
    let own = meta().trees.get(&m.ninja).unwrap_or(&empty);
    let points = |t: &MetaTree, n: &str| t.mechs.iter().find(|x| x.0 == n).map(|x| x.1).unwrap_or(0.0);
    let most = own.iter().map(|t| points(t, &m.ninja)).fold(0.0, f64::max);
    if most <= 0.0 { return 0.6; }
    let has_all = |t: &MetaTree| keys.iter().filter_map(|k| mech(k)).all(|x| points(t, &x.ninja) > 0.0);
    let (mut sum, mut n) = (0.0, 0.0);
    for t in meta().trees.values().flatten().filter(|t| has_all(t)) {
        sum += points(t, &m.ninja) * t.copies as f64;
        n += t.copies as f64;
    }
    if n > 0.0 { (sum / n / most).clamp(0.3, 1.0) } else { match keys.len() { 2 => 0.7, 3 => 0.55, n => 1.65 / n as f64 } }
}

/// How much more often than chance players who go for `a` also go for `b`, and their share.
fn lift(a: &str, b: &str) -> Option<(f64, f64)> {
    let (a, b) = (mech(a)?, mech(b)?);
    let both = meta().with.get(&a.ninja)?.share.get(&b.ninja).copied().unwrap_or(0.0);
    let all = meta().share.get(&b.ninja).copied().unwrap_or(0.0);
    (all > 0.0).then(|| (both / all, both))
}

/// How many players run one of the much-copied trees with exactly these mechanics.
fn copied(keys: &[String]) -> u32 {
    let f = farms();
    let mut seen: BTreeSet<&Vec<u32>> = BTreeSet::new();
    meta().trees.values().flatten().filter(|t| t.copies >= COPIES)
        .filter(|t| f.mechanics.iter().all(|m| t.mechs.iter().any(|x| x.0 == m.ninja) == keys.contains(&m.key)))
        .filter(|t| seen.insert(&t.nodes)).map(|t| t.copies).sum()
}

/// The finder's mechanics among these keys, in its own order.
fn in_order(keys: &BTreeSet<&str>) -> Vec<String> {
    farms().mechanics.iter().filter(|m| keys.contains(m.key.as_str())).map(|m| m.key.clone()).collect()
}

/// How many kinds of Thread these need.
fn thread_kinds(keys: &[String]) -> usize { keys.iter().filter_map(|k| mech(k)).filter(|m| m.thread.is_some()).count() }

/// Mixes with everything in `want` and more: ones with evidence, and each other mechanic added to it (ones people rarely
/// pair say so on the card, and come after the rest).
fn with_wanted(want: &[String]) -> Vec<Vec<String>> {
    let mut out: BTreeSet<Vec<String>> = combos().into_iter().filter(|c| c.len() > want.len() && want.iter().all(|w| c.contains(w))).collect();
    for m in &farms().mechanics {
        if want.contains(&m.key) { continue; }
        let mut keys: BTreeSet<&str> = want.iter().map(String::as_str).collect();
        keys.insert(&m.key);
        let keys = in_order(&keys);
        if keys.len() <= 5 && thread_kinds(&keys) <= MAX_THREAD_KINDS { out.insert(keys); }
    }
    out.into_iter().collect()
}

/// Combinations with evidence that they work in one tree: players' trees, creators' trees, or a known pairing.
fn combos() -> Vec<Vec<String>> {
    let f = farms();
    let order = |keys: BTreeSet<&str>| in_order(&keys);
    let mut out: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut add = |keys: Vec<String>| {
        if !keys.is_empty() && keys.len() <= 5 && thread_kinds(&keys) <= MAX_THREAD_KINDS { out.insert(keys); }
    };
    for m in &f.mechanics { add(vec![m.key.clone()]); }
    for t in meta().trees.values().flatten().filter(|t| t.copies >= COPIES) {
        add(order(f.mechanics.iter().filter(|m| t.mechs.iter().any(|x| x.0 == m.ninja)).map(|m| m.key.as_str()).collect()));
    }
    for p in &picks().picks { add(order(p.mechanics.iter().map(String::as_str).collect())); }
    for s in &f.synergies { add(order(s.with.iter().map(String::as_str).collect())); }
    for (i, a) in f.mechanics.iter().enumerate() {
        for b in &f.mechanics[i + 1..] {
            if lift(&a.key, &b.key).is_some_and(|(x, s)| x >= LIFT && s >= 0.05) { add(order([a.key.as_str(), b.key.as_str()].into())); }
        }
    }
    out.into_iter().collect()
}

/// The finder: cards best first (the mix of mechanics changes from card to card), and every mechanic on its own.
pub fn find(file: &PriceFile, temples: &Temples, runs: Runs, trees: &[OwnTree], ask: &FarmAsk) -> Finder {
    let mine: Vec<(Vec<String>, &OwnTree)> = trees.iter().filter(|t| t.result.maps > 0).map(|t| (farmed(&t.nodes), t)).collect();
    let f = farms();
    let div = file.items.iter().find(|i| i.name == "Divine Orb").map(|i| i.value).filter(|v| *v > 0.0).unwrap_or(1.0);
    let worth = worths(file, temples, div);
    let w = |k: &str| worth.iter().find(|x| x.key == k);
    let jack = if ask.count == Count::Steady { 0.0 } else { 1.0 };
    // `asked`: exactly what he picked, shown whatever it's worth.
    let card = |keys: Vec<String>, asked: bool| -> Option<Farm> {
        let parts: Vec<(&Mech, &Worth, f64)> = keys.iter().filter_map(|k| Some((mech(k)?, w(k)?))).map(|(m, x)| (m, x, depth(m, &keys))).collect();
        if parts.is_empty() || !asked && (parts.iter().all(|p| p.1.blind) || parts.iter().filter(|p| p.1.blind).count() * 2 > parts.len()) { return None; }
        // A creator's tree for exactly these (else one with all of these) says how many Threads.
        let creator = picks().picks.iter().filter(|p| keys.iter().all(|k| p.mechanics.contains(k)))
            .min_by_key(|p| (p.mechanics.iter().filter(|m| mech(m).is_some()).count(), p.group != "maxroll"));
        let exact = creator.filter(|p| p.mechanics.iter().filter(|m| mech(m).is_some()).count() == keys.len());
        let threads: Vec<ThreadCost> = parts.iter().filter_map(|(m, ..)| {
            let t = m.thread.as_ref()?;
            let item = file.items.iter().find(|i| &i.name == t);
            let count = exact.and_then(|p| p.threads.iter().find(|x| x.0 == m.key)).map(|x| x.1).unwrap_or(MAX_THREADS).min(MAX_THREADS);
            Some(ThreadCost { key: m.key.clone(), thread: t.clone(), count, each: item.map(|i| i.value).unwrap_or(0.0), change: item.and_then(|i| i.change) })
        }).collect();
        let cost: f64 = threads.iter().map(|t| t.each * t.count as f64).sum();

        let mut warn: Vec<Why> = vec![];
        if threads.len() > MAX_THREAD_KINDS {
            warn.push(Why { text: format!("A map takes {MAX_THREAD_KINDS} kinds of Thread and these are {}: you'd run them in turns", threads.len()), src: "guides".into() });
        }
        for (i, a) in parts.iter().enumerate() {
            for b in &parts[i + 1..] {
                if let Some((x, s)) = lift(&a.0.key, &b.0.key).filter(|(x, _)| *x < 0.7) {
                    warn.push(Why { text: format!("Players rarely take {} and {} together ({:.0}% of {} trees, {x:.1}× chance)", a.0.label, b.0.label, s * 100.0, a.0.label), src: "poe.ninja".into() });
                }
            }
        }
        warn.truncate(3);
        let mut why: Vec<Why> = vec![];
        for s in f.synergies.iter().filter(|s| s.with.iter().all(|k| keys.contains(k))) { why.push(Why { text: s.why.clone(), src: s.src.clone() }); }
        let helps = why.len();
        if keys.iter().any(|k| k == "incursion") {
            let best: Vec<String> = temples.worth().into_iter().take(4).map(|r| format!("{} {}", r.label.replace(" (Tier 3)", "").replace(" of Corruption", "").replace("'s Institute", "").replace(" of Creation", ""), if r.typical >= div { format!("{:.1} div", r.typical / div) } else { format!("{:.0}c", r.typical) })).collect();
            if !best.is_empty() { why.push(Why { text: format!("Temples to build and sell, asking about: {}", best.join(", ")), src: "trade site".into() }); }
        }
        let bonus = 1.0 + (SYNERGY * helps as f64).min(SYNERGY_MAX);
        let mut pairs: Vec<(f64, f64, &str, &str)> = vec![];
        for (i, a) in parts.iter().enumerate() {
            for b in &parts[i + 1..] {
                if let Some((x, s)) = lift(&a.0.key, &b.0.key).filter(|(x, s)| *x >= LIFT && *s >= 0.05) { pairs.push((x, s, &a.0.label, &b.0.label)); }
            }
        }
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (x, s, a, b) in pairs.iter().take(2) {
            why.push(Why { text: format!("{:.0}% of {a} trees also take {b}, {x:.1}× more than by chance", s * 100.0), src: "poe.ninja".into() });
        }
        let copies = copied(&keys);
        if keys.len() > 1 && copies >= 10 { why.push(Why { text: format!("{copies} players copied the same trees with exactly these"), src: "poe.ninja".into() }); }
        let sum = |g: &dyn Fn(&Worth) -> f64| parts.iter().map(|(_, x, d)| g(x) * d).sum::<f64>();
        let (steady, jackpots) = (sum(&|x| x.steady) + 0.0, (sum(&|x| x.jackpots) * jack).max(0.0));
        // Raw currency comes from the same monsters: the best of them counts in full, the others for half.
        let mut raws: Vec<f64> = parts.iter().map(|(_, x, d)| x.raw * d).collect();
        raws.sort_by(|a, b| b.total_cmp(a));
        let raw = raws.first().copied().unwrap_or(0.0) + raws.iter().skip(1).sum::<f64>() * 0.5;
        let value = (steady + jackpots + raw) * bonus;

        let players = if keys.len() == 1 { parts.first().map(|p| p.1.share) } else if keys.len() == 2 {
            meta().with.get(&parts[0].0.ninja).and_then(|v| Some(v.share.get(&parts[1].0.ninja)? * v.total / meta().total))
        } else { None };
        let mut drops: Vec<Drop> = parts.iter().flat_map(|p| p.1.drops.iter().take(6).cloned()).filter(|d| jack > 0.0 || !d.jackpot).collect();
        if parts.iter().any(|p| p.1.raw > 0.0) {
            drops.extend(file.items.iter().filter(|i| f.raw.names.contains(&i.name)).filter_map(|i| Some(Drop { name: i.name.clone(), pic: None, chaos: i.value,
                sold_div: i.volume? / div * 0.02, change: i.change, jackpot: i.value >= div })).filter(|d| jack > 0.0 || !d.jackpot));
        }
        drops.sort_by(|a, b| b.sold_div.total_cmp(&a.sold_div));
        drops.truncate(8);

        // Also worth points: what those players' trees take more than others do, or most of them take.
        let mut also: Vec<(f64, &Booster)> = f.boosters.iter().filter_map(|b| {
            let all = meta().share.get(&b.ninja).copied().unwrap_or(0.0);
            let of: Vec<f64> = parts.iter().filter_map(|p| meta().with.get(&p.0.ninja)?.share.get(&b.ninja).copied()).collect();
            if of.is_empty() || all <= 0.0 { return None; }
            let s = of.iter().sum::<f64>() / of.len() as f64;
            (s >= 0.25 || (s >= 0.15 && s / all >= 1.3)).then_some((s + (s / all - 1.0).clamp(0.0, 1.0) * 0.3, b))
        }).collect();
        also.sort_by(|a, b| b.0.total_cmp(&a.0));

        let pays = f.altars.iter().map(|(name, ks)| (name, parts.iter().filter(|p| ks.contains(&p.0.key)).map(|p| p.0.label.clone()).collect::<Vec<_>>()))
            // A map has one Eldritch influence: the side paying back the dearer Threads.
            .max_by(|a, b| { let paid = |n: &str| threads.iter().filter(|t| f.altars[n].contains(&t.key)).map(|t| t.each * t.count as f64).sum::<f64>(); paid(a.0).total_cmp(&paid(b.0)) }).filter(|x| !x.1.is_empty() && threads.iter().any(|t| t.each > 0.0))
            .map(|(n, l)| Why { text: format!("{n}: they can drop your {} Threads", l.join(" and ")), src: "patch notes".into() });
        // The creator's own choice for exactly these wins.
        let altar = match exact.map(|p| (p.altar, p.group.as_str())) {
            Some((a @ (1 | 2), g)) => Some(Why { text: format!("{}: what the creator's setup runs", if a == 2 { "Eater of Worlds" } else { "Searing Exarch" }), src: if g == "maxroll" { "maxroll" } else { "community" }.into() }),
            _ => pays,
        };
        // Any creator's tree with all of these that names its maps, the closest first.
        let mapped = picks().picks.iter().filter(|p| !p.maps.is_empty() && keys.iter().all(|k| p.mechanics.contains(k)))
            .min_by_key(|p| (p.group != "maxroll", p.mechanics.iter().filter(|m| mech(m).is_some()).count()));
        let maps = map_tips(&keys, exact.filter(|p| !p.maps.is_empty()).or(mapped), runs);
        let avoid = f.avoid.iter().filter(|a| keys.contains(&a.mech) && !maps.iter().any(|m| m.map == a.map))
            .map(|a| Why { text: format!("{}: {}", a.map.trim_end_matches(" Map"), a.why), src: a.src.clone() }).collect();
        Some(Farm {
            title: parts.iter().map(|p| p.0.label.as_str()).collect::<Vec<_>>().join(" + "),
            score: 0.0, value, steady, jackpots, raw, verdict: String::new(),
            estimate: parts.iter().any(|p| p.1.estimate), players,
            change: change_of(&drops.iter().filter_map(|d| Some((d.sold_div, d.change?))).collect::<Vec<_>>()),
            drops, threads, cost, why,
            also: also.into_iter().take(3).map(|(_, b)| (b.key.clone(), b.label.clone())).collect(),
            altar, maps, avoid,
            creator: creator.map(|p| (p.id.clone(), p.name.clone(), p.author.clone())),
            rolls: exact.filter(|p| !p.rolls.is_empty()).map(|p| p.rolls.join(", ")),
            unseen: parts.iter().filter_map(|p| p.1.unseen.clone()).collect(),
            own: own_of(&mine.iter().filter(|m| m.0 == keys).map(|m| m.1).collect::<Vec<_>>(), div),
            warn, picked: asked,
            keys,
        })
    };
    let every: Vec<Farm> = combos().into_iter().filter_map(|k| card(k, false)).collect();
    // Every score is against the best mix there is, whatever is being looked at.
    let best = every.iter().map(|x| x.value).fold(0.0, f64::max).max(1e-9);
    let want = in_order(&ask.want.iter().map(String::as_str).collect());
    let fits = |x: &Farm| !(ask.max_cost.is_some_and(|c| x.cost > c) || (ask.max_cost == Some(0.0) && !x.threads.is_empty()))
        && match ask.size { 1 => x.keys.len() == 1, 2 => x.keys.len() >= 2, _ => true };
    let mut list: Vec<Farm> = if want.is_empty() { every } else { with_wanted(&want).into_iter().filter_map(|k| card(k, false)).collect() };
    list.retain(fits);
    list.sort_by(|a, b| b.value.total_cmp(&a.value));
    if !want.is_empty() { list.sort_by_key(|x| !x.warn.is_empty()); }
    if !want.is_empty() { if let Some(x) = card(want.clone(), true) { list.insert(0, x); } }
    // Next to a typical mechanic on its own: plenty sold per farmer, or not much.
    let mut alone: Vec<f64> = worth.iter().filter(|x| !x.blind).map(|x| x.steady + x.jackpots * jack + x.raw).collect();
    alone.sort_by(f64::total_cmp);
    let middle = alone.get(alone.len() / 2).copied().unwrap_or(0.0);
    for x in &mut list {
        x.score = x.value / best * 100.0;
        // Per mechanic: a mix of three has to sell more than one on its own would.
        let each = x.value / (x.keys.len() as f64).sqrt();
        let share = x.keys.iter().filter_map(|k| mech(k)).filter_map(|m| meta().share.get(&m.ninja)).sum::<f64>() / x.keys.len() as f64;
        x.verdict = match each / middle.max(1e-9) { r if r >= 1.5 && share < 0.07 => "Under-farmed", r if r >= 1.5 => "Strong", r if r >= 0.7 => "Solid", _ => "Crowded" }.into();
    }
    // Cards that differ: one that only adds to or drops from a better card, or shares most of it, isn't a new idea.
    let mut farms_out: Vec<Farm> = vec![];
    for x in list {
        // With something picked, only what's added to it has to differ.
        let added = |k: &&String| !want.contains(k);
        let a: BTreeSet<&String> = x.keys.iter().filter(added).collect();
        let near = !x.picked && farms_out.iter().filter(|y| !y.picked).any(|y| {
            let b: BTreeSet<&String> = y.keys.iter().filter(added).collect();
            let both = a.intersection(&b).count();
            both == a.len() || both == b.len() || both * 2 > a.union(&b).count()
        });
        if !near { farms_out.push(x); }
        if farms_out.len() >= 10 { break; }
    }
    let label = |keys: &[String]| keys.iter().filter_map(|k| mech(k)).map(|m| m.label.as_str()).collect::<Vec<_>>().join(" + ");
    let mut own: Vec<OwnRow> = mine.iter().filter_map(|(keys, t)| Some(OwnRow { name: t.name.clone(), title: label(keys), keys: keys.clone(), own: own_of(&[*t], div)? })).collect();
    own.sort_by(|a, b| b.own.div_hour.total_cmp(&a.own.div_hour));
    let mut mechanics = worth;
    mechanics.sort_by(|a, b| (b.steady + b.jackpots * jack + b.raw).total_cmp(&(a.steady + a.jackpots * jack + a.raw)));
    Finder { farms: farms_out, mechanics, trees_from: meta().updated.clone(), trees: meta().total, prices_from: file.updated.clone(), temples_from: temples.checked.clone(), own, divine: div }
}

/// Maps that suit these mechanics: each reason counts for how many of them it covers; a map to avoid for one is out
/// unless a source picks it for exactly that mix. Your own average time goes with it.
fn map_tips(keys: &[String], creator: Option<&Pick>, runs: Runs) -> Vec<MapTip> {
    let f = farms();
    let mut by: BTreeMap<&str, (f64, Vec<Why>, BTreeSet<&str>)> = BTreeMap::new();
    for m in &f.maps {
        // A pick for a mix counts when you run at least two of it (or it is one mechanic, and you run it).
        let hit: Vec<&String> = m.mechs.iter().filter(|k| keys.contains(k)).collect();
        if hit.is_empty() || (m.mechs.len() > 1 && hit.len() < 2) { continue; }
        let e = by.entry(&m.map).or_default();
        let new = hit.iter().filter(|k| e.2.insert(k.as_str())).count() as f64;
        e.0 += new + if m.src == "maxroll" { 0.5 } else { 0.0 } + if m.mechs.len() > 1 { 0.25 } else { 0.0 };
        let names = hit.iter().filter_map(|k| mech(k)).map(|x| x.label.as_str()).collect::<Vec<_>>().join(" + ");
        e.1.push(Why { text: if keys.len() > 1 { format!("{names}: {}", m.why) } else { m.why.clone() }, src: m.src.clone() });
    }
    if let Some(p) = creator {
        for m in &p.maps {
            let e = by.entry(m.as_str()).or_default();
            e.0 += 0.5;
            if e.1.is_empty() { e.1.push(Why { text: format!("{} runs it in {}", if p.author.is_empty() { "The creator" } else { &p.author }, p.name.trim_matches('"')), src: if p.group == "maxroll" { "maxroll" } else { "community" }.into() }); }
        }
    }
    let bad = |m: &str| f.avoid.iter().any(|a| a.map == m && keys.contains(&a.mech) && !f.maps.iter().any(|x| x.map == m && x.mechs.len() > 1 && x.mechs.iter().all(|k| keys.contains(k))));
    let mut v: Vec<(&str, f64, Vec<Why>)> = by.into_iter().filter(|(m, _)| !bad(m)).map(|(m, (s, w, _))| (m, s, w)).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v.into_iter().take(2).map(|(m, _, mut why)| {
        why.sort_by_key(|w| w.src != "maxroll");
        let name = m.trim_end_matches(" Map");
        let mine: Vec<i64> = runs.iter().filter(|r| r.0 == name && r.1 >= 60).map(|r| r.1).collect();
        MapTip { map: m.to_string(), why, yours: (!mine.is_empty()).then(|| (mine.iter().sum::<i64>() as f64 / mine.len() as f64 / 60.0, mine.len())) }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, cat: &str, value: f64, volume: Option<f64>, listings: Option<u32>) -> PriceItem {
        PriceItem { name: name.into(), base: name.into(), cat: cat.into(), value, variant: None, listings, trend: None, change: Some(10.0), volume }
    }
    fn file() -> PriceFile {
        PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: None, updated: "now".into(), unit: "chaos".into(), bases: vec![], items: vec![
            item("Divine Orb", "Currency", 200.0, Some(1e6), None),
            item("Essence of Horror", "Essence", 10.0, Some(400_000.0), None),
            item("Prismatic Oil", "Oil", 1400.0, Some(90_000.0), None), item("Silver Oil", "Oil", 37.0, Some(28_000.0), None),
            item("Tainted Oil", "Oil", 197.0, Some(9e9), None),
            item("Timeless Vaal Emblem", "Fragment", 19.0, Some(27_000.0), None),
            item("Craicic Croaker", "Beast", 275.0, None, Some(2941)),
            item("Stacked Deck", "Currency", 3.7, Some(2_800_000.0), None),
            item("Legion Thread", "Thread", 5.0, Some(4e5), None), item("Abyss Thread", "Thread", 5.0, Some(4e5), None), item("Bestiary Thread", "Thread", 2.0, Some(4e5), None),
            item("Prismatic Catalyst", "Currency", 19.0, Some(75_000.0), None),
        ] }
    }

    #[test]
    fn data_files_agree() {
        for m in &farms().mechanics {
            assert!(meta().share.contains_key(&m.ninja), "{}", m.ninja);
            assert!(crate::atlas_opt::MECHANICS.iter().any(|x| x.0 == m.key), "{} isn't a Build my own mechanic", m.key);
            assert!(meta().with.contains_key(&m.ninja), "{}", m.ninja);
        }
        for b in &farms().boosters {
            assert!(meta().share.contains_key(&b.ninja), "{}", b.ninja);
            assert!(crate::atlas_opt::MECHANICS.iter().any(|x| x.0 == b.key), "{}", b.key);
        }
        let known = |k: &String| mech(k).is_some();
        assert!(farms().synergies.iter().all(|s| s.with.iter().all(known)) && farms().maps.iter().all(|m| m.mechs.iter().all(known)));
        assert!(farms().altars.values().flatten().all(known) && farms().avoid.iter().all(|a| known(&a.mech)));
    }

    #[test]
    fn combines_and_explains() {
        let r = find(&file(), Temples::shipped(), &[("Dunes".into(), 300), ("Dunes".into(), 420), ("Dunes".into(), 20)], &[], &FarmAsk::default());
        let top = |k: &[&str]| r.farms.iter().find(|f| f.keys.iter().map(String::as_str).collect::<Vec<_>>() == k);
        // Legion + Beasts (and Abyss) go together, with Threads, altars, a map and your own time in it.
        let lb = r.farms.iter().find(|f| f.keys.contains(&"legion".to_string()) && f.keys.contains(&"bestiary".to_string())).expect("legion + beasts");
        assert!(lb.why.iter().any(|w| w.src == "maxroll") && lb.why.iter().any(|w| w.src == "poe.ninja"), "{:?}", lb.why);
        assert!(lb.threads.iter().all(|t| t.count == 8) && lb.cost > 0.0 && lb.altar.is_some());
        let dunes = lb.maps.iter().find(|m| m.map == "Dunes Map").expect("Dunes");
        assert_eq!(dunes.yours, Some((6.0, 2)), "under a minute isn't a map");
        assert!(top(&["essence"]).is_some() || r.farms.iter().any(|f| f.keys.contains(&"essence".to_string())));
        assert_eq!(r.farms[0].score, 100.0);
        // Cards differ: none is another's mechanics plus or minus some.
        for (i, a) in r.farms.iter().enumerate() {
            for b in &r.farms[i + 1..] {
                assert!(!a.keys.iter().all(|k| b.keys.contains(k)) && !b.keys.iter().all(|k| a.keys.contains(k)), "{:?} {:?}", a.keys, b.keys);
            }
        }
        // No more than three kinds of Thread in a map.
        assert!(r.farms.iter().all(|f| f.threads.len() <= 3));
    }

    #[test]
    fn any_mix_can_be_looked_up() {
        let ask = |want: &[&str]| FarmAsk { want: want.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        // Ritual: its own card first, then mixes with it, each adding something else.
        let r = find(&file(), Temples::shipped(), &[], &[], &ask(&["ritual"]));
        assert!(r.farms[0].picked && r.farms[0].keys == ["ritual"]);
        assert!(r.farms.len() > 3 && r.farms[1..].iter().all(|f| f.keys.len() > 1 && f.keys.contains(&"ritual".to_string()) && !f.picked), "{:?}", r.farms.iter().map(|f| &f.keys).collect::<Vec<_>>());
        // Any order, odd mixes and too many Threads still get a card, with what to know about it.
        let r = find(&file(), Temples::shipped(), &[], &[], &ask(&["harvest", "ritual"]));
        assert_eq!(r.farms[0].keys, ["ritual", "harvest"]);
        let r = find(&file(), Temples::shipped(), &[], &[], &ask(&["blight", "legion", "breach", "ritual"]));
        assert!(r.farms[0].keys.len() == 4 && r.farms[0].warn[0].text.contains("kinds of Thread"), "{:?}", r.farms[0].warn);
        // Even with a cost limit or a size that doesn't fit, what was asked for shows; unknown names are dropped.
        let r = find(&file(), Temples::shipped(), &[], &[], &FarmAsk { max_cost: Some(0.0), size: 2, ..ask(&["legion", "nonsense"]) });
        assert_eq!(r.farms[0].keys, ["legion"]);
        // The score is on one scale: alone or picked, the same.
        let alone = find(&file(), Temples::shipped(), &[], &[], &FarmAsk { size: 1, ..Default::default() });
        let e = alone.farms.iter().find(|f| f.keys == ["essence"]).unwrap().score;
        assert_eq!(find(&file(), Temples::shipped(), &[], &[], &ask(&["essence"])).farms[0].score, e);
    }

    #[test]
    fn own_results_go_on_the_right_card() {
        // The most copied Legion + Beasts + Delirium tree: it farms exactly those, so its maps show on that card.
        let t = meta().trees["All"].iter().find(|t| t.copies == 297).expect("the most copied tree");
        assert_eq!(farmed(&t.nodes), ["legion", "bestiary", "delirium"], "{:?}", crate::atlas_opt::points_in(&t.nodes));
        let mine = OwnTree { name: "My legion".into(), nodes: t.nodes.clone(), result: crate::atlas::TreeResult { id: 1, maps: 12, loot: 2400.0, secs: 7200.0, ..Default::default() }, divines: None };
        let r = find(&file(), Temples::shipped(), &[], &[mine, OwnTree::default()], &FarmAsk::default());
        let card = r.farms.iter().find(|f| f.keys == ["legion", "bestiary", "delirium"]).expect("card");
        let own = card.own.as_ref().expect("own");
        assert!((own.div_hour - 6.0).abs() < 0.01 && own.maps == 12 && !own.few && own.chaos_map == 200.0, "{own:?}");
        assert_eq!(r.farms.iter().filter(|f| f.own.is_some()).count(), 1);
        assert_eq!((r.own.len(), r.own[0].title.as_str()), (1, "Legion + Beasts + Delirium"));
        // Maps run when a divine was 100c made 24 of them, whatever it is today.
        let then = OwnTree { divines: Some(24.0), ..OwnTree { name: "x".into(), nodes: t.nodes.clone(), result: crate::atlas::TreeResult { id: 1, maps: 12, loot: 2400.0, secs: 7200.0, ..Default::default() }, divines: None } };
        assert!((own_of(&[&then], 200.0).unwrap().div_hour - 12.0).abs() < 0.01);
    }

    /// How well `farmed` agrees with poe.ninja on the much-copied trees.
    #[test]
    fn farmed_agrees_with_poe_ninja() {
        let (mut same, mut all) = (0, 0);
        for t in meta().trees.values().flatten() {
            for m in &farms().mechanics {
                let p = t.mechs.iter().find(|x| x.0 == m.ninja).map(|x| x.1).unwrap_or(0.0);
                // Clear cases only: a real investment, or none.
                if p >= 11.0 || p == 0.0 { all += 1; same += usize::from((p > 0.0) == farmed(&t.nodes).contains(&m.key)); }
            }
        }
        println!("{same} of {all}");
        assert!(same as f64 / all as f64 > 0.95, "{same} of {all}");
    }

    #[test]
    fn counts_what_it_should() {
        let r = find(&file(), Temples::shipped(), &[], &[], &FarmAsk::default());
        let blight = r.mechanics.iter().find(|m| m.key == "blight").unwrap();
        assert!(blight.drops.iter().all(|d| d.name != "Tainted Oil"), "Tainted Oil isn't Blight's");
        assert!(blight.jackpots > 0.0 && blight.steady > 0.0 && blight.drops[0].jackpot);
        let beasts = r.mechanics.iter().find(|m| m.key == "bestiary").unwrap();
        assert!(beasts.estimate && beasts.drops[0].sold_div < 275.0 * 2941.0 / 200.0);
        let temples = r.mechanics.iter().find(|m| m.key == "incursion").unwrap();
        assert!(!temples.blind && temples.estimate && temples.drops.iter().all(|d| d.pic.as_deref() == Some("Chronicle of Atzoatl")), "temples have prices");
        // Steady only: Blight's Prismatic Oils don't count. No Threads: only farms without any.
        let s = find(&file(), Temples::shipped(), &[], &[], &FarmAsk { count: Count::Steady, ..Default::default() });
        assert!(s.farms.iter().all(|f| f.jackpots == 0.0 && f.drops.iter().all(|d| !d.jackpot)));
        let free = find(&file(), Temples::shipped(), &[], &[], &FarmAsk { max_cost: Some(0.0), ..Default::default() });
        assert!(!free.farms.is_empty() && free.farms.iter().all(|f| f.threads.is_empty()));
        let one = find(&file(), Temples::shipped(), &[], &[], &FarmAsk { size: 1, ..Default::default() });
        assert!(one.farms.iter().all(|f| f.keys.len() == 1));
    }

    /// Dev look with the real prices: `cargo test -p filter-core --lib farm::tests::real -- --ignored --nocapture`
    /// (FARM_OUT=<file> saves the answer for preview-app.html).
    #[test]
    #[ignore]
    fn real() {
        let data = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("com.grobbs.poefiltereditor");
        let prices: PriceFile = serde_json::from_str(&std::fs::read_to_string(data.join("prices_poe1.json")).unwrap()).unwrap();
        let play: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("play.json")).unwrap()).unwrap();
        let runs: Vec<(String, i64)> = play["maps"]["history"].as_array().unwrap().iter().map(|r| (r["name"].as_str().unwrap().into(), r["in_map"].as_i64().unwrap())).collect();
        for ask in [FarmAsk::default(), FarmAsk { count: Count::Steady, ..Default::default() }, FarmAsk { size: 2, ..Default::default() }] {
            let r = find(&prices, Temples::shipped(), &runs, &[], &ask);
            println!("== {ask:?}");
            for f in &r.farms { println!("{:5.1} {:40} value {:6.0} steady {:5.0} jack {:5.0} raw {:5.0} cost {:4.0}c {} players {:?} maps {:?} also {:?}", f.score, f.title, f.value, f.steady, f.jackpots, f.raw, f.cost, f.verdict, f.players.map(|p| (p * 1000.0).round() / 10.0), f.maps.iter().map(|m| (&m.map, m.yours)).collect::<Vec<_>>(), f.also.iter().map(|a| &a.0).collect::<Vec<_>>()); }
        }
        for want in [vec!["ritual"], vec!["ritual", "harvest"], vec!["ultimatum"], vec!["abyss"], vec!["blight", "expedition"]] {
            let r = find(&prices, Temples::shipped(), &runs, &[], &FarmAsk { want: want.iter().map(|s| s.to_string()).collect(), ..Default::default() });
            println!("== want {want:?}");
            if let Ok(o) = std::env::var("FARM_OUT") { std::fs::write(o.replace(".json", &format!("-{}.json", want.join("+"))), serde_json::to_string(&r).unwrap()).unwrap(); }
            for f in &r.farms { println!("{:5.1} {:40} {} cost {:3.0}c maps {:?} warn {:?}", f.score, f.title, f.verdict, f.cost, f.maps.iter().map(|m| &m.map).collect::<Vec<_>>(), f.warn.iter().map(|w| &w.text).collect::<Vec<_>>()); }
        }
        let r = find(&prices, Temples::shipped(), &runs, &[], &FarmAsk::default());
        for m in &r.mechanics { println!("{:12} share {:4.1}% steady {:5.0} jack {:5.0} raw {:4.0} blind {} top {:?}", m.key, m.share * 100.0, m.steady, m.jackpots, m.raw, m.blind, m.drops.iter().take(3).map(|d| format!("{} {:.0}div", d.name, d.sold_div)).collect::<Vec<_>>()); }
        for f in r.farms.iter().take(3) { println!("{f:#?}"); }
        if let Ok(o) = std::env::var("FARM_OUT") { std::fs::write(o, serde_json::to_string(&r).unwrap()).unwrap(); }
    }
}
