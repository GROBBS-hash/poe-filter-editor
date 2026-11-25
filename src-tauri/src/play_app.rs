//! Play helpers over the game (22 Nov 2026, from the "what do players want" list):
//! - Levelling overlay ("level" window): the route step you're on, following the zones you enter (exile-leveling's
//!   route), quest gems from your PoB build, zone level vs yours, and a vendor search for your build's gems.
//! - Map tracker ("tracker" window): time in each map, maps per hour, deaths, mods from a Ctrl+D map check just
//!   before, what each map dropped (your bags on pathofexile.com before and after, priced), the timer stopping in the
//!   Escape menu / skill tree (a few pixels of the screen); your level; a stopwatch.
//! - Search strings ("searches" window): saved stash/vendor searches typed into the game's search box with one click.
//! - Cheat sheets ("sheet" window): the Syndicate table and your own pictures.
//! - Notes ("notes" window), and a wiki key (opens the wiki page of the item under the mouse).
//!
//! Everything here reads the game's own log (Client.txt), copied item text, or pathofexile.com: nothing reads the
//! game's memory.

use super::*;
use filter_core::journey::{self, MapChange, MapRun, MapTracker, MenuLook, PlayEvent, RouteOptions, Steady};
use filter_core::loot::{self, LootLine, LootPrefs};
use std::collections::HashMap;

/// A saved search: a name and what to type into the game's search box.
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SearchString { pub name: String, pub text: String }

/// Kept in play.json in the data folder.
#[derive(Serialize, serde::Deserialize, Clone, Default)]
#[serde(default)]
struct PlayStore {
    maps: MapTracker,
    /// Levelling route step per character (index into the steps for your options).
    level_pos: HashMap<String, usize>,
    route: RouteOptions,
    searches: Vec<SearchString>,
    /// The levelling overlay follows the zones you enter (off: you move it with the arrows).
    level_manual: bool,
    /// Price alerts, checked against poe.ninja each hourly price update.
    alerts: Vec<Alert>,
    /// Hide the levelling, searches, cheat sheet and notes windows while you're in a map (back in town they return).
    keep_in_maps: bool,
    /// Chat commands ("!next" in game chat) switched off.
    chat_off: bool,
    /// Race splits per character (when each act and level was reached).
    races: journey::Races,
    /// Don't stop the map timer while the Escape menu or skill tree is open.
    pause_off: bool,
    /// How the game looks with no menu open (and with the Escape menu), from "Set up" on the Play tools page.
    menu: MenuLook,
    /// Keep the tracker up while the stash or inventory is open under it.
    over_panels: bool,
    /// The loot breakdown per map (Settings).
    loot: LootPrefs,
}

/// "Tell me when Mageblood is under 150 divines".
#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Alert {
    /// The item's name as poe.ninja has it (a unique's name, or a currency/card/base name).
    pub name: String,
    /// "below" or "above".
    pub when: String,
    pub value: f64,
    /// "chaos" or "divine".
    pub unit: String,
    /// Last price seen (chaos) and when the alert last went off (unix seconds), so it tells you once per crossing.
    pub last: Option<f64>,
    pub fired: Option<i64>,
}

/// What the tracker shows but the log can't say again after a relaunch; kept in play_live.json.
#[derive(Serialize, serde::Deserialize, Clone, Default)]
#[serde(default)]
struct Kept {
    /// When this was saved (the editor was running), and the last game event it had seen.
    at: i64,
    sess_start: i64,
    sess_last: i64,
    sess_deaths: u32,
    sess_levels: u32,
    watch_since: Option<i64>,
    watch_acc: i64,
    laps: Vec<i64>,
    site_char: Option<(String, u32)>,
    loot_base: Option<(RunKey, Vec<serde_json::Value>)>,
    loot_note: String,
    map_check: Option<(i64, String, Vec<String>)>,
    /// Play windows put away for the map you're in (they return in town).
    hidden: Vec<String>,
}

fn kept_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("play_live.json") }
/// What the last run left (read once, at the start), until the log has been caught up with.
static LAST_RUN: Mutex<Option<Kept>> = Mutex::new(None);
/// Loaded: from now on it may be written.
static KEPT_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Every few seconds, and just before a restart: what the tracker shows now.
pub fn keep_live(app: &AppHandle) {
    if !KEPT_READY.load(Ordering::Relaxed) { return; }
    let hidden = HIDDEN_FOR_MAP.lock().unwrap().clone();
    let k = with_live(|l| Kept {
        at: unix(), sess_start: l.sess_start, sess_last: l.sess_last, sess_deaths: l.sess_deaths, sess_levels: l.sess_levels,
        watch_since: l.watch_since, watch_acc: l.watch_acc, laps: l.laps.clone(), site_char: l.site_char.clone(),
        loot_base: l.loot_base.clone(), loot_note: l.loot_note.clone(), map_check: l.map_check.clone(), hidden,
    });
    let Ok(t) = serde_json::to_string(&k) else { return };
    let p = kept_path(app);
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, p); }
}

/// At the start: the tracker picks up where the last run left it (the session only when it's still going).
fn load_kept(app: &AppHandle) {
    if let Some(k) = std::fs::read_to_string(kept_path(app)).ok().and_then(|t| serde_json::from_str::<Kept>(&t).ok()) {
        with_live(|l| {
            if unix() - k.sess_last < 30 * 60 { l.sess_start = k.sess_start; l.sess_last = k.sess_last; l.sess_deaths = k.sess_deaths; l.sess_levels = k.sess_levels; }
            l.watch_since = k.watch_since;
            l.watch_acc = k.watch_acc;
            l.laps = k.laps.clone();
            l.site_char = k.site_char.clone();
            l.loot_base = k.loot_base.clone();
            l.loot_note = k.loot_note.clone();
            l.map_check = k.map_check.clone();
        });
        // The game went on (or not) without us for long: the clock stands still from then until you're seen playing.
        if unix() - k.at > 10 * 60 { with_store(app, |s| s.maps.set_paused(true, k.at)); }
        *LAST_RUN.lock().unwrap() = Some(k);
    }
    KEPT_READY.store(true, Ordering::Relaxed);
}

/// Once the log is found: what happened while the editor was closed (left the map, a new one, deaths) is counted too.
fn catch_up(app: &AppHandle, log: &str) {
    let Some(k) = LAST_RUN.lock().unwrap().take() else { return };
    // No event seen yet last time, or too long ago: only where you are now matters.
    let since = if k.sess_last > 0 && unix() - k.at < 12 * 3600 { k.sess_last } else { k.at };
    let events = journey::missed(log, utc_offset(), since);
    if events.is_empty() { return; }
    for ev in &events {
        let changes = with_store(app, |s| s.maps.on_event(ev));
        map_visibility(app, &changes);
        let t = match ev { PlayEvent::Area { time, .. } | PlayEvent::Entered { time, .. } | PlayEvent::Level { time, .. } | PlayEvent::Slain { time, .. } => *time };
        for c in &changes {
            let Some(key) = with_store(app, |s| s.maps.current.as_ref().map(run_key)) else { continue };
            // Loot only when it's fresh: after minutes in town your bags say nothing about the map any more.
            let fresh = unix() - t < 90;
            match c {
                MapChange::Started => {
                    let atlas = atlas_app::active_id(app);
                    with_store(app, |s| if let Some(cur) = s.maps.current.as_mut() { cur.atlas = atlas; });
                    if fresh { queue_loot(app, LootJob::Enter(key, t)); } else { loot_missed(app, &key); }
                }
                MapChange::Resumed => if fresh { queue_loot(app, LootJob::Enter(key, t)); } else { loot_missed(app, &key); },
                MapChange::Left => if fresh { queue_loot(app, LootJob::Leave(key, t)); } else { loot_missed(app, &key); },
                _ => {}
            }
        }
        with_live(|l| {
            if t - l.sess_last > 30 * 60 { l.sess_start = t; l.sess_deaths = 0; l.sess_levels = 0; }
            l.sess_last = t;
            match ev {
                PlayEvent::Slain { name, .. } if l.character.as_ref().map_or(true, |c| &c.0 == name) => l.sess_deaths += 1,
                PlayEvent::Level { .. } => l.sess_levels += 1,
                _ => {}
            }
        });
        if let PlayEvent::Area { id, .. } = ev { follow_route(app, id); }
    }
    // Only a session still going is shown.
    with_live(|l| if unix() - l.sess_last > 30 * 60 { l.sess_start = 0; });
    save_store(app);
}

/// The PC's clock minus UTC, in seconds (the log is in local time).
#[cfg(windows)]
fn utc_offset() -> i64 {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let s = format!("{:04}/{:02}/{:02} {:02}:{:02}:{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    let diff = journey::log_time(&s).unwrap_or_else(unix) - unix();
    (diff as f64 / 900.0).round() as i64 * 900
}
#[cfg(not(windows))]
fn utc_offset() -> i64 { 0 }

/// Play windows put away for a map, for the list of windows to reopen; and put back as put away after a relaunch.
pub fn reopen_hidden(app: &AppHandle) -> Vec<String> {
    let k = std::fs::read_to_string(kept_path(app)).ok().and_then(|t| serde_json::from_str::<Kept>(&t).ok()).unwrap_or_default();
    k.hidden
}
/// After a relaunch: windows to open now; ones put away for the map you're still in stay put away.
pub fn still_hidden(app: &AppHandle, label: &str) -> bool {
    if !with_store(app, |s| s.maps.inside && !s.keep_in_maps) { return false; }
    let mut h = HIDDEN_FOR_MAP.lock().unwrap();
    if !h.iter().any(|x| x == label) { h.push(label.into()); }
    true
}

#[derive(Default)]
struct Live {
    /// (area id, area level, when).
    zone: Option<(String, u32, i64)>,
    zone_name: String,
    /// (name, class, level) from the log.
    character: Option<(String, String, u32)>,
    /// Your character on pathofexile.com: (name, level). (The site stopped sending experience in Nov 2026, so there's
    /// no XP per hour any more.)
    site_char: Option<(String, u32)>,
    /// The menu check: smoothing, and a note for the Play tools page.
    steady: Steady,
    pause_note: String,
    /// Your bags when you last went into the current map: (run, items).
    loot_base: Option<(RunKey, Vec<serde_json::Value>)>,
    loot_note: String,
    /// Bag reads still to come after leaving a map (its loot shows when they're done).
    loot_reading: u32,
    /// Monsters left in this zone, and when the game last said so.
    left: Option<(journey::MonstersLeft, i64)>,
    /// Stopwatch: running since (unix ms), time banked (ms), laps (ms).
    watch_since: Option<i64>,
    watch_acc: i64,
    laps: Vec<i64>,
    /// The last Ctrl+D map check: (when, map name, mods), attached to the next map you open.
    map_check: Option<(i64, String, Vec<String>)>,
    /// This play session (starts with the first game event after 30 quiet minutes): start, last event, deaths,
    /// levels gained, experience at the start.
    sess_start: i64,
    sess_last: i64,
    sess_deaths: u32,
    sess_levels: u32,
}

/// Which run: (area id, seed, start).
type RunKey = (String, u64, i64);
fn run_key(r: &MapRun) -> RunKey { (r.area.clone(), r.seed, r.start) }

static STORE: Mutex<Option<PlayStore>> = Mutex::new(None);
static LIVE: Mutex<Option<Live>> = Mutex::new(None);

fn store_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("play.json") }

