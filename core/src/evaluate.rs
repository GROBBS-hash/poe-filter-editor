//! Works out what a filter does to an item, the way the game does it:
//! read blocks top to bottom, first full match wins (Continue keeps going).
//! Conditions we can't check from the copied text (mods, influences...) are
//! "unknown"; we report those honestly instead of guessing.

use crate::filter::{Block, Segment};
use crate::item::{is_gear, max_links, Item};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tri { Yes, No, Unknown }

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Style {
    pub font_size: Option<u32>,
    pub text: Option<[u8; 4]>,
    pub border: Option<[u8; 4]>,
    pub background: Option<[u8; 4]>,
    pub sound: Option<String>,   // "Sound 1", "custom.mp3", or None
    pub muted: bool,             // DisableDropSound
    pub minimap: Option<String>, // "Large Red Star"
    pub beam: Option<String>,    // "Red"
    pub beam_temp: bool,
    pub sound_id: Option<String>,   // "2" or "ShDivine" (built-in)
    pub sound_file: Option<String>, // custom sound, as written in the filter
    pub volume: Option<u32>,
    pub minimap_parts: Option<(u8, String, String)>, // (size, colour, shape)
    /// Label shrunk and see-through: only the icon and beam show.
    pub label_hidden: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Verdict {
    /// "Show", "Hide" or "Minimal". "None" = no block matched (game shows it plainly).
    pub visibility: String,
    pub style: Style,
    /// Which block decided it, e.g. "uniques › t2", or your edit's label.
    pub decided_by: String,
    pub by_user_edit: bool,
    /// Plain-English reason, e.g. "Plain normal/magic gear, once you're in maps".
    pub reason: String,
    /// Earlier blocks that might apply depending on things we can't see (mods etc).
    pub uncertain: Vec<String>,
    /// Exactly which rule decided it, so the app can open it for editing.
    pub source: Source,
}

/// Who decided an item's look, and where to change it.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Source {
    /// "edit" (your own), "preset" (levelling/farming presets), "price" (price tiers),
    /// "neversink" (his filter), or "none".
    pub kind: String,
    /// Your edit's or preset's name, as listed in My edits.
    pub label: String,
    /// NeverSink's section and tier, e.g. ["currency", "stackedthree"] / "t5".
    pub type_path: Vec<String>,
    pub tier: Option<String>,
    /// The same in the app's words, e.g. "Stacks of 3+" / "Tier 5", when it's in the Tiers tab.
    pub section: Option<String>,
    pub tier_name: Option<String>,
    /// That rule's conditions in plain words ("item level 86+", "not corrupted"), as the Tiers page shows them.
    #[serde(default)]
    pub conds: Vec<String>,
    /// Your edit or preset names just one item (`BaseType == "Omen of Return"`): F2 says it covers only that
    /// item, not the rest of its group (he asked 24 Nov).
    #[serde(default)]
    pub only_base: Option<String>,
}

/// The one base type a rule names, if it names exactly one.
fn only_base(b: &Block) -> Option<String> {
    let line = b.lines.iter().map(|l| l.trim()).find(|l| l.starts_with("BaseType"))?;
    let names: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
    (names.len() == 1).then(|| names[0].to_string())
}

fn source_of(b: &Block) -> Source {
    let label = b.header.trim_end().split_once(" # ").map(|(_, l)| l.trim().to_string()).unwrap_or_default();
    if b.type_path.is_empty() {
        let kind = if label.starts_with("Preset: ") { "preset" }
            else if label.starts_with("Price") { "price" }
            else if ["Hide: ", "Highlight: ", "Louder: ", "Quieter: ", "Style: "].iter().any(|p| label.starts_with(p)) { "edit" }
            else { "none" };
        // A worth line ("Price: Divination cards worth under 4c") says which section's, so its page can be opened.
        let section = label.strip_prefix("Price: ").and_then(|l| l.split_once(" worth ")).map(|(s, _)| s.to_string());
        return Source { kind: kind.into(), only_base: only_base(b), section, label, ..Default::default() };
    }
    Source {
        kind: "neversink".into(),
        section: crate::tiers::section_title(&b.type_path),
        tier_name: b.tier.as_deref().map(|t| crate::tiers::step_name(&b.type_path.join("->"), t)),
        conds: crate::conds::conds_of(&b.lines).into_iter().filter(|c| c.key != "Class" && c.key != "HasExplicitMod").map(|c| c.words).collect(),
        type_path: b.type_path.clone(),
        tier: b.tier.clone(),
        label: String::new(),
        only_base: None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StackNote { pub from: u32, pub visibility: String }

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub verdict: Verdict,
    /// Set when the outcome changes with stack size: [(1, Hide), (6, Show)]
    pub by_stack: Vec<StackNote>,
    /// Things we assumed, e.g. "item level 83 (ground item)".
    pub assumptions: Vec<String>,
}

/// What we know about the item, plus assumptions for unknowns.
#[derive(Debug, Clone)]
pub struct Ctx<'a> {
    pub item: &'a Item,
    pub area_level: Option<u32>,
    pub stack: Option<u32>,
    /// Item came from the screen: booleans like Corrupted aren't known.
    pub from_screen: bool,
}

