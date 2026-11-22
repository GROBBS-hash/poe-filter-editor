//! Play helpers over the game (22 Nov 2026, from the "what do players want" list):
//! - Levelling overlay ("level" window): the route step you're on, following the zones you enter (exile-leveling's
//!   route), quest gems from your PoB build, zone level vs yours, and a vendor search for your build's gems.
//! - Map tracker ("tracker" window): time in each map, maps per hour, deaths, mods from a Ctrl+D map check just
//!   before, roughly what an earnings session made per map; XP per hour (from your character on pathofexile.com
//!   when the Wealth page has your POESESSID); a stopwatch.
//! - Search strings ("searches" window): saved stash/vendor searches typed into the game's search box with one click.
//! - Cheat sheets ("sheet" window): the Syndicate table and your own pictures.
//! - Notes ("notes" window), and a wiki key (opens the wiki page of the item under the mouse).
//!
//! Everything here reads the game's own log (Client.txt), copied item text, or pathofexile.com: nothing reads the
//! game's memory.

use super::*;
use filter_core::journey::{self, MapChange, MapRun, MapTracker, PlayEvent, RouteOptions};
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

#[derive(Default)]
struct Live {
    /// (area id, area level, when).
    zone: Option<(String, u32, i64)>,
    zone_name: String,
    /// (name, class, level) from the log.
    character: Option<(String, String, u32)>,
    /// (unix time, total experience) samples for XP per hour.
    xp: Vec<(i64, u64)>,
    xp_note: String,
    xp_char: Option<(String, u32, u64)>,
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
    sess_xp_start: Option<u64>,
}

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
    emit_all(app);
}

