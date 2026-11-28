//! Uniques by how much Thaumaturgic Dust they give when disenchanted in Kingsmarch (PoE 1; he asked 28 Nov:
//! "a filter option for Dust uniques with a threshold of how much dust they give, or tiers").
//!
//! The game's formula (poedb.tw/us/Kingsmarch, Disenchant tab, checked 28 Nov 2026 for 3.30 Covens):
//!   dust = 5 * floor(25 * value * clamp(ilvl - 64, 1, 20) * (1 + 0.5 * (influences + corrupted implicits)
//!                    + 2 * quality / 100) + 0.01)
//! `value` is fixed per unique (core/data/poe1_dust.json, `tools/build_dust.mjs`). So item level matters most: 65 or
//! lower gives 1x, 84+ gives 20x. Quality, influence and corrupted implicits only add on top.
//!
//! A filter only sees the base, so a base counts at its lowest-dust unique (what any unique on it gives at least),
//! and each tier gets one rule per item level it needs. Quality etc. aren't counted: they only ever give more.

use crate::filter::Segment;
use crate::prices::{drops_on_ground, Tier};
use crate::rules::{Beam, CustomStyle, Minimap, Sound, style_actions};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DustSettings {
    pub enabled: bool,
    /// Gives at least `min` dust (at the item's own level, no quality). Sorted when used.
    pub tiers: Vec<Tier>,
}

#[derive(Deserialize, Default)]
struct DustFile { #[serde(default)] updated: String, bases: BTreeMap<String, Vec<(String, f64)>> }

fn data() -> &'static DustFile {
    static D: OnceLock<DustFile> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_dust.json")).unwrap_or_default())
}

/// When the dust list was made (YYYY-MM-DD).
pub fn updated() -> &'static str { &data().updated }

/// The game's formula. `extras` = influences + corrupted implicits.
pub fn dust(value: f64, ilvl: u32, quality: u32, extras: u32) -> u64 {
    let lvl = (ilvl as f64 - 64.0).clamp(1.0, 20.0);
    5 * (25.0 * value * lvl * (1.0 + 0.5 * extras as f64 + 2.0 * quality as f64 / 100.0) + 0.01).floor() as u64
}

/// The unique's disenchant value (None = not in the list).
pub fn unique_value(name: &str) -> Option<f64> {
    data().bases.values().flatten().find(|(n, _)| n == name).map(|x| x.1)
}

/// Every unique on this base with its value, least dust first.
pub fn uniques_on(base: &str) -> &'static [(String, f64)] {
    data().bases.get(base).map(|v| v.as_slice()).unwrap_or(&[])
}

/// A base's value as a ground drop: its lowest-dust unique (Replica and Foulborn ones don't drop as plain uniques).
pub fn base_value(base: &str) -> Option<f64> {
    uniques_on(base).iter().filter(|(n, _)| drops_on_ground(n)).map(|x| x.1).reduce(f64::min)
}

/// The lowest item level (64 = any) at which a unique worth `value` gives `min` dust; None if not even at 84.
pub fn level_for(value: f64, min: u64) -> Option<u32> {
    (64..=84).find(|l| dust(value, *l, 0, 0) >= min)
}

/// Bases NeverSink already makes loud as uniques (tier 1, tier 2, the best multi-unique bases): left to him, so a
/// money drop never gets the quieter dust look.
fn loud_bases(segs: &[Segment]) -> HashSet<String> {
    segs.iter().filter_map(|s| if let Segment::Block(b) = s { Some(b) } else { None })
        .filter(|b| b.type_path.first().is_some_and(|t| t == "uniques") && b.type_path.len() == 1
            && matches!(b.tier.as_deref(), Some("t1" | "t2" | "multispecialhigh")))
        .flat_map(|b| b.base_types()).collect()
}

/// One base the dust rules pick out.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DustPick {
    pub base: String,
    /// Dust at item level 84 from the base's lowest and highest unique.
    pub low: u64,
    pub high: u64,
    /// How many uniques drop on it.
    pub uniques: usize,
    /// The tier it reaches at 84 (0 = the top one), and from which item level.
    pub tier: usize,
    pub ilvl: u32,
    /// NeverSink already makes it loud, so it's left alone.
    pub neversink: bool,
}

fn sorted(tiers: &[Tier]) -> Vec<&Tier> {
    let mut t: Vec<&Tier> = tiers.iter().filter(|t| t.min > 0.0).collect();
    t.sort_by(|a, b| b.min.total_cmp(&a.min));
    t
}

