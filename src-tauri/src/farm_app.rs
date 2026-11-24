//! The farm finder's command (logic in `filter_core::farm`): poe.ninja prices the app already has, and your map runs.

use super::*;
use filter_core::farm::{FarmAsk, Finder};

#[tauri::command(async)]
pub fn farm_find(app: AppHandle, ask: FarmAsk) -> Result<Finder, String> {
    let prices = app.state::<AppState>().prices.lock().unwrap().get(&Game::Poe1).cloned()
        .ok_or("Prices haven't loaded yet: try again in a minute")?;
    let runs: Vec<(String, i64)> = play_app::maps_history(app).into_iter().map(|r| (r.name, r.in_map)).collect();
    Ok(filter_core::farm::find(&prices, &runs, &ask))
}