fn with_store<T>(app: &AppHandle, f: impl FnOnce(&mut PlayStore) -> T) -> T {
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(|| std::fs::read_to_string(store_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(default_store));
    f(s)
}

fn save_store(app: &AppHandle) {
    let text = with_store(app, |s| serde_json::to_string(s).ok());
    if let Some(t) = text {
        let p = store_path(app);
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, p); }
    }
}

fn with_live<T>(f: impl FnOnce(&mut Live) -> T) -> T { f(LIVE.lock().unwrap().get_or_insert_with(Live::default)) }

fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }
fn unix_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }

/// Searches players use a lot (editable on the Play tools page).
fn default_store() -> PlayStore {
    let s = |n: &str, t: &str| SearchString { name: n.into(), text: t.into() };
    PlayStore {
        searches: vec![
            s("Movement speed (vendor)", "\"nt sp\""),
            s("3-links (vendor)", "\"-\\w-\\w\""),
            s("Red-green-blue (chromatic recipe)", "\"r-g-b|r-b-g|b-r-g|b-g-r|g-r-b|g-b-r\""),
            s("Life and resistances", "\"to maximum life|all elemental|to fire res|to cold res|to lightning res\""),
            s("Quality gems (gemcutter's recipe)", "\"quality: \\+\""),
        ],
        ..Default::default()
    }
}

// ---------- the log ----------

/// Called once when the editor starts: where you are and who you're playing, from the end of the log.
pub fn prime(app: &AppHandle, log: &std::path::Path) {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(log) else { return };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if f.seek(SeekFrom::Start(len.saturating_sub(4 * 1024 * 1024))).is_err() { return; }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() { return; }
    let text = String::from_utf8_lossy(&buf);
    with_live(|l| {
        for line in text.lines() {
            match journey::parse_line(line) {
                Some(PlayEvent::Area { level, id, time, .. }) => { l.zone = Some((id, level, time)); }
                Some(PlayEvent::Entered { name, .. }) => l.zone_name = name,
                Some(PlayEvent::Level { name, class, level, .. }) => l.character = Some((name, class, level)),
                _ => {}
            }
        }
    });
    if let Some((name, ..)) = with_live(|l| l.character.clone()) { with_store(app, |s| s.maps.character = Some(name)); }
    catch_up(app, &text);
    // Started while you're inside a map (an update, a restart) without its bags: the site still has them as you went in.
    if let Some(run) = with_store(app, |s| s.maps.current.as_ref().filter(|_| s.maps.inside).map(run_key)) {
        let at = with_live(|l| {
            let have = l.loot_base.as_ref().is_some_and(|(k, _)| *k == run);
            l.zone.as_ref().filter(|z| !have && z.0 == run.0).map(|z| z.2 - utc_offset())
        });
        if let Some(at) = at { queue_loot(app, LootJob::Enter(run, at)); }
    }
    // A session still going shows straight away, not only from the next zone.
    if with_live(|l| l.sess_start == 0) {
        if let Some((start, deaths)) = with_store(app, |s| journey::session_start(&s.maps.history, s.maps.current.as_ref(), unix(), 30 * 60)) {
            with_live(|l| { l.sess_start = start; l.sess_deaths = deaths; l.sess_last = unix(); });
        }
    }
    emit_all(app);
}

/// Every new log line goes through here (from the log watcher in lib.rs).
pub fn on_line(app: &AppHandle, line: &str) {
    // "!next" and friends typed in game chat (or whispered to yourself).
    if line.contains(": !") && !with_store(app, |s| s.chat_off) {
        let me = with_live(|l| l.character.as_ref().map(|c| c.0.clone())).unwrap_or_default();
        if let Some(cmd) = journey::chat_command(line, &me) { run_chat_command(app, &cmd); return; }
    }
    // /remaining's answer (whoever typed it): ": 7 monsters remain."
    if line.contains("] : ") && line.contains("remain") {
        if let Some(m) = journey::monsters_left(line) { set_left(app, Some(m)); return; }
    }
    let Some(mut ev) = journey::parse_line(line) else { return };
    // The log's clock is your PC's local time; lines arrive as they're written, so stamp them with the real time
    // (the timers compare against it).
    match &mut ev {
        PlayEvent::Area { time, .. } | PlayEvent::Entered { time, .. } | PlayEvent::Level { time, .. } | PlayEvent::Slain { time, .. } => *time = unix(),
    }
    let changes = with_store(app, |s| s.maps.on_event(&ev));
    map_visibility(app, &changes);
    let now_run = with_store(app, |s| s.maps.current.as_ref().map(run_key));
    let mut save = false;
    for c in &changes {
        // Loot: your bags as you go into the map, and again as you come out.
        match c {
            MapChange::Started | MapChange::Resumed => if let Some(k) = &now_run { queue_loot(app, LootJob::Enter(k.clone(), unix())); },
            MapChange::Left => if let Some(k) = &now_run { queue_loot(app, LootJob::Leave(k.clone(), unix())); },
            _ => {}
        }
        match c {
            MapChange::Started => {
                // A Ctrl+D map check in the last 10 minutes: that's the map you just opened.
                let check = with_live(|l| l.map_check.take()).filter(|(t, ..)| unix() - t < 600);
                let value = session_value(app);
                let atlas = atlas_app::active_id(app);
                with_store(app, |s| if let Some(cur) = s.maps.current.as_mut() {
                    if let Some((_, _, mods)) = &check { cur.mods = mods.clone(); }
                    cur.value_at_start = value;
                    cur.atlas = atlas;
                });
                save = true;
            }
            MapChange::Finished(_) => {
                // Paused for most of the map: the bar's normal look was learnt wrong, so learn it again next map.
                let relearn = with_store(app, |s| {
                    let bad = s.maps.history.last().is_some_and(|l| journey::pause_look_suspect(l.in_map, l.paused));
                    if bad { s.menu.hud.clear(); }
                    bad
                });
                if relearn { with_live(|l| l.steady.reset()); }
                // Roughly what the earnings session made over that map.
                let now_value = session_value(app);
                with_store(app, |s| if let Some(last) = s.maps.history.last_mut() {
                    if let (Some(a), Some(b)) = (last.value_at_start.take(), now_value) { last.made = Some(b - a); }
                });
                save = true;
            }
            MapChange::Left | MapChange::Resumed | MapChange::Death => save = true,
            MapChange::None => {}
        }
    }
    // The session: a new one after 30 quiet minutes. When the editor starts mid-session (a restart or an update),
    // it carries on from the maps you'd already run.
    let fresh = with_live(|l| l.sess_start == 0 || unix() - l.sess_last > 30 * 60);
    let carried = if fresh { with_store(app, |s| journey::session_start(&s.maps.history, s.maps.current.as_ref(), unix(), 30 * 60)) } else { None };
    with_live(|l| {
        let now = unix();
        if fresh {
            let (start, deaths) = carried.unwrap_or((now, 0));
            l.sess_start = start; l.sess_deaths = deaths; l.sess_levels = 0;
        }
        l.sess_last = now;
        match &ev {
            PlayEvent::Slain { name, .. } if l.character.as_ref().map_or(true, |c| &c.0 == name) => l.sess_deaths += 1,
            PlayEvent::Level { .. } => l.sess_levels += 1,
            _ => {}
        }
    });
    match &ev {
        PlayEvent::Area { id, level, time, .. } => {
            with_live(|l| { l.zone = Some((id.clone(), *level, *time)); l.zone_name.clear(); l.left = None; });
            follow_route(app, id);
            // Race splits: the first time this character enters each act (maps count as act 11).
            let poe2 = is_poe2(app);
            let act = journey::route_for(poe2).areas.get(&journey::area_key(poe2, id)).map(|a| a.a)
                .filter(|a| !poe2 || *a <= 6).or_else(|| journey::is_map_area(id).then_some(11));
            if let (Some(act), Some(me)) = (act, with_live(|l| l.character.as_ref().map(|c| c.0.clone()))) {
                with_store(app, |s| s.races.act(&me, act, *time));
                save = true;
            }
        }
        PlayEvent::Entered { name, .. } => { with_live(|l| l.zone_name = name.clone()); save = true; }
        PlayEvent::Level { name, class, level, time } => {
            with_live(|l| l.character = Some((name.clone(), class.clone(), *level)));
            with_store(app, |s| s.races.level(name, class, *level, *time));
            save = true;
        }
        _ => {}
    }
    if save { save_store(app); }
    keep_live(app);
    emit_all(app);
}

/// The value an earnings session has right now (None when none is running).
fn session_value(app: &AppHandle) -> Option<f64> {
    let v = wealth_app::wealth_get(app.clone());
    v.current.filter(|c| c.end.is_none() && c.paused_at.is_none()).map(|c| c.last_value)
}

/// The Ctrl+D map window calls this so the next map you open shows its mods.
pub fn note_map_check(name: &str, mods: Vec<String>) {
    with_live(|l| l.map_check = Some((unix(), name.to_string(), mods)));
}

fn emit_all(app: &AppHandle) {
    let v = play_view(app.clone());
    for label in ["level", "tracker", "main"] {
        if let Some(w) = app.get_webview_window(label) { let _ = w.emit("play-update", &v); }
    }
}

// ---------- levelling ----------

fn char_key() -> String { with_live(|l| l.character.as_ref().map(|c| c.0.clone())).unwrap_or_else(|| "_".into()) }

/// Is the active filter for PoE 2 (the levelling route follows the game)?
fn is_poe2(app: &AppHandle) -> bool { active_game(&app.state::<AppState>()) == Game::Poe2 }

