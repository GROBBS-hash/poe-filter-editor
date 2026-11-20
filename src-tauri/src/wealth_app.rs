//! Wealth tracker in the app: reads your stash from pathofexile.com (with your
//! POESESSID), values it with poe.ninja prices, keeps a history, finds valuable
//! items you haven't listed, and runs named earnings sessions with a small
//! overlay showing chaos / divines per hour.

use super::*;
use filter_core::wealth::{self, ScanProfile, Session, Snapshot, Tab, Valued, WealthStore};

fn store_path(state: &AppState) -> PathBuf { state.data_dir.join("wealth.json") }

fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

static STORE: Mutex<Option<WealthStore>> = Mutex::new(None);

fn with_store<T>(state: &AppState, f: impl FnOnce(&mut WealthStore) -> T) -> T {
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(|| WealthStore::load(&store_path(state)));
    let out = f(s);
    let _ = s.save(&store_path(state));
    out
}

/// (account, POESESSID, league, PoE 2?)
fn who(app: &AppHandle) -> Result<(String, String, String, bool), String> {
    let state = app.state::<AppState>();
    let s = state.store.lock().unwrap().settings.clone();
    if s.poe_account.trim().is_empty() || s.poesessid.trim().is_empty() {
        return Err("Set your account name and POESESSID on the Wealth page first".into());
    }
    let (poe2, league) = trade_league(app);
    Ok((s.poe_account.trim().to_string(), s.poesessid.trim().to_string(), league, poe2))
}

/// Values items with the current poe.ninja prices.
fn value(app: &AppHandle, items: &serde_json::Value, tab: &str) -> Vec<Valued> {
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let Some(f) = prices.get(&game) else { return vec![] };
    let look = |name: Option<&str>, base: &str, kind: &str| -> Option<f64> {
        match kind {
            "unique" => price_of(f, name, base, "Unique"),
            // Gems come in many levels/qualities: count the cheapest, so totals don't run away.
            "gem" => f.items.iter().filter(|i| i.name == base && i.cat.contains("Gem")).map(|i| i.value).reduce(f64::min),
            _ => price_of(f, None, base, ""),
        }
    };
    wealth::value_tab(&items["items"], tab, &look)
}

fn divine(app: &AppHandle) -> Option<f64> { currency_rates(app).get("divine").copied() }

/// Reads and values these tabs (all when None), telling the page how far along it is.
fn scan(app: &AppHandle, only: Option<&[usize]>) -> Result<Vec<Valued>, String> {
    let (account, sessid, league, poe2) = who(app)?;
    let state = app.state::<AppState>();
    let mut tabs = with_store(&state, |s| s.tabs.clone());
    if tabs.is_empty() {
        let first = trade_api::stash_tab(&account, &sessid, &league, poe2, 0, true)?;
        tabs = wealth::tabs_from_json(&first);
        let t = tabs.clone();
        with_store(&state, |s| s.tabs = t);
    }
    let pick: Vec<Tab> = tabs.into_iter().filter(|t| only.map(|o| o.contains(&t.index)).unwrap_or(true)).collect();
    let mut out = Vec::new();
    for (n, t) in pick.iter().enumerate() {
        let _ = app.emit("wealth-progress", (n, pick.len(), t.name.clone()));
        let v = trade_api::stash_tab(&account, &sessid, &league, poe2, t.index, false)?;
        out.extend(value(app, &v, &t.name));
    }
    let _ = app.emit("wealth-progress", (pick.len(), pick.len(), String::new()));
    Ok(out)
}

#[derive(serde::Serialize, Clone)]
pub struct WealthView {
    account: String,
    has_sessid: bool,
    league: String,
    tabs: Vec<Tab>,
    tracked: Vec<usize>,
    min: f64,
    minutes: u32,
    snapshots: Vec<Snapshot>,
    missed: Vec<Valued>,
    sessions: Vec<Session>,
    current: Option<Session>,
    strategies: Vec<(String, usize, f64, f64)>,
    profiles: Vec<ScanProfile>,
    active: Option<String>,
    divine: Option<f64>,
    now: u64,
}

fn view(app: &AppHandle) -> WealthView {
    let state = app.state::<AppState>();
    let s = state.store.lock().unwrap().settings.clone();
    let (_, league) = trade_league(app);
    let st = with_store(&state, |w| w.clone());
    WealthView {
        account: s.poe_account, has_sessid: !s.poesessid.trim().is_empty(), league,
        tracked: s.wealth_tabs, min: s.wealth_min, minutes: s.earn_minutes.max(1),
        strategies: wealth::by_strategy(&st.sessions),
        tabs: st.tabs, snapshots: st.snapshots, missed: st.missed, sessions: st.sessions, current: st.current,
        profiles: st.profiles, active: st.active,
        divine: divine(app), now: now(),
    }
}

