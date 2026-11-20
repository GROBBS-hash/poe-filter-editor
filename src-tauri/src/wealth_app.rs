//! Wealth tracker in the app: reads your stash from pathofexile.com (with your
//! POESESSID), values it with poe.ninja prices, keeps a history, finds valuable
//! items you haven't listed, and runs named earnings sessions with a small
//! overlay showing chaos / divines per hour.

use super::*;
use filter_core::wealth::{self, Rare, RarePrice, ScanProfile, Session, Snapshot, Tab, Valued, WealthStore};

/// A rare's trade price counts for a week; "Price rares" looks again after two days.
const RARE_KEEP: u64 = 7 * 86400;
const RARE_STALE: u64 = 2 * 86400;
/// At most this many rares per "Price rares" click (about 2 trade requests each).
const RARE_BATCH: usize = 40;

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
/// Rares count at the trade price "Price rares" last found for them (if it's under a week old).
fn scan(app: &AppHandle, only: Option<&[usize]>) -> Result<(Vec<Valued>, Vec<Rare>), String> {
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
    let mut rares = Vec::new();
    let mut found = Vec::new();
    for (n, t) in pick.iter().enumerate() {
        let _ = app.emit("wealth-progress", (n, pick.len(), t.name.clone()));
        let v = trade_api::stash_tab(&account, &sessid, &league, poe2, t.index, false)?;
        let priced = value(app, &v, &t.name);
        let mut f = wealth::found_in_tab(&v["items"], t.index, &t.name);
        attach_values(&mut f, &priced);
        found.extend(f);
        out.extend(priced);
        rares.extend(wealth::rares_in_tab(&v["items"], &t.name));
    }
    let _ = app.emit("wealth-progress", (pick.len(), pick.len(), String::new()));
    let t = now();
    let mut rare_values = Vec::new();
    let skip = with_store(&state, |s| {
        for r in &rares {
            if let Some(p) = s.rare_prices.get(&r.id).filter(|p| p.chaos > 0.0 && t.saturating_sub(p.time) < RARE_KEEP) {
                rare_values.push(wealth::rare_valued(r, p.chaos));
            }
        }
        s.skip_rares
    });
    attach_values(&mut found, &rare_values);
    // "Count rares in my worth" unticked: totals and earnings leave them out ("Worth selling" still lists them).
    if !skip { out.extend(rare_values); }
    // "Find in stash": these tabs' items replace what the index had for them.
    let scanned: Vec<usize> = pick.iter().map(|t| t.index).collect();
    with_index(&state, |ix| {
        ix.items.retain(|f| !scanned.contains(&f.tab_index));
        ix.items.extend(found);
        ix.time = now();
        ix.league = league.clone();
    });
    Ok((out, rares))
}

/// Puts the scan's prices on the found items (same tab, same spot, same name; socketed gems aren't priced).
fn attach_values(found: &mut [wealth::Found], priced: &[Valued]) {
    for f in found.iter_mut().filter(|f| !f.note.starts_with("in ")) {
        if let Some(v) = priced.iter().find(|v| v.tab == f.tab && v.x == f.x && v.y == f.y && v.name == f.name) { f.value = Some(v.value); }
    }
}

/// Every item from the last scans, for "Find in stash" (stash_index.json: kept apart from wealth.json, it's big).
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct StashIndex { time: u64, league: String, items: Vec<wealth::Found> }

static INDEX: Mutex<Option<StashIndex>> = Mutex::new(None);

fn with_index<T>(state: &AppState, f: impl FnOnce(&mut StashIndex) -> T) -> T {
    let path = state.data_dir.join("stash_index.json");
    let mut g = INDEX.lock().unwrap();
    let ix = g.get_or_insert_with(|| std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default());
    let before = (ix.time, ix.items.len());
    let out = f(ix);
    if (ix.time, ix.items.len()) != before {
        let tmp = path.with_extension("tmp");
        if serde_json::to_string(&*ix).ok().and_then(|t| std::fs::write(&tmp, t).ok()).is_some() { let _ = std::fs::rename(&tmp, &path); }
    }
    out
}

#[derive(serde::Serialize)]
pub struct FindResult {
    /// Items in the index, and when it was last filled.
    items: usize,
    time: u64,
    league: String,
    /// How many matched (only the first 200 are sent).
    total: usize,
    hits: Vec<wealth::Found>,
}

