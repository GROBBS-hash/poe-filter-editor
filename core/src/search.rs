//! Finding items by everyday names ("2h mace", "ammy", "div card") and
//! one-click presets for builds and league mechanics.

use crate::item::base_classes;
use crate::rules::{style_actions, CustomStyle, Rule};
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Found {
    /// Empty for a whole item type (e.g. every Two Hand Maces).
    pub base_type: String,
    pub item_class: String,
}

/// Nicknames players use -> words in the game's names.
const ALIASES: [(&str, &str); 30] = [
    ("2h", "two hand"), ("2-h", "two hand"), ("two-handed", "two hand"), ("twohanded", "two hand"), ("two handed", "two hand"),
    ("1h", "one hand"), ("one-handed", "one hand"), ("one handed", "one hand"),
    ("ammy", "amulet"), ("ammies", "amulets"), ("ammys", "amulets"), ("chest", "body armour"), ("body", "body armour"),
    ("helm", "helmet"), ("div", "divination"), ("cards", "divination cards"), ("card", "divination card"),
    ("scepter", "sceptre"), ("scepters", "sceptres"), ("pots", "flasks"), ("pot", "flask"), ("jewl", "jewel"),
    ("currencies", "stackable currency"), ("cluster", "cluster jewel"), ("abyss", "abyss jewel"),
    ("staff", "staves"), ("staffs", "staves"), ("knife", "dagger"), ("hammer", "maces"), ("thrusting", "thrusting one hand"),
];

fn norm(s: &str) -> String {
    s.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '\'' { c } else { ' ' }).collect()
}

/// "maces" matches "mace", "staves" matches "staff"...
fn word_in(word: &str, hay: &str) -> bool {
    let w = word.trim_end_matches('s');
    hay.split(' ').any(|h| h.starts_with(w) || h.trim_end_matches('s') == w || (w.len() >= 4 && h.contains(w)))
}

