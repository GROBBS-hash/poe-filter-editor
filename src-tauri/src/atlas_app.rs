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
pub fn atlas_list(app: AppHandle) -> Result<AtlasStore, String> {
    add_missing_steps(&app);
    with_store(&app, false, |s| s.clone())
}

/// Strategies saved before trees had steps: work them out (maxroll ones are read again, once,
/// because the old save lost the order the points were taken in).
fn add_missing_steps(app: &AppHandle) {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) { return; }
    let Ok(old) = with_store(app, false, |s| s.list.iter()
        .filter(|x| x.variants.iter().any(|v| v.steps.is_empty() && v.nodes.len() > 1))
        .map(|x| (x.id, x.site.clone(), x.link.clone())).collect::<Vec<_>>()) else { return };
    if old.is_empty() { return; }
    let app = app.clone();
    thread::spawn(move || {
        for (id, site, link) in old {
            let fresh = if site == "maxroll" { fetch(&link).ok() } else { None };
            let _ = with_store(&app, true, |s| {
                let Some(x) = s.list.iter_mut().find(|x| x.id == id) else { return };
                match fresh {
                    Some(f) if f.variants.len() == x.variants.len() => { x.variants = f.variants; x.notes = f.notes; x.device = f.device; }
                    _ => { for v in &mut x.variants { if v.steps.is_empty() { v.steps = atlas::graph().steps(&v.nodes); } } }
                }
            });
            thread::sleep(Duration::from_secs(1));
        }
    });
}

/// Reads a strategy from a link: one request to maxroll; official links need none.
fn fetch(link: &str) -> Result<Strategy, String> {
    Ok(match atlas::parse_link(link)? {
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
    })
}

/// Imports a strategy from a link and saves it.
#[tauri::command(async)]
pub fn atlas_import(app: AppHandle, link: String, name: String) -> Result<AtlasStore, String> {
    let mut strat = fetch(&link)?;
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

// ---------- the window's place ----------

/// Where the atlas window was last (screen pixels), kept in atlas_window.json.
#[derive(Serialize, serde::Deserialize, Clone, Copy, PartialEq)]
struct Place { x: i32, y: i32, w: u32, h: u32 }

fn place_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("atlas_window.json") }

static PLACE: Mutex<Option<Place>> = Mutex::new(None);
/// Bumped on every move/resize; the save waits until it settles (no file write per pixel dragged).
static PLACE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// True while we place the window ourselves, so that doesn't count as him moving it.
static PLACING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Moved or resized by hand: remember it (saved half a second after it stops changing).
pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if !matches!(event, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)) { return; }
    if PLACING.load(Ordering::Relaxed) || !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) { return; }
    let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) else { return };
    if s.width < 200 || s.height < 200 { return; }
    *PLACE.lock().unwrap() = Some(Place { x: p.x, y: p.y, w: s.width, h: s.height });
    let gen = PLACE_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let app = window.app_handle().clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        if PLACE_GEN.load(Ordering::Relaxed) != gen { return; }
        if let Some(pl) = *PLACE.lock().unwrap() {
            if let Ok(text) = serde_json::to_string(&pl) { let _ = std::fs::write(place_path(&app), text); }
        }
    });
}

/// Puts the window where it was last time (pulled fully onto a screen), or top right the first time.
fn restore_place(app: &AppHandle, w: &tauri::WebviewWindow) {
    let saved = {
        let mut g = PLACE.lock().unwrap();
        if g.is_none() {
            *g = std::fs::read_to_string(place_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok());
        }
        *g
    };
    PLACING.store(true, Ordering::Relaxed);
    match saved {
        Some(pl) => {
            // The screen it was on (by its middle), else the first one; never bigger than that screen.
            if let Some((mx, my, mw, mh)) = monitor_at(w, pl.x as f64 + pl.w as f64 / 2.0, pl.y as f64 + pl.h as f64 / 2.0) {
                let (ww, hh) = (pl.w.min(mw as u32), pl.h.min(mh as u32));
                let x = pl.x.clamp(mx, (mx + mw - ww as i32).max(mx));
                let y = pl.y.clamp(my, (my + mh - hh as i32).max(my));
                let _ = w.set_size(tauri::PhysicalSize::new(ww, hh));
                let _ = w.set_position(PhysicalPosition::new(x, y));
            }
        }
        None => {
            if let (Ok(Some(m)), Ok(size)) = (w.current_monitor(), w.outer_size()) {
                let (mp, ms) = (m.position(), m.size());
                let (ww, hh) = (size.width.min(ms.width), size.height.min(ms.height * 9 / 10));
                let _ = w.set_size(tauri::PhysicalSize::new(ww, hh));
                let x = mp.x + ms.width as i32 - ww as i32 - (ms.width as f64 * 0.12) as i32;
                let _ = w.set_position(PhysicalPosition::new(x.max(mp.x), mp.y + (ms.height as f64 * 0.06) as i32));
            }
        }
    }
    let _ = std::thread::spawn(|| { thread::sleep(Duration::from_millis(400)); PLACING.store(false, Ordering::Relaxed); });
}

/// Hotkey: show/hide the atlas window over the game, leaving the game in front.
pub fn toggle_atlas_window(app: &AppHandle) {
    let Some(w) = app.get_webview_window("atlas") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    restore_place(app, &w);
    let front = game_in_front();
    let _ = w.emit("atlas-changed", ());
    let _ = w.show();
    if front { focus_game(); }
}

#[tauri::command]
pub fn hide_atlas_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("atlas") { let _ = w.hide(); }
}
