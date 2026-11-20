// The app (core logic lives in ../core; both are rebuilt by `tauri dev`).
mod screen;
mod trade_api;
mod wealth_app;

use filter_core::{
    evaluate::{status_with_fallback, Status},
    filter::{parse_filter, Segment},
    area::{find_log, read_area_level, selected_filter},
    ground::{ground_item, known_names, match_base, LabelColours},
    neversink,
    history::History,
    ask::{claude_tool, plan, read_offline, Plan, Request},
    search::{build_defaults, farm_presets, farm_rules, level_builds, level_rules_opts, search, weapon_classes, Found, LevelOptions, Preset, ARMOUR_TYPES, LEVEL_PREFIX, VENDOR_RANGES},
    item::{is_stackable, parse_item, resolve_magic_base, Item},
    preview::{preview, PreviewDrop},
    prices::{default_settings, price_of, PriceFile, PriceSettings},
    profiles::{build_with_prices, render_profile, Game, Settings, Store},
    themes::{builtin_themes, Theme},
    rules::{add_rule, shadowed_by, custom_rule, edited_rule, known_bases, locate, quick_rule, rule_detail, Action, CustomStyle, Location, MatchOpts, Rule, RuleDetail, RuleEdit},
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

// ---------- state ----------

struct AppState {
    store: Mutex<Store>,
    store_path: PathBuf,
    /// Latest prices per game, downloaded hourly from the app's own price cache.
    prices: Mutex<std::collections::HashMap<Game, PriceFile>>,
    data_dir: PathBuf,
    /// Item name -> picture URL, per game (from the price job).
    icons: Mutex<std::collections::HashMap<Game, std::collections::HashMap<String, String>>>,
    /// Every filter's recent changes, for going back.
    history: Mutex<History>,
    history_path: PathBuf,
    /// Registered hotkeys: shortcut id -> what it does.
    hotkeys: Mutex<std::collections::HashMap<u32, HotAction>>,
    /// Saved price searches (some of them live).
    saved: Mutex<filter_core::saved::Saved>,
}

#[derive(Clone, Copy, PartialEq)]
enum HotAction { QuickEdit, Batch, Reload, Cycle, Build, Price, Chat(usize) }

/// (Re)registers hotkeys from settings. Errors name the key that failed.
fn register_hotkeys(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let mut wanted = vec![(s.key_quick_edit.as_str(), HotAction::QuickEdit), (s.key_batch.as_str(), HotAction::Batch),
        (s.key_reload.as_str(), HotAction::Reload), (s.key_build.as_str(), HotAction::Build)];
    if !s.key_price.trim().is_empty() { wanted.push((s.key_price.as_str(), HotAction::Price)); }
    if s.cycle_enabled { wanted.push((s.key_cycle.as_str(), HotAction::Cycle)); }
    for (i, c) in s.chat_keys.iter().enumerate() {
        if !c.key.trim().is_empty() && !c.command.trim().is_empty() { wanted.push((c.key.as_str(), HotAction::Chat(i))); }
    }
    let mut parsed = Vec::new();
    for (key, action) in wanted {
        let sc: Shortcut = key.parse().map_err(|_| format!("“{key}” isn't a key the app understands"))?;
        if parsed.iter().any(|(p, _): &(Shortcut, HotAction)| p.id() == sc.id()) {
            return Err(format!("“{key}” is used for two things"));
        }
        parsed.push((sc, action));
    }
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let state = app.state::<AppState>();
    let mut map = state.hotkeys.lock().unwrap();
    map.clear();
    for (sc, action) in parsed {
        match gs.register(sc) {
            Ok(()) => { map.insert(sc.id(), action); }
            // Another price checker (e.g. Awakened PoE Trade) may own Ctrl+D: carry on without it.
            Err(e) if action == HotAction::Price => {
                let _ = app.emit("hotkey-warning", format!("Price check key unavailable (another program may own it): {e}"));
            }
            Err(e) if matches!(action, HotAction::Chat(_)) => {
                let _ = app.emit("hotkey-warning", format!("A chat hotkey is unavailable (another program may own it): {e}"));
            }
            Err(e) => return Err(format!("Couldn't use that key (another program may own it): {e}")),
        }
    }
    Ok(())
}

/// Switches to the next filter for this game and loads it in game.
fn cycle_filter(app: &AppHandle) {
    let state = app.state::<AppState>();
    let next = {
        let mut store = state.store.lock().unwrap();
        let Some(cur) = store.active.clone() else { return };
        let Some(game) = store.profiles.get(&cur).map(|p| p.game) else { return };
        let ids: Vec<String> = store.profiles.values().filter(|p| p.game == game).map(|p| p.id.clone()).collect();
        let i = ids.iter().position(|x| *x == cur).unwrap_or(0);
        let next = ids[(i + 1) % ids.len()].clone();
        store.active = Some(next.clone());
        let _ = store.save(&state.store_path);
        next
    };
    let _ = app.emit("active-changed", next);
    let _ = reload_in_game(app, 0); // the game's chat confirms which filter loaded
}

const PRICE_URL: &str = "https://raw.githubusercontent.com/GROBBS-hash/poe-filter-editor/prices";

fn game_key(g: Game) -> &'static str { match g { Game::Poe1 => "poe1", Game::Poe2 => "poe2" } }

/// Downloads the latest prices. Returns the games whose prices changed.
fn fetch_prices(app: &AppHandle) -> Vec<Game> {
    let state = app.state::<AppState>();
    let mut changed = Vec::new();
    for g in [Game::Poe1, Game::Poe2] {
        let url = format!("{PRICE_URL}/{}.json", game_key(g));
        let Ok(resp) = ureq::get(&url).set("User-Agent", "poe-filter-editor").timeout(Duration::from_secs(20)).call() else { continue };
        let Ok(body) = resp.into_string() else { continue };
        let Ok(file) = serde_json::from_str::<PriceFile>(&body) else { continue };
        let mut prices = state.prices.lock().unwrap();
        let no_icons = state.icons.lock().unwrap().get(&g).is_none();
        let new_prices = prices.get(&g).map(|p| p.updated != file.updated).unwrap_or(true);
        if new_prices || no_icons {
            let _ = std::fs::write(state.data_dir.join(format!("prices_{}.json", game_key(g))), &body);
            prices.insert(g, file);
            if new_prices { changed.push(g); }
            // Item pictures come with each price update.
            let icon_url = format!("{PRICE_URL}/{}_icons.json", game_key(g));
            if let Some(text) = ureq::get(&icon_url).timeout(Duration::from_secs(20)).call().ok().and_then(|r| r.into_string().ok()) {
                if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&text) {
                    let _ = std::fs::write(state.data_dir.join(format!("icons_{}.json", game_key(g))), &text);
                    state.icons.lock().unwrap().insert(g, map);
                }
            }
        }
    }
    changed
}

/// Rebuild every filter that uses price tiers for these games.
fn rebuild_priced(app: &AppHandle, games: &[Game]) {
    let state = app.state::<AppState>();
    let store = state.store.lock().unwrap();
    let ids: Vec<String> = store.profiles.values()
        .filter(|p| games.contains(&p.game) && p.prices.as_ref().map(|s| s.enabled).unwrap_or(false))
        .map(|p| p.id.clone()).collect();
    for id in ids {
        let _ = save_and_build(&state, &store, &id);
    }
}

/// Hourly price updates, and NeverSink updates every 6 hours, in the background.
fn start_price_updates(app: AppHandle) {
    thread::spawn(move || {
        let mut hour = 0u32;
        loop {
            let changed = fetch_prices(&app);
            if !changed.is_empty() {
                rebuild_priced(&app, &changed);
                let _ = app.emit("prices-updated", ());
            }
            if hour % 6 == 0 {
                update_neversink(&app);
            }
            hour += 1;
            thread::sleep(Duration::from_secs(60 * 60));
        }
    });
}

fn neversink_path(state: &AppState, game: Game, level: u8) -> PathBuf {
    state.data_dir.join("neversink").join(game_key(game)).join(neversink::file_name(game, level))
}

/// Downloads NeverSink's file for a strictness. Returns (path, version, changed).
fn download_neversink(state: &AppState, game: Game, level: u8) -> Result<(PathBuf, Option<String>, bool), String> {
    let path = neversink_path(state, game, level);
    let body = ureq::get(&neversink::url(game, level)).set("User-Agent", "poe-filter-editor")
        .timeout(Duration::from_secs(60)).call().map_err(|e| format!("Couldn't download NeverSink's filter: {e}"))?
        .into_string().map_err(err)?;
    if !neversink::looks_like_neversink(&body) {
        return Err("The download didn't look like NeverSink's filter".into());
    }
    let old = std::fs::read_to_string(&path).ok();
    let changed = old.as_deref().map(neversink::version_of) != Some(neversink::version_of(&body));
    if changed || old.is_none() {
        std::fs::create_dir_all(path.parent().unwrap()).map_err(err)?;
        std::fs::write(&path, &body).map_err(err)?;
    }
    Ok((path, neversink::version_of(&body), changed))
}

/// Gets new NeverSink versions for every filter built on one, and rebuilds them.
fn update_neversink(app: &AppHandle) {
    let state = app.state::<AppState>();
    let used: Vec<(Game, u8)> = {
        let store = state.store.lock().unwrap();
        let mut v: Vec<(Game, u8)> = store.profiles.values().filter_map(|p| p.strictness.map(|s| (p.game, s))).collect();
        v.sort_by_key(|(g, s)| (game_key(*g), *s));
        v.dedup();
        v
    };
    for (game, level) in used {
        if let Ok((_, version, true)) = download_neversink(&state, game, level) {
            let store = state.store.lock().unwrap();
            let ids: Vec<String> = store.profiles.values().filter(|p| p.game == game && p.strictness == Some(level)).map(|p| p.id.clone()).collect();
            for id in ids { let _ = save_and_build(&state, &store, &id); }
            let _ = app.emit("neversink-updated", version.unwrap_or_default());
        }
    }
}

/// Saves a filter's setup before a change, so it can be gone back to.
fn remember(state: &AppState, store: &Store, id: &str, what: &str) {
    if let Some(p) = store.profiles.get(id) {
        let mut h = state.history.lock().unwrap();
        h.record(p, what);
        let _ = h.save(&state.history_path);
    }
}

static BATCH_ON: AtomicBool = AtomicBool::new(false);

