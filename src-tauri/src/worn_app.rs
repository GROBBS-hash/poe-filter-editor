//! "Better than mine?" in the price window: the checked item against the gear you're wearing.
//! The gear comes along with the bags the map tracker reads anyway; otherwise it's read when you check an item (at most
//! every few minutes), or you say "I'm wearing this" when you aren't logged in. Kept in worn.json.

use super::*;
use filter_core::compare::{slots_for, Comparison};
use filter_core::worn::{from_items, Worn};

static WORN: Mutex<Option<Worn>> = Mutex::new(None);
/// When the site was last asked (unix seconds), answered or not.
static ASKED: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
/// Ask again after this long: the site only saves your gear when you change zone.
const STALE: i64 = 300;

fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }
fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("worn.json") }

fn current(app: &AppHandle) -> Worn {
    let mut w = WORN.lock().unwrap();
    if w.is_none() { *w = Some(std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()); }
    w.clone().unwrap_or_default()
}

fn keep(app: &AppHandle, worn: Worn) {
    if let Ok(t) = serde_json::to_string(&worn) {
        let (p, tmp) = (path(app), path(app).with_extension("json.tmp"));
        if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, p); }
    }
    *WORN.lock().unwrap() = Some(worn);
}

/// A character reply from the site (the map tracker reads one at each map): remember what it wears.
pub fn note(app: &AppHandle, reply: &serde_json::Value, character: &str, poe2: bool) {
    let pieces = from_items(&reply["items"]);
    if pieces.is_empty() { return; }
    let c = &reply["character"];
    ASKED.store(unix(), Ordering::Relaxed);
    keep(app, Worn { character: c["name"].as_str().unwrap_or(character).to_string(), at: c["lastLoginTime"].as_i64().unwrap_or_else(unix), poe2, by_hand: false, pieces });
}

#[derive(Serialize)]
pub struct WornView {
    /// Where this item could go ("Ring 1", "Ring 2"): empty for things you don't wear.
    slots: Vec<String>,
    character: String,
    /// When that gear was saved (unix seconds), None when nothing is known yet.
    at: Option<i64>,
    by_hand: bool,
    /// Logged in to pathofexile.com, so it can be read there.
    login: bool,
    /// Old enough to read again.
    stale: bool,
    list: Vec<Comparison>,
}

fn view(app: &AppHandle, text: &str, class: &str) -> Option<WornView> {
    let st = app.state::<AppState>();
    if !st.store.lock().unwrap().settings.price_compare { return None; }
    let poe2 = active_game(&st) == Game::Poe2;
    let slots: Vec<String> = slots_for(class).iter().map(|s| s.to_string()).collect();
    if slots.is_empty() || text.lines().any(|l| l.trim() == "Unidentified") { return None; }
    let w = current(app);
    let known = !w.pieces.is_empty() && w.poe2 == poe2;
    Some(WornView {
        slots, login: play_app::account(app).is_some(), by_hand: w.by_hand,
        stale: unix() - ASKED.load(Ordering::Relaxed) > STALE,
        character: if known { w.character.clone() } else { String::new() },
        at: known.then_some(w.at),
        list: if known { w.compare(text, class) } else { vec![] },
    })
}

/// The price window asks when it opens on an item.
#[tauri::command]
pub fn worn_compare(app: AppHandle, text: String, class: String) -> Option<WornView> { view(&app, &text, &class) }

/// Reads your character again (one or two requests), then compares. Waits its turn behind the trade searches.
#[tauri::command(async)]
pub fn worn_refresh(app: AppHandle, text: String, class: String) -> Result<Option<WornView>, String> {
    let (acc, sess, poe2) = play_app::account(&app).ok_or("Log in with pathofexile.com first (Settings → Map tracker)")?;
    ASKED.store(unix(), Ordering::Relaxed);
    let name = play_app::playing_character(&app, &acc, &sess, poe2)?;
    let v = trade_api::character_items(&acc, &sess, &name, poe2)?;
    note(&app, &v, &name, poe2);
    Ok(view(&app, &text, &class))
}

/// "I'm wearing this": without a login, the item you checked becomes what's in that slot.
#[tauri::command]
pub fn worn_set(app: AppHandle, text: String, class: String, slot: String) -> Option<WornView> {
    let mut w = current(&app);
    let poe2 = active_game(&app.state::<AppState>()) == Game::Poe2;
    if w.poe2 != poe2 { w = Worn { poe2, ..Default::default() }; }
    if let Some(it) = filter_core::trade::setup(&text, &Default::default()).map(|s| s.item) {
        w.by_hand = true;
        w.wear(&text, &slot, it.name.as_deref().unwrap_or(&it.base_type), &it.base_type, &it.rarity, unix());
        keep(&app, w);
    }
    view(&app, &text, &class)
}
