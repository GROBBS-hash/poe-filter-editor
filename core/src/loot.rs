//! Loot per map: what you picked up, from your inventory on pathofexile.com before and after each visit to a map
//! (the site saves your character every time you change zone), sorted into kinds and priced with poe.ninja.

use crate::wealth::{self, Lookup};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// One thing a map gave you (the same item added up across stacks and visits).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LootLine {
    pub name: String,
    /// "currency", "cards", "uniques", "maps", "scarabs", "fragments", "gems", "gear" or "other".
    pub cat: String,
    pub stack: u32,
    /// Chaos for all of them (0 when poe.ninja has no price).
    pub value: f64,
    pub icon: String,
    /// Uniques: 1 (worth a divine or more), 2 (10 chaos or more) or 3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<u8>,
    /// An unidentified unique: the most it could be worth (value is the cheapest unique on that base).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
}

/// The kinds, in the order they're shown, with their names.
pub const KINDS: [(&str, &str); 9] = [
    ("currency", "Currency"), ("cards", "Div cards"), ("uniques", "Uniques"), ("maps", "Maps"), ("scarabs", "Scarabs"),
    ("fragments", "Fragments"), ("gems", "Gems"), ("gear", "Gear"), ("other", "Other"),
];

/// Your choices for the loot breakdown (Settings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LootPrefs {
    pub on: bool,
    /// Kinds left out of the list (still counted in the total).
    pub hide: Vec<String>,
    /// Items worth less than this (chaos) aren't listed one by one (still counted).
    pub min_chaos: f64,
}

impl Default for LootPrefs {
    fn default() -> Self { Self { on: true, hide: vec!["gear".into()], min_chaos: 2.0 } }
}

/// Only the items in your bags (not what you're wearing or your flasks).
pub fn bag_items(items: &Value) -> Vec<Value> {
    items.as_array().map(|a| a.iter().filter(|i| i["inventoryId"].as_str() == Some("MainInventory")).cloned().collect()).unwrap_or_default()
}

fn stack_key(i: &Value) -> String {
    format!("{}|{}|{}", i["frameType"].as_u64().unwrap_or(0), i["name"].as_str().unwrap_or(""), i["typeLine"].as_str().unwrap_or(""))
}

/// What's new in `after` compared with `before` (both your bag items). Stackable things (currency, cards,
/// splinters…) count by how many more you have; everything else by items that weren't there. Things you used up
/// or dropped are ignored.
pub fn gained(before: &[Value], after: &[Value]) -> Vec<Value> {
    let mut had: HashMap<String, u64> = HashMap::new();
    let mut ids: std::collections::HashSet<&str> = Default::default();
    for i in before {
        if i.get("stackSize").is_some() { *had.entry(stack_key(i)).or_default() += i["stackSize"].as_u64().unwrap_or(1); }
        if let Some(id) = i["id"].as_str() { ids.insert(id); }
    }
    let mut now: Vec<(String, u64, &Value)> = vec![];
    let mut out = vec![];
    for i in after {
        if i.get("stackSize").is_some() {
            let k = stack_key(i);
            let n = i["stackSize"].as_u64().unwrap_or(1);
            match now.iter_mut().find(|x| x.0 == k) { Some(x) => x.1 += n, None => now.push((k, n, i)) }
        } else if !i["id"].as_str().is_some_and(|id| ids.contains(id)) {
            out.push(i.clone());
        }
    }
    for (k, n, i) in now {
        let more = n.saturating_sub(*had.get(&k).unwrap_or(&0));
        if more > 0 {
            let mut c = i.clone();
            c["stackSize"] = more.into();
            out.push(c);
        }
    }
    out
}

/// Which kind an item is.
pub fn kind(i: &Value) -> &'static str {
    let t = i["typeLine"].as_str().unwrap_or("");
    let frame = i["frameType"].as_u64().unwrap_or(0);
    let is_map = t.contains(" Map") || t.starts_with("Map (") || i["properties"].as_array().is_some_and(|p| p.iter().any(|x| x["name"] == "Map Tier"));
    match frame {
        6 => "cards",
        3 | 9 => if is_map { "maps" } else { "uniques" },
        4 => "gems",
        _ if t.contains("Scarab") => "scarabs",
        _ if is_map => "maps",
        _ if ["Splinter", "Fragment", "Breachstone", "Emblem", "Invitation", "Reliquary Key", "Crest", "Sacrifice at", "Mortal ", "Divine Vessel", "Offering to the Goddess"]
            .iter().any(|w| t.contains(w)) => "fragments",
        5 => "currency",
        0..=2 if i["frameType"].is_number() && (i.get("w").is_some() && i["w"].as_u64().unwrap_or(1) * i["h"].as_u64().unwrap_or(1) > 1 || i["typeLine"].as_str().is_some_and(|t| t.contains("Ring") || t.contains("Amulet") || t.contains("Belt") || t.contains("Jewel"))) => "gear",
        _ => "other",
    }
}

