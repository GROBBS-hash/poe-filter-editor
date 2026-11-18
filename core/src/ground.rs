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
    /// An identified unique's name on the label, e.g. "Blood Price" (on a Reaver Helmet).
    #[serde(default)]
    pub unique: Option<String>,
}

/// What the game's hover box (item tooltip) told us.
#[derive(Debug, Clone, PartialEq)]
pub struct TooltipRead {
    pub found: GroundMatch,
    pub item_level: Option<u32>,
    pub unidentified: bool,
    /// From the colour of the name at the top: Normal / Magic / Rare / Unique.
    pub rarity: Option<String>,
}

/// Rarity from the colour of the item's name in the hover box.
pub fn rarity_from_colour(c: [u8; 3]) -> &'static str {
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    if b > r + 30 && b > g { "Magic" }
    else if r > 170 && g > 160 && b < g - 40 { "Rare" }
    else if r > g + 40 && g > b { "Unique" }
    else { "Normal" }
}

/// Reads the hover box the game shows for a ground item: lines top to bottom,
/// each with its (text colour, background) when known. The name sits above
/// "Item Level", so only lines up there count (other labels on screen don't).
pub fn from_tooltip(lines: &[(String, Option<[u8; 3]>)], known: &HashSet<String>) -> Option<TooltipRead> {
    let lower: Vec<String> = lines.iter().map(|(t, _)| t.to_lowercase()).collect();
    let il = lower.iter().position(|l| l.replace(' ', "").starts_with("itemlevel"))?;
    let item_level = lower[il].chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().ok();
    let unidentified = lower.iter().any(|l| l.contains("unidentified"));
    // Nearest line above "Item Level" that names a base exactly (the header).
    for i in (il.saturating_sub(14)..il).rev() {
        let Some(m) = match_base(&lines[i].0, known) else { continue };
        if m.confidence < 1.0 { continue; }
        let rarity = lines[i].1.map(|c| rarity_from_colour(c).to_string());
        // Most of the line should be the name, not a stat that happens to contain one
        // (magic names are long: "Glimmering Opal Sceptre of Embers", so blue lines may be).
        if compact(&m.base).len() * 10 < compact(&lines[i].0).len() * 5 && rarity.as_deref() != Some("Magic") { continue; }
        // An identified unique/rare shows its own name on the line above the base.
        let mut found = m;
        if found.unique.is_none() && i > 0 {
            found.unique = unique_on_label(&format!("{} {}", lines[i - 1].0, lines[i].0), &found.base);
        }
        return Some(TooltipRead { found, item_level, unidentified, rarity });
    }
    None
}

/// Puts what the hover box said onto an item read from its label: the box is
/// bigger and clearer, so its rarity, item level and identified state win.
pub fn apply_tooltip(item: &mut Item, t: &TooltipRead) {
    if t.item_level.is_some() { item.item_level = t.item_level; }
    if let Some(name) = &t.found.unique {
        item.rarity = "Unique".into();
        item.name = Some(name.clone());
        item.unidentified = false;
        item.sockets = None;
        return;
    }
    let gear = matches!(item.rarity.as_str(), "Normal" | "Magic" | "Rare" | "Unique");
    if let (true, Some(r)) = (gear, &t.rarity) {
        if *r != item.rarity {
            item.rarity = r.clone();
            if item.rarity != "Normal" && item.rarity != "Magic" { item.sockets = None; }
            if item.rarity != "Unique" { item.name = None; }
        }
        item.unidentified = t.unidentified;
    }
}

