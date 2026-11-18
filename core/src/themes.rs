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
    /// One of NeverSink's official styles, copied rule by rule.
    #[serde(default)]
    pub neversink_style: Option<String>,
    /// "poe1" / "poe2" when a theme only fits one game.
    #[serde(default)]
    pub game: Option<String>,
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
fn key_of(l: &str) -> &str {
    l.trim_start().split_whitespace().next().unwrap_or("")
}
fn is_colour(l: &str) -> bool {
    matches!(key_of(l), "SetTextColor" | "SetBorderColor" | "SetBackgroundColor")
}

/// Puts a colour line in a block: replaces an existing one, or adds it after
/// the font size (keeping the block's indentation and line endings).
fn set_line(b: &mut crate::filter::Block, key: &str, c: [u8; 4]) {
    let sample = b.lines.iter().find(|l| is_colour(l) || key_of(l) == "SetFontSize").cloned()
        .or_else(|| b.lines.first().cloned()).unwrap_or_default();
    let indent: String = sample.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
    let nl = if sample.ends_with("\r\n") { "\r\n" } else { "\n" };
    let line = line_of(key, c, &indent, nl);
    if let Some(i) = b.lines.iter().position(|l| key_of(l) == key) {
        b.lines[i] = line;
    } else {
        let at = b.lines.iter().rposition(|l| is_colour(l) || key_of(l) == "SetFontSize").map(|i| i + 1)
            .or_else(|| b.lines.iter().position(|l| key_of(l) == "Continue"))
            .unwrap_or(b.lines.len());
        b.lines.insert(at, line);
    }
}

/// Recolours every NeverSink rule that sets colours. Rules that set none
/// (e.g. small add-ons that only change a sound) are left as they are.
pub fn apply_theme(segs: &mut [Segment], theme: &Theme) {
    let style = theme.neversink_style.as_deref().and_then(style_data);
    let plain = theme.neversink_style.as_deref().map(|s| s.ends_with("tabula_rasa")).unwrap_or(false);
    if theme.neversink_style.is_none() && theme.levels.len() < 6 {
        return;
    }
    // NeverSink's style files are matched rule by rule: "type|tier#n" (the nth rule with that tag).
    let mut seen: std::collections::HashMap<String, u32> = Default::default();
    for seg in segs.iter_mut() {
        let Segment::Block(b) = seg else { continue };
        let key = (!b.type_path.is_empty()).then(|| {
            let base = format!("{}|{}", b.type_path.join("->"), b.tier.clone().unwrap_or_default());
            let n = seen.entry(base.clone()).or_insert(0);
            *n += 1;
            format!("{base}#{n}")
        });
        if b.visibility == "Hide" || !b.lines.iter().any(|l| is_colour(l)) {
            continue;
        }
        let st = style_of(&b.lines);
        if st.label_hidden {
            continue; // icon-only labels stay invisible
        }
        if plain {
            // Tabula Rasa: plain game-default labels.
            b.lines.retain(|l| !is_colour(l));
            continue;
        }
        if theme.neversink_style.is_some() {
            let Some(look) = style.and_then(|s| key.as_ref().and_then(|k| s.rules.get(k)).map(|i| &s.palette[*i])) else { continue };
            if let Some(c) = look[0] { set_line(b, "SetTextColor", c); }
            if let Some(c) = look[1] { set_line(b, "SetBorderColor", c); }
            if let Some(c) = look[2] { set_line(b, "SetBackgroundColor", c); }
            continue;
        }
        let look = look_for(theme, b, &st);
        b.lines.retain(|l| !is_colour(l));
        set_line(b, "SetTextColor", look.text);
        set_line(b, "SetBorderColor", look.border.unwrap_or([0, 0, 0, 0]));
        set_line(b, "SetBackgroundColor", look.background);
    }
}

// ---------- NeverSink's own styles ----------

/// One of NeverSink's official colour styles: every rule's exact colours.
#[derive(Debug, Deserialize)]
pub struct StyleData {
    pub id: String,
    pub name: String,
    pub game: String,
    /// Distinct looks: [text, border, background], any may be missing.
    pub palette: Vec<[Option<[u8; 4]>; 3]>,
    /// "type|tier#n" -> index into the palette.
    pub rules: std::collections::HashMap<String, usize>,
}