/// Unique tier by worth: 1 = a divine or more, 2 = 10 chaos or more, 3 = the rest.
pub fn unique_tier(chaos_each: f64, divine: f64) -> u8 {
    if divine > 0.0 && chaos_each >= divine { 1 } else if chaos_each >= 10.0 { 2 } else { 3 }
}

/// Prices the new items and adds them up by name.
pub fn lines(new: &[Value], lookup: Lookup, divine: f64) -> Vec<LootLine> {
    let mut out: Vec<LootLine> = vec![];
    for i in new {
        let cat = kind(i);
        let v = wealth::value_item(i, "", lookup);
        let stack = i["stackSize"].as_u64().unwrap_or(1).max(1) as u32;
        let name = match &v {
            Some(v) => v.name.clone(),
            None => {
                let n = i["name"].as_str().unwrap_or("").trim();
                let t = i["typeLine"].as_str().unwrap_or("");
                if n.is_empty() { t.to_string() } else { format!("{n} {t}") }
            }
        };
        // An unidentified unique only says its base.
        let unid = cat == "uniques" && i["identified"] == false && i["name"].as_str().unwrap_or("").is_empty();
        let name = if unid { format!("Unidentified {name}") } else { name };
        let value = v.as_ref().map(|v| v.value).unwrap_or(0.0);
        // Could be a pricier unique on the same base: remember the top, so the line says "8c – 2 div".
        let base = i["baseType"].as_str().or(i["typeLine"].as_str()).unwrap_or("");
        let high = unid.then(|| lookup(None, base, "unique-max")).flatten().map(|h| h * stack as f64).filter(|h| *h > value + 0.5);
        let tier = (cat == "uniques").then(|| unique_tier(value / stack as f64, divine));
        let icon = i["icon"].as_str().unwrap_or("").to_string();
        add(&mut out, LootLine { name, cat: cat.into(), stack, value, icon, tier, high });
    }
    sort(&mut out);
    out
}

fn add(into: &mut Vec<LootLine>, l: LootLine) {
    match into.iter_mut().find(|x| x.name == l.name && x.cat == l.cat) {
        Some(x) => {
            if x.high.is_some() || l.high.is_some() { x.high = Some(x.high.unwrap_or(x.value) + l.high.unwrap_or(l.value)); }
            x.stack += l.stack; x.value += l.value; if x.icon.is_empty() { x.icon = l.icon; }
        }
        None => into.push(l),
    }
}

fn sort(v: &mut [LootLine]) { v.sort_by(|a, b| b.value.total_cmp(&a.value).then_with(|| a.name.cmp(&b.name))); }

/// Adds another visit's loot to a run's.
pub fn merge(into: &mut Vec<LootLine>, more: Vec<LootLine>) {
    for l in more { add(into, l); }
    sort(into);
}

/// Total chaos of some loot.
pub fn total(v: &[LootLine]) -> f64 { v.iter().map(|l| l.value).sum() }