fn follow_route(app: &AppHandle, area: &str) {
    let poe2 = is_poe2(app);
    let r = journey::route_for(poe2);
    let area = &journey::area_key(poe2, area);
    let key = char_key();
    let moved = with_store(app, |s| {
        if s.level_manual { return false; }
        let vis = journey::steps_for(r, &s.route);
        let pos = *s.level_pos.get(&key).unwrap_or(&0);
        match journey::advance(r, &vis, pos, area) {
            Some(p) if p != pos => { s.level_pos.insert(key.clone(), p); true }
            _ => false,
        }
    });
    if moved { save_store(app); }
}

/// One route step for the overlay.
#[derive(Serialize, Clone)]
pub struct StepView {
    pub act: u32,
    /// (kind, text to show): kinds as in the route ("enter" etc.) with zone ids already turned into names.
    pub parts: Vec<(String, String)>,
    pub subs: Vec<String>,
    /// Quest gems for your build: (gem, true = pick as the reward, npc).
    pub rewards: Vec<(String, bool, String)>,
}

#[derive(Serialize, Clone)]
pub struct LevelView {
    pub pos: usize,
    pub total: usize,
    /// The previous step (done), the current one and a few after.
    pub steps: Vec<StepView>,
    /// Index of the current step within `steps`.
    pub current: usize,
    pub zone: Option<String>,
    pub zone_level: Option<u32>,
    pub char_level: Option<u32>,
    pub class: Option<String>,
    /// Share of XP you keep in this zone (1 = no penalty).
    pub xp_keep: Option<f64>,
    /// Vendor search for your build's gems ("" without a PoB build).
    pub gem_search: String,
    pub gems_left_out: Vec<String>,
    pub options: RouteOptions,
    pub manual: bool,
    pub credit: String,
    /// The Trials of Ascendancy on the route: (zone, which lab, done = the route is past it, here = you're in that zone).
    pub trials: Vec<Trial>,
    /// Resistance penalty from the acts you've finished (0, -30 after act 5, -60 after act 10).
    pub res_penalty: i32,
    /// Your filter is for PoE 2: the route is PoE 1's campaign.
    pub poe2: bool,
}

#[derive(Serialize, Clone)]
pub struct Trial { pub zone: String, pub lab: String, pub done: bool, pub here: bool }

fn build_gems(app: &AppHandle) -> (Vec<String>, Option<String>) {
    let state = app.state::<AppState>();
    let Some(p) = pob_load(&state) else { return (vec![], None) };
    let mut gems: Vec<String> = vec![];
    // Every stage's gems up to and including the one you're on (later stages' gems aren't needed yet).
    for st in p.build.stages.iter().take(p.stage + 1) {
        for g in st.groups.iter().filter(|g| g.enabled).flat_map(|g| &g.gems) {
            // The route's data names supports "Arcane Surge Support"; PoB may leave the " Support" off.
            let n = if g.support && !g.name.ends_with(" Support") { format!("{} Support", g.name) } else { g.name.clone() };
            if !gems.iter().any(|x| *x == n) { gems.push(n); }
        }
    }
    (gems, Some(p.build.class.clone()))
}

fn step_view(r: &journey::Route, s: &journey::Step, class: &str, gems: &[String]) -> StepView {
    let zone = |id: &str| r.areas.get(id).map(|z| z.n.clone()).unwrap_or_else(|| id.to_string());
    let mut rewards = vec![];
    let parts = s.parts.iter().map(|(k, v)| {
        let text = match k.as_str() {
            "enter" => zone(v),
            "waypoint" if !v.is_empty() => format!("Waypoint to {}", zone(v)),
            "quest" => {
                if let Some(q) = r.quests.get(v) { rewards.extend(journey::build_rewards(q, class, gems)); }
                r.quests.get(v).map(|q| q.name.clone()).unwrap_or_else(|| v.clone())
            }
            "text" | "kill" | "quest_text" | "generic" | "arena" => v.clone(),
            _ => journey::step_text(r, &journey::Step { act: s.act, parts: vec![(k.clone(), v.clone())], subs: vec![], cond: vec![] }),
        };
        (k.clone(), text)
    }).collect();
    StepView { act: s.act, parts, subs: s.subs.clone(), rewards }
}

#[tauri::command]
pub fn level_view(app: AppHandle) -> LevelView {
    let poe2 = is_poe2(&app);
    let r = journey::route_for(poe2);
    let key = char_key();
    let (opts, pos, manual) = with_store(&app, |s| (s.route.clone(), *s.level_pos.get(&key).unwrap_or(&0), s.level_manual));
    let vis = journey::steps_for(r, &opts);
    let pos = pos.min(vis.len().saturating_sub(1));
    let (gems, pob_class) = build_gems(&app);
    let (zone, zone_name, character) = with_live(|l| (l.zone.clone(), l.zone_name.clone(), l.character.clone()));
    // Your class: the log's ("Necromancer" is an ascendancy: PoB's base class is what quests use), else PoB's.
    let class = pob_class.clone().or_else(|| character.as_ref().map(|c| c.1.clone())).unwrap_or_default();
    let from = pos.saturating_sub(1);
    let steps: Vec<StepView> = vis[from..vis.len().min(pos + 6)].iter().map(|&i| step_view(r, &r.steps[i], &class, &gems)).collect();
    let (gem_search, left) = journey::gem_search(&gems, &r.gems);
    let zone_level = zone.as_ref().map(|z| z.1);
    let char_level = character.as_ref().map(|c| c.2);
    let zone_label = zone.as_ref().map(|z| if !zone_name.is_empty() { zone_name.clone() } else { r.areas.get(&journey::area_key(poe2, &z.0)).map(|a| a.n.clone()).unwrap_or_else(|| journey::map_name(&z.0)) });
    // Trials: the route's "Complete the trial" steps, done once the route is past them.
    let zone_id = zone.as_ref().map(|z| journey::area_key(poe2, &z.0)).unwrap_or_default();
    let mut trials = vec![];
    let mut last_zone = String::new();
    for (p, &i) in vis.iter().enumerate() {
        let st = &r.steps[i];
        for (k, v) in &st.parts { if (k == "enter" || k == "waypoint") && !v.is_empty() { last_zone = v.clone(); } }
        if st.parts.iter().any(|(k, _)| k == "trial") {
            let lab = if st.act <= 3 { "Normal" } else if st.act <= 7 { "Cruel" } else { "Merciless" };
            trials.push(Trial { zone: r.areas.get(&last_zone).map(|a| a.n.clone()).unwrap_or_default(), lab: lab.into(), done: p < pos, here: last_zone == zone_id && p >= pos });
        }
    }
    let act_now = vis.get(pos).map(|&i| r.steps[i].act).unwrap_or(1);
    // PoE 1 only: PoE 2's penalties differ (and change between patches), so it shows none rather than a wrong one.
    let res_penalty = if poe2 { 0 } else if pos + 1 >= vis.len() { -60 } else if act_now >= 6 { -30 } else { 0 };
    LevelView {
        trials, res_penalty, poe2: active_game(&app.state::<AppState>()) == Game::Poe2,
        pos, total: vis.len(), current: pos - from, steps,
        zone: zone_label, zone_level, char_level, class: (!class.is_empty()).then_some(class),
        xp_keep: match (char_level, zone_level) { (Some(c), Some(z)) => Some(journey::xp_multiplier(c, z)), _ => None },
        gem_search, gems_left_out: left, options: opts, manual, credit: r.credit.clone(),
    }
}

/// The arrows: one step back or forward.
#[tauri::command]
pub fn level_move(app: AppHandle, by: i64) -> LevelView {
    let r = journey::route_for(is_poe2(&app));
    let key = char_key();
    with_store(&app, |s| {
        let n = journey::steps_for(r, &s.route).len() as i64;
        let p = *s.level_pos.get(&key).unwrap_or(&0) as i64;
        s.level_pos.insert(key.clone(), (p + by).clamp(0, (n - 1).max(0)) as usize);
    });
    save_store(&app);
    let v = level_view(app.clone());
    emit_all(&app);
    v
}

/// Jump to the first step of an act (the act picker), or to a step number.
#[tauri::command]
pub fn level_jump_act(app: AppHandle, act: u32) -> LevelView {
    let r = journey::route_for(is_poe2(&app));
    let key = char_key();
    with_store(&app, |s| {
        let vis = journey::steps_for(r, &s.route);
        if let Some(p) = vis.iter().position(|&i| r.steps[i].act >= act) { s.level_pos.insert(key.clone(), p); }
    });
    save_store(&app);
    emit_all(&app);
    level_view(app)
}

#[tauri::command]
pub fn level_options(app: AppHandle, options: RouteOptions, manual: bool) -> LevelView {
    // Keep your place: find the same step in the new list.
    let r = journey::route_for(is_poe2(&app));
    let key = char_key();
    with_store(&app, |s| {
        let old = journey::steps_for(r, &s.route);
        let at = old.get(*s.level_pos.get(&key).unwrap_or(&0)).copied().unwrap_or(0);
        s.route = options;
        s.level_manual = manual;
        let new = journey::steps_for(r, &s.route);
        let p = new.iter().position(|&i| i >= at).unwrap_or(0);
        s.level_pos.insert(key.clone(), p);
    });
    save_store(&app);
    emit_all(&app);
    level_view(app)
}

// ---------- map tracker, loot, stopwatch ----------

#[derive(Serialize, Clone)]
pub struct TrackerView {
    pub current: Option<MapRun>,
    pub inside: bool,
    /// Seconds inside the current map so far (standing still while paused).
    pub current_inside: i64,
    /// The timer is stopped: the Escape menu or the skill tree is open.
    pub paused: bool,
    pub recent: Vec<MapRun>,
    /// Today's runs (last 12 hours) and all runs.
    pub today: journey::MapStats,
    pub all: journey::MapStats,
    /// Your character on pathofexile.com (name, level), when the Wealth page has your account.
    pub char_name: Option<String>,
    pub char_level: Option<u32>,
    /// Stopwatch in milliseconds; running?
    pub watch_ms: i64,
    pub watch_running: bool,
    pub laps: Vec<i64>,
    pub now: i64,
    pub session: Option<SessionSummary>,
    pub pause: PauseInfo,
    pub loot_prefs: LootPrefs,
    /// Why loot isn't showing ("" when it is).
    pub loot_note: String,
    /// Your bags are being read after leaving a map: its loot is on the way.
    pub loot_reading: bool,
    /// Monsters left in the map, when the game says.
    pub left: Option<LeftView>,
    /// The tracker stays up over an open stash or inventory.
    pub over_panels: bool,
    /// The divine's price (chaos), to show totals in divines.
    pub divine: Option<f64>,
    /// Logged in to pathofexile.com (account + POESESSID saved).
    pub has_account: bool,
}