fn filter_dir(game: Game) -> PathBuf {
    let docs = dirs::document_dir().unwrap_or_else(|| PathBuf::from("."));
    docs.join("My Games").join(match game {
        Game::Poe1 => "Path of Exile",
        Game::Poe2 => "Path of Exile 2",
    })
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

/// Item text from the game -> Item, with magic bases worked out from the
/// active profile's base filter.
fn read_item(app: &AppHandle, text: &str) -> Option<Item> {
    let mut item = parse_item(text)?;
    let state = app.state::<AppState>();
    let store = state.store.lock().unwrap();
    if let Some(p) = store.active.as_ref().and_then(|id| store.profiles.get(id)) {
        if let Ok(base) = std::fs::read_to_string(filter_dir(p.game).join(&p.base_file)) {
            resolve_magic_base(&mut item, &known_bases(&parse_filter(&base)));
        }
    }
    Some(item)
}

/// The real area level from the game's log, if we can find it.
fn area_level(app: &AppHandle) -> Option<u32> {
    let state = app.state::<AppState>();
    let poe2 = {
        let store = state.store.lock().unwrap();
        store.active.as_ref().and_then(|id| store.profiles.get(id)).map(|p| p.game == Game::Poe2).unwrap_or(false)
    };
    read_area_level(&find_log(poe2, &[])?)
}

fn fallback_area_level(app: &AppHandle) -> u32 {
    app.state::<AppState>().store.lock().unwrap().settings.fallback_area_level
}

/// The filter the game has selected, and whether it's this app's one.
#[derive(Serialize, Clone)]
struct GameFilter { selected: Option<String>, expected: String, matches: bool }

fn game_filter(app: &AppHandle) -> Option<GameFilter> {
    let state = app.state::<AppState>();
    let store = state.store.lock().unwrap();
    let p = store.active.as_ref().and_then(|id| store.profiles.get(id))?;
    let cfg = match p.game { Game::Poe1 => "production_Config.ini", Game::Poe2 => "poe2_production_Config.ini" };
    let selected = std::fs::read_to_string(filter_dir(p.game).join(cfg)).ok().and_then(|t| selected_filter(&t));
    let expected = p.output_file.clone();
    let strip = |s: &str| s.trim_end_matches(".filter").to_lowercase();
    let matches = selected.as_deref().map(|s| strip(s) == strip(&expected)).unwrap_or(true);
    Some(GameFilter { selected, expected, matches })
}

/// The filter the game is actually using (with your edits), and the clean base.
fn active_filters(app: &AppHandle) -> Option<(Vec<Segment>, Vec<Segment>)> {
    let state = app.state::<AppState>();
    let store = state.store.lock().unwrap();
    let p = store.active.as_ref().and_then(|id| store.profiles.get(id))?;
    let dir = filter_dir(p.game);
    let base = std::fs::read_to_string(dir.join(&p.base_file)).ok()?;
    let out = std::fs::read_to_string(dir.join(&p.output_file)).unwrap_or_else(|_| base.clone());
    Some((parse_filter(&out), parse_filter(&base)))
}

#[derive(Serialize, Clone)]
struct Picked {
    item: Item,
    from_screen: bool,
    screen_text: Option<String>,
    confidence: f32,
    /// Read off the ground and your filter draws its rarities alike: we can't tell which.
    #[serde(default)]
    rarity_unsure: bool,
}

/// No item text on the clipboard: read the ground label under the mouse instead.
fn read_from_ground(app: &AppHandle) -> Result<Picked, String> {
    let (live, base) = active_filters(app).ok_or("Set up a filter first")?;
    let names = known_names(&known_bases(&base));
    let accept = |t: &str| match_base(t, &names).is_some();
    // Our own pop-ups must not be in the picture (their text was being read as a hover box).
    let mut hid = false;
    for label in ["overlay", "trade"] {
        if let Some(w) = app.get_webview_window(label) {
            if w.is_visible().unwrap_or(false) { let _ = w.hide(); hid = true; }
        }
    }
    if hid { thread::sleep(Duration::from_millis(120)); }
    let (read, tip_lines) = screen::read_ground(&accept)?;
    // The game's hover box: bigger and clearer than the label, and it says the rarity.
    let tip = filter_core::ground::from_tooltip(&tip_lines, &names);
    let label = if read.text.trim().is_empty() { None } else { match_base(&read.text, &names) };
    // The box is always about the item under the mouse, while the label read can catch
    // a neighbour, so a clearly read box wins. A box only guessed at (garbled name)
    // must agree with what was read off the label.
    let tip = tip.filter(|t| {
        t.found.confidence >= 1.0
            || label.as_ref().map(|l| l.base == t.found.base).unwrap_or(false)
            || filter_core::ground::text_fits(&read.text, &t.header)
    });
    // Failed reads are saved next to your filters (ReaderSnapshots) so they can be looked at and fixed.
    let keep = |why: &str| {
        let game = active_game(&app.state::<AppState>());
        screen::save_snapshot(&filter_dir(game).join("ReaderSnapshots"), &read, why)
            .map(|p| format!(" (saved what it saw: {})", p.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()))
            .unwrap_or_default()
    };
    let m = match (label, &tip) {
        // Same item both ways: the label knows the stack size and tier.
        (Some(l), Some(t)) if l.base == t.found.base => l,
        // They disagree: the label probably ran into its neighbour; trust the box.
        (_, Some(t)) => { let _ = keep("Label and hover box disagreed"); t.found.clone() }
        (Some(l), None) => l,
        (None, None) if read.text.trim().is_empty() =>
            return Err(format!("Couldn't see an item label under your mouse{}", keep("No label found near the mouse"))),
        (None, None) => return Err(format!("Read \"{}\" but couldn't match it to an item{}", read.text, keep("Read text but no item matched"))),
    };
    let colours = read.colours.map(|(text, background)| LabelColours { text, background });
    let mut item = ground_item(&live, &m, colours, area_level(app).unwrap_or_else(|| fallback_area_level(app)));
    let area = area_level(app).unwrap_or_else(|| fallback_area_level(app));
    let mut rarity_unsure = filter_core::item::is_gear(&item) && item.rarity != "Unique"
        && filter_core::ground::rarity_ambiguous(&live, &item, colours, area);
    if let Some(t) = &tip {
        filter_core::ground::apply_tooltip_checked(&live, &mut item, t, colours, area);
        // The hover box's name colour settles it.
        if t.rarity.is_some() || t.found.unique.is_some() { rarity_unsure = false; }
    }
    let text = if read.text.trim().is_empty() { m.base.clone() } else { read.text.clone() };
    Ok(Picked { item, from_screen: true, screen_text: Some(text), confidence: m.confidence, rarity_unsure })
}

/// The screen (x, y, width, height) the point is on, or the first screen.
fn monitor_at(w: &tauri::WebviewWindow, x: f64, y: f64) -> Option<(i32, i32, i32, i32)> {
    let mons = w.available_monitors().ok()?;
    let rect = |m: &tauri::Monitor| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32);
    mons.iter().map(rect).find(|(mx, my, mw, mh)| x >= *mx as f64 && x < (*mx + *mw) as f64 && y >= *my as f64 && y < (*my + *mh) as f64)
        .or_else(|| mons.first().map(rect))
}

/// Puts a pop-up next to the mouse on the side with more room (left of the mouse
/// on the right half of the screen), kept fully on that screen.
fn place_near_cursor(w: &tauri::WebviewWindow, gap: i32) {
    let Ok(pos) = w.cursor_position() else { return };
    let Some((mx, my, mw, mh)) = monitor_at(w, pos.x, pos.y) else { return };
    let size = w.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((380, 500));
    let (cx, cy) = (pos.x as i32, pos.y as i32);
    let x = if cx > mx + mw / 2 { cx - size.0 - gap } else { cx + gap };
    let y = cy - size.1 / 3;
    let x = x.clamp(mx, (mx + mw - size.0).max(mx));
    let y = y.clamp(my, (my + mh - size.1).max(my));
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

/// Called by a pop-up after it changes size: pulls it back fully onto its screen.
#[tauri::command]
fn keep_on_screen(window: tauri::WebviewWindow) {
    let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) else { return };
    let Some((mx, my, mw, mh)) = monitor_at(&window, p.x as f64 + 1.0, p.y as f64 + 1.0) else { return };
    let (w, h) = (s.width as i32, s.height as i32);
    let x = p.x.clamp(mx, (mx + mw - w).max(mx));
    let y = p.y.clamp(my, (my + mh - h).max(my));
    if (x, y) != (p.x, p.y) { let _ = window.set_position(PhysicalPosition::new(x, y)); }
}

fn show_overlay(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    if let Some(w) = app.get_webview_window("overlay") {
        place_near_cursor(&w, 12);
        let _ = w.emit(event, payload);
        let _ = w.show();
        let _ = w.set_focus();
    }
}

// ---------- hotkeys ----------

fn send_ctrl_c() {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    if let Ok(mut e) = Enigo::new(&Settings::default()) {
        let _ = e.key(Key::Control, Direction::Press);
        let _ = e.key(Key::Unicode('c'), Direction::Click);
        let _ = e.key(Key::Control, Direction::Release);
    }
}

/// F2: copy the hovered item, open the quick menu at the mouse.
fn quick_edit(app: AppHandle) {
    thread::spawn(move || {
        let before = app.clipboard().read_text().unwrap_or_default();
        let _ = app.clipboard().write_text(String::new());
        send_ctrl_c();
        let mut text = String::new();
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(30));
            text = app.clipboard().read_text().unwrap_or_default();
            if !text.is_empty() {
                break;
            }
        }
        let _ = app.clipboard().write_text(before);
        // Inventory/stash: the game copied the item. Ground: read the label instead.
        let picked = match read_item(&app, &text) {
            Some(item) => Ok(Picked { item, from_screen: false, screen_text: None, confidence: 1.0, rarity_unsure: false }),
            None => read_from_ground(&app),
        };
        match picked {
            Ok(p) => show_overlay(&app, "item-picked", p),
            Err(e) => show_overlay(&app, "pick-failed", e),
        }
    });
}

fn send_ctrl_alt_c() {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    if let Ok(mut e) = Enigo::new(&Settings::default()) {
        let _ = e.key(Key::Control, Direction::Press);
        let _ = e.key(Key::Alt, Direction::Press);
        let _ = e.key(Key::Unicode('c'), Direction::Click);
        let _ = e.key(Key::Alt, Direction::Release);
        let _ = e.key(Key::Control, Direction::Release);
    }
}

// ---------- price check (Ctrl+D) ----------

#[derive(Serialize, Clone)]
struct PriceCheck {
    setup: Option<filter_core::trade::TradeSetup>,
    /// poe.ninja's price (value, unit) when it has the item.
    ninja: Option<(f64, String)>,
    league: String,
    /// Read off the ground (name only, no stats).
    from_ground: bool,
    error: Option<String>,
    /// The trade site's bulk id when this sells in bulk (currency, scarabs, cards…).
    bulk: Option<String>,
    /// Ctrl+D on a map: its mods, with warnings for the ones you said you can't run.
    map: Option<filter_core::maps::MapInfo>,
    /// Chaos per divine (poe.ninja), to show prices in both.
    divine: Option<f64>,
    /// Against the gear in the same slot of your imported PoB build.
    compare: Vec<filter_core::compare::Comparison>,
}

fn trade_league(app: &AppHandle) -> (bool, String) {
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let league = state.prices.lock().unwrap().get(&game).map(|f| f.league.clone()).unwrap_or_else(|| "Standard".into());
    (game == Game::Poe2, league)
}

fn ninja_price(app: &AppHandle, item: &Item) -> Option<(f64, String)> {
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let f = prices.get(&game)?;
    price_of(f, item.name.as_deref(), &item.base_type, &item.rarity).map(|v| (v, f.unit.clone()))
}

