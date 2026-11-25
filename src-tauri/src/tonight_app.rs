//! "Tonight" on Home: up to three things to do in the time you have. Logic in `filter_core::tonight`; everything it
//! uses is already on this PC (the last stash scan, your maps, hourly prices).

use super::*;
use filter_core::tonight::{self, Card};

fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

#[tauri::command(async)]
pub fn tonight_get(app: AppHandle, minutes: u32) -> Vec<Card> {
    let st = app.state::<AppState>();
    let poe1 = active_game(&st) == Game::Poe1;
    let now = unix();
    let (items, snap) = wealth_app::last_scan(&app);
    let scanned = snap.as_ref().map(|s| s.time as i64);
    // A scan from another league says nothing about this one.
    let same = snap.as_ref().is_some_and(|s| s.league == trade_league(&app).1);
    let farm = if poe1 { farming_app::farm_now(app.clone()) } else { None };
    let keep: Vec<String> = farm.as_ref().map(|f| f.thread_names()).unwrap_or_default();
    let mut out = vec![];
    // Prices a week out of date would send you selling the wrong things.
    if let Some(t) = scanned.filter(|t| same && now - t < 7 * 86400) {
        let min = st.store.lock().unwrap().settings.wealth_min.max(5.0);
        out.extend(tonight::sell(&filter_core::wealth::missed(&items, min), minutes, &keep, t));
    }
    if let Some(f) = &farm { out.push(f.run_card(&app, minutes)); }
    else if poe1 {
        if let Ok(found) = farm_app::farm_find(app.clone(), Default::default()) {
            // Your own best tree first (enough maps to say so), else the finder's top card.
            let own = found.own.iter().find(|o| !o.own.few && !o.keys.is_empty()).map(|o| (o.title.clone(), o.keys.clone()));
            if let Some((title, keys)) = own.or_else(|| found.farms.first().map(|f| (f.title.clone(), f.keys.clone()))) { out.push(tonight::pick_farm(&title, &keys)); }
        }
    }
    let reprice = if same { filter_core::wealth::reprice(&items, &currency_rates(&app)) } else { vec![] };
    let patterns = filter_core::deaths::patterns(&play_app::maps_with_current(&app));
    let avoided = |m: &str| deaths_app::avoided(&app, m);
    out.extend(tonight::fix(&reprice, &patterns, &avoided, scanned.filter(|_| same), now, play_app::account(&app).is_some()));
    out
}