/// Monsters left: the number, "more than" it, and whether the game said so just now.
#[derive(Serialize, Clone, PartialEq)]
pub struct LeftView { pub n: u32, pub more: bool, pub fresh: bool }

/// The menu check, for the Play tools page.
#[derive(Serialize, Clone)]
pub struct PauseInfo {
    pub on: bool,
    /// The bottom bar is set up (needed); the Escape menu is set up too (optional).
    pub ready: bool,
    pub esc_ready: bool,
    pub note: String,
}

/// End of session card: what this play session added up to.
#[derive(Serialize, Clone)]
pub struct SessionSummary {
    pub start: i64,
    /// Seconds from the first to the latest game event.
    pub played: i64,
    pub maps: usize,
    pub map_time: i64,
    pub per_hour: f64,
    pub deaths: u32,
    pub levels: u32,
    /// Sum of "about made" over the session's maps (when an earnings session ran).
    pub made: Option<f64>,
    /// The map that made the most, or else the quickest.
    pub best: Option<MapRun>,
    /// Loot over the session's maps (the current one too): everything added up, chaos.
    pub loot: Vec<LootLine>,
    pub loot_value: f64,
    /// Maps that have loot counted (for "a map" averages).
    pub loot_maps: usize,
    /// Seconds from the first counted map's start to the end of the last (or now, while a map is going): what
    /// "divines per hour" divides by.
    pub loot_secs: i64,
}

#[derive(Serialize, Clone)]
pub struct PlayView { pub tracker: TrackerView }

#[tauri::command]
pub fn play_view(app: AppHandle) -> PlayView { PlayView { tracker: tracker_view(&app) } }

fn left_view(l: &Live) -> Option<LeftView> { l.left.map(|(m, at)| LeftView { n: m.n, more: m.more, fresh: unix() - at < 10 }) }

fn tracker_view(app: &AppHandle) -> TrackerView {
    let now = unix();
    let over_panels = with_store(app, |s| s.over_panels);
    let (current, inside, paused, current_inside, recent, today, all, all_runs, prefs, menu, pause_off) = with_store(app, |s| {
        let h = &s.maps.history;
        (s.maps.current.clone(), s.maps.inside, s.maps.paused_since.is_some(), s.maps.current_inside(now),
            h.iter().rev().take(8).cloned().collect::<Vec<_>>(),
            journey::map_stats(h, now - 12 * 3600), journey::map_stats(h, 0), h.iter().rev().take(200).rev().cloned().collect::<Vec<_>>(),
            s.loot.clone(), s.menu.clone(), s.pause_off)
    });
    let divine = currency_rates(app).get("divine").copied();
    let has_account = account(app).is_some();
    with_live(|l| {
        let watch_ms = l.watch_acc + l.watch_since.map(|t| unix_ms() - t).unwrap_or(0);
        let session = (l.sess_start > 0).then(|| {
            let runs: Vec<MapRun> = all_runs.iter().filter(|r| r.start >= l.sess_start).cloned().collect();
            let st = journey::map_stats(&runs, 0);
            let made: Vec<f64> = runs.iter().filter_map(|r| r.made).collect();
            let best = runs.iter().filter(|r| r.made.is_some()).max_by(|a, b| a.made.unwrap_or(0.0).total_cmp(&b.made.unwrap_or(0.0)))
                .or_else(|| runs.iter().filter(|r| r.in_map > 60).min_by_key(|r| r.in_map)).cloned();
            let mut sess_loot: Vec<LootLine> = vec![];
            let mut loot_maps = 0;
            let looted: Vec<&MapRun> = runs.iter().chain(current.iter().filter(|c| c.start >= l.sess_start)).filter(|r| !r.loot.is_empty()).collect();
            for r in &looted { loot_maps += 1; loot::merge(&mut sess_loot, r.loot.clone()); }
            let loot_secs = match (looted.first(), looted.last()) {
                (Some(a), Some(b)) => {
                    let still_going = current.as_ref().is_some_and(|c| c.start == b.start && c.area == b.area);
                    (if still_going { now } else { b.left.max(b.start + b.in_map) }) - a.start
                }
                _ => 0,
            }.max(0);
            SessionSummary {
                start: l.sess_start, played: (l.sess_last - l.sess_start).max(now - l.sess_start).max(0), maps: runs.len(),
                map_time: runs.iter().map(|r| r.in_map).sum(), per_hour: st.per_hour, deaths: l.sess_deaths, levels: l.sess_levels,
                made: (!made.is_empty()).then(|| made.iter().sum()), best,
                loot_value: loot::total(&sess_loot), loot: sess_loot, loot_maps, loot_secs,
            }
        });
        let pause = PauseInfo {
            on: !pause_off, ready: !menu.hud.is_empty(), esc_ready: !menu.esc.is_empty(),
            note: if menu.hud.is_empty() { "Gets ready by itself in your next map".into() } else { l.pause_note.clone() },
        };
        TrackerView {
            current, inside, current_inside, paused, recent, today, all,
            char_name: l.site_char.as_ref().map(|c| c.0.clone()), char_level: l.site_char.as_ref().map(|c| c.1),
            watch_ms, watch_running: l.watch_since.is_some(), laps: l.laps.clone(), now, session,
            pause, loot_prefs: prefs, loot_note: l.loot_note.clone(), loot_reading: l.loot_reading > 0, left: if inside { left_view(l) } else { None }, over_panels, divine, has_account,
        }
    })
}

/// Done with this map (it otherwise ends when you open the next one).
#[tauri::command]
pub fn maps_done(app: AppHandle) -> PlayView {
    let (was_inside, key) = with_store(&app, |s| (s.maps.inside, s.maps.current.as_ref().map(run_key)));
    with_store(&app, |s| { s.maps.finish(unix()); });
    // Pressed inside the map: what's in your bags now counts as its loot (as of your last zone change).
    if let (true, Some(k)) = (was_inside, key) { queue_loot(&app, LootJob::Leave(k, unix())); }
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

/// All your runs (the Play tools page's table).
#[tauri::command]
pub fn maps_history(app: AppHandle) -> Vec<MapRun> { with_store(&app, |s| s.maps.history.clone()) }

#[tauri::command]
pub fn maps_clear(app: AppHandle) -> PlayView {
    with_store(&app, |s| { s.maps.history.clear(); s.maps.current = None; s.maps.inside = false; s.maps.paused_since = None; });
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

#[tauri::command]
pub fn watch_action(app: AppHandle, action: String) -> PlayView {
    with_live(|l| match action.as_str() {
        "start" => { if l.watch_since.is_none() { l.watch_since = Some(unix_ms()); } }
        "pause" => { if let Some(t) = l.watch_since.take() { l.watch_acc += unix_ms() - t; } }
        "lap" => { let ms = l.watch_acc + l.watch_since.map(|t| unix_ms() - t).unwrap_or(0); l.laps.insert(0, ms); l.laps.truncate(10); }
        _ => { l.watch_since = None; l.watch_acc = 0; l.laps.clear(); }
    });
    keep_live(&app);
    emit_all(&app);
    play_view(app)
}

/// Starts the tracker's background helpers: your level from pathofexile.com, the menu check, and the loot reader.
pub fn start_watchers(app: AppHandle) {
    load_kept(&app);
    start_panel_watch(app.clone());
    start_level_watch(app.clone());
    start_pause_watch(app.clone());
    start_loot_worker(app);
}

fn set_left(app: &AppHandle, m: Option<journey::MonstersLeft>) {
    let before = with_live(|l| { let b = left_view(l); l.left = m.map(|m| (m, unix())); b });
    send_left(app, before);
}
/// Tells the tracker when what it shows changed.
fn send_left(app: &AppHandle, before: Option<LeftView>) {
    let now = with_live(|l| left_view(l));
    if now != before { if let Some(w) = app.get_webview_window("tracker") { let _ = w.emit("left-update", now); } }
}

/// The tracker is put away because the stash or inventory is open under it; and he asked for it back meanwhile.
static PANEL_HID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static PANEL_KEEP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Put away under a panel (it still counts as open).
pub fn under_panel() -> bool { PANEL_HID.load(Ordering::Relaxed) }

/// Shows or hides the tracker without touching which window has the keyboard.
#[cfg(windows)]
fn raw_show(w: &tauri::WebviewWindow, on: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE};
    if let Ok(h) = w.hwnd() { unsafe { let _ = ShowWindow(HWND(h.0 as _), if on { SW_SHOWNOACTIVATE } else { SW_HIDE }); } }
}
#[cfg(not(windows))]
fn raw_show(_w: &tauri::WebviewWindow, _on: bool) {}

/// Back from under a panel (closed, switched off, or he pressed its key).
fn panel_back(app: &AppHandle) -> bool {
    if !PANEL_HID.swap(false, Ordering::Relaxed) { return false; }
    if let Some(w) = app.get_webview_window("tracker") { raw_show(&w, true); }
    true
}

/// The map tracker gets out of the way while the stash or inventory is open under it (reads the panel's name off the
/// screen), and reads how many monsters are left from the map info in the game's top right corner.
fn start_panel_watch(app: AppHandle) {
    let mut closed = 0;
    let (mut atlas, mut ritual, mut looks) = (false, false, 0u32);
    let mut info_at = std::time::Instant::now();
    let mut settle = journey::LeftSettle::default();
    let mut area = String::new();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(350));
        let Some(w) = app.get_webview_window("tracker") else { continue };
        let hid = under_panel();
        if !hid && !w.is_visible().unwrap_or(false) { continue; }
        if !game_in_front() {
            // Another program in front: back where it was. One of ours: leave things as they are.
            if !ours_in_front() { panel_back(&app); }
            continue;
        }
        let Some(rect) = game_client_rect() else { continue };
        let read = |b: (i32, i32, i32, i32)| screen::read_lines_in(b.0, b.1, b.2, b.3);
        let mut open = false;
        if !with_store(&app, |s| s.over_panels) {
            if let (Ok(p), Ok(size)) = (w.outer_position(), w.outer_size()) {
                let pw = journey::panel_width(rect.3);
                let on = |a: i32, b: i32| p.x < b && p.x + size.width as i32 > a;
                for (left, a, b) in [(true, rect.0, rect.0 + pw), (false, rect.0 + rect.2 - pw, rect.0 + rect.2)] {
                    if !open && on(a, b) { 
                        open = read(journey::panel_title_box(rect, left)).is_ok_and(|l| journey::panel_open(&l, left));
                        // Not read: the search row under it. The tracker may sit right on the name, or an effect behind it
                        // spoil it (in the hideout always looked for, in a map now and then).
                        let mid = rect.0 + pw / 2;
                        let covers = p.y < rect.1 + rect.3 * 75 / 1000 && p.x < mid + pw / 7 && p.x + size.width as i32 > mid - pw / 7;
                        if left && !open && (covers || hid || looks % 4 == 0 || !with_store(&app, |s| s.maps.inside)) {
                            open = read(journey::stash_search_box(rect)).is_ok_and(|l| journey::panel_open(&l, true));
                        }
                    }
                }
            }
        }
        // The Atlas / map device screen fills the screen: out of the way wherever the tracker is. (In a map, looked for less.)
        looks = looks.wrapping_add(1);
        if with_store(&app, |s| s.over_panels) { atlas = false; }
        else if looks % 4 == 0 || !with_store(&app, |s| s.maps.inside) {
            atlas = read(journey::atlas_title_box(rect)).is_ok_and(|l| journey::atlas_open(&l));
        }
        // The Ritual window (only in maps): item boxes open towards either side of it, so the tracker goes wherever it is.
        if with_store(&app, |s| s.over_panels || !s.maps.inside) { ritual = false; }
        else if ritual || looks % 2 == 0 {
            let [top, low] = journey::ritual_boxes(rect);
            let now = read(top).is_ok_and(|l| journey::ritual_open(&l, &[], false));
            ritual = now || (ritual && read(low).is_ok_and(|l| journey::ritual_open(&[], &l, true)));
        }
        let open = open || atlas || ritual;
        if open {
            closed = 0;
            if !hid && !PANEL_KEEP.load(Ordering::Relaxed) {
                PANEL_HID.store(true, Ordering::Relaxed);
                raw_show(&w, false);
            }
        } else {
            // Two looks in a row, so one misread doesn't make it flicker.
            closed += 1;
            if closed >= 2 { PANEL_KEEP.store(false, Ordering::Relaxed); panel_back(&app); }
        }
        // Monsters left, every 1.2 s inside a map.
        if info_at.elapsed() < Duration::from_millis(1200) { continue; }
        info_at = std::time::Instant::now();
        if !with_store(&app, |s| s.maps.inside) { continue; }
        let said = read(journey::info_box(rect)).ok().and_then(|l| l.iter().find_map(|x| journey::monsters_left(x)));
        let here = with_store(&app, |s| s.maps.current.as_ref().map(|c| format!("{}{}{}", c.area, c.seed, c.last_enter))).unwrap_or_default();
        if here != area { area = here; settle.reset(); }
        match said.map(|m| settle.feed(m)) {
            Some(Some(m)) => set_left(&app, Some(m)),
            Some(None) => {}
            None => {
                // Not on screen now: the last number stays, marked as not fresh.
                let went = with_live(|l| l.left.is_some_and(|(_, at)| { let age = unix() - at; (10..14).contains(&age) }));
                if went { let v = with_live(|l| left_view(l)); let _ = w.emit("left-update", v); }
            }
        }
    });
}

