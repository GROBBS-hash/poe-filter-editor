//! NeverSink's tier lists (currency, div cards, uniques...) for the Tiers tab:
//! like FilterBlade, each section is a stack of tiers you can show/hide,
//! restyle, and drag items between.

use crate::evaluate::{style_of, Style};
use crate::filter::Segment;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TierRow {
    pub tier: String,
    /// "Tier 1", "Tier 2 (Divine)", "Exotic: hidden"...
    pub name: String,
    pub visibility: String,
    pub style: Style,
    pub bases: Vec<String>,
    /// Extra conditions in plain words, e.g. "stacks of 3+".
    pub note: String,
    /// "StackSize >= N" tiers (stacks, gold): N, so the app can offer a slider.
    pub min_stack: Option<u32>,
    /// "UnidentifiedItemTier >= N" (3.30 item tiers on rare gear).
    #[serde(default)]
    pub min_item_tier: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    /// e.g. ["currency", "stackedthree"].
    pub type_path: Vec<String>,
    pub title: String,
    pub group: String,
    pub tiers: Vec<TierRow>,
    /// The same base sits in several tiers (a grid, like rare gear by item tier):
    /// dragging would pull it out of the others, so the app doesn't offer it.
    #[serde(default)]
    pub shared_bases: bool,
}

/// Known sections: (type path, group, title). Order here is the order shown.
const KNOWN: &[(&str, &str, &str)] = &[
    ("currency", "Currency", "Currency"),
    ("currency->stackedthree", "Currency", "Stacks of 3+"),
    ("currency->stackedsix", "Currency", "Stacks of 6+"),
    ("currency->stackedsupplieswisdom", "Currency", "Wisdom scroll stacks"),
    ("currency->stackedsuppliesportal", "Currency", "Portal scroll stacks"),
    ("currency->stackedsupplieshigh", "Currency", "Transmutation stacks"),
    ("currency->stackedsupplieslow", "Currency", "Augmentation stacks"),
    ("currency->leveling", "Currency", "Campaign currency"),
    ("currency->levelingstacked", "Currency", "Campaign currency stacks"),
    ("divination", "Divination cards", "Divination cards"),
    ("uniques", "Uniques", "Uniques"),
    ("uniques->replicas", "Uniques", "Replica uniques"),
    ("uniques->foulborn", "Uniques", "Foulborn uniques"),
    ("fragments", "Fragments", "Fragments"),
    ("currency->splinter->simulacrum", "Fragments", "Simulacrum splinters"),
    ("currency->stackedsplintershigh", "Fragments", "Splinter stacks (high)"),
    ("currency->stackedsplinterslow", "Fragments", "Splinter stacks (low)"),
    ("currency->essence", "League currency", "Essences"),
    ("currency->fossil", "League currency", "Fossils"),
    ("currency->oil", "League currency", "Oils"),
    ("currency->deliriumorbs", "League currency", "Delirium orbs"),
    ("currency->trial->omen", "League currency", "Omens"),
    ("currency->trial->tattoo", "League currency", "Tattoos"),
    ("currency->harvest", "League currency", "Lifeforce"),
    ("currency->deadmansulphur", "League currency", "Sulphur"),
    ("runesgrafts", "League currency", "Runegrafts"),
    ("corpses", "League currency", "Corpses"),
    ("viscera", "League currency", "Viscera"),
    ("gold", "Currency", "Gold"),
    ("gear->tiered", "Gear", "Rare gear by item tier"),
    ("exotic->fractured", "Gear", "Fractured bases"),
    ("rare->exotic->veiled", "Gear", "Veiled items"),
    ("exoticbases", "Gear", "Exotic bases"),
];

