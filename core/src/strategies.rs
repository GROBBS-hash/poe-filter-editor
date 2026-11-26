//! Creator farms for "What to farm today" (26 Nov 2026): whole strategies from this week's videos (Vaal Temple boss rush,
//! Sanctuary vaults, Catarina medallions, Reverence chains, Essences + Crystal Resonance, 1-Thread Thread farming), ranked by
//! today's poe.ninja prices of their key items, after what a run costs. Data: `data/farm_strategies.json` (research:
//! `.claude/atlas-research/report_new_since_22nov.md`).
//!
//! Profit per real hour = runs an hour (the creator's, hideout time included) x (loot after selling - map - rolling - Threads -
//! Vaal Orbs - entry items). Jackpots (a Headhunter in a vault) are kept apart: they need many maps before they show.

use crate::atlas_opt::Ask;
use crate::prices::{PriceFile, PriceItem};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

const DATA: &str = include_str!("../data/farm_strategies.json");

#[derive(Debug, Clone, Deserialize)]
struct Line { label: String, names: Vec<String>, per_run: f64, #[serde(default = "one")] sell: f64, #[serde(default)] jackpot: bool, note: Option<String> }
fn one() -> f64 { 1.0 }

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Costs { map: Option<String>, map_chaos: f64, roll_chaos: f64, threads: Vec<(String, u32)>, vaal: f64, entry: Vec<(String, f64)> }

/// Who it's from and how much to trust it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source { pub who: String, pub video: String, pub date: String, pub sample: String, pub trust: String, pub said: String }

#[derive(Debug, Clone, Deserialize)]
struct Strategy {
    key: String,
    title: String,
    pic: String,
    #[serde(default)]
    mech: Vec<String>,
    how: String,
    runs_hour: f64,
    #[serde(default)]
    runs_guess: bool,
    unit: String,
    #[serde(default)]
    loot: Vec<Line>,
    #[serde(default)]
    other_div: f64,
    costs: Costs,
    #[serde(default)]
    jackpot: bool,
    #[serde(default)]
    one_item: bool,
    source: Source,
    #[serde(default)]
    ask: Ask,
    #[serde(default)]
    maps: Vec<String>,
    #[serde(default)]
    drops: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Data { checked: String, strategies: Vec<Strategy> }

fn data() -> &'static Data { static D: OnceLock<Data> = OnceLock::new(); D.get_or_init(|| serde_json::from_str(DATA).expect("farm_strategies.json")) }

/// When the farms were last read from the creators.
pub fn checked() -> &'static str { &data().checked }

/// One kind of loot in a run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Loot {
    pub label: String,
    /// For its picture.
    pub name: String,
    /// poe.ninja's price, chaos (the average of its names); None = no price today.
    pub each: Option<f64>,
    pub per_run: f64,
    /// Share of the price you get selling it.
    pub sell: f64,
    /// Chaos a run, after selling.
    pub chaos: f64,
    pub jackpot: bool,
    pub note: Option<String>,
}

/// One cost of a run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cost { pub label: String, pub item: String, pub count: f64, pub chaos: f64 }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Strat {
    pub key: String,
    pub title: String,
    pub pic: String,
    pub mech: Vec<String>,
    pub how: String,
    /// "map", "chain", "fight".
    pub unit: String,
    pub runs_hour: f64,
    /// Runs an hour is our guess (the creator didn't say).
    pub runs_guess: bool,
    pub loot: Vec<Loot>,
    /// The creator's other loot a run (gold, Lost Shipments, invitations…), chaos at today's divine.
    pub other: f64,
    pub costs: Vec<Cost>,
    pub cost: f64,
    /// Chaos a run: steady loot (other included) and jackpots, after selling.
    pub steady: f64,
    pub jackpots: f64,
    /// Divines an hour after costs, without jackpots, and with them.
    pub div_hour: f64,
    pub div_hour_all: f64,
    /// What it's ranked by: steady profit plus half the jackpots.
    pub rank: f64,
    /// Jackpot farm: needs a big sample before it shows.
    pub big_sample: bool,
    /// Rides on one item's price.
    pub one_item: Option<String>,
    /// Key items with no price today (counted as nothing).
    pub missing: Vec<String>,
    pub source: Source,
    pub ask: Ask,
    pub maps: Vec<String>,
    /// For the filter: the key items and drops.
    pub drops: Vec<String>,
    /// Threads a run (item name, how many) and the builder's keys for them.
    pub threads: Vec<(String, u32)>,
    pub thread_keys: Vec<(String, u32)>,
    /// For Farm this: the map's price, rolling, Vaal Orbs and entry items a run.
    pub map_cost: f64,
    pub roll_cost: f64,
    pub vaal: f64,
    pub entry: Vec<(String, f64)>,
}

/// The item poe.ninja trades most under this name (Mageblood has a price per flask count).
pub fn item<'a>(file: &'a PriceFile, name: &str) -> Option<&'a PriceItem> {
    file.items.iter().filter(|i| i.name == name && i.value > 0.0)
        .max_by(|a, b| a.listings.unwrap_or(0).cmp(&b.listings.unwrap_or(0)).then(a.volume.unwrap_or(0.0).total_cmp(&b.volume.unwrap_or(0.0))))
}