pub fn expand(q: &str) -> String {
    let mut s = format!(" {} ", norm(q));
    for (from, to) in ALIASES.iter() {
        s = s.replace(&format!(" {from} "), &format!(" {to} "));
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Item types and base types matching a search, best first (types before bases).
pub fn search(q: &str, extra_bases: &BTreeSet<String>, limit: usize) -> Vec<Found> {
    let q = expand(q);
    let words: Vec<&str> = q.split(' ').filter(|w| !w.is_empty()).collect();
    if words.is_empty() {
        return vec![];
    }
    let classes: BTreeSet<&String> = base_classes().values().filter(|c| !c.is_empty()).collect();
    let mut out: Vec<(i32, Found)> = Vec::new();
    for c in classes {
        let h = norm(c);
        if words.iter().all(|w| word_in(w, &h)) {
            out.push((0 - (h.len() as i32 / 4), Found { base_type: String::new(), item_class: c.clone() }));
        }
    }
    let mut bases: BTreeSet<&String> = base_classes().keys().collect();
    bases.extend(extra_bases.iter());
    for b in bases {
        let class = base_classes().get(b).cloned().unwrap_or_default();
        let h = norm(b);
        let hc = format!("{h} {}", norm(&class));
        if words.iter().all(|w| word_in(w, &hc)) {
            let exact = if h == q { -100 } else if h.starts_with(&q) { -50 } else { 0 };
            out.push((1000 + exact + h.len() as i32, Found { base_type: b.clone(), item_class: class }));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.base_type.cmp(&b.1.base_type)));
    out.dedup_by(|a, b| a.1 == b.1);
    out.into_iter().take(limit).map(|(_, f)| f).collect()
}

// ---------- presets ----------

#[derive(Debug, Clone, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub title: &'static str,
    pub hint: &'static str,
    /// How many items it covers (for the button label).
    pub count: usize,
}

/// League mechanics you might farm, and the items that belong to each.
fn farm_items(id: &str) -> (Vec<String>, Vec<&'static str>) {
    let names: Vec<&String> = base_classes().keys().collect();
    let pick = |f: &dyn Fn(&str) -> bool| -> Vec<String> { names.iter().filter(|n| f(n)).map(|n| n.to_string()).collect() };
    match id {
        "essences" => (pick(&|n| n.contains("Essence of") || n == "Remnant of Corruption"), vec![]),
        "scarabs" => (pick(&|n| n.contains("Scarab")), vec![]),
        "fossils" => (pick(&|n| n.ends_with(" Fossil") || n.contains("Resonator")), vec![]),
        "oils" => (pick(&|n| n.ends_with(" Oil") && base_classes().get(n).map(|c| c == "Stackable Currency").unwrap_or(false)), vec![]),
        "delirium" => (pick(&|n| n.contains("Delirium Orb") || n.starts_with("Simulacrum")), vec![]),
        "breach" => (pick(&|n| n.starts_with("Splinter of ") || n.contains("Breachstone") || n.starts_with("Blessing of ")), vec![]),
        "legion" => (pick(&|n| n.starts_with("Timeless ")), vec![]),
        "expedition" => (pick(&|n| ["Astragali", "Burial Medallion", "Scrap Metal", "Exotic Coinage"].contains(&n)), vec!["Expedition Logbooks"]),
        "harvest" => (pick(&|n| n.contains("Lifeforce")), vec![]),
        "ritual" => (pick(&|n| n.starts_with("Omen of")), vec![]),
        "heist" => (vec![], vec!["Contracts", "Blueprints"]),
        "abyss" => (vec![], vec!["Abyss Jewels"]),
        _ => (vec![], vec![]),
    }
}

const FARMS: [(&str, &str, &str); 12] = [
    ("essences", "Essences", "Every essence"),
    ("scarabs", "Scarabs", "Every scarab"),
    ("fossils", "Delve", "Fossils and resonators"),
    ("oils", "Blight", "Every oil"),
    ("delirium", "Delirium", "Delirium orbs and simulacrum splinters"),
    ("breach", "Breach", "Splinters, breachstones and blessings"),
    ("legion", "Legion", "Timeless splinters and emblems"),
    ("expedition", "Expedition", "Expedition currency and logbooks"),
    ("harvest", "Harvest", "Lifeforce"),
    ("ritual", "Ritual", "Omens"),
    ("heist", "Heist", "Contracts and blueprints"),
    ("abyss", "Abyss", "Abyss jewels"),
];

pub fn farm_presets() -> Vec<Preset> {
    FARMS.iter().map(|(id, title, hint)| {
        let (bases, classes) = farm_items(id);
        Preset { id, title, hint, count: bases.len() + classes.len() }
    }).filter(|p| p.count > 0).collect()
}

fn quote(names: &[String]) -> String {
    names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ")
}

/// Rules that make everything for a mechanic stand out. Labels start with
/// "Preset:" so they're listed (and removable) together in My edits.
pub fn farm_rules(id: &str, style: &CustomStyle) -> Vec<Rule> {
    let Some((_, title, _)) = FARMS.iter().find(|f| f.0 == id) else { return vec![] };
    let (bases, classes) = farm_items(id);
    let mut out = Vec::new();
    if !bases.is_empty() {
        out.push(Rule::Override { label: format!("Preset: {title} items stand out"), show: true,
            conditions: vec![format!("BaseType == {}", quote(&bases))], actions: style_actions(style) });
    }
    for c in classes {
        out.push(Rule::Override { label: format!("Preset: {title} ({c}) stand out"), show: true,
            conditions: vec![format!("Class == \"{c}\"")], actions: style_actions(style) });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nicknames_find_types() {
        let none = BTreeSet::new();
        let r = search("2h mace", &none, 5);
        assert_eq!(r[0], Found { base_type: String::new(), item_class: "Two Hand Maces".into() });
        assert_eq!(search("two-handed maces", &none, 1)[0].item_class, "Two Hand Maces");
        assert_eq!(search("ammy", &none, 1)[0].item_class, "Amulets");
        assert_eq!(search("div card", &none, 1)[0].item_class, "Divination Cards");
        let chaos = search("chaos orb", &none, 3);
        assert!(chaos.iter().any(|f| f.base_type == "Chaos Orb"), "{chaos:?}");
        assert!(search("zzzz", &none, 3).is_empty());
    }

    #[test]
    fn farm_presets_cover_real_items() {
        let p = farm_presets();
        let n = |id: &str| p.iter().find(|x| x.id == id).map(|x| x.count).unwrap_or(0);
        println!("{:?}", p.iter().map(|x| (x.id, x.count)).collect::<Vec<_>>());
        assert!(n("essences") > 20 && n("scarabs") > 20 && n("fossils") > 10);
        let style = CustomStyle { font_size: 42, text: [255, 0, 0, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: crate::rules::Sound::Default, label_hidden: false };
        let r = farm_rules("essences", &style);
        let Rule::Override { conditions, label, .. } = &r[0] else { panic!() };
        assert!(conditions[0].contains("Essence of Greed\""));
        assert_eq!(label, "Preset: Essences items stand out");
    }
}