/// Ctrl+D: copies the hovered item (advanced copy: mod tiers too) and opens the price window.
fn price_check(app: AppHandle) {
    thread::spawn(move || {
        if !game_in_front() { return; }
        let before = app.clipboard().read_text().unwrap_or_default();
        let _ = app.clipboard().write_text(String::new());
        send_ctrl_alt_c();
        let mut text = String::new();
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(30));
            text = app.clipboard().read_text().unwrap_or_default();
            if !text.is_empty() { break; }
        }
        let _ = app.clipboard().write_text(before);
        let (poe2, league) = trade_league(&app);
        let state = app.state::<AppState>();
        let payload = if !text.is_empty() {
            match trade_api::stats(&state.data_dir, poe2) {
                Ok(index) => match filter_core::trade::setup(&text, &index) {
                    Some(s) => PriceCheck { ninja: ninja_price(&app, &s.item), setup: Some(s), league, from_ground: false, error: None, bulk: None, map: None, divine: None, compare: vec![] },
                    None => PriceCheck { setup: None, ninja: None, league, from_ground: false, error: Some("That doesn't look like an item".into()), bulk: None, map: None, divine: None, compare: vec![] },
                },
                Err(e) => match filter_core::trade::setup(&text, &Default::default()) {
                    Some(s) => PriceCheck { ninja: ninja_price(&app, &s.item), setup: Some(s), league, from_ground: false,
                        error: Some(format!("Stat list unavailable, searching by name only ({e})")), bulk: None, map: None, divine: None, compare: vec![] },
                    None => PriceCheck { setup: None, ninja: None, league, from_ground: false, error: Some(e), bulk: None, map: None, divine: None, compare: vec![] },
                },
            }
        } else {
            // On the ground the game won't copy: read the label for its name.
            match read_from_ground(&app) {
                Ok(p) => {
                    let item = p.item;
                    let category = filter_core::trade::category(&item.item_class).map(String::from);
                    let s = filter_core::trade::TradeSetup { item: item.clone(), filters: vec![], base_only: true, ilvl_min: None,
                        links_min: None, gem_level: None, unknown: vec![], unknown_kinds: vec![], category, raw: String::new() };
                    PriceCheck { ninja: ninja_price(&app, &item), setup: Some(s), league, from_ground: true, error: None, bulk: None, map: None, divine: None, compare: vec![] }
                }
                Err(e) => PriceCheck { setup: None, ninja: None, league, from_ground: true, error: Some(e), bulk: None, map: None, divine: None, compare: vec![] },
            }
        };
        let mut payload = payload;
        payload.divine = currency_rates(&app).get("divine").copied();
        // "Is this an upgrade?" against your PoB build's gear in that slot.
        if !state.store.lock().unwrap().settings.price_rolls {
            if let Some(s) = payload.setup.as_mut() { for f in &mut s.filters { f.roll = None; } }
        }
        if !payload.from_ground && state.store.lock().unwrap().settings.price_compare {
            if let (Some(setup), Some(pob)) = (payload.setup.as_ref(), pob_load(&state)) {
                if let Some(stage) = pob.build.stages.get(pob.stage) {
                    let lines: Vec<String> = filter_core::trade::mods(&text).into_iter().map(|m| m.text).collect();
                    payload.compare = filter_core::compare::compare(&lines, &setup.item.item_class, &stage.gear);
                }
            }
        }
        if let Some(it) = payload.setup.as_ref().map(|s| s.item.clone()) {
            if filter_core::maps::is_map(&it.item_class) && !payload.from_ground {
                let avoid = state.store.lock().unwrap().settings.map_avoid.clone();
                payload.map = Some(filter_core::maps::map_info(&text, avoid.as_deref()));
            }
        }
        if let Some(it) = payload.setup.as_ref().map(|s| s.item.clone()) {
            if !matches!(it.rarity.as_str(), "Rare" | "Magic" | "Unique") && filter_core::trade::category(&it.item_class).is_none() {
                payload.bulk = trade_api::statics(&state.data_dir, poe2).ok().and_then(|st| st.id(&it.base_type));
            }
        }
        if let Some(w) = app.get_webview_window("trade") {
            place_near_cursor(&w, 24);
            let _ = w.emit("price-check", payload);
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
}

#[derive(Serialize)]
struct TradeResult {
    listings: Vec<filter_core::trade::Listing>,
    /// (lowest, typical) in chaos.
    summary: Option<(f64, f64)>,
    total: u64,
    url: String,
    /// What a Divine Orb is worth in chaos, for showing prices in divines.
    divine: Option<f64>,
}

/// Chaos value of each trade currency, from the latest poe.ninja prices.
fn currency_rates(app: &AppHandle) -> std::collections::HashMap<String, f64> {
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let f = prices.get(&game);
    filter_core::trade::rates(|name| f.and_then(|f| price_of(f, None, name, "Currency")))
}

/// The trade site search for a price window setup (with transfigured gems' search names).
fn trade_query(app: &AppHandle, poe2: bool, setup: &filter_core::trade::TradeSetup) -> serde_json::Value {
    let mut q = filter_core::trade::query(setup);
    if let Ok(types) = trade_api::item_types(&app.state::<AppState>().data_dir, poe2) { types.fix_query(&mut q); }
    q
}

/// When you last price checked by hand: live searches wait so yours go first.
static LAST_MANUAL: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Runs the price search on the trade site (from the price window).
#[tauri::command(async)]
fn trade_search(app: AppHandle, setup: filter_core::trade::TradeSetup) -> Result<TradeResult, String> {
    *LAST_MANUAL.lock().unwrap() = Some(std::time::Instant::now());
    let (poe2, league) = trade_league(&app);
    let q = trade_query(&app, poe2, &setup);
    let (id, ids, total) = trade_api::search(poe2, &league, &q)?;
    let fetched = trade_api::fetch(poe2, &ids, &id)?;
    let rates = currency_rates(&app);
    let listings = filter_core::trade::listings(&fetched, &rates);
    Ok(TradeResult { summary: filter_core::trade::summary(&listings), total, url: trade_api::site_url(poe2, &league, &id),
        divine: rates.get("divine").copied(), listings })
}

// ---------- saved & live searches ----------

#[derive(Serialize)]
struct SavedInfo { id: u64, name: String, live: bool, setup: filter_core::trade::TradeSetup }

fn saved_infos(state: &AppState) -> Vec<SavedInfo> {
    state.saved.lock().unwrap().list.iter().map(|s| SavedInfo { id: s.id, name: s.name.clone(), live: s.live, setup: s.setup.clone() }).collect()
}

fn saved_store(state: &AppState) -> Result<Vec<SavedInfo>, String> {
    state.saved.lock().unwrap().save(&state.data_dir.join("saved_searches.json"))?;
    Ok(saved_infos(state))
}

#[tauri::command]
fn saved_list(state: State<AppState>) -> Vec<SavedInfo> { saved_infos(&state) }

#[tauri::command]
fn saved_add(state: State<AppState>, name: String, setup: filter_core::trade::TradeSetup) -> Result<Vec<SavedInfo>, String> {
    state.saved.lock().unwrap().add(&name, setup);
    saved_store(&state)
}

#[tauri::command]
fn saved_delete(state: State<AppState>, id: u64) -> Result<Vec<SavedInfo>, String> {
    state.saved.lock().unwrap().remove(id);
    saved_store(&state)
}

#[tauri::command]
fn saved_live(state: State<AppState>, id: u64, on: bool) -> Result<Vec<SavedInfo>, String> {
    state.saved.lock().unwrap().set_live(id, on)?;
    saved_store(&state)
}

#[derive(Serialize, Clone)]
struct LiveHit { search: String, listings: Vec<filter_core::trade::Listing>, url: String }

/// Re-checks each live search on the trade site about every 90 seconds and pops up
/// anything newly listed. Polite to the site: only when there's room under its
/// limits with requests to spare, and never just after you price check something.
fn start_live_searches(app: AppHandle) {
    thread::spawn(move || {
        let mut due: std::collections::HashMap<u64, std::time::Instant> = Default::default();
        loop {
            thread::sleep(Duration::from_secs(5));
            let state = app.state::<AppState>();
            let live: Vec<(u64, String, filter_core::trade::TradeSetup)> = state.saved.lock().unwrap().list.iter()
                .filter(|s| s.live).map(|s| (s.id, s.name.clone(), s.setup.clone())).collect();
            due.retain(|id, _| live.iter().any(|l| l.0 == *id));
            let now = std::time::Instant::now();
            for (id, name, setup) in live {
                if due.get(&id).map(|t| *t > now).unwrap_or(false) { continue; }
                if LAST_MANUAL.lock().unwrap().map(|t| t.elapsed() < Duration::from_secs(20)).unwrap_or(false) { break; }
                if !trade_api::has_room("search", 3) || !trade_api::has_room("fetch", 3) { break; }
                let (poe2, league) = trade_league(&app);
                let q = trade_query(&app, poe2, &setup);
                let fresh = match trade_api::search(poe2, &league, &q) {
                    Ok((sid, ids, _)) => {
                        due.insert(id, now + Duration::from_secs(90));
                        let fresh = state.saved.lock().unwrap().fresh(id, &ids);
                        if fresh.is_empty() { continue; }
                        (sid, fresh)
                    }
                    Err(_) => { due.insert(id, now + Duration::from_secs(180)); continue; }
                };
                let (sid, ids) = fresh;
                let Ok(fetched) = trade_api::fetch(poe2, &ids, &sid) else { continue };
                let listings = filter_core::trade::listings(&fetched, &currency_rates(&app));
                if listings.is_empty() { continue; }
                if let Some(w) = app.get_webview_window("trades") {
                    let _ = w.emit("live-hit", LiveHit { search: name, listings, url: trade_api::site_url(poe2, &league, &sid) });
                    if !w.is_visible().unwrap_or(false) {
                        let front = game_in_front();
                        let _ = w.show();
                        if front { focus_game(); }
                    }
                }
            }
        }
    });
}

/// Sends a seller the trade site's whisper: brings the game to the front and types it (one message per press).
#[tauri::command]
async fn send_whisper(text: String) -> Result<(), String> {
    if !text.starts_with('@') { return Err("That isn't a whisper".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        focus_game();
        type_in_chat(&text, 1500)
    }).await.map_err(err)?
}

#[derive(Serialize)]
struct BulkResult { offers: Vec<filter_core::trade::Offer>, url: String, divine: Option<f64> }

/// Bulk rates: people selling `want` (a trade id like "divine") for `pay` ("chaos"/"divine").
#[tauri::command(async)]
fn trade_bulk(app: AppHandle, want: String, pay: String) -> Result<BulkResult, String> {
    let (poe2, league) = trade_league(&app);
    let (id, v) = trade_api::exchange(poe2, &league, &filter_core::trade::exchange_query(&pay, &want, None))?;
    let state = app.state::<AppState>();
    let game = active_game(&state);
    let divine = state.prices.lock().unwrap().get(&game).and_then(|f| price_of(f, None, "Divine Orb", "Currency"));
    Ok(BulkResult { offers: filter_core::trade::offers(&v).into_iter().take(20).collect(), url: trade_api::exchange_url(poe2, &league, &id), divine })
}

/// poeprices.info's rough price for a rare: (min, max, currency, confidence %).
#[tauri::command(async)]
fn rare_estimate(app: AppHandle, text: String) -> Result<Option<(f64, f64, String, f64)>, String> {
    let (_, league) = trade_league(&app);
    Ok(filter_core::trade::poeprices(&trade_api::poeprices(&league, &text)?))
}

// ---------- map tools (regex + trade) ----------

const POERE: &str = "https://data.poe.re";
const POERE_UA: &str = "poe-filter-editor/0.1 (local app)";

/// The map mod list from poe.re: downloaded once a day (kept on disk), else the last
/// copy, else the one built into the app. Returns (mods, where it came from).
fn map_mods(state: &AppState) -> (Vec<filter_core::mapregex::MapModEntry>, String) {
    static CACHE: Mutex<Option<(std::time::Instant, Vec<filter_core::mapregex::MapModEntry>, String)>> = Mutex::new(None);
    if let Some((t, m, src)) = CACHE.lock().unwrap().as_ref() {
        if t.elapsed() < Duration::from_secs(6 * 3600) { return (m.clone(), src.clone()); }
    }
    let mods_path = state.data_dir.join("poere_mapmods.json");
    let ids_path = state.data_dir.join("poere_trade_ids.json");
    let fresh = std::fs::metadata(&mods_path).and_then(|m| m.modified()).ok()
        .and_then(|t| std::time::SystemTime::now().duration_since(t).ok()).map(|a| a < Duration::from_secs(86_400)).unwrap_or(false);
    let get = |url: &str| -> Result<serde_json::Value, String> {
        ureq::get(url).set("User-Agent", POERE_UA).timeout(Duration::from_secs(15)).call().map_err(err)?
            .into_json::<serde_json::Value>().map_err(err)
    };
    if !fresh {
        let download = || -> Result<(), String> {
            let cur = get(&format!("{POERE}/current.json"))?;
            let patch = cur["current"]["poe1"]["patch"].as_str().ok_or("no patch")?;
            let commit = cur["current"]["poe1"]["commit"].as_str().unwrap_or("");
            let base = format!("{POERE}/generated/poe1/{patch}");
            let mods = get(&format!("{base}/mapmods/Generated.Map.ENGLISH.min.json?v={commit}"))?;
            let ids = get(&format!("{base}/trade-ids/Generated.TradeIds.Map.min.json?v={commit}"))?;
            if mods["tokens"].as_array().map(|a| a.is_empty()).unwrap_or(true) { return Err("empty".into()); }
            std::fs::write(&mods_path, mods.to_string()).map_err(err)?;
            std::fs::write(&ids_path, ids.to_string()).map_err(err)?;
            let _ = std::fs::write(state.data_dir.join("poere_patch.txt"), patch);
            Ok(())
        };
        let _ = download();
    }
    let read = |p: &Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
    let (mods, src) = match (read(&mods_path), read(&ids_path)) {
        (Some(m), Some(i)) => {
            let patch = std::fs::read_to_string(state.data_dir.join("poere_patch.txt")).unwrap_or_default();
            (filter_core::mapregex::load(&m, &i), format!("poe.re data, patch {}", patch.trim()))
        }
        _ => { let (m, i) = filter_core::mapregex::bundled(); (filter_core::mapregex::load(&m, &i), "poe.re data (built-in copy)".into()) }
    };
    *CACHE.lock().unwrap() = Some((std::time::Instant::now(), mods.clone(), src.clone()));
    (mods, src)
}

#[derive(Serialize)]
struct MapToolState { mods: Vec<filter_core::mapregex::MapModEntry>, bad: Vec<i64>, wants: filter_core::mapregex::MapWants, source: String }

/// The map tool page: every map mod, which you can't run, and what you want.
#[tauri::command]
async fn map_tool_get(app: AppHandle) -> Result<MapToolState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (mods, source) = map_mods(&state);
        let s = state.store.lock().unwrap().settings.clone();
        let bad = s.map_tool.bad.clone().unwrap_or_else(|| filter_core::mapregex::default_bad(&mods, s.map_avoid.as_deref()));
        MapToolState { mods, bad, wants: s.map_tool.wants, source }
    }).await.map_err(err)
}