/// Share of poe.ninja's price you get when you sell it: slow sellers go lower or sit unsold (research 26 Nov: T16 maps,
/// inscribed Ultimatums at 60-70%, thinly traded items).
pub fn sell_through(i: &PriceItem, divine: f64) -> f64 {
    if i.name.contains("Inscribed Ultimatum") || i.name.ends_with("Warrant") { return 0.65; }
    if i.cat == "Map" || i.cat == "BlightedMap" { return 0.75; }
    match (i.volume, i.listings) {
        // Traded in bulk: divines' worth sold lately.
        (Some(v), _) => match v / divine.max(1.0) { d if d < 20.0 => 0.8, d if d < 200.0 => 0.9, _ => 1.0 },
        (None, Some(n)) => match n { n if n < 20 => 0.75, n if n < 100 => 0.85, _ => 0.9 },
        _ => 0.85,
    }
}

fn thread_key(name: &str) -> String { name.trim_end_matches(" Thread").to_lowercase() }

/// Every creator farm at today's prices, best first.
pub fn rank(file: &PriceFile) -> Vec<Strat> {
    let div = item(file, "Divine Orb").map(|i| i.value).unwrap_or(1.0);
    let price = |n: &str| item(file, n).map(|i| i.value);
    let mut out: Vec<Strat> = data().strategies.iter().map(|s| {
        let mut missing = vec![];
        let loot: Vec<Loot> = s.loot.iter().map(|l| {
            let found: Vec<&PriceItem> = l.names.iter().filter_map(|n| item(file, n)).collect();
            missing.extend(l.names.iter().filter(|n| !found.iter().any(|f| &f.name == *n)).cloned());
            let each = (!found.is_empty()).then(|| found.iter().map(|i| i.value).sum::<f64>() / found.len() as f64);
            let st = if found.is_empty() { 1.0 } else { found.iter().map(|i| sell_through(i, div)).sum::<f64>() / found.len() as f64 };
            let sell = l.sell.min(st);
            Loot { label: l.label.clone(), name: l.names[0].clone(), each, per_run: l.per_run, sell, chaos: each.unwrap_or(0.0) * l.per_run * sell, jackpot: l.jackpot || s.jackpot, note: l.note.clone() }
        }).collect();
        let other = s.other_div * div;
        let c = &s.costs;
        let mut costs = vec![];
        let map_cost = c.map.as_deref().and_then(price).unwrap_or(c.map_chaos);
        if map_cost > 0.0 { costs.push(Cost { label: "The map".into(), item: c.map.clone().unwrap_or_else(|| "Map (Tier 16)".into()), count: 1.0, chaos: map_cost }); }
        if c.roll_chaos > 0.0 { costs.push(Cost { label: "Rolling it".into(), item: "Orb of Alchemy".into(), count: 1.0, chaos: c.roll_chaos }); }
        for (t, n) in &c.threads { costs.push(Cost { label: t.clone(), item: t.clone(), count: *n as f64, chaos: price(t).unwrap_or(0.0) * *n as f64 }); }
        if c.vaal > 0.0 { costs.push(Cost { label: "Vaal Orbs".into(), item: "Vaal Orb".into(), count: c.vaal, chaos: price("Vaal Orb").unwrap_or(1.0) * c.vaal }); }
        for (e, n) in &c.entry {
            if price(e).is_none() { missing.push(e.clone()); }
            costs.push(Cost { label: e.clone(), item: e.clone(), count: *n, chaos: price(e).unwrap_or(0.0) * n });
        }
        let cost: f64 = costs.iter().map(|x| x.chaos).sum();
        // A farm that's all jackpot (Catarina's loot isn't itemised) counts its other loot as steady but swings a lot.
        let steady = loot.iter().filter(|l| !l.jackpot || s.jackpot).map(|l| l.chaos).sum::<f64>() + other;
        let jackpots = loot.iter().filter(|l| l.jackpot && !s.jackpot).map(|l| l.chaos).sum::<f64>();
        let div_hour = (steady - cost) * s.runs_hour / div;
        let div_hour_all = (steady + jackpots - cost) * s.runs_hour / div;
        let one_item = s.one_item.then(|| loot.iter().max_by(|a, b| a.chaos.total_cmp(&b.chaos)).map(|l| l.label.clone())).flatten();
        let mut drops: Vec<String> = s.loot.iter().flat_map(|l| l.names.iter().cloned()).chain(s.drops.iter().cloned()).collect();
        drops.dedup();
        Strat {
            key: s.key.clone(), title: s.title.clone(), pic: s.pic.clone(), mech: s.mech.clone(), how: s.how.clone(), unit: s.unit.clone(),
            runs_hour: s.runs_hour, runs_guess: s.runs_guess, loot, other, costs, cost, steady, jackpots, div_hour, div_hour_all,
            rank: div_hour + (div_hour_all - div_hour) * 0.5,
            big_sample: s.jackpot || jackpots > 0.0, one_item, missing,
            source: s.source.clone(), ask: s.ask.clone(), maps: s.maps.clone(), drops,
            threads: c.threads.clone(), thread_keys: c.threads.iter().map(|(t, n)| (thread_key(t), *n)).collect(),
            map_cost, roll_cost: c.roll_chaos, vaal: c.vaal, entry: c.entry.clone(),
        }
    }).collect();
    out.sort_by(|a, b| b.rank.total_cmp(&a.rank));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(name: &str, cat: &str, value: f64, volume: Option<f64>, listings: Option<u32>) -> PriceItem {
        PriceItem { name: name.into(), base: name.into(), cat: cat.into(), value, variant: None, listings, trend: None, change: None, volume }
    }
    fn file(medallion: f64) -> PriceFile {
        let mut items = vec![it("Divine Orb", "Currency", 170.0, Some(1e7), None), it("Vaal Orb", "Currency", 0.5, Some(1e5), None),
            it("Vaal Temple Map", "Map", 40.0, None, Some(900)), it("Nightmare Map", "Map", 60.0, None, Some(60000)), it("Map (Tier 16)", "Map", 2.0, None, Some(100000)),
            it("Syndicate Medallion", "Fragment", medallion, Some(2e5), None), it("Echo of Reverence", "Fragment", 300.0, Some(2e5), None),
            it("Memory of Reverence", "Fragment", 88.0, Some(9e4), None), it("Legion Thread", "Thread", 6.0, Some(6e5), None),
            it("Headhunter", "UniqueAccessory", 36000.0, None, Some(245)), it("Mageblood", "UniqueAccessory", 13000.0, None, Some(60)),
            it("Mageblood", "UniqueAccessory", 43000.0, None, Some(229)), it("House of Mirrors", "DivinationCard", 17500.0, Some(1.5e7), None)];
        for n in ["Soul Core of Might", "Soul Core of Finesse", "Soul Core of Intellect", "Soul Core of Instability"] { items.push(it(n, "Currency", 1500.0, Some(2e5), None)); }
        for t in ["Abyss", "Bestiary", "Ritual", "Incursion", "Expedition"] { items.push(it(&format!("{t} Thread"), "Thread", 1.0, Some(2e4), None)); }
        PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: None, updated: "now".into(), unit: "chaos".into(), bases: vec![], items }
    }

    #[test]
    fn creator_farms_at_todays_prices() {
        let all = rank(&file(677.0));
        assert_eq!(all.len(), 6);
        let vaal = all.iter().find(|s| s.key == "vaal_temple").unwrap();
        // 37.8 maps an hour x (0.15 x 1500c x 0.95 + 0.94 div - 40c map) / 170 ≈ 79 div.
        assert!((vaal.div_hour - 37.8 * (0.15 * 1500.0 * 0.95 + 0.94 * 170.0 - 40.0) / 170.0).abs() < 1e-6, "{}", vaal.div_hour);
        assert_eq!(all[0].key, "vaal_temple", "{:?}", all.iter().map(|s| (&s.key, s.rank)).collect::<Vec<_>>());
        assert!(vaal.one_item.is_some() && !vaal.big_sample);
        let sanct = all.iter().find(|s| s.key == "sanctuary_vaults").unwrap();
        assert!(sanct.big_sample && sanct.jackpots > 0.0 && sanct.div_hour_all > sanct.div_hour);
        // The 4-flask Mageblood most people sell, at 85% (few listed).
        let mb = sanct.loot.iter().find(|l| l.name == "Mageblood").unwrap();
        assert_eq!((mb.each, mb.sell), (Some(43000.0), 0.85));
        let ess = all.iter().find(|s| s.key == "essence_resonance").unwrap();
        assert!(ess.costs.iter().any(|c| c.item == "Vaal Orb" && c.chaos == 6.0), "12 Vaal Orbs at 0.5c");
        assert!(!ess.missing.is_empty(), "essences with no price are listed, not guessed");
        // Medallions twice as dear: Catarina falls behind.
        let cat = |f: &PriceFile| rank(f).into_iter().find(|s| s.key == "catarina").unwrap().div_hour;
        assert!(cat(&file(1400.0)) < cat(&file(677.0)) - 20.0);
        assert!(all.iter().find(|s| s.key == "catarina").unwrap().big_sample);
        assert_eq!(all.iter().find(|s| s.key == "thread_farm").unwrap().thread_keys, [("abyss".to_string(), 1)]);
    }

    #[test]
    fn slow_sellers_sell_for_less() {
        let d = 170.0;
        assert_eq!(sell_through(&it("Divine Orb", "Currency", 170.0, Some(1e7), None), d), 1.0);
        assert_eq!(sell_through(&it("Rare thing", "Fragment", 5.0, Some(500.0), None), d), 0.8);
        assert_eq!(sell_through(&it("Nightmare Map", "Map", 60.0, None, Some(60000)), d), 0.75);
        assert_eq!(sell_through(&it("Inscribed Ultimatum", "Other", 170.0, None, Some(500)), d), 0.65);
        assert_eq!(sell_through(&it("Odd Unique", "UniqueArmour", 50.0, None, Some(8)), d), 0.75);
    }
}