/// Every base that reaches a tier at item level 84, most dust first.
pub fn picks(tiers: &[Tier], segs: &[Segment]) -> Vec<DustPick> {
    let tiers = sorted(tiers);
    let Some(lowest) = tiers.last() else { return vec![] };
    let loud = loud_bases(segs);
    let mut out: Vec<DustPick> = data().bases.keys().filter_map(|base| {
        let v = base_value(base)?;
        if dust(v, 84, 0, 0) < lowest.min as u64 { return None; }
        let tier = tiers.iter().position(|t| dust(v, 84, 0, 0) >= t.min as u64)?;
        let ground: Vec<f64> = uniques_on(base).iter().filter(|(n, _)| drops_on_ground(n)).map(|x| x.1).collect();
        let high = ground.iter().copied().fold(v, f64::max);
        Some(DustPick { base: base.clone(), low: dust(v, 84, 0, 0), high: dust(high, 84, 0, 0), uniques: ground.len(), tier,
            ilvl: level_for(v, tiers[tier].min as u64).unwrap_or(84), neversink: loud.contains(base) })
    }).collect();
    out.sort_by(|a, b| b.low.cmp(&a.low).then_with(|| a.base.cmp(&b.base)));
    out
}

/// The filter's rules: per tier (highest first), one block per item level it needs. Six-links are left to
/// NeverSink, and so are bases he already makes loud.
pub fn blocks(settings: &DustSettings, segs: &[Segment], nl: &str) -> String {
    if !settings.enabled { return String::new(); }
    let tiers = sorted(&settings.tiers);
    if tiers.is_empty() { return String::new(); }
    let loud = loud_bases(segs);
    // (tier, item level) -> bases
    let mut by: BTreeMap<(usize, std::cmp::Reverse<u32>), Vec<&str>> = BTreeMap::new();
    for base in data().bases.keys().filter(|b| !loud.contains(*b)) {
        let Some(v) = base_value(base) else { continue };
        for (i, t) in tiers.iter().enumerate() {
            if let Some(l) = level_for(v, t.min as u64) { by.entry((i, std::cmp::Reverse(l))).or_default().push(base); }
        }
    }
    let mut out = String::new();
    for ((i, level), names) in by {
        out.push_str(&format!("Show # Dust tier {}: uniques giving {}+ Thaumaturgic Dust{nl}", i + 1, tiers[i].min as u64));
        out.push_str(&format!("    Rarity Unique{nl}    LinkedSockets <= 5{nl}"));
        if level.0 > 64 { out.push_str(&format!("    ItemLevel >= {}{nl}", level.0)); }
        out.push_str(&format!("    BaseType == {}{nl}", names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ")));
        for a in style_actions(&tiers[i].style) { out.push_str(&format!("    {a}{nl}")); }
        out.push_str(nl);
    }
    out
}

/// What a unique gives at its item level, quality, influences and corrupted implicits.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UniqueDust { pub dust: u64, pub at84: u64 }

pub fn of_unique(name: &str, ilvl: Option<u32>, quality: u32, extras: u32) -> Option<UniqueDust> {
    let v = unique_value(name)?;
    Some(UniqueDust { dust: dust(v, ilvl.unwrap_or(84), quality, extras), at84: dust(v, 84, 0, 0) })
}

/// Starting tiers: NeverSink's own dust look (orange on dark blue, purple beam), quieter further down.
pub fn default_settings() -> DustSettings {
    let look = |size: u32, beam: Option<bool>, icon: u8, shape: &str, sound: Sound| CustomStyle {
        font_size: size, text: [175, 96, 37, 255], border: Some([175, 96, 37, 255]), background: [10, 30, 45, 255],
        beam: beam.map(|temp| Beam { colour: "Purple".into(), temp }),
        minimap: Some(Minimap { size: icon, colour: "Purple".into(), shape: shape.into() }), sound, label_hidden: false,
    };
    DustSettings { enabled: false, tiers: vec![
        Tier { min: 50_000.0, style: look(45, Some(false), 1, "Star", Sound::Builtin { id: "3".into(), volume: 300 }) },
        Tier { min: 20_000.0, style: look(42, Some(true), 2, "Star", Sound::Default) },
        Tier { min: 10_000.0, style: look(38, None, 2, "Circle", Sound::Default) },
    ] }
}

/// For the page: how many bases each tier picks out.
pub fn tier_counts(tiers: &[Tier], segs: &[Segment]) -> HashMap<usize, usize> {
    let mut m = HashMap::new();
    for p in picks(tiers, segs).into_iter().filter(|p| !p.neversink) { *m.entry(p.tier).or_insert(0) += 1; }
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate::status, filter::parse_filter, item::parse_item};

    #[test]
    fn the_games_formula() {
        // poedb's own examples: Original Sin 1128.89 -> 2,822,225 at 84 and 3,951,115 at 84 with 20% quality.
        assert_eq!(dust(1128.89, 84, 0, 0), 2_822_225);
        assert_eq!(dust(1128.89, 84, 20, 0), 3_951_115);
        assert_eq!(dust(1.0, 84, 0, 0), 2500);
        assert_eq!(dust(1.0, 60, 0, 0), 125, "65 and lower all count as 1x");
        assert_eq!(dust(1.0, 90, 0, 0), 2500, "84 and up all count as 20x");
        assert_eq!(dust(10.0, 84, 0, 1), 37_500, "an influence or corrupted implicit adds half");
        assert_eq!(level_for(10.0, 25_000), Some(84));
        assert_eq!(level_for(10.0, 12_500), Some(74));
        assert_eq!(level_for(10.0, 1_000), Some(64));
        assert_eq!(level_for(10.0, 25_001), None);
    }

    #[test]
    fn data_is_there() {
        assert!(data().bases.len() > 400, "{}", data().bases.len());
        assert!(unique_value("Headhunter").is_some_and(|v| v > 800.0));
        // Leather Belt has cheap uniques too, so it counts at the cheapest.
        assert!(base_value("Leather Belt").unwrap() < unique_value("Headhunter").unwrap());
        assert!(uniques_on("Leather Belt").iter().any(|(n, _)| n == "Headhunter"));
    }

    #[test]
    fn rules_by_dust() {
        // Pick a real base whose cheapest unique gives at least 50k at 84, and one well under 10k.
        let rich = data().bases.keys().find(|b| base_value(b).is_some_and(|v| dust(v, 84, 0, 0) >= 60_000)).unwrap().clone();
        let poor = data().bases.keys().find(|b| base_value(b).is_some_and(|v| dust(v, 84, 0, 0) < 5_000)).unwrap().clone();
        let mut s = default_settings();
        assert_eq!(blocks(&s, &[], "\n"), "", "off by default");
        s.enabled = true;
        let text = blocks(&s, &[], "\n") + "Show\n    SetFontSize 20\n";
        let segs = parse_filter(&text);
        let size = |base: &str, ilvl: u32, links: &str| {
            let it = parse_item(&format!("Item Class: Belts\nRarity: Unique\nSome Unique\n{base}\n--------\nItem Level: {ilvl}\n{links}")).unwrap();
            status(&segs, &it, None, false).verdict.style.font_size.map(|n| n.to_string())
        };
        assert_eq!(size(&rich, 84, "").as_deref(), Some("45"), "{rich}\n{text}");
        assert_eq!(size(&poor, 84, "").as_deref(), Some("20"), "{poor}");
        // At a low item level the rich base drops a tier or more.
        let v = base_value(&rich).unwrap();
        let low = (65..84).find(|l| dust(v, *l, 0, 0) < 50_000 && dust(v, *l, 0, 0) >= 20_000);
        if let Some(l) = low { assert_eq!(size(&rich, l, "").as_deref(), Some("42"), "{rich} at {l}"); }
        assert_eq!(size(&rich, 1, "").as_deref(), if dust(v, 1, 0, 0) >= 10_000 { Some("38") } else { Some("20") });
    }

    #[test]
    fn neversinks_loud_uniques_stay_his() {
        let rich = data().bases.keys().find(|b| base_value(b).is_some_and(|v| dust(v, 84, 0, 0) >= 60_000)).unwrap().clone();
        let filter = format!("Show # $type->uniques $tier->t1\n    Rarity Unique\n    BaseType == \"{rich}\"\n    SetFontSize 45\n");
        let segs = parse_filter(&filter);
        let mut s = default_settings();
        s.enabled = true;
        assert!(!blocks(&s, &segs, "\n").contains(&format!("\"{rich}\"")));
        assert!(picks(&s.tiers, &segs).iter().any(|p| p.base == rich && p.neversink));
    }

    #[test]
    fn on_neversinks_real_filter() {
        let base = include_str!("../tests/fixtures/poe1-semistrict.filter");
        let mut segs = parse_filter(base);
        let loud = loud_bases(&segs);
        assert!(loud.len() > 20, "NeverSink's t1/t2 unique bases are found: {}", loud.len());
        let mut s = default_settings();
        s.enabled = true;
        let extra = blocks(&s, &segs, "\n");
        assert!(extra.matches("Show # Dust tier").count() >= 3, "{extra}");
        for b in &loud { assert!(!extra.contains(&format!("\"{b}\"")), "{b} is NeverSink's"); }
        crate::rules::apply_with_extra(&mut segs, &[], &extra, "\n");
        // The app writes the filter out and the game reads it back.
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        // A quiet unique base that gives lots of dust now has the dust look.
        let pick = picks(&s.tiers, &segs).into_iter().find(|p| !p.neversink && p.tier == 0).unwrap();
        let it = parse_item(&format!("Item Class: Belts\nRarity: Unique\nSome Unique\n{}\n--------\nItem Level: 84\n", pick.base)).unwrap();
        let v = status(&segs, &it, None, false).verdict;
        assert_eq!((v.visibility.as_str(), v.style.background), ("Show", Some([10, 30, 45, 255])), "{} {}", pick.base, v.decided_by);
    }
}