/// Chaos per kind (in the KINDS order, kinds with nothing left out).
pub fn by_kind(v: &[LootLine]) -> Vec<(String, f64, u32)> {
    KINDS.iter().filter_map(|(k, _)| {
        let ls: Vec<&LootLine> = v.iter().filter(|l| l.cat == *k).collect();
        (!ls.is_empty()).then(|| (k.to_string(), ls.iter().map(|l| l.value).sum(), ls.iter().map(|l| l.stack).sum()))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cur(id: &str, t: &str, n: u64) -> Value { json!({"id": id, "inventoryId": "MainInventory", "frameType": 5, "typeLine": t, "baseType": t, "stackSize": n, "w": 1, "h": 1, "icon": "x.png"}) }

    #[test]
    fn finds_what_you_picked_up() {
        let before = vec![cur("a", "Chaos Orb", 2), cur("b", "Portal Scroll", 38),
            json!({"id": "m1", "inventoryId": "MainInventory", "frameType": 0, "typeLine": "Map (Tier 16)", "w": 1, "h": 1})];
        let after = vec![cur("a", "Chaos Orb", 10), cur("c", "Chaos Orb", 5), cur("b", "Portal Scroll", 37), cur("d", "Divine Orb", 1),
            json!({"id": "m1", "inventoryId": "MainInventory", "frameType": 0, "typeLine": "Map (Tier 16)", "w": 1, "h": 1}),
            json!({"id": "u1", "inventoryId": "MainInventory", "frameType": 3, "name": "", "typeLine": "Leather Belt", "identified": false, "w": 2, "h": 1}),
            json!({"id": "s1", "inventoryId": "MainInventory", "frameType": 0, "typeLine": "Divination Scarab of Curation", "stackSize": 2, "w": 1, "h": 1}),
            json!({"id": "k1", "inventoryId": "MainInventory", "frameType": 6, "typeLine": "The Nurse", "stackSize": 1, "maxStackSize": 8, "w": 1, "h": 1}),
            json!({"id": "x", "inventoryId": "Flask", "frameType": 1, "typeLine": "Quicksilver Flask"})];
        let got = gained(&bag_items(&Value::Array(before)), &bag_items(&Value::Array(after)));
        let names: Vec<(String, u64)> = got.iter().map(|i| (i["typeLine"].as_str().unwrap().to_string(), i["stackSize"].as_u64().unwrap_or(1))).collect();
        assert!(names.contains(&("Chaos Orb".into(), 13)));
        assert!(names.contains(&("Divine Orb".into(), 1)));
        assert!(!names.iter().any(|n| n.0 == "Portal Scroll" || n.0.starts_with("Map") || n.0.contains("Flask")));
        assert_eq!(got.len(), 5);
        let price = |name: Option<&str>, base: &str, _k: &str| -> Option<f64> {
            match (name, base) { (_, "Divine Orb") => Some(200.0), (_, "Leather Belt") => Some(1.0), (_, "The Nurse") => Some(38.0), (_, "Divination Scarab of Curation") => Some(4.5), _ => None }
        };
        let ls = lines(&got, &price, 200.0);
        assert_eq!(ls[0].name, "Divine Orb");
        assert_eq!(ls[0].cat, "currency");
        let k = |n: &str| ls.iter().find(|l| l.name.contains(n)).unwrap().clone();
        assert_eq!((k("Chaos").stack, k("Chaos").value), (13, 13.0));
        assert_eq!(k("Nurse").cat, "cards");
        assert_eq!((k("Scarab").cat.as_str(), k("Scarab").value), ("scarabs", 9.0));
        assert_eq!(k("Belt").name, "Unidentified Leather Belt");
        assert_eq!(k("Belt").tier, Some(3));
        assert_eq!(total(&ls), 200.0 + 13.0 + 38.0 + 9.0 + 1.0);
        let kinds = by_kind(&ls);
        assert_eq!(kinds[0], ("currency".to_string(), 213.0, 14));
        // A second visit adds to the first.
        let mut run = ls.clone();
        merge(&mut run, lines(&[cur("e", "Chaos Orb", 4)], &price, 200.0));
        assert_eq!(run.iter().find(|l| l.name == "Chaos Orb").unwrap().stack, 17);
    }

    /// An unidentified unique counts at the cheapest unique on its base, with the top as "high" (23 Nov: an
    /// unidentified Legion Sword showed as a 2 div tier 1 drop).
    #[test]
    fn unidentified_unique_counts_low() {
        let sword = json!({"id": "s", "inventoryId": "MainInventory", "frameType": 3, "identified": false, "name": "",
            "typeLine": "Legion Sword", "baseType": "Legion Sword", "w": 1, "h": 4, "icon": "x.png"});
        let price = |name: Option<&str>, _base: &str, k: &str| -> Option<f64> {
            match (k, name) { ("unique-max", _) => Some(400.0), ("unique", Some(n)) if !n.is_empty() => Some(400.0), ("unique", _) => Some(3.0), _ => None }
        };
        let ls = lines(&[sword], &price, 200.0);
        assert_eq!((ls[0].name.as_str(), ls[0].value, ls[0].high, ls[0].tier), ("Unidentified Legion Sword", 3.0, Some(400.0), Some(3)));
    }

    #[test]
    fn kinds_and_tiers() {
        let k = |v: Value| kind(&v);
        assert_eq!(k(json!({"frameType": 5, "typeLine": "Simulacrum Splinter", "stackSize": 3})), "fragments");
        assert_eq!(k(json!({"frameType": 0, "typeLine": "Shaper Guardian Map"})), "maps");
        assert_eq!(k(json!({"frameType": 3, "typeLine": "Shaper Guardian Map"})), "maps");
        assert_eq!(k(json!({"frameType": 5, "typeLine": "Dense Fossil", "stackSize": 1})), "currency");
        assert_eq!(k(json!({"frameType": 4, "typeLine": "Empower Support"})), "gems");
        assert_eq!(k(json!({"frameType": 2, "typeLine": "Dragon Turn Cogwork Ring", "w": 1, "h": 1})), "gear");
        assert_eq!(k(json!({"frameType": 2, "typeLine": "Vaal Regalia", "w": 2, "h": 3})), "gear");
        assert_eq!(unique_tier(250.0, 200.0), 1);
        assert_eq!(unique_tier(12.0, 200.0), 2);
        assert_eq!(unique_tier(1.0, 200.0), 3);
    }
}
