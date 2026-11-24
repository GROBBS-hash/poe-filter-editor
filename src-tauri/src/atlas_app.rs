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
        Link::Planner { data_url, hash } => {
            let resp = ureq::get(&data_url).set("User-Agent", UA).set("Accept", "application/json")
                .timeout(Duration::from_secs(30)).call();
            let v: serde_json::Value = match resp {
                Ok(r) => r.into_json().map_err(|_| "The atlas planner sent something unexpected".to_string())?,
                Err(ureq::Error::Status(code, _)) => return Err(format!("The atlas planner said no (error {code}); try again in a minute")),
                Err(_) => return Err("Couldn't reach the atlas planner (is the internet on?)".into()),
            };
            atlas::from_planner(&hash, &v, link)?
        }
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

/// First time ever: top right of the screen, clear of the minimap.
fn first_place(w: &tauri::WebviewWindow) {
    if let (Ok(Some(m)), Ok(size)) = (w.current_monitor(), w.outer_size()) {
        let (mp, ms) = (m.position(), m.size());
        let (ww, hh) = (size.width.min(ms.width), size.height.min(ms.height * 9 / 10));
        crate::win_place::placing();
        let _ = w.set_size(tauri::PhysicalSize::new(ww, hh));
        let x = mp.x + ms.width as i32 - ww as i32 - (ms.width as f64 * 0.12) as i32;
        let _ = w.set_position(PhysicalPosition::new(x.max(mp.x), mp.y + (ms.height as f64 * 0.06) as i32));
    }
}

/// Hotkey: show/hide the atlas window over the game, leaving the game in front.
pub fn toggle_atlas_window(app: &AppHandle) {
    let Some(w) = app.get_webview_window("atlas") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    if !crate::win_place::restore(app, &w) { first_place(&w); }
    let _ = w.emit("atlas-changed", ());
    crate::win_place::show_over_game(app, &w);
}

#[tauri::command]
pub fn hide_atlas_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("atlas") { let _ = w.hide(); }
}

/// Atlas optimiser: a tree for the picked strategies (see core atlas_opt.rs). No network.
#[tauri::command]
pub fn atlas_optimise(ask: filter_core::atlas_opt::Ask) -> filter_core::atlas_opt::Plan {
    filter_core::atlas_opt::optimise(&ask)
}

/// Saves the optimiser's tree as one of My strategies (and the one shown in game).
#[tauri::command]
pub fn atlas_save_plan(app: AppHandle, ask: filter_core::atlas_opt::Ask, name: String) -> Result<AtlasStore, String> {
    use filter_core::atlas::{NoteLine, Notes, Variant};
    let plan = filter_core::atlas_opt::optimise(&ask);
    let mechs: Vec<String> = ask.mechanics.iter().map(|m| { let mut c = m.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect();
    let how = match ask.priority { filter_core::atlas_opt::Priority::Profit => "most profit", filter_core::atlas_opt::Priority::Balanced => "balanced", filter_core::atlas_opt::Priority::Fewest => "fewest points" };
    let mut lines = vec![NoteLine { kind: "p".into(), depth: 0, text: format!("Built by the optimiser for {} ({how}), {} points, from {} creator trees.", mechs.join(" + "), plan.points, plan.trees_used) }];
    lines.extend(plan.top.iter().take(8).map(|w| NoteLine { kind: "li".into(), depth: 0, text: format!("{}: {}", w.name, w.why) }));
    let variant = Variant { name: "Optimised tree".into(), nodes: plan.nodes, choices: plan.choices, steps: plan.steps,
        notes: vec![Notes { title: "How it was built".into(), lines }] };
    let strat = Strategy { name: format!("{} (optimiser)", mechs.join(" + ")), site: "optimiser".into(), imported: now(), variants: vec![variant], ..Default::default() };
    with_store(&app, true, |s| { s.add(strat, &name); s.clone() })
}

/// Saves a tree edited on the Atlas page: over the strategy (`as_new` empty) or as a new strategy with that name.
#[tauri::command]
pub fn atlas_save_tree(app: AppHandle, id: u64, variant: usize, nodes: Vec<u32>, choices: Vec<(u32, u32)>, as_new: String) -> Result<AtlasStore, String> {
    let name = as_new.trim().to_string();
    with_store(&app, true, |s| s.set_tree(id, variant, &nodes, &choices, (!name.is_empty()).then_some(name.as_str()), now()).map(|_| s.clone()))?
}
