//! Parses the text Path of Exile puts on the clipboard when you Ctrl+C an item.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Item {
    pub item_class: String,
    pub rarity: String,
    pub name: Option<String>,
    pub base_type: String,
    pub item_level: Option<u32>,
    pub stack_size: Option<u32>,
    pub quality: Option<u32>,
    pub sockets: Option<String>,
    pub map_tier: Option<u32>,
    pub corrupted: bool,
    pub mirrored: bool,
    pub unidentified: bool,
    #[serde(default)]
    pub memory_strands: Option<u32>,
    /// 3.30 item tier (1-5), from the ground label or the copied text.
    #[serde(default)]
    pub item_tier: Option<u32>,
    /// Maps: blighted / blight-ravaged (`BlightedMap` / `UberBlightedMap`), Originator's memory (`ZanaMemory`).
    #[serde(default)]
    pub blighted: bool,
    #[serde(default)]
    pub uber_blighted: bool,
    #[serde(default)]
    pub zana_memory: bool,
    /// We know the map's special kind (from its name on the label or the copied text), even when read off the screen.
    #[serde(default)]
    pub map_kind_known: bool,
    /// "Shaper", "Elder", "Crusader"… from the copied text's "Shaper Item" lines.
    #[serde(default)]
    pub influences: Vec<String>,
    /// Corrupted a second time (3.30: "Twice Corrupted"); `corrupted` is true as well.
    #[serde(default)]
    pub twice_corrupted: bool,
    /// What the copied text says it is, by the filter's names: "FracturedItem", "SynthesisedItem", "Replica", "Foulborn",
    /// "TransfiguredGem", "Vestigial", "Fated", "ElderMap", "ShapedMap".
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Has an implicit mod (maps: "Map contains Drox's Citadel"), NeverSink's `HasImplicitMod`.
    #[serde(default)]
    pub has_implicit: bool,
    /// Maps: the conqueror ("Drox") or Elder guardian ("The Enslaver") the map belongs to, for its picture.
    #[serde(default)]
    pub occupied_by: Option<String>,
    /// Its explicit mods by the game's names (jewels, from the copied text), for the filter's `HasExplicitMod`.
    /// None = not known (e.g. read off the ground).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_names: Option<Vec<String>>,
}

/// "Map contains Drox's Citadel" → "Drox"; "Map is occupied by The Enslaver" → "The Enslaver";
/// "Area is influenced by Veritania, the Redeemer" → "Veritania". (implicit) tags are dropped.
pub fn map_occupant(line: &str) -> Option<String> {
    let l = line.trim().trim_end_matches("(implicit)").trim();
    let who = if let Some(r) = l.strip_prefix("Map contains ") {
        r.strip_suffix("'s Citadel")?.to_string()
    } else if let Some(r) = l.strip_prefix("Map is occupied by ") {
        r.to_string()
    } else if let Some(r) = l.strip_prefix("Area is influenced by ") {
        r.split(',').next()?.to_string()
    } else { return None };
    let who = who.trim().to_string();
    (!who.is_empty() && !who.contains("Originator")).then_some(who)
}

/// Map names carry their special kind: "Blighted Map (Tier 3)", "Blight-ravaged Map (Tier 16)".
/// Takes the prefix off, sets the flags and returns the plain base ("Map (Tier 3)").
pub fn map_kind(item: &mut Item, name: &str) -> String {
    let lower = name.to_lowercase();
    let mut base = name.trim().to_string();
    for (p, uber) in [("blight-ravaged ", true), ("blight ravaged ", true), ("blighted ", false)] {
        if let Some(i) = lower.find(p) {
            base = name[i + p.len()..].trim().to_string();
            item.blighted = true;
            item.uber_blighted |= uber;
            break;
        }
    }
    item.map_kind_known = true;
    // A magic map's affixes round the base: "Burning Map (Tier 3) of Frenzy" -> "Map (Tier 3)".
    if let Some(i) = base.to_lowercase().find("map (tier") {
        if let Some(j) = base[i..].find(')') { base = base[i..i + j + 1].to_string(); }
    }
    base
}

const SEP: &str = "--------";

