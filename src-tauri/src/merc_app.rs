//! Mercenary checker window ("merc", 28 Nov 2026). When a Mercenary talks to you (the game's log), a small strip comes
//! up over the game without taking the keyboard. While it's there, Ctrl+D on one of the Mercenary's items or skill
//! gems adds it to this window instead of opening the price window: each item is priced (poe.ninja for uniques and
//! the like, else the trade site's "sells for about"), the gems price "a Mercenary like this one" as a warrant
//! search, and the top says kill (take its best item) or recruit. The logic is `filter_core::merc` (tested).
//!
//! A Mercenary you recruited keeps talking in every area: only one first heard in this area counts as new, and the
//! ones you recruit are kept in merc_seen.json, so a restart doesn't bring the strip up for your own Mercenary either.

use crate::{trade_api, win_place, AppState};
use filter_core::merc::{self, MercEvent, MercGem, Verdict};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

/// One of its items.
#[derive(Serialize, Clone)]
pub struct MercItem {
    pub name: String,
    pub base: String,
    pub rarity: String,
    /// The copied text (Details opens the price window with it).
    #[serde(skip)]
    pub text: String,
    /// Chaos, once priced.
    pub chaos: Option<f64>,
    /// "pricing", "ninja", "trade", "none" (no price found), "error".
    pub how: String,
    /// How many are for sale (trade searches).
    pub listed: Option<u64>,
}

#[derive(Serialize, Clone, Default)]
pub struct MercView {
    /// Nobody here: the window says so.
    pub name: String,
    /// "met", "beaten", "joined", "took".
    pub stage: String,
    pub items: Vec<MercItem>,
    pub gems: Vec<MercGem>,
    /// What a Mercenary like it sells for (chaos), how many for sale, "pricing"/"none"/"" (no gems yet).
    pub recruit: Option<f64>,
    pub recruit_listed: Option<u64>,
    pub recruit_how: String,
    pub helps: Vec<String>,
    pub verdict: Option<Verdict>,
    pub divine: Option<f64>,
}

struct Live {
    view: MercView,
    /// Ctrl+D goes to this window until you change area (or 15 minutes pass).
    active: bool,
    since: Instant,
    /// Areas entered since the editor started; each name -> the area it was first heard in.
    area: u32,
    heard: HashMap<String, u32>,
    /// Mercenaries you recruited (saved).
    joined: Vec<String>,
    loaded: bool,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Live) -> T) -> T {
    let mut g = LIVE.lock().unwrap();
    let l = g.get_or_insert_with(|| Live { view: MercView::default(), active: false, since: Instant::now(), area: 1, heard: HashMap::new(), joined: vec![], loaded: false });
    if !l.loaded {
        l.loaded = true;
        // Your recruited Mercenaries count as heard in "area 0": never new again.
        l.joined = filter_core::keep::load(&seen_path(app)).unwrap_or_default();
        for n in l.joined.clone() { l.heard.entry(n).or_insert(0); }
    }
    f(l)
}

fn seen_path(app: &AppHandle) -> std::path::PathBuf { app.state::<AppState>().data_dir.join("merc_seen.json") }

fn save_seen(app: &AppHandle) {
    let mut names = with(app, |l| l.joined.clone());
    if names.len() > 500 { names.drain(..names.len() - 500); }
    if let Ok(t) = serde_json::to_string(&names) { let _ = filter_core::keep::save_text(&seen_path(app), &t); }
}

fn emit(app: &AppHandle) {
    let v = merc_view(app.clone());
    if let Some(w) = app.get_webview_window("merc") { let _ = w.emit("merc-update", &v); }
}

fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("merc") { if !w.is_visible().unwrap_or(false) { win_place::show_over_game(app, &w); } }
}

