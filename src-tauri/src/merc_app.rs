//! Mercenary checker window ("merc", 28 Nov 2026). When a Mercenary talks to you (the game's log), a small strip comes
//! up over the game without taking the keyboard. While it's there, Ctrl+D on one of the Mercenary's items or skill
//! gems adds it to this window instead of opening the price window: each item is priced (poe.ninja for uniques and
//! the like, else the trade site's "sells for about"), the gems price "a Mercenary like this one" as a warrant
//! search, and the top says kill (take its best item) or recruit. The logic is `filter_core::merc` (tested).
//!
//! A Mercenary you recruited keeps talking in every area: only one first heard in this area counts as new, and the
//! ones you recruit are kept in merc_seen.json, so a restart doesn't bring the strip up for your own Mercenary either.
//!
//! Its skills and supports are read off the screen (28 Nov, Ctrl+D copies nothing from them): from the beaten stage the
//! window after the duel is looked for every second or so, and Ctrl+D on its skill list reads it again. Supports are
//! told apart by their pictures (`filter_core::merc_screen`); one not known yet is learned when you hover it and
//! press Ctrl+D (its hover box names it), kept in merc_icons.json.

use crate::{trade_api, win_place, AppState};
use filter_core::merc::{self, MercEvent, MercGem, Verdict};
use filter_core::merc_screen::{self as ms, Known, SeenSkill};
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

/// A support read off the screen: its trade name ("Greater Brutality") once its picture is known.
#[derive(Serialize, Clone)]
pub struct ViewSupport { pub name: Option<String>, pub family: Option<String>, pub tier: Option<u32>, pub brick: bool }

/// A skill read off the screen, with its supports left to right.
#[derive(Serialize, Clone)]
pub struct ViewSkill { pub name: String, pub supports: Vec<ViewSupport>, pub brick: bool }

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
    /// Its skills as read off the screen.
    pub skills: Vec<ViewSkill>,
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
    /// Its skill rows as last read off the screen (with the support pictures).
    seen: Vec<SeenSkill>,
    /// Known support pictures: built in, then learned (merc_icons.json).
    book: Option<Vec<Known>>,
    /// The screen is being watched for its window.
    watching: bool,
}

static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Live) -> T) -> T {
    let mut g = LIVE.lock().unwrap();
    let l = g.get_or_insert_with(|| Live { view: MercView::default(), active: false, since: Instant::now(), area: 1, heard: HashMap::new(), joined: vec![], loaded: false, seen: vec![], book: None, watching: false });
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
            l.seen.clear();
            l.active = true;
            l.since = Instant::now();
        });
        show(app);
        emit(app);
        if stage == "beaten" { watch(app.clone()); }
    } else if current {
        let changed = with(app, |l| {
            // "beaten" once, then whatever you chose.
            let rank = |s: &str| match s { "met" => 0, "beaten" => 1, _ => 2 };
            if rank(stage) > rank(&l.view.stage) { l.view.stage = stage.into(); l.since = Instant::now(); true } else { false }
        });
        if changed { emit(app); if stage == "beaten" { watch(app.clone()); } }
    }
}