pub fn parse_item(text: &str) -> Option<Item> {
    let text = text.replace('\r', "");
    let sections: Vec<Vec<&str>> = text
        .split(SEP)
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect())
        .collect();

    let header = sections.first()?;
    let mut item = Item::default();
    let mut names: Vec<&str> = Vec::new();
    for line in header {
        if let Some(v) = line.strip_prefix("Item Class: ") {
            item.item_class = v.to_string();
        } else if let Some(v) = line.strip_prefix("Rarity: ") {
            item.rarity = v.to_string();
        } else {
            names.push(line);
        }
    }
    // Wickerman body parts (3.30, "Item Class: Viscera") are copied without a Rarity line; the site calls them Normal.
    if item.rarity.is_empty() && !item.item_class.is_empty() { item.rarity = "Normal".into(); }
    if item.rarity.is_empty() || names.is_empty() {
        return None;
    }

    match (item.rarity.as_str(), names.len()) {
        ("Rare" | "Unique", n) if n >= 2 => {
            item.name = Some(names[0].to_string());
            item.base_type = names[1].to_string();
        }
        _ => item.base_type = names.last()?.to_string(),
    }
    if item.item_class == "Maps" {
        let b = item.base_type.clone();
        item.base_type = map_kind(&mut item, &b);
        // Unique and rare maps carry the kind in their base line too.
        if let Some(n) = item.name.clone() { if n.to_lowercase().contains("blight") { let _ = map_kind(&mut item, &n); } }
    }

    for line in sections.iter().skip(1).flatten() {
        let num = |s: &str| -> Option<u32> {
            s.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
        };
        if let Some(v) = line.strip_prefix("Item Level: ") {
            item.item_level = num(v);
        } else if let Some(v) = line.strip_prefix("Stack Size: ") {
            item.stack_size = v.split('/').next().and_then(num);
        } else if let Some(v) = line.strip_prefix("Quality: ") {
            item.quality = v.split_whitespace().next().and_then(num);
        } else if let Some(v) = line.strip_prefix("Sockets: ") {
            item.sockets = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Item Tier: ").or_else(|| line.strip_prefix("Tier: ")) {
            item.item_tier = num(v);
        } else if let Some(v) = line.strip_prefix("Memory Strands: ") {
            item.memory_strands = num(v);
        } else if let Some(v) = line.strip_prefix("Map Tier: ").or_else(|| line.strip_prefix("Waystone Tier: ")) {
            item.map_tier = num(v);
        } else if let Some(inf) = line.strip_suffix(" Item").filter(|i| ["Shaper", "Elder", "Crusader", "Hunter", "Redeemer", "Warlord"].contains(i)) {
            item.influences.push(inf.to_string());
        } else if line.contains("Originator's Memories") || *line == "Originator Item" || line.starts_with("Contains memories of") {
            item.zana_memory = true;
        } else if line.starts_with("{ Implicit Modifier") || line.ends_with("(implicit)") {
            item.has_implicit = true;
            if item.occupied_by.is_none() { item.occupied_by = map_occupant(line); }
        } else if let Some(who) = map_occupant(line).filter(|_| item.occupied_by.is_none()) {
            item.occupied_by = Some(who);
        } else if *line == "Corrupted" || *line == "Twice Corrupted" {
            item.corrupted = true;
            item.twice_corrupted |= *line == "Twice Corrupted";
        } else if let Some(k) = line.strip_suffix(" Item").filter(|k| ["Fractured", "Synthesised"].contains(k)) {
            item.kinds.push(format!("{k}Item"));
        } else if *line == "Mirrored" {
            item.mirrored = true;
        } else if *line == "Unidentified" || line.starts_with("Unidentified (") {
            // "Unidentified (Tier 4)" on newer unidentified items.
            item.unidentified = true;
            if line.len() > "Unidentified".len() { item.item_tier = num(line).or(item.item_tier); }
        }
    }
    item.kinds.extend(name_kinds(&item));
    Some(item)
}