#[tauri::command]
pub fn tracker_over_panels(app: AppHandle, on: bool) -> PlayView {
    with_store(&app, |s| s.over_panels = on);
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

/// Your account name and POESESSID from the Wealth page (None when either is missing), and PoE 2 or not.
fn account(app: &AppHandle) -> Option<(String, String, bool)> {
    let st = app.state::<AppState>();
    let s = st.store.lock().unwrap().settings.clone();
    let (acc, sess) = (s.poe_account.trim().to_string(), s.poesessid.trim().to_string());
    (!acc.is_empty() && !sess.is_empty()).then(|| (acc, sess, active_game(&st) == Game::Poe2))
}

/// Your level from pathofexile.com every 10 minutes while the tracker is open (the loot reader updates it too).
fn start_level_watch(app: AppHandle) {
    thread::spawn(move || {
        let mut last_ask = 0i64;
        loop {
            thread::sleep(Duration::from_secs(10));
            let open = app.get_webview_window("tracker").map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false);
            if !open || unix() - last_ask < 600 { continue; }
            last_ask = unix();
            let Some((acc, sess, poe2)) = account(&app) else { continue };
            if let Ok(v) = trade_api::characters(&acc, &sess, poe2) {
                if let Some(c) = pick_character(&app, &v) {
                    with_live(|l| l.site_char = Some((c["name"].as_str().unwrap_or("").to_string(), c["level"].as_u64().unwrap_or(0) as u32)));
                    emit_all(&app);
                }
            }
        }
    });
}

/// The character the log says you're playing, else the one you played last.
fn pick_character(app: &AppHandle, list: &serde_json::Value) -> Option<serde_json::Value> {
    let want = with_live(|l| l.character.as_ref().map(|c| c.0.clone())).or_else(|| with_store(app, |s| s.maps.character.clone()));
    let list = list.as_array()?;
    list.iter().find(|c| want.as_deref().is_some_and(|w| c["name"].as_str() == Some(w)))
        .or_else(|| list.iter().max_by_key(|c| c["lastLoginTime"].as_i64().unwrap_or(0)))
        .cloned()
}

// ---------- the menu check (timer stops in the Escape menu and the skill tree) ----------

/// Once a second while you're inside a map: the clock stops when you alt-tab away from the game (clicking one of our
/// own windows doesn't count), and when a few pixels of the game's bottom bar (and the middle, if the Escape menu is
/// set up) look different from normal: the Escape menu darkens them, the skill tree covers them.
/// The normal look is learnt by itself (23 Nov 2026: he never pressed Set up, so the clock never stopped): a few
/// seconds into a map, once the bar has looked the same for 3 seconds in a row with the game in front.
fn start_pause_watch(app: AppHandle) {
    let mut learn: Vec<Vec<[u8; 3]>> = vec![];
    let mut away_streak = 0u32;
    // The bar a second ago: a menu holds still, play doesn't (MenuLook::covered_still).
    let mut prev: Vec<[u8; 3]> = vec![];
    // The Escape menu, read off the screen ("GAME PAUSED"): paused by it now, and when it was last looked for.
    let mut menu_up = false;
    let mut looked = 0i64;
    // Paused when the editor last closed: the first look at the game in front lets the clock go on.
    let mut first = true;
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(1));
        let (on, look, inside, in_for) = with_store(&app, |s| {
            let in_for = s.maps.current.as_ref().filter(|_| s.maps.inside).map(|c| unix() - c.last_enter).unwrap_or(0);
            (!s.pause_off, s.menu.clone(), s.maps.inside, in_for)
        });
        let set = |paused: bool, at: i64| {
            if with_store(&app, |s| s.maps.set_paused(paused, at)) { save_store(&app); emit_all(&app); }
        };
        if !on || !inside {
            // Switched off mid-pause: let the clock go on.
            set(false, unix());
            with_live(|l| l.steady.reset());
            learn.clear();
            prev.clear();
            away_streak = 0;
            menu_up = false;
            continue;
        }
        // Alt-tabbed to another program: stop the clock (after 2 seconds, so a quick look doesn't count).
        // One of our own windows in front (the tracker, the price window): leave the clock as it is.
        if !game_in_front() {
            learn.clear();
            prev.clear();
            if ours_in_front() { away_streak = 0; continue; }
            away_streak += 1;
            if away_streak == 2 { set(true, unix() - 1); with_live(|l| l.steady.reset()); }
            continue;
        }
        if away_streak >= 2 || std::mem::take(&mut first) {
            // Back in the game: look for the Escape menu straight away. If it was up when he left, the clock stays stopped
            // until that look says it's gone (24 Nov: he came back to an open menu and the clock ran on).
            if !menu_up { set(false, unix()); }
            looked = 0;
        }
        away_streak = 0;
        let Some(rect) = game_client_rect() else { continue };
        let Ok(hud) = screen::sample_points(&journey::hud_points(rect)) else { continue };
        // The Escape menu says so itself: look every second while it's up or the screen looks dimmed or still, else every 2.
        let known = !look.hud.is_empty() && (rect.2, rect.3) == look.size;
        let likely = menu_up || (known && journey::brightness(&look.hud, &hud) < 0.85) || (!prev.is_empty() && journey::changed_share(&prev, &hud) < 0.03);
        if unix() - looked >= if likely { 1 } else { 2 } {
            looked = unix();
            let (x, y, w, h) = journey::pause_box(rect);
            if let Ok(lines) = screen::read_lines_in(x, y, w, h) {
                let up = journey::is_pause_menu(&lines);
                // Up and the clock running (whatever set it going): stop it.
                if up { set(true, unix() - 1); }
                if up != menu_up {
                    menu_up = up;
                    set(up, unix() - 1);
                    with_live(|l| l.steady.reset());
                    prev = hud.clone();
                    learn.clear();
                }
            }
        }
        // While it's up nothing else decides (and its dimmed bar isn't learnt as the normal look).
        if menu_up { prev = hud; continue; }
        // No normal look yet, or the game changed size: learn it now.
        if look.hud.is_empty() || (rect.2, rect.3) != look.size {
            prev = hud.clone();
            if in_for < 4 { continue; }
            learn.push(hud);
            if learn.len() > 3 { learn.remove(0); }
            let steady = learn.len() == 3 && learn.windows(2).all(|w| journey::changed_share(&w[0], &w[1]) < 0.1);
            if steady {
                let bar = learn.pop().unwrap_or_default();
                with_store(&app, |s| {
                    if s.menu.size != (rect.2, rect.3) { s.menu.esc.clear(); }
                    s.menu.hud = bar;
                    s.menu.size = (rect.2, rect.3);
                });
                learn.clear();
                with_live(|l| { l.pause_note.clear(); l.steady.reset(); });
                save_store(&app);
                emit_all(&app);
            }
            continue;
        }
        let centre = if look.esc.is_empty() { vec![] } else { screen::sample_points(&journey::centre_points(rect)).unwrap_or_default() };
        let covered = look.covered_still(&hud, &prev, &centre);
        prev = hud.clone();
        if let Some(state) = with_live(|l| { l.pause_note.clear(); l.steady.feed(covered) }) {
            // Two readings a second apart agreed: it changed about a second ago.
            set(state, unix() - 1);
        }
    });
}

