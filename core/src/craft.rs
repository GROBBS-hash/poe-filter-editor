//! Crafting helper: which mods can roll on a base (tiers, item level, how common), and for one mod, the ways to get it
//! (essence, crafting bench, fossils, plain Chaos Orbs) with rough odds and cost.
//! Data: core/data/poe1_craft.json, built by tools/build_craft_data.mjs from the game's files (RePoE).
//! Odds come from rolling the item many times here (like Craft of Exile's simulator, much simpler): a rare gets
//! 4 mods (8 in 12), 5 (3 in 12) or 6 (1 in 12), at most 3 prefixes and 3 suffixes, one mod per group,
//! each picked by its weight.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

pub mod routes;
pub mod search;

#[derive(Debug, Deserialize)]
struct RawMod { id: String, name: String, gen: String, lvl: u32, text: String, r#type: String, group: String, tags: Vec<String>, sw: Vec<(String, f64)>, src: String }
#[derive(Debug, Deserialize)]
struct RawEssence { name: String, corrupt: bool, mods: HashMap<String, String> }
#[derive(Debug, Deserialize)]
struct RawFossil { name: String, more: Vec<(String, f64)>, less: Vec<(String, f64)> }
#[derive(Debug, Deserialize)]
struct RawBench { r#mod: String, classes: Vec<String>, cost: Vec<(String, u32)> }
#[derive(Debug, Deserialize)]
struct Data { bases: HashMap<String, (String, u32, Vec<String>)>, mods: Vec<RawMod>, essences: Vec<RawEssence>, fossils: Vec<RawFossil>, bench: Vec<RawBench> }

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_craft.json")).expect("poe1_craft.json"))
}

/// One tier of a mod on this base. Tier 1 is the best.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Tier { pub tier: u32, pub name: String, pub lvl: u32, pub text: String, pub weight: f64, pub ok: bool }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EssenceWay { pub name: String, pub text: String, pub like_tier: Option<u32>, pub price: Option<f64>, pub corrupt: bool }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BenchWay { pub text: String, pub cost: Vec<(String, u32)> }

/// A mod "family": all tiers of one mod (e.g. "+# to maximum Life").
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Family {
    pub key: String,
    /// "prefix" / "suffix".
    pub affix: String,
    /// The best tier you can get at this item level (or the bench / essence text when it can't roll).
    pub text: String,
    pub tags: Vec<String>,
    pub tiers: Vec<Tier>,
    /// Chance a Chaos Orb gives it (any tier), 0–1. 0 = can't roll naturally.
    pub chaos: f64,
    /// Tier the item has now, if it has this mod.
    pub have: Option<u32>,
    pub essences: Vec<EssenceWay>,
    pub bench: Vec<BenchWay>,
    /// Fossils that make it more common / impossible.
    pub fossils_more: Vec<String>,
    pub fossils_none: Vec<String>,
    /// Only on an item with this influence ("Shaper"…): shown even when the base has none, so you know what it takes.
    pub needs: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pool { pub base: String, pub class: String, pub ilvl: u32, pub influences: Vec<String>, pub families: Vec<Family>, pub item_mods: Vec<ItemMod> }

/// A mod on the item you brought, and the family it belongs to (to click through).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ItemMod { pub text: String, pub affix: Option<String>, pub tier: Option<u32>, pub key: Option<String>, pub needs: Option<String>,
    /// "fractured", "crafted" or "explicit".
    pub kind: String,
    /// How you get it, when that isn't the usual rolling (fractured, bench, beastcraft…).
    pub how: Option<String> }

/// How to get a mod line that isn't a normal roll on this base.
fn how_to(text: &str, kind: &str, found: bool) -> Option<String> {
    if kind == "fractured" { return Some("Fractured: locked, it never changes when you reroll the item. A Fracturing Orb locks one mod on a rare with 4 or more mods.".into()) }
    if text.contains("Aspect of the") { return Some("Beastcrafting: an \"Aspect of the …\" recipe at the Menagerie's Blood Altar adds it (it takes a suffix). It never rolls on its own.".into()) }
    if kind == "crafted" { return Some(if found { "Added at the crafting bench: click to see its cost. Remove it with the bench's \"Remove Crafted Mods\" to craft another.".into() }
        else { "A crafting bench mod (some need an unlock from Jun's veiled mods). Remove it at the bench with \"Remove Crafted Mods\".".into() }) }
    if !found { return Some("Not in this base's normal mod pool: it may be a veiled mod (Aisling / unveil at Jun), Delve, a beastcraft or a mod from an older league.".into()) }
    None
}

/// One way to get a mod, with its odds.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Way {
    /// "Guaranteed", "Reroll" or "Add a mod".
    pub group: String,
    /// "Chaos Orb", "Shrieking Essence of Greed", "Jagged Fossil + Pristine Fossil"…
    pub method: String,
    /// Chance per try, 0–1.
    pub chance: f64,
    /// What one try costs in chaos (None = price unknown).
    pub cost_each: Option<f64>,
    /// Average cost to hit it (cost_each / chance).
    pub average: Option<f64>,
    pub note: String,
    /// Routes only: about how many currency uses it takes (0 = not worked out).
    pub tries: f64,
    /// Routes only: 0 safe, 1 can go wrong (start a step again), 2 can lose mods you already have.
    pub risk: u8,
    /// Routes only: a rule here that isn't confirmed yet, to check in game.
    pub check: Option<String>,
}

/// "+(15-25)% to Fire Resistance" / "+56 to Intelligence" -> "+#% to fire resistance" / "+# to intelligence".
fn shape(text: &str) -> String {
    // Item lines can carry their affix ("prefix|+21% to Item Rarity", see `start`).
    let text = shape_strip(text);
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '(' {
            // "(15-25)" or "(-25--15)": one number.
            let mut inner = String::new();
            for d in chars.by_ref() { if d == ')' { break } inner.push(d) }
            if inner.chars().all(|x| x.is_ascii_digit() || x == '-' || x == '.') { out.push('#'); } else { out.push('('); out.push_str(&inner); out.push(')'); }
        } else if c.is_ascii_digit() {
            while chars.peek().is_some_and(|d| d.is_ascii_digit() || *d == '.') { chars.next(); }
            out.push('#');
        } else { out.push(c); }
    }
    out.replace("+#", "#").replace("-#", "#").to_lowercase()
}

/// An item line without its "prefix|" / "suffix|" mark.
fn shape_strip(mut t: &str) -> &str {
    for m in ["prefix|", "suffix|", "fractured|", "crafted|"] { if let Some(r) = t.strip_prefix(m) { t = r } }
    for m in ["fractured|", "crafted|"] { if let Some(r) = t.strip_prefix(m) { t = r } }
    t
}

