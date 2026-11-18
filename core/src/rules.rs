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
    /// Only stacks of at least this many (e.g. 3+ Chaos Orbs).
    #[serde(default)]
    pub min_stack: Option<u32>,
    /// Only stacks smaller than this (e.g. hide Gold under 500).
    #[serde(default)]
    pub below_stack: Option<u32>,
}
impl Default for MatchOpts {
    fn default() -> Self { Self { same_rarity: true, max_item_level: None, min_stack: None, below_stack: None } }
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
    if let Some(n) = opts.min_stack.filter(|n| *n > 1) {
        conditions.push(format!("StackSize >= {n}"));
    }
    if let Some(n) = opts.below_stack.filter(|n| *n > 1) {
        conditions.push(format!("StackSize < {n}"));
    }
    let mut who = match RARITIES.contains(&item.rarity.as_str()) {
        true => format!("{} {}", item.rarity, item.base_type),
        false => item.base_type.clone(),
    };
    match (opts.min_stack.filter(|n| *n > 1), opts.below_stack.filter(|n| *n > 1)) {
        (Some(a), Some(b)) => who.push_str(&format!(" (stacks {a}–{})", b - 1)),
        (Some(a), None) => who.push_str(&format!(" (stacks of {a}+)")),
        (None, Some(b)) => who.push_str(&format!(" (stacks under {b})")),
        _ => {}
    }

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

// ---------- full custom look ----------

pub const BEAM_COLOURS: [&str; 11] = ["Red", "Green", "Blue", "Brown", "White", "Yellow", "Cyan", "Grey", "Orange", "Pink", "Purple"];
pub const ICON_SHAPES: [&str; 12] = ["Circle", "Diamond", "Hexagon", "Square", "Star", "Triangle", "Cross", "Moon", "Raindrop", "Kite", "Pentagon", "UpsideDownHouse"];
pub const NAMED_SOUNDS: [&str; 10] = ["ShAlchemy", "ShBlessed", "ShChaos", "ShDivine", "ShExalted", "ShFusing", "ShGeneral", "ShMirror", "ShRegal", "ShVaal"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Beam { pub colour: String, #[serde(default)] pub temp: bool }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Minimap { pub size: u8, pub colour: String, pub shape: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Sound {
    /// The game's normal drop sound.
    Default,
    Off,
    /// "1".."16" or a named one like "ShDivine".
    Builtin { id: String, volume: u32 },
    /// A sound file in the game's filter folder.
    Custom { file: String, volume: u32 },
}

/// Exactly how an item should look. Every part is explicit, so the editor
/// starts from the item's current look and you change only what you want.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomStyle {
    pub font_size: u32,
    pub text: [u8; 4],
    pub border: Option<[u8; 4]>,
    pub background: [u8; 4],
    pub beam: Option<Beam>,
    pub minimap: Option<Minimap>,
    pub sound: Sound,
}

fn rgba(c: [u8; 4]) -> String { format!("{} {} {} {}", c[0], c[1], c[2], c[3]) }

fn pick<'a>(v: &str, list: &[&'a str], fallback: &'a str) -> &'a str {
    list.iter().find(|x| x.eq_ignore_ascii_case(v)).copied().unwrap_or(fallback)
}

/// The filter lines for a custom look. Values are checked so a typo can
/// never produce a filter the game rejects.
pub fn style_actions(s: &CustomStyle) -> Vec<String> {
    let mut a = vec![
        format!("SetFontSize {}", s.font_size.clamp(1, 45)),
        format!("SetTextColor {}", rgba(s.text)),
        format!("SetBorderColor {}", rgba(s.border.unwrap_or([0, 0, 0, 0]))),
        format!("SetBackgroundColor {}", rgba(s.background)),
    ];
    match &s.sound {
        Sound::Default => {}
        Sound::Off => a.push("DisableDropSound".into()),
        Sound::Builtin { id, volume } => {
            let id = match id.parse::<u32>() {
                Ok(n) => n.clamp(1, 16).to_string(),
                Err(_) => pick(id, &NAMED_SOUNDS, "ShGeneral").to_string(),
            };
            a.push(format!("PlayAlertSound {id} {}", volume.min(&300)));
        }
        Sound::Custom { file, volume } => {
            let clean: String = file.chars().filter(|c| !matches!(c, '"' | '\r' | '\n')).collect();
            a.push(format!("CustomAlertSound \"{clean}\" {}", volume.min(&300)));
        }
    }
    match &s.minimap {
        Some(m) => a.push(format!("MinimapIcon {} {} {}", m.size.min(2),
            pick(&m.colour, &BEAM_COLOURS, "White"), pick(&m.shape, &ICON_SHAPES, "Circle"))),
        None => a.push("MinimapIcon -1".into()),
    }
    match &s.beam {
        Some(b) => a.push(format!("PlayEffect {}{}", pick(&b.colour, &BEAM_COLOURS, "White"), if b.temp { " Temp" } else { "" })),
        None => a.push("PlayEffect None".into()),
    }
    a
}

