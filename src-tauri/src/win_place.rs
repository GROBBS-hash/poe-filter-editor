//! Our windows over the game remember where you put them (and how big you made them),
//! all the same way: saved in windows.json in the data folder half a second after a move
//! or resize, put back (and pulled fully onto a screen) each time they're shown.
//!
//! - "build" (F6) and "atlas" (F7): place and size; they have a resize grip.
//! - "trades" and "earnings": place only (they size themselves to what they show).

use super::*;
use std::collections::HashMap;

/// Windows whose size is remembered too.
const SIZED: [&str; 2] = ["build", "atlas"];
/// Windows whose place is remembered.
const PLACED: [&str; 4] = ["build", "atlas", "trades", "earnings"];

#[derive(Serialize, serde::Deserialize, Clone, Copy, PartialEq)]
struct Place { x: i32, y: i32, w: u32, h: u32 }

static PLACES: Mutex<Option<HashMap<String, Place>>> = Mutex::new(None);
/// Bumped on every move/resize; the save waits until it settles (no file write per pixel dragged).
static GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// While we place a window ourselves, that doesn't count as you moving it.
static PLACING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("windows.json") }

fn with_places<T>(app: &AppHandle, f: impl FnOnce(&mut HashMap<String, Place>) -> T) -> T {
    let mut g = PLACES.lock().unwrap();
    let map = g.get_or_insert_with(|| {
        let dir = app.state::<AppState>().data_dir.clone();
        let mut m: HashMap<String, Place> = std::fs::read_to_string(path(app)).ok()
            .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        // The atlas window kept its own file before 21 Nov.
        if !m.contains_key("atlas") {
            if let Some(p) = std::fs::read_to_string(dir.join("atlas_window.json")).ok().and_then(|t| serde_json::from_str(&t).ok()) {
                m.insert("atlas".into(), p);
            }
        }
        m
    });
    f(map)
}

/// Moved or resized by hand: remember it.
pub fn on_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if let tauri::WindowEvent::Focused(focused) = event {
        if window.label() == "main" { beat_as(window.app_handle(), Some(*focused)); }
        return;
    }
    if !matches!(event, tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)) { return; }
    let label = window.label().to_string();
    if !PLACED.contains(&label.as_str()) { return; }
    if PLACING.load(Ordering::Relaxed) || !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false) { return; }
    let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) else { return };
    if s.width < 60 || s.height < 40 { return; }
    let app = window.app_handle().clone();
    with_places(&app, |m| { m.insert(label, Place { x: p.x, y: p.y, w: s.width, h: s.height }); });
    let gen = GEN.fetch_add(1, Ordering::Relaxed) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(500));
        if GEN.load(Ordering::Relaxed) != gen { return; }
        let text = with_places(&app, |m| serde_json::to_string_pretty(m).ok());
        if let Some(text) = text { let _ = std::fs::write(path(&app), text); }
    });
}

/// Puts a window back where it was (and its size, for the resizable ones), fully on a screen.
/// Returns false when it has never been placed (the caller may pick a first spot).
pub fn restore(app: &AppHandle, w: &tauri::WebviewWindow) -> bool {
    let label = w.label().to_string();
    let Some(pl) = with_places(app, |m| m.get(&label).copied()) else { return false };
    let sized = SIZED.contains(&label.as_str());
    let (pw, ph) = if sized { (pl.w, pl.h) } else {
        w.outer_size().map(|s| (s.width, s.height)).unwrap_or((pl.w, pl.h))
    };
    let Some((mx, my, mw, mh)) = monitor_at(w, pl.x as f64 + pw as f64 / 2.0, pl.y as f64 + ph as f64 / 2.0) else { return false };
    let (ww, hh) = (pw.min(mw as u32), ph.min(mh as u32));
    let x = pl.x.clamp(mx, (mx + mw - ww as i32).max(mx));
    let y = pl.y.clamp(my, (my + mh - hh as i32).max(my));
    placing();
    if sized { let _ = w.set_size(tauri::PhysicalSize::new(ww, hh)); }
    let _ = w.set_position(PhysicalPosition::new(x, y));
    true
}