/// The top of a mod's first number ("+(15-25)% …" -> 25, "+56 …" -> 56).
fn first_top(text: &str) -> Option<f64> {
    let t = text.trim_start_matches(['+', '-']);
    if let Some(r) = t.strip_prefix('(') {
        let inner = r.split(')').next()?;
        let (_, b) = inner.char_indices().skip(1).find(|(_, c)| *c == '-').map(|(i, _)| inner.split_at(i)).unwrap_or(("", inner));
        return b.trim_start_matches('-').parse().ok().or_else(|| inner.parse().ok());
    }
    let n: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    n.parse().ok()
}
fn first_bottom(text: &str) -> Option<f64> {
    let t = text.trim_start_matches(['+', '-']);
    if let Some(r) = t.strip_prefix('(') {
        let inner = r.split(')').next()?;
        return inner.split('-').find(|s| !s.is_empty())?.parse().ok();
    }
    first_top(text)
}

const INFLUENCE_TAGS: [(&str, &str); 6] = [("Shaper", "shaper"), ("Elder", "elder"), ("Crusader", "crusader"), ("Redeemer", "eyrie"), ("Hunter", "basilisk"), ("Warlord", "adjudicator")];

/// The base as the crafting data knows it ("Fecund Eternal Burgonet of the Genius" -> "Eternal Burgonet").
pub fn find_base(name: &str) -> Option<String> {
    let d = data();
    if d.bases.contains_key(name) { return Some(name.to_string()) }
    d.bases.keys().filter(|b| name.contains(b.as_str())).max_by_key(|b| b.len()).cloned()
}

/// Base names for the search box.
pub fn base_names() -> Vec<String> {
    let mut v: Vec<String> = data().bases.keys().cloned().collect();
    v.sort();
    v
}

fn tagset(base: &str, influences: &[String]) -> Option<(String, HashSet<String>)> {
    let (class, _, tags) = data().bases.get(base)?;
    let mut set: HashSet<String> = tags.iter().cloned().collect();
    for inf in influences {
        if let Some((_, t)) = INFLUENCE_TAGS.iter().find(|(n, _)| n.eq_ignore_ascii_case(inf)) {
            for tag in tags { set.insert(format!("{tag}_{t}")); }
        }
    }
    Some((class.clone(), set))
}

/// Which influence a mod's weight came from ("helmet_shaper" -> Shaper), if any.
fn influence_of(m: &RawMod, tags: &HashSet<String>) -> Option<String> {
    let (t, _) = m.sw.iter().find(|(t, _)| tags.contains(t))?;
    INFLUENCE_TAGS.iter().find(|(_, s)| t.ends_with(&format!("_{s}"))).map(|(n, _)| n.to_string())
}

/// A mod's weight on this base: the first of its tags the base has decides.
fn weight(m: &RawMod, tags: &HashSet<String>) -> f64 {
    m.sw.iter().find(|(t, _)| tags.contains(t)).map(|(_, w)| *w).unwrap_or(0.0)
}

/// Fossils' effect on a mod's weight (each matching tag multiplies: 1000 = ×10, 0 = never).
fn fossil_factor(m: &RawMod, fossils: &[&RawFossil]) -> f64 {
    let mut f = 1.0;
    for fo in fossils {
        for (t, w) in fo.more.iter().chain(fo.less.iter()) { if m.tags.contains(t) { f *= w / 100.0; } }
    }
    f
}

/// Small fast random numbers (no extra crate needed); seeded so the same question gives the same answer.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; (self.0 >> 11) as f64 / (1u64 << 53) as f64 }
}

/// The mods a rare can roll here: (mod index, weight, is prefix, group number).
fn rollable(tags: &HashSet<String>, ilvl: u32, fossils: &[&RawFossil]) -> Vec<(usize, f64, bool, usize)> {
    let d = data();
    d.mods.iter().enumerate().filter(|(_, m)| m.src.is_empty() && m.lvl <= ilvl).filter_map(|(i, m)| {
        let w = weight(m, tags) * fossil_factor(m, fossils);
        if w <= 0.0 { return None }
        Some((i, w, m.gen == "p", group_id(&m.group)))
    }).collect()
}

/// A mod group's number, the same in every pool (so mods kept on an item line up with a fossil pool's).
fn group_id(g: &str) -> usize {
    static G: OnceLock<HashMap<String, usize>> = OnceLock::new();
    let m = G.get_or_init(|| { let mut m = HashMap::new(); for x in &data().mods { let n = m.len(); m.entry(x.group.clone()).or_insert(n); } m });
    m.get(g).copied().unwrap_or(usize::MAX)
}

/// How an item gets rolled: a rare (4–6 mods, 3+3) or magic (1–2 mods, 1+1); `tag` = Harvest's "keeps at least one
/// mod of this kind" (the first mod is picked among those).
#[derive(Clone, Copy)]
struct Roll<'a> { magic: bool, tag: Option<&'a str> }
const RARE: Roll = Roll { magic: false, tag: None };

/// Rolls a rare `trials` times; counts how often each mod lands.
fn simulate(pool: &[(usize, f64, bool, usize)], trials: u32) -> HashMap<usize, u32> { simulate_with(pool, trials, RARE) }

fn simulate_with(pool: &[(usize, f64, bool, usize)], trials: u32, roll: Roll) -> HashMap<usize, u32> {
    let d = data();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut counts = vec![0u32; pool.len()];
    let mut used: Vec<usize> = Vec::with_capacity(6);
    let side_max = if roll.magic { 1 } else { 3 };
    let tagged: Vec<bool> = pool.iter().map(|(i, ..)| roll.tag.is_some_and(|t| d.mods[*i].tags.iter().any(|x| x == t))).collect();
    for _ in 0..trials {
        let r = rng.next();
        // Magic: one or two mods, about even (a rough guide); rare: 4 (8 in 12), 5 (3 in 12), 6 (1 in 12).
        let n = if roll.magic { if r < 0.5 { 1 } else { 2 } } else if r < 8.0 / 12.0 { 4 } else if r < 11.0 / 12.0 { 5 } else { 6 };
        let (mut pre, mut suf) = (0, 0);
        used.clear();
        for step in 0..n {
            let only_tag = step == 0 && roll.tag.is_some();
            let open = |k: usize, p: bool, g: usize, pre: i32, suf: i32, used: &[usize]| !used.contains(&g)
                && if p { pre < side_max } else { suf < side_max } && (!only_tag || tagged[k]);
            let total: f64 = pool.iter().enumerate().filter(|(k, (_, _, p, g))| open(*k, *p, *g, pre, suf, &used)).map(|(_, (_, w, ..))| w).sum();
            if total <= 0.0 { break }
            let mut x = rng.next() * total;
            for (k, (_, w, p, g)) in pool.iter().enumerate() {
                if !open(k, *p, *g, pre, suf, &used) { continue }
                x -= w;
                if x <= 0.0 {
                    used.push(*g);
                    if *p { pre += 1 } else { suf += 1 }
                    counts[k] += 1;
                    break;
                }
            }
        }
    }
    pool.iter().zip(counts).filter(|(_, n)| *n > 0).map(|((i, ..), n)| (*i, n)).collect()
}

