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

// ---------- levelling (like FilterBlade's "Auto-Adjust: Campaign") ----------

use crate::profiles::Game;

/// Every weapon-ish item type, per game.
pub fn weapon_classes(game: Game) -> &'static [&'static str] {
    match game {
        Game::Poe1 => &["Warstaves", "Two Hand Maces", "Two Hand Axes", "One Hand Maces", "One Hand Axes", "Two Hand Swords",
            "One Hand Swords", "Thrusting One Hand Swords", "Bows", "Quivers", "Claws", "Daggers", "Rune Daggers", "Wands",
            "Sceptres", "Staves", "Shields"],
        Game::Poe2 => &["Two Hand Maces", "One Hand Maces", "Quarterstaves", "Spears", "Bows", "Crossbows", "Quivers",
            "Wands", "Sceptres", "Staves", "Foci", "Shields", "Bucklers"],
    }
}

/// Build buttons: (id, title). "custom" keeps whatever you tick.
pub const LEVEL_BUILDS: [(&str, &str); 8] = [
    ("custom", "Custom"), ("claw", "Claw & Dagger"), ("bow", "Bow Ranger"), ("sword", "Sword & Shield"),
    ("twohand", "2H Axe/Mace"), ("templar", "Fire Templar"), ("spells", "Spells & Minions"), ("spear", "Spear (PoE 2)"),
];

/// Defence types, as on FilterBlade.
pub const ARMOUR_TYPES: [(&str, &str); 6] = [
    ("ar", "Armour"), ("ar_ev", "AR/EV"), ("ev", "Evasion"), ("ev_es", "EV/ES"), ("es", "Energy Shield"), ("ar_es", "AR/ES"),
];

/// Vendor-rares level ranges, as on FilterBlade: [from, below).
pub const VENDOR_RANGES: [(u32, u32); 4] = [(1, 16), (16, 24), (24, 42), (42, 68)];

/// What a build button ticks: (weapon types, armour types).
pub fn build_defaults(id: &str, game: Game) -> (Vec<&'static str>, Vec<&'static str>) {
    match (id, game) {
        ("claw", Game::Poe1) => (vec!["Claws", "Daggers", "Rune Daggers"], vec!["ev", "ev_es"]),
        ("bow", Game::Poe1) => (vec!["Bows", "Quivers"], vec!["ev", "ev_es", "ar_ev"]),
        ("bow", Game::Poe2) => (vec!["Bows", "Quivers", "Crossbows"], vec!["ev", "ev_es", "ar_ev"]),
        ("sword", Game::Poe1) => (vec!["One Hand Swords", "Thrusting One Hand Swords", "One Hand Axes", "One Hand Maces", "Shields"], vec!["ar", "ar_ev"]),
        ("sword", Game::Poe2) => (vec!["One Hand Maces", "Shields"], vec!["ar", "ar_ev"]),
        ("twohand", Game::Poe1) => (vec!["Two Hand Axes", "Two Hand Maces", "Two Hand Swords", "Warstaves"], vec!["ar", "ar_ev"]),
        ("twohand", Game::Poe2) => (vec!["Two Hand Maces", "Quarterstaves"], vec!["ar", "ar_ev"]),
        ("templar", Game::Poe1) => (vec!["Sceptres", "Staves", "Shields"], vec!["ar_es", "es"]),
        ("templar", Game::Poe2) => (vec!["Sceptres", "Staves", "Foci"], vec!["ar_es", "es"]),
        ("spells", Game::Poe1) => (vec!["Wands", "Sceptres", "Rune Daggers", "Staves", "Shields"], vec!["es", "ev_es", "ar_es"]),
        ("spells", Game::Poe2) => (vec!["Wands", "Sceptres", "Staves", "Foci"], vec!["es", "ev_es", "ar_es"]),
        ("spear", Game::Poe2) => (vec!["Spears", "Bucklers"], vec!["ev", "ar_ev", "ev_es"]),
        _ => (vec![], vec![]),
    }
}

pub fn level_builds(game: Game) -> Vec<(&'static str, &'static str)> {
    LEVEL_BUILDS.iter().copied().filter(|(id, _)| *id == "custom" || !build_defaults(id, game).0.is_empty()).collect()
}

/// Your levelling choices (kept with the profile).
#[derive(Debug, Clone, serde::Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct LevelOptions {
    pub build: String,
    /// Weapon types you want: they stand out. Others are hidden.
    pub keep: Vec<String>,
    /// Armour defence types you want. Others are hidden. Empty = all.
    pub armour_types: Vec<String>,
    /// Which VENDOR_RANGES still show rares of unpicked gear (to sell).
    pub vendor_rares: Vec<usize>,
    /// Also hide linked (3/4-link) unpicked gear.
    pub aggressive: bool,
    /// Minion bases (Convoking Wand, Bone Helmet...) always stand out.
    pub minion: bool,
    /// From this area level on, only rares of your weapons (0 = whole campaign).
    pub rares_from: u32,
    /// From that level on, hide white and blue armour too.
    pub hide_armour: bool,
    /// Item progression (up to 3 types): the newest base of each is highlighted as you level.
    pub progression: Vec<String>,
}

