//! Profiles: one NeverSink base + your own edits = one output filter.
//! "League Start", "Strict 1", "Strict 2"... each can build on a parent.

use crate::filter::{parse_filter, render_filter};
use crate::prices::{price_blocks, PriceFile, PriceSettings};
use crate::rules::{apply_with_extra, Rule};
use crate::themes::{apply_theme, builtin_themes, find_theme, Theme};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Game { Poe1, Poe2 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game: Game,
    pub base_file: String,      // NeverSink/FilterBlade file it builds on
    pub output_file: String,    // what the game loads, e.g. "League Start.filter"
    pub parent: Option<String>, // inherits this profile's edits first
    pub rules: Vec<Rule>,
    /// Price tiering for this filter (off until you switch it on).
    #[serde(default)]
    pub prices: Option<PriceSettings>,
    /// Colour theme for NeverSink's labels (None = NeverSink's own look).
    #[serde(default)]
    pub theme: Option<String>,
    /// Built on NeverSink's own file at this strictness (0 Soft .. 6 Uber Plus),
    /// downloaded and kept up to date by the app. None = your own file.
    #[serde(default)]
    pub strictness: Option<u8>,
    /// Levelling build choices (Presets tab); the rules themselves are in `rules`.
    #[serde(default)]
    pub levelling: Option<crate::search::LevelOptions>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    pub profiles: BTreeMap<String, Profile>,
    pub active: Option<String>,
    /// Themes you've made.
    #[serde(default)]
    pub themes: Vec<Theme>,
    #[serde(default)]
    pub settings: Settings,
}

/// App settings (Settings tab).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub key_quick_edit: String,
    pub key_batch: String,
    pub key_reload: String,
    pub key_cycle: String,
    /// F5 cycling through your filters: off unless you switch it on.
    pub cycle_enabled: bool,
    /// Area level to assume when the game's log can't be found.
    pub fallback_area_level: u32,
    /// Optional Claude API key for the "ask" box (kept only on this PC).
    pub claude_api_key: String,
    /// Hotkey for the build overlay (PoB gems, gear, tree).
    pub key_build: String,
    /// Your Path of Building link or code, and the stage you're on.
    pub pob_link: String,
    pub pob_stage: Option<usize>,
    /// Pick the build stage from where you are in the game (on unless you pick one yourself).
    pub pob_follow: bool,
    /// Price check the item under the mouse (like Awakened PoE Trade).
    pub key_price: String,
    /// Map mods to warn about on Ctrl+D (None = every known danger).
    pub map_avoid: Option<Vec<String>>,
    /// Trade helper: show incoming trade whispers with Invite / Trade / Kick / Thanks.
    pub trade_helper: bool,
    /// What the Thanks button whispers.
    pub thanks_message: String,
    /// Chat hotkeys: one key types one command (e.g. F7 -> /hideout).
    pub chat_keys: Vec<ChatKey>,
    /// Map regex / trade tool choices.
    pub map_tool: crate::mapregex::MapTool,
    /// Wealth tracker: your PoE account ("Name#1234") and POESESSID (kept on this PC, only sent to pathofexile.com).
    pub poe_account: String,
    pub poesessid: String,
    /// Tabs an earnings session watches (your dump tabs).
    pub wealth_tabs: Vec<usize>,
    /// "Worth selling" from this many chaos.
    pub wealth_min: f64,
    /// Minutes between earnings session checks.
    pub earn_minutes: u32,
    /// Ctrl+D: compare the item with your PoB build's gear in that slot.
    pub price_compare: bool,
    /// Ctrl+D: show where each mod rolled in its range (low / high roll).
    pub price_rolls: bool,
    /// Ctrl+D searches the trade site straight away for every item (off: like Awakened, rares and gear wait for Search).
    pub price_instant_all: bool,
    /// Ctrl+D prices only from instant buyout listings (sellers you can't whisper or haggle with). On by default.
    pub price_buyout_only: bool,
    /// Show/hide the atlas strategy window over the game.
    pub key_atlas: String,
    /// Open the build window on its Tree tab when the game's passive tree key is pressed (and close it again).
    pub build_with_tree: bool,
    /// The game's passive skill tree key (P unless changed in the game's options).
    pub key_tree: String,
    /// Play windows (22 Nov): levelling overlay, map tracker, search strings, cheat sheets, notes, and the wiki key.
    /// Empty = no key. A key another program owns is skipped with a warning.
    pub key_level: String,
    pub key_tracker: String,
    pub key_searches: String,
    pub key_sheet: String,
    pub key_notes: String,
    pub key_wiki: String,
}