/// "t2divine" -> "Tier 2 (divine)", "exhide" -> "Hidden extras".
pub fn tier_name(t: &str) -> String {
    if let Some(n) = map_rule_name(t) { return n; }
    // "t5r1": item tier 5 and up, on the best (rank 1) bases.
    if let Some(rest) = t.strip_prefix('t') {
        if let Some((a, b)) = rest.split_once('r') {
            if let (Ok(tier), Ok(rank)) = (a.parse::<u32>(), b.parse::<u32>()) {
                let r = match rank { 1 => "top bases", 2 => "good bases", 3 => "decent bases", _ => "other bases" };
                return format!("Item tier {tier}+ · {r}");
            }
        }
    }
    match t {
        "strandedfractured" => return "Fractured, memory-stranded".into(),
        "strandedveiled" => return "Veiled, memory-stranded".into(),
        "exhide" => return "Hidden extras".into(),
        "tportal" => return "Portal scrolls".into(),
        "twisdom" => return "Wisdom scrolls".into(),
        "scrollfragments" => return "Scroll fragments".into(),
        "anyother" | "any" => return "Any other".into(),
        _ => {}
    }
    if let Some(rest) = t.strip_prefix('t') {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let tail = rest[digits.len()..].trim_start_matches('_');
            return if tail.is_empty() { format!("Tier {digits}") } else { format!("Tier {digits} ({tail})") };
        }
    }
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// The Tiers tab's name for a NeverSink section, when it lists it.
pub fn section_title(type_path: &[String]) -> Option<String> {
    let key = type_path.join("->");
    KNOWN.iter().find(|k| k.0 == key).map(|k| k.2.to_string())
}

pub fn gold_name(t: &str) -> String {
    match t {
        "stack3" => "Huge piles".into(), "stack2" => "Large piles".into(), "stack1" => "Medium piles".into(),
        "stacklvl1" => "Small piles (levelling only)".into(), "anyother" => "Any other gold".into(), _ => tier_name(t),
    }
}

fn note_of(lines: &[String]) -> String {
    let mut out = vec![];
    for l in lines {
        let t = l.trim();
        if let Some(v) = t.strip_prefix("StackSize") {
            let v = v.trim();
            if let Some(n) = v.strip_prefix(">=") { out.push(format!("stacks of {}+", n.trim())); }
            else if let Some(n) = v.strip_prefix('>') { out.push(format!("stacks over {}", n.trim())); }
            else if let Some(n) = v.strip_prefix('<') { out.push(format!("stacks under {}", n.trim_start_matches('=').trim())); }
        } else if let Some(v) = t.strip_prefix("ItemLevel") {
            let v = v.trim();
            out.push(match v.strip_prefix(">=") { Some(n) => format!("item level {}+", n.trim()), None => format!("item level {v}") });
        } else if let Some(v) = t.strip_prefix("AreaLevel") {
            out.push(format!("area level {}", v.trim()));
        }
    }
    out.join(", ")
}

/// Every tier-listed section in a filter: known ones first in a fixed order,
/// or (when none are known, e.g. PoE 2) any section with 3+ base-listed tiers.
pub fn sections(segs: &[Segment]) -> Vec<Section> {
    let mut found: Vec<Section> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        let Some(tier) = b.tier.clone() else { continue };
        let bases = b.base_types();
        if bases.is_empty() || b.type_path.is_empty() { continue; }
        let key = b.type_path.join("->");
        let i = match found.iter().position(|x| x.type_path == b.type_path) {
            Some(i) => i,
            None => {
                let (group, title) = KNOWN.iter().find(|k| k.0 == key).map(|k| (k.1.to_string(), k.2.to_string()))
                    .unwrap_or_else(|| ("Other".into(), b.type_path.iter().map(|p| tier_name(p)).collect::<Vec<_>>().join(" › ")));
                found.push(Section { type_path: b.type_path.clone(), title, group, tiers: vec![], shared_bases: false });
                found.len() - 1
            }
        };
        // The same tier can appear twice (e.g. a campaign copy); keep the first.
        if found[i].tiers.iter().any(|t| t.tier == tier) { continue; }
        found[i].tiers.push(TierRow {
            name: if key == "gold" { gold_name(&tier) } else { tier_name(&tier) },
            tier, visibility: b.visibility.clone(), style: style_of(&b.lines), bases, note: note_of(&b.lines),
            min_stack: b.lines.iter().find_map(|l| l.trim().strip_prefix("StackSize >=").and_then(|n| n.trim().parse().ok())),
            min_item_tier: b.lines.iter().find_map(|l| l.trim().strip_prefix("UnidentifiedItemTier >=").and_then(|n| n.trim().parse().ok())),
        });
    }
    for s in found.iter_mut() {
        let mut seen = std::collections::HashSet::new();
        s.shared_bases = s.tiers.iter().flat_map(|t| t.bases.iter()).any(|b| !seen.insert(b.clone()));
    }
    let rank = |s: &Section| KNOWN.iter().position(|k| k.0 == s.type_path.join("->")).unwrap_or(usize::MAX);
    // Unknown sections only when none are known (e.g. PoE 2's different names).
    let any_known = found.iter().any(|s| rank(s) != usize::MAX);
    found.retain(|s| if any_known { rank(s) != usize::MAX } else { s.tiers.len() >= 3 });
    found.sort_by_key(|s| rank(s));
    found
}