/// We're about to move a window ourselves: don't remember that as his choice.
pub fn placing() {
    PLACING.store(true, Ordering::Relaxed);
    thread::spawn(|| { thread::sleep(Duration::from_millis(400)); PLACING.store(false, Ordering::Relaxed); });
}

/// Shows one of our windows over the game where it was last, leaving the game in front.
pub fn show_over_game(app: &AppHandle, w: &tauri::WebviewWindow) {
    if !w.is_visible().unwrap_or(false) { restore(app, w); }
    let front = game_in_front();
    let _ = w.show();
    if front { focus_game(); }
}

/// The main window when the editor starts. Restarts (a dev rebuild, or "Restart to update") used to pop the
/// editor up in the middle of the screen while he was playing or working on something else. So: started within
/// 30 minutes of the last run (a restart), it comes back the way it was: in front if it was the window he was
/// using, quietly minimised to the taskbar (no focus) if it was open behind other things, still in the tray if
/// he'd closed it. A normal start opens it as usual.
/// A small heartbeat file ("alive.txt": time and front/shown/hidden) is how a restart is told from a start.
pub fn start_main(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let file = alive_path(app);
    let last = std::fs::read_to_string(&file).unwrap_or_default();
    let mut parts = last.split_whitespace();
    let at: u64 = parts.next().and_then(|t| t.parse().ok()).unwrap_or(0);
    let state = parts.next().unwrap_or("shown");
    // A rebuild can take minutes (it ran over 45s on 21 Nov and the editor popped up mid-game): anything within
    // half an hour of the last run counts as a restart. A start after a longer break opens as usual.
    match (unix_now().saturating_sub(at) < 30 * 60, state) {
        (false, _) | (true, "front") => { let _ = w.show(); let _ = w.set_focus(); }
        (true, "hidden") => {}
        (true, _) => show_minimised_quietly(&w),
    }
    let app = app.clone();
    thread::spawn(move || loop {
        beat(&app);
        thread::sleep(Duration::from_secs(10));
    });
}

fn alive_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("alive.txt") }
fn unix_now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

/// Writes the heartbeat now: "front" when the main window is the one he's using, "shown" when it's open behind
/// other windows or minimised, "hidden" when it's in the tray. Also called when the main window gains or loses
/// focus, and just before "Restart to update", so a restart sees how things really were.
pub fn beat(app: &AppHandle) { beat_as(app, None) }

/// A restart he asked for (tray "Restart editor", an update): the editor comes back in front afterwards.
pub fn mark_front(app: &AppHandle) {
    let _ = std::fs::write(alive_path(app), format!("{} front", unix_now()));
}

/// focused: what a focus event just said (asking the window right then can still give the old answer).
fn beat_as(app: &AppHandle, focused: Option<bool>) {
    let Some(w) = app.get_webview_window("main") else { return };
    let state = if !w.is_visible().unwrap_or(false) { "hidden" }
        else if focused.unwrap_or_else(|| w.is_focused().unwrap_or(false)) && !w.is_minimized().unwrap_or(false) { "front" }
        else { "shown" };
    let _ = std::fs::write(alive_path(app), format!("{} {state}", unix_now()));
}

#[cfg(windows)]
fn show_minimised_quietly(w: &tauri::WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWMINNOACTIVE};
    match w.hwnd() {
        Ok(h) => unsafe { let _ = ShowWindow(HWND(h.0 as _), SW_SHOWMINNOACTIVE); },
        Err(_) => { let _ = w.minimize(); let _ = w.show(); }
    }
}

#[cfg(not(windows))]
fn show_minimised_quietly(w: &tauri::WebviewWindow) { let _ = w.minimize(); let _ = w.show(); }
