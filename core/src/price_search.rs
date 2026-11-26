//! Price search (26 Nov 2026, GROBBS: "press a key, type an item name and see its price, like poe.ninja but easier";
//! he picked mock-up B, a list with the chosen item's details beside it). Searches the hourly price file only: no
//! network while typing. Items with several versions (Mageblood with 2-5 flasks, Foulborn ones) are one entry with
//! every version listed.

use crate::prices::{PriceFile, PriceItem};
use serde::Serialize;

/// One version of an item ("4 Flasks"; "" when it has just the one).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Version {
    pub label: String,
    /// In the file's unit (chaos for PoE 1, exalted for PoE 2).
    pub value: f64,
    pub listings: Option<u32>,
    /// Roughly how many sold recently (exchange items).
    pub sold: Option<u32>,
    pub change: Option<f64>,
    pub trend: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Entry {
    pub name: String,
    pub base: String,
    pub cat: String,
    /// The kind in plain words ("Unique accessory", "Divination card", "Currency").
    pub kind: String,
    /// Dearest first. `main` is the one shown first (the most listed, or the only one).
    pub versions: Vec<Version>,
    pub main: usize,
    pub ninja: String,
    pub trade: String,
}

/// poe.ninja's word for a category, as in its page addresses (checked on poe.ninja 26 Nov 2026).
pub fn ninja_category(poe2: bool, cat: &str) -> String {
    let fixed = match (poe2, cat) {
        (true, "Abyss") => Some("abyssal-bones"),
        (true, "Ritual") => Some("omens"),
        (true, "Delirium") => Some("liquid-emotions"),
        (true, "Breach") => Some("breach-catalyst"),
        (true, "Expedition") => Some("expedition"),
        (_, "Currency") => Some("currency"),
        _ => None,
    };
    if let Some(f) = fixed { return f.into(); }
    let mut s = String::new();
    for (i, c) in cat.chars().enumerate() {
        if c.is_uppercase() && i > 0 { s.push('-'); }
        s.push(c.to_ascii_lowercase());
    }
    if s.ends_with('y') { s.pop(); s.push_str("ies"); } else if !s.ends_with('s') { s.push('s'); }
    s
}

/// poe.ninja's word for a league: "Covens" → "covens", "HC Forbidden Rites" → "forbiddenriteshc",
/// "Hardcore Covens" → "covenshc"; Standard and Hardcore stay as they are.
pub fn ninja_league(league: &str) -> String {
    let l = league.trim();
    let (rest, hc) = if let Some(r) = l.strip_prefix("HC ") { (r, true) }
        else if let Some(r) = l.strip_prefix("Hardcore ") { (r, true) } else { (l, false) };
    let mut s: String = rest.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    if hc { s.push_str("hc"); }
    s
}

/// "Maven's Chisel of Avarice" → "mavens-chisel-of-avarice".
fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() { s.push(c.to_ascii_lowercase()); }
        else if c == ' ' || c == '-' { if !s.ends_with('-') { s.push('-'); } }
    }
    s.trim_matches('-').to_string()
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn is_unique(cat: &str) -> bool { cat.starts_with("Unique") }

/// Items poe.ninja shows as one line each with a page of their own (its exchange); the rest are found by name.
fn exchange_item(cat: &str) -> bool {
    !is_unique(cat) && !matches!(cat, "SkillGem" | "Map" | "BlightedMap" | "BlightRavagedMap" | "Beast")
}

pub fn ninja_url(poe2: bool, league: &str, it: &PriceItem) -> String {
    let game = if poe2 { "poe2" } else { "poe1" };
    let page = format!("https://poe.ninja/{game}/economy/{}/{}", ninja_league(league), ninja_category(poe2, &it.cat));
    if exchange_item(&it.cat) { format!("{page}/{}", slug(&it.name)) } else { format!("{page}?name={}", enc(&it.name)) }
}

/// A trade site search for the item, cheapest first (uniques by name and base, the rest by type).
pub fn trade_url(poe2: bool, league: &str, it: &PriceItem) -> String {
    let q = if is_unique(&it.cat) {
        serde_json::json!({ "query": { "status": { "option": "available" }, "name": it.name, "type": it.base }, "sort": { "price": "asc" } })
    } else {
        serde_json::json!({ "query": { "status": { "option": "available" }, "type": it.base }, "sort": { "price": "asc" } })
    };
    let site = if poe2 { format!("https://www.pathofexile.com/trade2/search/poe2/{}", enc(league)) }
        else { format!("https://www.pathofexile.com/trade/search/{}", enc(league)) };
    format!("{site}?q={}", enc(&q.to_string()))
}

