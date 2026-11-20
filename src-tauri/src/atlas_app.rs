//! Atlas strategies in the app: import from maxroll or a pathofexile.com atlas tree link,
//! keep them under a name (atlas.json in the data folder), and show the chosen one over
//! the game in the "atlas" window (hotkey F7 by default) without taking focus from it.

use super::*;
use filter_core::atlas::{self, AtlasStore, Link, Strategy};

const UA: &str = "poe-filter-editor/0.1 (local app)";

fn store_path(state: &AppState) -> PathBuf { state.data_dir.join("atlas.json") }

static STORE: Mutex<Option<AtlasStore>> = Mutex::new(None);

/// Runs `f` on the saved strategies, saves them, and tells the in-game window.
fn with_store<T>(app: &AppHandle, save: bool, f: impl FnOnce(&mut AtlasStore) -> T) -> Result<T, String> {
    let state = app.state::<AppState>();
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(|| AtlasStore::load(&store_path(&state)));
    let out = f(s);
    if save {
        s.save(&store_path(&state))?;
        let _ = app.emit("atlas-changed", ());
    }
    Ok(out)
}

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

#[tauri::command]
pub fn atlas_list(app: AppHandle) -> Result<AtlasStore, String> { with_store(&app, false, |s| s.clone()) }

/// Reads a strategy from a link (one request to maxroll; official links need none) and saves it.
#[tauri::command(async)]
pub fn atlas_import(app: AppHandle, link: String, name: String) -> Result<AtlasStore, String> {
    let mut strat: Strategy = match atlas::parse_link(&link)? {
        Link::Official(code) => atlas::from_official(&code, link.trim())?,
        Link::Maxroll(id) => {
            let resp = ureq::get(&atlas::maxroll_api_url(&id)).set("User-Agent", UA).set("Accept", "application/json")
                .timeout(Duration::from_secs(20)).call();
            let v: serde_json::Value = match resp {
                Ok(r) => r.into_json().map_err(|_| "maxroll sent something unexpected".to_string())?,
                Err(ureq::Error::Status(404, _)) => return Err("maxroll doesn't know that strategy (check the link)".into()),
                Err(ureq::Error::Status(code, _)) => return Err(format!("maxroll said no (error {code}); try again in a minute")),
                Err(_) => return Err("Couldn't reach maxroll (is the internet on?)".into()),
            };
            let page = if link.contains("maxroll.gg/poe/") { link.trim().to_string() } else { format!("https://maxroll.gg/poe/farming-strategy-planner/{id}") };
            atlas::from_maxroll(&v, &page)?
        }
    };
    strat.imported = now();
    with_store(&app, true, |s| { s.add(strat, &name); s.clone() })
}

#[tauri::command]
pub fn atlas_rename(app: AppHandle, id: u64, name: String) -> Result<AtlasStore, String> {
    with_store(&app, true, |s| s.rename(id, &name).map(|_| s.clone()))?
}

#[tauri::command]
pub fn atlas_delete(app: AppHandle, id: u64) -> Result<AtlasStore, String> {
    with_store(&app, true, |s| { s.remove(id); s.clone() })
}

/// Picks the strategy (and which of its trees) shown in game.
#[tauri::command]
pub fn atlas_use(app: AppHandle, id: u64, variant: usize) -> Result<AtlasStore, String> {
    with_store(&app, true, |s| { s.use_one(id, variant); s.clone() })
}

#[tauri::command]
pub fn atlas_own_notes(app: AppHandle, id: u64, text: String) -> Result<AtlasStore, String> {
    with_store(&app, true, |s| { s.set_own_notes(id, &text); s.clone() })
}

/// The pathofexile.com link for one of a strategy's trees.
#[tauri::command]
pub fn atlas_official_link(app: AppHandle, id: u64, variant: usize) -> Result<String, String> {
    with_store(&app, false, |s| {
        let st = s.list.iter().find(|x| x.id == id).ok_or("That strategy is gone")?;
        let v = st.variants.get(variant).ok_or("That tree is gone")?;
        Ok(atlas::official_link(&v.nodes, &v.choices))
    })?
}

/// Hotkey: show/hide the atlas window over the game, leaving the game in front.
pub fn toggle_atlas_window(app: &AppHandle) {
    static PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let Some(w) = app.get_webview_window("atlas") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    // First time: top right of the screen, clear of the minimap's left edge.
    if !PLACED.swap(true, Ordering::Relaxed) {
        if let (Ok(Some(m)), Ok(size)) = (w.current_monitor(), w.outer_size()) {
            let (mp, ms) = (m.position(), m.size());
            let x = mp.x + ms.width as i32 - size.width as i32 - (ms.width as f64 * 0.17) as i32;
            let _ = w.set_position(PhysicalPosition::new(x.max(mp.x), mp.y + (ms.height as f64 * 0.08) as i32));
        }
    }
    let front = game_in_front();
    let _ = w.emit("atlas-changed", ());
    let _ = w.show();
    if front { focus_game(); }
}

#[tauri::command]
pub fn hide_atlas_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("atlas") { let _ = w.hide(); }
}