/// Ctrl+D: true when the copy went to the Mercenary window (one is here and hasn't been dealt with long ago).
pub fn take_copy(app: &AppHandle, text: &str) -> bool {
    let on = with(app, |l| l.active && !l.view.name.is_empty() && l.since.elapsed() < Duration::from_secs(15 * 60));
    if !on { return false; }
    // Its skill list copies nothing: read it off the screen (and learn the support under the mouse, if it's new).
    // Anywhere else (an item on the ground), the normal price check carries on.
    if text.trim().is_empty() { return read_panel(app, true); }
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

fn book(app: &AppHandle) -> Vec<Known> {
    if let Some(b) = with(app, |l| l.book.clone()) { return b; }
    let mut b = ms::seed();
    b.extend(filter_core::keep::load::<Vec<Known>>(&icons_path(app)).unwrap_or_default());
    with(app, |l| l.book = Some(b.clone()));
    b
}

fn icons_path(app: &AppHandle) -> std::path::PathBuf { app.state::<AppState>().data_dir.join("merc_icons.json") }

/// A support picture named by its hover box: kept, so it's known from now on.
fn learn(app: &AppHandle, family: &str, print: &[u8]) {
    let mut learned: Vec<Known> = filter_core::keep::load(&icons_path(app)).unwrap_or_default();
    learned.push(Known { name: family.to_string(), print: ms::to_hex(print) });
    if learned.len() > 2000 { learned.drain(..learned.len() - 2000); }
    if let Ok(t) = serde_json::to_string(&learned) { let _ = filter_core::keep::save_text(&icons_path(app), &t); }
    with(app, |l| l.book = None);
}

/// Reads its window off the screen. True when it was there (its buttons or skill rows).
fn read_panel(app: &AppHandle, from_hover: bool) -> bool {
    let Some((gx, gy, gw, gh)) = crate::game_client_rect() else { return false };
    let Ok(g) = crate::screen::grab_and_read(gx, gy, gw, gh) else { return false };
    // Our own window's text (its skill names) mustn't be read as the game's.
    let ours = app.get_webview_window("merc").filter(|w| w.is_visible().unwrap_or(false))
        .and_then(|w| Some((w.outer_position().ok()?, w.outer_size().ok()?)))
        .map(|(p, s)| [(p.x - g.x) as f32, (p.y - g.y) as f32, s.width as f32, s.height as f32]);
    let lines: Vec<(String, [f32; 4])> = g.lines.into_iter().filter(|(_, r)| ours.map_or(true, |o| {
        let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        !(cx >= o[0] && cx <= o[0] + o[2] && cy >= o[1] && cy <= o[1] + o[3])
    })).collect();
    let (poe2, _) = crate::trade_league(app);
    let index = trade_api::stats(&app.state::<AppState>().data_dir, poe2).unwrap_or_default();
    let (skills, supports): (Vec<String>, Vec<String>) = index.merc.iter().cloned().partition(|n| !n.contains(" (Tier "));
    let pic = ms::Pic { px: &g.px, w: g.w as usize, h: g.h as usize, bgra: true };
    let rows = ms::read_rows(&pic, &lines, &skills);
    if rows.is_empty() && !ms::panel_open(&lines) { return false; }
    if from_hover {
        // The support under the mouse, named by its hover box.
        let (mx, my) = (g.mouse.0.max(0) as usize, g.mouse.1.max(0) as usize);
        let under = rows.iter().flat_map(|r| &r.icons).find(|i| { let [x, y, s] = i.rect; let m = s / 8;
            mx + m >= x && mx <= x + s + m && my + m >= y && my <= y + s + m });
        if let (Some(i), Some(fam)) = (under, ms::tooltip_support(&lines, &supports)) { learn(app, &fam, &i.print); }
    }
    with(app, |l| {
        for r in rows {
            match l.seen.iter_mut().find(|s| s.name == r.name) {
                // A hover box can hide some of a row: keep the fuller read.
                Some(s) => if r.icons.len() >= s.icons.len() { *s = r; },
                None => l.seen.push(r),
            }
        }
        l.seen.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    });
    apply(app, &supports);
    show(app);
    true
}

/// Names what was read off the screen; the gems price "a Mercenary like it" again when they changed.
fn apply(app: &AppHandle, names: &[String]) {
    let book = book(app);
    let changed = with(app, |l| {
        let main = l.seen.iter().max_by_key(|s| s.icons.len()).map(|s| s.name.clone()).unwrap_or_default();
        let mut skills = vec![];
        let mut gems = vec![];
        for s in &l.seen {
            let sups: Vec<ViewSupport> = s.icons.iter().map(|i| {
                let fam = ms::identify(&i.print, &book).map(|(n, _)| n.to_string());
                let named = fam.as_deref().and_then(|f| ms::tier_name(f, i.tier, names));
                ViewSupport { name: named.as_ref().map(|n| n.0.clone()), family: fam, tier: named.as_ref().map(|n| n.1).or(i.tier), brick: false }
            }).collect();
            gems.push(MercGem { name: s.name.clone(), support: false, tier: None });
            for p in &sups { if let Some(n) = &p.name { gems.push(MercGem { name: n.clone(), support: true, tier: p.tier }); } }
            skills.push(ViewSkill { name: s.name.clone(), supports: sups, brick: false });
        }
        let labels: Vec<String> = skills.iter().flat_map(|s| std::iter::once(s.name.clone())
            .chain(s.supports.iter().filter_map(|p| Some(format!("{} (Tier {})", p.name.as_ref()?, p.tier?))))).collect();
        let bad = merc::bricks(&main, &labels);
        for s in &mut skills {
            s.brick = bad.contains(&s.name);
            for p in &mut s.supports { p.brick = p.name.as_ref().zip(p.tier).map_or(false, |(n, t)| bad.contains(&format!("{n} (Tier {t})"))); }
        }
        l.view.skills = skills;
        let changed = l.view.gems != gems;
        if changed { l.view.gems = gems; l.view.recruit_how = "pricing".into(); }
        changed
    });
    emit(app);
    if changed { price_recruit(app.clone()); }
}

/// From the beaten stage: looks for its window every second or so, until it closes (or a few minutes pass).
fn watch(app: AppHandle) {
    if with(&app, |l| std::mem::replace(&mut l.watching, true)) { return; }
    thread::spawn(move || {
        let start = Instant::now();
        let (mut seen, mut misses) = (false, 0);
        while with(&app, |l| l.active) && start.elapsed() < Duration::from_secs(if seen { 600 } else { 180 }) {
            thread::sleep(Duration::from_millis(1200));
            if read_panel(&app, false) { seen = true; misses = 0; } else if seen { misses += 1; if misses >= 4 { break; } }
        }
        with(&app, |l| l.watching = false);
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
