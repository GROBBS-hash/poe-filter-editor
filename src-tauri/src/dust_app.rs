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
