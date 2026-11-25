//! Death notes (Play tools page, and the Ctrl+D map check). Logic in `filter_core::deaths`. Kept in deaths.json.

use super::*;
use filter_core::deaths::{self, Death, Patterns};

static DEATHS: Mutex<Option<Vec<Death>>> = Mutex::new(None);
const KEEP: usize = 300;

fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("deaths.json") }

fn all(app: &AppHandle) -> Vec<Death> {
    let mut d = DEATHS.lock().unwrap();
    if d.is_none() { *d = Some(std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()); }
    d.clone().unwrap_or_default()
}

/// The map tracker saw you die in this map.
pub fn note(app: &AppHandle, death: Death) {
    let mut list = all(app);
    // The log read again after a restart isn't a second death.
    if list.iter().any(|d| d.time == death.time && d.map == death.map) { return; }
    list.push(death);
    if list.len() > KEEP { let n = list.len() - KEEP; list.drain(..n); }
    if let Ok(t) = serde_json::to_string(&list) {
        let tmp = path(app).with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, path(app)); }
    }
    *DEATHS.lock().unwrap() = Some(list);
    let _ = app.emit("deaths-changed", ());
}

/// Your map runs, the one you're in counted too once you've died in it.
fn runs(app: &AppHandle) -> Vec<filter_core::journey::MapRun> { play_app::maps_with_current(app) }

#[derive(Serialize)]
pub struct DeathView {
    /// Newest first.
    recent: Vec<Death>,
    patterns: Patterns,
    /// For each mod in `patterns`: it's on your can't-run list, or can be put there.
    avoid: Vec<Avoid>,
}

#[derive(Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Avoid { On, Can, No }

/// The map mod these words are, and whether it's ticked as "can't run".
fn mod_id(mods: &[filter_core::mapregex::MapModEntry], text: &str) -> Option<i64> {
    let s = filter_core::maps::shape(text);
    mods.iter().find(|m| m.text.split(" / ").any(|p| filter_core::maps::shape(p) == s)).map(|m| m.id)
}

fn bad_list(state: &AppState, mods: &[filter_core::mapregex::MapModEntry]) -> Vec<i64> {
    let s = state.store.lock().unwrap().settings.clone();
    s.map_tool.bad.clone().unwrap_or_else(|| filter_core::mapregex::default_bad(mods, s.map_avoid.as_deref()))
}

/// Already on your can't-run list (or not on any list, so nothing to add).
pub fn avoided(app: &AppHandle, text: &str) -> bool {
    let st = app.state::<AppState>();
    let (mods, _) = map_mods(&st);
    mod_id(&mods, text).is_none_or(|id| bad_list(&st, &mods).contains(&id))
}

#[tauri::command(async)]
pub fn deaths_get(app: AppHandle) -> DeathView {
    let mut recent = all(&app);
    recent.reverse();
    recent.truncate(30);
    let patterns = deaths::patterns(&runs(&app));
    let st = app.state::<AppState>();
    let (mods, _) = map_mods(&st);
    let bad = bad_list(&st, &mods);
    let avoid = patterns.mods.iter().map(|r| match mod_id(&mods, &r.name) { Some(id) if bad.contains(&id) => Avoid::On, Some(_) => Avoid::Can, None => Avoid::No }).collect();
    DeathView { recent, patterns, avoid }
}

/// "Can't run this": ticks the mod on the Maps page.
#[tauri::command(async)]
pub fn deaths_avoid(app: AppHandle, text: String, on: bool) -> Result<DeathView, String> {
    let st = app.state::<AppState>();
    let (mods, _) = map_mods(&st);
    let id = mod_id(&mods, &text).ok_or("That mod isn't on the Maps page's list")?;
    let mut bad = bad_list(&st, &mods);
    bad.retain(|b| *b != id);
    if on { bad.push(id); }
    {
        let mut s = st.store.lock().unwrap();
        s.settings.map_tool.bad = Some(bad);
        s.save(&st.store_path).map_err(err)?;
    }
    Ok(deaths_get(app))
}

/// The map check: (maps, maps you died in) for each mod, where you've died with it.
#[tauri::command(async)]
pub fn deaths_for(app: AppHandle, mods: Vec<String>) -> Vec<Option<(usize, usize)>> { deaths::history_of(&runs(&app), &mods) }
