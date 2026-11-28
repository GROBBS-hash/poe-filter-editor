//! Prices page: uniques by how much Thaumaturgic Dust they give (PoE 1). Saved with the page's other price settings.

use super::*;
use filter_core::dust::{self, DustPick};
use filter_core::prices::Tier;

#[derive(Serialize)]
pub struct DustPicks { total: usize, list: Vec<DustPick>, /// When the dust list was made.
    updated: String }

/// Which bases these tiers pick out right now, most dust first (for this filter's NeverSink file).
#[tauri::command]
pub fn dust_picks(app: AppHandle, id: String, tiers: Vec<Tier>) -> Result<DustPicks, String> {
    let st = app.state::<AppState>();
    let segs = {
        let s = st.store.lock().unwrap();
        let p = s.profiles.get(&id).ok_or("Profile not found")?;
        std::fs::read_to_string(filter_dir(p.game).join(&p.base_file)).map(|b| parse_filter(&b)).unwrap_or_default()
    };
    let mut list = dust::picks(&tiers, &segs);
    let total = list.iter().filter(|p| !p.neversink).count();
    list.truncate(80);
    Ok(DustPicks { total, list, updated: dust::updated().to_string() })
}

/// What a unique gives when disenchanted, for Ctrl+D and F2 (28 Nov 2026: the dust list was on the Prices page only).
#[derive(Serialize)]
pub struct DustOf {
    /// At this copy's item level, quality and influences (corrupted implicits not counted: they only add).
    dust: u64,
    /// At item level 84 with no quality: the most a fresh one gives.
    at84: u64,
    ilvl: Option<u32>,
}

#[tauri::command]
pub fn unique_dust(app: AppHandle, item: filter_core::item::Item) -> Option<DustOf> {
    if active_game(&app.state::<AppState>()) != Game::Poe1 || !item.rarity.eq_ignore_ascii_case("unique") { return None; }
    let name = item.name.as_deref()?;
    let name = name.strip_prefix("Foulborn ").unwrap_or(name);
    let d = dust::of_unique(name, item.item_level, item.quality.unwrap_or(0), item.influences.len() as u32)?;
    Some(DustOf { dust: d.dust, at84: d.at84, ilvl: item.item_level })
}