/// "Set up" (bottom bar, with nothing open) or "Escape menu" (with it open): waits 5 seconds so you can click into the
/// game (and press Escape), then remembers how it looks.
#[tauri::command]
pub async fn pause_setup(app: AppHandle, kind: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        thread::sleep(Duration::from_secs(5));
        let rect = game_client_rect().ok_or("Can't find the game window")?;
        if !game_in_front() { return Err("Path of Exile wasn't in front: click into the game within 5 seconds".to_string()); }
        let msg = if kind == "esc" {
            let centre = screen::sample_points(&journey::centre_points(rect))?;
            let hud = screen::sample_points(&journey::hud_points(rect))?;
            let already = with_store(&app, |s| {
                let seen = s.menu.covered(&hud, &[]);
                s.menu.esc = centre;
                seen
            });
            if already { "Escape menu saved (the bottom bar check already spots it too)".into() } else { "Escape menu saved".into() }
        } else {
            let hud = screen::sample_points(&journey::hud_points(rect))?;
            with_store(&app, |s| {
                // A new size: the old Escape picture no longer lines up.
                if s.menu.size != (rect.2, rect.3) { s.menu.esc.clear(); }
                s.menu.hud = hud;
                s.menu.size = (rect.2, rect.3);
            });
            "Saved. Now open the skill tree in a map: the tracker should say Paused".into()
        };
        with_live(|l| { l.pause_note.clear(); l.steady.reset(); });
        save_store(&app);
        emit_all(&app);
        Ok(msg)
    }).await.map_err(err)?
}

#[tauri::command]
pub fn pause_prefs(app: AppHandle, on: bool, forget: bool) -> PlayView {
    with_store(&app, |s| { s.pause_off = !on; if forget { s.menu = MenuLook::default(); } });
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

// ---------- loot per map ----------

enum LootJob {
    /// You went into the map (at this time): remember your bags.
    Enter(RunKey, i64),
    /// You came out (or pressed Done inside): what's new in your bags is the map's loot.
    Leave(RunKey, i64),
}

static LOOT_TX: Mutex<Option<std::sync::mpsc::Sender<LootJob>>> = Mutex::new(None);

fn queue_loot(app: &AppHandle, job: LootJob) {
    if !with_store(app, |s| s.loot.on) { return; }
    if account(app).is_none() {
        let (LootJob::Enter(k, _) | LootJob::Leave(k, _)) = &job;
        loot_missed(app, k);
        with_live(|l| l.loot_note = "Loot per map needs you logged in: press \"Log in with pathofexile.com\" in Settings → Map tracker".into());
        return;
    }
    let leave = matches!(job, LootJob::Leave(..));
    if let Some(tx) = LOOT_TX.lock().unwrap().as_ref() {
        if tx.send(job).is_ok() && leave { with_live(|l| l.loot_reading += 1); }
    }
}

/// This map's loot couldn't be read: say so in the history instead of "0c", and leave it out of the averages.
fn loot_missed(app: &AppHandle, key: &RunKey) {
    with_store(app, |s| {
        let run = match s.maps.current.as_mut() {
            Some(c) if run_key(c) == *key => Some(c),
            _ => s.maps.history.iter_mut().rev().take(5).find(|r| run_key(r) == *key),
        };
        if let Some(r) = run { r.loot_missed = true; }
    });
    save_store(app);
}

/// Reads your bags from pathofexile.com, one job at a time in order.
fn start_loot_worker(app: AppHandle) {
    let (tx, rx) = std::sync::mpsc::channel::<LootJob>();
    *LOOT_TX.lock().unwrap() = Some(tx);
    thread::spawn(move || {
        // Whatever way a read ends, the tracker stops saying it's reading.
        let done = |job: &LootJob| if matches!(job, LootJob::Leave(..)) { with_live(|l| l.loot_reading = l.loot_reading.saturating_sub(1)); };
        for job in rx {
            let (key, at) = match &job { LootJob::Enter(k, t) | LootJob::Leave(k, t) => (k.clone(), *t) };
            let bags = match read_bags(&app, at) {
                Ok(b) => b,
                Err(e) => { done(&job); loot_missed(&app, &key); with_live(|l| l.loot_note = format!("Couldn't read your bags: {e}")); emit_all(&app); continue; }
            };
            done(&job);
            match job {
                LootJob::Enter(..) => with_live(|l| { l.loot_base = Some((key, bags)); l.loot_note.clear(); }),
                LootJob::Leave(..) => {
                    let base = with_live(|l| match &l.loot_base { Some((k, b)) if *k == key => { let b = b.clone(); l.loot_base = None; Some(b) } _ => None });
                    let Some(before) = base else {
                        loot_missed(&app, &key);
                        with_live(|l| l.loot_note = "That visit's loot wasn't counted: your bags weren't read as you went in".into());
                        emit_all(&app);
                        continue;
                    };
                    let new = loot::gained(&before, &bags);
                    let divine = currency_rates(&app).get("divine").copied().unwrap_or(0.0);
                    let Some(lines) = wealth_app::with_lookup(&app, |look| loot::lines(&new, look, divine)) else {
                        loot_missed(&app, &key);
                        with_live(|l| l.loot_note = "Prices haven't loaded yet: loot not counted for that visit".into());
                        emit_all(&app);
                        continue;
                    };
                    with_store(&app, |s| {
                        let run = match s.maps.current.as_mut() {
                            Some(c) if run_key(c) == key => Some(c),
                            _ => s.maps.history.iter_mut().rev().take(5).find(|r| run_key(r) == key),
                        };
                        if let Some(r) = run {
                            loot::merge(&mut r.loot, lines);
                            r.loot.truncate(60);
                        }
                    });
                    with_live(|l| l.loot_note.clear());
                    save_store(&app);
                }
            }
            emit_all(&app);
        }
    });
}

/// Your bags as the site saved them at your zone change at `at`. The site saves your character when you change
/// zone; waits a moment and checks it has (its lastLoginTime), trying a few times.
fn read_bags(app: &AppHandle, at: i64) -> Result<Vec<serde_json::Value>, String> {
    let (acc, sess, poe2) = account(app).ok_or("no account")?;
    let name = match with_live(|l| l.character.as_ref().map(|c| c.0.clone())).or_else(|| with_store(app, |s| s.maps.character.clone())) {
        Some(n) => n,
        None => {
            let v = trade_api::characters(&acc, &sess, poe2)?;
            let c = pick_character(app, &v).ok_or("no characters on this account")?;
            let n = c["name"].as_str().unwrap_or("").to_string();
            with_store(app, |s| s.maps.character = Some(n.clone()));
            n
        }
    };
    let wait = (at + 4 - unix()).clamp(0, 10);
    thread::sleep(Duration::from_secs(wait as u64));
    let mut last = Err("not read".to_string());
    for attempt in 0..4 {
        if attempt > 0 { thread::sleep(Duration::from_secs(6)); }
        match trade_api::character_items(&acc, &sess, &name, poe2) {
            Ok(v) => {
                let c = &v["character"];
                with_live(|l| l.site_char = Some((c["name"].as_str().unwrap_or(&name).to_string(), c["level"].as_u64().unwrap_or(0) as u32)));
                let saved = c["lastLoginTime"].as_i64().unwrap_or(0);
                let bags = loot::bag_items(&v["items"]);
                // Saved at (or after) this zone change, allowing for clocks a little apart.
                if saved >= at - 15 || attempt == 3 { return Ok(bags); }
                last = Ok(bags);
            }
            Err(e) => last = Err(e),
        }
    }
    last
}

#[tauri::command]
pub fn loot_prefs_save(app: AppHandle, prefs: LootPrefs) -> PlayView {
    with_store(&app, |s| s.loot = prefs);
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

// ---------- search strings ----------

#[derive(Serialize, Clone)]
pub struct SearchView {
    pub saved: Vec<SearchString>,
    /// Made for you: your build's gems at a vendor, your map regex.
    pub made: Vec<SearchString>,
}

#[tauri::command]
pub fn searches_get(app: AppHandle) -> SearchView {
    let mut made = vec![];
    let (gems, _) = build_gems(&app);
    if !gems.is_empty() {
        let (s, left) = journey::gem_search(&gems, &journey::route().gems);
        made.push(SearchString { name: format!("My build's gems (vendor){}", if left.is_empty() { String::new() } else { format!(", {} didn't fit", left.len()) }), text: s });
    }
    if let Ok(r) = map_regex_text(&app) { if !r.trim().is_empty() { made.push(SearchString { name: "My map regex (Map mods page)".into(), text: r }); } }
    SearchView { saved: with_store(&app, |s| s.searches.clone()), made }
}

#[tauri::command]
pub fn searches_save(app: AppHandle, list: Vec<SearchString>) -> SearchView {
    with_store(&app, |s| s.searches = list.into_iter().filter(|x| !x.text.trim().is_empty()).collect());
    save_store(&app);
    searches_get(app)
}

/// Types a search into the game's stash or vendor search box (Ctrl+F, then the text). One click, one search.
#[tauri::command]
pub async fn search_type(app: AppHandle, text: String) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("searches") { let _ = w.hide(); }
    tauri::async_runtime::spawn_blocking(move || {
        focus_game();
        type_in_search(&app, &text, 1500)
    }).await.map_err(err)?
}

/// Ctrl+F (the game's "highlight items" shortcut), clear, then paste the text. Pasted, not typed: typed letters
/// would set off skill and panel keys if no search box is open, while Ctrl+V there does nothing. Your clipboard is
/// put back afterwards. Only ever sends keys to the game.
fn type_in_search(app: &AppHandle, text: &str, wait_for_focus_ms: u64) -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut waited = 0;
    while !game_in_front() {
        if waited >= wait_for_focus_ms { return Err("Path of Exile isn't the window in front, so nothing was typed".into()); }
        thread::sleep(Duration::from_millis(50));
        waited += 50;
    }
    thread::sleep(Duration::from_millis(60));
    let before = app.clipboard().read_text().unwrap_or_default();
    app.clipboard().write_text(text.to_string()).map_err(err)?;
    let mut e = Enigo::new(&Settings::default()).map_err(err)?;
    let ctrl = |e: &mut Enigo, c: char| -> enigo::InputResult<()> {
        e.key(Key::Control, Direction::Press)?;
        e.key(Key::Unicode(c), Direction::Click)?;
        e.key(Key::Control, Direction::Release)
    };
    let r = (|| -> enigo::InputResult<()> {
        ctrl(&mut e, 'f')?;
        thread::sleep(Duration::from_millis(80));
        ctrl(&mut e, 'a')?;
        ctrl(&mut e, 'v')
    })();
    thread::sleep(Duration::from_millis(150));
    let _ = app.clipboard().write_text(before);
    r.map_err(err)
}