#[tauri::command]
pub fn wealth_get(app: AppHandle) -> WealthView { view(&app) }

/// Saves your account + POESESSID and checks them by reading your tab list.
#[tauri::command]
pub async fn wealth_setup(app: AppHandle, account: String, sessid: Option<String>) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        {
            let mut s = state.store.lock().unwrap();
            s.settings.poe_account = account.trim().to_string();
            if let Some(id) = sessid.filter(|x| !x.trim().is_empty()) { s.settings.poesessid = id.trim().to_string(); }
            s.save(&state.store_path).map_err(err)?;
        }
        let (account, sessid, league, poe2) = who(&app)?;
        let first = trade_api::stash_tab(&account, &sessid, &league, poe2, 0, true)?;
        let tabs = wealth::tabs_from_json(&first);
        if tabs.is_empty() { return Err(format!("Signed in, but no stash tabs in {league}")); }
        with_store(&state, |s| s.tabs = tabs);
        Ok(view(&app))
    }).await.map_err(err)?
}

/// Which tabs earnings sessions watch, the "worth selling" level and how often to check.
#[tauri::command]
pub fn wealth_prefs(app: AppHandle, tracked: Vec<usize>, min: f64, minutes: u32) -> Result<WealthView, String> {
    let state = app.state::<AppState>();
    {
        let mut s = state.store.lock().unwrap();
        s.settings.wealth_tabs = tracked;
        s.settings.wealth_min = min.max(0.0);
        s.settings.earn_minutes = minutes.clamp(1, 60);
        s.save(&state.store_path).map_err(err)?;
    }
    Ok(view(&app))
}

/// Values your whole stash: total, history point, and valuable items not listed.
#[tauri::command]
pub async fn wealth_scan(app: AppHandle) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        // The profile in use: only its tabs (None = the whole stash).
        let (name, tabs) = with_store(&state, |s| {
            let p = s.active.as_ref().and_then(|a| s.profiles.iter().find(|p| &p.name == a));
            (p.map(|p| p.name.clone()).unwrap_or_default(), p.map(|p| p.tabs.clone()))
        });
        if tabs.as_ref().is_some_and(|t| t.is_empty()) { return Err(format!("Tick some tabs for \"{name}\" first")); }
        let valued = scan(&app, tabs.as_deref())?;
        let (_, league) = trade_league(&app);
        let min = state.store.lock().unwrap().settings.wealth_min;
        let mut snap = wealth::snapshot(now(), &league, divine(&app), &valued);
        snap.profile = name;
        with_store(&state, |s| { s.add_snapshot(snap); s.missed = wealth::missed(&valued, min); });
        Ok(view(&app))
    }).await.map_err(err)?
}


/// Saves a scan profile (a named set of tabs); `old` renames an existing one. The saved one becomes the one in use.
#[tauri::command]
pub fn wealth_profile_save(app: AppHandle, name: String, tabs: Vec<usize>, old: Option<String>) -> Result<WealthView, String> {
    let name = name.trim().to_string();
    if name.is_empty() { return Err("Give the profile a name".into()); }
    let state = app.state::<AppState>();
    with_store(&state, |s| {
        let from = old.unwrap_or_else(|| name.clone());
        s.profiles.retain(|p| p.name != from && p.name != name);
        // Its history follows a rename.
        for snap in s.snapshots.iter_mut().filter(|x| x.profile == from) { snap.profile = name.clone(); }
        s.profiles.push(ScanProfile { name: name.clone(), tabs });
        s.active = Some(name);
    });
    Ok(view(&app))
}

#[tauri::command]
pub fn wealth_profile_delete(app: AppHandle, name: String) -> WealthView {
    let state = app.state::<AppState>();
    with_store(&state, |s| {
        s.profiles.retain(|p| p.name != name);
        if s.active.as_deref() == Some(&name) { s.active = None; }
    });
    view(&app)
}

/// Picks the profile to scan (None = the whole stash).
#[tauri::command]
pub fn wealth_profile_use(app: AppHandle, name: Option<String>) -> WealthView {
    let state = app.state::<AppState>();
    with_store(&state, |s| s.active = name.filter(|n| s.profiles.iter().any(|p| &p.name == n)));
    view(&app)
}