/// Rolls `trials` times and returns how often *every* target lands (each target = the pool slots that count for it).
/// `forced` = an essence's mod: (is prefix, its group number if it can roll naturally); it takes one slot.
fn simulate_all(pool: &[(usize, f64, bool, usize)], trials: u32, roll: Roll, forced: Option<(bool, Option<usize>)>, targets: &[Vec<bool>]) -> f64 {
    let d = data();
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let side_max = if roll.magic { 1 } else { 3 };
    let tagged: Vec<bool> = pool.iter().map(|(i, ..)| roll.tag.is_some_and(|t| d.mods[*i].tags.iter().any(|x| x == t))).collect();
    let mut used: Vec<usize> = Vec::with_capacity(6);
    let mut got = vec![false; targets.len()];
    let mut wins = 0u32;
    for _ in 0..trials {
        let r = rng.next();
        let n = if roll.magic { if r < 0.5 { 1 } else { 2 } } else if r < 8.0 / 12.0 { 4 } else if r < 11.0 / 12.0 { 5 } else { 6 };
        let (mut pre, mut suf) = (0, 0);
        used.clear();
        got.iter_mut().for_each(|g| *g = false);
        let mut start = 0;
        if let Some((p, g)) = forced { if p { pre += 1 } else { suf += 1 } if let Some(g) = g { used.push(g) } start = 1; }
        for step in start..n {
            let only_tag = step == start && roll.tag.is_some();
            let open = |k: usize, p: bool, g: usize, pre: i32, suf: i32, used: &[usize]| !used.contains(&g)
                && if p { pre < side_max } else { suf < side_max } && (!only_tag || tagged[k]);
            let total: f64 = pool.iter().enumerate().filter(|(k, (_, _, p, g))| open(*k, *p, *g, pre, suf, &used)).map(|(_, (_, w, ..))| w).sum();
            if total <= 0.0 { break }
            let mut x = rng.next() * total;
            for (k, (_, w, p, g)) in pool.iter().enumerate() {
                if !open(k, *p, *g, pre, suf, &used) { continue }
                x -= w;
                if x <= 0.0 {
                    used.push(*g);
                    if *p { pre += 1 } else { suf += 1 }
                    for (t, set) in targets.iter().enumerate() { if set[k] { got[t] = true } }
                    break;
                }
            }
        }
        if got.iter().all(|g| *g) { wins += 1 }
    }
    wins as f64 / trials as f64
}

/// Several mods at once ("T1 life + T1 cold res + …"): the chance each method gives all of them in one go.
/// `targets` = (family key, tier or better).
pub fn plan(base: &str, ilvl: u32, influences: &[String], targets: &[(String, u32)], price: &dyn Fn(&str) -> Option<f64>) -> Vec<Way> {
    let d = data();
    let Some(p) = pool(base, ilvl, influences, &[], price) else { return vec![] };
    let Some((_, tags)) = tagset(base, influences) else { return vec![] };
    let fams: Vec<(&Family, u32)> = targets.iter().filter_map(|(k, t)| p.families.iter().find(|f| &f.key == k).map(|f| (f, *t))).collect();
    if fams.is_empty() { return vec![] }
    let cost = |name: &str| price(name).or_else(|| (name == "Chaos Orb").then_some(1.0));
    let way = |group: &str, method: String, chance: f64, each: Option<f64>, note: String| Way {
        group: group.into(), method, chance, cost_each: each, average: each.filter(|_| chance > 0.0).map(|e| e / chance), note, ..Default::default() };
    let sets = |pool: &[(usize, f64, bool, usize)], skip: Option<usize>| -> Vec<Vec<bool>> {
        fams.iter().enumerate().filter(|(n, _)| Some(*n) != skip).map(|(_, (f, t))| {
            let names: HashSet<&str> = f.tiers.iter().filter(|x| x.tier <= *t).map(|x| x.name.as_str()).collect();
            pool.iter().map(|(i, ..)| { let m = &d.mods[*i]; format!("{}|{}", m.gen, m.r#type) == f.key && names.contains(m.name.as_str()) }).collect()
        }).collect()
    };
    let n_pre = fams.iter().filter(|(f, _)| f.affix == "prefix").count();
    let n_suf = fams.len() - n_pre;
    let mut out = Vec::new();
    if n_pre > 3 || n_suf > 3 { return vec![way("Reroll", "Not possible".into(), 0.0, None, "A rare holds at most 3 prefixes and 3 suffixes".into())] }
    let roll = rollable(&tags, ilvl, &[]);
    let all = sets(&roll, None);
    let c = simulate_all(&roll, TRIALS, RARE, None, &all);
    out.push(way("Reroll", "Chaos Orb".into(), c, cost("Chaos Orb"), "Rerolls a rare item: 4–6 new mods".into()));
    if n_pre <= 1 && n_suf <= 1 {
        let a = simulate_all(&roll, TRIALS, Roll { magic: true, tag: None }, None, &all);
        out.push(way("Reroll", "Orb of Alteration".into(), a, cost("Orb of Alteration"), "Rerolls a magic item: 1–2 mods, then Regal Orb".into()));
    }
    let mut tags_seen: Vec<&str> = Vec::new();
    for (f, _) in &fams { for t in &f.tags { if HARVEST.iter().any(|(h, _)| h == t) && !tags_seen.contains(&t.as_str()) { tags_seen.push(t) } } }
    for t in tags_seen {
        let name = HARVEST.iter().find(|(h, _)| *h == t).map(|(_, n)| *n).unwrap_or(t);
        let h = simulate_all(&roll, TRIALS, Roll { magic: false, tag: Some(t) }, None, &all);
        out.push(way("Reroll", format!("Harvest reforge: {name}"), h, None, format!("Rerolls a rare keeping at least one {} mod (costs lifeforce)", name.to_lowercase())));
    }
    // Essences: one target guaranteed, the rest have to roll.
    let groups: HashMap<&str, usize> = roll.iter().map(|(i, _, _, g)| (d.mods[*i].group.as_str(), *g)).collect();
    let by_name: HashMap<&str, &RawMod> = d.mods.iter().map(|m| (m.id.as_str(), m)).collect();
    let class = &p.class;
    for (n, (f, t)) in fams.iter().enumerate() {
        for e in f.essences.iter().filter(|e| e.like_tier.is_some_and(|x| x <= *t)).take(1) {
            let Some(raw) = d.essences.iter().find(|r| r.name == e.name).and_then(|r| r.mods.get(class)).and_then(|id| by_name.get(id.as_str())) else { continue };
            let rest = sets(&roll, Some(n));
            let x = if rest.is_empty() { 1.0 } else { simulate_all(&roll, TRIALS, RARE, Some((raw.gen == "p", groups.get(raw.group.as_str()).copied())), &rest) };
            out.push(way("Reroll", e.name.clone(), x, e.price, format!("Always gives {}; the others have to roll", e.text.replace('\n', " / "))));
        }
    }
    // Fossils that help any target and block none.
    let blocked = |fo: &RawFossil| fams.iter().any(|(f, _)| f.fossils_none.contains(&fo.name));
    let helpful: Vec<&RawFossil> = d.fossils.iter().filter(|fo| !blocked(fo) && fams.iter().any(|(f, _)| f.fossils_more.contains(&fo.name))).collect();
    let mut combos: Vec<Vec<&RawFossil>> = helpful.iter().map(|f| vec![*f]).collect();
    for (a, f) in helpful.iter().enumerate() { for g in helpful.iter().skip(a + 1) { combos.push(vec![*f, *g]); } }
    let mut tried: Vec<Way> = combos.into_iter().take(10).map(|fs| {
        let pool = rollable(&tags, ilvl, &fs);
        let x = simulate_all(&pool, TRIALS, RARE, None, &sets(&pool, None));
        let res = ["Primitive Chaotic Resonator", "Potent Chaotic Resonator", "Powerful Chaotic Resonator", "Prime Chaotic Resonator"][fs.len() - 1];
        let each: Option<f64> = fs.iter().map(|f| price(&f.name)).chain(std::iter::once(price(res))).sum();
        way("Reroll", fs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" + "), x, each, format!("Rerolls a rare with these fossils in a {res}"))
    }).filter(|w| w.chance > 0.0).collect();
    tried.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal));
    out.extend(tried.into_iter().take(3));
    out.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)
        .then(b.chance.partial_cmp(&a.chance).unwrap_or(std::cmp::Ordering::Equal)));
    out
}
/// Chance one added mod (Exalted / Regal / Augmentation) is a target: its weight over everything that can still go on.
fn add_one(pool: &[(usize, f64, bool, usize)], used: &HashSet<usize>, pre: i32, suf: i32, side_max: i32, is_target: &dyn Fn(&usize) -> bool) -> f64 {
    let open: Vec<&(usize, f64, bool, usize)> = pool.iter().filter(|(_, _, p, g)| !used.contains(g) && if *p { pre < side_max } else { suf < side_max }).collect();
    let total: f64 = open.iter().map(|(_, w, ..)| w).sum();
    if total <= 0.0 { return 0.0 }
    open.iter().filter(|(i, ..)| is_target(i)).map(|(_, w, ..)| w).sum::<f64>() / total
}