/// The Wealth page's "Highlight in stash": lights up these items in the game's stash. Returns the search and how
/// many of the names fit in it (the game allows 250 characters).
#[tauri::command]
pub async fn stash_highlight(app: AppHandle, names: Vec<String>, type_it: bool) -> Result<(String, usize), String> {
    let (s, n) = journey::names_search(&names);
    if s.is_empty() { return Err("Nothing to highlight".into()); }
    if type_it {
        let (t, a) = (s.clone(), app.clone());
        tauri::async_runtime::spawn_blocking(move || { focus_game(); type_in_search(&a, &t, 2500) }).await.map_err(err)??;
    } else {
        app.clipboard().write_text(s.clone()).map_err(err)?;
    }
    Ok((s, n))
}

fn map_regex_text(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let (mods, _) = map_mods(&state);
    if mods.is_empty() { return Err("no map data".into()); }
    let s = state.store.lock().unwrap().settings.clone();
    let bad = s.map_tool.bad.clone().unwrap_or_else(|| filter_core::mapregex::default_bad(&mods, s.map_avoid.as_deref()));
    Ok(filter_core::mapregex::regex(&mods, &bad, &s.map_tool.wants))
}

// ---------- cheat sheets and notes ----------

fn sheets_dir(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("cheatsheets") }

#[derive(Serialize, Clone)]
pub struct Sheet { pub name: String, pub data: String }

/// Your own cheat sheet pictures (kept in the data folder's "cheatsheets"), as pictures the window can show.
#[tauri::command]
pub fn sheets_list(app: AppHandle) -> Vec<Sheet> {
    use base64::Engine;
    let mut out: Vec<Sheet> = std::fs::read_dir(sheets_dir(&app)).into_iter().flatten().flatten().filter_map(|e| {
        let p = e.path();
        let ext = p.extension()?.to_string_lossy().to_lowercase();
        let mime = match ext.as_str() { "png" => "image/png", "jpg" | "jpeg" => "image/jpeg", "webp" => "image/webp", "gif" => "image/gif", _ => return None };
        let bytes = std::fs::read(&p).ok()?;
        Some(Sheet { name: p.file_stem()?.to_string_lossy().to_string(), data: format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) })
    }).collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Copies pictures you picked into the cheat sheets folder.
#[tauri::command]
pub fn sheets_add(app: AppHandle, paths: Vec<String>) -> Result<Vec<Sheet>, String> {
    let dir = sheets_dir(&app);
    std::fs::create_dir_all(&dir).map_err(err)?;
    for p in paths {
        let src = PathBuf::from(&p);
        let Some(name) = src.file_name() else { continue };
        std::fs::copy(&src, dir.join(name)).map_err(|e| format!("Couldn't copy {p}: {e}"))?;
    }
    Ok(sheets_list(app))
}

#[tauri::command]
pub fn sheets_remove(app: AppHandle, name: String) -> Vec<Sheet> {
    if let Ok(rd) = std::fs::read_dir(sheets_dir(&app)) {
        for e in rd.flatten() {
            if e.path().file_stem().map(|s| s.to_string_lossy() == name).unwrap_or(false) { let _ = std::fs::remove_file(e.path()); }
        }
    }
    sheets_list(app)
}

fn notes_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("notes.txt") }

#[tauri::command]
pub fn notes_get(app: AppHandle) -> String { std::fs::read_to_string(notes_path(&app)).unwrap_or_default() }

#[tauri::command]
pub fn notes_save(app: AppHandle, text: String) -> Result<(), String> { std::fs::write(notes_path(&app), text).map_err(err) }

// ---------- windows ----------

/// Show or hide one of the play windows (its hotkey, or the Play tools page's buttons).
pub fn toggle(app: &AppHandle, label: &str) {
    let Some(w) = app.get_webview_window(label) else { return };
    // Put away under the stash: its key brings it back, until the stash closes.
    if label == "tracker" && under_panel() {
        PANEL_KEEP.store(true, Ordering::Relaxed);
        panel_back(app);
        return;
    }
    if w.is_visible().unwrap_or(false) { let _ = w.hide(); return; }
    let _ = w.emit("play-shown", ());
    if label == "searches" || label == "notes" {
        // These take typing/clicks: open by the mouse (searches) or where you left them, and take focus.
        if label == "searches" && !win_place::restore(app, &w) { place_near_cursor(&w, 16); }
        else { win_place::restore(app, &w); }
        let _ = w.show();
        let _ = w.set_focus();
    } else {
        win_place::show_over_game(app, &w);
    }
    emit_all(app);
}

#[tauri::command]
pub fn play_toggle(app: AppHandle, label: String) {
    if ["level", "tracker", "searches", "sheet", "notes"].contains(&label.as_str()) { toggle(&app, &label); }
}

#[tauri::command]
pub fn play_hide(app: AppHandle, label: String) {
    if label == "tracker" { panel_back(&app); }
    if let Some(w) = app.get_webview_window(&label) { let _ = w.hide(); }
    if label == "notes" || label == "searches" || label == "sheet" { focus_game(); }
}

/// Wiki key: opens the wiki page of the item under the mouse (its name for uniques, else its base).
pub fn wiki_lookup(app: AppHandle) {
    thread::spawn(move || {
        if !game_back_in_front(&app) { return; }
        let before = app.clipboard().read_text().unwrap_or_default();
        let _ = app.clipboard().write_text(String::new());
        send_ctrl_c();
        let mut text = String::new();
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(30));
            text = app.clipboard().read_text().unwrap_or_default();
            if !text.is_empty() { break; }
        }
        let _ = app.clipboard().write_text(before);
        let Some(item) = parse_item(&text) else { let _ = app.emit("hotkey-warning", "Wiki: hover an item in your inventory or stash first"); return };
        let poe2 = active_game(&app.state::<AppState>()) == Game::Poe2;
        let page = wiki_page(&item.rarity, item.name.as_deref(), &item.base_type);
        let site = if poe2 { "https://www.poe2wiki.net/wiki/" } else { "https://www.poewiki.net/wiki/" };
        let _ = open_url(format!("{site}{}", page.replace(' ', "_").replace('?', "%3F")));
    });
}

/// Which wiki page: uniques by name, everything else by base type.
pub fn wiki_page(rarity: &str, name: Option<&str>, base: &str) -> String {
    match (rarity, name) {
        ("Unique", Some(n)) if !n.is_empty() => n.trim_start_matches("Foulborn ").to_string(),
        _ => base.trim_start_matches("Superior ").to_string(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wiki_pages() {
        assert_eq!(super::wiki_page("Unique", Some("Tabula Rasa"), "Simple Robe"), "Tabula Rasa");
        assert_eq!(super::wiki_page("Rare", Some("Doom Grip"), "Titan Gauntlets"), "Titan Gauntlets");
        assert_eq!(super::wiki_page("Normal", None, "Superior Leather Belt"), "Leather Belt");
    }
}

// ---------- trades: is the price right, and where's the item ----------

/// The trades pop-up asks this for each whisper: your asking price next to what poe.ninja says it's worth.
#[tauri::command]
pub fn trade_price_check(app: AppHandle, item: String, price: String) -> Option<filter_core::whispers::PriceCheck> {
    let rates = currency_rates(&app);
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    filter_core::whispers::check_price(&item, &price, &prices.get(&game)?.items, &rates)
}

/// Where a stash cell is on screen: (x, y, size) in physical pixels, from the game's drawing area.
/// The stash grid sits at a fixed share of the game's height (like the panel width the price window docks beside):
/// at 1080 tall, the 12x12 grid starts about 16 px in and 161 px down, cells about 52.8 px. Quad tabs: 24x24, half size.
/// `left`/`top` are the whisper's 1-based cell.
pub fn stash_cell(game: (i32, i32, i32, i32), left: u32, top: u32, quad: bool) -> (i32, i32, i32) {
    let (gx, gy, _gw, gh) = game;
    let h = gh as f64;
    let cell = h * 52.8 / 1080.0 / if quad { 2.0 } else { 1.0 };
    let x0 = gx as f64 + h * 16.0 / 1080.0;
    let y0 = gy as f64 + h * 161.0 / 1080.0;
    let x = x0 + (left.saturating_sub(1)) as f64 * cell;
    let y = y0 + (top.saturating_sub(1)) as f64 * cell;
    (x.round() as i32, y.round() as i32, cell.round() as i32)
}

/// Draws a box on the stash square the buyer's item is in (for about 20 seconds; it never takes clicks).
#[tauri::command]
pub fn show_stash_spot(app: AppHandle, left: u32, top: u32, quad: bool) -> Result<(), String> {
    let game = game_client_rect().ok_or("Can't find the game window")?;
    let w = app.get_webview_window("stashbox").ok_or("no window")?;
    // A whisper past 12 can only be a quad tab.
    let quad = quad || left > 12 || top > 12;
    let (x, y, size) = stash_cell(game, left, top, quad);
    let pad = 4;
    win_place::placing();
    let _ = w.set_size(tauri::PhysicalSize::new((size + pad * 2) as u32, (size + pad * 2) as u32));
    let _ = w.set_position(PhysicalPosition::new(x - pad, y - pad));
    let _ = w.set_ignore_cursor_events(true);
    let front = game_in_front();
    let _ = w.show();
    if front { focus_game(); }
    let gen = STASH_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    let app2 = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(20));
        if STASH_GEN.load(Ordering::Relaxed) == gen { if let Some(w) = app2.get_webview_window("stashbox") { let _ = w.hide(); } }
    });
    Ok(())
}
static STASH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[tauri::command]
pub fn hide_stash_spot(app: AppHandle) {
    if let Some(w) = app.get_webview_window("stashbox") { let _ = w.hide(); }
}

#[cfg(test)]
mod stash_tests {
    #[test]
    fn cells() {
        // 1920x1080 game at the screen's corner: first cell, then the 12th across.
        assert_eq!(super::stash_cell((0, 0, 1920, 1080), 1, 1, false), (16, 161, 53));
        let (x, _, _) = super::stash_cell((0, 0, 1920, 1080), 12, 1, false);
        assert_eq!(x, 597);
        let (_, _, s) = super::stash_cell((0, 0, 2560, 1440), 1, 1, true);
        assert_eq!(s, 35);
    }
}

// ---------- price alerts ----------