/// Every new log line (from the log watcher in lib.rs).
pub fn on_line(app: &AppHandle, line: &str) {
    if line.contains("] : You have entered ") {
        // You've left it: Ctrl+D prices normally again. The window stays until you hide it.
        with(app, |l| { l.area += 1; l.active = false; });
        return;
    }
    let known: Vec<String> = with(app, |l| l.heard.keys().cloned().collect());
    let Some(ev) = merc::log_event(line, &known) else { return };
    let (name, stage) = match &ev {
        MercEvent::Met(n) => (n.clone(), "met"), MercEvent::Beaten(n) => (n.clone(), "beaten"),
        MercEvent::Joined(n) => (n.clone(), "joined"), MercEvent::Took(n) => (n.clone(), "took"),
    };
    let (new, current, first_join) = with(app, |l| {
        let first = matches!(ev, MercEvent::Joined(_)) && !l.joined.contains(&name);
        if first { l.joined.push(name.clone()); }
        let here = *l.heard.entry(name.clone()).or_insert(l.area) == l.area;
        let current = l.view.name == name;
        (here && !current, current, first)
    });
    if first_join { save_seen(app); }
    if new {
        // A new Mercenary in this area: start its check.
        with(app, |l| {
            l.view = MercView { name: name.clone(), stage: stage.into(), ..Default::default() };
            l.active = true;
            l.since = Instant::now();
        });
        show(app);
        emit(app);
    } else if current {
        let changed = with(app, |l| {
            // "beaten" once, then whatever you chose.
            let rank = |s: &str| match s { "met" => 0, "beaten" => 1, _ => 2 };
            if rank(stage) > rank(&l.view.stage) { l.view.stage = stage.into(); l.since = Instant::now(); true } else { false }
        });
        if changed { emit(app); }
    }
}

/// Ctrl+D: true when the copy went to the Mercenary window (one is here and hasn't been dealt with long ago).
pub fn take_copy(app: &AppHandle, text: &str) -> bool {
    let on = with(app, |l| l.active && !l.view.name.is_empty() && l.since.elapsed() < Duration::from_secs(15 * 60));
    if !on || text.trim().is_empty() { return false; }
    if let Some(mut g) = merc::gem(text) {
        let (poe2, _) = crate::trade_league(app);
        merc::fill_tier(&mut g, &trade_api::stats(&app.state::<AppState>().data_dir, poe2).unwrap_or_default());
        let added = with(app, |l| {
            if l.view.gems.iter().any(|x| *x == g) { return false; }
            l.view.gems.push(g);
            l.view.recruit_how = "pricing".into();
            true
        });
        if added { emit(app); show(app); price_recruit(app.clone()); }
        return true;
    }
    let payload = crate::text_payload(app, text);
    let Some(setup) = payload.setup.clone() else { return false };
    let item = &setup.item;
    let name = item.name.clone().unwrap_or_else(|| item.base_type.clone());
    let dup = with(app, |l| l.view.items.iter().any(|x| x.text == text));
    if !dup {
        with(app, |l| l.view.items.push(MercItem { name, base: item.base_type.clone(), rarity: item.rarity.clone(), text: text.to_string(),
            chaos: None, how: "pricing".into(), listed: None }));
    }
    show(app);
    emit(app);
    if !dup { price_item(app.clone(), text.to_string(), payload); }
    true
}

/// One trade search at a time for this window (the site's limits are shared with the price window).
static QUEUE: Mutex<()> = Mutex::new(());

fn price_item(app: AppHandle, text: String, payload: crate::PriceCheck) {
    thread::spawn(move || {
        let _turn = QUEUE.lock().unwrap();
        let (chaos, how, listed) = match payload.ninja {
            Some((c, _)) => (Some(c), "ninja", None),
            None => match payload.setup.as_ref().map(|s| crate::trade_search(app.clone(), s.clone())) {
                Some(Ok(r)) => match r.summary { Some((_, typical)) => (Some(typical), "trade", Some(r.total)), None => (None, "none", Some(r.total)) },
                Some(Err(_)) => (None, "error", None),
                None => (None, "none", None),
            },
        };
        with(&app, |l| if let Some(it) = l.view.items.iter_mut().find(|x| x.text == text) { it.chaos = chaos; it.how = how.into(); it.listed = listed; });
        emit(&app);
    });
}