#[tauri::command]
fn map_tool_save(state: State<AppState>, bad: Vec<i64>, wants: filter_core::mapregex::MapWants) -> Result<(), String> {
    let mut s = state.store.lock().unwrap();
    s.settings.map_tool = filter_core::mapregex::MapTool { bad: Some(bad), wants };
    s.save(&state.store_path).map_err(err)
}

#[tauri::command]
fn map_tool_regex(state: State<AppState>, bad: Vec<i64>, wants: filter_core::mapregex::MapWants) -> String {
    filter_core::mapregex::regex(&map_mods(&state).0, &bad, &wants)
}

/// Your saved map regex on the clipboard (from the Ctrl+D map window).
#[tauri::command]
fn map_regex_copy(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let (mods, _) = map_mods(&state);
    let s = state.store.lock().unwrap().settings.clone();
    let bad = s.map_tool.bad.clone().unwrap_or_else(|| filter_core::mapregex::default_bad(&mods, s.map_avoid.as_deref()));
    let r = filter_core::mapregex::regex(&mods, &bad, &s.map_tool.wants);
    app.clipboard().write_text(r.clone()).map_err(err)?;
    Ok(r)
}

/// Maps for sale on the trade site without your bad mods and with your minimums.
#[tauri::command]
async fn map_tool_trade(app: AppHandle, bad: Vec<i64>, wants: filter_core::mapregex::MapWants) -> Result<TradeResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        *LAST_MANUAL.lock().unwrap() = Some(std::time::Instant::now());
        let state = app.state::<AppState>();
        let q = filter_core::mapregex::trade_query(&map_mods(&state).0, &bad, &wants);
        let (poe2, league) = trade_league(&app);
        let (id, ids, total) = trade_api::search(poe2, &league, &q)?;
        let fetched = trade_api::fetch(poe2, &ids, &id)?;
        let rates = currency_rates(&app);
        let listings = filter_core::trade::listings(&fetched, &rates);
        Ok(TradeResult { summary: filter_core::trade::summary(&listings), total, url: trade_api::site_url(poe2, &league, &id),
            divine: rates.get("divine").copied(), listings })
    }).await.map_err(err)?
}

/// Every map mod the map check knows (for Settings).
#[tauri::command]
fn map_dangers() -> Vec<&'static str> {
    filter_core::maps::danger_names()
}

/// Map mods to warn about, suggested from your imported PoB build.
#[tauri::command]
fn map_suggest(state: State<AppState>) -> Result<Vec<filter_core::maps::Suggestion>, String> {
    let pob = pob_load(&state).ok_or("Import your build on the My build page first")?;
    Ok(filter_core::maps::suggest(&pob.build))
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(err)
}

#[tauri::command]
fn hide_trade_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("trade") { let _ = w.hide(); }
}

/// Is Path of Exile the window in front? We only ever type into the game.
#[cfg(windows)]
fn game_in_front() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};
    unsafe {
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(GetForegroundWindow(), &mut buf).max(0) as usize;
        String::from_utf16_lossy(&buf[..n]).starts_with("Path of Exile")
    }
}
#[cfg(not(windows))]
fn game_in_front() -> bool { false }

/// The filter name the game knows ("League Start" for "League Start.filter").
fn active_filter_name(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let store = state.store.lock().unwrap();
    let p = store.active.as_ref().and_then(|id| store.profiles.get(id))?;
    Some(p.output_file.trim_end_matches(".filter").to_string())
}

/// Types "/itemfilter <name>" into the game's chat: one keypress, one command.
fn reload_in_game(app: &AppHandle, wait_for_focus_ms: u64) -> Result<(), String> {
    let name = active_filter_name(app).ok_or("Set up a filter first")?;
    type_in_chat(&format!("/itemfilter {name}"), wait_for_focus_ms)
}

/// Types one line into the game's chat and sends it (one command per key or button
/// press, as GGG's rules ask). Only ever types into the game.
fn type_in_chat(line: &str, wait_for_focus_ms: u64) -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    // Give focus a moment to return to the game (e.g. after the pop-up closes).
    let mut waited = 0;
    while !game_in_front() {
        if waited >= wait_for_focus_ms {
            return Err("Path of Exile isn't the window in front, so nothing was typed".into());
        }
        thread::sleep(Duration::from_millis(50));
        waited += 50;
    }
    let mut e = Enigo::new(&Settings::default()).map_err(err)?;
    let mut step = || -> enigo::InputResult<()> {
        e.key(Key::Return, Direction::Click)?;            // open chat
        thread::sleep(Duration::from_millis(40));
        e.key(Key::Control, Direction::Press)?;           // clear anything already typed
        e.key(Key::Unicode('a'), Direction::Click)?;
        e.key(Key::Control, Direction::Release)?;
        e.key(Key::Backspace, Direction::Click)?;
        e.text(line)?;
        thread::sleep(Duration::from_millis(30));
        e.key(Key::Return, Direction::Click)                // send
    };
    step().map_err(err)
}

/// Brings the game window to the front (after you click a button in one of our windows).
#[cfg(windows)]
fn focus_game() -> bool {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    unsafe {
        let found = FindWindowW(w!("POEWindowClass"), PCWSTR::null())
            .or_else(|_| FindWindowW(PCWSTR::null(), w!("Path of Exile")))
            .or_else(|_| FindWindowW(PCWSTR::null(), w!("Path of Exile 2")));
        match found {
            Ok(h) if !h.is_invalid() => SetForegroundWindow(h).as_bool(),
            _ => false,
        }
    }
}
#[cfg(not(windows))]
fn focus_game() -> bool { false }

// ---------- trade helper (incoming whispers) ----------

/// Watches the game's log for trade whispers and buyers arriving, and shows them
/// in the "trades" window (without taking focus from the game).
fn start_whisper_watcher(app: AppHandle) {
    use std::io::{Read, Seek, SeekFrom};
    thread::spawn(move || {
        let mut log: Option<(PathBuf, u64)> = None;
        let mut last_look = std::time::Instant::now().checked_sub(Duration::from_secs(60)).unwrap_or_else(std::time::Instant::now);
        let mut partial = String::new();
        loop {
            thread::sleep(Duration::from_millis(400));
            let state = app.state::<AppState>();
            if !state.store.lock().unwrap().settings.trade_helper { continue; }
            if log.is_none() {
                if last_look.elapsed() < Duration::from_secs(30) { continue; }
                last_look = std::time::Instant::now();
                let poe2 = active_game(&state) == Game::Poe2;
                // Start at the end: only new whispers.
                log = find_log(poe2, &[]).and_then(|p| std::fs::metadata(&p).ok().map(|m| (p, m.len())));
                continue;
            }
            let Some((path, offset)) = log.as_mut() else { continue };
            let Ok(len) = std::fs::metadata(&*path).map(|m| m.len()) else { log = None; continue };
            if len < *offset { *offset = len; continue; } // log was cleared
            if len == *offset { continue; }
            let Ok(mut f) = std::fs::File::open(&*path) else { continue };
            if f.seek(SeekFrom::Start(*offset)).is_err() { continue; }
            let mut buf = Vec::new();
            if f.take(4 * 1024 * 1024).read_to_end(&mut buf).is_err() { continue; }
            *offset += buf.len() as u64;
            partial.push_str(&String::from_utf8_lossy(&buf));
            // Keep a half-written last line for next time.
            let keep = match partial.rfind('\n') { Some(i) => partial.split_off(i + 1), None => continue };
            let lines = std::mem::replace(&mut partial, keep);
            for line in lines.lines() {
                let Some(ev) = filter_core::whispers::parse_line(line) else { continue };
                let is_trade = matches!(&ev, filter_core::whispers::LogEvent::Whisper(w) if w.item.is_some());
                let Some(w) = app.get_webview_window("trades") else { continue };
                let _ = w.emit("trade-event", &ev);
                if is_trade && !w.is_visible().unwrap_or(false) {
                    let front = game_in_front();
                    let _ = w.show();
                    if front { focus_game(); } // never steal focus from the game
                }
            }
        }
    });
}

/// A trade button: Invite / Trade / Kick / Thanks. Brings the game to the front and
/// types that one command.
#[tauri::command]
async fn trade_action(app: AppHandle, action: String, name: String) -> Result<(), String> {
    let thanks = app.state::<AppState>().store.lock().unwrap().settings.thanks_message.clone();
    let line = filter_core::whispers::command(&action, &name, &thanks).ok_or("Unknown button")?;
    tauri::async_runtime::spawn_blocking(move || {
        focus_game();
        type_in_chat(&line, 1500)
    }).await.map_err(err)?
}

