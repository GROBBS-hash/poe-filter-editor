//! Share packs: a filter's edits, saved looks, theme and price settings in one file or short code, so a friend
//! can put them on their own NeverSink base ("import"). The NeverSink file itself is never in the pack.

use crate::prices::PriceSettings;
use crate::profiles::{Game, Store};
use crate::rules::SavedLook;
use crate::themes::Theme;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// What a pack file says it is, so a random file isn't taken for one.
pub const KIND: &str = "poe-lodestone-pack";
/// Short codes start with this.
pub const CODE_PREFIX: &str = "LODE1:";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharePack {
    pub kind: String,
    pub version: u32,
    pub name: String,
    pub game: Game,
    /// NeverSink strictness it was made on (0 Soft .. 6 Uber Plus), if it used NeverSink's own file.
    #[serde(default)]
    pub strictness: Option<u8>,
    /// Every edit, the parent filter's first (as the filter is built).
    pub rules: Vec<crate::rules::Rule>,
    /// The saved looks those edits use.
    #[serde(default)]
    pub looks: Vec<SavedLook>,
    /// The theme's name; `custom_theme` holds it when it's one the sender made.
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub custom_theme: Option<Theme>,
    #[serde(default)]
    pub prices: Option<PriceSettings>,
    #[serde(default)]
    pub levelling: Option<crate::search::LevelOptions>,
}

/// A short summary to show before importing.
#[derive(Debug, Clone, Serialize)]
pub struct PackSummary {
    pub name: String,
    pub game: Game,
    pub strictness: Option<u8>,
    pub edits: usize,
    pub edits_off: usize,
    pub looks: Vec<String>,
    pub theme: Option<String>,
    pub prices: bool,
}

pub fn export(store: &Store, id: &str) -> Option<SharePack> {
    let p = store.profiles.get(id)?;
    let rules = store.resolved_rules(id);
    let mut looks: Vec<SavedLook> = Vec::new();
    for name in rules.iter().filter_map(|r| r.look()) {
        if looks.iter().any(|l| l.name == name) { continue; }
        if let Some(l) = store.find_look(name) { looks.push(l.clone()); }
    }
    let custom_theme = p.theme.as_ref().and_then(|t| store.themes.iter().find(|x| &x.name == t)).cloned();
    Some(SharePack {
        kind: KIND.into(), version: 1, name: p.name.clone(), game: p.game, strictness: p.strictness, rules, looks,
        theme: p.theme.clone(), custom_theme, prices: p.prices.clone(), levelling: p.levelling.clone(),
    })
}

pub fn summary(pack: &SharePack) -> PackSummary {
    PackSummary {
        name: pack.name.clone(), game: pack.game, strictness: pack.strictness, edits: pack.rules.len(),
        edits_off: pack.rules.iter().filter(|r| r.is_off()).count(),
        looks: pack.looks.iter().map(|l| l.name.clone()).collect(), theme: pack.theme.clone(),
        prices: pack.prices.as_ref().map(|p| p.enabled).unwrap_or(false),
    }
}

/// The pack as a file (readable JSON).
pub fn to_file(pack: &SharePack) -> String { serde_json::to_string_pretty(pack).unwrap_or_default() }