/// "Find in stash": searches every item from your last scans by name, kind, tab and mods.
#[tauri::command]
pub fn wealth_find(app: AppHandle, query: String) -> FindResult {
    let state = app.state::<AppState>();
    with_index(&state, |ix| {
        let hits = wealth::find(&ix.items, &query);
        FindResult { items: ix.items.len(), time: ix.time, league: ix.league.clone(), total: hits.len(),
            hits: hits.into_iter().take(200).cloned().collect() }
    })
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
    last_items: Vec<Valued>,
    sessions: Vec<Session>,
    current: Option<Session>,
    strategies: Vec<(String, usize, f64, f64)>,
    profiles: Vec<ScanProfile>,
    active: Option<String>,
    /// Rares in the last scan, how many have a trade price, and how many need (re)pricing.
    rares: usize,
    rares_priced: usize,
    rares_todo: usize,
    /// "Count rares in my worth" ticked.
    count_rares: bool,
    divine: Option<f64>,
    now: u64,
}

fn view(app: &AppHandle) -> WealthView {
    let state = app.state::<AppState>();
    let s = state.store.lock().unwrap().settings.clone();
    let (_, league) = trade_league(app);
    let st = with_store(&state, |w| w.clone());
    let t = now();
    let fresh = |r: &Rare, age: u64| st.rare_prices.get(&r.id).filter(|p| t.saturating_sub(p.time) < age);
    let rares_priced = st.rares.iter().filter(|r| fresh(r, RARE_KEEP).is_some_and(|p| p.chaos > 0.0)).count();
    let rares_todo = st.rares.iter().filter(|r| fresh(r, RARE_STALE).is_none()).count();
    WealthView {
        rares: st.rares.len(), rares_priced, rares_todo, count_rares: !st.skip_rares,
        account: s.poe_account, has_sessid: !s.poesessid.trim().is_empty(), league,
        tracked: s.wealth_tabs, min: s.wealth_min, minutes: s.earn_minutes.max(1),
        strategies: wealth::by_strategy(&st.sessions),
        tabs: st.tabs, snapshots: st.snapshots, missed: st.missed, last_items: st.last_items, sessions: st.sessions, current: st.current,
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
        let (valued, rares) = scan(&app, tabs.as_deref())?;
        let (_, league) = trade_league(&app);
        let min = state.store.lock().unwrap().settings.wealth_min;
        let mut snap = wealth::snapshot(now(), &league, divine(&app), &valued);
        snap.profile = name;
        let mut items = valued.clone();
        items.sort_by(|a, b| b.value.total_cmp(&a.value));
        items.truncate(3000);
        let t = now();
        with_store(&state, |s| {
            s.add_snapshot(snap); s.missed = wealth::missed(&valued, min); s.last_items = items;
            // Forget prices of rares that are gone (sold, vendored) once they're old.
            s.rare_prices.retain(|id, p| t.saturating_sub(p.time) < RARE_KEEP || rares.iter().any(|r| &r.id == id));
            s.rares = rares;
            // Rares in or out of the total as ticked, and always in "Worth selling".
            add_rares(s, min);
        });
        Ok(view(&app))
    }).await.map_err(err)?
}

/// Prices the last scan's rares on the trade site the way Ctrl+D would (its usual stats ticked, the middle
/// of the cheapest sellers). Slow on purpose: waits for room under the trade site's limit, leaving some for
/// your own price checks, and pauses while you price check by hand. Up to RARE_BATCH rares per click; then
/// adds them to the last scan's total, "By tab" and "Worth selling".
#[tauri::command]
pub async fn wealth_price_rares(app: AppHandle) -> Result<WealthView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (poe2, league) = trade_league(&app);
        let index = trade_api::stats(&state.data_dir, poe2)?;
        let rates = currency_rates(&app);
        let t = now();
        let todo: Vec<Rare> = with_store(&state, |s| s.rares.iter()
            .filter(|r| s.rare_prices.get(&r.id).map(|p| t.saturating_sub(p.time) >= RARE_STALE).unwrap_or(true))
            .take(RARE_BATCH).cloned().collect());
        let n = todo.len();
        let mut failed = None;
        let mut done = 0;
        for (i, r) in todo.iter().enumerate() {
            let _ = app.emit("wealth-progress", (i, n, format!("Pricing {}", r.name)));
            // Hand price checks go first; then wait for room under the limit (at most ~2 minutes per rare).
            for _ in 0..60 {
                let manual = LAST_MANUAL.lock().unwrap().map(|t| t.elapsed() < Duration::from_secs(20)).unwrap_or(false);
                if !manual && trade_api::has_room("search", 2) && trade_api::has_room("fetch", 2) { break; }
                thread::sleep(Duration::from_secs(2));
            }
            let Some(setup) = filter_core::trade::setup(&r.text, &index) else { continue };
            let q = trade_query(&app, poe2, &setup);
            let found = trade_api::search(poe2, &league, &q).and_then(|(id, ids, total)|
                trade_api::fetch(poe2, &ids, &id).map(|f| (filter_core::trade::listings(&f, &rates), total)));
            match found {
                Ok((ls, total)) => {
                    let chaos = filter_core::trade::summary(&ls).map(|s| s.1).unwrap_or(0.0);
                    with_store(&state, |s| { s.rare_prices.insert(r.id.clone(), RarePrice { chaos, time: now(), found: total }); });
                    done += 1;
                }
                // The trade site couldn't search this one item (e.g. "Unknown item base type"): skip it, carry on.
                Err(e) if e.contains(" 400") => {
                    with_store(&state, |s| { s.rare_prices.insert(r.id.clone(), RarePrice { chaos: 0.0, time: now(), found: 0 }); });
                }
                // The limit, or the site is down: stop, keep what we have.
                Err(e) => { failed = Some(e); break; }
            }
        }
        let _ = app.emit("wealth-progress", (n, n, String::new()));
        let min = state.store.lock().unwrap().settings.wealth_min;
        with_store(&state, |s| add_rares(s, min));
        match failed {
            // Nothing priced this time: say why. Otherwise keep what we got (click again for the rest).
            Some(e) if done == 0 => Err(e),
            _ => Ok(view(&app)),
        }
    }).await.map_err(err)?
}

