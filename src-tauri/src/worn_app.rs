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

// ---- "Find something better": a weighted-sum trade search per slot you wear (filter_core::upgrade).

/// A listing the upgrade search found, scored like your own gear.
#[derive(Serialize)]
pub struct Found { name: String, base: String, price: String, score: f64, better_by: f64, lines: Vec<String>, icon: String,
    /// What Path of Building says it changes on the build's stage (its numbers minus the stage's), when it could tell.
    pob: Option<std::collections::HashMap<String, f64>> }

#[derive(Serialize)]
pub struct Upgrades { total: u64, url: String, items: Vec<Found>, logged_in: bool,
    /// Why there are no Path of Building numbers ("" when there are, or when it wasn't asked).
    pob_note: String }

/// Whose gear the upgrade search improves on: the shown build's stage (its gear, its Str / Dex / Int shortfall, its
/// level) or what your character wears now (read from the site). Picked on the Upgrades tab; the build is the default
/// there, since it sits under the build (asked for 28 Nov 2026: "showing upgrades for a build that isn't active").
struct Source { gear: Vec<filter_core::pob::Gear>, short: [i64; 3], level: Option<u32> }

fn source(app: &AppHandle, from: &str, stage: Option<usize>) -> Source {
    let st = app.state::<AppState>();
    if from == "build" {
        let Some(p) = pob_load(&st) else { return Source { gear: vec![], short: [0; 3], level: None } };
        let i = stage.unwrap_or(p.stage).min(p.build.stages.len().saturating_sub(1));
        let c = p.checks.get(i);
        return Source {
            gear: p.build.stages.get(i).map(|s| s.gear.clone()).unwrap_or_default(),
            short: c.map(|c| c.short).unwrap_or([0; 3]),
            level: c.map(|c| c.level).filter(|l| *l > 0),
        };
    }
    // Your character: the shown build's shortfall doesn't apply to it.
    Source { gear: current(app).gear(), short: [0; 3], level: play_app::level_view(app.clone()).char_level }
}

#[derive(Serialize)]
pub struct UpgradePlans {
    plans: Vec<filter_core::upgrade::Plan>,
    /// The character whose gear is known ("" when none).
    character: String,
    /// The shown build's stage ("" without a build).
    stage: String,
}

/// A plan for every slot the build's stage (or your character) wears that the trade site can search by stats (not weapons).
#[tauri::command]
pub fn upgrade_plans(app: AppHandle, from: String, stage: Option<usize>) -> UpgradePlans {
    let s = source(&app, &from, stage);
    // Weapons first: a rare or magic attack weapon is searched by damage per second (28 Nov 2026); others aren't listed.
    let weapons = ["Weapon 1", "Weapon 2"].iter().filter(|sl| s.gear.iter().any(|g| g.slot == **sl))
        .map(|sl| filter_core::upgrade::plan(sl, &s.gear, s.short)).filter(|p| p.dps.is_some());
    let plans = weapons.chain(["Helmet", "Body Armour", "Gloves", "Boots", "Belt", "Amulet", "Ring 1", "Ring 2"].iter()
        .filter(|sl| s.gear.iter().any(|g| g.slot == **sl))
        .map(|sl| filter_core::upgrade::plan(sl, &s.gear, s.short))).collect();
    let st = app.state::<AppState>();
    let poe2 = active_game(&st) == Game::Poe2;
    let w = current(&app);
    let character = if !w.pieces.is_empty() && w.poe2 == poe2 { w.character } else { String::new() };
    let stage = pob_load(&st).and_then(|p| { let i = stage.unwrap_or(p.stage); p.build.stages.get(i).map(|s| s.title.clone()) }).unwrap_or_default();
    UpgradePlans { plans, character, stage }
}

