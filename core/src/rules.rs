//! The user's edits, stored separately from NeverSink's file and applied on top.

use crate::filter::{move_base, Block, Segment};
use crate::item::Item;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Rule {
    /// A block placed at the very top of the filter, so it wins over everything.
    Override { label: String, show: bool, conditions: Vec<String>, actions: Vec<String> },
    /// Move a base into another of NeverSink's tiers within the same section.
    MoveBase { type_path: Vec<String>, base: String, to_tier: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action { Hide, Highlight, Louder, Quieter }

/// How tightly a quick rule matches. Defaults keep it safe: hiding a magic
/// Coral Ring never hides a rare one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchOpts {
    pub same_rarity: bool,
    pub max_item_level: Option<u32>, // only items at or below this ilvl
}
impl Default for MatchOpts {
    fn default() -> Self { Self { same_rarity: true, max_item_level: None } }
}

const RARITIES: [&str; 4] = ["Normal", "Magic", "Rare", "Unique"];

/// Named colours with matching minimap icon colours the game accepts.
pub fn colour(name: &str) -> (&'static str, &'static str) {
    match name.to_lowercase().as_str() {
        "gold" | "yellow" => ("255 200 40", "Yellow"),
        "cyan" => ("0 240 190", "Cyan"),
        "red" => ("255 60 60", "Red"),
        "green" => ("70 230 70", "Green"),
        "blue" => ("60 140 255", "Blue"),
        "white" => ("255 255 255", "White"),
        "orange" => ("255 140 30", "Orange"),
        _ => ("170 60 255", "Purple"),
    }
}

pub fn quick_rule(item: &Item, action: Action, colour_name: Option<&str>, opts: &MatchOpts) -> Rule {
    let mut conditions = vec![
        format!("Class == \"{}\"", item.item_class),
        format!("BaseType == \"{}\"", item.base_type),
    ];
    if opts.same_rarity && RARITIES.contains(&item.rarity.as_str()) {
        conditions.push(format!("Rarity = {}", item.rarity));
    }
    if let Some(lvl) = opts.max_item_level {
        conditions.push(format!("ItemLevel <= {lvl}"));
    }
    let who = match RARITIES.contains(&item.rarity.as_str()) {
        true => format!("{} {}", item.rarity, item.base_type),
        false => item.base_type.clone(),
    };

    let (show, actions, verb) = match action {
        Action::Hide => (false, vec!["DisableDropSound".into()], "Hide"),
        Action::Highlight => {
            let (rgb, icon) = colour(colour_name.unwrap_or("purple"));
            (true, vec![
                "SetFontSize 42".into(),
                format!("SetTextColor {rgb} 255"),
                format!("SetBorderColor {rgb} 255"),
                "SetBackgroundColor 20 20 20 240".into(),
                format!("MinimapIcon 1 {icon} Star"),
                format!("PlayEffect {icon}"),
            ], "Highlight")
        }
        Action::Louder => (true, vec!["SetFontSize 45".into(), "PlayAlertSound 2 300".into()], "Louder"),
        Action::Quieter => (true, vec!["SetFontSize 26".into(), "DisableDropSound".into()], "Quieter"),
    };
    Rule::Override { label: format!("{verb}: {who}"), show, conditions, actions }
}

/// Every base name NeverSink lists, used to work out magic item bases.
pub fn known_bases(segs: &[Segment]) -> HashSet<String> {
    segs.iter()
        .filter_map(|s| if let Segment::Block(b) = s { Some(b.base_types()) } else { None })
        .flatten()
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Location { pub type_path: Vec<String>, pub tier: Option<String>, pub visibility: String }

/// Where this item currently sits in NeverSink's filter (for the overlay to
/// show "Uniques > T2", and for moving it between tiers).
pub fn locate(segs: &[Segment], item: &Item) -> Vec<Location> {
    segs.iter()
        .filter_map(|s| if let Segment::Block(b) = s { Some(b) } else { None })
        .filter(|b| b.base_types().iter().any(|x| x == &item.base_type))
        .filter(|b| rarity_matches(b, &item.rarity))
        .map(|b| Location { type_path: b.type_path.clone(), tier: b.tier.clone(), visibility: b.visibility.clone() })
        .collect()
}

fn rarity_matches(b: &Block, rarity: &str) -> bool {
    let Some(line) = b.lines.iter().map(|l| l.trim()).find(|l| l.starts_with("Rarity")) else { return true };
    let Some(rank) = RARITIES.iter().position(|r| *r == rarity) else { return true };
    let rest = line["Rarity".len()..].trim();
    let (op, vals) = ["<=", ">=", "==", "=", "<", ">"]
        .iter()
        .find_map(|op| rest.strip_prefix(op).map(|v| (*op, v)))
        .unwrap_or(("=", rest));
    let ranks: Vec<usize> = vals.split_whitespace()
        .filter_map(|v| RARITIES.iter().position(|r| *r == v)).collect();
    match (op, ranks.first()) {
        ("<=", Some(&v)) => rank <= v,
        (">=", Some(&v)) => rank >= v,
        ("<", Some(&v)) => rank < v,
        (">", Some(&v)) => rank > v,
        _ => ranks.contains(&rank),
    }
}

pub const START: &str = "#=== LIVE-EDITOR START ===";
pub const END: &str = "#=== LIVE-EDITOR END ===";

/// Applies rules to a parsed filter. Overrides go at the top, newest first,
/// so your latest choice always wins. Returns warnings for rules that no
/// longer fit (e.g. after a NeverSink update renamed a tier).
pub fn apply(segs: &mut Vec<Segment>, rules: &[Rule], nl: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut top = format!("{START}{nl}# Edits made with the live editor. Rebuilt automatically; edit in the app.{nl}{nl}");
    for rule in rules.iter().rev() {
        match rule {
            Rule::Override { label, show, conditions, actions } => {
                top.push_str(&format!("{} # {label}{nl}", if *show { "Show" } else { "Hide" }));
                for l in conditions.iter().chain(actions) {
                    top.push_str(&format!("    {l}{nl}"));
                }
                top.push_str(nl);
            }
            Rule::MoveBase { type_path, base, to_tier } => {
                let tp: Vec<&str> = type_path.iter().map(String::as_str).collect();
                if !move_base(segs, &tp, base, to_tier) {
                    warnings.push(format!("Couldn't move {base} to {} {to_tier}: that tier is gone in this version", type_path.join(" > ")));
                }
            }
        }
    }
    top.push_str(&format!("{END}{nl}{nl}"));
    segs.insert(0, Segment::Raw(top));
    warnings
}