#[tauri::command]
fn hide_trades_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("trades") { let _ = w.hide(); }
}

#[tauri::command]
fn reload_filter(app: AppHandle) -> Result<(), String> {
    // Called right after the pop-up hides: wait for the game to be in front.
    reload_in_game(&app, 1500)
}

/// While batch mode is on, every item you Ctrl+C in game is sent to the list.
fn start_batch_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut last = String::new();
        loop {
            thread::sleep(Duration::from_millis(150));
            if !BATCH_ON.load(Ordering::Relaxed) {
                continue;
            }
            let text = app.clipboard().read_text().unwrap_or_default();
            if text == last {
                continue;
            }
            last = text.clone();
            if let Some(item) = read_item(&app, &text) {
                let _ = app.emit("batch-item", &item);
            }
        }
    });
}

// ---------- commands the screens call ----------

#[derive(Serialize)]
struct Overview {
    profiles: Vec<ProfileInfo>,
    active: Option<String>,
    poe1_dir: String,
    poe2_dir: String,
    batch_on: bool,
}

#[derive(Serialize)]
struct ProfileInfo {
    id: String,
    name: String,
    game: Game,
    base_file: String,
    output_file: String,
    parent: Option<String>,
    rules: Vec<RuleInfo>,
    theme: Option<String>,
    strictness: Option<u8>,
    base_version: Option<String>,
}

#[derive(Serialize)]
struct RuleInfo {
    index: usize,
    label: String,
    /// What it matches and how it looks (for grouping and previews).
    detail: Option<RuleDetail>,
    /// A newer edit always wins over this one, so it never applies.
    shadowed_by: Option<String>,
}

fn rule_label(r: &Rule) -> String {
    match r {
        Rule::Override { label, .. } => label.clone(),
        Rule::MoveBase { type_path, base, to_tier } if to_tier.is_empty() => {
            format!("Tier: {base} taken out of {}", type_path.join(" > "))
        }
        Rule::MoveBase { type_path, base, to_tier } => {
            format!("Tier: {base} moved to {} {}", type_path.join(" > "), filter_core::tiers::tier_name(to_tier))
        }
        Rule::Tier { type_path, tier, show, actions, min_stack, min_item_tier } => {
            let what = match (show, actions, min_stack) {
                (Some(false), ..) => "hidden".to_string(),
                (_, Some(_), _) => "restyled".to_string(),
                (_, _, Some(n)) => format!("set to stacks of {n}+"),
                _ if min_item_tier.is_some() => format!("set to item tier {}+", min_item_tier.unwrap_or(0)),
                (Some(true), ..) => "shown".to_string(),
                _ => "changed".to_string(),
            };
            format!("Tier: {} {} {what}", type_path.join(" > "), filter_core::tiers::tier_name(tier))
        }
    }
}

/// NeverSink's tier lists for a profile, as they'll be in game (your tier edits applied).
#[tauri::command]
fn get_tiers(state: State<AppState>, id: String) -> Result<Vec<filter_core::tiers::Section>, String> {
    let store = state.store.lock().unwrap();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?;
    Ok(filter_core::tiers::sections(&parse_filter(&text)))
}

/// One of NeverSink's rules that isn't a tier list ("Modify rule" on e.g. white 4-links).
#[tauri::command]
fn get_rule_section(state: State<AppState>, id: String, type_path: Vec<String>) -> Result<Option<filter_core::tiers::Section>, String> {
    let store = state.store.lock().unwrap();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?;
    Ok(filter_core::tiers::section_at(&parse_filter(&text), &type_path))
}

#[derive(serde::Serialize)]
struct GearSetup {
    /// From the game's log: (name, class, level).
    character: Option<(String, String, u32)>,
    stats: Vec<filter_core::gear::StatInfo>,
    /// Gem name -> the stats it likes.
    gems: std::collections::BTreeMap<String, Vec<String>>,
    league: String,
}

/// Levelling gear helper: your character (from the log), the stat list and gems.
#[tauri::command]
fn gear_setup(state: State<AppState>) -> GearSetup {
    let character = find_log(false, &[]).and_then(|p| filter_core::area::read_character(&p));
    let league = state.prices.lock().unwrap().get(&Game::Poe1).map(|f| f.league.clone()).unwrap_or_else(|| "Standard".into());
    GearSetup {
        character,
        stats: filter_core::gear::stat_list().clone(),
        gems: filter_core::gear::gems().iter().map(|(k, v)| (k.clone(), v.stats.clone())).collect(),
        league,
    }
}

#[derive(serde::Serialize)]
struct GearPick {
    #[serde(flatten)]
    pick: filter_core::gear::Pick,
    trade: String,
    /// poe.ninja price in chaos, when known.
    price: Option<f64>,
}

#[tauri::command]
fn gear_picks(state: State<AppState>, level: u32, ahead: u32, wanted: Vec<String>, skill: Vec<String>, classes: Vec<String>) -> Vec<GearPick> {
    let prices = state.prices.lock().unwrap();
    let file = prices.get(&Game::Poe1);
    let league = file.map(|f| f.league.clone()).unwrap_or_else(|| "Standard".into());
    filter_core::gear::picks(level, ahead, &wanted, &skill, &classes).into_iter().map(|p| {
        let price = file.and_then(|f| f.items.iter().filter(|i| i.name == p.unique.name && i.base == p.unique.base)
            .map(|i| i.value).reduce(f64::min));
        GearPick { trade: filter_core::gear::trade_url(&league, &p.unique.name, &p.unique.base), price, pick: p }
    }).collect()
}

/// Opens a web page (trade links) in your normal browser.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") { return Err("Only web links".into()); }
    #[cfg(windows)]
    let r = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(err)
}

// ---------- Path of Building ----------

fn pob_file(state: &AppState) -> PathBuf {
    state.store_path.with_file_name("pob.txt")
}

#[derive(serde::Serialize)]
struct PobState { build: filter_core::pob::Build, stage: usize, link: String, follow: bool }

fn pob_load(state: &AppState) -> Option<PobState> {
    let code = std::fs::read_to_string(pob_file(state)).ok()?;
    let build = filter_core::pob::parse(&filter_core::pob::decode(&code).ok()?).ok()?;
    let s = state.store.lock().unwrap();
    let stage = s.settings.pob_stage.unwrap_or(build.active).min(build.stages.len().saturating_sub(1));
    Some(PobState { build, stage, link: s.settings.pob_link.clone(), follow: s.settings.pob_follow })
}

/// Imports a PoB link (pobb.in, pastebin, poe.ninja) or a pasted code, and remembers it.
#[tauri::command]
fn pob_import(app: AppHandle, state: State<AppState>, link: String) -> Result<PobState, String> {
    let code = match filter_core::pob::raw_url(&link) {
        Some(url) => ureq::get(&url).set("User-Agent", "poe-filter-editor").timeout(Duration::from_secs(20)).call()
            .map_err(|e| format!("Couldn't download that build: {e}"))?
            .into_string().map_err(err)?,
        None => link.trim().to_string(),
    };
    let build = filter_core::pob::parse(&filter_core::pob::decode(&code)?)?;
    std::fs::write(pob_file(&state), code.trim()).map_err(err)?;
    {
        let mut s = state.store.lock().unwrap();
        s.settings.pob_link = if filter_core::pob::raw_url(&link).is_some() { link.trim().to_string() } else { "(pasted code)".into() };
        s.settings.pob_stage = Some(build.active);
        s.save(&state.store_path).map_err(err)?;
    }
    let _ = app.emit("pob-imported", ());
    pob_load(&state).ok_or_else(|| "Couldn't read the build back".into())
}

#[tauri::command]
fn pob_get(state: State<AppState>) -> Option<PobState> {
    pob_load(&state)
}

/// "Follow my game": move to the stage for your current act. Returns the stage shown.
#[tauri::command]
fn pob_follow_now(app: AppHandle, state: State<AppState>) -> Option<usize> {
    let p = pob_load(&state)?;
    if !p.follow { return Some(p.stage); }
    let lvl = area_level(&app)?;
    let want = filter_core::pob::stage_for(&p.build.stages, lvl)?;
    if want != p.stage {
        let mut s = state.store.lock().unwrap();
        s.settings.pob_stage = Some(want);
        let _ = s.save(&state.store_path);
    }
    Some(want)
}

#[tauri::command]
fn pob_set_follow(app: AppHandle, state: State<AppState>, on: bool) -> Result<(), String> {
    {
        let mut s = state.store.lock().unwrap();
        s.settings.pob_follow = on;
        s.save(&state.store_path).map_err(err)?;
    }
    if on { pob_follow_now(app.clone(), state); }
    let _ = app.emit("pob-stage", ());
    Ok(())
}

/// Picking a stage yourself stops following the game.
#[tauri::command]
fn pob_set_stage(app: AppHandle, state: State<AppState>, stage: usize) -> Result<(), String> {
    let mut s = state.store.lock().unwrap();
    s.settings.pob_stage = Some(stage);
    s.settings.pob_follow = false;
    s.save(&state.store_path).map_err(err)?;
    let _ = app.emit("pob-stage", stage);
    Ok(())
}

/// "Modify rule" in the quick menu: bring the main app forward on the page for that rule.
#[tauri::command]
fn open_rule(app: AppHandle, source: serde_json::Value) {
    if let Some(o) = app.get_webview_window("overlay") { let _ = o.hide(); }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        let _ = w.emit("goto-rule", source);
    }
}

/// F6: show/hide the build window over the game.
fn toggle_build_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("build") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.emit("build-shown", ());
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[tauri::command]
fn hide_build_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("build") { let _ = w.hide(); }
}

/// In-game panel: the stack tiers a currency can go in, and where it is now.
#[tauri::command]
fn stack_choices(app: AppHandle, base: String) -> Result<Vec<filter_core::tiers::StackChoice>, String> {
    let (live, _) = active_filters(&app).ok_or("Set up a filter first")?;
    Ok(filter_core::tiers::stack_choices(&live, &base))
}

/// Puts a currency's stacks into a tier of a stack section ("" = take it out), for the active filter.
#[tauri::command]
fn set_stack_tier(state: State<AppState>, type_path: Vec<String>, base: String, tier: String) -> Result<Vec<String>, String> {
    let id = state.store.lock().unwrap().active.clone().ok_or("Set up a filter first")?;
    move_tier_item(state, id, type_path, base, tier)
}

/// Drag & drop: move an item into another tier of the same section.
#[tauri::command]
fn move_tier_item(state: State<AppState>, id: String, type_path: Vec<String>, base: String, to_tier: String) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("Moved {base} to {}", filter_core::tiers::tier_name(&to_tier)));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    p.rules.retain(|r| !matches!(r, Rule::MoveBase { type_path: tp, base: b, .. } if *tp == type_path && *b == base));
    p.rules.push(Rule::MoveBase { type_path, base, to_tier });
    save_and_build(&state, &store, &id)
}