/// A trade listing's mods as lines ("+99 to maximum Life"), whether the site sent them as text or with details.
fn listing_lines(item: &serde_json::Value) -> Vec<String> {
    let clean = |s: &str| {
        // "[PlayerItemRarity|Item Rarity]" -> "Item Rarity"
        let mut out = String::new();
        let mut rest = s;
        while let Some(i) = rest.find('[') {
            let Some(j) = rest[i..].find(']').map(|j| i + j) else { break };
            out.push_str(&rest[..i]);
            let inner = &rest[i + 1..j];
            out.push_str(inner.rsplit('|').next().unwrap_or(inner));
            rest = &rest[j + 1..];
        }
        out.push_str(rest);
        out
    };
    ["implicitMods", "explicitMods", "craftedMods", "fracturedMods", "enchantMods"].iter()
        .flat_map(|k| item[*k].as_array().cloned().unwrap_or_default())
        .filter_map(|m| m.as_str().map(String::from).or_else(|| m["description"].as_str().map(String::from)))
        .flat_map(|t| t.lines().map(clean).collect::<Vec<_>>())
        .collect()
}

/// Runs the search (logged in: the site needs that for weighted sums) and lists the cheapest ten with their score.
/// Without a login it opens the same search on the trade site, where your browser's login runs it.
#[tauri::command(async)]
pub fn upgrade_search(app: AppHandle, mut plan: filter_core::upgrade::Plan, from: String, stage: Option<usize>) -> Result<Upgrades, String> {
    let src = source(&app, &from, stage);
    // Stats switched off change what yours scores: work it out again from what's worn.
    // A weapon: the DPS kind picked on the page.
    if let Some(d) = &plan.dps { plan.now = d.value(&d.kind); }
    else if let Some(g) = src.gear.iter().find(|g| g.slot == plan.slot) { plan.now = filter_core::upgrade::score(&g.lines, &plan.wants); }
    let (poe2, league) = trade_league(&app);
    let level = src.level;
    let q = filter_core::upgrade::query(&plan, level).ok_or("Nothing to search for: switch at least one stat on")?;
    let Some((_, sess, _)) = play_app::account(&app) else {
        let url = if poe2 { format!("https://www.pathofexile.com/trade2/search/poe2/{}?q={}", trade_api::enc(&league), trade_api::enc(&q.to_string())) }
            else { format!("https://www.pathofexile.com/trade/search/{}?q={}", trade_api::enc(&league), trade_api::enc(&q.to_string())) };
        open_url(url.clone())?;
        return Ok(Upgrades { total: 0, url, items: vec![], logged_in: false, pob_note: String::new() });
    };
    let (id, ids, total) = trade_api::search_logged_in(poe2, &league, &q, &sess)?;
    let url = trade_api::site_url(poe2, &league, &id);
    let fetched = trade_api::fetch_logged_in(poe2, &ids, &id, &sess)?;
    let listed: Vec<&serde_json::Value> = fetched["result"].as_array().into_iter().flatten().collect();
    // Path of Building's own numbers for each listing on the build's stage (28 Nov 2026): only for a build (not
    // what your character wears: PoB would need its whole character).
    let (pob, pob_note) = if from == "build" { pob_numbers(&app, stage, &plan.slot, &listed) } else { (vec![], String::new()) };
    let items = listed.iter().enumerate().filter_map(|(i, r)| {
        let it = &r["item"];
        let lines = listing_lines(it);
        // Weapons: the trade site's own DPS numbers for the listing ("extended").
        let score = match &plan.dps {
            Some(d) => it["extended"][d.kind.as_str()].as_f64().unwrap_or(0.0),
            None => filter_core::upgrade::score(&lines, &plan.wants),
        };
        let p = &r["listing"]["price"];
        let price = match (p["amount"].as_f64(), p["currency"].as_str()) { (Some(a), Some(c)) => format!("{a} {c}"), _ => "no price".into() };
        Some(Found {
            name: it["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(it["typeLine"].as_str()?).to_string(),
            base: it["typeLine"].as_str().unwrap_or("").to_string(),
            price, better_by: (score - plan.now).round(), score: score.round(), lines,
            icon: it["icon"].as_str().unwrap_or("").to_string(),
            pob: pob.get(i).cloned().flatten(),
        })
    }).collect();
    Ok(Upgrades { total, url, items, logged_in: true, pob_note })
}

/// Each listing's change to the build's stage, by Path of Building (pob_worker.rs). Empty with a reason when PoB isn't
/// installed or couldn't tell; a listing PoB can't read gets None.
fn pob_numbers(app: &AppHandle, stage: Option<usize>, slot: &str, listed: &[&serde_json::Value]) -> (Vec<Option<std::collections::HashMap<String, f64>>>, String) {
    let st = app.state::<AppState>();
    let Some(code) = std::fs::read_to_string(pob_file(&st)).ok() else { return (vec![], String::new()) };
    let Ok(xml) = filter_core::pob::decode(code.trim()) else { return (vec![], String::new()) };
    let Some(p) = pob_load(&st) else { return (vec![], String::new()) };
    let i = stage.unwrap_or(p.stage).min(p.build.stages.len().saturating_sub(1));
    let title = p.build.stages.get(i).map(|s| s.title.clone()).unwrap_or_default();
    let texts: Vec<Option<String>> = listed.iter().map(|r| crate::pob_worker::listing_text(&r["item"])).collect();
    let asked: Vec<String> = texts.iter().map(|t| t.clone().unwrap_or_default()).collect();
    match crate::pob_worker::compare(&st.data_dir, &xml, &title, slot, &asked) {
        Ok((base, each)) => (each.into_iter().zip(&texts).map(|(r, t)| {
            let n = r.ok().filter(|_| t.is_some())?;
            Some(n.iter().map(|(k, v)| (k.clone(), v - base.get(k).copied().unwrap_or(0.0))).collect())
        }).collect(), String::new()),
        Err(e) => (vec![], e),
    }
}

/// Ctrl+D: what Path of Building says this item would do to the shown build's stage, per slot it could go in
/// (both rings). 28 Nov 2026, he picked "PoB numbers in more places" after Upgrades got them.
#[derive(Serialize)]
pub struct PobCheck {
    /// "Luminary" and the stage's name.
    build: String,
    stage: String,
    rows: Vec<PobRow>,
    /// Why there's nothing ("" when there are rows).
    note: String,
}

#[derive(Serialize)]
pub struct PobRow { slot: String, now: String, change: Option<std::collections::HashMap<String, f64>>, error: String }

#[tauri::command(async)]
pub fn pob_check(app: AppHandle, text: String, item_class: String) -> PobCheck {
    let st = app.state::<AppState>();
    let empty = |note: &str| PobCheck { build: String::new(), stage: String::new(), rows: vec![], note: note.into() };
    let slots = filter_core::compare::slots_for(&item_class);
    if slots.is_empty() { return empty(""); }
    let (Some(code), Some(p)) = (std::fs::read_to_string(pob_file(&st)).ok(), pob_load(&st)) else { return empty("") };
    let Ok(xml) = filter_core::pob::decode(code.trim()) else { return empty("") };
    let Some(stage) = p.build.stages.get(p.stage) else { return empty("") };
    let build = if p.build.ascendancy.is_empty() { p.build.class.clone() } else { p.build.ascendancy.clone() };
    let mut rows = vec![];
    for slot in slots {
        let now = stage.gear.iter().find(|g| g.slot == *slot).map(|g| g.name.clone()).unwrap_or_default();
        match crate::pob_worker::compare(&st.data_dir, &xml, &stage.title, slot, std::slice::from_ref(&text)) {
            Ok((base, each)) => {
                let (change, error) = match each.into_iter().next() {
                    Some(Ok(n)) => (Some(n.iter().map(|(k, v)| (k.clone(), v - base.get(k).copied().unwrap_or(0.0))).collect()), String::new()),
                    Some(Err(e)) => (None, e),
                    None => (None, "no answer".into()),
                };
                rows.push(PobRow { slot: slot.to_string(), now, change, error });
            }
            Err(e) => return PobCheck { build, stage: stage.title.clone(), rows: vec![], note: e },
        }
    }
    PobCheck { build, stage: stage.title.clone(), rows, note: String::new() }
}
