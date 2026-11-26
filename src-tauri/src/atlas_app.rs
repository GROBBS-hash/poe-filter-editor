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
        Link::PoePlanner(code) => atlas::from_poeplanner(&code, link)?,
        Link::PoePlannerShort(id) => {
            let resp = ureq::get(&atlas::poeplanner_short_url(&id)).set("User-Agent", UA).timeout(Duration::from_secs(20)).call();
            let code = match resp {
                Ok(r) => r.into_string().map_err(|_| "poeplanner sent something unexpected".to_string())?,
                Err(ureq::Error::Status(404, _)) => return Err("poeplanner doesn't know that link (check it)".into()),
                Err(ureq::Error::Status(code, _)) => return Err(format!("poeplanner said no (error {code}); try again in a minute")),
                Err(_) => return Err("Couldn't reach poeplanner (is the internet on?)".into()),
            };
            let code = code.trim().trim_matches('"').to_string();
            atlas::from_poeplanner(&code, &format!("https://poeplanner.com/atlas-tree/{code}"))?
        }
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
#[tauri::command(async)]
pub fn atlas_optimise(app: AppHandle, ask: filter_core::atlas_opt::Ask) -> filter_core::atlas_opt::Plan {
    filter_core::atlas_opt::optimise(&priced(&app, ask))
}

/// With today's prices, for the choices that follow them (Thread steering, essence types, lifeforce colour…).
fn priced(app: &AppHandle, mut ask: filter_core::atlas_opt::Ask) -> filter_core::atlas_opt::Ask {
    if let Some(p) = app.state::<AppState>().prices.lock().unwrap().get(&Game::Poe1) {
        ask.prices = p.items.iter().filter(|i| i.name.ends_with(" Thread") || i.name.contains("Essence") || i.name.ends_with("Lifeforce") || i.name.ends_with("Emblem"))
            .map(|i| (i.name.clone(), i.value)).collect();
    }
    ask
}

/// The goals the builder knows besides mechanics: (key, label).
#[tauri::command]
pub fn atlas_goals() -> Vec<(String, String)> { filter_core::atlas_opt::goals() }

/// Saves the optimiser's tree as one of My strategies (and the one shown in game).
#[tauri::command]
pub fn atlas_save_plan(app: AppHandle, ask: filter_core::atlas_opt::Ask, name: String) -> Result<AtlasStore, String> {
    use filter_core::atlas::{NoteLine, Notes, Variant};
    let ask = priced(&app, ask);
    let plan = filter_core::atlas_opt::optimise(&ask);
    let goals = filter_core::atlas_opt::goals();
    let mut mechs: Vec<String> = ask.goals.iter().filter_map(|g| goals.iter().find(|x| &x.0 == g).map(|x| x.1.clone())).collect();
    mechs.extend(ask.mechanics.iter().map(|m| filter_core::atlas_model::cap(m)));
    let how = match ask.priority { filter_core::atlas_opt::Priority::Profit => "most profit", filter_core::atlas_opt::Priority::Balanced => "balanced", filter_core::atlas_opt::Priority::Fewest => "fewest points" };
    let mut lines = vec![NoteLine { kind: "p".into(), depth: 0, text: format!("Built for {} ({how}), {} points.", mechs.join(" + "), plan.points) }];
    if !plan.threads.is_empty() { lines.push(NoteLine { kind: "li".into(), depth: 0, text: format!("Threads: {}", plan.threads.iter().map(|(t, n)| format!("{n} {}", filter_core::atlas_model::cap(t))).collect::<Vec<_>>().join(", ")) }); }
    lines.extend(plan.notes.iter().chain(&plan.tips).map(|t| NoteLine { kind: "li".into(), depth: 0, text: t.clone() }));
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

/// Your saved strategies by name (for the map tracker's strategy switch).
pub(crate) fn names(app: &AppHandle) -> Vec<(u64, String)> {
    with_store(app, false, |s| s.list.iter().map(|x| (x.id, x.name.clone())).collect()).unwrap_or_default()
}

/// Puts a strategy in use from outside the Atlas page (the map tracker), keeping its tree if it's in use already.
pub(crate) fn use_strategy(app: &AppHandle, id: u64) {
    let _ = with_store(app, true, |s| { let v = if s.active == Some(id) { s.variant } else { 0 }; s.use_one(id, v); });
}

/// The atlas tree in use now (the one F7 shows), for tagging map runs and earnings sessions.
pub(crate) fn active_id(app: &AppHandle) -> Option<u64> {
    with_store(app, false, |s| s.active).ok().flatten()
}

/// What each saved tree earned: loot per map / per hour from the map tracker, and earnings sessions started with it.
/// Your saved trees with what maps on them made, for the farm finder. Boss fights and other special areas have no place
/// in a farm's number: only ordinary maps count (the others still end a map's clock).
pub(crate) fn own_trees(app: &AppHandle) -> Vec<filter_core::farm::OwnTree> {
    let runs: Vec<(i64, i64, Option<f64>, Option<u64>)> = play_app::maps_history(app.clone()).iter()
        .map(|r| (r.start, (r.left - r.start).max(r.in_map), (!r.loot_missed).then(|| r.loot.iter().map(|l| l.value).sum::<f64>()), r.atlas.filter(|_| r.area.starts_with("MapWorlds")))).collect();
    let results = atlas::tree_results(&runs, &[]);
    // The same in divines, each map at its own day's rate (a map without one counts at today's).
    let today = currency_rates(app).get("divine").copied().filter(|d| *d > 0.0);
    let rate = wealth_app::run_rates(app);
    let in_divines = today.map(|today| {
        let history = play_app::maps_history(app.clone());
        let runs: Vec<_> = runs.iter().zip(&history).map(|(r, h)| (r.0, r.1, r.2.map(|l| l / rate(h).unwrap_or(today)), r.3)).collect();
        atlas::tree_results(&runs, &[])
    });
    with_store(app, false, |s| s.list.iter().filter_map(|x| {
        let result = results.iter().find(|r| r.id == x.id)?.clone();
        // The tree itself: the one in game for the active strategy, else its biggest.
        let v = if s.active == Some(x.id) { x.variants.get(s.variant) } else { None }.or_else(|| x.variants.iter().max_by_key(|v| v.nodes.len()))?;
        let divines = in_divines.as_ref().and_then(|d| d.iter().find(|r| r.id == x.id)).map(|r| r.loot);
        Some(filter_core::farm::OwnTree { name: x.name.clone(), nodes: v.nodes.clone(), result, divines })
    }).collect()).unwrap_or_default()
}

#[tauri::command]
pub fn atlas_results(app: AppHandle) -> Vec<atlas::TreeResult> {
    let runs: Vec<(i64, i64, Option<f64>, Option<u64>)> = play_app::maps_history(app.clone()).iter()
        .map(|r| (r.start, (r.left - r.start).max(r.in_map), (!r.loot_missed).then(|| r.loot.iter().map(|l| l.value).sum::<f64>()), r.atlas)).collect();
    let sessions: Vec<(f64, f64, Option<u64>)> = wealth_app::finished_sessions(&app).iter()
        .filter(|s| s.end.is_some()).map(|s| (s.seconds(0) as f64 / 3600.0, s.profit(), s.atlas)).collect();
    atlas::tree_results(&runs, &sessions)
}