/// Kinds told by the name: "Replica Headhunter", "Foulborn Mageblood", a Synthesised base, a transfigured gem.
fn name_kinds(item: &Item) -> Vec<String> {
    let mut k = Vec::new();
    let name = item.name.as_deref().unwrap_or(&item.base_type);
    if item.rarity == "Unique" {
        for (w, kind) in [("Replica ", "Replica"), ("Foulborn ", "Foulborn")] { if name.starts_with(w) { k.push(kind.to_string()); } }
    }
    if item.base_type.starts_with("Synthesised ") && !item.kinds.iter().any(|x| x == "SynthesisedItem") { k.push("SynthesisedItem".into()); }
    for (w, kind) in [("Vestigial ", "Vestigial"), ("Fated ", "Fated")] { if item.base_type.starts_with(w) { k.push(kind.to_string()); } }
    if item.item_class.contains("Gem") && crate::gear::is_transfigured(&item.base_type) { k.push("TransfiguredGem".into()); }
    k
}

/// Magic items show "Prefix Base of Suffix" as one line. Given every base
/// name we know (taken straight from the filter), keep the longest match.
pub fn resolve_magic_base(item: &mut Item, known_bases: &HashSet<String>) {
    if item.rarity != "Magic" || known_bases.contains(&item.base_type) {
        return;
    }
    if let Some(best) = known_bases
        .iter()
        .filter(|b| item.base_type.contains(b.as_str()))
        .max_by_key(|b| b.len())
    {
        item.base_type = best.clone();
    }
}

/// Every PoE 1 base type with its item class, from the game's own data.
pub fn base_classes() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_bases.json")).unwrap_or_default())
}

/// Armour bases' defence type: "ar", "ev", "es", "ar_ev", "ar_es", "ev_es", "ward"...
pub fn defences() -> &'static HashMap<String, String> {
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_defences.json")).unwrap_or_default())
}

/// Fills in the item class when we only know the base (items read off the ground).
pub fn fill_class(item: &mut Item) {
    if item.item_class.trim().is_empty() {
        if let Some(c) = base_classes().get(&item.base_type) {
            item.item_class = c.clone();
        }
    }
}

/// Gear can roll mods; currency, gems, cards etc. can't.
pub fn is_gear(item: &Item) -> bool {
    if item.item_class.trim().is_empty() {
        return matches!(item.rarity.as_str(), "Normal" | "Magic" | "Rare" | "Unique");
    }
    GEAR_CLASSES.contains(&item.item_class.as_str())
}

/// Item classes that come in Normal/Magic/Rare/Unique and can roll mods.
pub const GEAR_CLASSES: [&str; 45] = [
    "Abyss Jewels", "Amulets", "Belts", "Body Armours", "Boots", "Bows", "Claws", "Daggers", "Gloves",
    "Heist Brooches", "Heist Cloaks", "Heist Gear", "Heist Tools", "Helmets", "Hybrid Flasks", "Idols",
    "Jewels", "Life Flasks", "Mana Flasks", "One Hand Axes", "One Hand Maces", "One Hand Swords", "Quivers",
    "Rings", "Rune Daggers", "Sceptres", "Shields", "Staves", "Thrusting One Hand Swords", "Tinctures",
    "Two Hand Axes", "Two Hand Maces", "Two Hand Swords", "Utility Flasks", "Wands", "Warstaves", "Maps",
    "Contracts", "Blueprints", "Expedition Logbooks", "Trinkets", "Fishing Rods", "Relics", "Sanctified Relics", "Charms",
];

/// Things that drop in stacks (where stack size can matter to the filter).
pub fn is_stackable(item: &Item) -> bool {
    item.stack_size.is_some()
        || matches!(item.item_class.as_str(), "Stackable Currency" | "Gold" | "Divination Cards" | "Map Fragments"
            | "Incubators" | "Delve Stackable Socketable Currency" | "Delve Socketable Currency" | "Corpses" | "Cursed Ducat")
}

