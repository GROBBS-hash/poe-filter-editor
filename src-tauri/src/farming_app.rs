//! "Farm this" (Atlas page): sets up the tree, the Thread shopping list, the map search and the filter for one farm, and
//! shows what it makes after costs. Logic in `filter_core::farming`. Kept in farm_now.json.

use super::*;
use filter_core::farming::{self, FarmNow, Profit};

static NOW: Mutex<Option<Option<FarmNow>>> = Mutex::new(None);

fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("farm_now.json") }
fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

pub(crate) fn current(app: &AppHandle) -> Option<FarmNow> {
    let mut n = NOW.lock().unwrap();
    if n.is_none() { *n = Some(std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok())); }
    n.clone().flatten()
}

fn keep(app: &AppHandle, farm: Option<FarmNow>) {
    match &farm {
        Some(f) => if let Ok(t) = serde_json::to_string_pretty(f) {
            let tmp = path(app).with_extension("json.tmp");
            if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, path(app)); }
        },
        None => { let _ = std::fs::remove_file(path(app)); }
    }
    *NOW.lock().unwrap() = Some(farm);
    let _ = app.emit("farm-now", ());
}

fn price(app: &AppHandle, name: &str) -> Option<f64> {
    let st = app.state::<AppState>();
    let p = st.prices.lock().unwrap();
    p.get(&Game::Poe1).and_then(|f| f.items.iter().find(|i| i.name == name).map(|i| i.value))
}

#[derive(Serialize)]
pub struct ThreadNeed {
    name: String,
    /// A map, for the planned maps, and how many the last stash scan saw.
    each_map: u32, need: u32, have: Option<u32>, buy: u32,
    each: Option<f64>,
    /// The trade site's bulk id, to open its sellers.
    bulk: Option<String>,
}

#[derive(Serialize)]
pub struct FarmView {
    farm: FarmNow,
    threads: Vec<ThreadNeed>,
    /// Threads a map at today's prices.
    threads_now: f64,
    cost: f64,
    profit: Option<Profit>,
    divine: Option<f64>,
    /// Its tree is the one in use (maps count for it).
    tree_in_use: bool,
    /// The tree is still among your strategies.
    tree_saved: bool,
    scanned: Option<u64>,
    /// Its filter edit is in the filter in use.
    in_filter: bool,
}

impl FarmView {
    pub fn thread_names(&self) -> Vec<String> { self.threads.iter().map(|t| t.name.clone()).collect() }
    /// "Tonight": as many maps of it as fit in this long.
    pub fn run_card(&self, app: &AppHandle, minutes: u32) -> filter_core::tonight::Card {
        let mine = made(app, self.farm.strategy);
        let cycle = if mine.len() >= 3 { mine.iter().map(|r| r.secs).sum::<f64>() / mine.len() as f64 }
            else { match play_app::avg_cycle(app) { 0 => 420.0, s => s as f64 } };
        let buy: Vec<(String, u32)> = self.threads.iter().filter(|t| t.have.is_some_and(|h| h < t.each_map)).map(|t| (t.name.clone(), t.buy)).collect();
        filter_core::tonight::run(&self.farm, self.profit.as_ref(), cycle, minutes, &buy)
    }
}

/// Your ordinary maps on the farm's tree (boss fights and other special areas don't count).
fn made(app: &AppHandle, tree: u64) -> Vec<farming::RunMade> {
    let rate = wealth_app::run_rates(app);
    let runs: Vec<farming::Run> = play_app::maps_history(app.clone()).iter().map(|r| (r.start, (r.left - r.start).max(r.in_map),
        (!r.loot_missed).then(|| r.loot.iter().map(|l| l.value).sum::<f64>()), r.atlas.filter(|_| r.area.starts_with("MapWorlds")), rate(r))).collect();
    farming::runs_on(&runs, tree)
}

fn view(app: &AppHandle) -> Option<FarmView> {
    let farm = current(app)?;
    let st = app.state::<AppState>();
    let (items, snap) = wealth_app::last_scan(app);
    let statics = trade_api::statics(&st.data_dir, false).ok();
    let threads: Vec<ThreadNeed> = farm.threads.iter().map(|(name, n)| {
        let need = n * farm.plan;
        let have = snap.as_ref().map(|_| items.iter().filter(|i| i.name == *name).map(|i| i.stack).sum::<u32>());
        ThreadNeed { each_map: *n, need, have, buy: need.saturating_sub(have.unwrap_or(0)), each: price(app, name),
            bulk: statics.as_ref().and_then(|s| s.id(name)), name: name.clone() }
    }).collect();
    let threads_now = threads.iter().map(|t| t.each.unwrap_or(0.0) * t.each_map as f64).sum::<f64>();
    let cost = farm.map_cost + farm.other_cost + farm.thread_cost.unwrap_or(threads_now);
    let store = atlas_app::atlas_list(app.clone()).ok();
    let in_filter = {
        let s = st.store.lock().unwrap();
        s.active.as_ref().and_then(|id| s.profiles.get(id)).is_some_and(|p| p.rules.iter().any(|r| !r.is_off() && rule_label(r).starts_with(farming::RULE_PREFIX)))
    };
    Some(FarmView {
        profit: farming::profit(&made(app, farm.strategy), cost, currency_rates(app).get("divine").copied()), threads, threads_now, cost, in_filter,
        divine: currency_rates(app).get("divine").copied(),
        tree_in_use: atlas_app::active_id(app) == Some(farm.strategy),
        tree_saved: store.is_some_and(|s| s.list.iter().any(|x| x.id == farm.strategy)),
        scanned: snap.map(|s| s.time), farm,
    })
}