/// The kind of item in plain words.
pub fn kind(cat: &str) -> String {
    let plain = match cat {
        "DivinationCard" => "Divination card",
        "SkillGem" => "Gem",
        "LineageSupportGems" => "Lineage support gem",
        "UncutGems" => "Uncut gem",
        "DeliriumOrb" => "Delirium orb",
        "AllflameEmber" => "Allflame ember",
        "BlightedMap" => "Blighted map",
        "BlightRavagedMap" => "Blight-ravaged map",
        "SoulCores" => "Soul core",
        "Abyss" => "Abyssal bone",
        "Ritual" => "Omen",
        "Delirium" => "Liquid emotion",
        "Breach" => "Breach item",
        "Expedition" => "Expedition item",
        _ => "",
    };
    if !plain.is_empty() { return plain.into(); }
    // "UniqueAccessories" → "Unique accessory", "Fragment" → "Fragment".
    let mut words = Vec::new();
    let mut w = String::new();
    for c in cat.chars() {
        if c.is_uppercase() && !w.is_empty() { words.push(std::mem::take(&mut w)); }
        w.push(c);
    }
    if !w.is_empty() { words.push(w); }
    let mut s = words.join(" ").to_lowercase();
    for (from, to) in [("ies", "y"), ("ours", "our"), ("ons", "on"), ("ks", "k"), ("ms", "m"), ("ls", "l"), ("ts", "t"), ("es", "e")] {
        if s.ends_with(from) && !s.ends_with("ss") { s.truncate(s.len() - from.len()); s.push_str(to); break; }
    }
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Every item in the file, versions put together.
pub fn entries(file: &PriceFile) -> Vec<Entry> {
    let poe2 = file.game == "poe2";
    let league = file.league_name.as_deref().unwrap_or(&file.league);
    let mut out: Vec<Entry> = Vec::new();
    let mut seen: std::collections::HashMap<(String, String, String), usize> = Default::default();
    for it in file.items.iter().filter(|i| i.value > 0.0) {
        let v = Version {
            label: it.variant.clone().unwrap_or_default(),
            value: it.value,
            listings: it.listings,
            sold: it.sold().map(|n| n.round() as u32),
            change: it.change,
            trend: it.trend.clone(),
        };
        let key = (it.name.clone(), it.base.clone(), it.cat.clone());
        if let Some(&i) = seen.get(&key) {
            // The same version twice (gems are kept at one price): keep the first.
            if !out[i].versions.iter().any(|x| x.label == v.label) { out[i].versions.push(v); }
            continue;
        }
        seen.insert(key, out.len());
        out.push(Entry {
            name: it.name.clone(), base: it.base.clone(), cat: it.cat.clone(), kind: kind(&it.cat),
            versions: vec![v], main: 0, ninja: ninja_url(poe2, league, it), trade: trade_url(poe2, league, it),
        });
    }
    for e in &mut out {
        e.versions.sort_by(|a, b| b.value.total_cmp(&a.value));
        // The most listed version is the usual one (Mageblood: 4 flasks, not the rare 5).
        e.main = e.versions.iter().enumerate().max_by_key(|(i, v)| (v.listings.unwrap_or(0), std::cmp::Reverse(*i)))
            .map_or(0, |(i, _)| i);
    }
    out
}

fn fold(s: &str) -> String {
    s.chars().filter(|c| *c != '\'' && *c != '’' && *c != ',').collect::<String>().to_lowercase()
}

/// How well a name fits what he typed (lower is better), or None.
fn rank(name: &str, base: &str, q: &str) -> Option<u8> {
    let n = fold(name);
    if n == q { return Some(0); }
    if n.starts_with(q) { return Some(1); }
    if n.split([' ', '-']).any(|w| w.starts_with(q)) { return Some(2); }
    if n.contains(q) { return Some(3); }
    let words: Vec<&str> = q.split_whitespace().collect();
    let hay = format!("{n} {}", fold(base));
    if words.len() > 1 && words.iter().all(|w| hay.split([' ', '-']).any(|h| h.starts_with(w))) { return Some(4); }
    if fold(base).contains(q) { return Some(5); }
    None
}

/// The best matches for what he typed: closest names first, dearer before cheaper.
pub fn search(all: &[Entry], q: &str, limit: usize) -> Vec<Entry> {
    let q = fold(q.trim());
    let q = q.split_whitespace().collect::<Vec<_>>().join(" ");
    if q.is_empty() { return vec![]; }
    let mut hits: Vec<(u8, f64, &Entry)> = all.iter()
        .filter_map(|e| rank(&e.name, &e.base, &q).map(|r| (r, e.versions[e.main].value, e))).collect();
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
    hits.into_iter().take(limit).map(|(_, _, e)| e.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str, base: &str, cat: &str, value: f64, variant: Option<&str>, listings: Option<u32>) -> PriceItem {
        PriceItem { name: name.into(), base: base.into(), cat: cat.into(), value, variant: variant.map(Into::into), listings,
            trend: None, change: None, volume: None }
    }

    fn file() -> PriceFile {
        PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: Some("Covens".into()), updated: String::new(),
            unit: "chaos".into(), bases: vec![], items: vec![
                item("Mageblood", "Heavy Belt", "UniqueAccessory", 390000.0, Some("5 Flasks"), Some(5)),
                item("Mageblood", "Heavy Belt", "UniqueAccessory", 43000.0, Some("4 Flasks"), Some(229)),
                item("Mageblood", "Heavy Belt", "UniqueAccessory", 13554.0, Some("2 Flasks"), Some(67)),
                item("Foulborn Mageblood", "Heavy Belt", "UniqueAccessory", 42484.0, Some("4 Flasks"), Some(24)),
                item("Divine Orb", "Divine Orb", "Currency", 171.7, None, None),
                item("Divine Beauty", "Divine Beauty", "DivinationCard", 92.0, None, None),
                item("Tainted Divine Teardrop", "Tainted Divine Teardrop", "Currency", 634.0, None, None),
                item("The Sorrow of the Divine", "Sulphur Flask", "UniqueFlask", 1.0, None, Some(2377)),
                item("Maven's Chisel of Avarice", "Maven's Chisel of Avarice", "Currency", 5.0, None, None),
                item("Dead", "Dead", "Currency", 0.0, None, None),
            ] }
    }

    #[test]
    fn versions_come_together() {
        let all = entries(&file());
        let mb = all.iter().find(|e| e.name == "Mageblood").unwrap();
        assert_eq!(mb.versions.len(), 3);
        assert_eq!(mb.versions[0].label, "5 Flasks", "dearest first");
        assert_eq!(mb.versions[mb.main].label, "4 Flasks", "the most listed is the main one");
        assert_eq!(mb.kind, "Unique accessory");
        assert!(!all.iter().any(|e| e.name == "Dead"), "no price, not shown");
    }

    #[test]
    fn finds_by_start_then_word_then_part() {
        let all = entries(&file());
        let names: Vec<String> = search(&all, "divine", 10).into_iter().map(|e| e.name).collect();
        assert_eq!(names, ["Divine Orb", "Divine Beauty", "Tainted Divine Teardrop", "The Sorrow of the Divine"]);
        assert_eq!(search(&all, "mavens chisel", 5)[0].name, "Maven's Chisel of Avarice");
        assert_eq!(search(&all, "heavy belt", 5).len(), 2, "uniques by their base too");
        assert_eq!(search(&all, "MAGEB", 5)[0].name, "Mageblood");
        assert!(search(&all, "  ", 5).is_empty());
    }

    #[test]
    fn links() {
        let f = file();
        let get = |n: &str| f.items.iter().find(|i| i.name == n).unwrap();
        assert_eq!(ninja_url(false, "Covens", get("Divine Orb")), "https://poe.ninja/poe1/economy/covens/currency/divine-orb");
        assert_eq!(ninja_url(false, "Covens", get("Maven's Chisel of Avarice")), "https://poe.ninja/poe1/economy/covens/currency/mavens-chisel-of-avarice");
        assert_eq!(ninja_url(false, "Covens", get("Mageblood")), "https://poe.ninja/poe1/economy/covens/unique-accessories?name=Mageblood");
        assert_eq!(ninja_url(false, "Covens", get("Divine Beauty")), "https://poe.ninja/poe1/economy/covens/divination-cards/divine-beauty");
        assert_eq!(ninja_league("HC Forbidden Rites"), "forbiddenriteshc");
        assert_eq!(ninja_league("Hardcore"), "hardcore");
        assert_eq!(ninja_category(true, "UniqueArmours"), "unique-armours");
        assert_eq!(ninja_category(true, "Ritual"), "omens");
        assert_eq!(ninja_category(false, "DeliriumOrb"), "delirium-orbs");
        assert!(trade_url(false, "Covens", get("Mageblood")).starts_with("https://www.pathofexile.com/trade/search/Covens?q=%7B"));
        assert!(trade_url(true, "Forbidden Rites", get("Divine Orb")).starts_with("https://www.pathofexile.com/trade2/search/poe2/Forbidden%20Rites?q="));
        assert_eq!(kind("UniqueAccessories"), "Unique accessory");
        assert_eq!(kind("UniqueArmour"), "Unique armour");
        assert_eq!(kind("Fragment"), "Fragment");
        assert_eq!(kind("Essences"), "Essence");
        assert_eq!(kind("Runes"), "Rune");
        assert_eq!(kind("Currency"), "Currency");
    }
}
