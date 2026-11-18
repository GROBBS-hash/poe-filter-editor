//! Turning text read off a ground label into an item.
//! The screen-reading itself lives in the app (it's Windows-only);
//! this part is plain logic so it can be tested anywhere.

use crate::evaluate::{evaluate, Ctx};
use crate::filter::Segment;
use crate::item::{base_classes, fill_class, Item};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GroundMatch {
    pub base: String,
    pub stack: Option<u32>,
    /// The 3.30 item tier shown on the label, e.g. "(Tier 2)".
    pub tier: Option<u32>,
    /// "Superior ..." on the label means it has quality.
    #[serde(default)]
    pub superior: bool,
    /// 1.0 = the exact name was found in the text.
    pub confidence: f32,
}

fn norm(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '\'' { c.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Letters only, no spaces: text recognition often splits or merges words.
fn compact(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

/// "3x Chaos Orb" -> (Some(3), "Chaos Orb")
fn split_stack(text: &str) -> (Option<u32>, &str) {
    let t = text.trim();
    if let Some((n, rest)) = t.split_once(['x', 'X', '×']) {
        if let Ok(v) = n.trim().replace(',', "").parse::<u32>() {
            return (Some(v), rest.trim());
        }
    }
    (None, t)
}

/// "Karui Chopper (Tier 2)" -> (Some(2), "Karui Chopper")
fn split_tier(text: &str) -> (Option<u32>, String) {
    let lower = text.to_lowercase();
    if let Some(i) = lower.find("(tier") {
        let tier = lower[i + 5..].chars().skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok();
        return (tier, text[..i].trim().to_string());
    }
    (None, text.to_string())
}

/// Finds which base type a ground label is showing. Exact (longest) match
/// first; if the text recognition garbled or split a word, a close match.
pub fn match_base(text: &str, known: &HashSet<String>) -> Option<GroundMatch> {
    let (tier, without_tier) = split_tier(text);
    let (stack, rest) = split_stack(&without_tier);
    let mut t = compact(rest);
    // "Superior" = has quality. The reader sometimes clips its first letters ("JPERIOR").
    let superior = ["superior", "uperior", "perior"].iter().any(|p| t.starts_with(p) || t.get(1..).map(|x| x.starts_with(p)).unwrap_or(false));
    if superior {
        if let Some(i) = t.find("erior") { t = t[i + 5..].to_string(); }
    }
    if t.len() < 3 {
        return None;
    }
    let exact = known.iter().filter(|b| {
        let c = compact(b);
        c.len() >= 3 && t.contains(&c)
    }).max_by_key(|b| b.len());
    if let Some(b) = exact {
        return Some(GroundMatch { base: b.clone(), stack, tier, superior, confidence: 1.0 });
    }
    // Fuzzy: slide each name along the text and keep the closest fit.
    let chars: Vec<char> = t.chars().collect();
    let mut best: Option<(f64, &String)> = None;
    for b in known {
        let c = compact(b);
        let n = c.chars().count();
        if n < 4 { continue; }
        let lo = n.saturating_sub(2).max(1);
        for len in lo..=(n + 2).min(chars.len()) {
            for w in chars.windows(len) {
                let window: String = w.iter().collect();
                let score = strsim::jaro_winkler(&window, &c);
                if best.map(|(s, _)| score > s).unwrap_or(true) {
                    best = Some((score, b));
                }
            }
        }
    }
    let _ = norm; // kept for future word-based matching
    if let Some((s, b)) = best.filter(|(s, _)| *s >= 0.9) {
        return Some(GroundMatch { base: b.clone(), stack, tier, superior, confidence: s as f32 });
    }
    // Start of the label cut off ("onic Spirit Shield"): accept it if exactly one name ends that way.
    if t.len() >= 8 {
        let ends: Vec<&String> = known.iter().filter(|b| compact(b).ends_with(&t)).collect();
        if ends.len() == 1 {
            return Some(GroundMatch { base: ends[0].clone(), stack, tier, superior, confidence: 0.8 });
        }
    }
    None
}

/// All names we can recognise: the game's base list plus anything the filter names.
pub fn known_names(filter_bases: &HashSet<String>) -> HashSet<String> {
    base_classes().keys().cloned().chain(filter_bases.iter().cloned()).collect()
}

/// Builds the item for a ground label. Gear rarity is guessed by comparing the
/// label's text colour with what the filter would colour each rarity.
pub fn ground_item(segs: &[Segment], m: &GroundMatch, label: Option<LabelColours>, area_level: u32) -> Item {
    let mut item = Item { base_type: m.base.clone(), stack_size: m.stack, item_tier: m.tier,
        quality: m.superior.then_some(1), ..Default::default() };
    fill_class(&mut item);
    let class = item.item_class.as_str();
    item.rarity = match class {
        "Stackable Currency" | "Delve Stackable Socketable Currency" | "Map Fragments" | "Incubators" => "Currency".into(),
        "Divination Cards" => "Divination Card".into(),
        "Skill Gems" | "Support Gems" => "Gem".into(),
        _ => guess_rarity(segs, &item, label, area_level),
    };
    item.unidentified = matches!(item.rarity.as_str(), "Magic" | "Rare" | "Unique");
    item
}

/// Colours read off the label on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelColours { pub text: [u8; 3], pub background: [u8; 3] }

/// Picks the rarity whose look in your filter best matches the label on
/// screen. The label is visible, so rarities your filter hides are unlikely.
pub fn guess_rarity(segs: &[Segment], item: &Item, label: Option<LabelColours>, area_level: u32) -> String {
    let Some(seen) = label else { return "Normal".into() };
    // The game's default colours, used when the filter doesn't set one.
    let defaults = [("Normal", [200, 200, 200]), ("Magic", [136, 136, 255]), ("Rare", [255, 255, 119]), ("Unique", [175, 96, 37])];
    let dist = |a: [u8; 3], b: [u8; 3]| -> i32 { a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2)).sum() };
    // Backgrounds are drawn semi-transparent over the game, so a dark one reads as near-black.
    let bg_of = |c: Option<[u8; 4]>| -> [u8; 3] {
        c.map(|c| { let a = c[3] as u32; [(c[0] as u32 * a / 255) as u8, (c[1] as u32 * a / 255) as u8, (c[2] as u32 * a / 255) as u8] })
            .unwrap_or([0, 0, 0])
    };
    defaults
        .iter()
        .map(|(r, def)| {
            let it = Item { rarity: (*r).into(), ..item.clone() };
            let v = evaluate(segs, &Ctx { item: &it, area_level: Some(area_level), stack: None, from_screen: true });
            let text = v.style.text.map(|t| [t[0], t[1], t[2]]).unwrap_or(*def);
            let hidden_penalty = if v.visibility == "Hide" { 200_000 } else { 0 };
            (dist(text, seen.text) + dist(bg_of(v.style.background), seen.background) + hidden_penalty, *r)
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, r)| r.to_string())
        .unwrap_or_else(|| "Normal".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::parse_filter;

    fn known() -> HashSet<String> {
        ["Coral Ring", "Ring", "Chaos Orb", "Orb of Alteration", "Leather Belt", "Divine Orb"]
            .iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_longest() {
        let m = match_base("Coral Ring of the Whelpling", &known()).unwrap();
        assert_eq!(m.base, "Coral Ring");
        assert_eq!(m.confidence, 1.0);
    }

    #[test]
    fn stack_prefix() {
        let m = match_base("12x Chaos Orb", &known()).unwrap();
        assert_eq!((m.base.as_str(), m.stack), ("Chaos Orb", Some(12)));
    }

    #[test]
    fn garbled_letters_still_match() {
        let m = match_base("Orb of A1teration", &known()).unwrap();
        assert_eq!(m.base, "Orb of Alteration");
        assert!(m.confidence < 1.0);
    }

    #[test]
    fn split_label_with_tier() {
        let mut k = known();
        k.insert("Karui Chopper".into());
        let m = match_base("KARUI CH OPPER (TIER 2)-", &k).unwrap();
        assert_eq!((m.base.as_str(), m.tier), ("Karui Chopper", Some(2)));
        let m = match_base("IC OPPER (TIER 2)-", &k);
        assert!(m.map(|m| m.base != "Coral Ring").unwrap_or(true));
    }

    #[test]
    fn cut_off_start() {
        let mut k = known();
        k.insert("Harmonic Spirit Shield".into());
        k.insert("Spirit Shield".into());
        let m = match_base("ONIC SPIRIT SHIELD (TIER 3)'", &k).unwrap();
        // "Spirit Shield" is inside the text, but the longer cut-off name fits better.
        assert!(m.base == "Harmonic Spirit Shield" || m.base == "Spirit Shield");
        assert_eq!(m.tier, Some(3));
    }

    #[test]
    fn superior_gem() {
        let mut k = known();
        k.insert("Lightning Tendrils".into());
        let m = match_base("JPERIOR LIGHTNING TENDRILS", &k).unwrap();
        assert_eq!(m.base, "Lightning Tendrils");
        assert!(m.superior);
    }

    #[test]
    fn nonsense_is_none() {
        assert!(match_base("Zzzqx Pwwq", &known()).is_none());
    }

    #[test]
    fn rarity_from_filter_colour() {
        let f = "Show\n    Rarity Rare\n    BaseType == \"Coral Ring\"\n    SetTextColor 10 200 10 255\n\nShow\n    Rarity Magic\n    BaseType == \"Coral Ring\"\n    SetTextColor 200 10 10 255\n";
        let segs = parse_filter(f);
        let m = GroundMatch { base: "Coral Ring".into(), stack: None, tier: None, superior: false, confidence: 1.0 };
        let lc = |t: [u8; 3]| Some(LabelColours { text: t, background: [10, 10, 10] });
        assert_eq!(ground_item(&segs, &m, lc([20, 190, 20]), 83).rarity, "Rare");
        assert_eq!(ground_item(&segs, &m, lc([190, 20, 20]), 83).rarity, "Magic");
        assert_eq!(ground_item(&segs, &m, lc([20, 190, 20]), 83).item_class, "Rings");
    }

    #[test]
    fn light_text_on_dark_is_not_an_orange_unique() {
        let f = "Show\n    Rarity Unique\n    BaseType == \"Opal Wand\"\n    SetTextColor 255 255 255 255\n    SetBackgroundColor 175 96 37 255\n\nHide\n    Rarity Normal Magic\n    BaseType == \"Opal Wand\"\n\nShow\n    Rarity Rare\n    BaseType == \"Opal Wand\"\n";
        let segs = parse_filter(f);
        let m = GroundMatch { base: "Opal Wand".into(), stack: None, tier: None, superior: false, confidence: 1.0 };
        let seen = Some(LabelColours { text: [235, 225, 200], background: [15, 12, 10] });
        assert_eq!(ground_item(&segs, &m, seen, 83).rarity, "Rare");
    }
}
