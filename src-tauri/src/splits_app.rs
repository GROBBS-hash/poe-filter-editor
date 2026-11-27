//! Campaign split timer window ("splits", Alt+T, 27 Nov): each act's time against your best run, like LiveSplit.
//!
//! Follows the PoE 1 Client.txt on its own, from where it last stopped (splits.json `read_to`), so the first start reads
//! the whole log (your past campaigns, well under a second for 150 MB) and time played with the editor shut is caught up.
//! The logic is `filter_core::splits` (tested).

use crate::{play_app, win_place, AppState};
use filter_core::splits::{self, Splits};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    /// The log these runs came from (another log: start again).
    log: String,
    read_to: u64,
    splits: Splits,
}

static STATE: Mutex<Option<Saved>> = Mutex::new(None);

fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("splits.json") }

fn with<T>(app: &AppHandle, f: impl FnOnce(&mut Saved) -> T) -> T {
    let mut g = STATE.lock().unwrap();
    let s = g.get_or_insert_with(|| filter_core::keep::load(&path(app)).unwrap_or_default());
    f(s)
}

fn save(app: &AppHandle) {
    if let Some(t) = with(app, |s| serde_json::to_string(s).ok()) { let _ = filter_core::keep::save_text(&path(app), &t); }
}

/// The log's clock now (it writes local time as if it were UTC).
fn log_now() -> i64 { unix() + play_app::utc_offset() }
fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

/// Starts following the log (once, at startup).
pub fn start(app: AppHandle) {
    thread::spawn(move || {
        let mut log: Option<PathBuf> = None;
        let mut last_look = Instant::now() - Duration::from_secs(60);
        let mut dirty = false;
        let mut last_save = Instant::now();
        let mut caught_up = false;
        loop {
            thread::sleep(Duration::from_millis(if caught_up { 1000 } else { 10 }));
            if log.is_none() {
                if last_look.elapsed() < Duration::from_secs(30) { continue; }
                last_look = Instant::now();
                log = filter_core::area::find_log(false, &[]);
                let Some(p) = &log else { continue };
                let p = p.to_string_lossy().to_string();
                with(&app, |s| if s.log != p { *s = Saved { log: p, ..Default::default() }; });
            }
            let Some(p) = log.clone() else { continue };
            let Ok(len) = std::fs::metadata(&p).map(|m| m.len()) else { log = None; continue };
            let from = with(&app, |s| {
                if len < s.read_to { s.read_to = 0; s.splits.last_line = 0; } // the log was cleared
                s.read_to
            });
            if len == from {
                caught_up = true;
                if dirty && last_save.elapsed() > Duration::from_secs(10) { save(&app); dirty = false; last_save = Instant::now(); }
                continue;
            }
            let Ok(mut f) = std::fs::File::open(&p) else { continue };
            if f.seek(SeekFrom::Start(from)).is_err() { continue; }
            let mut buf = Vec::new();
            if f.take(16 * 1024 * 1024).read_to_end(&mut buf).is_err() { continue; }
            // Only whole lines; a half-written last line waits for next time.
            let Some(end) = buf.iter().rposition(|b| *b == b'\n') else { continue };
            let text = String::from_utf8_lossy(&buf[..=end]);
            let (changed, new_run) = with(&app, |s| {
                let runs = s.splits.runs.len();
                let changed = text.lines().fold(false, |c, l| s.splits.feed(l) | c);
                s.read_to = from + end as u64 + 1;
                (changed, s.splits.runs.len() > runs)
            });
            dirty = true;
            if changed { save(&app); dirty = false; last_save = Instant::now(); }
            if caught_up {
                // A new character: the timer comes up by itself (without taking the keyboard from the game).
                if new_run {
                    if let Some(w) = app.get_webview_window("splits") {
                        if !w.is_visible().unwrap_or(false) { win_place::show_over_game(&app, &w); }
                    }
                }
                if let Some(w) = app.get_webview_window("splits") { let _ = w.emit("splits-update", ()); }
            }
        }
    });
}

#[derive(Serialize)]
pub struct Row {
    pub label: String,
    /// Your time at the end of this act (time played since level 2).
    pub mine: Option<i64>,
    /// Your best run's time there.
    pub best: Option<i64>,
    /// How long this act took you, and your fastest ever.
    pub seg: Option<i64>,
    pub best_seg: Option<i64>,
    /// Faster than you've ever done this act.
    pub gold: bool,
    pub level: Option<u32>,
}

#[derive(Serialize)]
pub struct Past {
    pub idx: usize,
    pub name: String,
    pub class: String,
    /// Unix seconds, roughly.
    pub started: i64,
    pub acts: usize,
    pub time: Option<i64>,
    pub best: bool,
    pub hidden: bool,
}

#[derive(Serialize)]
pub struct View {
    pub name: String,
    pub class: String,
    pub level: u32,
    /// Time played so far; while `live` the clock is running (the window counts on between updates).
    pub played: i64,
    pub live: bool,
    pub done: bool,
    pub rows: Vec<Row>,
    pub best_name: Option<String>,
    pub best_total: Option<i64>,
    /// Your fastest act times added up: the best run you could have.
    pub sum_best: Option<i64>,
    pub past: Vec<Past>,
}

#[tauri::command]
pub fn splits_view(app: AppHandle) -> Option<View> {
    with(&app, |s| {
        let sp = &s.splits;
        let i = sp.current()?;
        let r = &sp.runs[i];
        let best = sp.best_run(Some(i));
        let cmp = sp.compare(Some(i));
        let segs = sp.best_segments(Some(i));
        let rows = (0..splits::ACTS).map(|a| {
            let seg = r.segment(a);
            Row {
                label: splits::act_label(a), mine: r.splits.get(a).copied(), best: cmp.get(a).copied().flatten(),
                seg, best_seg: segs[a], gold: matches!((seg, segs[a]), (Some(x), Some(b)) if x < b), level: r.levels.get(a).copied(),
            }
        }).collect();
        let elapsed = sp.elapsed(i, log_now());
        let off = play_app::utc_offset();
        let mut past: Vec<Past> = sp.runs.iter().enumerate().filter(|(_, r)| !r.splits.is_empty()).map(|(k, r)| Past {
            idx: k, name: r.name.clone(), class: r.class.clone(), started: r.started - off, acts: r.splits.len(),
            time: r.splits.last().copied(), best: Some(k) == sp.best_run(None), hidden: r.hidden,
        }).collect();
        past.reverse();
        Some(View {
            name: r.name.clone(), class: r.class.clone(), level: r.level, played: elapsed,
            live: elapsed > r.played, done: r.done(), rows,
            best_name: best.map(|b| sp.runs[b].name.clone()), best_total: best.map(|b| sp.runs[b].splits[splits::ACTS - 1]),
            sum_best: segs.iter().copied().sum::<Option<i64>>(), past,
        })
    })
}

/// Leave a run out of the bests (a test character, a mix-up), or put it back.
#[tauri::command]
pub fn splits_hide(app: AppHandle, idx: usize, hidden: bool) {
    with(&app, |s| if let Some(r) = s.splits.runs.get_mut(idx) { r.hidden = hidden; });
    save(&app);
    if let Some(w) = app.get_webview_window("splits") { let _ = w.emit("splits-update", ()); }
}