/// Show/hide a whole tier, give it a new look, or put it back (reset).
#[tauri::command]
fn set_tier(state: State<AppState>, id: String, type_path: Vec<String>, tier: String, show: Option<bool>,
            style: Option<CustomStyle>, min_stack: Option<u32>, min_item_tier: Option<u32>, reset: bool) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("Tier {}", filter_core::tiers::tier_name(&tier)));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    let same = |r: &Rule| matches!(r, Rule::Tier { type_path: tp, tier: t, .. } if *tp == type_path && *t == tier);
    let old = p.rules.iter().find(|r| same(r)).cloned();
    p.rules.retain(|r| !same(r));
    if reset {
        // Also undo items dragged into this tier.
        p.rules.retain(|r| !matches!(r, Rule::MoveBase { type_path: tp, to_tier, .. } if *tp == type_path && *to_tier == tier));
    } else {
        let (old_show, old_actions, old_stack, old_it) = match old {
            Some(Rule::Tier { show, actions, min_stack, min_item_tier, .. }) => (show, actions, min_stack, min_item_tier), _ => (None, None, None, None) };
        p.rules.push(Rule::Tier { type_path, tier, show: show.or(old_show),
            actions: style.map(|s| filter_core::rules::style_actions(&s)).or(old_actions), min_stack: min_stack.or(old_stack),
            min_item_tier: min_item_tier.or(old_it) });
    }
    save_and_build(&state, &store, &id)
}

#[tauri::command]
fn overview(state: State<AppState>) -> Overview {
    let store = state.store.lock().unwrap();
    Overview {
        profiles: store
            .profiles
            .values()
            .map(|p| ProfileInfo {
                id: p.id.clone(),
                name: p.name.clone(),
                game: p.game,
                base_file: p.base_file.clone(),
                output_file: p.output_file.clone(),
                parent: p.parent.clone(),
                theme: p.theme.clone(),
                strictness: p.strictness,
                base_version: {
                    use std::io::Read;
                    let mut head = String::new();
                    std::fs::File::open(filter_dir(p.game).join(&p.base_file)).ok()
                        .and_then(|f| f.take(4096).read_to_string(&mut head).ok());
                    neversink::version_of(&head)
                },
                rules: {
                    let sh = shadowed_by(&p.rules);
                    p.rules.iter().enumerate().map(|(index, r)| RuleInfo {
                        index,
                        label: rule_label(r),
                        detail: rule_detail(r),
                        shadowed_by: sh[index].map(|j| rule_label(&p.rules[j])),
                    }).collect()
                },
            })
            .collect(),
        active: store.active.clone(),
        poe1_dir: filter_dir(Game::Poe1).display().to_string(),
        poe2_dir: filter_dir(Game::Poe2).display().to_string(),
        batch_on: BATCH_ON.load(Ordering::Relaxed),
    }
}

/// Saves, rebuilds the profile's filter, and returns any warnings.
fn save_and_build(state: &AppState, store: &Store, id: &str) -> Result<Vec<String>, String> {
    store.save(&state.store_path).map_err(err)?;
    let game = store.profiles.get(id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    build_with_prices(store, id, &filter_dir(game), prices.get(&game)).map_err(err)
}

/// A filter file found in the game's folder (for first-time setup).
#[derive(Serialize)]
struct FoundFilter { name: String, path: String, modified: u64, neversink: bool, version: Option<String> }

#[tauri::command]
fn list_filters(state: State<AppState>, game: Game) -> Vec<FoundFilter> {
    let ours: Vec<String> = state.store.lock().unwrap().profiles.values().map(|p| p.output_file.to_lowercase()).collect();
    let mut v: Vec<FoundFilter> = std::fs::read_dir(filter_dir(game)).into_iter().flatten().filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".filter"))
        .filter(|e| !ours.contains(&e.file_name().to_string_lossy().to_lowercase()))
        .map(|e| {
            use std::io::Read;
            let mut head = String::new();
            std::fs::File::open(e.path()).ok().and_then(|f| f.take(4096).read_to_string(&mut head).ok());
            FoundFilter {
                name: e.file_name().to_string_lossy().to_string(),
                path: e.path().display().to_string(),
                modified: e.metadata().and_then(|m| m.modified()).ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0),
                neversink: head.contains("NeverSink"),
                version: neversink::version_of(&head),
            }
        })
        .collect();
    v.sort_by(|a, b| (b.neversink, b.modified).cmp(&(a.neversink, a.modified)));
    v
}

#[tauri::command]
fn create_profile(state: State<AppState>, name: String, game: Game, base_path: Option<String>, strictness: Option<u8>) -> Result<String, String> {
    let dir = filter_dir(game);
    let file_name = match (strictness, base_path) {
        (Some(level), _) => download_neversink(&state, game, level.min(6))?.0.display().to_string(),
        (None, Some(base_path)) => {
            let src = Path::new(&base_path);
            let file_name = src.file_name().ok_or("That isn't a file")?.to_string_lossy().to_string();
            // Keep the base next to the game's filters so rebuilds always find it.
            if src.parent() != Some(dir.as_path()) {
                std::fs::create_dir_all(&dir).map_err(err)?;
                std::fs::copy(src, dir.join(&file_name)).map_err(err)?;
            }
            file_name
        }
        _ => return Err("Pick a filter to start from".into()),
    };
    let mut store = state.store.lock().unwrap();
    if store.profiles.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("You already have a profile called {name}"));
    }
    let id = store.create(&name, game, &file_name);
    if let Some(p) = store.profiles.get_mut(&id) { p.strictness = strictness.map(|l| l.min(6)); }
    store.active = Some(id.clone());
    save_and_build(&state, &store, &id)?;
    Ok(id)
}

#[tauri::command]
fn set_active(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    if !store.profiles.contains_key(&id) {
        return Err("Profile not found".into());
    }
    store.active = Some(id);
    store.save(&state.store_path).map_err(err)
}

#[tauri::command]
fn fork_profile(state: State<AppState>, from: String, name: String, inherit: bool) -> Result<String, String> {
    let mut store = state.store.lock().unwrap();
    if store.profiles.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("You already have a profile called {name}"));
    }
    let id = store.fork(&from, &name, inherit).ok_or("Profile not found")?;
    store.active = Some(id.clone());
    save_and_build(&state, &store, &id)?;
    Ok(id)
}

#[tauri::command]
fn delete_profile(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    if store.profiles.values().any(|p| p.parent.as_deref() == Some(id.as_str())) {
        return Err("Another profile builds on this one. Delete or detach that first.".into());
    }
    store.profiles.remove(&id);
    if store.active.as_deref() == Some(id.as_str()) {
        store.active = store.profiles.keys().next().cloned();
    }
    store.save(&state.store_path).map_err(err)
}

#[derive(Serialize)]
struct ActionResult {
    label: String,
    warnings: Vec<String>,
}

#[tauri::command]
fn apply_quick_action(
    state: State<AppState>,
    item: Item,
    action: Action,
    colour: Option<String>,
    opts: Option<MatchOpts>,
) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    let rule = quick_rule(&item, action, colour.as_deref(), &opts.unwrap_or_default());
    let label = rule_label(&rule);
    remember(&state, &store, &id, &label);
    add_rule(&mut store.profiles.get_mut(&id).ok_or("Profile not found")?.rules, rule);
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
}

/// Saves a full custom look (colours, beam, map icon, sound) for this item.
#[tauri::command]
fn apply_custom_style(state: State<AppState>, item: Item, style: CustomStyle, opts: Option<MatchOpts>) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    let rule = custom_rule(&item, &style, &opts.unwrap_or_default());
    let label = rule_label(&rule);
    remember(&state, &store, &id, &label);
    add_rule(&mut store.profiles.get_mut(&id).ok_or("Profile not found")?.rules, rule);
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
}

fn active_game(state: &AppState) -> Game {
    let store = state.store.lock().unwrap();
    store.active.as_ref().and_then(|id| store.profiles.get(id)).map(|p| p.game).unwrap_or(Game::Poe1)
}

/// Sound files you've put in the game's filter folder (usable as drop sounds).
#[tauri::command]
fn list_sounds(state: State<AppState>) -> Vec<String> {
    let dir = filter_dir(active_game(&state));
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| { let l = n.to_lowercase(); l.ends_with(".wav") || l.ends_with(".mp3") || l.ends_with(".ogg") })
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

/// A sound file as a data URL so the pop-up can preview it.
#[tauri::command]
fn sound_data(state: State<AppState>, file: String) -> Result<String, String> {
    use base64::Engine;
    let name = std::path::Path::new(&file).file_name().ok_or("Bad file name")?;
    let bytes = std::fs::read(filter_dir(active_game(&state)).join(name)).map_err(err)?;
    let mime = if file.to_lowercase().ends_with(".mp3") { "audio/mpeg" } else if file.to_lowercase().ends_with(".ogg") { "audio/ogg" } else { "audio/wav" };
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
fn undo_last(state: State<AppState>) -> Result<Option<String>, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    // Undo = go back to before the latest change (any kind of change).
    let back = {
        let mut h = state.history.lock().unwrap();
        let n = h.list(&id).len();
        if n == 0 { return Ok(None); }
        let what = h.list(&id)[n - 1].what.clone();
        let current = store.profiles.get(&id).ok_or("Profile not found")?.clone();
        let p = h.go_back(&current, n - 1);
        let _ = h.save(&state.history_path);
        p.map(|p| (p, what))
    };
    let Some((profile, what)) = back else { return Ok(None) };
    store.profiles.insert(id.clone(), profile);
    save_and_build(&state, &store, &id)?;
    Ok(Some(what))
}

#[derive(Serialize)]
struct HistoryItem { index: usize, at: u64, what: String }

/// Recent changes to a filter, newest first.
#[derive(Serialize)]
struct Preview { drops: Vec<PreviewDrop>, area_level: u32 }

/// How sample drops (and items you've edited) look with this filter. With
/// `theme` set, previews that theme without switching to it ("" = NeverSink's own colours).
#[tauri::command]
fn preview_drops(app: AppHandle, state: State<AppState>, id: String, theme: Option<String>, area_level: Option<u32>) -> Result<Preview, String> {
    let level = area_level.or_else(|| crate::area_level(&app)).unwrap_or_else(|| fallback_area_level(&app));
    let store = state.store.lock().unwrap();
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let game = p.game;
    let prices = state.prices.lock().unwrap();
    let theme_override = theme.as_deref().map(|t| if t.is_empty() { None } else { Some(t) });
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), theme_override).map_err(err)?;
    // Your edited items first, so you can see your own changes.
    let extra: Vec<Item> = p.rules.iter().filter_map(rule_detail).filter(|d| !d.base_type.is_empty()).map(|d| {
        let mut it = Item { item_class: d.item_class.clone(), base_type: d.base_type.clone(),
            rarity: d.rarity.clone().unwrap_or_else(|| "Currency".into()),
            stack_size: d.min_stack.or(d.below_stack.map(|b| b.saturating_sub(1).max(1))), ..Default::default() };
        it.unidentified = matches!(it.rarity.as_str(), "Magic" | "Rare" | "Unique");
        it
    }).collect();
    Ok(Preview { drops: preview(&parse_filter(&text), &extra, game, level), area_level: level })
}

/// Item pictures for the active filter's game: name -> picture URL.
#[tauri::command]
fn get_icons(state: State<AppState>) -> std::collections::HashMap<String, String> {
    let game = active_game(&state);
    state.icons.lock().unwrap().get(&game).cloned().unwrap_or_default()
}

#[tauri::command]
fn get_history(state: State<AppState>, id: String) -> Vec<HistoryItem> {
    let h = state.history.lock().unwrap();
    h.list(&id).iter().enumerate().rev().map(|(index, e)| HistoryItem { index, at: e.at, what: e.what.clone() }).collect()
}

