//! Price search window (`pricesearch`, Alt+F by default): type an item name, see its poe.ninja price from the
//! hourly price file. No network while typing. See `filter_core::price_search`.

use crate::{active_game, focus_game, monitor_at, win_place, AppState};
use filter_core::price_search::{entries, search, Entry};
use filter_core::profiles::Game;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};

#[derive(serde::Serialize)]
pub struct PriceSearch {
    pub poe2: bool,
    pub league: String,
    /// "c" or "ex".
    pub unit: String,
    /// One Divine Orb in the file's unit (0 when unknown).
    pub per_div: f64,
    /// When the prices were fetched (ISO time), "" without a price file.
    pub updated: String,
    pub results: Vec<Entry>,
}

#[tauri::command]
pub fn price_search(state: State<AppState>, q: String) -> PriceSearch {
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let Some(file) = prices.get(&game) else {
        return PriceSearch { poe2: game == Game::Poe2, league: String::new(), unit: "c".into(), per_div: 0.0, updated: String::new(), results: vec![] };
    };
    let per_div = file.items.iter().find(|i| i.name == "Divine Orb" && !i.cat.starts_with("Unique")).map_or(0.0, |i| i.value);
    PriceSearch {
        poe2: game == Game::Poe2,
        league: file.league_name.clone().unwrap_or_else(|| file.league.clone()),
        unit: filter_core::prices::unit_short(&file.unit).into(),
        per_div,
        updated: file.updated.clone(),
        results: search(&entries(file), &q, 40),
    }
}

/// The key: shows the box in the top middle of the screen the mouse is on, ready to type; again hides it.
pub fn toggle(app: &AppHandle) {
    let Some(w) = app.get_webview_window("pricesearch") else { return };
    if w.is_visible().unwrap_or(false) { hide(app, true); return; }
    // Where he last dragged it (26 Nov: he couldn't move it); the first time, the top middle of the screen.
    if win_place::restore(app, &w) {
    } else if let (Ok(pos), Ok(size)) = (w.cursor_position(), w.outer_size()) {
        win_place::placing();
        if let Some((mx, my, mw, mh)) = monitor_at(&w, pos.x, pos.y) {
            let x = mx + (mw - size.width as i32) / 2;
            let y = my + mh / 8;
            let _ = w.set_position(PhysicalPosition::new(x, y));
        }
    }
    let _ = w.emit("pricesearch-shown", ());
    let _ = w.show();
    let _ = w.set_focus();
}

fn hide(app: &AppHandle, back_to_game: bool) {
    if let Some(w) = app.get_webview_window("pricesearch") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
            if back_to_game { focus_game(); }
        }
    }
}

/// Esc (back to the game), or a link opened (the browser stays in front).
#[tauri::command]
pub fn price_search_hide(app: AppHandle, back_to_game: bool) { hide(&app, back_to_game); }