/// Base drop levels per item type (PoE 1).
pub fn drop_levels() -> &'static std::collections::HashMap<String, Vec<(String, u32)>> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<String, Vec<(String, u32)>>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let raw: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> =
            serde_json::from_str(include_str!("../data/poe1_droplevels.json")).unwrap_or_default();
        raw.into_iter().map(|(c, m)| {
            let mut v: Vec<(String, u32)> = m.into_iter().map(|(b, l)| (b, l.as_u64().unwrap_or(1) as u32)).collect();
            v.sort_by_key(|x| x.1);
            (c, v)
        }).collect()
    })
}

/// (bases, from area level, below area level) for each step of a type's progression.
pub fn progression_steps(class: &str) -> Vec<(Vec<String>, u32, u32)> {
    let Some(list) = drop_levels().get(class) else { return vec![] };
    let mut levels: Vec<u32> = list.iter().map(|x| x.1).filter(|l| *l < 68).collect();
    levels.dedup();
    levels.iter().enumerate().map(|(i, &l)| {
        // Still worth a look a few levels after the next base appears.
        let until = levels.get(i + 1).map(|n| (n + 4).min(68)).unwrap_or(68);
        (list.iter().filter(|x| x.1 == l).map(|x| x.0.clone()).collect(), l, until)
    }).collect()
}

impl Default for LevelOptions {
    fn default() -> Self {
        Self { build: "custom".into(), keep: vec![], armour_types: vec![], vendor_rares: vec![0, 1], aggressive: false,
            minion: false, rares_from: 16, hide_armour: false, progression: vec![] }
    }
}

pub const LEVEL_PREFIX: &str = "Preset: Levelling";
const ARMOUR: [&str; 4] = ["Body Armours", "Boots", "Gloves", "Helmets"];
const MINION_BASES: [&str; 6] = ["Bone Helmet", "Bone Spirit Shield", "Calling Wand", "Convoking Wand", "Necromancer Circlet", "Necromancer Silks"];