#[tauri::command(async)]
pub fn farm_now(app: AppHandle) -> Option<FarmView> { view(&app) }

/// One button: the tree (built for these mechanics, or the one already saved for them) goes in use, and the rest is
/// listed. `farm` comes from the card.
#[tauri::command(async)]
pub fn farm_start(app: AppHandle, farm: FarmNow, thread_keys: Vec<(String, u32)>) -> Result<Option<FarmView>, String> {
    let mut farm = farm;
    let name = format!("Farm: {}", farm.title);
    let store = atlas_app::atlas_list(app.clone())?;
    if let Some(s) = store.list.iter().find(|s| s.name == name) {
        atlas_app::atlas_use(app.clone(), s.id, 0)?;
        farm.strategy = s.id;
    } else {
        let ask = filter_core::atlas_opt::Ask { mechanics: farm.keys.clone(), threads: thread_keys, ..Default::default() };
        let after = atlas_app::atlas_save_plan(app.clone(), ask, name.clone())?;
        farm.strategy = after.active.ok_or("The tree couldn't be saved")?;
    }
    farm.strategy_name = name;
    farm.started = unix();
    if farm.plan == 0 { farm.plan = 20; }
    // Keep your own costs when it's the same farm again.
    if let Some(old) = current(&app).filter(|o| o.keys == farm.keys) {
        (farm.map_cost, farm.other_cost, farm.thread_cost, farm.plan) = (old.map_cost, old.other_cost, old.thread_cost, old.plan);
    } else {
        farm.map_cost = price(&app, "Map (Tier 16)").map(|v| (v * 10.0).round() / 10.0).unwrap_or(0.0);
    }
    drop_rule(&app)?;
    keep(&app, Some(farm));
    Ok(view(&app))
}

/// Your costs and how many maps you plan.
#[tauri::command(async)]
pub fn farm_costs(app: AppHandle, plan: u32, map_cost: f64, other_cost: f64, thread_cost: Option<f64>) -> Option<FarmView> {
    let mut f = current(&app)?;
    (f.plan, f.map_cost, f.other_cost, f.thread_cost) = (plan.clamp(1, 500), map_cost.max(0.0), other_cost.max(0.0), thread_cost.map(|t| t.max(0.0)));
    keep(&app, Some(f));
    view(&app)
}

/// Puts the farm's tree back in use.
#[tauri::command(async)]
pub fn farm_use_tree(app: AppHandle) -> Result<Option<FarmView>, String> {
    let f = current(&app).ok_or("No farm picked")?;
    atlas_app::atlas_use(app.clone(), f.strategy, 0)?;
    Ok(view(&app))
}

/// Takes earlier farm edits out of the filter in use. True when there were any.
fn drop_rule(app: &AppHandle) -> Result<bool, String> {
    let st = app.state::<AppState>();
    let mut store = st.store.lock().unwrap();
    let Some(id) = store.active.clone() else { return Ok(false) };
    let has = |p: &filter_core::profiles::Profile| p.rules.iter().any(|r| rule_label(r).starts_with(farming::RULE_PREFIX));
    if !store.profiles.get(&id).is_some_and(|p| has(p)) { return Ok(false); }
    remember(&st, &store, &id, "Farm's drops back to normal");
    if let Some(p) = store.profiles.get_mut(&id) { p.rules.retain(|r| !rule_label(r).starts_with(farming::RULE_PREFIX)); }
    save_and_build(&st, &store, &id)?;
    Ok(true)
}

/// Its drops stand out in the filter in use (with this look), or not.
#[tauri::command(async)]
pub fn farm_filter(app: AppHandle, on: bool, style: CustomStyle) -> Result<Option<FarmView>, String> {
    drop_rule(&app)?;
    if on {
        let f = current(&app).ok_or("No farm picked")?;
        let rule = farming::rule(&f, &style).ok_or("Nothing it drops has a filter rule to add")?;
        let st = app.state::<AppState>();
        let mut store = st.store.lock().unwrap();
        let id = store.active.clone().ok_or("No filter is in use")?;
        remember(&st, &store, &id, &format!("{} drops stand out", f.title));
        store.profiles.get_mut(&id).ok_or("Profile not found")?.rules.push(rule);
        save_and_build(&st, &store, &id)?;
    }
    Ok(view(&app))
}

/// Done with it: the filter edit goes, your tree and results stay.
#[tauri::command(async)]
pub fn farm_stop(app: AppHandle) -> Result<(), String> {
    drop_rule(&app)?;
    keep(&app, None);
    Ok(())
}
