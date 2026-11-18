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
            // An exact name beats everything, even an item type of the same name (Gold).
            let exact = if h == q || h.trim_end_matches('s') == q.trim_end_matches('s') { -3000 } else if h.starts_with(&q) { -50 } else { 0 };
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

const FARMS: [(&str, &str, &str); 11] = [
    ("essences", "Essences", "Every essence"),
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

// ---------- levelling presets (like FilterBlade's build types) ----------

/// Every weapon-ish item type, per game.
fn weapon_classes(game: crate::profiles::Game) -> &'static [&'static str] {
    match game {
        crate::profiles::Game::Poe1 => &["Bows", "Claws", "Daggers", "One Hand Axes", "One Hand Maces", "One Hand Swords", "Quivers",
            "Rune Daggers", "Sceptres", "Shields", "Staves", "Thrusting One Hand Swords", "Two Hand Axes", "Two Hand Maces",
            "Two Hand Swords", "Wands", "Warstaves"],
        crate::profiles::Game::Poe2 => &["Bows", "Bucklers", "Crossbows", "Foci", "One Hand Maces", "Quarterstaves", "Quivers",
            "Sceptres", "Shields", "Spears", "Staves", "Two Hand Maces", "Wands"],
    }
}

/// (id, title, what it keeps) — the kept item types, per game.
pub const LEVEL_BUILDS: [(&str, &str, &str); 6] = [
    ("caster", "Caster", "Wands, staves, sceptres, rune daggers, shields / foci"),
    ("minion", "Minions", "Wands, sceptres, shields / foci"),
    ("melee2h", "Two-handed melee", "Two hand axes, maces, swords and staves"),
    ("melee1h", "One-handed melee", "One hand weapons, claws, daggers and shields"),
    ("ranged", "Bow / ranged", "Bows and quivers (crossbows and spears in PoE 2)"),
    ("spear", "Spear (PoE 2)", "Spears and bucklers"),
];

fn level_keep(id: &str, game: crate::profiles::Game) -> Vec<&'static str> {
    use crate::profiles::Game::*;
    match (id, game) {
        ("caster", Poe1) => vec!["Wands", "Staves", "Sceptres", "Rune Daggers", "Shields"],
        ("caster", Poe2) => vec!["Wands", "Staves", "Sceptres", "Foci"],
        ("minion", Poe1) => vec!["Wands", "Sceptres", "Shields"],
        ("minion", Poe2) => vec!["Wands", "Sceptres", "Foci"],
        ("melee2h", Poe1) => vec!["Two Hand Axes", "Two Hand Maces", "Two Hand Swords", "Staves", "Warstaves"],
        ("melee2h", Poe2) => vec!["Two Hand Maces", "Quarterstaves"],
        ("melee1h", Poe1) => vec!["Claws", "Daggers", "One Hand Axes", "One Hand Maces", "One Hand Swords", "Thrusting One Hand Swords", "Shields"],
        ("melee1h", Poe2) => vec!["One Hand Maces", "Shields"],
        ("ranged", Poe1) => vec!["Bows", "Quivers"],
        ("ranged", Poe2) => vec!["Bows", "Quivers", "Crossbows"],
        ("spear", Poe2) => vec!["Spears", "Bucklers"],
        _ => vec![],
    }
}

pub fn level_presets(game: crate::profiles::Game) -> Vec<Preset> {
    LEVEL_BUILDS.iter().filter(|(id, ..)| !level_keep(id, game).is_empty())
        .map(|(id, title, hint)| Preset { id, title, hint, count: level_keep(id, game).len() }).collect()
}

pub const LEVEL_PREFIX: &str = "Preset: Levelling";

/// While levelling (area level under 68): your weapon types stand out, every
/// other weapon type is hidden. Uniques are never touched; in PoE 1, linked
/// (4+ link) items still show, since they're worth wearing for anyone.
pub fn level_rules(id: &str, game: crate::profiles::Game, style: &CustomStyle) -> Vec<Rule> {
    let Some((_, title, _)) = LEVEL_BUILDS.iter().find(|b| b.0 == id) else { return vec![] };
    let keep = level_keep(id, game);
    if keep.is_empty() { return vec![]; }
    let hide: Vec<String> = weapon_classes(game).iter().filter(|c| !keep.contains(c)).map(|c| c.to_string()).collect();
    let keep: Vec<String> = keep.iter().map(|c| c.to_string()).collect();
    let mut out = vec![Rule::Override {
        label: format!("{LEVEL_PREFIX} ({title}): your weapons stand out"), show: true,
        conditions: vec!["AreaLevel < 68".into(), "Rarity <= Rare".into(), format!("Class == {}", quote(&keep))],
        actions: style_actions(style),
    }];
    let mut cond = vec!["AreaLevel < 68".into(), "Rarity <= Rare".into(), format!("Class == {}", quote(&hide))];
    if game == crate::profiles::Game::Poe1 {
        cond.push("LinkedSockets < 4".into());
    }
    out.push(Rule::Override { label: format!("{LEVEL_PREFIX} ({title}): other weapons hidden"), show: false, conditions: cond, actions: vec![] });
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn levelling_presets() {
        use crate::profiles::Game;
        let style = CustomStyle { font_size: 40, text: [0, 255, 255, 255], border: None, background: [0, 0, 0, 220],
            beam: None, minimap: None, sound: crate::rules::Sound::Default, label_hidden: false };
        let r = level_rules("caster", Game::Poe1, &style);
        let Rule::Override { conditions, show, .. } = &r[1] else { panic!() };
        assert!(!show);
        assert!(conditions[2].contains("\"Two Hand Maces\"") && !conditions[2].contains("\"Wands\""), "{conditions:?}");
        assert!(level_presets(Game::Poe1).iter().all(|p| p.id != "spear"));
        assert!(level_presets(Game::Poe2).iter().any(|p| p.id == "spear"));
        // Every PoE 1 type named is a real item type.
        let real: BTreeSet<&String> = base_classes().values().collect();
        for c in weapon_classes(Game::Poe1) { assert!(real.contains(&c.to_string()), "{c}"); }
    }

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
        assert!(n("essences") > 20 && n("fossils") > 10);
        assert_eq!(n("scarabs"), 0, "scarabs were removed from the game");
        let style = CustomStyle { font_size: 42, text: [255, 0, 0, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: crate::rules::Sound::Default, label_hidden: false };
        let r = farm_rules("essences", &style);
        let Rule::Override { conditions, label, .. } = &r[0] else { panic!() };
        assert!(conditions[0].contains("Essence of Greed\""));
        assert_eq!(label, "Preset: Essences items stand out");
    }
}
