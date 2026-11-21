//! Crafting helper: which mods can roll on a base (tiers, item level, how common), and for one mod, the ways to get it
//! (essence, crafting bench, fossils, plain Chaos Orbs) with rough odds and cost.
//! Data: core/data/poe1_craft.json, built by tools/build_craft_data.mjs from the game's files (RePoE).
//! Odds come from rolling the item many times here (like Craft of Exile's simulator, much simpler): a rare gets
//! 4 mods (8 in 12), 5 (3 in 12) or 6 (1 in 12), at most 3 prefixes and 3 suffixes, one mod per group,
//! each picked by its weight.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

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
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pool { pub base: String, pub class: String, pub ilvl: u32, pub influences: Vec<String>, pub families: Vec<Family> }

/// One way to get a mod, with its odds.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Way {
    /// "Chaos Orb", "Shrieking Essence of Greed", "Jagged Fossil + Pristine Fossil"…
    pub method: String,
    /// Chance per try, 0–1.
    pub chance: f64,
    /// What one try costs in chaos (None = price unknown).
    pub cost_each: Option<f64>,
    /// Average cost to hit it (cost_each / chance).
    pub average: Option<f64>,
    pub note: String,
}

/// "+(15-25)% to Fire Resistance" / "+56 to Intelligence" -> "+#% to fire resistance" / "+# to intelligence".
fn shape(text: &str) -> String {
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
    let mut groups: HashMap<&str, usize> = HashMap::new();
    d.mods.iter().enumerate().filter(|(_, m)| m.src.is_empty() && m.lvl <= ilvl).filter_map(|(i, m)| {
        let w = weight(m, tags) * fossil_factor(m, fossils);
        if w <= 0.0 { return None }
        let n = groups.len();
        let g = *groups.entry(m.group.as_str()).or_insert(n);
        Some((i, w, m.gen == "p", g))
    }).collect()
}