/// Harvest reforges: "Reforge … including a <kind> modifier".
const HARVEST: [(&str, &str); 11] = [("life", "Life"), ("defences", "Defence"), ("physical", "Physical"), ("fire", "Fire"), ("cold", "Cold"),
    ("lightning", "Lightning"), ("chaos", "Chaos"), ("attack", "Attack"), ("caster", "Caster"), ("speed", "Speed"), ("critical", "Critical")];
const TRIALS: u32 = 20_000;

/// Everything that can roll on `base` at `ilvl` (plus bench-only and essence-only mods), grouped into families,
/// with how often a Chaos Orb gives each. `have` = the item's own mod lines (to mark what it has).
pub fn pool(base: &str, ilvl: u32, influences: &[String], have: &[String], price: &dyn Fn(&str) -> Option<f64>) -> Option<Pool> {
    let d = data();
    let (class, tags) = tagset(base, influences)?;
    let roll = rollable(&tags, ilvl, &[]);
    let counts = simulate(&roll, TRIALS);
    // Families: every mod that can roll on this base at any level, by type.
    let mut fams: Vec<Family> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut shapes: HashMap<String, usize> = HashMap::new();
    let mut members: Vec<Vec<(usize, f64)>> = Vec::new();
    // Whole-mod wording (both lines of a hybrid) -> family, for bench and essence mods.
    let mut whole: HashMap<String, usize> = HashMap::new();
    // Each influence's tags, for mods only an influenced item can roll.
    let each_influence: Vec<(&str, HashSet<String>)> = INFLUENCE_TAGS.iter().filter_map(|(n, _)| Some((*n, tagset(base, &[n.to_string()])?.1))).collect();
    let mut needs_of: HashMap<usize, String> = HashMap::new();
    for (i, m) in d.mods.iter().enumerate() {
        if !m.src.is_empty() { continue }
        let (w, needs) = if weight(m, &tags) > 0.0 { (weight(m, &tags), influence_of(m, &tags)) }
            else if let Some((n, t)) = each_influence.iter().find(|(_, t)| weight(m, t) > 0.0) { (weight(m, t), Some(n.to_string())) }
            else { continue };
        let key = format!("{}|{}", m.gen, m.r#type);
        let f = *by_key.entry(key.clone()).or_insert_with(|| {
            fams.push(Family { key, affix: if m.gen == "p" { "prefix" } else { "suffix" }.into(), text: String::new(), tags: m.tags.clone(),
                tiers: vec![], chaos: 0.0, have: None, essences: vec![], bench: vec![], fossils_more: vec![], fossils_none: vec![], needs: None });
            members.push(vec![]);
            fams.len() - 1
        });
        members[f].push((i, w));
        if let Some(n) = needs { needs_of.insert(f, n); }
        for line in m.text.lines() { shapes.entry(shape(line)).or_insert(f); }
        whole.entry(shape(&m.text)).or_insert(f);
    }
    for (f, ids) in members.iter().enumerate() {
        let mut ids = ids.clone();
        ids.sort_by(|a, b| d.mods[b.0].lvl.cmp(&d.mods[a.0].lvl));
        let fam = &mut fams[f];
        fam.needs = needs_of.get(&f).cloned();
        fam.tiers = ids.iter().enumerate().map(|(n, (i, w))| { let m = &d.mods[*i];
            Tier { tier: n as u32 + 1, name: m.name.clone(), lvl: m.lvl, text: m.text.clone(), weight: *w, ok: m.lvl <= ilvl } }).collect();
        fam.text = fam.tiers.iter().find(|t| t.ok).or(fam.tiers.last()).map(|t| t.text.clone()).unwrap_or_default();
        fam.chaos = ids.iter().map(|(i, _)| counts.get(i).copied().unwrap_or(0)).sum::<u32>() as f64 / TRIALS as f64;
    }
    // Bench and essence mods: on the family with the same wording, or a family of their own ("bench only").
    let attach = |m: &RawMod, fams: &mut Vec<Family>, shapes: &mut HashMap<String, usize>| -> usize {
        let s = shape(&m.text);
        if let Some(&f) = shapes.get(&s) { return f }
        fams.push(Family { key: format!("{}|{}", m.gen, m.r#type), affix: if m.gen == "p" { "prefix" } else { "suffix" }.into(), text: m.text.clone(),
            tags: m.tags.clone(), tiers: vec![], chaos: 0.0, have: None, essences: vec![], bench: vec![], fossils_more: vec![], fossils_none: vec![], needs: None });
        shapes.insert(s, fams.len() - 1);
        fams.len() - 1
    };
    let by_id: HashMap<&str, &RawMod> = d.mods.iter().map(|m| (m.id.as_str(), m)).collect();
    for b in d.bench.iter().filter(|b| b.classes.contains(&class)) {
        let Some(m) = by_id.get(b.r#mod.as_str()) else { continue };
        let f = attach(m, &mut fams, &mut whole);
        fams[f].bench.push(BenchWay { text: m.text.clone(), cost: b.cost.iter().map(|(c, n)| (currency_name(c).to_string(), *n)).collect() });
    }
    for e in &d.essences {
        let Some(id) = e.mods.get(&class) else { continue };
        let Some(m) = by_id.get(id.as_str()) else { continue };
        let f = attach(m, &mut fams, &mut whole);
        // Like the best tier its lowest roll reaches.
        let like_tier = first_bottom(&m.text).and_then(|v| fams[f].tiers.iter().find(|t| first_bottom(&t.text).is_some_and(|b| b <= v)).map(|t| t.tier));
        fams[f].essences.push(EssenceWay { name: e.name.clone(), text: m.text.clone(), like_tier, price: price(&e.name), corrupt: e.corrupt });
    }
    // Fossils.
    for fam in fams.iter_mut() {
        for fo in &d.fossils {
            if fo.more.iter().any(|(t, w)| *w > 100.0 && fam.tags.contains(t)) { fam.fossils_more.push(fo.name.clone()); }
            if fo.less.iter().chain(fo.more.iter()).any(|(t, w)| *w == 0.0 && fam.tags.contains(t)) { fam.fossils_none.push(fo.name.clone()); }
        }
        fam.essences.sort_by(|a, b| a.like_tier.unwrap_or(99).cmp(&b.like_tier.unwrap_or(99)));
        // Best bench craft first.
        fam.bench.sort_by(|a, b| first_top(&b.text).unwrap_or(0.0).partial_cmp(&first_top(&a.text).unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
    }
    // What the item has now. Catalyst quality first: "quality|Life and Mana|20" (see `start`).
    let quality: Option<(Vec<&str>, f64)> = have.iter().find_map(|l| {
        let mut p = l.strip_prefix("quality|")?.split('|');
        let kind = p.next()?;
        let pct: f64 = p.next()?.parse().ok()?;
        let tags: Vec<&str> = match kind { "Life and Mana" => vec!["life", "mana"], "Attribute" => vec!["attribute"], "Resistance" => vec!["resistance"],
            "Defence" => vec!["defences"], "Elemental Damage" => vec!["elemental_damage", "elemental"], "Physical and Chaos Damage" => vec!["physical_damage", "chaos_damage"],
            "Caster" => vec!["caster"], "Attack" => vec!["attack"], "Speed" => vec!["speed"], "Critical" => vec!["critical"], _ => vec![] };
        Some((tags, pct))
    });
    let mut item_mods = Vec::new();
    for line in have.iter().filter(|l| !l.starts_with("quality|")) {
        let s = shape(line);
        let affix = if line.starts_with("prefix|") { Some("prefix") } else if line.starts_with("suffix|") { Some("suffix") } else { None };
        let kind = if line.contains("fractured|") { "fractured" } else if line.contains("crafted|") { "crafted" } else { "explicit" };
        // Same wording as a prefix and a suffix (Item Rarity): the item says which.
        let own = affix.and_then(|a| fams.iter().position(|f| f.affix == a && f.tiers.iter().any(|t| t.text.lines().any(|l| shape(l) == s))));
        // Bench-only mods ("Channelling Skills have -3 to Total Mana Cost") are found by their whole wording.
        let found = own.or_else(|| shapes.get(&s).copied()).or_else(|| whole.get(&s).copied());
        if let Some(f) = found {
            let boost = quality.as_ref().filter(|(t, _)| fams[f].tags.iter().any(|x| t.contains(&x.as_str()))).map(|(_, p)| 1.0 + p / 100.0).unwrap_or(1.0);
            let v = first_top(shape_strip(line).trim_start_matches(['+', '-'])).map(|v| (v / boost).floor());
            fams[f].have = Some(v.and_then(|v| fams[f].tiers.iter().find(|t| first_bottom(&t.text).is_some_and(|b| b <= v)).map(|t| t.tier)).unwrap_or(0));
        }
        item_mods.push(ItemMod { text: shape_strip(line).to_string(), affix: affix.map(String::from).or_else(|| found.map(|f| fams[f].affix.clone())),
            tier: found.and_then(|f| fams[f].have).filter(|t| *t > 0), key: found.map(|f| fams[f].key.clone()), needs: found.and_then(|f| fams[f].needs.clone()),
            kind: kind.to_string(), how: how_to(shape_strip(line), kind, found.is_some()) });
    }
    // Armour pieces: the bench and essences only add a defence the base has (no Evasion crafts on a pure Armour helmet).
    let (_, _, base_tags) = &d.bases[base];
    if let Some(def) = base_tags.iter().find(|t| t.ends_with("_armour") && t.as_str() != "ward_armour") {
        let (ar, ev, es) = (def.contains("str"), def.contains("dex"), def.contains("int"));
        fams.retain(|f| {
            if !f.tiers.is_empty() { return true }
            let t = f.text.to_lowercase();
            !((t.contains("armour") && !ar && !t.contains("armour items")) || (t.contains("evasion") && !ev) || (t.contains("energy shield") && !es && !t.contains("recharge")))
        });
    }
    fams.retain(|f| !f.text.is_empty());
    fams.sort_by(|a, b| a.affix.cmp(&b.affix).then(b.chaos.partial_cmp(&a.chaos).unwrap_or(std::cmp::Ordering::Equal)));
    Some(Pool { base: base.to_string(), class, ilvl, influences: influences.to_vec(), families: fams, item_mods })
}

/// The ways to get one family at `min_tier` or better (1 = best), cheapest first. `have` = the item's mod lines and
/// `rarity` its rarity ("Magic", "Rare", or "" with no item): with an item, slams (Exalted, Regal, Augmentation) are
/// worked out on its own open slots.
pub fn ways(base: &str, ilvl: u32, influences: &[String], key: &str, min_tier: u32, have: &[String], rarity: &str, price: &dyn Fn(&str) -> Option<f64>) -> Vec<Way> {
    let d = data();
    let Some(p) = pool(base, ilvl, influences, &[], price) else { return vec![] };
    let Some(fam) = p.families.iter().find(|f| f.key == key) else { return vec![] };
    let Some((_, tags)) = tagset(base, influences) else { return vec![] };
    let wanted: HashSet<&str> = fam.tiers.iter().filter(|t| t.tier <= min_tier).map(|t| t.name.as_str()).collect();
    let is_target = |i: &usize| { let m = &d.mods[*i]; format!("{}|{}", m.gen, m.r#type) == fam.key && wanted.contains(m.name.as_str()) };
    let hit = |counts: &HashMap<usize, u32>| counts.iter().filter(|(i, _)| is_target(i)).map(|(_, n)| *n).sum::<u32>() as f64 / TRIALS as f64;
    let cost = |name: &str| price(name).or_else(|| (name == "Chaos Orb").then_some(1.0));
    let way = |group: &str, method: String, chance: f64, each: Option<f64>, note: String| Way {
        group: group.into(), method, chance, cost_each: each, average: each.filter(|_| chance > 0.0).map(|e| e / chance), note, ..Default::default() };
    let mut out = Vec::new();
    let roll = rollable(&tags, ilvl, &[]);
    let natural = !fam.tiers.is_empty() && fam.tiers.iter().any(|t| t.ok && t.tier <= min_tier);
    // Needs an influence the item doesn't have: a conqueror / Shaper / Elder Exalted Orb adds it with one of its mods.
    if let Some(n) = fam.needs.as_ref().filter(|n| !influences.contains(n)) {
        let inf = [n.clone()];
        let Some((_, itags)) = tagset(base, &inf) else { return vec![] };
        let ipool = rollable(&itags, ilvl, &[]);
        let plain: HashSet<usize> = roll.iter().map(|(i, ..)| *i).collect();
        let only: Vec<(usize, f64, bool, usize)> = ipool.iter().filter(|(i, ..)| !plain.contains(i)).copied().collect();
        let orb = format!("{n}'s Exalted Orb");
        let x = add_one(&only, &HashSet::new(), 0, 0, 3, &is_target);
        out.push(way("Add a mod", orb.clone(), x, cost(&orb), format!("Makes a non-influenced rare {n} and adds one {n} mod (needs a free {} slot)", fam.affix)));
        let c = hit(&simulate(&ipool, TRIALS));
        out.push(way("Reroll", format!("Chaos Orb on a {n} item"), c, cost("Chaos Orb"), format!("Once the item is {n}, Chaos Orbs can roll it too")));
        out.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal));
        return out;
    }

    // What the item has now: groups taken, prefixes / suffixes used.
    let groups: HashMap<&str, usize> = roll.iter().map(|(i, _, _, g)| (d.mods[*i].group.as_str(), *g)).collect();
    let (mut used, mut pre, mut suf) = (HashSet::new(), 0, 0);
    let mut seen: HashSet<&str> = HashSet::new();
    let mut k = 0;
    while k < have.len() {
        // A two-line mod ("40% increased Armour" / "16% increased Stun and Block Recovery") is one mod.
        let gen = if have[k].starts_with("prefix|") { Some("p") } else if have[k].starts_with("suffix|") { Some("s") } else { None };
        let fits = |m: &&RawMod| gen.is_none_or(|g| m.gen == g) && (m.src.is_empty() || m.src == "bench");
        let pair = have.get(k + 1).map(|n| shape(&format!("{}\n{}", have[k], shape_strip(n))));
        let two = pair.and_then(|s| d.mods.iter().filter(fits).find(|m| shape(&m.text) == s));
        let s = shape(&have[k]);
        let m = two.or_else(|| d.mods.iter().filter(fits).find(|m| shape(&m.text) == s)).or_else(|| d.mods.iter().filter(fits).find(|m| m.text.lines().any(|l| shape(l) == s)));
        k += if two.is_some() { 2 } else { 1 };
        let Some(m) = m else { continue };
        if !seen.insert(m.group.as_str()) { continue }
        if let Some(g) = groups.get(m.group.as_str()) { used.insert(*g); }
        if m.gen == "p" { pre += 1 } else { suf += 1 }
    }
    let already = fam.have.is_some();

    // Guaranteed: essences and the bench.
    for e in &fam.essences {
        if e.like_tier.is_some_and(|t| t <= min_tier) || fam.tiers.is_empty() {
            out.push(way("Guaranteed", e.name.clone(), 1.0, e.price,
                if e.corrupt { "Rerolls the item as a rare with this mod (corrupted essence: needs a Remnant of Corruption)".into() }
                else { format!("Rerolls the item as a rare with {} and random other mods", e.text.replace('\n', " / ")) }));
        }
    }
    if let Some(b) = fam.bench.first() {
        let each: Option<f64> = b.cost.iter().map(|(c, n)| cost(c).map(|v| v * *n as f64)).sum();
        let like = first_bottom(&b.text).and_then(|v| fam.tiers.iter().find(|t| first_bottom(&t.text).is_some_and(|x| x <= v))).map(|t| format!(", like tier {}", t.tier)).unwrap_or_default();
        out.push(way("Guaranteed", "Crafting bench".into(), 1.0, each,
            format!("Adds {}{like}. Needs a free {} slot; one bench craft per item", b.text.replace('\n', " / "), fam.affix)));
    }

    if natural {
        // Reroll the whole item.
        let c = hit(&simulate(&roll, TRIALS));
        out.push(way("Reroll", "Chaos Orb".into(), c, cost("Chaos Orb"), "Rerolls a rare item: 4–6 new mods".into()));
        let a = hit(&simulate_with(&roll, TRIALS, Roll { magic: true, tag: None }));
        out.push(way("Reroll", "Orb of Alteration".into(), a, cost("Orb of Alteration"),
            "Rerolls a magic item: 1–2 mods. Then Regal Orb to make it rare and keep it".into()));
        for (tag, name) in HARVEST.iter().filter(|(t, _)| fam.tags.iter().any(|x| x == t)) {
            let h = hit(&simulate_with(&roll, TRIALS, Roll { magic: false, tag: Some(tag) }));
            out.push(way("Reroll", format!("Harvest reforge: {name}"), h, None,
                format!("Rerolls a rare keeping at least one {} mod (Horticrafting station, costs lifeforce)", name.to_lowercase())));
        }
        // Fossils: each one that helps, and the best pairs.
        let helpful: Vec<&RawFossil> = d.fossils.iter().filter(|f| fam.fossils_more.contains(&f.name)).collect();
        let blockers: Vec<&RawFossil> = d.fossils.iter().filter(|f| !fam.fossils_none.contains(&f.name) && !helpful.iter().any(|h| h.name == f.name)
            && f.more.iter().all(|(_, w)| *w <= 100.0) && f.less.iter().any(|(_, w)| *w == 0.0)).collect();
        let mut combos: Vec<Vec<&RawFossil>> = helpful.iter().map(|f| vec![*f]).collect();
        for (a, f) in helpful.iter().enumerate() {
            for g in helpful.iter().skip(a + 1).chain(blockers.iter()) { combos.push(vec![*f, *g]); }
        }
        let mut tried: Vec<Way> = combos.into_iter().take(12).map(|fs| {
            let c = hit(&simulate(&rollable(&tags, ilvl, &fs), TRIALS));
            let res = ["Primitive Chaotic Resonator", "Potent Chaotic Resonator", "Powerful Chaotic Resonator", "Prime Chaotic Resonator"][fs.len() - 1];
            let each: Option<f64> = fs.iter().map(|f| price(&f.name)).chain(std::iter::once(price(res))).sum();
            way("Reroll", fs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" + "), c, each, format!("Rerolls a rare with these fossils in a {res}"))
        }).filter(|w| w.chance > 0.0).collect();
        tried.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)
            .then(b.chance.partial_cmp(&a.chance).unwrap_or(std::cmp::Ordering::Equal)));
        out.extend(tried.into_iter().take(3));

        // Add one mod to what's there.
        let side_full = |max: i32| if fam.affix == "prefix" { pre >= max } else { suf >= max };
        if !already {
            match rarity {
                "Rare" if !side_full(3) && pre + suf < 6 => {
                    let x = add_one(&roll, &used, pre, suf, 3, &is_target);
                    out.push(way("Add a mod", "Exalted Orb".into(), x, cost("Exalted Orb"), format!("Adds one mod to your item ({pre} prefixes, {suf} suffixes now)")));
                }
                "Magic" if pre + suf < 2 && !side_full(1) => {
                    let x = add_one(&roll, &used, pre, suf, 1, &is_target);
                    out.push(way("Add a mod", "Orb of Augmentation".into(), x, cost("Orb of Augmentation"), "Adds the missing mod to your magic item".into()));
                    let r = add_one(&roll, &used, pre, suf, 3, &is_target);
                    out.push(way("Add a mod", "Regal Orb".into(), r, cost("Regal Orb"), "Makes your magic item rare, adding one mod".into()));
                }
                "Magic" if !side_full(3) => {
                    let r = add_one(&roll, &used, pre, suf, 3, &is_target);
                    out.push(way("Add a mod", "Regal Orb".into(), r, cost("Regal Orb"), "Makes your magic item rare, adding one mod".into()));
                }
                "" => {
                    let x = add_one(&roll, &HashSet::new(), 0, 0, 3, &is_target);
                    out.push(way("Add a mod", "Exalted Orb".into(), x, cost("Exalted Orb"),
                        format!("On a rare with a free {} slot (rough: the real odds depend on its other mods; bring the item for exact ones)", fam.affix)));
                }
                _ => {}
            }
        }
    }
    out.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)
        .then(b.chance.partial_cmp(&a.chance).unwrap_or(std::cmp::Ordering::Equal)));
    out
}
/// One step of a crafting route: what to do, and the ways to do it (cheapest first).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Step { pub title: String, pub detail: String, pub ways: Vec<Way> }

