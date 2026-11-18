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

/// Finds which base type a ground label is showing. Exact (longest) match
/// first; if the text recognition garbled a letter, a close-enough match.
pub fn match_base(text: &str, known: &HashSet<String>) -> Option<GroundMatch> {
    let (stack, rest) = split_stack(text);
    let t = norm(rest);
    if t.is_empty() {
        return None;
    }
    let padded = format!(" {t} ");
    let exact = known
        .iter()
        .filter(|b| padded.contains(&format!(" {} ", norm(b))))
        .max_by_key(|b| b.len());
    if let Some(b) = exact {
        return Some(GroundMatch { base: b.clone(), stack, confidence: 1.0 });
    }
    // Fuzzy: compare against every run of words the same length as the base.
    let words: Vec<&str> = t.split(' ').collect();
    let mut best: Option<(f64, &String)> = None;
    for b in known {
        let nb = norm(b);
        let n = nb.split(' ').count();
        if n > words.len() {
            continue;
        }
        for w in words.windows(n) {
            let score = strsim::jaro_winkler(&w.join(" "), &nb);
            if best.map(|(s, _)| score > s).unwrap_or(true) {
                best = Some((score, b));
            }
        }
    }
    best.filter(|(s, _)| *s >= 0.9).map(|(s, b)| GroundMatch { base: b.clone(), stack, confidence: s as f32 })
}

/// All names we can recognise: the game's base list plus anything the filter names.
pub fn known_names(filter_bases: &HashSet<String>) -> HashSet<String> {
    base_classes().keys().cloned().chain(filter_bases.iter().cloned()).collect()
}

/// Builds the item for a ground label. Gear rarity is guessed by comparing the
/// label's text colour with what the filter would colour each rarity.
pub fn ground_item(segs: &[Segment], m: &GroundMatch, label_rgb: Option<[u8; 3]>, area_level: u32) -> Item {
    let mut item = Item { base_type: m.base.clone(), stack_size: m.stack, ..Default::default() };
    fill_class(&mut item);
    let class = item.item_class.as_str();
    item.rarity = match class {
        "Stackable Currency" | "Delve Stackable Socketable Currency" | "Map Fragments" | "Incubators" => "Currency".into(),
        "Divination Cards" => "Divination Card".into(),
        "Skill Gems" | "Support Gems" => "Gem".into(),
        _ => guess_rarity(segs, &item, label_rgb, area_level),
    };
    item.unidentified = matches!(item.rarity.as_str(), "Magic" | "Rare" | "Unique");
    item
}

pub fn guess_rarity(segs: &[Segment], item: &Item, label_rgb: Option<[u8; 3]>, area_level: u32) -> String {
    let Some(seen) = label_rgb else { return "Normal".into() };
    // The game's default label colours, used when the filter doesn't set one.
    let defaults = [("Normal", [200, 200, 200]), ("Magic", [136, 136, 255]), ("Rare", [255, 255, 119]), ("Unique", [175, 96, 37])];
    let dist = |a: [u8; 3], b: [u8; 3]| -> i32 {
        a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2)).sum()
    };
    defaults
        .iter()
        .map(|(r, def)| {
            let it = Item { rarity: (*r).into(), ..item.clone() };
            let v = evaluate(segs, &Ctx { item: &it, area_level: Some(area_level), stack: None, from_screen: true });
            let c = v.style.text.map(|t| [t[0], t[1], t[2]]).unwrap_or(*def);
            (dist(c, seen), *r)
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
    fn nonsense_is_none() {
        assert!(match_base("Zzzqx Pwwq", &known()).is_none());
    }

    #[test]
    fn rarity_from_filter_colour() {
        let f = "Show\n    Rarity Rare\n    BaseType == \"Coral Ring\"\n    SetTextColor 10 200 10 255\n\nShow\n    Rarity Magic\n    BaseType == \"Coral Ring\"\n    SetTextColor 200 10 10 255\n";
        let segs = parse_filter(f);
        let m = GroundMatch { base: "Coral Ring".into(), stack: None, confidence: 1.0 };
        assert_eq!(ground_item(&segs, &m, Some([20, 190, 20]), 83).rarity, "Rare");
        assert_eq!(ground_item(&segs, &m, Some([190, 20, 20]), 83).rarity, "Magic");
        assert_eq!(ground_item(&segs, &m, Some([20, 190, 20]), 83).item_class, "Rings");
    }
}