const RARITIES: [&str; 4] = ["Normal", "Magic", "Rare", "Unique"];

/// Splits `Key op values` and drops trailing comments outside quotes.
fn split_condition(line: &str) -> Option<(&str, &str, Vec<String>)> {
    let line = strip_comment(line.trim());
    if line.is_empty() { return None; }
    let (key, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
    let rest = rest.trim();
    let ops = ["==", "!=", "<=", ">=", "=", "<", ">"];
    let (op, vals) = ops.iter()
        .find_map(|o| rest.strip_prefix(o).map(|v| (*o, v.trim())))
        .unwrap_or(("", rest));
    Some((key, op, values(vals)))
}

fn strip_comment(s: &str) -> &str {
    let mut in_q = false;
    for (i, c) in s.char_indices() {
        match c { '"' => in_q = !in_q, '#' if !in_q => return s[..i].trim_end(), _ => {} }
    }
    s
}

fn values(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    for c in s.chars() {
        match c {
            '"' => { in_q = !in_q; if !in_q { out.push(std::mem::take(&mut cur)); } }
            c if c.is_whitespace() && !in_q => { if !cur.is_empty() { out.push(std::mem::take(&mut cur)); } }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn cmp_num(have: Option<u32>, op: &str, vals: &[String]) -> Tri {
    let Some(h) = have else { return Tri::Unknown };
    let nums: Vec<u32> = vals.iter().filter_map(|v| v.parse().ok()).collect();
    let Some(&v) = nums.first() else { return Tri::Unknown };
    let ok = match op {
        "<" => h < v, "<=" => h <= v, ">" => h > v, ">=" => h >= v,
        "!=" => !nums.contains(&h),
        _ => nums.contains(&h),
    };
    if ok { Tri::Yes } else { Tri::No }
}

fn cmp_text(have: &str, op: &str, vals: &[String]) -> Tri {
    let hit = vals.iter().any(|v| if op == "==" { have == v } else { have.contains(v.as_str()) });
    let ok = if op == "!=" { !hit } else { hit };
    if ok { Tri::Yes } else { Tri::No }
}

fn cmp_bool(have: Option<bool>, vals: &[String]) -> Tri {
    let Some(h) = have else { return Tri::Unknown };
    match vals.first().map(|v| v.eq_ignore_ascii_case("true")) {
        Some(want) if want == h => Tri::Yes,
        Some(_) => Tri::No,
        None => Tri::Unknown,
    }
}

fn check(key: &str, op: &str, vals: &[String], c: &Ctx) -> Tri {
    let it = c.item;
    let gear_like = RARITIES.contains(&it.rarity.as_str());
    // Off the ground we can't see gear details; everything else simply has none.
    let c = &Ctx { from_screen: c.from_screen && (is_gear(it) || it.item_class.is_empty()), ..c.clone() };
    match key {
        "Class" => if it.item_class.is_empty() { Tri::Unknown } else { cmp_text(&it.item_class, op, vals) },
        "BaseType" => cmp_text(&it.base_type, op, vals),
        // We know which defences a base has (not the exact numbers): 1 if it has it, 0 if not.
        "BaseArmour" | "BaseEvasion" | "BaseEnergyShield" | "BaseWard" => {
            let part = match key { "BaseArmour" => "ar", "BaseEvasion" => "ev", "BaseEnergyShield" => "es", _ => "ward" };
            match crate::item::defences().get(&it.base_type) {
                Some(d) => cmp_num(Some(d.split('_').any(|x| x == part) as u32), op, vals),
                None if ["Body Armours", "Boots", "Gloves", "Helmets", "Shields"].contains(&it.item_class.as_str()) => Tri::Unknown,
                None => cmp_num(Some(0), op, vals),
            }
        }
        "Rarity" => {
            let Some(rank) = RARITIES.iter().position(|r| *r == it.rarity) else {
                // Currency, gems etc. are "Normal" to the filter.
                return cmp_rarity(0, op, vals);
            };
            cmp_rarity(rank, op, vals)
        }
        "ItemLevel" => cmp_num(it.item_level.or(c.area_level), op, vals),
        "AreaLevel" => cmp_num(c.area_level, op, vals),
        "StackSize" => cmp_num(c.stack.or(it.stack_size).or(Some(1)), op, vals),
        "Quality" => cmp_num(if c.from_screen { None } else { Some(it.quality.unwrap_or(0)) }, op, vals),
        "MapTier" | "WaypointTier" => cmp_num(it.map_tier, op, vals),
        // Read from the ground we can't see sockets: assume none unless the label's look says it's linked.
        "LinkedSockets" => cmp_num(Some(it.sockets.as_deref().map(max_links).unwrap_or(0)), op, vals),
        "Sockets" => match (&it.sockets, c.from_screen) {
            (_, true) => Tri::Unknown,
            _ if vals.iter().any(|v| v.chars().any(|ch| ch.is_alphabetic())) => Tri::Unknown,
            (Some(s), _) => cmp_num(Some(s.chars().filter(|ch| ch.is_alphabetic()).count() as u32), op, vals),
            (None, _) => cmp_num(Some(0), op, vals),
        },
        "Corrupted" => cmp_bool(if c.from_screen { Some(false) } else { Some(it.corrupted) }, vals),
        "Mirrored" => cmp_bool(if c.from_screen { Some(false) } else { Some(it.mirrored) }, vals),
        "Identified" => cmp_bool(
            if c.from_screen { gear_like.then_some(false) } else { Some(!it.unidentified) }, vals),
        // Maps whose kind we know from their name (label or copied text).
        "BlightedMap" if it.map_kind_known || !c.from_screen => cmp_bool(Some(it.blighted), vals),
        "UberBlightedMap" if it.map_kind_known || !c.from_screen => cmp_bool(Some(it.uber_blighted), vals),
        // Maps' implicits (the conqueror citadels): known from the copied text, or from the label's look.
        "HasImplicitMod" if it.item_class == "Maps" && (it.map_kind_known || !c.from_screen) => cmp_bool(Some(it.has_implicit), vals),
        "ZanaMemory" if it.map_kind_known || !c.from_screen => cmp_bool(Some(it.zana_memory), vals),
        "HasInfluence" if !it.influences.is_empty() => {
            let any = vals.iter().any(|v| it.influences.iter().any(|i| i.eq_ignore_ascii_case(v)));
            if any { Tri::Yes } else { Tri::No }
        }
        "FracturedItem" | "SynthesisedItem" | "Replica" | "Foulborn" | "Scourged" | "IsVaalUnique"
        | "ElderMap" | "ShapedMap" | "BlightedMap" | "UberBlightedMap"
        | "TransfiguredGem" | "ZanaMemory" | "Imbued" =>
            if c.from_screen { Tri::Unknown } else { cmp_bool(Some(false), vals) },
        // Jewels copied from the game: their mod names are known ("HasExplicitMod >=2 \"Shimmering\" \"Vivid\"").
        "HasExplicitMod" if it.mod_names.is_some() && !c.from_screen => {
            let have = it.mod_names.as_deref().unwrap_or(&[]);
            let (need, names) = match vals.first().and_then(|v| v.parse::<u32>().ok()) {
                Some(n) => (n, &vals[1..]),
                None => (1, vals),
            };
            let count = names.iter().filter(|n| have.iter().any(|h| h.eq_ignore_ascii_case(n))).count() as u32;
            let op = if op.is_empty() || op == "=" || op == "==" { if vals.first().is_some_and(|v| v.parse::<u32>().is_ok()) { op } else { ">=" } } else { op };
            cmp_num(Some(count), if op.is_empty() { "==" } else { op }, &[need.to_string()])
        }
        "UnidentifiedItemTier" if it.item_tier.is_some() => cmp_num(it.item_tier, op, vals),
        "MemoryStrands" if !c.from_screen => cmp_num(Some(it.memory_strands.unwrap_or(0)), op, vals),
        // Things only gear can have: rule them out for currency, gems, cards...
        "HasExplicitMod" | "UnidentifiedItemTier" | "MemoryStrands" | "HasInfluence" | "HasEaterOfWorldsImplicit"
        | "HasSearingExarchImplicit" | "EnchantmentPassiveNum" | "HasEnchantment" | "SocketGroup"
        | "HasCruciblePassiveTree" | "HasImplicitMod" | "AnyEnchantment" | "CorruptedMods"
            if !is_gear(it) && !it.item_class.is_empty() => absent(key, op, vals),
        _ => Tri::Unknown,
    }
}

/// The item has none of this. "HasExplicitMod >= 2 ..." -> No, "HasInfluence None" -> Yes.
fn absent(key: &str, op: &str, vals: &[String]) -> Tri {
    if vals.first().map(|v| v.eq_ignore_ascii_case("false")).unwrap_or(false) { return Tri::Yes; }
    if vals.first().map(|v| v.eq_ignore_ascii_case("true")).unwrap_or(false) { return Tri::No; }
    if key == "HasInfluence" {
        return if vals.iter().any(|v| v == "None") { Tri::Yes } else { Tri::No };
    }
    let first_num = vals.first().and_then(|v| v.parse::<u32>().ok());
    match (op, first_num) {
        ("<" | "<=", Some(_)) => Tri::Yes,
        ("=" | "==", Some(0)) | (">=", Some(0)) => Tri::Yes,
        _ => Tri::No,
    }
}

fn cmp_rarity(rank: usize, op: &str, vals: &[String]) -> Tri {
    let ranks: Vec<usize> = vals.iter().filter_map(|v| RARITIES.iter().position(|r| r == v)).collect();
    let Some(&v) = ranks.first() else { return Tri::Unknown };
    let ok = match op {
        "<" => rank < v, "<=" => rank <= v, ">" => rank > v, ">=" => rank >= v,
        "!=" => !ranks.contains(&rank), _ => ranks.contains(&rank),
    };
    if ok { Tri::Yes } else { Tri::No }
}

fn colour(vals: &[String]) -> Option<[u8; 4]> {
    let n: Vec<u8> = vals.iter().filter_map(|v| v.parse().ok()).collect();
    (n.len() >= 3).then(|| [n[0], n[1], n[2], *n.get(3).unwrap_or(&255)])
}

fn apply_actions(b: &Block, s: &mut Style) -> bool {
    apply_action_lines(&b.lines, s)
}

/// The look a list of filter action lines gives (e.g. a saved edit).
pub fn style_of(lines: &[String]) -> Style {
    let mut s = Style::default();
    apply_action_lines(lines, &mut s);
    s.label_hidden = label_invisible(&s);
    s
}

fn label_invisible(s: &Style) -> bool {
    s.font_size == Some(1) && s.text.map(|t| t[3] == 0).unwrap_or(false)
        && s.background.map(|b| b[3] == 0).unwrap_or(true)
}

fn apply_action_lines(lines: &[String], s: &mut Style) -> bool {
    let mut cont = false;
    for line in lines {
        let Some((key, _, v)) = split_condition(line) else { continue };
        match key {
            "SetFontSize" => s.font_size = v.first().and_then(|x| x.parse().ok()),
            "SetTextColor" => s.text = colour(&v),
            "SetBorderColor" => s.border = colour(&v),
            "SetBackgroundColor" => s.background = colour(&v),
            "PlayAlertSound" | "PlayAlertSoundPositional" => {
                s.sound = v.first().map(|x| format!("Sound {x}"));
                s.sound_id = v.first().cloned();
                s.sound_file = None;
                s.volume = v.get(1).and_then(|x| x.parse().ok());
                s.muted = false;
            }
            "CustomAlertSound" | "CustomAlertSoundOptional" => {
                s.sound = v.first().map(|x| x.rsplit(['/', '\\']).next().unwrap_or(x).to_string());
                s.sound_file = v.first().cloned();
                s.sound_id = None;
                s.volume = v.get(1).and_then(|x| x.parse().ok());
                s.muted = false;
            }
            "DisableDropSound" if v.first().map(|x| !x.eq_ignore_ascii_case("false")).unwrap_or(true) => {
                s.muted = true; s.sound = None; s.sound_id = None; s.sound_file = None;
            }
            "EnableDropSound" => s.muted = false,
            "MinimapIcon" if v.first().map(|x| x == "-1").unwrap_or(false) => { s.minimap = None; s.minimap_parts = None; }
            "MinimapIcon" if v.first().map(|x| x != "-1").unwrap_or(false) => {
                s.minimap_parts = Some((v[0].parse().unwrap_or(2), v.get(1).cloned().unwrap_or_default(), v.get(2).cloned().unwrap_or_default()));
                let size = ["Large", "Medium", "Small"].get(v[0].parse::<usize>().unwrap_or(2)).unwrap_or(&"Small");
                s.minimap = Some(format!("{size} {} {}", v.get(1).cloned().unwrap_or_default(), v.get(2).cloned().unwrap_or_default()).trim().to_string());
            }
            "PlayEffect" if v.first().map(|x| x == "None").unwrap_or(false) => { s.beam = None; s.beam_temp = false; }
            "PlayEffect" => { s.beam = v.first().cloned(); s.beam_temp = v.get(1).map(|x| x == "Temp").unwrap_or(false); }
            "Continue" => cont = true,
            _ => {}
        }
    }
    cont
}

fn describe(b: &Block) -> (String, bool) {
    let content = b.header.trim_end();
    if let Some((_, label)) = content.split_once(" # ").filter(|_| b.type_path.is_empty()) {
        let label = label.trim();
        if ["Hide: ", "Highlight: ", "Louder: ", "Quieter: ", "Style: "].iter().any(|p| label.starts_with(p)) {
            return (label.to_string(), true);
        }
        if let Some(rest) = label.strip_prefix("Preset: ") {
            return (rest.to_string(), true);
        }
        if label.starts_with("Price") {
            return (label.to_string(), false);
        }
    }
    if b.type_path.is_empty() {
        return ("an untagged rule".into(), false);
    }
    let mut s = b.type_path.join(" › ");
    if let Some(t) = &b.tier { s.push_str(&format!(" › {t}")); }
    (s, false)
}

const CONDITION_KEYS_ARE_NOT_ACTIONS: [&str; 10] = [
    "SetFontSize", "SetTextColor", "SetBorderColor", "SetBackgroundColor", "PlayAlertSound",
    "PlayAlertSoundPositional", "CustomAlertSound", "CustomAlertSoundOptional", "MinimapIcon", "PlayEffect",
];
const OTHER_ACTIONS: [&str; 4] = ["DisableDropSound", "EnableDropSound", "Continue", "DisableDropSoundIfAlertSound"];

pub fn evaluate(segs: &[Segment], c: &Ctx) -> Verdict {
    let mut style = Style::default();
    let mut uncertain = Vec::new();
    for s in segs {
        let Segment::Block(b) = s else { continue };
        let mut result = Tri::Yes;
        let mut unknown_keys = Vec::new();
        for line in &b.lines {
            let Some((key, op, vals)) = split_condition(line) else { continue };
            if CONDITION_KEYS_ARE_NOT_ACTIONS.contains(&key) || OTHER_ACTIONS.contains(&key) { continue; }
            match check(key, op, &vals, c) {
                Tri::No => { result = Tri::No; break; }
                Tri::Unknown => { result = Tri::Unknown; unknown_keys.push(key.to_string()); }
                Tri::Yes => {}
            }
        }
        match result {
            Tri::No => continue,
            Tri::Unknown => {
                if uncertain.len() < 5 {
                    let (d, _) = describe(b);
                    unknown_keys.dedup();
                    uncertain.push(format!("{d} (if {})", unknown_keys.join(", ")));
                }
            }
            Tri::Yes => {
                let cont = apply_actions(b, &mut style);
                style.label_hidden = label_invisible(&style);
                if !cont {
                    let (decided_by, by_user_edit) = describe(b);
                    let reason = if by_user_edit || b.type_path.is_empty() { decided_by.clone() } else { crate::explain::explain(b) };
                    return Verdict { visibility: b.visibility.clone(), style, decided_by, by_user_edit, reason, uncertain, source: source_of(b) };
                }
            }
        }
    }
    Verdict { visibility: "None".into(), style, decided_by: "no rule matched".into(), by_user_edit: false,
        reason: "Not covered by any rule (the game shows it normally)".into(), uncertain, source: Source { kind: "none".into(), ..Default::default() } }
}

/// Full status, including whether stack size changes the outcome.
pub fn status(segs: &[Segment], item: &Item, known_area_level: Option<u32>, from_screen: bool) -> Status {
    status_with_fallback(segs, item, known_area_level, 83, from_screen)
}

/// Like `status`, assuming `fallback` as the area level when it isn't known.
pub fn status_with_fallback(segs: &[Segment], item: &Item, known_area_level: Option<u32>, fallback: u32, from_screen: bool) -> Status {
    let mut assumptions = Vec::new();
    let area_level = known_area_level.unwrap_or(fallback);
    if known_area_level.is_none() {
        assumptions.push(if fallback >= 68 { format!("you're in a level {fallback} map") } else { format!("area level {fallback}") });
    }
    if item.item_level.is_none() && RARITIES.contains(&item.rarity.as_str()) {
        assumptions.push(format!("item level {area_level}"));
    }
    if from_screen && is_gear(item) {
        assumptions.push(match item.sockets.as_deref().map(max_links) {
            Some(n) if n >= 3 => format!("not corrupted, unidentified, {n}-linked (from its look)"),
            _ => "not corrupted, unidentified, no linked sockets".into(),
        });
    }
    let ctx = Ctx { item, area_level: Some(area_level), stack: None, from_screen };
    let verdict = evaluate(segs, &ctx);

    // Look for StackSize thresholds on blocks that mention this base.
    let mut points: Vec<u32> = vec![1];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if !b.lines.iter().any(|l| l.contains("StackSize")) { continue; }
        let names_base = b.base_types().iter().any(|x| x == &item.base_type || item.base_type.contains(x.as_str()));
        if !names_base { continue; }
        for l in &b.lines {
            if let Some(("StackSize", op, v)) = split_condition(l) {
                if let Some(n) = v.first().and_then(|x| x.parse::<u32>().ok()) {
                    points.push(match op { ">" => n + 1, "<" => n, "<=" => n + 1, _ => n });
                }
            }
        }
    }
    points.sort();
    points.dedup();
    let mut by_stack: Vec<StackNote> = Vec::new();
    if points.len() > 1 {
        for p in points {
            let v = evaluate(segs, &Ctx { stack: Some(p), ..ctx.clone() });
            if by_stack.last().map(|l| l.visibility != v.visibility).unwrap_or(true) {
                by_stack.push(StackNote { from: p, visibility: v.visibility });
            }
        }
        if by_stack.len() < 2 { by_stack.clear(); }
    }
    Status { verdict, by_stack, assumptions }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::parse_filter;
    use crate::item::parse_item;

    const F: &str = "Show # $type->currency $tier->t1\n    BaseType == \"Divine Orb\"\n    SetFontSize 45\n    SetTextColor 255 0 0 255\n    PlayAlertSound 1 300\n\nHide # $type->currency $tier->wisdom\n    StackSize < 6\n    BaseType == \"Scroll of Wisdom\"\n\nShow # $type->currency $tier->wisdomstack\n    BaseType == \"Scroll of Wisdom\"\n    SetFontSize 30\n\nShow # $type->rare $tier->mods\n    HasExplicitMod \"of Haast\"\n    Rarity Rare\n    SetFontSize 40\n\nHide # $type->rare $tier->rest\n    Rarity Rare\n";

    fn item(t: &str) -> Item { parse_item(t).unwrap() }

    #[test]
    fn shown_with_style() {
        let s = status(&parse_filter(F), &item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n"), Some(83), false);
        assert_eq!(s.verdict.visibility, "Show");
        assert_eq!(s.verdict.style.font_size, Some(45));
        assert_eq!(s.verdict.style.text, Some([255, 0, 0, 255]));
        assert_eq!(s.verdict.style.sound.as_deref(), Some("Sound 1"));
        assert_eq!(s.verdict.decided_by, "currency › t1");
    }

    #[test]
    fn jewels_with_my_mods() {
        // 24 Nov: "highlight jewels if they have certain mods". Rule from the app, then NeverSink hiding magic jewels.
        let one = crate::rules::quick_rule(&Item { item_class: "Jewels".into(), rarity: "Magic".into(), ..Default::default() }, crate::rules::Action::Highlight, None,
            &crate::rules::MatchOpts { same_rarity: false, mods: vec!["Shimmering".into(), "Vivid".into()], ..Default::default() });
        let crate::rules::Rule::Override { label, conditions, .. } = &one else { panic!() };
        assert_eq!(label, "Highlight: every Jewels with my mods");
        assert!(conditions.contains(&"HasExplicitMod \"Shimmering\" \"Vivid\"".to_string()), "{conditions:?}");
        let two = crate::rules::quick_rule(&Item { item_class: "Jewels".into(), ..Default::default() }, crate::rules::Action::Highlight, None,
            &crate::rules::MatchOpts { same_rarity: false, mods: vec!["Shimmering".into(), "Vivid".into()], mods_min: 2, ..Default::default() });
        let crate::rules::Rule::Override { conditions: c2, .. } = &two else { panic!() };
        assert!(c2.contains(&"HasExplicitMod >=2 \"Shimmering\" \"Vivid\"".to_string()));
        let filter = |conds: &[String]| format!("Show # Highlight: x\n    {}\n    SetFontSize 42\n\nHide # $type->jewels $tier->rest\n    Class == \"Jewels\"\n", conds.join("\n    "));
        let jewel = |mods: &[&str]| { let mut i = item("Item Class: Jewels\nRarity: Magic\nShimmering Cobalt Jewel\n"); i.mod_names = Some(mods.iter().map(|s| s.to_string()).collect()); i };
        let vis = |f: &str, it: &Item| status(&parse_filter(f), it, Some(83), false).verdict.visibility;
        assert_eq!(vis(&filter(conditions), &jewel(&["Shimmering"])), "Show");
        assert_eq!(vis(&filter(conditions), &jewel(&["of Haste"])), "Hide");
        assert_eq!(vis(&filter(c2), &jewel(&["Shimmering"])), "Hide");
        assert_eq!(vis(&filter(c2), &jewel(&["Shimmering", "Vivid"])), "Show");
        // Kept when the edit's look changes; its detail lists them.
        let d = crate::rules::rule_detail(&two).unwrap();
        assert_eq!((d.mods.len(), d.mods_min), (2, 2));
    }

    #[test]
    fn stack_size_changes_outcome() {
        let s = status(&parse_filter(F), &item("Item Class: Stackable Currency\nRarity: Currency\nScroll of Wisdom\n--------\nStack Size: 2/40\n"), Some(83), false);
        assert_eq!(s.verdict.visibility, "Hide");
        let v: Vec<(u32, &str)> = s.by_stack.iter().map(|n| (n.from, n.visibility.as_str())).collect();
        assert_eq!(v, vec![(1, "Hide"), (6, "Show")]);
    }

    #[test]
    fn unknown_mods_are_reported_not_guessed() {
        let s = status(&parse_filter(F), &item("Item Class: Rings\nRarity: Rare\nDoom Loop\nCoral Ring\n"), Some(83), false);
        assert_eq!(s.verdict.visibility, "Hide");
        assert_eq!(s.verdict.uncertain.len(), 1);
        assert!(s.verdict.uncertain[0].contains("HasExplicitMod"));
    }

    #[test]
    fn user_edit_label() {
        let f = format!("Hide # Hide: Divine Orb\n    BaseType == \"Divine Orb\"\n\n{F}");
        let s = status(&parse_filter(&f), &item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n"), Some(83), false);
        assert_eq!(s.verdict.visibility, "Hide");
        assert!(s.verdict.by_user_edit);
        assert_eq!(s.verdict.decided_by, "Hide: Divine Orb");
    }
}