macro_rules! styles {
    ($($id:literal),* $(,)?) => { &[$(($id, include_str!(concat!("../data/styles/", $id, ".json")))),*] };
}
/// NeverSink's styles (MIT licence, see data/styles/NEVERSINK-LICENSE).
const STYLE_FILES: &[(&str, &str)] = styles!(
    "poe1_abyss", "poe1_alloy", "poe1_azurite", "poe1_cobalt", "poe1_crimson", "poe1_elder", "poe1_gaia",
    "poe1_krangled", "poe1_lunaris", "poe1_necros", "poe1_purple", "poe1_vaal", "poe1_velvet", "poe1_wow",
    "poe2_aura", "poe2_cobalt", "poe2_darkmode", "poe2_mythic", "poe2_vaal", "poe2_zen",
);

pub fn style_data(id: &str) -> Option<&'static StyleData> {
    static ALL: std::sync::OnceLock<Vec<StyleData>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| STYLE_FILES.iter().filter_map(|(_, j)| serde_json::from_str(j).ok()).collect())
        .iter().find(|s| s.id == id)
}

/// A NeverSink style as a theme you can pick (and preview by level).
fn neversink_theme(id: &str, game: &str) -> Option<Theme> {
    let (name, levels) = if id.ends_with("tabula_rasa") {
        ("Tabula Rasa".to_string(), Level::ALL.iter().map(|_| LevelLook { text: [200, 200, 200, 255], border: None, background: [0, 0, 0, 200] }).collect())
    } else {
        let s = style_data(id)?;
        (s.name.clone(), preview_levels(s))
    };
    let name = if game == "poe2" { format!("NeverSink: {name} (PoE 2)") } else { format!("NeverSink: {name}") };
    Some(Theme { name, levels, classic: None, neversink_style: Some(id.into()), game: Some(game.into()) })
}

/// Sample looks from a style for the six preview labels (Mirror .. Wisdom).
fn preview_levels(s: &StyleData) -> Vec<LevelLook> {
    let pick = |keys: &[&str]| -> LevelLook {
        let look = keys.iter().find_map(|k| s.rules.get(*k)).map(|i| s.palette[*i]).unwrap_or([None, None, None]);
        LevelLook { text: look[0].unwrap_or([200, 200, 200, 255]), border: look[1].filter(|b| b[3] > 0), background: look[2].unwrap_or([0, 0, 0, 200]) }
    };
    vec![
        pick(&["currency|t1exalted#1", "currency->exalted|t1#1", "currency|t1#1"]),
        pick(&["currency|t2divine#1", "currency|t2#1", "uniques|t2#1"]),
        pick(&["currency|t4chaos#1", "currency|t3#1"]),
        pick(&["exoticbases|t3#1", "currency|t4#1", "maps|maps_d_t5#1"]),
        pick(&["rr|t1#1", "rareid|weapon_phys#1", "currency|t5#1"]),
        pick(&["currency|twisdom#1", "currency|t6#1", "currency|t7#1", "hidelayer|final#1"]),
    ]
}

fn lk(text: [u8; 3], border: Option<[u8; 3]>, bg: [u8; 4]) -> LevelLook {
    LevelLook { text: [text[0], text[1], text[2], 255], border: border.map(|b| [b[0], b[1], b[2], 255]), background: bg }
}

