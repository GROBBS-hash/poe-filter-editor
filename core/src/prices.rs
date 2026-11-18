//! Price tiering: sorts items into your own value tiers using poe.ninja
//! prices (from the app's hourly price file) and writes filter blocks for them.
//! Your manual edits stay on top; price tiers come next; NeverSink after that.

use crate::rules::{style_actions, Beam, CustomStyle, Minimap, Sound};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceItem {
    pub name: String,
    pub base: String,
    pub cat: String,
    pub value: f64,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub listings: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFile {
    pub game: String,
    pub league: String,
    #[serde(default)]
    pub league_name: Option<String>,
    pub updated: String,
    /// "chaos" (PoE 1) or "exalted" (PoE 2).
    pub unit: String,
    pub items: Vec<PriceItem>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group { Uniques, DivinationCards, Currency, Other }

impl Group {
    pub const ALL: [Group; 4] = [Group::Uniques, Group::DivinationCards, Group::Currency, Group::Other];
    pub fn title(self) -> &'static str {
        match self {
            Group::Uniques => "Uniques",
            Group::DivinationCards => "Divination cards",
            Group::Currency => "Currency & league items",
            Group::Other => "Other",
        }
    }
    pub fn of(cat: &str) -> Option<Group> {
        if cat.starts_with("Unique") {
            return Some(Group::Uniques);
        }
        match cat {
            "DivinationCard" => Some(Group::DivinationCards),
            "Incubator" | "Invitation" | "Memory" | "Vial" | "Corpse" | "Wombgift" => Some(Group::Other),
            // Everything else poe.ninja prices on its exchange.
            _ => Some(Group::Currency),
        }
    }
    /// Extra conditions so a tier only catches the right kind of item.
    fn conditions(self) -> Vec<String> {
        match self {
            Group::Uniques => vec!["Rarity Unique".into()],
            Group::DivinationCards => vec!["Class == \"Divination Cards\"".into()],
            _ => vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tier {
    /// Worth at least this much (in the price file's unit).
    pub min: f64,
    pub style: CustomStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupSettings {
    pub group: Group,
    pub enabled: bool,
    /// Highest first is not required; they're sorted when used.
    pub tiers: Vec<Tier>,
    /// Hide priced items worth less than this (unpriced items are left to NeverSink).
    #[serde(default)]
    pub hide_below: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceSettings {
    pub enabled: bool,
    pub groups: Vec<GroupSettings>,
}

fn look(size: u32, text: [u8; 4], border: [u8; 4], bg: [u8; 4], beam: &str, icon: (u8, &str, &str), sound: Sound) -> CustomStyle {
    CustomStyle {
        font_size: size, text, border: Some(border), background: bg,
        beam: Some(Beam { colour: beam.into(), temp: false }),
        minimap: Some(Minimap { size: icon.0, colour: icon.1.into(), shape: icon.2.into() }),
        sound,
        label_hidden: false,
    }
}

/// Sensible starting tiers: big and loud for the best, calmer further down.
pub fn default_settings(unit: &str) -> PriceSettings {
    let (t1, t2, t3) = if unit == "chaos" { (100.0, 20.0, 5.0) } else { (500.0, 100.0, 20.0) };
    let tiers = |sound1: &str| vec![
        Tier { min: t1, style: look(45, [255, 0, 0, 255], [255, 0, 0, 255], [255, 255, 255, 255], "Red", (0, "Red", "Star"),
            Sound::Builtin { id: sound1.into(), volume: 300 }) },
        Tier { min: t2, style: look(42, [0, 0, 0, 255], [0, 0, 0, 255], [255, 210, 60, 255], "Yellow", (1, "Yellow", "Diamond"),
            Sound::Builtin { id: "2".into(), volume: 250 }) },
        Tier { min: t3, style: look(38, [255, 255, 255, 255], [255, 255, 255, 255], [40, 40, 40, 240], "White", (2, "White", "Circle"),
            Sound::Default) },
    ];
    PriceSettings {
        enabled: false,
        groups: Group::ALL.iter().map(|g| GroupSettings {
            group: *g,
            enabled: matches!(g, Group::Uniques | Group::DivinationCards),
            // Tier 1 always gets NeverSink's tier 1 sound: the classic Divine Orb drop.
            tiers: tiers("6"),
            hide_below: None,
        }).collect(),
    }
}

/// Value of each base type in a group. Uniques sharing a base take the most
/// valuable one, so hiding or downgrading a base never loses a good unique.
pub fn values_by_base(file: &PriceFile, group: Group) -> BTreeMap<String, f64> {
    let mut m: BTreeMap<String, f64> = BTreeMap::new();
    for it in file.items.iter().filter(|i| Group::of(&i.cat) == Some(group)) {
        let key = if group == Group::Uniques { it.base.clone() } else { it.name.clone() };
        let e = m.entry(key).or_insert(0.0);
        if it.value > *e { *e = it.value; }
    }
    m
}

fn base_list(names: &[&String]) -> String {
    names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ")
}

pub fn unit_short(unit: &str) -> &'static str {
    match unit { "chaos" => "c", "exalted" => "ex", "divine" => "div", _ => "" }
}

/// Filter blocks for your price tiers (empty if switched off).
pub fn price_blocks(settings: &PriceSettings, file: &PriceFile, nl: &str) -> String {
    if !settings.enabled {
        return String::new();
    }
    let u = unit_short(&file.unit);
    let mut out = format!("# Price tiers from poe.ninja ({}, {}). Rebuilt when prices update.{nl}{nl}", file.league, file.updated);
    for g in settings.groups.iter().filter(|g| g.enabled) {
        let values = values_by_base(file, g.group);
        let mut tiers = g.tiers.clone();
        tiers.sort_by(|a, b| b.min.partial_cmp(&a.min).unwrap_or(std::cmp::Ordering::Equal));
        let mut upper = f64::INFINITY;
        for (i, t) in tiers.iter().enumerate() {
            let names: Vec<&String> = values.iter().filter(|(_, v)| **v >= t.min && **v < upper).map(|(k, _)| k).collect();
            upper = t.min;
            if names.is_empty() { continue; }
            out.push_str(&format!("Show # Price tier {}: {} worth {}{u}+{nl}", i + 1, g.group.title(), t.min));
            for c in g.group.conditions() { out.push_str(&format!("    {c}{nl}")); }
            out.push_str(&format!("    BaseType == {}{nl}", base_list(&names)));
            for a in style_actions(&t.style) { out.push_str(&format!("    {a}{nl}")); }
            out.push_str(nl);
        }
        if let Some(limit) = g.hide_below.filter(|_| g.group != Group::Currency) {
            let names: Vec<&String> = values.iter().filter(|(_, v)| **v < limit).map(|(k, _)| k).collect();
            if !names.is_empty() {
                out.push_str(&format!("Hide # Price: {} worth under {limit}{u}{nl}", g.group.title()));
                for c in g.group.conditions() { out.push_str(&format!("    {c}{nl}")); }
                out.push_str(&format!("    BaseType == {}{nl}", base_list(&names)));
                out.push_str(&format!("    DisableDropSound True{nl}{nl}"));
            }
        }
    }
    out
}

/// The price of one item, if poe.ninja has it (for the in-game pop-up).
pub fn price_of(file: &PriceFile, name: Option<&str>, base: &str, rarity: &str) -> Option<f64> {
    if rarity == "Unique" {
        if let Some(n) = name {
            if let Some(v) = file.items.iter().filter(|i| i.name == n && i.cat.starts_with("Unique")).map(|i| i.value).reduce(f64::max) {
                return Some(v);
            }
        }
        return values_by_base(file, Group::Uniques).get(base).copied();
    }
    file.items.iter().filter(|i| i.name == base && !i.cat.starts_with("Unique")).map(|i| i.value).reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate::status, filter::parse_filter, item::parse_item, rules::apply};

    fn file() -> PriceFile {
        let it = |name: &str, base: &str, cat: &str, value: f64| PriceItem {
            name: name.into(), base: base.into(), cat: cat.into(), value, variant: None, listings: Some(10) };
        PriceFile {
            game: "poe1".into(), league: "Covens".into(), league_name: None, updated: "now".into(), unit: "chaos".into(),
            items: vec![
                it("Headhunter", "Leather Belt", "UniqueAccessory", 29584.0),
                it("Wurm's Molt", "Leather Belt", "UniqueAccessory", 1.0),
                it("Goldrim", "Leather Cap", "UniqueArmour", 1.0),
                it("Kaom's Heart", "Glorious Plate", "UniqueArmour", 30.0),
                it("The Doctor", "The Doctor", "DivinationCard", 900.0),
                it("Rain of Chaos", "Rain of Chaos", "DivinationCard", 0.3),
                it("Divine Orb", "Divine Orb", "Currency", 176.0),
            ],
        }
    }

    #[test]
    fn shared_base_uses_best_unique() {
        let v = values_by_base(&file(), Group::Uniques);
        assert_eq!(v["Leather Belt"], 29584.0);
        assert_eq!(price_of(&file(), Some("Wurm's Molt"), "Leather Belt", "Unique"), Some(1.0));
        assert_eq!(price_of(&file(), None, "Leather Belt", "Unique"), Some(29584.0));
        assert_eq!(price_of(&file(), None, "Divine Orb", "Currency"), Some(176.0));
    }

    #[test]
    fn tiers_and_hide_below_in_the_filter() {
        let mut s = default_settings("chaos");
        s.enabled = true;
        s.groups.iter_mut().find(|g| g.group == Group::Uniques).unwrap().hide_below = Some(5.0);
        let blocks = price_blocks(&s, &file(), "\n");
        assert!(blocks.contains("Price tier 1: Uniques worth 100c+"));
        // Leather Belt (Headhunter) is tier 1, never hidden; Leather Cap is hidden.
        let t1 = blocks.split("Show # Price tier 1: Uniques").nth(1).unwrap().split("\n\n").next().unwrap();
        assert!(t1.contains("\"Leather Belt\"") && !t1.contains("Leather Cap"));
        let hide = blocks.split("Hide # Price: Uniques").nth(1).unwrap().split("\n\n").next().unwrap();
        assert!(hide.contains("\"Leather Cap\"") && !hide.contains("Leather Belt"));

        // Through the whole pipeline: user edits on top, then prices, then the base filter.
        let mut segs = parse_filter("Show\n    Rarity Unique\n    SetFontSize 30\n\nShow\n    Class == \"Divination Cards\"\n    SetFontSize 30\n");
        crate::rules::apply_with_extra(&mut segs, &[], &blocks, "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let st = |clip: &str| status(&segs, &parse_item(clip).unwrap(), Some(83), false).verdict;
        let doctor = st("Item Class: Divination Cards\nRarity: Normal\nThe Doctor\n");
        assert_eq!((doctor.visibility.as_str(), doctor.style.font_size), ("Show", Some(45)));
        assert!(doctor.decided_by.starts_with("Price tier 1"));
        assert_eq!(st("Item Class: Helmets\nRarity: Unique\nGoldrim\nLeather Cap\n").visibility, "Hide");
        assert_eq!(st("Item Class: Body Armours\nRarity: Unique\nKaom's Heart\nGlorious Plate\n").style.font_size, Some(42));
        let _ = apply; // plain apply still exists for profiles without prices
    }

    #[test]
    fn off_means_nothing_written() {
        assert!(price_blocks(&default_settings("chaos"), &file(), "\n").is_empty());
    }
}
