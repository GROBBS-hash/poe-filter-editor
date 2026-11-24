//! The farm finder's command (logic in `filter_core::farm`): poe.ninja prices the app already has, and your map runs.

use super::*;
use filter_core::farm::{FarmAsk, Finder};
use filter_core::temple::{self, Temples};

#[tauri::command(async)]
pub fn farm_find(app: AppHandle, ask: FarmAsk) -> Result<Finder, String> {
    let prices = app.state::<AppState>().prices.lock().unwrap().get(&Game::Poe1).cloned()
        .ok_or("Prices haven't loaded yet: try again in a minute")?;
    let runs: Vec<(String, i64)> = play_app::maps_history(app.clone()).into_iter().map(|r| (r.name, r.in_map)).collect();
    let temples = temples(&app);
    let divine = prices.items.iter().find(|i| i.name == "Divine Orb").map(|i| i.value).unwrap_or(0.0);
    refresh_temples(&app, &prices.league, divine);
    let trees = atlas_app::own_trees(&app);
    Ok(filter_core::farm::find(&prices, &temples, &runs, &trees, &ask))
}

fn temples_path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("temples.json") }

/// The last prices read from the trade site, else the ones shipped with the app.
fn temples(app: &AppHandle) -> Temples {
    std::fs::read_to_string(temples_path(app)).ok().and_then(|t| serde_json::from_str::<Temples>(&t).ok())
        .filter(|t| t.checked > Temples::shipped().checked && !t.rooms.is_empty())
        .unwrap_or_else(|| Temples::shipped().clone())
}

/// Temple prices older than this are read again when the page is opened.
const TEMPLES_OLD: Duration = Duration::from_secs(12 * 3600);

/// Reads the rooms worth something again, one every 20 s and only while your own price checks have room to spare.
fn now_unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

fn refresh_temples(app: &AppHandle, league: &str, divine: f64) {
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let fresh = std::fs::metadata(temples_path(app)).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a < TEMPLES_OLD);
    if fresh || BUSY.swap(true, Ordering::Relaxed) { return; }
    let (app, league) = (app.clone(), league.to_string());
    thread::spawn(move || {
        let mut t = temples(&app);
        let rates: std::collections::HashMap<&str, f64> = [("chaos", 1.0), ("divine", divine)].into();
        let want: Vec<String> = t.worth().into_iter().take(10).map(|r| r.label.clone()).collect();
        let mut read = 0;
        for label in want {
            let mut waited = 0;
            while !trade_api::has_room("search", 3) && waited < 600 { thread::sleep(Duration::from_secs(15)); waited += 15; }
            let Some(room) = t.rooms.iter_mut().find(|r| r.label == label) else { continue };
            let Ok((id, ids, total)) = trade_api::search(false, &league, &temple::query(&room.stats)) else { continue };
            let Ok(v) = trade_api::fetch(false, &ids, &id) else { continue };
            let prices: Vec<f64> = v["result"].as_array().into_iter().flatten().filter_map(|x| {
                let p = &x["listing"]["price"];
                Some(p["amount"].as_f64()? * rates.get(p["currency"].as_str()?).copied()?)
            }).filter(|c| *c > 0.0).collect();
            room.listed = total as u32;
            if let Some(c) = temple::typical(&prices) { room.typical = c; }
            read += 1;
            thread::sleep(Duration::from_secs(20));
        }
        if read > 0 {
            t.checked = temple::iso(now_unix());
            if let Ok(s) = serde_json::to_string(&t) { let _ = std::fs::write(temples_path(&app), s); }
            let _ = app.emit("temples-updated", ());
        }
        BUSY.store(false, Ordering::Relaxed);
    });
}