#[tauri::command]
pub fn alerts_get(app: AppHandle) -> Vec<Alert> {
    check_alerts(&app);
    with_store(&app, |s| s.alerts.clone())
}

#[tauri::command]
pub fn alerts_save(app: AppHandle, list: Vec<Alert>) -> Vec<Alert> {
    with_store(&app, |s| {
        // Keep what we knew about alerts that are still there.
        let old = std::mem::take(&mut s.alerts);
        s.alerts = list.into_iter().filter(|a| !a.name.trim().is_empty() && a.value > 0.0).map(|mut a| {
            if let Some(o) = old.iter().find(|o| o.name == a.name && o.when == a.when && o.value == a.value && o.unit == a.unit) { a.last = o.last; a.fired = o.fired; }
            a
        }).collect();
    });
    save_store(&app);
    alerts_get(app)
}

/// Each price update: an alert whose price has crossed its line tells you (in the trades pop-up, once per crossing).
pub fn check_alerts(app: &AppHandle) {
    let (items, divine) = {
        let state = app.state::<AppState>();
        let game = active_game(&state);
        let prices = state.prices.lock().unwrap();
        let Some(f) = prices.get(&game) else { return };
        let div = f.items.iter().find(|i| i.name == "Divine Orb").map(|i| i.value).unwrap_or(0.0);
        (f.items.clone(), div)
    };
    let mut fired = vec![];
    with_store(app, |s| for a in s.alerts.iter_mut() {
        let Some(now) = items.iter().filter(|i| i.name.eq_ignore_ascii_case(a.name.trim())).map(|i| i.value).reduce(f64::max) else { continue };
        let line = a.value * if a.unit == "divine" && divine > 0.0 { divine } else { 1.0 };
        let crossed = if a.when == "above" { now >= line } else { now <= line };
        let was = a.last.map(|l| if a.when == "above" { l >= line } else { l <= line }).unwrap_or(false);
        if crossed && !was {
            a.fired = Some(unix());
            let shown = if a.unit == "divine" && divine > 0.0 { format!("{:.1} div", now / divine) } else { format!("{:.0}c", now) };
            fired.push(format!("{} is {} now (your alert: {} {} {})", a.name, shown, a.when, a.value, a.unit));
        }
        a.last = Some(now);
    });
    save_store(app);
    if fired.is_empty() { return; }
    if let Some(w) = app.get_webview_window("trades") {
        for f in fired { let _ = w.emit("price-alert", f); }
        if !w.is_visible().unwrap_or(false) { win_place::show_over_game(app, &w); }
    }
}

// ---------- reading league mechanic choices off the screen ----------

/// The choice reader's key: reads the text around the mouse (an Eldritch altar, an Expedition remnant, a reward screen)
/// and says which to pick, what's dangerous for you, and what the items are worth.
pub fn read_choice(app: AppHandle) {
    thread::spawn(move || {
        let lines = match screen::read_lines_near_mouse(1500, 1000) {
            Ok(l) => l,
            Err(e) => { show_choice(&app, None, Some(e)); return; }
        };
        let (avoid, game) = {
            let st = app.state::<AppState>();
            let s = st.store.lock().unwrap().settings.clone();
            (s.map_avoid.unwrap_or_default(), active_game(&st))
        };
        let file = app.state::<AppState>().prices.lock().unwrap().get(&game).cloned();
        let price = |n: &str| file.as_ref().and_then(|f| f.items.iter().filter(|i| i.name.eq_ignore_ascii_case(n) && i.cat != "SkillGem").map(|i| i.value).reduce(f64::max));
        let read = filter_core::choices::read(&lines, &avoid, &price);
        if read.lines.is_empty() { show_choice(&app, None, Some("Couldn't read any text there: hover the altar or remnant and try again".into())); return; }
        show_choice(&app, Some(read), None);
    });
}

#[derive(Serialize, Clone)]
struct ChoiceView { read: Option<filter_core::choices::ChoiceRead>, error: Option<String>, divine: Option<f64> }

fn show_choice(app: &AppHandle, read: Option<filter_core::choices::ChoiceRead>, error: Option<String>) {
    let divine = currency_rates(app).get("divine").copied();
    if let Some(w) = app.get_webview_window("choice") {
        place_near_cursor(&w, 24);
        let _ = w.emit("choice-read", ChoiceView { read, error, divine });
        let front = game_in_front();
        let _ = w.show();
        if front { focus_game(); }
    }
}

// ---------- out of the way while you map ----------

static HIDDEN_FOR_MAP: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Entering a map hides the play windows that aren't for mapping; leaving it brings back the ones it hid.
fn map_visibility(app: &AppHandle, changes: &[MapChange]) {
    if with_store(app, |s| s.keep_in_maps) { return; }
    for c in changes {
        match c {
            MapChange::Started | MapChange::Resumed => {
                let mut hidden = HIDDEN_FOR_MAP.lock().unwrap();
                for label in ["level", "searches", "sheet", "notes"] {
                    if let Some(w) = app.get_webview_window(label) {
                        if w.is_visible().unwrap_or(false) { let _ = w.hide(); if !hidden.iter().any(|h| h == label) { hidden.push(label.into()); } }
                    }
                }
            }
            MapChange::Left => {
                let labels: Vec<String> = std::mem::take(&mut *HIDDEN_FOR_MAP.lock().unwrap());
                for label in labels { if let Some(w) = app.get_webview_window(&label) { win_place::show_over_game(app, &w); } }
            }
            _ => {}
        }
    }
}

#[tauri::command]
pub fn play_prefs(app: AppHandle, keep_in_maps: bool) {
    with_store(&app, |s| s.keep_in_maps = keep_in_maps);
    save_store(&app);
}

#[tauri::command]
pub fn play_prefs_get(app: AppHandle) -> bool { with_store(&app, |s| s.keep_in_maps) }

// ---------- prices for the small lookups (oils, the exchange check) ----------

/// poe.ninja's chaos price for each of these names (oils, currencies), when it has one.
#[tauri::command]
pub fn price_names(app: AppHandle, names: Vec<String>) -> HashMap<String, f64> {
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let Some(f) = prices.get(&game) else { return HashMap::new() };
    names.into_iter().filter_map(|n| {
        if n == "Chaos Orb" { return Some((n, 1.0)); }
        let v = f.items.iter().filter(|i| i.name.eq_ignore_ascii_case(&n) && i.cat != "SkillGem").map(|i| i.value).reduce(f64::max)?;
        Some((n, v))
    }).collect()
}

// ---------- chat commands ----------

/// What each chat command does (shown on the Play tools page too).
pub const CHAT_COMMANDS: &[(&str, &str)] = &[
    ("!next", "switch to your next filter and load it"),
    ("!filter <name>", "switch to the filter whose name has that word (e.g. !filter strict)"),
    ("!reload", "load your filter again"),
    ("!done", "finish the map timer's current map"),
    ("!start / !stop / !lap / !reset", "the stopwatch"),
    ("!level / !tracker / !sheet / !notes", "show or hide that window"),
];

fn run_chat_command(app: &AppHandle, cmd: &str) {
    let (word, rest) = cmd.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((cmd, ""));
    let app2 = app.clone();
    // The game is still closing the chat box: wait a moment before typing our own command.
    let later = move |f: Box<dyn FnOnce(&AppHandle) + Send>| { thread::spawn(move || { thread::sleep(Duration::from_millis(400)); f(&app2); }); };
    match word {
        "next" => later(Box::new(|a| cycle_filter(a))),
        "reload" => later(Box::new(|a| { let _ = reload_in_game(a, 1000); })),
        "filter" if !rest.is_empty() => {
            let want = rest.to_lowercase();
            let state = app.state::<AppState>();
            let picked = {
                let mut store = state.store.lock().unwrap();
                let game = store.active.as_ref().and_then(|id| store.profiles.get(id)).map(|p| p.game);
                let hit = store.profiles.values().filter(|p| Some(p.game) == game || game.is_none())
                    .find(|p| p.name.to_lowercase().contains(&want)).map(|p| p.id.clone());
                if let Some(id) = &hit { store.active = Some(id.clone()); let _ = store.save(&state.store_path); }
                hit
            };
            match picked {
                Some(id) => { let _ = app.emit("active-changed", id); later(Box::new(|a| { let _ = reload_in_game(a, 1000); })); }
                None => { let _ = app.emit("hotkey-warning", format!("No filter called “{rest}”")); }
            }
        }
        "done" => { let _ = maps_done(app.clone()); }
        "start" | "stop" | "pause" | "lap" | "reset" => {
            let a = if word == "stop" { "pause" } else { word };
            let _ = watch_action(app.clone(), a.into());
        }
        "level" | "tracker" | "sheet" | "notes" | "searches" => toggle(app, word),
        _ => {}
    }
}

#[tauri::command]
pub fn chat_prefs(app: AppHandle, on: bool) -> bool {
    with_store(&app, |s| s.chat_off = !on);
    save_store(&app);
    on
}

#[tauri::command]
pub fn chat_get(app: AppHandle) -> (bool, Vec<(String, String)>) {
    (!with_store(&app, |s| s.chat_off), CHAT_COMMANDS.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect())
}

// ---------- race splits ----------

#[derive(Serialize, Clone)]
pub struct RaceView {
    pub name: String,
    pub class: String,
    /// Seconds since level 2.
    pub elapsed: i64,
    pub level: u32,
    /// (act, your time, your best time on another character).
    pub acts: Vec<(u32, i64, Option<i64>)>,
    /// Your best time to the act you're in now (to beat), and to the next act.
    pub best_next: Option<(u32, i64)>,
    pub others: usize,
}

/// Your current character's race against your earlier ones.
#[tauri::command]
pub fn race_get(app: AppHandle) -> Option<RaceView> {
    let me = with_live(|l| l.character.clone())?;
    with_store(&app, |s| {
        let r = s.races.chars.get(&me.0)?;
        let (best_acts, _) = s.races.best_except(&me.0);
        let best = |a: u32| best_acts.iter().find(|x| x.0 == a).map(|x| x.1);
        let reached = r.acts.iter().map(|a| a.0).max().unwrap_or(1);
        Some(RaceView {
            name: me.0.clone(), class: r.class.clone(), elapsed: unix() - r.start, level: me.2,
            acts: r.acts.iter().map(|&(a, t)| (a, t, best(a))).collect(),
            best_next: best(reached + 1).map(|t| (reached + 1, t)),
            others: s.races.chars.len().saturating_sub(1),
        })
    })
}