/// Any one of NeverSink's rules (all blocks sharing a `$type` path), even ones that
/// aren't tier lists, e.g. "levelling › white & blue › 4-links". Each tier is a row
/// with its conditions in plain words, so "Modify rule" can open it.
pub fn section_at(segs: &[Segment], type_path: &[String]) -> Option<Section> {
    let mut tiers: Vec<TierRow> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if b.type_path != type_path { continue; }
        let Some(tier) = b.tier.clone() else { continue };
        if tiers.iter().any(|t| t.tier == tier) { continue; }
        let mut note = conditions(&b.lines);
        let base_note = note_of(&b.lines);
        if !base_note.is_empty() { note.push(base_note); }
        tiers.push(TierRow {
            name: tier_name(&tier), tier, visibility: b.visibility.clone(), style: style_of(&b.lines),
            bases: b.base_types(), note: note.join(", "),
            min_stack: b.lines.iter().find_map(|l| l.trim().strip_prefix("StackSize >=").and_then(|n| n.trim().parse().ok())),
            min_item_tier: b.lines.iter().find_map(|l| l.trim().strip_prefix("UnidentifiedItemTier >=").and_then(|n| n.trim().parse().ok())),
        });
    }
    if tiers.is_empty() { return None; }
    let title = section_title(type_path).unwrap_or_else(|| type_path.iter().map(|p| plain_part(p)).collect::<Vec<_>>().join(" › "));
    Some(Section { type_path: type_path.to_vec(), title, group: "Opened rule".into(), tiers, shared_bases: true })
}