/// Ready-made themes. Copy one to make your own.
pub fn builtin_themes() -> Vec<Theme> {
    let ours: Vec<Theme> = vec![
        Theme { name: "Classic PoE".into(), classic: Some(classic_hues()), neversink_style: None, game: None,
            levels: Level::ALL.iter().map(|l| classic_look([210, 210, 210], *l)).collect() },
        Theme { classic: None, neversink_style: None, game: None, name: "Midnight Neon".into(), levels: vec![
            lk([255, 255, 255], Some([255, 40, 140]), [170, 0, 90, 255]),
            lk([255, 120, 210], Some([255, 120, 210]), [35, 0, 35, 245]),
            lk([255, 225, 90], Some([255, 225, 90]), [28, 22, 0, 240]),
            lk([90, 235, 255], Some([90, 235, 255]), [0, 22, 32, 235]),
            lk([200, 200, 225], Some([90, 90, 125]), [10, 10, 22, 220]),
            lk([150, 150, 165], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, neversink_style: None, game: None, name: "Ember".into(), levels: vec![
            lk([255, 255, 255], Some([255, 200, 60]), [200, 40, 0, 255]),
            lk([255, 150, 40], Some([255, 150, 40]), [45, 12, 0, 245]),
            lk([255, 210, 120], Some([200, 140, 60]), [35, 20, 5, 240]),
            lk([240, 200, 170], Some([150, 100, 70]), [25, 15, 10, 230]),
            lk([190, 160, 140], None, [15, 10, 8, 210]),
            lk([140, 125, 115], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, neversink_style: None, game: None, name: "Frost".into(), levels: vec![
            lk([0, 20, 60], Some([255, 255, 255]), [200, 240, 255, 255]),
            lk([255, 255, 255], Some([120, 200, 255]), [20, 70, 140, 245]),
            lk([170, 225, 255], Some([170, 225, 255]), [8, 30, 60, 240]),
            lk([200, 230, 245], Some([90, 140, 180]), [10, 22, 36, 230]),
            lk([170, 185, 200], None, [8, 12, 18, 210]),
            lk([125, 135, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, neversink_style: None, game: None, name: "High Contrast".into(), levels: vec![
            lk([0, 0, 0], Some([255, 0, 0]), [255, 255, 255, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 230, 0, 255]),
            lk([255, 230, 0], Some([255, 230, 0]), [0, 0, 0, 255]),
            lk([255, 255, 255], Some([255, 255, 255]), [0, 0, 0, 255]),
            lk([220, 220, 220], None, [0, 0, 0, 230]),
            lk([150, 150, 150], None, [0, 0, 0, 200]),
        ]},
        // Blue / orange / white: readable with red-green colour blindness.
        Theme { classic: None, neversink_style: None, game: None, name: "Colourblind Friendly".into(), levels: vec![
            lk([255, 255, 255], Some([255, 255, 255]), [0, 90, 200, 255]),
            lk([0, 0, 0], Some([0, 0, 0]), [255, 150, 0, 255]),
            lk([255, 170, 40], Some([255, 170, 40]), [20, 12, 0, 240]),
            lk([110, 180, 255], Some([110, 180, 255]), [0, 15, 35, 235]),
            lk([210, 210, 210], None, [10, 10, 10, 215]),
            lk([150, 150, 150], None, [0, 0, 0, 170]),
        ]},
        Theme { classic: None, neversink_style: None, game: None, name: "Soft".into(), levels: vec![
            lk([60, 30, 45], Some([255, 255, 255]), [255, 200, 215, 255]),
            lk([255, 205, 225], Some([200, 150, 180]), [60, 35, 50, 240]),
            lk([235, 225, 170], Some([170, 160, 110]), [45, 42, 25, 235]),
            lk([190, 220, 210], Some([110, 150, 140]), [25, 38, 35, 230]),
            lk([200, 195, 205], None, [22, 20, 26, 210]),
            lk([150, 145, 155], None, [0, 0, 0, 165]),
        ]},
    ];
    ours.into_iter().chain(palette_themes()).chain(neversink_themes()).collect()
}

fn hex(h: &str) -> [u8; 3] {
    let v = u32::from_str_radix(h.trim_start_matches('#'), 16).unwrap_or(0xc8c8c8);
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// A theme from a popular colour palette: [chase text, chase background,
/// chase border], then the accent for High, Good, Medium, Low, Junk, and the
/// palette's own background colour.
fn palette(name: &str, chase: (&str, &str, &str), accents: [&str; 5], bg: &str) -> Theme {
    let b = hex(bg);
    let on_bg = |a: u8| [b[0], b[1], b[2], a];
    let accent = |h: &str, border: bool, a: u8| LevelLook {
        text: { let c = hex(h); [c[0], c[1], c[2], 255] },
        border: border.then(|| { let c = hex(h); [c[0], c[1], c[2], 255] }),
        background: on_bg(a),
    };
    let ct = hex(chase.0); let cb = hex(chase.1); let cr = hex(chase.2);
    Theme {
        name: name.into(), classic: None, neversink_style: None, game: None,
        levels: vec![
            LevelLook { text: [ct[0], ct[1], ct[2], 255], border: Some([cr[0], cr[1], cr[2], 255]), background: [cb[0], cb[1], cb[2], 255] },
            accent(accents[0], true, 245),
            accent(accents[1], true, 240),
            accent(accents[2], true, 230),
            accent(accents[3], false, 210),
            LevelLook { text: { let c = hex(accents[4]); [c[0], c[1], c[2], 255] }, border: None, background: [0, 0, 0, 170] },
        ],
    }
}

/// Themes from popular colour palettes (their published colours).
fn palette_themes() -> Vec<Theme> {
    vec![
        palette("Dracula", ("#f8f8f2", "#ff5555", "#f8f8f2"), ["#ff79c6", "#f1fa8c", "#8be9fd", "#bd93f9", "#6272a4"], "#282a36"),
        palette("Nord", ("#eceff4", "#bf616a", "#eceff4"), ["#d08770", "#ebcb8b", "#88c0d0", "#d8dee9", "#616e88"], "#2e3440"),
        palette("Catppuccin", ("#1e1e2e", "#f38ba8", "#f5e0dc"), ["#fab387", "#f9e2af", "#89dceb", "#cba6f7", "#7f849c"], "#1e1e2e"),
        palette("Gruvbox", ("#282828", "#fabd2f", "#fb4934"), ["#fe8019", "#b8bb26", "#83a598", "#ebdbb2", "#928374"], "#282828"),
        palette("Tokyo Night", ("#1a1b26", "#f7768e", "#c0caf5"), ["#ff9e64", "#e0af68", "#7dcfff", "#bb9af7", "#565f89"], "#1a1b26"),
        palette("Solarized", ("#fdf6e3", "#dc322f", "#fdf6e3"), ["#cb4b16", "#b58900", "#2aa198", "#93a1a1", "#586e75"], "#002b36"),
        palette("Rosé Pine", ("#191724", "#eb6f92", "#e0def4"), ["#f6c177", "#ebbcba", "#9ccfd8", "#c4a7e7", "#6e6a86"], "#191724"),
        palette("Monokai", ("#272822", "#f92672", "#f8f8f2"), ["#fd971f", "#e6db74", "#66d9ef", "#a6e22e", "#75715e"], "#272822"),
        palette("One Dark", ("#282c34", "#e06c75", "#abb2bf"), ["#d19a66", "#e5c07b", "#61afef", "#c678dd", "#5c6370"], "#282c34"),
    ]
}

/// NeverSink's official styles for both games (plus Tabula Rasa: plain labels).
fn neversink_themes() -> Vec<Theme> {
    let mut ids: Vec<(&str, &str)> = STYLE_FILES.iter().map(|(id, _)| (*id, if id.starts_with("poe1") { "poe1" } else { "poe2" })).collect();
    ids.push(("poe1_tabula_rasa", "poe1"));
    ids.into_iter().filter_map(|(id, g)| neversink_theme(id, g)).collect()
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
    fn neversink_styles_copy_his_colours_exactly() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        let all = builtin_themes();
        println!("{} built-in themes: {:?}", all.len(), all.iter().map(|t| t.name.as_str()).collect::<Vec<_>>());
        let crimson = all.iter().find(|t| t.name == "NeverSink: Crimson").unwrap();
        let mut segs = parse_filter(&text);
        apply_theme(&mut segs, crimson);
        let out = render_filter(&segs);
        // His own Crimson file's Divine/Exalted tier text colour.
        let t1 = out.split("$type->currency $tier->t1exalted").nth(1).unwrap().split("\r\n\r\n").next().unwrap();
        assert!(t1.contains("SetTextColor 215 20 20 255"), "{t1}");
        // Still a valid filter with every rule.
        assert_eq!(parse_filter(&out).iter().filter(|s| matches!(s, Segment::Block(_))).count(), 786);
        // Tabula Rasa: no colour lines left on shown rules.
        let tabula = all.iter().find(|t| t.name == "NeverSink: Tabula Rasa").unwrap();
        let mut segs = parse_filter(&text);
        apply_theme(&mut segs, tabula);
        let shown_coloured = segs.iter().filter(|s| matches!(s, Segment::Block(b) if b.visibility == "Show"
            && b.lines.iter().any(|l| l.contains("SetTextColor")) && !style_of(&b.lines).label_hidden)).count();
        assert_eq!(shown_coloured, 0);
        // Every theme has its six preview looks.
        assert!(all.iter().all(|t| t.levels.len() == 6));
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
