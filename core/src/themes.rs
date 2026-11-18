//! Colour themes: recolour NeverSink's labels by how important they are.
//! Each NeverSink rule is sorted into one of six levels from how loud it is
//! (NeverSink uses beam colour for value: red best, then orange/yellow, white,
//! grey/brown). A theme says what text, border and background each level gets.
//! Sizes, beams, icons and sounds are left alone. Your own edits and price
//! tiers sit above NeverSink, so they keep their own looks.

use crate::evaluate::{style_of, Style};
use crate::filter::Segment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Level { Chase, High, Good, Medium, Low, Junk }

impl Level {
    pub const ALL: [Level; 6] = [Level::Chase, Level::High, Level::Good, Level::Medium, Level::Low, Level::Junk];
    fn index(self) -> usize { Level::ALL.iter().position(|l| *l == self).unwrap() }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LevelLook {
    pub text: [u8; 4],
    /// None = no border.
    pub border: Option<[u8; 4]>,
    pub background: [u8; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    pub name: String,
    /// Looks for Chase, High, Good, Medium, Low, Junk (in that order).
    pub levels: Vec<LevelLook>,
    /// Classic PoE style: colour by what the item is (uniques orange-brown,
    /// rares yellow...), with value shown by how solid and bright it is.
    #[serde(default)]
    pub classic: Option<std::collections::BTreeMap<String, [u8; 3]>>,
}

/// Item kinds for the classic style, in display order.
pub const CATEGORIES: [&str; 11] = ["Unique", "Rare", "Magic", "Normal", "Currency", "Divination", "Gem",
    "Fractured", "Top base", "Map", "Quest"];

pub fn classic_hues() -> std::collections::BTreeMap<String, [u8; 3]> {
    [("Unique", [175, 96, 37]), ("Rare", [255, 255, 119]), ("Magic", [136, 136, 255]), ("Normal", [210, 210, 210]),
     ("Currency", [196, 180, 140]), ("Divination", [110, 200, 255]), ("Gem", [27, 162, 155]),
     ("Fractured", [190, 120, 255]), ("Top base", [235, 235, 245]), ("Map", [225, 225, 225]), ("Quest", [74, 230, 58])]
        .into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// What kind of item a NeverSink rule is for (classic style).
pub fn category_of(b: &crate::filter::Block) -> &'static str {
    let path = b.type_path.join(">");
    let has = |s: &str| b.lines.iter().any(|l| l.contains(s));
    let first = b.type_path.first().map(String::as_str).unwrap_or("");
    if path.contains("fract") || has("FracturedItem True") { return "Fractured"; }
    match first {
        "uniques" => return "Unique",
        "divination" => return "Divination",
        "gems" => return "Gem",
        "currency" | "fragments" | "gold" | "runesgrafts" | "artefact" | "viscera" | "corpses" | "miscmapitems"
        | "miscmapitemsextra" | "expedition" | "event" | "enshrouded" => return "Currency",
        "exoticbases" | "crafting" | "influenced" | "exoticmods" | "6l" | "socketslinks" => return "Top base",
        "maps" | "exoticmap" | "charts" | "mapenchantments" => return "Map",
        "questlike" | "questlikeexception" | "heisttarget" => return "Quest",
        _ => {}
    }
    let rarity = b.lines.iter().map(|l| l.trim()).find(|l| l.starts_with("Rarity")).unwrap_or("");
    if rarity.contains("Unique") && !rarity.contains("Rare") { "Unique" }
    else if rarity.contains("Rare") && !rarity.contains("<") { "Rare" }
    else if rarity.contains("Magic") && !rarity.contains("Normal") { "Magic" }
    else { "Normal" }
}

/// Classic look: the item kind's colour, more solid and brighter the better it is.
pub fn classic_look(hue: [u8; 3], level: Level) -> LevelLook {
    let i = level.index();
    let tint = [0.42, 0.30, 0.20, 0.11, 0.05, 0.0][i];
    let bg_alpha = [255u8, 245, 230, 210, 180, 140][i];
    let shade = [0.45, 0.25, 0.10, 0.0, -0.22, -0.42][i]; // + toward white, - toward black
    let mix = |c: u8, s: f32| -> u8 {
        let c = c as f32;
        (if s >= 0.0 { c + (255.0 - c) * s } else { c * (1.0 + s) }).round().clamp(0.0, 255.0) as u8
    };
    let text = [mix(hue[0], shade), mix(hue[1], shade), mix(hue[2], shade), 255];
    let bg = [(hue[0] as f32 * tint) as u8, (hue[1] as f32 * tint) as u8, (hue[2] as f32 * tint) as u8, bg_alpha];
    let border = match level {
        Level::Chase => Some([255, 255, 255, 255]),
        Level::High => Some([mix(hue[0], 0.3), mix(hue[1], 0.3), mix(hue[2], 0.3), 255]),
        Level::Good => Some([hue[0], hue[1], hue[2], 255]),
        Level::Medium => Some([hue[0], hue[1], hue[2], 140]),
        _ => None,
    };
    LevelLook { text, border, background: bg }
}

/// The look a theme gives one NeverSink rule.
fn look_for(theme: &Theme, b: &crate::filter::Block, st: &Style) -> LevelLook {
    let level = level_of(st);
    match &theme.classic {
        Some(hues) => {
            let cat = category_of(b);
            let hue = hues.get(cat).copied().or_else(|| classic_hues().get(cat).copied()).unwrap_or([210, 210, 210]);
            classic_look(hue, level)
        }
        None => theme.levels[level.index()].clone(),
    }
}

/// How important NeverSink made this rule, from how it looks.
pub fn level_of(s: &Style) -> Level {
    let font = s.font_size.unwrap_or(32);
    let icon = s.minimap_parts.as_ref().map(|m| m.0);
    match s.beam.as_deref() {
        Some("Red") if icon == Some(0) => Level::Chase,
        Some("Red") | Some("Orange") => Level::High,
        Some("Yellow") => Level::Good,
        Some("Grey") | Some("Brown") => Level::Low,
        Some(_) if icon == Some(0) => Level::Good, // a category colour, marked as important
        Some(_) => Level::Medium,
        None if font >= 40 => Level::Low,
        None => Level::Junk,
    }
}

fn line_of(key: &str, c: [u8; 4], indent: &str, nl: &str) -> String {
    format!("{indent}{key} {} {} {} {}{nl}", c[0], c[1], c[2], c[3])
}

/// Recolours every NeverSink rule that sets colours. Rules that set none
/// (e.g. small add-ons that only change a sound) are left as they are.
pub fn apply_theme(segs: &mut [Segment], theme: &Theme) {
    if theme.levels.len() < 6 {
        return;
    }
    for seg in segs.iter_mut() {
        let Segment::Block(b) = seg else { continue };
        if b.visibility == "Hide" {
            continue;
        }
        let key_of = |l: &str| l.trim_start().split_whitespace().next().unwrap_or("").to_string();
        let has_colour = b.lines.iter().any(|l| matches!(key_of(l).as_str(), "SetTextColor" | "SetBorderColor" | "SetBackgroundColor"));
        if !has_colour {
            continue;
        }
        let st = style_of(&b.lines);
        if st.label_hidden {
            continue; // icon-only labels stay invisible
        }
        let look = &look_for(theme, b, &st);
        let sample = b.lines.iter().find(|l| key_of(l) == "SetTextColor" || key_of(l) == "SetBackgroundColor").cloned().unwrap_or_default();
        let indent: String = sample.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let nl = if sample.ends_with("\r\n") { "\r\n" } else { "\n" };
        // Drop the old colour lines, then add the theme's right after the font size (or at the end).
        b.lines.retain(|l| !matches!(key_of(l).as_str(), "SetTextColor" | "SetBorderColor" | "SetBackgroundColor"));
        let mut new = vec![line_of("SetTextColor", look.text, &indent, nl)];
        new.push(line_of("SetBorderColor", look.border.unwrap_or([0, 0, 0, 0]), &indent, nl));
        new.push(line_of("SetBackgroundColor", look.background, &indent, nl));
        let at = b.lines.iter().position(|l| key_of(l) == "SetFontSize").map(|i| i + 1)
            .or_else(|| b.lines.iter().position(|l| key_of(l) == "Continue"))
            .unwrap_or(b.lines.len());
        for (k, l) in new.into_iter().enumerate() {
            b.lines.insert(at + k, l);
        }
    }
}

fn lk(text: [u8; 3], border: Option<[u8; 3]>, bg: [u8; 4]) -> LevelLook {
    LevelLook { text: [text[0], text[1], text[2], 255], border: border.map(|b| [b[0], b[1], b[2], 255]), background: bg }
}

/// Ready-made themes. Copy one to make your own.
pub fn builtin_themes() -> Vec<Theme> {
    vec![
        Theme { name: "Classic PoE".into(), classic: Some(classic_hues()),
            levels: Level::ALL.iter().map(|l| classic_look([210, 210, 210], *l)).collect() },
        Theme { classic: None, name: "Midnight Neon".into(), levels: vec![
            lk([255, 255, 255], Some([255, 40, 140]), [170, 0, 90, 255]),
            lk([255, 120, 210], Some([255, 120, 210]), [35, 0, 35, 245]),
            lk([255, 225, 90], Some([255, 225, 90]), [28, 22, 0, 240]),
            lk([90, 235, 255], Some([90, 235, 255]), [0, 22, 32, 235]),
            lk([200, 200, 225], Some([90, 90, 125]), [10, 10, 22, 220]),
            lk([150, 150, 165], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, name: "Ember".into(), levels: vec![
            lk([255, 255, 255], Some([255, 200, 60]), [200, 40, 0, 255]),
            lk([255, 150, 40], Some([255, 150, 40]), [45, 12, 0, 245]),
            lk([255, 210, 120], Some([200, 140, 60]), [35, 20, 5, 240]),
            lk([240, 200, 170], Some([150, 100, 70]), [25, 15, 10, 230]),
            lk([190, 160, 140], None, [15, 10, 8, 210]),
            lk([140, 125, 115], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, name: "Frost".into(), levels: vec![
            lk([0, 20, 60], Some([255, 255, 255]), [200, 240, 255, 255]),
            lk([255, 255, 255], Some([120, 200, 255]), [20, 70, 140, 245]),
            lk([170, 225, 255], Some([170, 225, 255]), [8, 30, 60, 240]),
            lk([200, 230, 245], Some([90, 140, 180]), [10, 22, 36, 230]),
            lk([170, 185, 200], None, [8, 12, 18, 210]),
            lk([125, 135, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, name: "High Contrast".into(), levels: vec![
            lk([0, 0, 0], Some([255, 0, 0]), [255, 255, 255, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 230, 0, 255]),
            lk([255, 230, 0], Some([255, 230, 0]), [0, 0, 0, 255]),
            lk([255, 255, 255], Some([255, 255, 255]), [0, 0, 0, 255]),
            lk([220, 220, 220], None, [0, 0, 0, 230]),
            lk([150, 150, 150], None, [0, 0, 0, 200]),
        ]},
        // Blue / orange / white: readable with red-green colour blindness.
        Theme { classic: None, name: "Colourblind Friendly".into(), levels: vec![
            lk([255, 255, 255], Some([255, 255, 255]), [0, 90, 200, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 150, 0, 255]),
            lk([255, 170, 40], Some([255, 170, 40]), [20, 12, 0, 240]),
            lk([110, 180, 255], Some([110, 180, 255]), [0, 15, 35, 235]),
            lk([210, 210, 210], None, [10, 10, 10, 215]),
            lk([150, 150, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, name: "Soft".into(), levels: vec![
            lk([60, 30, 45], Some([255, 255, 255]), [255, 200, 215, 255]),
            lk([255, 205, 225], Some([200, 150, 180]), [60, 35, 50, 240]),
            lk([235, 225, 170], Some([170, 160, 110]), [45, 42, 25, 235]),
            lk([190, 220, 210], Some([110, 150, 140]), [25, 38, 35, 230]),
            lk([200, 195, 205], None, [22, 20, 26, 210]),
            lk([150, 145, 155], None, [0, 0, 0, 165]),
        ]},
    ]
}

/// Finds a theme by name among the built-in ones and yours.
pub fn find_theme<'a>(name: &str, custom: &'a [Theme], builtin: &'a [Theme]) -> Option<&'a Theme> {
    custom.iter().chain(builtin.iter()).find(|t| t.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{parse_filter, render_filter};

    const F: &str = "Show # $type->currency $tier->t1\r\n\tBaseType == \"Mirror of Kalandra\"\r\n\tSetFontSize 45\r\n\tSetTextColor 255 0 0 255\r\n\tSetBorderColor 255 0 0 255\r\n\tSetBackgroundColor 255 255 255 255\r\n\tPlayAlertSound 6 300\r\n\tPlayEffect Red\r\n\tMinimapIcon 0 Red Star\r\n\r\nShow # $type->currency $tier->t5\r\n\tBaseType == \"Chaos Orb\"\r\n\tSetFontSize 45\r\n\tSetTextColor 0 0 0 255\r\n\tSetBackgroundColor 213 159 0 255\r\n\tPlayEffect Yellow\r\n\tMinimapIcon 1 Yellow Circle\r\n\r\nShow # $type->decorators $tier->x\r\n\tRarity Rare\r\n\tPlayAlertSound 3 100\r\n\tContinue\r\n\r\nHide # $type->junk $tier->x\r\n\tBaseType == \"Scroll of Wisdom\"\r\n\tSetFontSize 18\r\n\tSetTextColor 1 1 1 255\r\n";

    #[test]
    fn levels_follow_neversink_beams() {
        let segs = parse_filter(F);
        let levels: Vec<Level> = segs.iter().filter_map(|s| if let Segment::Block(b) = s { Some(level_of(&style_of(&b.lines))) } else { None }).collect();
        assert_eq!(&levels[..2], &[Level::Chase, Level::Good]);
    }

    #[test]
    fn recolours_only_colour_rules_and_keeps_everything_else() {
        let all = builtin_themes();
        let t = all.iter().find(|t| t.name == "Midnight Neon").unwrap();
        let mut segs = parse_filter(F);
        apply_theme(&mut segs, t);
        let out = render_filter(&segs);
        // Mirror: Chase colours; size, sound, beam, icon untouched; CRLF kept.
        assert!(out.contains("\tSetFontSize 45\r\n\tSetTextColor 255 255 255 255\r\n\tSetBorderColor 255 40 140 255\r\n\tSetBackgroundColor 170 0 90 255\r\n\tPlayAlertSound 6 300"));
        assert!(out.contains("PlayEffect Red\r\n\tMinimapIcon 0 Red Star"));
        // Chaos (Good level) gets a border line even though it had none.
        assert!(out.contains("BaseType == \"Chaos Orb\"\r\n\tSetFontSize 45\r\n\tSetTextColor 255 225 90 255\r\n\tSetBorderColor 255 225 90 255"));
        // Sound-only add-on and hidden rules untouched.
        assert!(out.contains("Rarity Rare\r\n\tPlayAlertSound 3 100\r\n\tContinue"));
        assert!(out.contains("SetFontSize 18\r\n\tSetTextColor 1 1 1 255"));
        assert_eq!(out.matches("SetTextColor").count(), 3);
    }

    #[test]
    fn classic_colours_by_item_kind() {
        let f = "Show # $type->uniques $tier->t1\n\tRarity Unique\n\tSetFontSize 45\n\tSetTextColor 255 0 0 255\n\tPlayEffect Red\n\tMinimapIcon 0 Red Star\n\nShow # $type->uniques $tier->t3\n\tRarity Unique\n\tSetFontSize 40\n\tSetTextColor 175 96 37 255\n\nShow # $type->exotic->fractured $tier->fractt1\n\tFracturedItem True\n\tSetFontSize 45\n\tSetTextColor 0 240 190 255\n\tPlayEffect Yellow\n";
        let classic = builtin_themes().into_iter().find(|t| t.name == "Classic PoE").unwrap();
        let mut segs = parse_filter(f);
        apply_theme(&mut segs, &classic);
        let looks: Vec<(String, LevelLook)> = segs.iter().filter_map(|s| if let Segment::Block(b) = s {
            Some((category_of(b).to_string(), LevelLook {
                text: style_of(&b.lines).text.unwrap(), border: style_of(&b.lines).border, background: style_of(&b.lines).background.unwrap() }))
        } else { None }).collect();
        assert_eq!(looks[0].0, "Unique");
        assert_eq!(looks[2].0, "Fractured");
        // Top unique: solid, brighter, white border. Plain unique: see-through, no border.
        assert_eq!(looks[0].1.background[3], 255);
        assert_eq!(looks[0].1.border, Some([255, 255, 255, 255]));
        assert!(looks[1].1.background[3] < 200);
        assert!(looks[0].1.text[0] > looks[1].1.text[0]);
        // Both uniques keep the orange-brown family (red > green > blue).
        for l in &looks[..2] { assert!(l.1.text[0] > l.1.text[1] && l.1.text[1] > l.1.text[2]); }
        // Fractured is violet, not unique-coloured.
        assert!(looks[2].1.text[2] > looks[2].1.text[1]);
    }

    #[test]
    fn real_filter_gets_a_sensible_spread() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        let mut counts = [0usize; 6];
        for s in parse_filter(&text) {
            if let Segment::Block(b) = s {
                if b.visibility != "Hide" && b.lines.iter().any(|l| l.contains("SetTextColor") || l.contains("SetBackgroundColor")) {
                    counts[level_of(&style_of(&b.lines)).index()] += 1;
                }
            }
        }
        println!("Chase {} High {} Good {} Medium {} Low {} Junk {}", counts[0], counts[1], counts[2], counts[3], counts[4], counts[5]);
        assert!(counts.iter().all(|c| *c > 0));
        // Whole filter still round-trips with a theme applied, and every block still parses.
        let mut segs = parse_filter(&text);
        apply_theme(&mut segs, &builtin_themes()[3]);
        let again = parse_filter(&render_filter(&segs));
        assert_eq!(again.iter().filter(|s| matches!(s, Segment::Block(_))).count(), 786);
    }
}