/// The rules for your levelling choices. Later rules win (they go higher in the filter).
/// Everything is limited to area level < 68, and uniques are never touched.
pub fn level_rules_opts(game: Game, o: &LevelOptions, style: &CustomStyle) -> Vec<Rule> {
    let title = LEVEL_BUILDS.iter().find(|b| b.0 == o.build).map(|b| b.1).unwrap_or("Custom");
    let poe1 = game == Game::Poe1;
    let all = weapon_classes(game);
    let keep: Vec<String> = all.iter().filter(|c| o.keep.iter().any(|k| k == *c)).map(|c| c.to_string()).collect();
    let other: Vec<String> = all.iter().filter(|c| !keep.contains(&c.to_string())).map(|c| c.to_string()).collect();
    let lbl = |what: &str| format!("{LEVEL_PREFIX} ({title}): {what}");
    let mut out = vec![];

    // Hides of unpicked gear: white/blue always, rares outside the vendor ranges you picked.
    // Linked items are spared like NeverSink (3-links until 28, 4-links always) unless aggressive.
    let hide = |out: &mut Vec<Rule>, label: String, base: Vec<String>| {
        let mut variants: Vec<Vec<String>> = vec![vec!["AreaLevel < 68".into(), "Rarity <= Magic".into()]];
        for (i, (from, below)) in VENDOR_RANGES.iter().enumerate() {
            if !o.vendor_rares.contains(&i) {
                variants.push(vec![format!("AreaLevel >= {from}"), format!("AreaLevel < {below}"), "Rarity Rare".into()]);
            }
        }
        for v in variants {
            let mut c = v;
            c.extend(base.iter().cloned());
            if !poe1 || o.aggressive {
                out.push(Rule::Override { label: label.clone(), show: false, conditions: c, actions: vec![] });
            } else {
                let mut early = c.clone();
                early.extend(["AreaLevel <= 28".to_string(), "LinkedSockets < 3".to_string()]);
                c.extend(["AreaLevel > 28".to_string(), "LinkedSockets < 4".to_string()]);
                out.push(Rule::Override { label: label.clone(), show: false, conditions: early, actions: vec![] });
                out.push(Rule::Override { label: label.clone(), show: false, conditions: c, actions: vec![] });
            }
        }
    };

    if !keep.is_empty() {
        out.push(Rule::Override { label: lbl("your weapons stand out"), show: true,
            conditions: vec!["AreaLevel < 68".into(), "Rarity <= Rare".into(), format!("Class == {}", quote(&keep))],
            actions: style_actions(style) });
        if !other.is_empty() {
            hide(&mut out, lbl("other weapons hidden"), vec![format!("Class == {}", quote(&other))]);
        }
    }
    if !o.armour_types.is_empty() {
        let mut classes: Vec<String> = ARMOUR.iter().map(|c| c.to_string()).collect();
        classes.push("Shields".into());
        for (t, _) in ARMOUR_TYPES.iter().filter(|(t, _)| !o.armour_types.iter().any(|k| k == t)) {
            let mut c = vec![format!("Class == {}", quote(&classes))];
            for (part, key) in [("ar", "BaseArmour"), ("ev", "BaseEvasion"), ("es", "BaseEnergyShield")] {
                c.push(format!("{key} {}", if t.split('_').any(|x| x == part) { "> 0" } else { "== 0" }));
            }
            hide(&mut out, lbl(&format!("{} armour hidden", ARMOUR_TYPES.iter().find(|x| x.0 == *t).unwrap().1)), c);
        }
    }
    let n = o.rares_from;
    if n > 1 && n < 68 && !keep.is_empty() {
        let mut c = vec![format!("AreaLevel >= {n}"), "AreaLevel < 68".into(), "Rarity <= Magic".into(), format!("Class == {}", quote(&keep))];
        if poe1 && !o.aggressive { c.push("LinkedSockets < 3".into()); }
        out.push(Rule::Override { label: lbl(&format!("white & blue weapons hidden from area {n}")), show: false, conditions: c, actions: vec![] });
        if o.hide_armour {
            let armour: Vec<String> = ARMOUR.iter().map(|c| c.to_string()).collect();
            let mut c = vec![format!("AreaLevel >= {n}"), "AreaLevel < 68".into(), "Rarity <= Magic".into(), format!("Class == {}", quote(&armour))];
            if poe1 && !o.aggressive { c.push("LinkedSockets < 3".into()); }
            out.push(Rule::Override { label: lbl(&format!("white & blue armour hidden from area {n}")), show: false, conditions: c, actions: vec![] });
        }
    }
    // Item progression: white/blue copies of the newest base, on top of the hides above.
    if poe1 {
        let subtle = CustomStyle { beam: None, minimap: None, font_size: style.font_size.min(40), ..style.clone() };
        for class in o.progression.iter().take(3) {
            for (bases, from, below) in progression_steps(class) {
                out.push(Rule::Override { label: lbl(&format!("{class} progression")), show: true,
                    conditions: vec![format!("AreaLevel >= {from}"), format!("AreaLevel < {below}"), "Rarity <= Magic".into(),
                        format!("Class == \"{class}\""), format!("BaseType == {}", quote(&bases))],
                    actions: style_actions(&subtle) });
            }
        }
    }
    if o.minion && poe1 {
        let bases: Vec<String> = MINION_BASES.iter().map(|b| b.to_string()).collect();
        out.push(Rule::Override { label: lbl("minion bases stand out"), show: true,
            conditions: vec!["AreaLevel < 68".into(), "Rarity <= Rare".into(), format!("BaseType == {}", quote(&bases))],
            actions: style_actions(style) });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn levelling_presets() {
        let style = CustomStyle { font_size: 40, text: [0, 255, 255, 255], border: None, background: [0, 0, 0, 220],
            beam: None, minimap: None, sound: crate::rules::Sound::Default, label_hidden: false };
        let (w, a) = build_defaults("spells", Game::Poe1);
        let o = LevelOptions { build: "spells".into(), keep: w.iter().map(|s| s.to_string()).collect(),
            armour_types: a.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        let r = level_rules_opts(Game::Poe1, &o, &style);
        let Rule::Override { conditions, show, label, .. } = &r[1] else { panic!() };
        assert!(!show && label.contains("Spells & Minions"));
        assert!(conditions.iter().any(|c| c.contains("\"Two Hand Maces\"")) && !conditions.iter().any(|c| c.contains("\"Wands\"")));
        assert!(level_builds(Game::Poe1).iter().all(|p| p.0 != "spear"));
        assert!(level_builds(Game::Poe2).iter().any(|p| p.0 == "spear"));
        let real: BTreeSet<&String> = base_classes().values().collect();
        for c in weapon_classes(Game::Poe1) { assert!(real.contains(&c.to_string()), "{c}"); }
        for b in MINION_BASES { assert!(base_classes().contains_key(b), "{b}"); }
        assert_eq!(crate::item::defences().get("Vine Circlet").map(String::as_str), Some("es"));
        let w = progression_steps("Wands");
        assert_eq!(w[0], (vec!["Driftwood Wand".to_string()], 1, 10));
        assert!(w.iter().all(|s| s.1 < s.2 && s.2 <= 68));
    }

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