/// Puts a filter back the way it was before change `index` (undoable).
#[tauri::command]
fn go_back(state: State<AppState>, id: String, index: usize) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    let current = store.profiles.get(&id).ok_or("Profile not found")?.clone();
    let back = {
        let mut h = state.history.lock().unwrap();
        let p = h.go_back(&current, index).ok_or("That change isn't in the history any more")?;
        let _ = h.save(&state.history_path);
        p
    };
    store.profiles.insert(id.clone(), back);
    save_and_build(&state, &store, &id)
}

#[tauri::command]
fn delete_rule(state: State<AppState>, id: String, index: usize) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    if let Some(r) = store.profiles.get(&id).and_then(|p| p.rules.get(index)) {
        let what = format!("Removed: {}", rule_label(r));
        remember(&state, &store, &id, &what);
    }
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    if index < p.rules.len() {
        p.rules.remove(index);
    }
    save_and_build(&state, &store, &id).map(|_| ())
}

/// Opens a saved edit for changing. With no index, finds the active
/// profile's newest edit with this label (used by the in-game pop-up).
#[tauri::command]
fn get_rule(state: State<AppState>, id: Option<String>, index: Option<usize>, label: Option<String>) -> Result<(String, usize, RuleDetail), String> {
    let store = state.store.lock().unwrap();
    let id = id.or_else(|| store.active.clone()).ok_or("Set up a profile first")?;
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let index = match (index, label) {
        (Some(i), _) => i,
        (None, Some(l)) => p.rules.iter().rposition(|r| rule_label(r) == l).ok_or("That edit lives in another profile")?,
        _ => return Err("Which edit?".into()),
    };
    let detail = p.rules.get(index).and_then(rule_detail).ok_or("This edit can't be opened")?;
    Ok((id, index, detail))
}

/// Saves changes to an edit, keeping its place in the list.
#[tauri::command]
fn update_rule(state: State<AppState>, id: String, index: usize, edit: RuleEdit) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let old = store.profiles.get(&id).ok_or("Profile not found")?.rules.get(index).ok_or("That edit no longer exists")?;
    let new = edited_rule(old, &edit).ok_or("This edit can't be changed")?;
    let label = rule_label(&new);
    remember(&state, &store, &id, &format!("Changed: {label}"));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    p.rules[index] = new;
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
}

#[tauri::command]
fn rebuild(state: State<AppState>, id: String) -> Result<Vec<String>, String> {
    let store = state.store.lock().unwrap();
    save_and_build(&state, &store, &id)
}

/// Where NeverSink currently places this item, e.g. "uniques > t2".
#[tauri::command]
fn locate_item(state: State<AppState>, item: Item) -> Vec<Location> {
    let store = state.store.lock().unwrap();
    let Some(p) = store.active.as_ref().and_then(|id| store.profiles.get(id)) else { return vec![] };
    std::fs::read_to_string(filter_dir(p.game).join(&p.base_file))
        .map(|base| locate(&parse_filter(&base), &item))
        .unwrap_or_default()
}

/// What the filter currently does to this item: shown or hidden, its look,
/// its sound, and whether stack size changes that.
#[tauri::command]
fn item_status(app: AppHandle, item: Item, from_screen: bool) -> Result<StatusReply, String> {
    let (live, _) = active_filters(&app).ok_or("Set up a filter first")?;
    let level = area_level(&app);
    let st = status_with_fallback(&live, &item, level, fallback_area_level(&app), from_screen);
    let stackable = is_stackable(&item) || !st.by_stack.is_empty();
    Ok(StatusReply { status: st, area_level: level, game_filter: game_filter(&app), stackable })
}

#[derive(Serialize)]
struct StatusReply { status: Status, area_level: Option<u32>, game_filter: Option<GameFilter>, stackable: bool }

#[derive(Serialize)]
struct PriceInfo { league: String, updated: String, unit: String, count: usize }

#[derive(Serialize)]
struct PricePage { settings: PriceSettings, info: Option<PriceInfo>, game: Game }

/// The Prices screen: this filter's tier settings and the latest price data.
#[tauri::command]
fn get_price_settings(state: State<AppState>, id: String) -> Result<PricePage, String> {
    let store = state.store.lock().unwrap();
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let prices = state.prices.lock().unwrap();
    let file = prices.get(&p.game);
    let unit = file.map(|f| f.unit.clone()).unwrap_or_else(|| if p.game == Game::Poe1 { "chaos".into() } else { "exalted".into() });
    Ok(PricePage {
        settings: p.prices.clone().unwrap_or_else(|| default_settings(&unit)),
        info: file.map(|f| PriceInfo { league: f.league_name.clone().unwrap_or_else(|| f.league.clone()), updated: f.updated.clone(), unit: f.unit.clone(), count: f.items.len() }),
        game: p.game,
    })
}

#[tauri::command]
fn set_price_settings(state: State<AppState>, id: String, settings: PriceSettings) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, if settings.enabled { "Price tiers changed" } else { "Price tiers switched off" });
    store.profiles.get_mut(&id).ok_or("Profile not found")?.prices = Some(settings);
    save_and_build(&state, &store, &id)
}

#[tauri::command]
fn refresh_prices(app: AppHandle) -> bool {
    let changed = fetch_prices(&app);
    rebuild_priced(&app, &changed);
    !changed.is_empty()
}

/// What poe.ninja says this item is worth (for the pop-up).
#[tauri::command]
fn item_price(state: State<AppState>, item: Item) -> Option<(f64, String)> {
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let f = prices.get(&game)?;
    price_of(f, item.name.as_deref(), &item.base_type, &item.rarity).map(|v| (v, f.unit.clone()))
}

#[derive(Serialize)]
struct ThemeList { builtin: Vec<Theme>, custom: Vec<Theme> }

#[tauri::command]
fn list_themes(state: State<AppState>) -> ThemeList {
    ThemeList { builtin: builtin_themes(), custom: state.store.lock().unwrap().themes.clone() }
}

/// Picks a theme for a filter (None = NeverSink's own look) and rebuilds it.
/// What a filter is built on: NeverSink at a strictness (downloaded and kept
/// up to date), or a file of your own (e.g. a FilterBlade download).
#[tauri::command]
fn set_base(state: State<AppState>, id: String, strictness: Option<u8>, file_path: Option<String>) -> Result<Vec<String>, String> {
    let game = state.store.lock().unwrap().profiles.get(&id).ok_or("Profile not found")?.game;
    let (base_file, level) = match (strictness, file_path) {
        (Some(level), _) => {
            let level = level.min(6);
            let (path, _, _) = download_neversink(&state, game, level)?;
            (path.display().to_string(), Some(level))
        }
        (None, Some(f)) => {
            let src = Path::new(&f);
            let dir = filter_dir(game);
            let name = src.file_name().ok_or("That isn't a file")?.to_string_lossy().to_string();
            if src.parent() != Some(dir.as_path()) {
                std::fs::create_dir_all(&dir).map_err(err)?;
                std::fs::copy(src, dir.join(&name)).map_err(err)?;
            }
            (name, None)
        }
        _ => return Err("Pick a strictness or a file".into()),
    };
    let mut store = state.store.lock().unwrap();
    let what = match level { Some(l) => format!("Built on NeverSink {}", neversink::STRICTNESS_NAMES[l as usize]), None => format!("Built on {base_file}") };
    remember(&state, &store, &id, &what);
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    p.base_file = base_file;
    p.strictness = level;
    save_and_build(&state, &store, &id)
}

#[tauri::command]
fn set_profile_theme(state: State<AppState>, id: String, name: Option<String>) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("Theme: {}", name.as_deref().unwrap_or("NeverSink's own colours")));
    store.profiles.get_mut(&id).ok_or("Profile not found")?.theme = name;
    save_and_build(&state, &store, &id)
}

/// Rebuilds every filter using one of these themes.
fn rebuild_using(state: &AppState, store: &Store, names: &[String]) {
    let ids: Vec<String> = store.profiles.values()
        .filter(|p| p.theme.as_ref().map(|t| names.contains(t)).unwrap_or(false)).map(|p| p.id.clone()).collect();
    for id in ids { let _ = save_and_build(state, store, &id); }
}

/// Saves one of your themes (new, or replacing `old_name`).
#[tauri::command]
fn save_theme(state: State<AppState>, theme: Theme, old_name: Option<String>) -> Result<(), String> {
    let name = theme.name.trim().to_string();
    if name.is_empty() { return Err("Give your theme a name".into()); }
    if builtin_themes().iter().any(|t| t.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("“{name}” is a built-in theme; pick another name"));
    }
    if theme.levels.len() != 6 { return Err("A theme needs all six levels".into()); }
    let mut store = state.store.lock().unwrap();
    let replacing = old_name.clone().unwrap_or_else(|| name.clone());
    if store.themes.iter().any(|t| t.name.eq_ignore_ascii_case(&name) && !t.name.eq_ignore_ascii_case(&replacing)) {
        return Err(format!("You already have a theme called “{name}”"));
    }
    store.themes.retain(|t| t.name != replacing);
    store.themes.push(Theme { name: name.clone(), ..theme });
    for p in store.profiles.values_mut() {
        if p.theme.as_deref() == Some(replacing.as_str()) { p.theme = Some(name.clone()); }
    }
    store.save(&state.store_path).map_err(err)?;
    rebuild_using(&state, &store, &[name]);
    Ok(())
}

#[tauri::command]
fn delete_theme(state: State<AppState>, name: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.themes.retain(|t| t.name != name);
    let ids: Vec<String> = store.profiles.values().filter(|p| p.theme.as_deref() == Some(name.as_str())).map(|p| p.id.clone()).collect();
    for id in &ids { if let Some(p) = store.profiles.get_mut(id) { p.theme = None; } }
    store.save(&state.store_path).map_err(err)?;
    for id in ids { let _ = save_and_build(&state, &store, &id); }
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.store.lock().unwrap().settings.clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, state: State<AppState>, settings: Settings) -> Result<(), String> {
    let old = state.store.lock().unwrap().settings.clone();
    if let Err(e) = register_hotkeys(&app, &settings) {
        let _ = register_hotkeys(&app, &old); // put the working keys back
        return Err(e);
    }
    let mut store = state.store.lock().unwrap();
    store.settings = Settings { fallback_area_level: settings.fallback_area_level.clamp(1, 86), ..settings };
    store.save(&state.store_path).map_err(err)
}

/// Asks Claude (with the player's own key) to read a request into a `Request`.
/// Returns None on any problem, so the offline reader takes over.
fn ask_claude(key: &str, text: &str) -> Option<Request> {
    let call = |forced: bool| -> Result<serde_json::Value, ureq::Error> {
        let mut tool = claude_tool();
        tool["strict"] = serde_json::json!(true);
        let mut body = serde_json::json!({
            "model": "claude-haiku-5-5",
            "max_tokens": 300,
            "system": "You turn a Path of Exile player's loot filter request into exactly one make_rule call. \
                Use the game's own names for items and item types (e.g. 'Two Hand Maces', 'Chaos Orb', 'Divination Cards', 'Gold'). \
                Default to highlight when they want something to stand out or be easier to see.",
            "tools": [tool],
            "messages": [{"role": "user", "content": text}],
        });
        if forced { body["tool_choice"] = serde_json::json!({"type": "tool", "name": "make_rule"}); }
        let resp = ureq::post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", key).set("anthropic-version", "2023-06-01").set("content-type", "application/json")
            .timeout(Duration::from_secs(20)).send_json(body)?;
        Ok(resp.into_json::<serde_json::Value>().unwrap_or_default())
    };
    // Some models refuse a forced tool; ask again without forcing if so.
    let reply = match call(true) {
        Ok(v) => v,
        Err(ureq::Error::Status(400, _)) => call(false).ok()?,
        Err(_) => return None,
    };
    let input = reply["content"].as_array()?.iter().find(|b| b["type"] == "tool_use")?["input"].clone();
    serde_json::from_value::<Request>(input).ok()
}