/// NeverSink's short section names in plain words.
pub fn plain_part(p: &str) -> String {
    match p {
        "leveling" => "Levelling".into(), "normalmagic" => "White & blue".into(), "rare" => "Rares".into(),
        "4l" => "4-links".into(), "3l" => "3-links".into(), "5l" => "5-links".into(), "6l" => "6-links".into(),
        "rgb" => "R-G-B sockets".into(), "exotic" => "Special".into(), "general" => "General".into(),
        "gear" => "Gear".into(), "tiered" => "By item tier".into(), "uniques" => "Uniques".into(),
        "currency" => "Currency".into(), "divination" => "Divination cards".into(), "decorators" => "Highlights".into(),
        _ => {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// The other conditions of a block in plain words: rarity, links, item classes…
fn conditions(lines: &[String]) -> Vec<String> {
    let mut out = vec![];
    let names = |v: &str| -> Vec<String> { v.split('"').map(str::trim).filter(|x| !x.is_empty() && *x != "==" && *x != "=").map(String::from).collect() };
    for l in lines {
        let t = l.trim();
        let (key, rest) = t.split_once(' ').unwrap_or((t, ""));
        let rest = rest.trim();
        match key {
            "Rarity" => {
                let r = rest.trim_start_matches(['=', '<', '>', ' ']).split_whitespace().map(|x| x.to_lowercase()).collect::<Vec<_>>();
                let op = rest.split_whitespace().next().unwrap_or("");
                out.push(match op { "<=" | "<" | ">=" | ">" => format!("rarity {op} {}", r.join(" ")), _ => r.join(" & ") });
            }
            "LinkedSockets" => {
                let n: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
                let more = if rest.starts_with('>') { "+" } else if rest.starts_with('<') { " or fewer" } else { "" };
                out.push(format!("{n}{more} links"));
            }
            "Sockets" | "SocketGroup" => out.push(format!("sockets {}", rest.trim_start_matches(['=', ' ']))),
            "Class" => {
                let c = names(rest);
                out.push(if c.len() <= 3 { c.join(", ") } else { format!("{} item types", c.len()) });
            }
            "Corrupted" => out.push(if rest == "True" { "corrupted".into() } else { "not corrupted".into() }),
            "Identified" => out.push(if rest == "True" { "identified".into() } else { "unidentified".into() }),
            "Quality" => out.push(format!("quality {rest}")),
            "DropLevel" => out.push(format!("drop level {rest}")),
            _ => {}
        }
    }
    out
}

/// One stack section as offered in the in-game panel.
#[derive(Debug, Clone, Serialize)]
pub struct StackChoice {
    pub type_path: Vec<String>,
    pub title: String,
    pub tiers: Vec<TierRow>,
    /// The tier this item is in now, if any.
    pub current: Option<String>,
}

/// Stack sections for a currency: the general ones (3+, 6+) and any that already list it.
pub fn stack_choices(segs: &[Segment], base: &str) -> Vec<StackChoice> {
    sections(segs).into_iter()
        .filter(|s| s.group == "Currency" && s.title != "Gold" && s.tiers.iter().all(|t| t.min_stack.is_some()))
        .filter_map(|s| {
            let current = s.tiers.iter().find(|t| t.bases.iter().any(|b| b == base)).map(|t| t.tier.clone());
            let general = matches!(s.type_path.last().map(String::as_str), Some("stackedthree" | "stackedsix"));
            (general || current.is_some()).then(|| StackChoice { type_path: s.type_path, title: s.title, tiers: s.tiers, current })
        })
        .collect()
}

/// One map tier rule of NeverSink's ("maps_d_t3" → maps of tier 3).
#[derive(Debug, Clone, Serialize)]
pub struct MapTierRow {
    pub map_tier: u32,
    /// NeverSink's band letter: a = T14–16, b = T11–13, c = T6–10, d = T1–5.
    pub band: String,
    #[serde(flatten)]
    pub row: TierRow,
}

/// Maps for the Tiers tab's map panel: the plain tier-by-tier rules and the special kinds
/// (influenced, blighted, Valdo…), which NeverSink checks first.
#[derive(Debug, Clone, Serialize)]
pub struct MapTiers {
    pub tiers: Vec<MapTierRow>,
    pub special: Vec<Section>,
    /// Border highlights NeverSink adds on top (upgrades, implicits…): each can be switched off.
    pub extras: Vec<MapExtra>,
}

/// One of NeverSink's map highlights that only add a border and let the tier rules decide the rest.
#[derive(Debug, Clone, Serialize)]
pub struct MapExtra {
    pub key: String,
    pub title: String,
    /// What it does in game, in one line.
    pub what: String,
    pub tiers: Vec<String>,
    /// Still draws something (not switched off).
    pub on: bool,
    /// The border colour it draws (from NeverSink's rule, or its usual one when switched off), for the preview.
    pub border: [u8; 4],
    /// A map tier it applies to, to show an example label.
    pub sample_tier: u32,
}

/// NeverSink's map rule names in plain words ("maps_a_t16" → "Tier 16 maps").
fn map_rule_name(t: &str) -> Option<String> {
    if let Some((_, n)) = map_tier_of(t) { return Some(format!("Tier {n} maps")); }
    if let Some(n) = t.strip_prefix("deco_mapup_t") { return Some(format!("Upgrade border: tier {n}+ maps while you run lower")); }
    if let Some(n) = t.strip_prefix("deco_zone").and_then(|r| r.strip_suffix("general")) {
        return Some(match n { "1" => "Border: tier 14–16 maps", "2" => "Border: tier 11–13 maps", "3" => "Border: tier 6–10 maps", _ => "Border: tier 1–5 maps" }.into());
    }
    Some(match t {
        "decor_implicit" => "Gold border: maps with implicit mods",
        "deco_corruptedmod" => "Gold border: maps with corrupted mods",
        "zanamap" => "Zana / Originator maps (best)", "zanamaplow" => "Zana / Originator maps (others)",
        "zanacorrupthider" => "Corrupted Zana maps (hidden)",
        "infelder" => "Shaper / Elder influenced maps", "infconquerors" => "Conqueror influenced maps",
        "valdomaps" => "Valdo's maps", "nightmaremaps" => "Nightmare maps", "shaperguardianmaps" => "Shaper guardian maps",
        "exoticmod1" => "Maps with the best special mods", "exoticmod2" => "Maps with good special mods", "mod8" => "Maps with 8 mods",
        "uber" => "Blight-ravaged maps",
        "corruptedmaphider" => "Corrupted white/blue maps", "mirroredmaphider" => "Mirrored white/blue maps",
        _ => return None,
    }.into())
}

/// "maps_a_t16" → ("a", 16).
fn map_tier_of(tier: &str) -> Option<(String, u32)> {
    let rest = tier.strip_prefix("maps_")?;
    let (band, t) = rest.split_once("_t")?;
    Some((band.to_string(), t.parse().ok()?))
}

fn map_kind(p: &str) -> String {
    match p {
        "influenced" => "Influenced maps (Shaper, Elder, Conquerors, Zana)".into(),
        "nightmare" => "Valdo's, nightmare & guardian maps".into(),
        "blighted" => "Blighted maps".into(),
        "vaaltemple" => "Vaal Temple maps".into(),
        "specialmods" => "Maps with special mods (8 mods, rare boss mods)".into(),
        "enchanted" => "Enchanted maps".into(),
        "corruptedimplicit" => "Maps with corrupted implicits".into(),
        "implicitmod" => "Maps with implicits".into(),
        _ => plain_part(p),
    }
}

pub fn map_tiers(segs: &[Segment]) -> MapTiers {
    let maps = ["maps".to_string()];
    let mut tiers: Vec<MapTierRow> = section_at(segs, &maps).map(|s| s.tiers).unwrap_or_default().into_iter()
        .filter_map(|row| map_tier_of(&row.tier).map(|(band, map_tier)| MapTierRow { map_tier, band, row }))
        .collect();
    tiers.sort_by_key(|t| t.map_tier);
    let mut paths: Vec<Vec<String>> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if b.type_path.len() >= 2 && b.type_path[0] == "maps" && !paths.contains(&b.type_path) { paths.push(b.type_path.clone()); }
    }
    let special = paths.iter().filter_map(|p| section_at(segs, p).map(|mut s| {
        s.title = map_kind(&p[1..].join("->"));
        s.group = "Maps".into();
        s
    })).collect();
    // Highlights: blocks under "maps" that aren't tier rules, grouped by what they do.
    let groups: [(&str, &str, &str, fn(&str) -> bool); 4] = [
        ("upgrade", "Upgrade borders", "A red border on maps above the tier you're running now, so upgrades stand out.", |t| t.starts_with("deco_mapup_")),
        ("bands", "Tier group borders", "A dark border on red maps and a light one on lower maps.", |t| t.starts_with("deco_zone")),
        ("implicit", "Implicit borders", "A gold border on maps with implicit mods (e.g. from the atlas).", |t| t == "decor_implicit"),
        ("corrupted", "Corrupted mod borders", "A gold border on maps with corrupted mods.", |t| t == "deco_corruptedmod"),
    ];
    let mut extras = vec![];
    for (key, title, what, test) in groups {
        let mut names = vec![];
        let mut on = false;
        let mut border: Option<[u8; 4]> = None;
        for s in segs {
            let Segment::Block(b) = s else { continue };
            let Some(t) = b.tier.as_deref() else { continue };
            if b.type_path != maps || !test(t) || names.iter().any(|n| n == t) { continue; }
            names.push(t.to_string());
            if border.is_none() {
                border = b.lines.iter().find_map(|l| l.trim().strip_prefix("SetBorderColor").map(|v| {
                    let n: Vec<u8> = v.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    [n.first().copied().unwrap_or(0), n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0), n.get(3).copied().unwrap_or(255)]
                }));
            }
            on |= b.lines.iter().any(|l| { let l = l.trim_start(); l.starts_with("Set") || l.starts_with("Play") || l.starts_with("MinimapIcon") });
        }
        if !names.is_empty() { let (usual, sample_tier) = match key { "upgrade" => ([220, 50, 0, 255], 14), "bands" => ([0, 0, 0, 255], 14), _ => ([255, 180, 0, 255], 10) };
            extras.push(MapExtra { key: key.into(), title: title.into(), what: what.into(), tiers: names, on, border: border.unwrap_or(usual), sample_tier }); }
    }
    MapTiers { tiers, special, extras }
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_tier_rules() {
        let segs = crate::filter::parse_filter(&fixture());
        let m = super::map_tiers(&segs);
        assert_eq!(m.tiers.iter().map(|t| t.map_tier).collect::<Vec<_>>(), (1..=16).collect::<Vec<_>>());
        assert_eq!(m.tiers[15].band, "a");
        assert_eq!(m.tiers[0].band, "d");
        assert!(m.special.iter().any(|s| s.title == "Blighted maps"));
        assert_eq!(m.tiers[15].row.name, "Tier 16 maps");
        assert_eq!(m.extras.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["upgrade", "bands", "implicit", "corrupted"]);
        assert!(m.extras.iter().all(|e| e.on));
        assert_eq!(m.extras[0].border, [220, 50, 0, 255]);
        // Switching the upgrade borders off empties their look but keeps them as pass-through rules.
        let up = m.extras[0].tiers.clone();
        let mut off = crate::filter::parse_filter(&fixture());
        let rules: Vec<Rule> = up.iter().map(|t| Rule::Tier { type_path: vec!["maps".into()], tier: t.clone(), show: None, actions: Some(vec![]), min_stack: None, min_item_tier: None }).collect();
        apply(&mut off, &rules, "
");
        assert!(!super::map_tiers(&off).extras[0].on);
        // Hiding tiers 1-3 survives as tier rules.
        let mut segs = segs;
        let rules: Vec<Rule> = m.tiers.iter().filter(|t| t.map_tier < 4).map(|t| Rule::Tier {
            type_path: vec!["maps".into()], tier: t.row.tier.clone(), show: Some(false), actions: None, min_stack: None, min_item_tier: None }).collect();
        assert!(apply(&mut segs, &rules, "\n").is_empty());
        let after = super::map_tiers(&segs);
        assert!(after.tiers.iter().all(|t| (t.row.visibility == "Hide") == (t.map_tier < 4)));
    }

    #[test]
    fn opens_any_rule() {
        let f = "Show # $type->leveling->normalmagic->4l $tier->general\n\tLinkedSockets >= 4\n\tRarity Normal Magic\n\tAreaLevel <= 67\n\tSetTextColor 255 255 255 255\n";
        let segs = crate::filter::parse_filter(f);
        let tp: Vec<String> = ["leveling", "normalmagic", "4l"].iter().map(|s| s.to_string()).collect();
        let s = super::section_at(&segs, &tp).unwrap();
        assert_eq!(s.title, "Levelling › White & blue › 4-links");
        assert_eq!(s.tiers[0].note, "4+ links, normal & magic, area level <= 67");
    }
    use super::*;
    use crate::filter::{parse_filter, render_filter};
    use crate::rules::{apply, Rule};

    fn fixture() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap()
    }

    #[test]
    fn lists_real_tiers() {
        let s = sections(&parse_filter(&fixture()));
        let cur = &s[0];
        assert_eq!(cur.title, "Currency");
        assert!(cur.tiers[0].bases.contains(&"Mirror of Kalandra".to_string()), "{:?}", cur.tiers[0]);
        assert_eq!(tier_name("t2divine"), "Tier 2 (divine)");
        assert_eq!(tier_name("twisdom"), "Wisdom scrolls");
        let w = s.iter().find(|x| x.title == "Wisdom scroll stacks").unwrap();
        assert_eq!(w.tiers.iter().map(|t| t.min_stack.unwrap()).collect::<Vec<_>>(), vec![10, 5, 3]);
        let stacks = s.iter().find(|x| x.title == "Stacks of 3+").unwrap();
        assert!(stacks.tiers[0].note.contains("stacks of 3+"), "{:?}", stacks.tiers[0].note);
        assert!(s.iter().any(|x| x.title == "Divination cards" && x.tiers.len() >= 5));
        println!("{:?}", s.iter().map(|x| (&x.group, &x.title, x.tiers.len())).collect::<Vec<_>>());
    }

    #[test]
    fn verdict_names_its_rule() {
        let segs = parse_filter(&fixture());
        let it = |n| crate::item::Item { item_class: "Stackable Currency".into(), rarity: "Currency".into(), base_type: "Jeweller's Orb".into(), stack_size: Some(n), ..Default::default() };
        let v = crate::evaluate::status(&segs, &it(3), Some(70), false).verdict;
        println!("{:?}", v.source);
        assert_eq!(v.source.kind, "neversink");
        assert!(v.source.section.as_deref().map(|s| s.starts_with("Stacks of")).unwrap_or(false), "{:?}", v.source);
        assert!(v.source.tier_name.as_deref().unwrap_or("").starts_with("Tier"));
    }

    #[test]
    fn drag_item_between_tiers() {
        let mut segs = parse_filter(&fixture());
        let before = sections(&segs);
        let from = before[0].tiers.iter().find(|t| t.bases.contains(&"Chaos Orb".to_string())).unwrap().tier.clone();
        let rules = vec![Rule::MoveBase { type_path: vec!["currency".into()], base: "Chaos Orb".into(), to_tier: "t2divine".into() }];
        assert!(apply(&mut segs, &rules, "\r\n").is_empty());
        let after = sections(&parse_filter(&render_filter(&segs)));
        let has = |t: &str| after[0].tiers.iter().find(|x| x.tier == t).unwrap().bases.contains(&"Chaos Orb".to_string());
        assert!(has("t2divine") && !has(&from));
    }

    #[test]
    fn add_a_currency_to_a_stack_tier() {
        use crate::evaluate::status;
        let text = fixture();
        let segs0 = parse_filter(&text);
        let secs = sections(&segs0);
        let stacked: Vec<&String> = secs.iter().filter(|x| x.title.starts_with("Stacks") || x.title.ends_with("stacks"))
            .flat_map(|x| x.tiers.iter().flat_map(|t| t.bases.iter())).collect();
        // A currency NeverSink only tiers as single drops.
        let base = secs[0].tiers.iter().flat_map(|t| t.bases.iter()).find(|b| !stacked.contains(b)).unwrap().clone();
        let mut segs = parse_filter(&text);
        apply(&mut segs, &[Rule::MoveBase { type_path: vec!["currency".into(), "stackedsix".into()], base: base.clone(), to_tier: "t7".into() }], "\r\n");
        let segs = parse_filter(&render_filter(&segs));
        let it = |n| crate::item::Item { item_class: "Stackable Currency".into(), rarity: "Currency".into(), base_type: base.clone(), stack_size: Some(n), ..Default::default() };
        let v6 = status(&segs, &it(6), Some(70), false).verdict;
        let v1 = status(&segs, &it(1), Some(70), false).verdict;
        assert!(v6.decided_by.contains("stackedsix") && v6.decided_by.contains("t7"), "{base}: {}", v6.decided_by);
        assert!(!v1.decided_by.contains("stackedsix"), "single drops keep their normal tier");
        let ch = stack_choices(&segs, &base);
        assert!(ch.iter().any(|c| c.title == "Stacks of 6+" && c.current.as_deref() == Some("t7")));
        // Taking it out again.
        let mut segs2 = parse_filter(&render_filter(&segs));
        apply(&mut segs2, &[Rule::MoveBase { type_path: vec!["currency".into(), "stackedsix".into()], base: base.clone(), to_tier: String::new() }], "\r\n");
        let segs2 = parse_filter(&render_filter(&segs2));
        assert!(stack_choices(&segs2, &base).iter().all(|c| c.current.is_none()));
        let w = stack_choices(&segs2, "Scroll of Wisdom");
        assert!(w.iter().any(|c| c.title == "Wisdom scroll stacks" && c.current.is_some()), "{:?}", w.iter().map(|c| &c.title).collect::<Vec<_>>());
    }

    #[test]
    fn tier_rule_hides_and_restyles() {
        let mut segs = parse_filter(&fixture());
        let rules = vec![
            Rule::Tier { type_path: vec!["currency".into()], tier: "t5alchemy".into(), show: Some(false), actions: None, min_stack: None, min_item_tier: None },
            Rule::Tier { type_path: vec!["gold".into()], tier: "stack2".into(), show: None, actions: None, min_stack: Some(800), min_item_tier: None },
            Rule::Tier { type_path: vec!["divination".into()], tier: "t1".into(), show: None,
                actions: Some(vec!["SetFontSize 45".into(), "SetTextColor 1 2 3 255".into()]), min_stack: None, min_item_tier: None },
        ];
        let w = apply(&mut segs, &rules, "\r\n");
        assert!(w.is_empty(), "{w:?}");
        let out = parse_filter(&render_filter(&segs));
        let s = sections(&out);
        let alch = s[0].tiers.iter().find(|t| t.tier == "t5alchemy").unwrap();
        assert_eq!(alch.visibility, "Hide");
        let div = s.iter().find(|x| x.title == "Divination cards").unwrap();
        let t1 = div.tiers.iter().find(|t| t.tier == "t1").unwrap();
        assert_eq!(t1.style.text, Some([1, 2, 3, 255]));
        assert_eq!(t1.style.sound, None, "old sound removed");
        let gold = s.iter().find(|x| x.title == "Gold").unwrap();
        let large = gold.tiers.iter().find(|t| t.tier == "stack2").unwrap();
        assert_eq!((large.name.as_str(), large.min_stack), ("Large piles", Some(800)));
    }
}
