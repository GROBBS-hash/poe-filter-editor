//! "Sell in bulk" page (Money → Sell in bulk): what you have lots of, from the last Stash worth scan, priced at
//! poe.ninja minus a discount, with the price to type in game and a Discord post. Logic in `filter_core::bulk`.
//! It never lists or trades: the player types the prices in game.

use super::*;
use filter_core::bulk::{self, BulkItem, Note};
use serde::Deserialize;

#[derive(Serialize)]
pub struct BulkView {
    items: Vec<BulkItem>,
    groups: Vec<&'static str>,
    divine: Option<f64>,
    league: String,
    /// When the stash was last read (Unix seconds), if ever.
    scanned: Option<u64>,
    /// Which scan profile that was ("" = the whole stash).
    profile: String,
    has_sessid: bool,
}

/// What you have to sell in bulk, from the last stash scan.
#[tauri::command]
pub fn bulk_get(app: AppHandle) -> BulkView {
    let (items, snap) = wealth_app::last_scan(&app);
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let has_sessid = !state.store.lock().unwrap().settings.poesessid.trim().is_empty();
    let (_, league) = trade_league(&app);
    let found = {
        let prices = state.prices.lock().unwrap();
        prices.get(&game).map(|f| {
            let cats: std::collections::HashMap<&str, &str> = f.items.iter().map(|i| (i.name.as_str(), i.cat.as_str())).collect();
            bulk::bulk_items(&items, &|n: &str| cats.get(n).map(|c| c.to_string()))
        }).unwrap_or_default()
    };
    let groups = bulk::GROUPS.iter().copied().filter(|g| found.iter().any(|i| i.group == *g)).collect();
    BulkView {
        items: found, groups, divine: currency_rates(&app).get("divine").copied(), league,
        scanned: snap.as_ref().map(|s| s.time), profile: snap.map(|s| s.profile).unwrap_or_default(), has_sessid,
    }
}

#[derive(Deserialize)]
pub struct Pick { name: String, count: u32, /// Market price for one, in chaos (poe.ninja or the live rate).
    each: f64 }

#[derive(Serialize)]
pub struct BasketRow { name: String, count: u32, note: Note, total: f64 }

#[derive(Serialize)]
pub struct Basket { rows: Vec<BasketRow>, total: f64, total_text: String, post: String }

/// The basket's price notes, total and Discord post for these picks at poe.ninja minus `discount` %.
#[tauri::command]
pub fn bulk_basket(app: AppHandle, picks: Vec<Pick>, discount: f64, title: String, character: String) -> Basket {
    let divine = currency_rates(&app).get("divine").copied();
    let (_, league) = trade_league(&app);
    let rows: Vec<BasketRow> = picks.into_iter().filter(|p| p.count > 0 && p.each > 0.0).map(|p| {
        let note = bulk::price_note(bulk::price_each(p.each, discount), p.count, divine);
        let total = note.each * p.count as f64;
        BasketRow { name: p.name, count: p.count, note, total }
    }).collect();
    let total: f64 = rows.iter().map(|r| r.total).sum();
    let lines: Vec<bulk::Line> = rows.iter().map(|r| bulk::Line { name: &r.name, count: r.count, total: r.total }).collect();
    let title = if title.trim().is_empty() { "Bulk".to_string() } else { title.trim().to_string() };
    let post = if rows.is_empty() { String::new() } else { bulk::post(&title, &league, &lines, divine, discount, &character) };
    Basket { total_text: bulk::chaos_or_div(total, divine), rows, total, post }
}

#[derive(Serialize)]
pub struct Live { chaos: f64, offers: usize, url: String, /// Close enough to poe.ninja to use (bulk::fair_live).
    fair: bool }

/// What the trade site's bulk exchange says one of these sells for now (chaos each, stock-weighted middle).
/// Asks in divines for items worth a divine or more, else chaos (and divines when nobody sells for chaos).
#[tauri::command(async)]
pub fn bulk_live(app: AppHandle, name: String, each: f64) -> Result<Option<Live>, String> {
    let state = app.state::<AppState>();
    let (poe2, league) = trade_league(&app);
    let Some(id) = trade_api::statics(&state.data_dir, poe2)?.id(&name) else { return Ok(None) };
    let divine = currency_rates(&app).get("divine").copied().unwrap_or(0.0);
    let pays: &[&str] = if divine > 0.0 && each >= divine { &["divine"] } else { &["chaos", "divine"] };
    for pay in pays {
        let (qid, v) = trade_api::exchange(poe2, &league, &filter_core::trade::exchange_query(pay, &id, None))?;
        let offers = filter_core::trade::offers(&v);
        let rate = if *pay == "divine" { divine } else { 1.0 };
        let rows: Vec<(f64, u32)> = offers.iter().take(20).map(|o| (o.each * rate, o.stock)).collect();
        if let Some(chaos) = bulk::going_rate(&rows).filter(|c| *c > 0.0) {
            return Ok(Some(Live { chaos, offers: rows.len(), url: trade_api::exchange_url(poe2, &league, &qid), fair: bulk::fair_live(chaos, each) }));
        }
    }
    Ok(None)
}
