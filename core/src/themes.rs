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
        let look = &theme.levels[level_of(&st).index()];
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
        Theme { name: "Midnight Neon".into(), levels: vec![
            lk([255, 255, 255], Some([255, 40, 140]), [170, 0, 90, 255]),
            lk([255, 120, 210], Some([255, 120, 210]), [35, 0, 35, 245]),
            lk([255, 225, 90], Some([255, 225, 90]), [28, 22, 0, 240]),
            lk([90, 235, 255], Some([90, 235, 255]), [0, 22, 32, 235]),
            lk([200, 200, 225], Some([90, 90, 125]), [10, 10, 22, 220]),
            lk([150, 150, 165], None, [0, 0, 0, 170]),
        ]},
        Theme { name: "Ember".into(), levels: vec![
            lk([255, 255, 255], Some([255, 200, 60]), [200, 40, 0, 255]),
            lk([255, 150, 40], Some([255, 150, 40]), [45, 12, 0, 245]),
            lk([255, 210, 120], Some([200, 140, 60]), [35, 20, 5, 240]),
            lk([240, 200, 170], Some([150, 100, 70]), [25, 15, 10, 230]),
            lk([190, 160, 140], None, [15, 10, 8, 210]),
            lk([140, 125, 115], None, [0, 0, 0, 170]),
        ]},
        Theme { name: "Frost".into(), levels: vec![
            lk([0, 20, 60], Some([255, 255, 255]), [200, 240, 255, 255]),
            lk([255, 255, 255], Some([120, 200, 255]), [20, 70, 140, 245]),
            lk([170, 225, 255], Some([170, 225, 255]), [8, 30, 60, 240]),
            lk([200, 230, 245], Some([90, 140, 180]), [10, 22, 36, 230]),
            lk([170, 185, 200], None, [8, 12, 18, 210]),
            lk([125, 135, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { name: "High Contrast".into(), levels: vec![
            lk([0, 0, 0], Some([255, 0, 0]), [255, 255, 255, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 230, 0, 255]),
            lk([255, 230, 0], Some([255, 230, 0]), [0, 0, 0, 255]),
            lk([255, 255, 255], Some([255, 255, 255]), [0, 0, 0, 255]),
            lk([220, 220, 220], None, [0, 0, 0, 230]),
            lk([150, 150, 150], None, [0, 0, 0, 200]),
        ]},
        // Blue / orange / white: readable with red-green colour blindness.
        Theme { name: "Colourblind Friendly".into(), levels: vec![
            lk([255, 255, 255], Some([255, 255, 255]), [0, 90, 200, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 150, 0, 255]),
            lk([255, 170, 40], Some([255, 170, 40]), [20, 12, 0, 240]),
            lk([110, 180, 255], Some([110, 180, 255]), [0, 15, 35, 235]),
            lk([210, 210, 210], None, [10, 10, 10, 215]),
            lk([150, 150, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { name: "Soft".into(), levels: vec![
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
        let t = &builtin_themes()[0];
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