/// A hotkey that types one chat command in game.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChatKey {
    pub key: String,
    pub command: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self { key_quick_edit: "F2".into(), key_batch: "F3".into(), key_reload: "F4".into(), key_cycle: "F5".into(),
            cycle_enabled: false, fallback_area_level: 83, claude_api_key: String::new(),
            key_build: "F6".into(), pob_link: String::new(), pob_stage: None, pob_follow: true,
            key_price: "Ctrl+D".into(), map_avoid: None,
            trade_helper: true, thanks_message: "thanks, good luck!".into(), chat_keys: vec![], map_tool: Default::default(),
            poe_account: String::new(), poesessid: String::new(), wealth_tabs: vec![], wealth_min: 10.0, earn_minutes: 3,
            price_compare: true, price_rolls: true, price_instant_all: false, price_buyout_only: true, build_with_tree: false, key_tree: "P".into(), key_atlas: "F7".into(),
            key_level: "F8".into(), key_tracker: "F9".into(), key_searches: "F10".into(), key_sheet: "F11".into(), key_notes: String::new(), key_wiki: "Alt+W".into() }
    }
}

pub fn slug(name: &str) -> String {
    name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

impl Store {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Writes to a temp file then swaps it in, so a crash never leaves a half file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(tmp, path)
    }

    pub fn create(&mut self, name: &str, game: Game, base_file: &str) -> String {
        let id = slug(name);
        self.profiles.insert(id.clone(), Profile {
            id: id.clone(), name: name.into(), game, base_file: base_file.into(),
            output_file: format!("{name}.filter"), parent: None, rules: vec![], prices: None, theme: None, strictness: None, levelling: None,
        });
        self.active.get_or_insert(id.clone());
        id
    }

    /// "Save as new". `inherit` = the copy keeps following the original's edits.
    pub fn fork(&mut self, from: &str, new_name: &str, inherit: bool) -> Option<String> {
        let src = self.profiles.get(from)?.clone();
        let id = slug(new_name);
        let p = Profile {
            id: id.clone(),
            name: new_name.into(),
            output_file: format!("{new_name}.filter"),
            parent: inherit.then(|| from.to_string()),
            rules: if inherit { vec![] } else { src.rules.clone() },
            ..src
        };
        self.profiles.insert(id.clone(), p);
        Some(id)
    }

    /// Parent edits first, then this profile's own, so the child wins.
    pub fn resolved_rules(&self, id: &str) -> Vec<Rule> {
        let mut chain = Vec::new();
        let mut cur = self.profiles.get(id);
        while let Some(p) = cur {
            if chain.len() > 20 { break; }
            chain.push(p);
            cur = p.parent.as_deref().and_then(|pid| self.profiles.get(pid));
        }
        chain.iter().rev().flat_map(|p| p.rules.clone()).collect()
    }
}

/// Rebuilds a profile's filter from its base file plus all edits, backing up
/// the previous output first. Returns warnings to show the user.
pub fn build(store: &Store, id: &str, filter_dir: &Path) -> io::Result<Vec<String>> {
    build_with_prices(store, id, filter_dir, None)
}

/// Rebuild including price tiers, when the profile has them switched on.
pub fn build_with_prices(store: &Store, id: &str, filter_dir: &Path, prices: Option<&PriceFile>) -> io::Result<Vec<String>> {
    let p = store.profiles.get(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "profile not found"))?;
    let (text, warnings) = render_profile(store, id, filter_dir, prices, None)?;

    let out = filter_dir.join(&p.output_file);
    if out.exists() {
        let backups = filter_dir.join("LiveEditorBackups");
        fs::create_dir_all(&backups)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        fs::copy(&out, backups.join(format!("{}.{stamp}.filter", p.id)))?;
        prune_backups(&backups, &p.id, 30)?;
    }
    let tmp = out.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, out)?;
    Ok(warnings)
}

/// The finished filter text for a profile, without saving it. `theme`
/// overrides the profile's theme (Some(None) = NeverSink's own colours),
/// which is how a theme is previewed before switching to it.
pub fn render_profile(store: &Store, id: &str, filter_dir: &Path, prices: Option<&PriceFile>,
    theme: Option<Option<&str>>) -> io::Result<(String, Vec<String>)> {
    let p = store.profiles.get(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "profile not found"))?;
    let base = fs::read_to_string(filter_dir.join(&p.base_file))?;
    let nl = if base.contains("\r\n") { "\r\n" } else { "\n" };
    let mut segs = parse_filter(&base);
    let theme_name = match theme { Some(t) => t, None => p.theme.as_deref() };
    if let Some(theme) = theme_name.and_then(|n| find_theme(n, &store.themes, &builtin_themes()).cloned()) {
        apply_theme(&mut segs, &theme);
    }
    let extra = match (&p.prices, prices) {
        (Some(settings), Some(file)) => price_blocks(settings, file, nl),
        _ => String::new(),
    };
    let warnings = apply_with_extra(&mut segs, &store.resolved_rules(id), &extra, nl);
    Ok((render_filter(&segs), warnings))
}

fn prune_backups(dir: &Path, id: &str, keep: usize) -> io::Result<()> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{id}.")))
        .collect();
    files.sort_by_key(|e| e.file_name());
    while files.len() > keep {
        fs::remove_file(files.remove(0).path())?;
    }
    Ok(())
}