/// Reads the tab list again (after adding, renaming or moving tabs in game).
#[tauri::command]
pub async fn wealth_tabs_refresh(app: AppHandle) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (account, sessid, league, poe2) = who(&app)?;
        let tabs = wealth::tabs_from_json(&trade_api::stash_tab(&account, &sessid, &league, poe2, 0, true)?);
        let state = app.state::<AppState>();
        with_store(&state, |s| s.tabs = tabs);
        Ok(view(&app))
    }).await.map_err(err)?
}
/// Starts an earnings session for a strategy: values your tracked tabs now.
#[tauri::command]
pub async fn session_start(app: AppHandle, name: String) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let tabs = state.store.lock().unwrap().settings.wealth_tabs.clone();
        if tabs.is_empty() { return Err("Tick the tabs your loot goes into (your dump tabs) first".into()); }
        let total: f64 = scan(&app, Some(&tabs))?.iter().map(|v| v.value).sum();
        let (_, league) = trade_league(&app);
        let name = if name.trim().is_empty() { "Unnamed strategy".to_string() } else { name.trim().to_string() };
        with_store(&state, |s| {
            if let Some(mut old) = s.current.take() { old.end = Some(now()); s.sessions.push(old); }
            s.current = Some(Session { name, league, tabs, start: now(), end: None, start_value: total, last_value: total, paused: 0, paused_at: None });
        });
        show_overlay(&app);
        notify(&app);
        Ok(view(&app))
    }).await.map_err(err)?
}

/// Re-values the session's tabs now (also runs on its own every few minutes).
#[tauri::command]
pub async fn session_check(app: AppHandle) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || { check_session(&app)?; Ok(view(&app)) }).await.map_err(err)?
}

fn check_session(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let Some(cur) = with_store(&state, |s| s.current.clone()) else { return Ok(()) };
    if cur.paused_at.is_some() { return Ok(()); }
    let total: f64 = scan(app, Some(&cur.tabs))?.iter().map(|v| v.value).sum();
    with_store(&state, |s| if let Some(c) = s.current.as_mut() { c.last_value = total; });
    notify(app);
    Ok(())
}

#[tauri::command]
pub fn session_pause(app: AppHandle, pause: bool) -> WealthView {
    let state = app.state::<AppState>();
    with_store(&state, |s| if let Some(c) = s.current.as_mut() {
        match (pause, c.paused_at) {
            (true, None) => c.paused_at = Some(now()),
            (false, Some(at)) => { c.paused += now().saturating_sub(at); c.paused_at = None; }
            _ => {}
        }
    });
    notify(&app);
    view(&app)
}

/// Ends the session (one last check first) and keeps it for the strategy comparison.
#[tauri::command]
pub async fn session_stop(app: AppHandle) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _ = check_session(&app);
        let state = app.state::<AppState>();
        with_store(&state, |s| if let Some(mut c) = s.current.take() {
            if let Some(at) = c.paused_at.take() { c.paused += now().saturating_sub(at); }
            c.end = Some(now());
            s.sessions.push(c);
        });
        if let Some(w) = app.get_webview_window("earnings") { let _ = w.hide(); }
        notify(&app);
        Ok(view(&app))
    }).await.map_err(err)?
}

#[tauri::command]
pub fn session_delete(app: AppHandle, start: u64) -> WealthView {
    let state = app.state::<AppState>();
    with_store(&state, |s| s.sessions.retain(|x| x.start != start));
    view(&app)
}

#[tauri::command]
pub fn earnings_overlay(app: AppHandle, show: bool) {
    if show { show_overlay(&app); } else if let Some(w) = app.get_webview_window("earnings") { let _ = w.hide(); }
}

fn show_overlay(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("earnings") {
        let front = game_in_front();
        let _ = w.show();
        if front { focus_game(); } // never take focus from the game
    }
}

/// Tells the page and the overlay the latest numbers.
fn notify(app: &AppHandle) {
    let _ = app.emit("earn-update", view(app));
}

/// Checks a running session every few minutes.
pub fn start_session_timer(app: AppHandle) {
    thread::spawn(move || {
        let mut last = std::time::Instant::now();
        loop {
            thread::sleep(Duration::from_secs(15));
            let state = app.state::<AppState>();
            let minutes = state.store.lock().unwrap().settings.earn_minutes.max(1) as u64;
            let running = with_store(&state, |s| s.current.as_ref().map(|c| c.paused_at.is_none()).unwrap_or(false));
            if !running { last = std::time::Instant::now(); continue; }
            if last.elapsed() < Duration::from_secs(minutes * 60) { continue; }
            last = std::time::Instant::now();
            if let Err(e) = check_session(&app) { let _ = app.emit("earn-error", e); }
        }
    });
}