/// The pack as one line of text to paste in chat or Discord: "LODE1:" + packed, URL-safe base64.
pub fn to_code(pack: &SharePack) -> String {
    let json = serde_json::to_vec(pack).unwrap_or_default();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = z.write_all(&json);
    let bytes = z.finish().unwrap_or_default();
    format!("{CODE_PREFIX}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

/// Reads a pack from a code or a pack file's text.
pub fn from_text(text: &str) -> Result<SharePack, String> {
    let t = text.trim();
    let json = if let Some(code) = t.strip_prefix(CODE_PREFIX) {
        let clean: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(clean.trim_end_matches('='))
            .map_err(|_| "That code is cut off or mistyped. Copy the whole line again.".to_string())?;
        let mut s = String::new();
        flate2::read::ZlibDecoder::new(&bytes[..]).take(20_000_000).read_to_string(&mut s)
            .map_err(|_| "That code is cut off or mistyped. Copy the whole line again.".to_string())?;
        s
    } else if t.starts_with('{') {
        t.to_string()
    } else {
        return Err("That isn't a share code or pack file (codes start with LODE1:)".into());
    };
    let pack: SharePack = serde_json::from_str(&json).map_err(|_| "That pack couldn't be read. It may be from a newer version of the app.".to_string())?;
    if pack.kind != KIND { return Err("That isn't a share pack".into()); }
    if pack.version > 1 { return Err("That pack is from a newer version of the app: update first".into()); }
    Ok(pack)
}

/// Adds the pack as a new filter called `name`, built on `base_file` (your own NeverSink file).
/// Saved looks and a custom theme come along; one whose name you already use for something different is
/// renamed "<name> (<pack name>)" so yours stay as they are. Returns the new filter's id.
pub fn import(store: &mut Store, pack: &SharePack, name: &str, base_file: &str, strictness: Option<u8>) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() { return Err("Give the new filter a name".into()); }
    if store.profiles.values().any(|p| p.name.eq_ignore_ascii_case(name)) {
        return Err(format!("You already have a filter called {name}"));
    }
    if store.profiles.contains_key(&crate::profiles::slug(name)) {
        return Err(format!("Pick another name: “{name}” clashes with one of your filters"));
    }
    let mut rules = pack.rules.clone();
    for l in &pack.looks {
        // Their name, else "<name> (<pack name>)", "<name> (<pack name> 2)"...: the first that's free or already
        // holds this very look.
        let mut i = 1;
        loop {
            let n = match i { 1 => l.name.clone(), 2 => format!("{} ({})", l.name, pack.name), _ => format!("{} ({} {i})", l.name, pack.name) };
            match store.looks.iter().find(|x| x.name.eq_ignore_ascii_case(&n)) {
                Some(x) if x.style == l.style => { let n = x.name.clone(); crate::rules::rename_look(&mut rules, &l.name, &n); break; }
                Some(_) => i += 1,
                None => {
                    crate::rules::rename_look(&mut rules, &l.name, &n);
                    store.looks.push(SavedLook { name: n, style: l.style.clone() });
                    break;
                }
            }
        }
    }
    // Edits pointing at a look that didn't come along keep their colours and just stop following it.
    for r in rules.iter_mut() {
        if r.look().is_some_and(|n| store.find_look(n).is_none()) { r.drop_look(); }
    }
    let mut theme = pack.theme.clone();
    if let Some(t) = &pack.custom_theme {
        match store.themes.iter().find(|x| x.name == t.name) {
            Some(mine) if serde_json::to_string(mine).ok() == serde_json::to_string(t).ok() => {}
            Some(_) => {
                let n = format!("{} ({})", t.name, pack.name);
                store.themes.retain(|x| x.name != n);
                store.themes.push(Theme { name: n.clone(), ..t.clone() });
                theme = Some(n);
            }
            None => store.themes.push(t.clone()),
        }
    } else if let Some(t) = &theme {
        // A built-in theme the sender used: fine if this app has it too.
        if crate::themes::find_theme(t, &store.themes, &crate::themes::builtin_themes()).is_none() { theme = None; }
    }
    let id = store.create(name, pack.game, base_file);
    let p = store.profiles.get_mut(&id).ok_or("Couldn't add the filter")?;
    p.rules = rules;
    p.theme = theme;
    p.prices = pack.prices.clone();
    p.levelling = pack.levelling.clone();
    p.strictness = strictness;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::parse_item;
    use crate::rules::*;

    fn style(rgb: [u8; 4]) -> CustomStyle {
        CustomStyle { font_size: 45, text: rgb, border: None, background: [0, 0, 0, 255], beam: None, minimap: None, sound: Sound::Default, label_hidden: false }
    }

    fn sample() -> (Store, String) {
        let mut store = Store::default();
        let id = store.create("Friend's Strict", Game::Poe1, "NeverSink 3.filter");
        store.save_look(SavedLook { name: "Chase".into(), style: style([255, 0, 0, 255]) }, None).unwrap();
        let divine = parse_item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n").unwrap();
        let gold = parse_item("Item Class: Gold\nRarity: Currency\nGold\n").unwrap();
        let mut r = custom_rule(&divine, &style([1, 1, 1, 255]), &MatchOpts::default());
        r.use_look("Chase", &style([255, 0, 0, 255]));
        let mut off = quick_rule(&gold, Action::Hide, None, &MatchOpts::default());
        off.set_off(true);
        let p = store.profiles.get_mut(&id).unwrap();
        p.rules = vec![r, off];
        p.strictness = Some(3);
        (store, id)
    }

    #[test]
    fn code_and_file_round_trip() {
        let (store, id) = sample();
        let pack = export(&store, &id).unwrap();
        assert_eq!(pack.looks.len(), 1);
        let code = to_code(&pack);
        assert!(code.starts_with(CODE_PREFIX));
        assert!(code.len() < 1200, "short enough to paste: {}", code.len());
        for text in [code.clone(), format!("  {code}\n"), to_file(&pack)] {
            let back = from_text(&text).unwrap();
            assert_eq!(back.rules, pack.rules);
            assert_eq!(back.looks, pack.looks);
            assert_eq!(back.strictness, Some(3));
        }
        let s = summary(&pack);
        assert_eq!((s.edits, s.edits_off, s.looks.len()), (2, 1, 1));
        assert!(from_text("hello").is_err());
        assert!(from_text(&code[..code.len() / 2]).is_err());
        assert!(from_text(r#"{"kind":"something-else"}"#).is_err());
    }

    #[test]
    fn import_keeps_your_own_looks() {
        let (friend, id) = sample();
        let pack = export(&friend, &id).unwrap();
        let mut mine = Store::default();
        mine.create("League Start", Game::Poe1, "NeverSink 2.filter");
        // I already have a different "Chase" look: mine stays, theirs comes in renamed.
        mine.save_look(SavedLook { name: "Chase".into(), style: style([0, 255, 0, 255]) }, None).unwrap();
        let new = import(&mut mine, &pack, "From Sam", "NeverSink 3.filter", Some(3)).unwrap();
        assert_eq!(mine.find_look("Chase").unwrap().style.text, [0, 255, 0, 255]);
        let theirs = mine.find_look("Chase (Friend's Strict)").expect("renamed copy");
        assert_eq!(theirs.style.text, [255, 0, 0, 255]);
        let p = &mine.profiles[&new];
        assert_eq!(p.base_file, "NeverSink 3.filter");
        assert_eq!(p.rules[0].look(), Some("Chase (Friend's Strict)"));
        assert!(p.rules[1].is_off(), "switched-off edits stay off");
        assert_eq!(mine.look_uses("Chase (Friend's Strict)"), 1);
        // Same name again is refused; the same look again isn't copied twice.
        assert!(import(&mut mine, &pack, "from sam", "x.filter", None).is_err());
        import(&mut mine, &pack, "From Sam 2", "x.filter", None).unwrap();
        assert_eq!(mine.looks.len(), 2);
    }
}
