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
    /// Change one of NeverSink's tiers as a whole: show/hide it, or give it a new look.
    Tier {
        type_path: Vec<String>, tier: String, show: Option<bool>, actions: Option<Vec<String>>,
        /// New "StackSize >= N" for stack and gold tiers.
        #[serde(default)]
        min_stack: Option<u32>,
        /// New "UnidentifiedItemTier >= N" for rare gear tiers (3.30 item tiers).
        #[serde(default)]
        min_item_tier: Option<u32>,
    },
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
    /// Only while levelling (areas below 68) or only in maps (68+).
    #[serde(default)]
    pub when: When,
}
impl Default for MatchOpts {
    fn default() -> Self { Self { same_rarity: true, max_item_level: None, min_stack: None, below_stack: None, when: When::Always } }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum When {
    #[default]
    Always,
    /// The campaign: areas below level 68.
    Levelling,
    /// Endgame: areas level 68 and up.
    Maps,
}

impl When {
    fn condition(self) -> Option<&'static str> {
        match self { When::Always => None, When::Levelling => Some("AreaLevel < 68"), When::Maps => Some("AreaLevel >= 68") }
    }
    fn words(self) -> &'static str {
        match self { When::Always => "", When::Levelling => " while levelling", When::Maps => " in maps" }
    }
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
    // An empty base means "every item of this type" (e.g. every Two Hand Maces).
    let mut conditions = vec![format!("Class == \"{}\"", item.item_class)];
    if !item.base_type.is_empty() {
        conditions.push(format!("BaseType == \"{}\"", item.base_type));
    }
    // A blighted map's edit only covers blighted maps of that tier, and a plain map's edit leaves blighted ones alone.
    if item.item_class == "Maps" && item.map_kind_known {
        conditions.push(format!("BlightedMap {}", if item.blighted { "True" } else { "False" }));
        if item.uber_blighted { conditions.push("UberBlightedMap True".into()); }
    }
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
    if let Some(c) = opts.when.condition() {
        conditions.push(c.to_string());
    }
    let what = if item.base_type.is_empty() { format!("every {}", item.item_class) } else { item.base_type.clone() };
    let what = if item.uber_blighted { format!("Blight-ravaged {what}") } else if item.blighted { format!("Blighted {what}") } else { what };
    let mut who = match RARITIES.contains(&item.rarity.as_str()) && opts.same_rarity {
        true => format!("{} {what}", item.rarity),
        false => what,
    };
    match (opts.min_stack.filter(|n| *n > 1), opts.below_stack.filter(|n| *n > 1)) {
        (Some(a), Some(b)) => who.push_str(&format!(" (stacks {a}–{})", b - 1)),
        (Some(a), None) => who.push_str(&format!(" (stacks of {a}+)")),
        (None, Some(b)) => who.push_str(&format!(" (stacks under {b})")),
        _ => {}
    }
    who.push_str(opts.when.words());

    let (show, actions, verb) = match action {
        Action::Hide => (false, vec!["DisableDropSound True".into()], "Hide"),
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
        Action::Quieter => (true, vec!["SetFontSize 26".into(), "DisableDropSound True".into()], "Quieter"),
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
    /// Icon & beam only: the label is shrunk and see-through, so only the
    /// minimap icon and beam show.
    #[serde(default)]
    pub label_hidden: bool,
}

fn rgba(c: [u8; 4]) -> String { format!("{} {} {} {}", c[0], c[1], c[2], c[3]) }

fn pick<'a>(v: &str, list: &[&'a str], fallback: &'a str) -> &'a str {
    list.iter().find(|x| x.eq_ignore_ascii_case(v)).copied().unwrap_or(fallback)
}