/// Rolls a rare `trials` times; counts how often each mod lands.
fn simulate(pool: &[(usize, f64, bool, usize)], trials: u32) -> HashMap<usize, u32> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut counts = vec![0u32; pool.len()];
    let mut used: Vec<usize> = Vec::with_capacity(6);
    for _ in 0..trials {
        let r = rng.next();
        let n = if r < 8.0 / 12.0 { 4 } else if r < 11.0 / 12.0 { 5 } else { 6 };
        let (mut pre, mut suf) = (0, 0);
        used.clear();
        for _ in 0..n {
            let open = |p: bool, g: usize, pre: i32, suf: i32, used: &[usize]| !used.contains(&g) && if p { pre < 3 } else { suf < 3 };
            let total: f64 = pool.iter().filter(|(_, _, p, g)| open(*p, *g, pre, suf, &used)).map(|(_, w, ..)| w).sum();
            if total <= 0.0 { break }
            let mut x = rng.next() * total;
            for (k, (_, w, p, g)) in pool.iter().enumerate() {
                if !open(*p, *g, pre, suf, &used) { continue }
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
    let mut members: Vec<Vec<usize>> = Vec::new();
    // Whole-mod wording (both lines of a hybrid) -> family, for bench and essence mods.
    let mut whole: HashMap<String, usize> = HashMap::new();
    for (i, m) in d.mods.iter().enumerate() {
        if !m.src.is_empty() || weight(m, &tags) <= 0.0 { continue }
        let key = format!("{}|{}", m.gen, m.r#type);
        let f = *by_key.entry(key.clone()).or_insert_with(|| {
            fams.push(Family { key, affix: if m.gen == "p" { "prefix" } else { "suffix" }.into(), text: String::new(), tags: m.tags.clone(),
                tiers: vec![], chaos: 0.0, have: None, essences: vec![], bench: vec![], fossils_more: vec![], fossils_none: vec![] });
            members.push(vec![]);
            fams.len() - 1
        });
        members[f].push(i);
        for line in m.text.lines() { shapes.entry(shape(line)).or_insert(f); }
        whole.entry(shape(&m.text)).or_insert(f);
    }
    for (f, ids) in members.iter().enumerate() {
        let mut ids = ids.clone();
        ids.sort_by(|a, b| d.mods[*b].lvl.cmp(&d.mods[*a].lvl));
        let fam = &mut fams[f];
        fam.tiers = ids.iter().enumerate().map(|(n, i)| { let m = &d.mods[*i];
            Tier { tier: n as u32 + 1, name: m.name.clone(), lvl: m.lvl, text: m.text.clone(), weight: weight(m, &tags), ok: m.lvl <= ilvl } }).collect();
        fam.text = fam.tiers.iter().find(|t| t.ok).or(fam.tiers.last()).map(|t| t.text.clone()).unwrap_or_default();
        fam.chaos = ids.iter().map(|i| counts.get(i).copied().unwrap_or(0)).sum::<u32>() as f64 / TRIALS as f64;
    }
    // Bench and essence mods: on the family with the same wording, or a family of their own ("bench only").
    let attach = |m: &RawMod, fams: &mut Vec<Family>, shapes: &mut HashMap<String, usize>| -> usize {
        let s = shape(&m.text);
        if let Some(&f) = shapes.get(&s) { return f }
        fams.push(Family { key: format!("{}|{}", m.gen, m.r#type), affix: if m.gen == "p" { "prefix" } else { "suffix" }.into(), text: m.text.clone(),
            tags: m.tags.clone(), tiers: vec![], chaos: 0.0, have: None, essences: vec![], bench: vec![], fossils_more: vec![], fossils_none: vec![] });
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
    // What the item has now.
    for line in have {
        let s = shape(line);
        if let Some(&f) = shapes.get(&s) {
            let v = first_top(line.trim_start_matches(['+', '-']));
            fams[f].have = Some(v.and_then(|v| fams[f].tiers.iter().find(|t| first_bottom(&t.text).is_some_and(|b| b <= v)).map(|t| t.tier)).unwrap_or(0));
        }
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
    Some(Pool { base: base.to_string(), class, ilvl, influences: influences.to_vec(), families: fams })
}

/// The ways to get one family at `min_tier` or better (1 = best), cheapest first.
pub fn ways(base: &str, ilvl: u32, influences: &[String], key: &str, min_tier: u32, price: &dyn Fn(&str) -> Option<f64>) -> Vec<Way> {
    let d = data();
    let Some(p) = pool(base, ilvl, influences, &[], price) else { return vec![] };
    let Some(fam) = p.families.iter().find(|f| f.key == key) else { return vec![] };
    let Some((_, tags)) = tagset(base, influences) else { return vec![] };
    let wanted: HashSet<&str> = fam.tiers.iter().filter(|t| t.tier <= min_tier).map(|t| t.name.as_str()).collect();
    let is_target = |i: &usize| { let m = &d.mods[*i]; format!("{}|{}", m.gen, m.r#type) == fam.key && wanted.contains(m.name.as_str()) };
    let hit = |counts: &HashMap<usize, u32>| counts.iter().filter(|(i, _)| is_target(i)).map(|(_, n)| *n).sum::<u32>() as f64 / TRIALS as f64;
    let mut out = Vec::new();
    let chaos = price("Chaos Orb").unwrap_or(1.0);
    // Chaos Orbs.
    if !fam.tiers.is_empty() {
        let c = hit(&simulate(&rollable(&tags, ilvl, &[]), TRIALS));
        out.push(Way { method: "Chaos Orb".into(), chance: c, cost_each: Some(chaos), average: (c > 0.0).then(|| chaos / c), note: "Reroll a rare item".into() });
    }
    // Essences that give it outright.
    for e in &fam.essences {
        if e.like_tier.is_some_and(|t| t <= min_tier) || fam.tiers.is_empty() {
            out.push(Way { method: e.name.clone(), chance: 1.0, cost_each: e.price, average: e.price,
                note: if e.corrupt { "Always gives it (corrupted essence: needs a Remnant of Corruption)".into() } else { format!("Always gives {}", e.text.replace('\n', " / ")) } });
        }
    }
    // Bench.
    if let Some(b) = fam.bench.first() {
        let cost: Option<f64> = b.cost.iter().map(|(c, n)| price(c).or_else(|| (c == "Chaos Orb").then_some(1.0)).map(|v| v * *n as f64)).sum();
        let like = first_bottom(&b.text).and_then(|v| fam.tiers.iter().find(|t| first_bottom(&t.text).is_some_and(|x| x <= v))).map(|t| format!(", like tier {}", t.tier)).unwrap_or_default();
        out.push(Way { method: "Crafting bench".into(), chance: 1.0, cost_each: cost, average: cost,
            note: format!("{}{like} (needs a free {} slot; one bench craft per item)", b.text.replace('\n', " / "), fam.affix) });
    }
    // Fossils: each one that helps, and the best pair.
    if !fam.tiers.is_empty() {
        let helpful: Vec<&RawFossil> = d.fossils.iter().filter(|f| fam.fossils_more.contains(&f.name)).collect();
        // Fossils that only block mods it doesn't have also help (fewer other mods in the way).
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
            Way { method: fs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" + "), chance: c, cost_each: each,
                average: each.filter(|_| c > 0.0).map(|e| e / c), note: format!("In a {res}") }
        }).filter(|w| w.chance > 0.0).collect();
        tried.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)
            .then(b.chance.partial_cmp(&a.chance).unwrap_or(std::cmp::Ordering::Equal)));
        out.extend(tried.into_iter().take(3));
    }
    out.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// A copied item, ready for the Crafting page: its base, item level, influences and mod lines.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Start { pub base: String, pub ilvl: u32, pub influences: Vec<String>, pub have: Vec<String>, pub name: String }

pub fn start(text: &str) -> Option<Start> {
    let item = crate::item::parse_item(text)?;
    let base = find_base(&item.base_type)?;
    let have = crate::trade::mods(text).into_iter().filter(|m| m.kind == "explicit" || m.kind == "crafted" || m.kind == "fractured").map(|m| m.text).collect();
    Some(Start { base, ilvl: item.item_level.unwrap_or(84), influences: item.influences.clone(), have,
        name: item.name.clone().unwrap_or_else(|| item.base_type.clone()) })
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
        assert!(sh.families.len() > p.families.len());
        // Chances of all mods add up to the 4–6 mods a rare gets.
        let sum: f64 = p.families.iter().map(|f| f.chaos).sum();
        assert!((sum - 4.4).abs() < 0.3, "{sum}");
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
        let w = ways("Eternal Burgonet", 85, &[], &life.key, 1, &price);
        assert!(w.iter().any(|w| w.method == "Chaos Orb" && w.chance > 0.0 && w.chance < life.chaos));
        assert!(w.iter().any(|w| w.method.contains("Fossil")), "{w:?}");
    }
}