#[derive(Serialize)]
struct AskReply { plan: Option<Plan>, used_ai: bool }

/// "Show gold in a purple highlight" -> a plan to show before doing it.
#[tauri::command]
fn ask(app: AppHandle, state: State<AppState>, text: String) -> AskReply {
    let key = state.store.lock().unwrap().settings.claude_api_key.trim().to_string();
    let extra: std::collections::BTreeSet<String> = active_filters(&app)
        .map(|(_, base)| known_bases(&base).into_iter().collect()).unwrap_or_default();
    if !key.is_empty() {
        if let Some(p) = ask_claude(&key, &text).and_then(|r| plan(&r, &extra)) {
            return AskReply { plan: Some(p), used_ai: true };
        }
    }
    AskReply { plan: plan(&read_offline(&text), &extra), used_ai: false }
}

/// Search by everyday names ("2h mace", "ammy", "chaos").
#[tauri::command]
fn search_items(app: AppHandle, q: String) -> Vec<Found> {
    let extra: std::collections::BTreeSet<String> = active_filters(&app)
        .map(|(_, base)| known_bases(&base).into_iter().collect()).unwrap_or_default();
    search(&q, &extra, 40)
}

#[tauri::command]
fn list_farm_presets() -> Vec<Preset> {
    farm_presets()
}

/// Makes everything for a league mechanic stand out (replaces an earlier copy).
#[tauri::command]
fn apply_farm_preset(state: State<AppState>, id: String, style: CustomStyle) -> Result<Vec<String>, String> {
    let rules = farm_rules(&id, &style);
    if rules.is_empty() { return Err("Unknown preset".into()); }
    let mut store = state.store.lock().unwrap();
    let pid = store.active.clone().ok_or("Set up a profile first")?;
    remember(&state, &store, &pid, &format!("Preset: {id}"));
    let p = store.profiles.get_mut(&pid).ok_or("Profile not found")?;
    let labels: Vec<String> = rules.iter().map(rule_label).collect();
    p.rules.retain(|r| !labels.contains(&rule_label(r)));
    p.rules.extend(rules);
    save_and_build(&state, &store, &pid)
}

#[derive(serde::Serialize)]
struct LevelBuild { id: String, title: String, keep: Vec<String>, armour: Vec<String> }

#[derive(serde::Serialize)]
struct LevelState {
    builds: Vec<LevelBuild>,
    weapons: Vec<String>,
    armour_types: Vec<(String, String)>,
    vendor_ranges: Vec<(u32, u32)>,
    /// Your current choices, if a levelling setup is on.
    current: Option<LevelOptions>,
}

/// Levelling setup for the active profile's game (like FilterBlade's Auto-Adjust: Campaign).
#[tauri::command]
fn list_level_presets(state: State<AppState>) -> LevelState {
    let store = state.store.lock().unwrap();
    let p = store.active.as_ref().and_then(|id| store.profiles.get(id));
    let game = p.map(|p| p.game).unwrap_or(filter_core::profiles::Game::Poe1);
    let on = p.map(|p| p.rules.iter().any(|r| rule_label(r).starts_with(LEVEL_PREFIX))).unwrap_or(false);
    LevelState {
        builds: level_builds(game).into_iter().map(|(id, title)| {
            let (w, a) = build_defaults(id, game);
            LevelBuild { id: id.into(), title: title.into(), keep: w.iter().map(|s| s.to_string()).collect(), armour: a.iter().map(|s| s.to_string()).collect() }
        }).collect(),
        weapons: weapon_classes(game).iter().map(|c| c.to_string()).collect(),
        armour_types: ARMOUR_TYPES.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        vendor_ranges: VENDOR_RANGES.to_vec(),
        current: if on { p.and_then(|p| p.levelling.clone()) } else { None },
    }
}

/// Applies your levelling setup (None turns it off). Replaces any earlier one.
#[tauri::command]
fn apply_level_preset(state: State<AppState>, options: Option<LevelOptions>, style: CustomStyle) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    let pid = store.active.clone().ok_or("Set up a profile first")?;
    let game = store.profiles.get(&pid).ok_or("Profile not found")?.game;
    let rules = match &options { Some(o) => level_rules_opts(game, o, &style), None => vec![] };
    if options.is_some() && rules.is_empty() { return Err("Pick at least one weapon or armour type".into()); }
    remember(&state, &store, &pid, if options.is_some() { "Levelling setup" } else { "Levelling setup off" });
    let p = store.profiles.get_mut(&pid).ok_or("Profile not found")?;
    p.rules.retain(|r| !rule_label(r).starts_with(LEVEL_PREFIX));
    p.rules.extend(rules);
    p.levelling = options;
    save_and_build(&state, &store, &pid)
}

#[tauri::command]
fn set_batch_mode(app: AppHandle, on: bool) {
    if on {
        // Ignore whatever was already on the clipboard.
        let _ = app.clipboard().write_text(String::new());
    }
    BATCH_ON.store(on, Ordering::Relaxed);
    let _ = app.emit("batch-mode", on);
}

// ---------- startup ----------

/// Everything the app keeps, loaded before any window opens (so a window can never
/// ask for it too early). Same folder Tauri uses: <AppData>/com.grobbs.poefiltereditor.
fn load_state() -> AppState {
    let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("com.grobbs.poefiltereditor");
    let _ = std::fs::create_dir_all(&data);
    let store_path = data.join("profiles.json");
    // Start from the last downloaded prices, then keep them fresh.
    let mut cached = std::collections::HashMap::new();
    for g in [Game::Poe1, Game::Poe2] {
        if let Some(f) = std::fs::read_to_string(data.join(format!("prices_{}.json", game_key(g)))).ok()
            .and_then(|t| serde_json::from_str::<PriceFile>(&t).ok()) {
            cached.insert(g, f);
        }
    }
    let store = Store::load(&store_path);
    let history_path = data.join("history.json");
    let mut icons = std::collections::HashMap::new();
    for g in [Game::Poe1, Game::Poe2] {
        if let Some(m) = std::fs::read_to_string(data.join(format!("icons_{}.json", game_key(g)))).ok()
            .and_then(|t| serde_json::from_str::<std::collections::HashMap<String, String>>(&t).ok()) {
            icons.insert(g, m);
        }
    }
    AppState { store: Mutex::new(store), store_path, history: Mutex::new(History::load(&history_path)), history_path,
        icons: Mutex::new(icons), prices: Mutex::new(cached), saved: Mutex::new(filter_core::saved::Saved::load(&data.join("saved_searches.json"))),
        data_dir: data, hotkeys: Mutex::new(Default::default()) }
}

pub fn run() {
    tauri::Builder::default()
        .manage(load_state())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    let Some(state) = app.try_state::<AppState>() else { return };
                    let action = state.hotkeys.lock().unwrap().get(&sc.id()).copied();
                    if action == Some(HotAction::Cycle) {
                        let app = app.clone();
                        thread::spawn(move || cycle_filter(&app));
                    } else if action == Some(HotAction::QuickEdit) {
                        quick_edit(app.clone());
                    } else if action == Some(HotAction::Reload) {
                        let app = app.clone();
                        thread::spawn(move || {
                            if let Err(e) = reload_in_game(&app, 0) {
                                let _ = app.emit("reload-failed", e);
                            }
                        });
                    } else if action == Some(HotAction::Price) {
                        price_check(app.clone());
                    } else if let Some(HotAction::Chat(i)) = action {
                        // One key press, one chat command, and only when the game is in front.
                        let cmd = state.store.lock().unwrap().settings.chat_keys.get(i).map(|c| c.command.trim().to_string());
                        if let Some(cmd) = cmd {
                            let app = app.clone();
                            thread::spawn(move || {
                                if let Err(e) = type_in_chat(&cmd, 0) { let _ = app.emit("reload-failed", e); }
                            });
                        }
                    } else if action == Some(HotAction::Build) {
                        toggle_build_window(app);
                    } else if action == Some(HotAction::Batch) {
                        let on = !BATCH_ON.load(Ordering::Relaxed);
                        set_batch_mode(app.clone(), on);
                        if on {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let settings = app.state::<AppState>().store.lock().unwrap().settings.clone();
            start_price_updates(app.handle().clone());
            if register_hotkeys(app.handle(), &settings).is_err() {
                // Fall back to the standard keys if saved ones can't be used.
                let _ = register_hotkeys(app.handle(), &Settings::default());
            }
            start_batch_watcher(app.handle().clone());
            start_whisper_watcher(app.handle().clone());
            start_live_searches(app.handle().clone());
            wealth_app::start_session_timer(app.handle().clone());

            // Tray icon: the app lives here so hotkeys work while you play.
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            let open = MenuItem::with_id(app, "open", "Open editor", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("PoE Filter Editor")
                .menu(&menu)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window just hides it; the hotkeys keep working.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            create_profile,
            list_filters,
            set_active,
            fork_profile,
            delete_profile,
            apply_quick_action,
            apply_custom_style,
            reload_filter,
            list_sounds,
            sound_data,
            undo_last,
            delete_rule,
            rebuild,
            get_rule,
            update_rule,
            locate_item,
            item_status,
            set_batch_mode,
            get_price_settings,
            set_price_settings,
            refresh_prices,
            item_price,
            list_themes,
            set_profile_theme,
            set_base,
            save_theme,
            delete_theme,
            get_settings,
            save_settings,
            search_items,
            list_farm_presets,
            apply_farm_preset,
            list_level_presets,
            get_tiers, get_rule_section, trade_search, trade_bulk, rare_estimate, map_dangers, map_suggest, map_tool_get, map_tool_save, map_tool_regex, map_regex_copy, map_tool_trade, trade_action, hide_trades_window, wealth_app::wealth_get, wealth_app::wealth_setup, wealth_app::wealth_prefs, wealth_app::wealth_scan, wealth_app::session_start, wealth_app::session_check, wealth_app::session_pause, wealth_app::session_stop, wealth_app::session_delete, wealth_app::earnings_overlay, saved_list, saved_add, saved_delete, saved_live, send_whisper, copy_text, hide_trade_window, keep_on_screen,
            move_tier_item,
            stack_choices,
            gear_setup,
            pob_import,
            pob_get,
            pob_set_stage,
            pob_follow_now,
            pob_set_follow,
            hide_build_window,
            open_rule,
            gear_picks,
            open_url,
            set_stack_tier,
            set_tier,
            apply_level_preset,
            ask,
            get_history,
            get_icons,
            preview_drops,
            go_back
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}

#[cfg(test)]
mod tests {
    use super::Shortcut;
    #[test]
    fn key_names_parse() {
        for k in ["F2", "F5", "F12", "Ctrl+Shift+K", "Alt+Q", "Shift+F3"] {
            assert!(k.parse::<Shortcut>().is_ok(), "{k}");
        }
        assert_ne!("F2".parse::<Shortcut>().unwrap().id(), "F3".parse::<Shortcut>().unwrap().id());
        assert!("Banana".parse::<Shortcut>().is_err());
    }
}