/// How many links the biggest group has, e.g. "R-G-B G" -> 3.
pub fn max_links(sockets: &str) -> u32 {
    sockets
        .split_whitespace()
        .map(|g| g.split('-').count() as u32)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3.30's lines (25 Nov): "Twice Corrupted" was read as not corrupted, "Unidentified (Tier 4)" lost its tier.
    #[test]
    fn new_item_lines() {
        let i = parse_item("Item Class: Belts\nRarity: Unique\nMageblood\nHeavy Belt\n--------\nItem Level: 85\n--------\nTwice Corrupted\n").unwrap();
        assert!(i.corrupted && i.twice_corrupted);
        let i = parse_item("Item Class: Rings\nRarity: Rare\nComposite Ring\n--------\nItem Level: 85\n--------\nUnidentified (Tier 4)\n").unwrap();
        assert!(i.unidentified && i.item_tier == Some(4));
        assert_eq!(parse_item("Item Class: Rings\nRarity: Rare\nComposite Ring\n--------\nItem Level: 85\n--------\nUnidentified\n").unwrap().item_tier, None);
        let kinds = |t: &str| parse_item(t).unwrap().kinds;
        assert_eq!(kinds("Item Class: Boots\nRarity: Rare\nDoom Pace\nTwo-Toned Boots\n--------\nItem Level: 85\n--------\n+60 to maximum Life (fractured)\n--------\nFractured Item\n"), ["FracturedItem"]);
        assert_eq!(kinds("Item Class: Belts\nRarity: Unique\nReplica Headhunter\nLeather Belt\n--------\nItem Level: 85\n"), ["Replica"]);
        assert_eq!(kinds("Item Class: Skill Gems\nRarity: Gem\nFireball of Radiation\n--------\nSpell\n"), ["TransfiguredGem"]);
        assert!(kinds("Item Class: Skill Gems\nRarity: Gem\nHerald of Ash\n--------\nSpell\n").is_empty());
        assert_eq!(parse_item("Item Class: Waystones\nRarity: Rare\nGrim Path\nWaystone (Tier 15)\n--------\nWaystone Tier: 15\n--------\nItem Level: 80\n").unwrap().map_tier, Some(15));
    }

    #[test]
    fn unique() {
        let t = "Item Class: Body Armours\r\nRarity: Unique\r\nKaom's Heart\r\nGlorious Plate\r\n--------\r\nQuality: +20% (augmented)\r\n--------\r\nSockets: R-R-R G\r\n--------\r\nItem Level: 84\r\n--------\r\nCorrupted\r\n";
        let i = parse_item(t).unwrap();
        assert_eq!(i.name.as_deref(), Some("Kaom's Heart"));
        assert_eq!(i.base_type, "Glorious Plate");
        assert_eq!(i.item_level, Some(84));
        assert_eq!(i.quality, Some(20));
        assert_eq!(max_links(i.sockets.as_deref().unwrap()), 3);
        assert!(i.corrupted);
    }

    #[test]
    fn map_occupants() {
        let map = |line: &str| parse_item(&format!("Item Class: Maps\nRarity: Rare\nGrim Trail\nMap (Tier 16)\n--------\nMap Tier: 16\n--------\n{line}\n")).unwrap();
        assert_eq!(map("Map contains Drox's Citadel (implicit)").occupied_by.as_deref(), Some("Drox"));
        assert_eq!(map("Map is occupied by The Enslaver (implicit)").occupied_by.as_deref(), Some("The Enslaver"));
        assert_eq!(map("Area is influenced by Veritania, the Redeemer (implicit)").occupied_by.as_deref(), Some("Veritania"));
        let z = map("Area is influenced by the Originator's Memories (implicit)");
        assert_eq!(z.occupied_by, None);
        assert!(z.zana_memory);
    }

    #[test]
    fn currency_stack_with_commas() {
        let t = "Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n--------\nStack Size: 1,234/5,000\n";
        let i = parse_item(t).unwrap();
        assert_eq!(i.base_type, "Chaos Orb");
        assert_eq!(i.stack_size, Some(1234));
    }

    #[test]
    fn magic_base_from_known_list() {
        let t = "Item Class: Rings\nRarity: Magic\nCoral Ring of the Whelpling\n--------\nItem Level: 70\n";
        let mut i = parse_item(t).unwrap();
        let known: HashSet<String> = ["Ring", "Coral Ring", "Iron Ring"].iter().map(|s| s.to_string()).collect();
        resolve_magic_base(&mut i, &known);
        assert_eq!(i.base_type, "Coral Ring");
    }

    #[test]
    fn map_tier() {
        let t = "Item Class: Maps\nRarity: Normal\nStrand Map\n--------\nMap Tier: 16\n--------\nItem Level: 83\n";
        assert_eq!(parse_item(t).unwrap().map_tier, Some(16));
    }

    #[test]
    fn garbage_returns_none() {
        assert!(parse_item("hello world").is_none());
    }
}