/// Every new log line goes through here (from the log watcher in lib.rs).
pub fn on_line(app: &AppHandle, line: &str) {
    let Some(mut ev) = journey::parse_line(line) else { return };
    // The log's clock is your PC's local time; lines arrive as they're written, so stamp them with the real time
    // (the timers compare against it).
    match &mut ev {
        PlayEvent::Area { time, .. } | PlayEvent::Entered { time, .. } | PlayEvent::Level { time, .. } | PlayEvent::Slain { time, .. } => *time = unix(),
    }
    let changes = with_store(app, |s| s.maps.on_event(&ev));
    map_visibility(app, &changes);
    let mut save = false;
    for c in &changes {
        match c {
            MapChange::Started => {
                // A Ctrl+D map check in the last 10 minutes: that's the map you just opened.
                let check = with_live(|l| l.map_check.take()).filter(|(t, ..)| unix() - t < 600);
                let value = session_value(app);
                with_store(app, |s| if let Some(cur) = s.maps.current.as_mut() {
                    if let Some((_, _, mods)) = &check { cur.mods = mods.clone(); }
                    cur.value_at_start = value;
                });
                save = true;
            }
            MapChange::Finished(_) => {
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
    // The session: a new one after 30 quiet minutes.
    with_live(|l| {
        let now = unix();
        if l.sess_start == 0 || now - l.sess_last > 30 * 60 {
            l.sess_start = now; l.sess_deaths = 0; l.sess_levels = 0; l.sess_xp_start = l.xp.last().map(|x| x.1);
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
            with_live(|l| { l.zone = Some((id.clone(), *level, *time)); l.zone_name.clear(); });
            follow_route(app, id);
        }
        PlayEvent::Entered { name, .. } => with_live(|l| l.zone_name = name.clone()),
        PlayEvent::Level { name, class, level, .. } => with_live(|l| l.character = Some((name.clone(), class.clone(), *level))),
        _ => {}
    }
    if save { save_store(app); }
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

fn follow_route(app: &AppHandle, area: &str) {
    let r = journey::route();
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
    let r = journey::route();
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
    let zone_label = zone.as_ref().map(|z| if !zone_name.is_empty() { zone_name.clone() } else { r.areas.get(&z.0).map(|a| a.n.clone()).unwrap_or_else(|| journey::map_name(&z.0)) });
    // Trials: the route's "Complete the trial" steps, done once the route is past them.
    let zone_id = zone.as_ref().map(|z| z.0.clone()).unwrap_or_default();
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
    let res_penalty = if pos + 1 >= vis.len() { -60 } else if act_now >= 6 { -30 } else { 0 };
    LevelView {
        trials, res_penalty,
        pos, total: vis.len(), current: pos - from, steps,
        zone: zone_label, zone_level, char_level, class: (!class.is_empty()).then_some(class),
        xp_keep: match (char_level, zone_level) { (Some(c), Some(z)) => Some(journey::xp_multiplier(c, z)), _ => None },
        gem_search, gems_left_out: left, options: opts, manual, credit: r.credit.clone(),
    }
}

/// The arrows: one step back or forward.
#[tauri::command]
pub fn level_move(app: AppHandle, by: i64) -> LevelView {
    let r = journey::route();
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
    let r = journey::route();
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
    let r = journey::route();
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

// ---------- map tracker, XP, stopwatch ----------

#[derive(Serialize, Clone)]
pub struct TrackerView {
    pub current: Option<MapRun>,
    pub inside: bool,
    /// Seconds inside the current map so far.
    pub current_inside: i64,
    pub recent: Vec<MapRun>,
    /// Today's runs (last 12 hours) and all runs.
    pub today: journey::MapStats,
    pub all: journey::MapStats,
    /// XP per hour, experience left to the next level, how far through it (0..1), minutes to level.
    pub xp_per_hour: Option<f64>,
    pub xp_left: Option<u64>,
    pub xp_frac: Option<f64>,
    pub xp_minutes: Option<f64>,
    pub xp_char: Option<String>,
    pub xp_level: Option<u32>,
    pub xp_note: String,
    /// Stopwatch in milliseconds; running?
    pub watch_ms: i64,
    pub watch_running: bool,
    pub laps: Vec<i64>,
    pub now: i64,
    pub session: Option<SessionSummary>,
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
    pub xp_gained: Option<u64>,
    /// Sum of "about made" over the session's maps (when an earnings session ran).
    pub made: Option<f64>,
    /// The map that made the most, or else the quickest.
    pub best: Option<MapRun>,
}

#[derive(Serialize, Clone)]
pub struct PlayView { pub tracker: TrackerView }

#[tauri::command]
pub fn play_view(app: AppHandle) -> PlayView { PlayView { tracker: tracker_view(&app) } }

fn tracker_view(app: &AppHandle) -> TrackerView {
    let now = unix();
    let (current, inside, recent, today, all, all_runs) = with_store(app, |s| {
        let h = &s.maps.history;
        (s.maps.current.clone(), s.maps.inside, h.iter().rev().take(8).cloned().collect::<Vec<_>>(),
            journey::map_stats(h, now - 12 * 3600), journey::map_stats(h, 0), h.iter().rev().take(200).rev().cloned().collect::<Vec<_>>())
    });
    let current_inside = current.as_ref().map(|c| c.time_inside(inside, now)).unwrap_or(0);
    with_live(|l| {
        let rate = journey::xp_per_hour(&l.xp, 30 * 60);
        let last = l.xp.last().map(|x| x.1);
        let next = last.and_then(journey::to_next);
        let watch_ms = l.watch_acc + l.watch_since.map(|t| unix_ms() - t).unwrap_or(0);
        let session = (l.sess_start > 0).then(|| {
            let runs: Vec<MapRun> = all_runs.iter().filter(|r| r.start >= l.sess_start).cloned().collect();
            let st = journey::map_stats(&runs, 0);
            let made: Vec<f64> = runs.iter().filter_map(|r| r.made).collect();
            let best = runs.iter().filter(|r| r.made.is_some()).max_by(|a, b| a.made.unwrap_or(0.0).total_cmp(&b.made.unwrap_or(0.0)))
                .or_else(|| runs.iter().filter(|r| r.in_map > 60).min_by_key(|r| r.in_map)).cloned();
            SessionSummary {
                start: l.sess_start, played: (l.sess_last - l.sess_start).max(now - l.sess_start).max(0), maps: runs.len(),
                map_time: runs.iter().map(|r| r.in_map).sum(), per_hour: st.per_hour, deaths: l.sess_deaths, levels: l.sess_levels,
                xp_gained: match (l.sess_xp_start, l.xp.last()) { (Some(a), Some(b)) if b.1 >= a => Some(b.1 - a), _ => None },
                made: (!made.is_empty()).then(|| made.iter().sum()), best,
            }
        });
        TrackerView {
            current, inside, current_inside, recent, today, all,
            xp_per_hour: rate, xp_left: next.map(|n| n.0), xp_frac: next.map(|n| n.1),
            xp_minutes: match (rate, next) { (Some(r), Some((left, _))) if r > 0.0 => Some(left as f64 / r * 60.0), _ => None },
            xp_char: l.xp_char.as_ref().map(|c| c.0.clone()), xp_level: l.xp_char.as_ref().map(|c| c.1),
            xp_note: l.xp_note.clone(),
            watch_ms, watch_running: l.watch_since.is_some(), laps: l.laps.clone(), now, session,
        }
    })
}

/// Done with this map (it otherwise ends when you open the next one).
#[tauri::command]
pub fn maps_done(app: AppHandle) -> PlayView {
    with_store(&app, |s| { s.maps.finish(unix()); });
    save_store(&app);
    emit_all(&app);
    play_view(app)
}

/// All your runs (the Play tools page's table).
#[tauri::command]
pub fn maps_history(app: AppHandle) -> Vec<MapRun> { with_store(&app, |s| s.maps.history.clone()) }

#[tauri::command]
pub fn maps_clear(app: AppHandle) -> PlayView {
    with_store(&app, |s| { s.maps.history.clear(); s.maps.current = None; s.maps.inside = false; });
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
    emit_all(&app);
    play_view(app)
}

/// Asks pathofexile.com for your character's experience every minute while the tracker window is open
/// (needs the account and POESESSID from the Wealth page). Without them, XP per hour shows a note instead.
pub fn start_xp_watch(app: AppHandle) {
    thread::spawn(move || {
        let mut last_ask = 0i64;
        loop {
            thread::sleep(Duration::from_secs(5));
            let open = app.get_webview_window("tracker").map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false);
            if !open || unix() - last_ask < 60 { continue; }
            last_ask = unix();
            let (acc, sess) = {
                let st = app.state::<AppState>();
                let s = st.store.lock().unwrap().settings.clone();
                (s.poe_account.trim().to_string(), s.poesessid.trim().to_string())
            };
            if acc.is_empty() || sess.is_empty() {
                with_live(|l| l.xp_note = "XP per hour needs your account and POESESSID (Wealth page)".into());
                emit_all(&app);
                continue;
            }
            let poe2 = active_game(&app.state::<AppState>()) == Game::Poe2;
            let want = with_live(|l| l.character.as_ref().map(|c| c.0.clone()));
            match trade_api::characters(&acc, &sess, poe2) {
                Ok(v) => {
                    let list = v.as_array().cloned().unwrap_or_default();
                    // The character the log says you're playing, else the last one played.
                    let pick = list.iter().find(|c| want.as_deref().is_some_and(|w| c["name"].as_str() == Some(w)))
                        .or_else(|| list.iter().find(|c| c["lastActive"].as_bool() == Some(true)))
                        .or_else(|| list.first());
                    if let Some(c) = pick {
                        let name = c["name"].as_str().unwrap_or("").to_string();
                        let xp = c["experience"].as_u64().unwrap_or(0);
                        let level = c["level"].as_u64().unwrap_or(0) as u32;
                        with_live(|l| {
                            if l.xp_char.as_ref().map(|x| &x.0) != Some(&name) { l.xp.clear(); }
                            l.xp_char = Some((name, level, xp));
                            l.xp.push((unix(), xp));
                            if l.xp.len() > 240 { l.xp.remove(0); }
                            l.xp_note = if l.xp.len() < 2 { "Measuring… (a minute or two)".into() } else { String::new() };
                        });
                    }
                }
                Err(e) => with_live(|l| l.xp_note = format!("Couldn't read your character: {e}")),
            }
            emit_all(&app);
        }
    });
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
    if let Ok(r) = map_regex_text(&app) { if !r.trim().is_empty() { made.push(SearchString { name: "My map regex (Maps page)".into(), text: r }); } }
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