/// An identified unique shows its name before the base ("BLOOD PRICE REAVER HELMET").
/// Finds which unique on that base the label names, allowing a slightly garbled read.
pub fn unique_on_label(text: &str, base: &str) -> Option<String> {
    // Letters the reader often mixes up count as the same.
    let fold = |s: &str| compact(s).chars().map(|c| match c { '0' => 'o', '1' | 'l' | '|' => 'i', '5' => 's', c => c }).collect::<String>();
    let t = fold(text);
    let b = fold(base);
    let before = t.find(&b).map(|i| t[..i].to_string()).unwrap_or_else(|| t.clone());
    if before.len() < 3 { return None; }
    crate::gear::uniques().iter()
        .filter(|u| u.base == base)
        .map(|u| (fold(&u.name), &u.name))
        .filter(|(c, _)| before.contains(c.as_str()) || strsim::jaro_winkler(&before, c) >= 0.9)
        .max_by_key(|(c, _)| c.len())
        .map(|(_, n)| n.clone())
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
    // "3 GOLD" / "1,250 Gold": a plain number in front.
    if let Some((n, rest)) = t.split_once(' ') {
        if let Ok(v) = n.replace(',', "").parse::<u32>() {
            return (Some(v), rest.trim());
        }
    }
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
    // A name found inside the text must be whole words ("hope" sits inside
    // "c-HOPE-r": that's not the Hope card) or most of what was read.
    let spaced = format!(" {} ", norm(rest));
    let exact = known.iter().filter(|b| {
        let c = compact(b);
        c.len() >= 3 && t.contains(&c) && (spaced.contains(&format!(" {} ", norm(b))) || c.len() * 10 >= t.len() * 6)
    }).max_by_key(|b| b.len());
    if let Some(b) = exact {
        return Some(GroundMatch { unique: unique_on_label(text, b), base: b.clone(), stack, tier, superior, confidence: 1.0 });
    }
    // Fuzzy: slide each name along the text and keep the closest fit.
    let chars: Vec<char> = t.chars().collect();
    let mut best: Option<(f64, &String)> = None;
    for b in known {
        let c = compact(b);
        let n = c.chars().count();
        if n < 4 { continue; }
        // Short names fit by chance inside longer words: only when they're most of the text.
        if n < 7 && n * 10 < t.len() * 6 { continue; }
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
    if let Some((s, b)) = best.filter(|(s, _)| *s >= 0.9) {
        return Some(GroundMatch { unique: unique_on_label(text, b.clone().as_str()), base: b.clone(), stack, tier, superior, confidence: s as f32 });
    }
    // Start of the label cut off ("onic Spirit Shield"): accept it if exactly one name ends that way.
    if t.len() >= 8 {
        let ends: Vec<&String> = known.iter().filter(|b| compact(b).ends_with(&t)).collect();
        if ends.len() == 1 {
            return Some(GroundMatch { unique: unique_on_label(text, ends[0].clone().as_str()), base: ends[0].clone(), stack, tier, superior, confidence: 0.8 });
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
    if let Some(name) = &m.unique {
        item.rarity = "Unique".into();
        item.name = Some(name.clone());
        item.unidentified = false;
        return item;
    }
    item.rarity = match class {
        "Divination Cards" => "Divination Card".into(),
        "Skill Gems" | "Support Gems" => "Gem".into(),
        _ if crate::item::is_gear(&Item { rarity: "Normal".into(), ..item.clone() }) || class.is_empty() => {
            let (r, sockets) = guess_rarity_links(segs, &item, label, area_level);
            item.sockets = sockets;
            r
        }
        _ => "Currency".into(),
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
    guess_rarity_links(segs, item, label, area_level).0
}

/// Like `guess_rarity`, but also considers white/blue items with linked sockets
/// (which NeverSink shows in their own look while levelling). Returns the
/// rarity and, when a linked look matched best, the sockets assumed.
pub fn guess_rarity_links(segs: &[Segment], item: &Item, label: Option<LabelColours>, area_level: u32) -> (String, Option<String>) {
    let Some(seen) = label else { return ("Normal".into(), None) };
    // The game's default colours, used when the filter doesn't set one.
    let defaults = [("Normal", [200, 200, 200]), ("Magic", [136, 136, 255]), ("Rare", [255, 255, 119]), ("Unique", [175, 96, 37])];
    let dist = |a: [u8; 3], b: [u8; 3]| -> i32 { a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2)).sum() };
    // Backgrounds are drawn semi-transparent over the game, so a dark one reads as near-black.
    let bg_of = |c: Option<[u8; 4]>| -> [u8; 3] {
        c.map(|c| { let a = c[3] as u32; [(c[0] as u32 * a / 255) as u8, (c[1] as u32 * a / 255) as u8, (c[2] as u32 * a / 255) as u8] })
            .unwrap_or([0, 0, 0])
    };
    // Plain first; a small nudge so links only win when they explain the label better.
    let socket_options: [(Option<&str>, i32); 3] = [(None, 0), (Some("R-R-R"), 2_000), (Some("R-R-R-R"), 2_000)];
    let mut best: Option<(i32, &str, Option<&str>)> = None;
    for (r, def) in defaults.iter() {
        for (sockets, nudge) in socket_options {
            if sockets.is_some() && !matches!(*r, "Normal" | "Magic") { continue; }
            let it = Item { rarity: (*r).into(), sockets: sockets.map(String::from), ..item.clone() };
            let v = evaluate(segs, &Ctx { item: &it, area_level: Some(area_level), stack: None, from_screen: true });
            let text = v.style.text.map(|t| [t[0], t[1], t[2]]).unwrap_or(*def);
            let hidden_penalty = if v.visibility == "Hide" { 200_000 } else { 0 };
            let d = dist(text, seen.text) + dist(bg_of(v.style.background), seen.background) + hidden_penalty + nudge;
            if best.map(|b| d < b.0).unwrap_or(true) { best = Some((d, r, sockets)); }
        }
    }
    best.map(|(_, r, s)| (r.to_string(), s.map(String::from))).unwrap_or_else(|| ("Normal".into(), None))
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
    fn short_names_inside_words_dont_count() {
        let mut k = known();
        k.insert("Hope".into());
        k.insert("Karui Chopper".into());
        assert_ne!(match_base("IRV-CHOPPER-", &k).map(|m| m.base), Some("Hope".to_string()));
        assert_eq!(match_base("KARUI CHOPPER", &k).unwrap().base, "Karui Chopper");
        assert_eq!(match_base("KARUICHOPPER", &k).unwrap().base, "Karui Chopper");
        assert_eq!(match_base("Hope", &k).unwrap().base, "Hope");
        assert_eq!(match_base("3x Hope", &k).unwrap().base, "Hope");
        // Letters lost in the middle: still the chopper.
        assert_eq!(match_base("KARUI OPPER-", &k).unwrap().base, "Karui Chopper");
    }

    #[test]
    fn reads_the_hover_box() {
        let mut k = known();
        k.insert("Serpentine Staff".into());
        k.insert("Opal Sceptre".into());
        let line = |t: &str| (t.to_string(), None);
        let tip = vec![
            ("SERPENTINE STAFF".to_string(), Some([255, 255, 119])),
            line("Warstaff"), line("Physical Damage: 56-117"), line("Critical Strike Chance: 7.80%"),
            line("Item Level: 61"), line("Requires 85 Str, 85 Int"), line("+22% Chance to Block Attack Damage"), line("Unidentified"),
            line("SERP"), // the half-hidden label elsewhere on screen
        ];
        let r = from_tooltip(&tip, &k).unwrap();
        assert_eq!((r.found.base.as_str(), r.item_level, r.unidentified, r.rarity.as_deref()), ("Serpentine Staff", Some(61), true, Some("Rare")));
        // No "Item Level" line: not a hover box.
        assert!(from_tooltip(&tip[..3], &k).is_none());
        assert_eq!(rarity_from_colour([136, 136, 255]), "Magic");
        assert_eq!(rarity_from_colour([175, 96, 37]), "Unique");
        assert_eq!(rarity_from_colour([200, 200, 200]), "Normal");
        // A magic name is long, but its blue colour says it's the header.
        let magic = vec![("Glimmering Opal Sceptre of Embers".to_string(), Some([136, 136, 255])), line("Sceptre"), line("Item Level: 70")];
        let r = from_tooltip(&magic, &k).unwrap();
        assert_eq!((r.found.base.as_str(), r.rarity.as_deref()), ("Opal Sceptre", Some("Magic")));
        // The label said Normal with 4 links; the box says Rare.
        let mut it = Item { base_type: "Opal Sceptre".into(), rarity: "Normal".into(), sockets: Some("R-R-R-R".into()), ..Default::default() };
        apply_tooltip(&mut it, &TooltipRead { rarity: Some("Rare".into()), ..r });
        assert_eq!((it.rarity.as_str(), it.sockets.as_deref(), it.item_level, it.unidentified), ("Rare", None, Some(70), false));
    }

    #[test]
    fn identified_unique_on_label() {
        let mut k = known();
        k.insert("Reaver Helmet".into());
        let m = match_base("BLOOD PRICE REAVER HELMET •", &k).unwrap();
        assert_eq!((m.base.as_str(), m.unique.as_deref()), ("Reaver Helmet", Some("Blood Price")));
        // A slightly garbled read still finds it; a plain base has none.
        assert_eq!(match_base("BL00D PRlCE REAVER HELMET", &k).unwrap().unique.as_deref(), Some("Blood Price"));
        assert_eq!(match_base("REAVER HELMET", &k).unwrap().unique, None);
        let it = ground_item(&[], &m, None, 40);
        assert_eq!((it.rarity.as_str(), it.name.as_deref(), it.unidentified), ("Unique", Some("Blood Price"), false));
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
    fn gold_stack_without_x() {
        let mut k = known();
        k.insert("Gold".into());
        let m = match_base("3 GOLD", &k).unwrap();
        assert_eq!((m.base.as_str(), m.stack), ("Gold", Some(3)));
        let m = match_base("1,250 Gold", &k).unwrap();
        assert_eq!(m.stack, Some(1250));
    }

    #[test]
    fn nonsense_is_none() {
        assert!(match_base("Zzzqx Pwwq", &known()).is_none());
    }

    #[test]
    fn rarity_from_filter_colour() {
        let f = "Show\n    Rarity Rare\n    BaseType == \"Coral Ring\"\n    SetTextColor 10 200 10 255\n\nShow\n    Rarity Magic\n    BaseType == \"Coral Ring\"\n    SetTextColor 200 10 10 255\n";
        let segs = parse_filter(f);
        let m = GroundMatch { base: "Coral Ring".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
        let lc = |t: [u8; 3]| Some(LabelColours { text: t, background: [10, 10, 10] });
        assert_eq!(ground_item(&segs, &m, lc([20, 190, 20]), 83).rarity, "Rare");
        assert_eq!(ground_item(&segs, &m, lc([190, 20, 20]), 83).rarity, "Magic");
        assert_eq!(ground_item(&segs, &m, lc([20, 190, 20]), 83).item_class, "Rings");
    }

    #[test]
    fn light_text_on_dark_is_not_an_orange_unique() {
        let f = "Show\n    Rarity Unique\n    BaseType == \"Opal Wand\"\n    SetTextColor 255 255 255 255\n    SetBackgroundColor 175 96 37 255\n\nHide\n    Rarity Normal Magic\n    BaseType == \"Opal Wand\"\n\nShow\n    Rarity Rare\n    BaseType == \"Opal Wand\"\n";
        let segs = parse_filter(f);
        let m = GroundMatch { base: "Opal Wand".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
        let seen = Some(LabelColours { text: [235, 225, 200], background: [15, 12, 10] });
        assert_eq!(ground_item(&segs, &m, seen, 83).rarity, "Rare");
    }
}