/// A copied item, ready for the Crafting page: its base, item level, influences and mod lines.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Start { pub base: String, pub ilvl: u32, pub influences: Vec<String>, pub have: Vec<String>, pub name: String, pub rarity: String }

pub fn start(text: &str) -> Option<Start> {
    let item = crate::item::parse_item(text)?;
    let base = find_base(&item.base_type)?;
    // "Searing Exarch Item", "Shaper Item"… mark the item; they aren't mods (21 Nov: they showed as suffixes, 5/3).
    let marker = |t: &str| t.ends_with(" Item") && !t.chars().any(|c| c.is_ascii_digit());
    let have = crate::trade::mods(text).into_iter().filter(|m| (m.kind == "explicit" || m.kind == "crafted" || m.kind == "fractured") && !marker(&m.text))
        .map(|m| {
            let kind = if m.kind == "fractured" || m.kind == "crafted" { format!("{}|", m.kind) } else { String::new() };
            match m.affix.as_deref() { Some(a @ ("prefix" | "suffix")) => format!("{a}|{kind}{}", m.text), _ => format!("{kind}{}", m.text) }
        }).collect::<Vec<_>>();
    // Catalyst quality ("Quality (Life and Mana Modifiers): +20%") raises those mods' numbers: passed on so tiers are read right.
    let mut have = have;
    if let Some(l) = text.lines().find(|l| l.starts_with("Quality (")) {
        if let (Some(kind), Some(pct)) = (l.split('(').nth(1).and_then(|s| s.split(" Modifiers").next()), l.split('+').nth(1).and_then(|s| s.split('%').next()).and_then(|s| s.trim().parse::<f64>().ok())) {
            have.push(format!("quality|{kind}|{pct}"));
        }
    }
    Some(Start { base, ilvl: item.item_level.unwrap_or(84), influences: item.influences.clone(), have,
        name: item.name.clone().unwrap_or_else(|| item.base_type.clone()), rarity: item.rarity.clone() })
}