/// "A Mercenary like this one": its skills and supports as a warrant search, cheapest first.
fn price_recruit(app: AppHandle) {
    thread::spawn(move || {
        let _turn = QUEUE.lock().unwrap();
        let (name, gems) = with(&app, |l| (l.view.name.clone(), l.view.gems.clone()));
        let text = merc::warrant_text(&name, &gems);
        let (poe2, _) = crate::trade_league(&app);
        let index = trade_api::stats(&app.state::<AppState>().data_dir, poe2).unwrap_or_default();
        let mut result = (None, None, "none");
        if let Some(mut s) = filter_core::trade::setup(&text, &index) {
            // Its skills too, not just the Tier 3 supports.
            for f in s.filters.iter_mut().filter(|f| f.kind == "mercenary" && f.tier.is_none()) { f.enabled = true; }
            if s.filters.iter().any(|f| f.kind == "mercenary" && f.enabled) {
                result = match crate::trade_search(app.clone(), s) {
                    Ok(r) => match r.summary { Some((_, t)) => (Some(t), Some(r.total), "trade"), None => (None, Some(r.total), "none") },
                    Err(_) => (None, None, "error"),
                };
            }
        }
        with(&app, |l| if l.view.name == name { l.view.recruit = result.0; l.view.recruit_listed = result.1; l.view.recruit_how = result.2.into(); });
        emit(&app);
    });
}

#[tauri::command]
pub fn merc_view(app: AppHandle) -> MercView {
    let divine = crate::currency_rates(&app).get("divine").copied();
    with(&app, |l| {
        let mut v = l.view.clone();
        let skills: Vec<String> = v.gems.iter().filter(|g| !g.support).map(|g| g.name.clone()).collect();
        v.helps = merc::helps(&skills);
        let prices: Vec<Option<f64>> = v.items.iter().map(|i| i.chaos).collect();
        v.verdict = (!v.name.is_empty()).then(|| merc::verdict(&prices, v.recruit, &v.helps));
        v.divine = divine;
        v
    })
}

/// ✕ on a row: not one of its items after all.
#[tauri::command]
pub fn merc_remove(app: AppHandle, index: usize, gem: bool) {
    let again = with(&app, |l| {
        if gem { if index < l.view.gems.len() { l.view.gems.remove(index); return true; } }
        else if index < l.view.items.len() { l.view.items.remove(index); }
        false
    });
    if again { price_recruit(app.clone()); }
    emit(&app);
}

/// Details on a row: the normal price window for that item (to change what's searched).
#[tauri::command]
pub fn merc_details(app: AppHandle, index: usize) {
    let Some(text) = with(&app, |l| l.view.items.get(index).map(|i| i.text.clone())) else { return };
    thread::spawn(move || {
        let payload = crate::text_payload(&app, &text);
        crate::show_price(&app, payload, &text, false);
    });
}

/// "A Mercenary like it" in the price window, to see the listings or change the supports searched.
#[tauri::command]
pub fn merc_recruit_details(app: AppHandle) {
    let text = with(&app, |l| merc::warrant_text(&l.view.name, &l.view.gems));
    thread::spawn(move || {
        let payload = crate::text_payload(&app, &text);
        crate::show_price(&app, payload, &text, false);
    });
}

/// Hide (Esc / ✕). Ctrl+D goes back to the price window too.
#[tauri::command]
pub fn merc_hide(app: AppHandle) {
    with(&app, |l| l.active = false);
    if let Some(w) = app.get_webview_window("merc") { let _ = w.hide(); }
    crate::focus_game();
}