/// The filter lines for a custom look. Values are checked so a typo can
/// never produce a filter the game rejects.
pub fn style_actions(s: &CustomStyle) -> Vec<String> {
    let clear = |c: [u8; 4]| [c[0], c[1], c[2], 0];
    let mut a = if s.label_hidden {
        vec![
            "SetFontSize 1".to_string(),
            format!("SetTextColor {}", rgba(clear(s.text))),
            format!("SetBorderColor {}", rgba(clear(s.border.unwrap_or([0, 0, 0, 0])))),
            format!("SetBackgroundColor {}", rgba(clear(s.background))),
        ]
    } else {
        vec![
            format!("SetFontSize {}", s.font_size.clamp(1, 45)),
            format!("SetTextColor {}", rgba(s.text)),
            format!("SetBorderColor {}", rgba(s.border.unwrap_or([0, 0, 0, 0]))),
            format!("SetBackgroundColor {}", rgba(s.background)),
        ]
    };
    match &s.sound {
        Sound::Default => {}
        Sound::Off => a.push("DisableDropSound True".into()),
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

// ---------- editing saved rules ----------

/// A saved edit, opened for changing: what it matches and how it looks.
#[derive(Debug, Clone, Serialize)]
pub struct RuleDetail {
    pub label: String,
    pub show: bool,
    pub item_class: String,
    pub base_type: String,
    /// None = applies to every rarity.
    pub rarity: Option<String>,
    pub min_stack: Option<u32>,
    pub below_stack: Option<u32>,
    #[serde(default)]
    pub when: When,
    pub style: crate::evaluate::Style,
}

/// Changes to apply to a saved edit.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleEdit {
    pub show: bool,
    /// Required when shown.
    pub style: Option<CustomStyle>,
    pub every_rarity: bool,
    pub min_stack: Option<u32>,
    pub below_stack: Option<u32>,
    #[serde(default)]
    pub when: When,
}

fn first_quoted(line: &str) -> Option<String> {
    line.split('"').nth(1).map(str::to_string)
}

pub fn rule_detail(rule: &Rule) -> Option<RuleDetail> {
    let Rule::Override { label, show, conditions, actions } = rule else { return None };
    if label.starts_with("Preset:") {
        return None; // presets cover many items; they're removed, not edited
    }
    let mut d = RuleDetail {
        label: label.clone(), show: *show, item_class: String::new(), base_type: String::new(),
        rarity: None, min_stack: None, below_stack: None, when: When::Always, style: crate::evaluate::style_of(actions),
    };
    for c in conditions {
        let c = c.trim();
        if c.starts_with("Class") { d.item_class = first_quoted(c).unwrap_or_default(); }
        else if c.starts_with("BaseType") { d.base_type = first_quoted(c).unwrap_or_default(); }
        else if let Some(r) = c.strip_prefix("Rarity = ") { d.rarity = Some(r.trim().to_string()); }
        else if let Some(n) = c.strip_prefix("StackSize >= ") { d.min_stack = n.trim().parse().ok(); }
        else if let Some(n) = c.strip_prefix("StackSize < ") { d.below_stack = n.trim().parse().ok(); }
        else if c == "AreaLevel < 68" { d.when = When::Levelling; }
        else if c == "AreaLevel >= 68" { d.when = When::Maps; }
    }
    Some(d)
}

/// Two edits fill the same "slot" when they're about the same item and the
/// same kind of stack limit (none / N+ / under N / a range). A new edit
/// replaces an older one in its slot instead of piling up on top of it.
pub fn same_slot(a: &Rule, b: &Rule) -> bool {
    let (Some(x), Some(y)) = (rule_detail(a), rule_detail(b)) else { return false };
    x.item_class == y.item_class && x.base_type == y.base_type && x.rarity == y.rarity
        && x.min_stack.is_some() == y.min_stack.is_some()
        && x.below_stack.is_some() == y.below_stack.is_some()
        && x.when == y.when
}

/// For each edit, the newer edit that always wins over it (so it never
/// applies), if any. Newer edits sit higher in the filter.
pub fn shadowed_by(rules: &[Rule]) -> Vec<Option<usize>> {
    let details: Vec<Option<RuleDetail>> = rules.iter().map(rule_detail).collect();
    (0..rules.len()).map(|i| {
        let old = details[i].as_ref()?;
        (i + 1..rules.len()).rev().find(|&j| {
            let Some(new) = details[j].as_ref() else { return false };
            let same_item = new.item_class == old.item_class
                && (new.base_type == old.base_type || new.base_type.is_empty());
            let covers_rarity = new.rarity.is_none() || new.rarity == old.rarity;
            let covers_stacks = new.min_stack.is_none() && new.below_stack.is_none();
            let covers_when = new.when == When::Always || new.when == old.when;
            same_item && covers_rarity && covers_stacks && covers_when
        })
    }).collect()
}

/// Adds an edit, replacing any older edit in the same slot.
pub fn add_rule(rules: &mut Vec<Rule>, rule: Rule) {
    rules.retain(|r| !same_slot(r, &rule));
    rules.push(rule);
}

/// Rebuilds a saved edit with changes, keeping what it matches.
pub fn edited_rule(rule: &Rule, edit: &RuleEdit) -> Option<Rule> {
    let d = rule_detail(rule)?;
    let item = Item {
        item_class: d.item_class.clone(),
        base_type: d.base_type.clone(),
        rarity: d.rarity.clone().unwrap_or_else(|| "Currency".into()),
        ..Default::default()
    };
    let opts = MatchOpts {
        same_rarity: !edit.every_rarity && d.rarity.is_some(),
        max_item_level: None,
        min_stack: edit.min_stack,
        below_stack: edit.below_stack,
        when: edit.when,
    };
    Some(match (edit.show, &edit.style) {
        (true, Some(style)) => custom_rule(&item, style, &opts),
        _ => quick_rule(&item, Action::Hide, None, &opts),
    })
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
    apply_with_extra(segs, rules, "", nl)
}

/// Like `apply`, with extra generated blocks (price tiers) placed after your
/// own edits and before the base filter.
pub fn apply_with_extra(segs: &mut Vec<Segment>, rules: &[Rule], extra: &str, nl: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    // NeverSink ships his citadel (conqueror) map rules switched off; the app switches them on so those maps stand out.
    crate::filter::enable_commented(segs, &["maps", "implicitmod"]);
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
            Rule::Tier { type_path, tier, show, actions, min_stack, min_item_tier } => {
                let mut found = false;
                for seg in segs.iter_mut() {
                    let Segment::Block(b) = seg else { continue };
                    if b.type_path != *type_path || b.tier.as_deref() != Some(tier.as_str()) { continue; }
                    found = true;
                    if let Some(sh) = show { b.set_visibility(*sh); }
                    if let Some(a) = actions { b.replace_style(a); }
                    if let Some(n) = min_stack { b.set_min_stack(*n); }
                    if let Some(n) = min_item_tier { b.set_min("UnidentifiedItemTier", *n); }
                }
                if !found {
                    warnings.push(format!("Couldn't change {} {tier}: that tier is gone in this version", type_path.join(" > ")));
                }
            }
            Rule::MoveBase { type_path, base, to_tier } if to_tier.is_empty() => {
                // Take the item out of every tier of this section.
                for seg in segs.iter_mut() {
                    let Segment::Block(b) = seg else { continue };
                    if b.type_path != *type_path { continue; }
                    let mut bases = b.base_types();
                    if bases.iter().any(|x| x == base) { bases.retain(|x| x != base); b.set_base_types(&bases); }
                }
            }
            Rule::MoveBase { type_path, base, to_tier } => {
                let tp: Vec<&str> = type_path.iter().map(String::as_str).collect();
                if !move_base(segs, &tp, base, to_tier) {
                    warnings.push(format!("Couldn't move {base} to {} {to_tier}: that tier is gone in this version", type_path.join(" > ")));
                }
            }
        }
    }
    top.push_str(extra);
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
            label_hidden: false,
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
            beam: Some(Beam { colour: "Red".into(), temp: false }), minimap: None, sound: Sound::Default, label_hidden: false };
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
    fn edit_a_saved_rule() {
        let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
        let hide = quick_rule(&gold, Action::Hide, None, &MatchOpts { below_stack: Some(500), ..Default::default() });
        let d = rule_detail(&hide).unwrap();
        assert_eq!((d.base_type.as_str(), d.item_class.as_str(), d.below_stack, d.show), ("Gold", "Gold", Some(500), false));
        // Raise the cut-off to 800.
        let e = RuleEdit { show: false, style: None, every_rarity: false, min_stack: None, below_stack: Some(800), when: When::Always };
        let Rule::Override { label, conditions, .. } = edited_rule(&hide, &e).unwrap() else { panic!() };
        assert_eq!(label, "Hide: Gold (stacks under 800)");
        assert!(conditions.contains(&"StackSize < 800".to_string()));

        // A unique belt style: switch to every rarity and a new colour.
        let belt = parse_item("Item Class: Belts\nRarity: Unique\nHeadhunter\nLeather Belt\n").unwrap();
        let style = CustomStyle { font_size: 40, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
        let r = custom_rule(&belt, &style, &MatchOpts::default());
        assert_eq!(rule_detail(&r).unwrap().rarity.as_deref(), Some("Unique"));
        assert_eq!(rule_detail(&r).unwrap().style.font_size, Some(40));
        let e = RuleEdit { show: true, style: Some(CustomStyle { text: [9, 9, 9, 255], ..style }), every_rarity: true, min_stack: None, below_stack: None, when: When::Always };
        let Rule::Override { conditions, actions, .. } = edited_rule(&r, &e).unwrap() else { panic!() };
        assert!(!conditions.iter().any(|c| c.starts_with("Rarity")));
        assert!(actions.contains(&"SetTextColor 9 9 9 255".to_string()));
    }

    #[test]
    fn hide_small_and_style_big_gold_live_together() {
        let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
        let style = CustomStyle { font_size: 40, text: [0, 255, 0, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
        let mut rules = Vec::new();
        add_rule(&mut rules, quick_rule(&gold, Action::Hide, None, &MatchOpts { below_stack: Some(500), ..Default::default() }));
        add_rule(&mut rules, custom_rule(&gold, &style, &MatchOpts { min_stack: Some(500), ..Default::default() }));
        assert_eq!(rules.len(), 2, "different slots: both kept");
        add_rule(&mut rules, quick_rule(&gold, Action::Hide, None, &MatchOpts { below_stack: Some(800), ..Default::default() }));
        assert_eq!(rules.len(), 2, "same slot: the under-500 hide is replaced by under-800");
        let labels: Vec<String> = rules.iter().map(|r| rule_detail(r).unwrap().label).collect();
        assert!(labels.contains(&"Hide: Gold (stacks under 800)".to_string()));
        assert!(labels.contains(&"Style: Gold (stacks of 500+)".to_string()));

        let mut segs = parse_filter("Show\n    BaseType == \"Gold\"\n    SetFontSize 30\n");
        apply(&mut segs, &rules, "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let at = |n: u32| status(&segs, &crate::item::Item { stack_size: Some(n), ..gold.clone() }, Some(70), false).verdict;
        assert_eq!(at(200).visibility, "Hide");
        assert_eq!(at(600).visibility, "Hide", "under 800 now hidden");
        assert_eq!((at(900).visibility.as_str(), at(900).style.text), ("Show", Some([0, 255, 0, 255])));
    }

    #[test]
    fn icon_and_beam_only() {
        let style = CustomStyle { font_size: 40, text: [255, 0, 0, 255], border: Some([255, 0, 0, 255]), background: [0, 0, 0, 240],
            beam: Some(Beam { colour: "Red".into(), temp: false }), minimap: Some(Minimap { size: 0, colour: "Red".into(), shape: "Star".into() }),
            sound: Sound::Off, label_hidden: true };
        let a = style_actions(&style);
        assert!(a.contains(&"SetFontSize 1".to_string()));
        assert!(a.contains(&"SetTextColor 255 0 0 0".to_string()));
        assert!(a.contains(&"SetBackgroundColor 0 0 0 0".to_string()));
        assert!(a.contains(&"PlayEffect Red".to_string()));
        assert!(a.contains(&"MinimapIcon 0 Red Star".to_string()));
        let st = crate::evaluate::style_of(&a);
        assert!(st.label_hidden);
    }

    #[test]
    fn spots_edits_that_never_apply() {
        let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
        let belt = parse_item("Item Class: Belts\nRarity: Unique\nHeadhunter\nLeather Belt\n").unwrap();
        let style = CustomStyle { font_size: 40, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
        let rules = vec![
            custom_rule(&gold, &style, &MatchOpts { min_stack: Some(500), ..Default::default() }), // 0: covered by 2
            quick_rule(&belt, Action::Hide, None, &MatchOpts::default()),                          // 1: fine
            quick_rule(&gold, Action::Hide, None, &MatchOpts::default()),                          // 2: hides all gold
            quick_rule(&gold, Action::Hide, None, &MatchOpts { below_stack: Some(100), ..Default::default() }), // 3: fine (newest)
        ];
        assert_eq!(shadowed_by(&rules), vec![Some(2), None, None, None]);
    }

    #[test]
    fn wands_only_while_levelling() {
        let wands = parse_item("Item Class: Wands\nRarity: Rare\nX\nOpal Wand\n").unwrap();
        let wands = crate::item::Item { base_type: String::new(), ..wands };
        let style = CustomStyle { font_size: 42, text: [0, 255, 255, 255], border: None, background: [0, 0, 0, 255],
            beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
        let r = custom_rule(&wands, &style, &MatchOpts { same_rarity: false, when: When::Levelling, ..Default::default() });
        let d = rule_detail(&r).unwrap();
        assert_eq!((d.label.as_str(), d.when), ("Style: every Wands while levelling", When::Levelling));
        let mut segs = parse_filter("Show\n    Class == \"Wands\"\n    SetFontSize 30\n");
        apply(&mut segs, &[r], "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let item = crate::item::Item { base_type: "Opal Wand".into(), ..wands };
        assert_eq!(status(&segs, &item, Some(30), false).verdict.style.font_size, Some(42));
        assert_eq!(status(&segs, &item, Some(75), false).verdict.style.font_size, Some(30));
    }

    #[test]
    fn silent_no_beam_no_icon() {
        let style = CustomStyle { font_size: 99, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 0],
            beam: None, minimap: None, sound: Sound::Off, label_hidden: false };
        let a = style_actions(&style);
        assert!(a.contains(&"SetFontSize 45".to_string()));
        assert!(a.contains(&"DisableDropSound True".to_string()));
        assert!(a.contains(&"PlayEffect None".to_string()));
        assert!(a.contains(&"MinimapIcon -1".to_string()));
    }

    #[test]
    fn custom_sound_file_is_sanitised() {
        let style = CustomStyle { font_size: 30, text: [1, 2, 3, 255], border: None, background: [0, 0, 0, 0],
            beam: None, minimap: None, sound: Sound::Custom { file: "gold.wav\"\nHide".into(), volume: 900 }, label_hidden: false };
        assert!(style_actions(&style).contains(&"CustomAlertSound \"gold.wavHide\" 300".to_string()));
    }
}