pub fn custom_rule(item: &Item, style: &CustomStyle, opts: &MatchOpts) -> Rule {
    let Rule::Override { label, conditions, .. } = quick_rule(item, Action::Highlight, None, opts) else { unreachable!() };
    let who = label.trim_start_matches("Highlight: ").to_string();
    Rule::Override { label: format!("Style: {who}"), show: true, conditions, actions: style_actions(style) }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate::status, filter::parse_filter, item::parse_item};

    #[test]
    fn custom_look_round_trips_through_the_filter() {
        let item = parse_item("Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n").unwrap();
        let style = CustomStyle {
            font_size: 40, text: [255, 0, 0, 255], border: Some([0, 255, 0, 255]), background: [0, 0, 0, 230],
            beam: Some(Beam { colour: "cyan".into(), temp: true }),
            minimap: Some(Minimap { size: 1, colour: "pink".into(), shape: "moon".into() }),
            sound: Sound::Builtin { id: "ShDivine".into(), volume: 250 },
        };
        let mut segs = parse_filter("Show\n    BaseType == \"Chaos Orb\"\n    SetFontSize 20\n");
        apply(&mut segs, &[custom_rule(&item, &style, &MatchOpts::default())], "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let v = status(&segs, &item, Some(83), false).verdict;
        assert_eq!(v.visibility, "Show");
        assert_eq!(v.style.font_size, Some(40));
        assert_eq!(v.style.text, Some([255, 0, 0, 255]));
        assert_eq!(v.style.beam.as_deref(), Some("Cyan"));
        assert!(v.style.beam_temp);
        assert_eq!(v.style.minimap_parts, Some((1, "Pink".into(), "Moon".into())));
        assert_eq!((v.style.sound_id.as_deref(), v.style.volume), (Some("ShDivine"), Some(250)));
    }

    #[test]
    fn hide_small_gold_style_big_chaos() {
        let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n--------\nStack Size: 40\n").unwrap();
        let chaos = parse_item("Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n").unwrap();
        let hide_small = quick_rule(&gold, Action::Hide, None, &MatchOpts { below_stack: Some(500), ..Default::default() });
        let style = CustomStyle { font_size: 45, text: [255, 0, 0, 255], border: None, background: [0, 0, 0, 255],
            beam: Some(Beam { colour: "Red".into(), temp: false }), minimap: None, sound: Sound::Default };
        let big_chaos = custom_rule(&chaos, &style, &MatchOpts { min_stack: Some(3), ..Default::default() });
        let mut segs = parse_filter("Show\n    BaseType == \"Gold\" \"Chaos Orb\"\n    SetFontSize 30\n");
        apply(&mut segs, &[hide_small.clone(), big_chaos.clone()], "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let at = |it: &crate::item::Item, n: u32| {
            let it = crate::item::Item { stack_size: Some(n), ..it.clone() };
            status(&segs, &it, Some(60), false).verdict
        };
        assert_eq!(at(&gold, 40).visibility, "Hide");
        assert_eq!(at(&gold, 800).visibility, "Show");
        assert_eq!(at(&chaos, 1).style.font_size, Some(30));
        assert_eq!(at(&chaos, 3).style.font_size, Some(45));
        let Rule::Override { label, .. } = big_chaos else { panic!() };
        assert_eq!(label, "Style: Chaos Orb (stacks of 3+)");
        let Rule::Override { label, .. } = hide_small else { panic!() };
        assert_eq!(label, "Hide: Gold (stacks under 500)");
        // The pop-up's stack summary sees the new cut-off too.
        let s = status(&segs, &crate::item::Item { stack_size: Some(40), ..gold }, Some(60), false);
        assert!(s.by_stack.iter().any(|b| b.from == 500 && b.visibility == "Show"), "{:?}", s.by_stack);
    }

    #[test]
    fn silent_no_beam_no_icon() {
        let style = CustomStyle { font_size: 99, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 0],
            beam: None, minimap: None, sound: Sound::Off };
        let a = style_actions(&style);
        assert!(a.contains(&"SetFontSize 45".to_string()));
        assert!(a.contains(&"DisableDropSound".to_string()));
        assert!(a.contains(&"PlayEffect None".to_string()));
        assert!(a.contains(&"MinimapIcon -1".to_string()));
    }

    #[test]
    fn custom_sound_file_is_sanitised() {
        let style = CustomStyle { font_size: 30, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 0],
            beam: None, minimap: None, sound: Sound::Custom { file: "gold.wav\"\nHide".into(), volume: 900 } };
        assert!(style_actions(&style).contains(&"CustomAlertSound \"gold.wavHide\" 300".to_string()));
    }
}