/// The game's file names for currency -> what it's called.
fn currency_name(id: &str) -> &str {
    match id {
        "CurrencyRerollMagic" => "Orb of Alteration", "CurrencyUpgradeToMagic" => "Orb of Transmutation", "CurrencyAddModToMagic" => "Orb of Augmentation",
        "CurrencyUpgradeToRare" => "Orb of Alchemy", "CurrencyRerollRare" => "Chaos Orb", "CurrencyUpgradeMagicToRare" => "Regal Orb",
        "CurrencyAddModToRare" => "Exalted Orb", "CurrencyFlaskQuality" => "Glassblower's Bauble", "CurrencyCorrupt" => "Vaal Orb",
        "CurrencyGemQuality" => "Gemcutter's Prism", "CurrencyArmourQuality" => "Armourer's Scrap", "CurrencyModValues" => "Divine Orb",
        "CurrencyRerollDefences" => "Sacred Orb", "CurrencyRerollImplicit" => "Blessed Orb", other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_match_item_lines() {
        assert_eq!(shape("+(15-25)% to Fire Resistance"), shape("+21% to Fire Resistance"));
        assert_eq!(shape("+(3-9) to maximum Life"), "# to maximum life");
        assert_eq!(first_top("+(145-159) to maximum Life"), Some(159.0));
        assert_eq!(first_bottom("+(145-159) to maximum Life"), Some(145.0));
        assert_eq!(first_top("+56 to Intelligence"), Some(56.0));
    }

    #[test]
    fn helmet_pool() {
        let none = |_: &str| None;
        assert_eq!(find_base("Fecund Eternal Burgonet of the Genius").as_deref(), Some("Eternal Burgonet"));
        let p = pool("Eternal Burgonet", 85, &[], &["+56 to Intelligence".into()], &none).unwrap();
        assert_eq!(p.class, "Helmet");
        let life = p.families.iter().find(|f| f.text.contains("to maximum Life") && f.affix == "prefix" && !f.tiers.is_empty()).unwrap();
        assert!(life.tiers.len() >= 9, "{:?}", life.tiers);
        assert_eq!(life.tiers[0].tier, 1);
        assert!(life.chaos > 0.05 && life.chaos < 0.6, "{}", life.chaos);
        assert!(life.essences.iter().any(|e| e.name.contains("Greed")));
        assert!(!life.bench.is_empty());
        let int = p.families.iter().find(|f| f.text.contains("to Intelligence") && !f.text.contains("Strength") && !f.text.contains("Dexterity")).unwrap();
        assert!(int.have.is_some());
        // Shaper helmets can roll "Socketed Gems are Supported by…"; plain ones can't.
        let sh = pool("Eternal Burgonet", 85, &["Shaper".into()], &[], &none).unwrap();
        // ...but they're listed on a plain one too, marked as needing Shaper, with no Chaos odds.
        let plain = p.families.iter().find(|f| f.needs.as_deref() == Some("Shaper")).expect("shaper mods listed");
        assert_eq!(plain.chaos, 0.0);
        assert!(sh.families.iter().any(|f| f.key == plain.key && f.chaos > 0.0));
        // Chances of all mods add up to the 4–6 mods a rare gets.
        let sum: f64 = p.families.iter().map(|f| f.chaos).sum();
        assert!((sum - 4.4).abs() < 0.3, "{sum}");
    }

    #[test]
    fn plan_and_influence() {
        let price = |n: &str| match n { "Chaos Orb" => Some(1.0), _ => Some(2.0) };
        let p = pool("Eternal Burgonet", 85, &[], &[], &price).unwrap();
        let life = p.families.iter().find(|f| f.text.contains("to maximum Life") && f.affix == "prefix" && !f.tiers.is_empty() && f.needs.is_none()).unwrap();
        let cold = p.families.iter().find(|f| f.text.contains("to Cold Resistance") && !f.text.contains(" and ") && f.needs.is_none()).unwrap();
        let one = ways("Eternal Burgonet", 85, &[], &life.key, 3, &[], "", &price).into_iter().find(|w| w.method == "Chaos Orb").unwrap().chance;
        let both = plan("Eternal Burgonet", 85, &[], &[(life.key.clone(), 3), (cold.key.clone(), 3)], &price);
        let c = both.iter().find(|w| w.method == "Chaos Orb").unwrap().chance;
        assert!(c > 0.0 && c < one, "{c} vs {one}");
        assert!(both.iter().any(|w| w.method.contains("Essence of") && w.chance > c), "{both:?}");
        // A Shaper-only mod on a plain helmet: the Shaper's Exalted Orb route.
        let sh = p.families.iter().find(|f| f.needs.as_deref() == Some("Shaper")).unwrap();
        let w = ways("Eternal Burgonet", 85, &[], &sh.key, 99, &[], "", &price);
        assert!(w.iter().any(|w| w.method == "Shaper's Exalted Orb" && w.chance > 0.0), "{w:?}");
    }

    #[test]
    fn start_from_his_helmet() {
        let text = "Item Class: Helmets\nRarity: Rare\nDemon Cowl\nEternal Burgonet\n--------\nArmour: 773 (augmented)\n--------\nRequirements:\nLevel: 69\nStr: 138\n--------\nSockets: W-W-W-W \n--------\nItem Level: 85\n--------\n{ Prefix Modifier \"Mammoth's\" (Tier: 1) — Defences, Armour }\n40(39-42)% increased Armour\n16(16-17)% increased Stun and Block Recovery\n{ Prefix Modifier \"Dragon's\" (Tier: 1) — Drop }\n+21(19-24)% to Item Rarity\n{ Prefix Modifier \"Enveloped\" (Tier: 1) — Defences, Armour }\n+152(151-200) to Armour\n{ Suffix Modifier \"of the Polymath\" (Tier: 1) — Attribute }\n+56(56-60) to Intelligence\n{ Suffix Modifier \"of the Ice\" (Tier: 2) — Elemental, Cold, Resistance }\n+45(42-45)% to Cold Resistance\n{ Master Crafted Suffix Modifier \"of Craft\" — Elemental, Fire, Chaos, Resistance }\n+15(13-15)% to Fire and Chaos Resistances\n";
        let s = start(text).unwrap();
        assert_eq!((s.base.as_str(), s.ilvl, s.name.as_str()), ("Eternal Burgonet", 85, "Demon Cowl"));
        assert_eq!(s.have.len(), 7);
        let none = |_: &str| None;
        let p = pool(&s.base, s.ilvl, &s.influences, &s.have, &none).unwrap();
        let cold = p.families.iter().find(|f| f.text.contains("to Cold Resistance") && !f.text.contains(" and ")).unwrap();
        assert_eq!(cold.have, Some(2));
        let int = p.families.iter().find(|f| f.text.ends_with("to Intelligence")).unwrap();
        assert_eq!(int.have, Some(1));
    }

    #[test]
    fn ways_for_life() {
        let price = |n: &str| match n { "Chaos Orb" => Some(1.0), "Jagged Fossil" => Some(1.0), _ => Some(2.0) };
        let p = pool("Eternal Burgonet", 85, &[], &[], &price).unwrap();
        let life = p.families.iter().find(|f| f.text.contains("to maximum Life") && f.affix == "prefix" && !f.tiers.is_empty()).unwrap();
        let w = ways("Eternal Burgonet", 85, &[], &life.key, 1, &[], "", &price);
        assert!(w.iter().any(|w| w.method == "Orb of Alteration" && w.chance > 0.0), "{w:?}");
        let chaos = w.iter().find(|w| w.method == "Chaos Orb").unwrap().chance;
        assert!(w.iter().any(|w| w.method == "Harvest reforge: Life" && w.chance > chaos), "{w:?}");
        // His helmet (3 prefixes): no Exalted Orb for another prefix.
        let w = ways("Eternal Burgonet", 85, &[], &life.key, 1, &["prefix|40% increased Armour".into(), "prefix|16% increased Stun and Block Recovery".into(),
            "prefix|+21% to Item Rarity".into(), "prefix|+152 to Armour".into(), "suffix|+56 to Intelligence".into()], "Rare", &price);
        assert!(!w.iter().any(|w| w.method == "Exalted Orb"), "{w:?}");
        // Same item with only the suffix: an Exalted Orb can add life.
        let w = ways("Eternal Burgonet", 85, &[], &life.key, 2, &["+56 to Intelligence".into()], "Rare", &price);
        assert!(w.iter().any(|w| w.method == "Exalted Orb" && w.chance > 0.0), "{w:?}");
        assert!(w.iter().any(|w| w.method == "Chaos Orb" && w.chance > 0.0 && w.chance < life.chaos));
        assert!(w.iter().any(|w| w.method.contains("Fossil")), "{w:?}");
    }
}
