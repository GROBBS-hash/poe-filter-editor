//! "Worth picking up": what a plain base lists at (F2, Ctrl+D), and making dear ones stand out in the filter (Prices page).

use super::*;
use filter_core::bases::{self, BaseWorth, Pick};

#[derive(Serialize)]
pub struct BaseView { worth: BaseWorth, divine: Option<f64>, /// Bases from this much (chaos) already stand out in your filter.
    loud: Option<f64> }

#[tauri::command]
pub fn base_worth(app: AppHandle, item: Item) -> Option<BaseView> {
    let st = app.state::<AppState>();
    let game = active_game(&st);
    let worth = bases::worth(st.prices.lock().unwrap().get(&game)?, &item)?;
    let loud = { let s = st.store.lock().unwrap(); s.active.as_ref().and_then(|id| s.profiles.get(id)).and_then(|p| p.prices.as_ref()).and_then(|p| p.bases_min) };
    Some(BaseView { worth, divine: currency_rates(&app).get("divine").copied(), loud })
}

#[derive(Serialize)]
pub struct Picks { min: Option<f64>, influenced: bool, total: usize, list: Vec<Pick>, divine: Option<f64>, known: bool }

/// The Prices page's card: the setting, and which bases it picks out right now (dearest first).
#[tauri::command]
pub fn bases_get(app: AppHandle, id: String, min: Option<f64>, influenced: Option<bool>) -> Result<Picks, String> {
    let st = app.state::<AppState>();
    let (game, on, inf) = {
        let s = st.store.lock().unwrap();
        let p = s.profiles.get(&id).ok_or("Profile not found")?;
        (p.game, p.prices.as_ref().and_then(|x| x.bases_min), p.prices.as_ref().is_some_and(|x| x.bases_influenced))
    };
    let divine = currency_rates(&app).get("divine").copied();
    let influenced = influenced.unwrap_or(inf);
    let shown = min.or(on).or(divine).unwrap_or(100.0);
    let prices = st.prices.lock().unwrap();
    let f = prices.get(&game);
    let mut list = f.map(|f| bases::picks(f, shown, influenced)).unwrap_or_default();
    let total = list.len();
    list.truncate(60);
    Ok(Picks { min: on, influenced, total, list, divine, known: f.is_some_and(|f| !f.bases.is_empty()) })
}

/// Switches it on (from `min` chaos) or off, for this filter or the one in use.
#[tauri::command]
pub fn bases_save(app: AppHandle, id: Option<String>, min: Option<f64>, influenced: Option<bool>) -> Result<Vec<String>, String> {
    let st = app.state::<AppState>();
    let mut store = st.store.lock().unwrap();
    let id = id.or_else(|| store.active.clone()).ok_or("No filter is in use")?;
    remember(&st, &store, &id, if min.is_some() { "Dear bases stand out" } else { "Dear bases switched off" });
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    let unit = if p.game == Game::Poe1 { "chaos" } else { "exalted" };
    let s = p.prices.get_or_insert_with(|| default_settings(unit));
    s.bases_min = min.filter(|m| *m > 0.0);
    if let Some(i) = influenced { s.bases_influenced = i; }
    save_and_build(&st, &store, &id)
}