/// Puts the rares' trade prices into the last scan: its total and tabs and "By tab" (unless "Count rares in my
/// worth" is unticked), and "Worth selling" (always).
fn add_rares(s: &mut WealthStore, min: f64) {
    let t = now();
    let old: Vec<Valued> = s.last_items.iter().filter(|v| v.rare).cloned().collect();
    s.last_items.retain(|v| !v.rare);
    let new: Vec<Valued> = s.rares.iter().filter_map(|r| s.rare_prices.get(&r.id)
        .filter(|p| p.chaos > 0.0 && t.saturating_sub(p.time) < RARE_KEEP).map(|p| wealth::rare_valued(r, p.chaos))).collect();
    let counted: &[Valued] = if s.skip_rares { &[] } else { &new };
    s.last_items.extend(counted.iter().cloned());
    s.last_items.sort_by(|a, b| b.value.total_cmp(&a.value));
    s.last_items.truncate(3000);
    let mut sellable = s.last_items.clone();
    if s.skip_rares { sellable.extend(new.iter().cloned()); }
    s.missed = wealth::missed(&sellable, min);
    if let Some(last) = s.snapshots.last_mut() {
        for (v, sign) in old.iter().map(|v| (v, -1.0)).chain(counted.iter().map(|v| (v, 1.0))) {
            last.total += sign * v.value;
            match last.tabs.iter_mut().find(|x| x.0 == v.tab) {
                Some(x) => x.1 += sign * v.value,
                None => last.tabs.push((v.tab.clone(), sign * v.value)),
            }
        }
        last.tabs.retain(|x| x.1 > 0.005);
        last.tabs.sort_by(|a, b| b.1.total_cmp(&a.1));
    }
}


/// "Count rares in my worth": takes the rares in or out of the last scan's totals straight away.
#[tauri::command]
pub fn wealth_count_rares(app: AppHandle, count: bool) -> WealthView {
    let state = app.state::<AppState>();
    let min = state.store.lock().unwrap().settings.wealth_min;
    with_store(&state, |s| { s.skip_rares = !count; add_rares(s, min); });
    view(&app)
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
        let total: f64 = scan(&app, Some(&tabs))?.0.iter().map(|v| v.value).sum();
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
    let total: f64 = scan(app, Some(&cur.tabs))?.0.iter().map(|v| v.value).sum();
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
    // Where you left it; never takes focus from the game.
    if let Some(w) = app.get_webview_window("earnings") { crate::win_place::show_over_game(app, &w); }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rares_in_and_out_of_the_total() {
        let r = Rare { id: "a".into(), tab: "TENDY".into(), name: "Woe Buckle Stygian Vise".into(), text: String::new(),
            icon: String::new(), x: 0, y: 0, listed: false };
        let mut s = WealthStore { rares: vec![r], ..Default::default() };
        s.rare_prices.insert("a".into(), RarePrice { chaos: 40.0, time: now(), found: 5 });
        s.add_snapshot(wealth::snapshot(1, "Covens", None, &[]));
        s.snapshots[0].total = 100.0;
        s.snapshots[0].tabs = vec![("$$$".into(), 100.0)];
        add_rares(&mut s, 10.0);
        assert_eq!(s.snapshots[0].total, 140.0);
        assert_eq!(s.last_items.len(), 1);
        // Twice in a row doesn't count it twice.
        add_rares(&mut s, 10.0);
        assert_eq!(s.snapshots[0].total, 140.0);
        s.skip_rares = true;
        add_rares(&mut s, 10.0);
        assert_eq!((s.snapshots[0].total, s.snapshots[0].tabs.len(), s.last_items.len()), (100.0, 1, 0));
        assert_eq!(s.missed.len(), 1, "still worth selling");
        s.skip_rares = false;
        add_rares(&mut s, 10.0);
        assert_eq!(s.snapshots[0].total, 140.0);
    }
}
