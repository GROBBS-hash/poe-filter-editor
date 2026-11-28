// The app (core logic lives in ../core; both are rebuilt by `tauri dev`).
mod screen;
mod trade_api;
mod wealth_app;
mod bulk_app;
mod atlas_app;
mod farm_app;
mod play_app;
mod splits_app;
mod merc_app;
mod worn_app;
mod tonight_app;
mod deaths_app;
mod farming_app;
mod bases_app;
mod league_app;
mod phone_app;
mod win_place;
mod updates;
mod price_tick;
mod price_search_app;

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
    prices::{chaos_per_unit, default_settings, price_of, PriceFile, PriceSettings},
    profiles::{build_with_prices, render_profile, Game, Settings, Store},
    themes::{builtin_themes, Theme},
    rules::{add_rule, shadowed_by, custom_rule, edited_rule, known_bases, locate, quick_rule, rule_detail, Action, CustomStyle, Location, MatchOpts, Rule, RuleDetail, RuleEdit, SavedLook},
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
enum HotAction { QuickEdit, Batch, Reload, Cycle, Build, Price, Atlas, Chat(usize), Play(&'static str), Wiki, Read, Find, Editor }

/// The in-game keys are only held while Path of Exile (or one of our windows over it) is in front, like Awakened
/// PoE Trade: elsewhere F5 refreshes the browser again (he asked 23 Nov). `KEYS_ON` = they're registered now.
static KEYS_ON: AtomicBool = AtomicBool::new(false);
/// Set while the focus watcher puts the keys back, so a key another program owns doesn't warn on every alt-tab.
static KEYS_QUIET: AtomicBool = AtomicBool::new(false);

/// Where the keys should work: the game, or one of our windows that sits over it (price window, map tracker,
/// F2 panel…), not the editor's main window and not any other program.
#[cfg(windows)]
fn keys_zone(app: &AppHandle) -> bool {
    game_in_front() || (ours_in_front() && !app.get_webview_window("main").is_some_and(|w| w.is_focused().unwrap_or(false)))
}
#[cfg(not(windows))]
fn keys_zone(_app: &AppHandle) -> bool { true }

/// Registers the keys from the saved settings when the game comes in front, and lets them go when it doesn't.
fn start_hotkey_focus_watcher(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(150));
        let want = keys_zone(&app);
        if want == KEYS_ON.load(Ordering::Relaxed) { continue; }
        if want {
            let s = app.state::<AppState>().store.lock().unwrap().settings.clone();
            KEYS_QUIET.store(true, Ordering::Relaxed);
            let _ = register_hotkeys_now(&app, &s);
            KEYS_QUIET.store(false, Ordering::Relaxed);
        } else {
            let _ = app.global_shortcut().unregister_all();
        }
        KEYS_ON.store(want, Ordering::Relaxed);
    });
}

/// (Re)registers hotkeys from settings (checking every key works), then lets them go again straight away if the
/// game isn't in front; the focus watcher takes them back when it is. Errors name the key that failed.
fn register_hotkeys(app: &AppHandle, s: &Settings) -> Result<(), String> {
    register_hotkeys_now(app, s)?;
    if keys_zone(app) { KEYS_ON.store(true, Ordering::Relaxed); }
    else { let _ = app.global_shortcut().unregister_all(); KEYS_ON.store(false, Ordering::Relaxed); }
    Ok(())
}

fn register_hotkeys_now(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let warn = |m: String| { if !KEYS_QUIET.load(Ordering::Relaxed) { let _ = app.emit("hotkey-warning", m); } };
    let mut wanted = vec![(s.key_quick_edit.as_str(), HotAction::QuickEdit), (s.key_batch.as_str(), HotAction::Batch),
        (s.key_reload.as_str(), HotAction::Reload), (s.key_build.as_str(), HotAction::Build)];
    if !s.key_price.trim().is_empty() { wanted.push((s.key_price.as_str(), HotAction::Price)); }
    if s.cycle_enabled { wanted.push((s.key_cycle.as_str(), HotAction::Cycle)); }
    for (i, c) in s.chat_keys.iter().enumerate() {
        if !c.key.trim().is_empty() && !c.command.trim().is_empty() { wanted.push((c.key.as_str(), HotAction::Chat(i))); }
    }
    // Last, so a clash skips the atlas key and keeps the older ones.
    if !s.key_atlas.trim().is_empty() { wanted.push((s.key_atlas.as_str(), HotAction::Atlas)); }
    // The play windows' keys (22 Nov): new, so like the atlas key a clash or a key another program owns skips just that one.
    for (key, label) in [(&s.key_level, "level"), (&s.key_tracker, "tracker"), (&s.key_searches, "searches"), (&s.key_sheet, "sheet"), (&s.key_notes, "notes"), (&s.key_splits, "splits")] {
        if !key.trim().is_empty() { wanted.push((key.as_str(), HotAction::Play(label))); }
    }
    if !s.key_wiki.trim().is_empty() { wanted.push((s.key_wiki.as_str(), HotAction::Wiki)); }
    if !s.key_read.trim().is_empty() { wanted.push((s.key_read.as_str(), HotAction::Read)); }
    if !s.key_find.trim().is_empty() { wanted.push((s.key_find.as_str(), HotAction::Find)); }
    if !s.key_editor.trim().is_empty() { wanted.push((s.key_editor.as_str(), HotAction::Editor)); }
    let mut parsed = Vec::new();
    for (key, action) in wanted {
        let soft = matches!(action, HotAction::Atlas | HotAction::Play(_) | HotAction::Wiki | HotAction::Read | HotAction::Find | HotAction::Editor);
        let sc: Shortcut = match key.parse() {
            Ok(sc) => sc,
            Err(_) if soft => { let _ = warn(format!("“{key}” isn't a key the app understands: pick another in Settings")); continue; }
            Err(_) => return Err(format!("“{key}” isn't a key the app understands")),
        };
        if parsed.iter().any(|(p, _): &(Shortcut, HotAction)| p.id() == sc.id()) {
            // The newer keys: if one clashes with a key he already uses, skip it rather than lose his other keys.
            if soft {
                let _ = warn(format!("The key “{key}” is set for two things: pick another in Settings → Hotkeys"));
                continue;
            }
            return Err(format!("“{key}” is used for two things"));
        }
        parsed.push((sc, action));
    }
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let state = app.state::<AppState>();
    // Never hold the hotkey list while registering: `register` hands the job to the window thread and waits for it,
    // and a key press on that thread reads this list. Holding it froze the editor (24 Nov, "app crashed": a key
    // pressed while the game came back to the front, so the focus watcher was re-registering).
    let mut map = std::collections::HashMap::new();
    let result = (|| {
    for (sc, action) in parsed {
        match gs.register(sc) {
            Ok(()) => { map.insert(sc.id(), action); }
            // Another price checker (e.g. Awakened PoE Trade) may own Ctrl+D: carry on without it.
            Err(e) if action == HotAction::Price => {
                let _ = warn(format!("Price check key unavailable (another program may own it): {e}"));
            }
            Err(e) if action == HotAction::Atlas => {
                let _ = warn(format!("Atlas strategy key unavailable (another program may own it): {e}"));
            }
            Err(e) if matches!(action, HotAction::Play(_) | HotAction::Wiki | HotAction::Read | HotAction::Find | HotAction::Editor) => {
                let _ = warn(format!("A play window key is unavailable (another program may own it): {e}"));
            }
            Err(e) if matches!(action, HotAction::Chat(_)) => {
                let _ = warn(format!("A chat hotkey is unavailable (another program may own it): {e}"));
            }
            Err(e) => return Err(format!("Couldn't use that key (another program may own it): {e}")),
        }
    }
    Ok(())
    })();
    *state.hotkeys.lock().unwrap() = map;
    result
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
    league_app::refresh(app);
    for g in [Game::Poe1, Game::Poe2] {
        let url = format!("{PRICE_URL}/{}", league_app::price_file(app, g));
        let Ok(resp) = ureq::get(&url).set("User-Agent", "poe-filter-editor").timeout(Duration::from_secs(20)).call() else { continue };
        let Ok(body) = resp.into_string() else { continue };
        let Ok(file) = serde_json::from_str::<PriceFile>(&body) else { continue };
        let mut prices = state.prices.lock().unwrap();
        let no_icons = state.icons.lock().unwrap().get(&g).is_none();
        let new_prices = prices.get(&g).map(|p| p.updated != file.updated || p.league != file.league).unwrap_or(true);
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

/// Filters rebuilt because an app update changed how they're built (their names, for the main window's note).
static REBUILT: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Once, shortly after the start (the first price check is done by then): filters an older version built are rebuilt.
fn start_rebuild_old(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(12));
        let state = app.state::<AppState>();
        let store = (*state.store.lock().unwrap()).clone();
        let mut names = vec![];
        for (id, p) in &store.profiles {
            let prices = state.prices.lock().unwrap().get(&p.game).cloned();
            if filter_core::profiles::out_of_date(&store, id, &filter_dir(p.game), prices.as_ref())
                && build_with_prices(&store, id, &filter_dir(p.game), prices.as_ref()).is_ok() { names.push(p.name.clone()); }
        }
        if !names.is_empty() {
            *REBUILT.lock().unwrap() = names.clone();
            let _ = app.emit("filters-rebuilt", names);
        }
    });
}

#[tauri::command]
fn rebuilt_filters(seen: bool) -> Vec<String> {
    let mut r = REBUILT.lock().unwrap();
    if seen { r.clear(); }
    r.clone()
}

/// Fetches the prices; new ones rebuild the filters that use them and can set off price alerts.
fn take_new_prices(app: &AppHandle) {
    let changed = fetch_prices(app);
    if !changed.is_empty() {
        rebuild_priced(app, &changed);
        let _ = app.emit("prices-updated", ());
    }
    play_app::check_alerts(app);
}

/// Hourly price updates, and NeverSink updates every 6 hours, in the background.
fn start_price_updates(app: AppHandle) {
    thread::spawn(move || {
        let mut hour = 0u32;
        loop {
            take_new_prices(&app);
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

/// The app, for sending events from places that only have the state (set once at startup).
static APP: std::sync::OnceLock<AppHandle> = std::sync::OnceLock::new();

/// Saves a filter's setup before a change, so it can be gone back to. Every change to a filter passes through
/// here, so it also tells the editor window to show its "Saved" note.
fn remember(state: &AppState, store: &Store, id: &str, what: &str) {
    if let Some(p) = store.profiles.get(id) {
        let mut h = state.history.lock().unwrap();
        h.record(p, what);
        let _ = h.save(&state.history_path);
        if let Some(app) = APP.get() { let _ = app.emit_to("main", "filter-changed", what.to_string()); }
    }
}

static BATCH_ON: AtomicBool = AtomicBool::new(false);
/// What batch mode has collected so far, kept here (not in the page) so nothing is lost while the editor is on
/// another page or closed to the tray. One entry per item kind (rarity, base and, for uniques, the name).
static BATCH_LIST: Mutex<Vec<Item>> = Mutex::new(Vec::new());

fn batch_key(i: &Item) -> String { format!("{}|{}|{}", i.rarity, i.base_type, if i.rarity == "Unique" { i.name.as_deref().unwrap_or("") } else { "" }) }

/// The count and the last item, for the small in-game batch window.
#[derive(Serialize, Clone)]
struct BatchCount { count: usize, last: String, added: bool }

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
    // Jewels: their mods by name, for "Jewels with my mods" (F2 ticks them; HasExplicitMod checks them).
    if item.item_class == "Jewels" && !item.unidentified {
        item.mod_names = Some(filter_core::craft::jewel_mods_in(text));
    }
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
    // Also unsure when the guess doesn't really look like the label (26 Nov: a magic cluster jewel guessed as unique,
    // because the filter's cluster jewel looks depend on passives the label doesn't show).
    let poor_fit = item.name.is_none() && colours.is_some_and(|c|
        filter_core::ground::label_distance(&live, &item, c, area, &item.rarity, item.sockets.as_deref()) > filter_core::ground::LABEL_CLOSE);
    // A label that doesn't look like your filter's unique look isn't a unique (it was priced as The Voices).
    if poor_fit && item.rarity == "Unique" { item.rarity = "Normal".into(); }
    let mut rarity_unsure = filter_core::item::is_gear(&item)
        && ((item.rarity != "Unique" && filter_core::ground::rarity_ambiguous(&live, &item, colours, area)) || poor_fit);
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

/// The game's drawing area on screen (x, y, width, height), in physical pixels.
#[cfg(windows)]
fn game_client_rect() -> Option<(i32, i32, i32, i32)> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect};
    unsafe {
        let h = FindWindowW(w!("POEWindowClass"), PCWSTR::null())
            .or_else(|_| FindWindowW(PCWSTR::null(), w!("Path of Exile")))
            .or_else(|_| FindWindowW(PCWSTR::null(), w!("Path of Exile 2"))).ok()?;
        if h.is_invalid() { return None; }
        let mut r = RECT::default();
        GetClientRect(h, &mut r).ok()?;
        let mut p = POINT { x: 0, y: 0 };
        if !ClientToScreen(h, &mut p).as_bool() { return None; }
        let (w, ht) = (r.right - r.left, r.bottom - r.top);
        (w > 200 && ht > 200).then_some((p.x, p.y, w, ht))
    }
}
#[cfg(not(windows))]
fn game_client_rect() -> Option<(i32, i32, i32, i32)> { None }

/// The price window's docked edge: (true = its right edge stays put, x). Set when it opens beside a game panel, so it
/// grows away from the inventory (or stash) instead of over it.
static TRADE_DOCK: Mutex<Option<(bool, i32)>> = Mutex::new(None);

/// Like Awakened PoE Trade: the price window opens in the same place every time, beside the game's side panel. Price
/// check in your inventory (right half of the game) and it sits just left of the inventory; in the stash (left half),
/// just right of the stash. The panel is 370 px wide at 600 px tall, so it scales with the game's height.
fn place_beside_panel(w: &tauri::WebviewWindow) {
    let Some((gx, gy, gw, gh)) = game_client_rect() else { *TRADE_DOCK.lock().unwrap() = None; place_near_cursor(w, 24); return };
    let Ok(pos) = w.cursor_position() else { return };
    let panel = (gh as f64 * 370.0 / 600.0).round() as i32;
    let width = w.outer_size().map(|s| s.width as i32).unwrap_or(400);
    let inventory = pos.x as i32 > gx + gw / 2;
    let x = if inventory { gx + gw - panel - width } else { gx + panel };
    *TRADE_DOCK.lock().unwrap() = Some(if inventory { (true, gx + gw - panel) } else { (false, gx + panel) });
    let _ = w.set_position(PhysicalPosition::new(x, gy));
}

/// Called by a pop-up after it changes size: pulls it back fully onto its screen.
#[tauri::command]
fn keep_on_screen(window: tauri::WebviewWindow) {
    let (Ok(mut p), Ok(s)) = (window.outer_position(), window.outer_size()) else { return };
    // The docked price window keeps its edge beside the game panel when it grows or shrinks.
    if window.label() == "trade" {
        if let Some((right, edge)) = *TRADE_DOCK.lock().unwrap() {
            let x = if right { edge - s.width as i32 } else { edge };
            if x != p.x { let _ = window.set_position(PhysicalPosition::new(x, p.y)); p.x = x; }
        }
    }
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
    /// Search the trade site straight away for every item (Settings), not only uniques/bulk/unidentified.
    instant_all: bool,
    /// poe.ninja's last 7 days for this item (the line under the price).
    trend: Option<filter_core::prices::Trend>,
    /// About how many sold recently on the in-game exchange (poe.ninja), for cards and other exchange items:
    /// a handful means its poe.ninja price is a guess.
    sold: Option<f64>,
    /// A unique whose version can't be told: poe.ninja's price for the dearest one (`ninja` is the cheapest).
    ninja_high: Option<f64>,
}


/// Keeps your recent price checks (and their trade results) in the data folder's "samples",
/// so the price window can be tried out without the game (preview.html). Local only, newest 60.
fn save_sample(data_dir: &std::path::Path, item: &filter_core::item::Item, suffix: &str, v: &impl Serialize) {
    let dir = data_dir.join("samples");
    if std::fs::create_dir_all(&dir).is_err() { return; }
    let key = filter_core::profiles::slug(&format!("{} {}", item.name.as_deref().unwrap_or(""), item.base_type));
    let Ok(json) = serde_json::to_string_pretty(v) else { return };
    let _ = std::fs::write(dir.join(format!("{}{suffix}.json", key.trim_matches('-'))), json);
    if let Ok(rd) = std::fs::read_dir(&dir) {
        let mut files: Vec<_> = rd.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
        if files.len() > 60 { files.sort(); for (_, p) in &files[..files.len() - 60] { let _ = std::fs::remove_file(p); } }
    }
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
    // Conqueror and Elder guardian maps have poe.ninja prices of their own ("Drox Map (Tier 16)"; 26 Nov: his Drox map
    // showed the plain T16's 2c).
    let special = item.occupied_by.as_ref().filter(|_| item.item_class == "Maps")
        .and_then(|who| price_of(f, None, &format!("{who} {}", item.base_type), ""));
    special.or_else(|| price_of(f, item.name.as_deref(), &item.base_type, &item.rarity)).map(|v| (v * chaos_per_unit(f), "chaos".into()))
}

/// Ctrl+D: copies the hovered item (advanced copy: mod tiers too) and opens the price window.
fn price_check(app: AppHandle) {
    thread::spawn(move || {
        // The price window from the last Ctrl+D (or F2, F6…) may have focus: give it back to the game and price
        // what's under the mouse, so you can check one item after another (like Awakened).
        if !game_back_in_front(&app) { return; }
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
        // A Mercenary is here (its strip is up): its items and gems go to the Mercenary window.
        if merc_app::take_copy(&app, &text) { return; }
        let (_, league) = trade_league(&app);
        let payload = if !text.is_empty() {
            text_payload(&app, &text)
        } else {
            // On the ground the game won't copy: read the label for its name.
            match read_from_ground(&app) {
                Ok(p) => {
                    let item = p.item;
                    let category = filter_core::trade::category(&item.item_class).map(|c| if item.base_type.ends_with("Cluster Jewel") { "jewel.cluster" } else { c }).map(String::from);
                    let s = filter_core::trade::TradeSetup { item: item.clone(), filters: vec![], base_only: true, ilvl_min: None, ilvl_max: None,
                        links_min: None, gem_level: None, unknown: vec![], unknown_kinds: vec![], category, raw: String::new(), misc: vec![], base: String::new(), base_item: None, affix_max: None, corrupted: None, buyout: None };
                    PriceCheck { ninja: ninja_price(&app, &item), setup: Some(s), league, from_ground: true, error: None, bulk: None, map: None, divine: None, compare: vec![], instant_all: false, trend: None, sold: None, ninja_high: None }
                }
                Err(e) => PriceCheck { setup: None, ninja: None, league, from_ground: true, error: Some(e), bulk: None, map: None, divine: None, compare: vec![], instant_all: false, trend: None, sold: None, ninja_high: None },
            }
        };
        show_price(&app, payload, &text, true);
    });
}

/// An item's copied text -> what the price window shows (stats picked, poe.ninja price).
fn text_payload(app: &AppHandle, text: &str) -> PriceCheck {
    let (poe2, league) = trade_league(app);
    let state = app.state::<AppState>();
    let none = |league: String, error: String| PriceCheck { setup: None, ninja: None, league, from_ground: false, error: Some(error), bulk: None,
        map: None, divine: None, compare: vec![], instant_all: false, trend: None, sold: None, ninja_high: None };
    match trade_api::stats(&state.data_dir, poe2) {
        Ok(index) => match filter_core::trade::setup(text, &index) {
            Some(s) => PriceCheck { ninja: ninja_price(app, &s.item), setup: Some(s), league, from_ground: false, error: None, bulk: None, map: None, divine: None, compare: vec![], instant_all: false, trend: None, sold: None, ninja_high: None },
            // The game copied no item (the pointer was on an empty slot, a ground label or a window): say what to do.
            None => none(league, "No item was copied: hold the mouse over the item itself (in your stash or inventory) and press the key again".into()),
        },
        Err(e) => match filter_core::trade::setup(text, &Default::default()) {
            Some(s) => PriceCheck { ninja: ninja_price(app, &s.item), setup: Some(s), league, from_ground: false,
                error: Some(format!("Stat list unavailable, searching by name only ({e})")), bulk: None, map: None, divine: None, compare: vec![], instant_all: false, trend: None, sold: None, ninja_high: None },
            None => none(league, e),
        },
    }
}

/// "Price" on an item or gem in the build window / My build: the price window for it, like Ctrl+D in game.
#[tauri::command]
fn price_build_item(app: AppHandle, text: Option<String>, gem: Option<filter_core::pob::Gem>) {
    let Some(text) = text.filter(|t| !t.is_empty()).or_else(|| gem.as_ref().map(filter_core::pob::gem_copy)) else { return };
    thread::spawn(move || {
        let payload = text_payload(&app, &text);
        // It's your own build's item: no "compared with your build" box.
        show_price(&app, payload, &text, false);
    });
}

#[derive(Serialize)]
struct BuyResult {
    /// For sale right now.
    total: u64,
    /// Mods the search asks for (shown as "N for sale" under the button).
    of: usize,
    need: usize,
}

/// "Buy" on an item in your build: a trade search for that base with every mod it has (no numbers, like
/// PoEStash's build links), opened in your browser.
#[tauri::command(async)]
fn buy_build_item(app: AppHandle, text: String) -> Result<BuyResult, String> {
    let (poe2, league) = trade_league(&app);
    let dir = app.state::<AppState>().data_dir.clone();
    let index = trade_api::stats(&dir, poe2).unwrap_or_default();
    let (mut q, of) = filter_core::trade::buy_query(&text, &index).ok_or("Couldn't read that item")?;
    if let Ok(t) = trade_api::item_types(&dir, poe2) { t.fix_query(&mut q); }
    let (mut id, _, mut total) = trade_api::search(poe2, &league, &q)?;
    let (mut of, mut need) = (of, of);
    // Nobody sells one with every mod: ask for fewer of the mods that vary ("any 2 of 3 notables"), keeping what
    // makes the item (a cluster's passives and enchant, a seed). At most three more searches.
    if total == 0 {
        for k in 1..=3 {
            let Some((q2, n, k_need)) = filter_core::trade::buy_loosen(&q, k) else { break };
            let Ok((id2, _, t2)) = trade_api::search(poe2, &league, &q2) else { break };
            id = id2;
            (of, need, total) = (n, k_need, t2);
            if t2 > 0 { break; }
        }
    }
    open_url(trade_api::site_url(poe2, &league, &id))?;
    Ok(BuyResult { total, of, need })
}

#[derive(Serialize)]
struct MercBuyResult { total: u64, need: usize, greater: bool, skill: String, supports: Vec<String> }

/// "Buy" on a Mercenary stage of your build: instant-buyout Mercenary Warrants with its main attack and all its supports
/// as Greater (Tier 3), cheapest first. With none for sale: one Greater support fewer (down to two), then any tier.
#[tauri::command(async)]
fn buy_build_merc(app: AppHandle, groups: Vec<filter_core::pob::Group>) -> Result<MercBuyResult, String> {
    let (poe2, league) = trade_league(&app);
    let dir = app.state::<AppState>().data_dir.clone();
    let index = trade_api::stats(&dir, poe2).unwrap_or_default();
    let n = filter_core::merc::from_build(&groups, &index, usize::MAX, false).ok_or("No Mercenary skill in this stage")?.supports.len();
    let tries: Vec<(bool, usize)> = (2..=n).rev().map(|k| (true, k)).chain((1..=n.max(1)).rev().map(|k| (false, k))).collect();
    let mut last = None;
    for (greater, need) in tries {
        let w = filter_core::merc::from_build(&groups, &index, need, greater).ok_or("No Mercenary skill in this stage")?;
        let (id, _, total) = trade_api::search(poe2, &league, &w.query)?;
        last = Some((id, MercBuyResult { total, need: need.min(n), greater, skill: w.skill, supports: w.supports }));
        if total > 0 { break; }
    }
    let (id, r) = last.ok_or("No Mercenary skill in this stage")?;
    open_url(trade_api::site_url(poe2, &league, &id))?;
    Ok(r)
}

/// "Price" on a Mercenary stage: the price window's warrant search, where the supports can be ticked.
#[tauri::command]
fn price_build_merc(app: AppHandle, groups: Vec<filter_core::pob::Group>) {
    thread::spawn(move || {
        let (poe2, _) = trade_league(&app);
        let index = trade_api::stats(&app.state::<AppState>().data_dir, poe2).unwrap_or_default();
        let Some(w) = filter_core::merc::from_build(&groups, &index, usize::MAX, true) else { return };
        let payload = text_payload(&app, &w.text);
        show_price(&app, payload, &w.text, false);
    });
}

#[derive(Serialize)]
struct BaseResult { total: u64, ilvl: u32, notables: Vec<(String, u32)> }

/// "Buy base" on a cluster jewel in your build: the base to craft it on (same passives and enchant, the item level
/// its notables need), cheapest first, opened in your browser.
#[tauri::command(async)]
fn buy_base_item(app: AppHandle, text: String) -> Result<BaseResult, String> {
    let (poe2, league) = trade_league(&app);
    let dir = app.state::<AppState>().data_dir.clone();
    let index = trade_api::stats(&dir, poe2).unwrap_or_default();
    let (mut q, ilvl, notables) = filter_core::trade::cluster_base_query(&text, &index).ok_or("Only cluster jewels have a base to buy here")?;
    if let Ok(t) = trade_api::item_types(&dir, poe2) { t.fix_query(&mut q); }
    let (id, _, total) = trade_api::search(poe2, &league, &q)?;
    open_url(trade_api::site_url(poe2, &league, &id))?;
    Ok(BaseResult { total, ilvl, notables })
}

/// Fills in the rest (divine rate, build comparison, map check, bulk) and shows the price window.
fn show_price(app: &AppHandle, payload: PriceCheck, text: &str, compare: bool) {
    let (poe2, _) = trade_league(app);
    let state = app.state::<AppState>();
    let text = text.to_string();
    let mut payload = payload;
    payload.divine = currency_rates(app).get("divine").copied();
    // A unique with versions (Voices, Bubonic Trail): the copied text says which one this is.
    if let Some(n) = payload.setup.as_ref().filter(|s| s.item.rarity == "Unique").and_then(|s| s.item.name.clone()) {
        let base = payload.setup.as_ref().map(|s| s.item.base_type.clone()).unwrap_or_default();
        let game = active_game(&state);
        let prices = state.prices.lock().unwrap();
        if let Some((f, u)) = prices.get(&game).and_then(|f| filter_core::prices::unique_price(f, &n, &base, Some(&text)).map(|u| (f, u))) {
            let k = chaos_per_unit(f);
            payload.ninja = Some((u.value * k, "chaos".into()));
            payload.ninja_high = u.high.map(|h| h * k);
        }
    }
    if let Some(it) = payload.setup.as_ref().map(|s| &s.item) {
        let game = active_game(&state);
        payload.trend = state.prices.lock().unwrap().get(&game)
            .and_then(|f| filter_core::prices::trend_of(f, it.name.as_deref(), &it.base_type, &it.rarity));
        if it.rarity != "Unique" {
            payload.sold = state.prices.lock().unwrap().get(&game)
                .and_then(|f| f.items.iter().find(|p| p.name == it.base_type && p.volume.is_some()).and_then(|p| p.sold())
                    .or_else(|| (it.item_class == "Divination Cards" && filter_core::prices::unsold_card(f, &it.base_type)).then_some(0.0)));
        }
    }
    payload.instant_all = state.store.lock().unwrap().settings.price_instant_all;
    // "Is this an upgrade?" against your PoB build's gear in that slot.
    if !state.store.lock().unwrap().settings.price_rolls {
        if let Some(s) = payload.setup.as_mut() { for f in &mut s.filters { f.roll = None; f.range = None; } }
    }
    if compare && !payload.from_ground && state.store.lock().unwrap().settings.price_compare {
        if let (Some(setup), Some(pob)) = (payload.setup.as_ref(), pob_load(&state)) {
            if let Some(stage) = pob.build.stages.get(pob.stage) {
                let lines: Vec<String> = filter_core::trade::mods(&text).into_iter().map(|m| m.text).collect();
                payload.compare = filter_core::compare::compare(&lines, &setup.item.item_class, &stage.gear);
            }
        }
    }
    if let Some(it) = payload.setup.as_ref().map(|s| s.item.clone()) {
        let invitation = filter_core::maps::runs_like_map(&it.item_class, &it.base_type);
        if (filter_core::maps::is_map(&it.item_class) || invitation) && !payload.from_ground {
            // One list of mods you can't run: the Maps page's ticks (which start from the map check list).
            let (mods, _) = map_mods(&state);
            let s = state.store.lock().unwrap().settings.clone();
            payload.map = Some(if mods.is_empty() {
                filter_core::maps::map_info(&text, s.map_avoid.as_deref())
            } else {
                let bad = s.map_tool.bad.clone().unwrap_or_else(|| filter_core::mapregex::default_bad(&mods, s.map_avoid.as_deref()));
                filter_core::maps::map_info_bad(&text, &mods, &bad)
            });
            // Kirac's invitations can't be sold: the trade site doesn't list them.
            if invitation {
                let sells = trade_api::item_types(&state.data_dir, poe2).map(|t| t.sells(&it.base_type)).unwrap_or(true);
                if let Some(m) = payload.map.as_mut() { m.no_sale = !sells; }
            }
            // The map tracker shows these mods on the next map you open.
            if let Some(m) = payload.map.as_ref() {
                // Name and base both: a unique map's area is its name, any other map's area is in its base.
                let names = format!("{} {}", it.name.as_deref().unwrap_or(""), it.base_type);
                play_app::note_map_check(&names, m.mods.iter().map(|x| x.text.clone()).collect());
            }
        }
    }
    if let Some(it) = payload.setup.as_ref().map(|s| s.item.clone()) {
        if !matches!(it.rarity.as_str(), "Rare" | "Magic" | "Unique") && filter_core::trade::category(&it.item_class).is_none() {
            payload.bulk = trade_api::statics(&state.data_dir, poe2).ok().and_then(|st| st.id(&it.base_type));
        }
    }
    if let Some(w) = app.get_webview_window("trade") {
        place_beside_panel(&w);
        if let Some(s) = payload.setup.as_ref() { save_sample(&state.data_dir, &s.item, "", &payload); }
        let _ = w.emit("price-check", payload);
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[derive(Serialize)]
struct TradeResult {
    listings: Vec<filter_core::trade::Listing>,
    /// Vessels: (how many of the listings have all of yours' unique monsters, how many were looked at).
    same: Option<(usize, usize)>,
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
    let k = f.map_or(1.0, chaos_per_unit);
    filter_core::trade::rates(|name| f.and_then(|f| price_of(f, None, name, "Currency")).map(|v| v * k))
}

/// The trade site search for a price window setup (with transfigured gems' search names).
fn trade_query(app: &AppHandle, poe2: bool, setup: &filter_core::trade::TradeSetup) -> serde_json::Value {
    let mut q = filter_core::trade::query(setup);
    // Instant buyout only (the default): prices you can actually buy at, no whisper-and-wait sellers.
    // The price window's "Instant buyout only" chip can switch it for one search; otherwise your Settings choice (on).
    let buyout = setup.buyout.unwrap_or_else(|| app.state::<AppState>().store.lock().unwrap().settings.price_buyout_only);
    if buyout { q["query"]["status"]["option"] = serde_json::json!("securable"); }
    if let Ok(types) = trade_api::item_types(&app.state::<AppState>().data_dir, poe2) { types.fix_query(&mut q); }
    q
}

/// When you last price checked by hand: live searches wait so yours go first.
static LAST_MANUAL: Mutex<Option<std::time::Instant>> = Mutex::new(None);

/// Everything the price window's search asks for besides the ticked stats, in plain words ("Instant buyout only",
/// "Base: Two-Toned Boots", "Not corrupted"…): the "hidden filters" list. No network.
#[tauri::command]
fn trade_hidden(app: AppHandle, setup: filter_core::trade::TradeSetup) -> Vec<String> {
    let (poe2, _) = trade_league(&app);
    filter_core::trade::describe(&trade_query(&app, poe2, &setup))
}

/// Runs the price search on the trade site (from the price window).
#[tauri::command(async)]
fn trade_search(app: AppHandle, setup: filter_core::trade::TradeSetup) -> Result<TradeResult, String> {
    *LAST_MANUAL.lock().unwrap() = Some(std::time::Instant::now());
    let (poe2, league) = trade_league(&app);
    let q = trade_query(&app, poe2, &setup);
    let (id, ids, total) = trade_api::search(poe2, &league, &q)?;
    let fetched = trade_api::fetch(poe2, &ids, &id)?;
    let rates = currency_rates(&app);
    let mut listings = filter_core::trade::listings(&fetched, &rates);
    // A vessel's uniques can't be searched by name: look through the thirty cheapest for ones with yours.
    let uniques = filter_core::trade::vessel(&setup.raw).map(|v| v.0).unwrap_or_default();
    let mut same = None;
    if !uniques.is_empty() {
        for more in ids.chunks(10).skip(1).take(2) {
            if let Ok(v) = trade_api::fetch(poe2, more, &id) { listings.extend(filter_core::trade::listings(&v, &rates)); }
        }
        same = Some((filter_core::trade::same_monsters(&mut listings, &uniques), listings.len()));
    }
    // Three or more like yours decide the price; fewer and it's every one found.
    let like: Vec<_> = if same.is_some_and(|s| s.0 >= 3) { listings.iter().filter(|l| l.same).cloned().collect() } else { listings.clone() };
    let r = TradeResult { same, summary: filter_core::trade::summary(&like), total, url: trade_api::site_url(poe2, &league, &id),
        divine: rates.get("divine").copied(), listings };
    save_sample(&app.state::<AppState>().data_dir, &setup.item, ".result", &r);
    Ok(r)
}

#[derive(Serialize)]
struct UntickTry {
    /// A filter id, or "base" (same base only) / "ilvl" (item level).
    key: String,
    label: String,
    total: u64,
}

/// A search found nothing: tries it once without each ticked stat (and without "same base only" / item level),
/// most hopeful first, so the window can say which one to untick. One search request per try (no listings
/// fetched); stops early, keeping what it has, if the trade site's limit gets close. At most 8 tries.
#[tauri::command(async)]
fn trade_untick_one(app: AppHandle, setup: filter_core::trade::TradeSetup) -> Result<Vec<UntickTry>, String> {
    *LAST_MANUAL.lock().unwrap() = Some(std::time::Instant::now());
    let (poe2, league) = trade_league(&app);
    let mut tries: Vec<(String, String, filter_core::trade::TradeSetup)> = Vec::new();
    for (i, f) in setup.filters.iter().enumerate().filter(|(_, f)| f.enabled) {
        let mut s = setup.clone();
        s.filters[i].enabled = false;
        tries.push((f.id.clone(), f.label.clone(), s));
    }
    if setup.base_only && setup.category.is_some() {
        let mut s = setup.clone();
        s.base_only = false;
        tries.push(("base".into(), format!("{} only", if setup.base.is_empty() { &setup.item.base_type } else { &setup.base }), s));
    }
    if setup.ilvl_min.is_some() {
        let mut s = setup.clone();
        s.ilvl_min = None; s.ilvl_max = None;
        tries.push(("ilvl".into(), "Item level".into(), s));
    }
    let mut out = Vec::new();
    for (key, label, s) in tries.into_iter().take(8) {
        match trade_api::search(poe2, &league, &trade_query(&app, poe2, &s)) {
            Ok((_, _, total)) => out.push(UntickTry { key, label, total }),
            Err(e) if out.is_empty() => return Err(e),
            Err(_) => break,
        }
    }
    out.sort_by(|a, b| b.total.cmp(&a.total));
    Ok(out)
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

/// "Something's wrong" in the price window: keeps the note with everything the window had
/// (item text, search, trade result) in the data folder's "reports", for Claude to look at. Local only.
#[tauri::command]
fn report_problem(state: State<AppState>, note: String, data: serde_json::Value) -> Result<String, String> {
    let dir = state.data_dir.join("reports");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("{secs}.json"));
    let body = serde_json::json!({ "note": note, "at": secs, "window": data });
    std::fs::write(&path, serde_json::to_string_pretty(&body).unwrap_or_default()).map_err(|e| e.to_string())?;
    Ok(path.display().to_string())
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
                    if !w.is_visible().unwrap_or(false) { win_place::show_over_game(&app, &w); }
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
struct BulkResult { offers: Vec<filter_core::trade::Offer>, url: String, divine: Option<f64>,
    /// What sellers ask, when poe.ninja's price is too thin to go by.
    asking: Option<filter_core::bulk::Asking> }

/// Bulk rates: people selling `want` (a trade id like "divine") for `pay` ("chaos"/"divine").
#[tauri::command(async)]
/// `minimum`: only sellers with at least this many in stock (Awakened's stock filter: your stack size).
/// `market` / `sold`: poe.ninja's chaos price and how many sold recently, to judge it by the sellers.
fn trade_bulk(app: AppHandle, want: String, pay: String, minimum: Option<u32>, market: Option<f64>, sold: Option<f64>, card: Option<bool>) -> Result<BulkResult, String> {
    let (poe2, league) = trade_league(&app);
    let (id, v) = trade_api::exchange(poe2, &league, &filter_core::trade::exchange_query(&pay, &want, minimum.filter(|m| *m > 1)))?;
    let divine = currency_rates(&app).get("divine").copied();
    let offers = filter_core::trade::offers(&v);
    // Judged on every seller for chaos (not only the twenty shown, nor the ones with a big stock).
    let asking = Some(market.unwrap_or(0.0)).filter(|_| pay == "chaos" && minimum.is_none_or(|m| m <= 1))
        .and_then(|m| filter_core::bulk::asking(&offers.iter().map(|o| o.each).collect::<Vec<_>>(), m, sold, card.unwrap_or(false)));
    Ok(BulkResult { offers: offers.into_iter().take(20).collect(), url: trade_api::exchange_url(poe2, &league, &id), divine, asking })
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
        Ok(TradeResult { same: None, summary: filter_core::trade::summary(&listings), total, url: trade_api::site_url(poe2, &league, &id),
            divine: rates.get("divine").copied(), listings })
    }).await.map_err(err)?
}

/// Every line the trade site knows a map can have ("Must have" on the Maps page).
#[tauri::command(async)]
fn map_tool_stats(app: AppHandle) -> Result<Vec<filter_core::mapregex::MapExtra>, String> {
    let (poe2, _) = trade_league(&app);
    trade_api::map_stats(&app.state::<AppState>().data_dir, poe2)
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

/// "Open the editor" from a window over the game: brings the main window up on that page.
#[tauri::command]
fn open_editor(app: AppHandle, tab: Option<String>) {
    if let Some(w) = app.get_webview_window("main") { let _ = w.unminimize(); let _ = w.show(); let _ = w.set_focus(); }
    if let Some(t) = tab { let _ = app.emit_to("main", "open-tab", t); }
}

/// The "Open the editor" key pressed in the editor: minimise it and put the game back in front (26 Nov, he asked).
#[tauri::command]
fn editor_to_game(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") { let _ = w.minimize(); }
    focus_game();
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard().write_text(text).map_err(err)
}

#[tauri::command]
fn hide_trade_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("trade") { let _ = w.hide(); }
}

/// The game is in front, or one of our windows over it is and the game can be put back in front (the editor's
/// main window doesn't count: a hotkey pressed there shouldn't pull you into the game).
fn game_back_in_front(app: &AppHandle) -> bool {
    if game_in_front() { return true; }
    let ours = app.webview_windows().iter().any(|(label, w)| label != "main" && w.is_focused().unwrap_or(false));
    if !ours || !focus_game() { return false; }
    for _ in 0..15 {
        if game_in_front() {
            // A moment for the game to take keys again before we send it the copy.
            thread::sleep(Duration::from_millis(60));
            return true;
        }
        thread::sleep(Duration::from_millis(20));
    }
    false
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

/// Is one of our own windows in front (the editor, the map tracker, the price window)?
#[cfg(windows)]
fn ours_in_front() -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(GetForegroundWindow(), Some(&mut pid));
        pid == std::process::id()
    }
}
#[cfg(not(windows))]
fn ours_in_front() -> bool { false }

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
            // The play windows (levelling, map tracker) read the log too, so it's always followed (22 Nov);
            // only the trade pop-ups depend on the trade helper being on.
            let trades_on = state.store.lock().unwrap().settings.trade_helper;
            if log.is_none() {
                if last_look.elapsed() < Duration::from_secs(30) { continue; }
                last_look = std::time::Instant::now();
                let poe2 = active_game(&state) == Game::Poe2;
                // Start at the end: only new whispers. Where you are and who you play come from the end of the log.
                log = find_log(poe2, &[]).and_then(|p| std::fs::metadata(&p).ok().map(|m| (p, m.len())));
                if let Some((p, _)) = &log { play_app::prime(&app, p); }
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
                play_app::on_line(&app, line);
                merc_app::on_line(&app, line);
                let Some(ev) = filter_core::whispers::parse_line(line) else { continue };
                // Phone alerts work whether or not the trade pop-up is on.
                phone_app::on_log_event(&app, &ev);
                if !trades_on { continue; }
                let is_trade = matches!(&ev, filter_core::whispers::LogEvent::Whisper(w) if w.item.is_some());
                let Some(w) = app.get_webview_window("trades") else { continue };
                let _ = w.emit("trade-event", &ev);
                if is_trade && !w.is_visible().unwrap_or(false) { win_place::show_over_game(&app, &w); } // never steals focus from the game
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
                let name = item.name.clone().filter(|_| item.rarity == "Unique" || item.rarity == "Rare").map(|n| format!("{n} ({})", item.base_type)).unwrap_or_else(|| item.base_type.clone());
                let (count, added) = {
                    let mut l = BATCH_LIST.lock().unwrap();
                    let added = !l.iter().any(|x| batch_key(x) == batch_key(&item));
                    if added { l.push(item.clone()); }
                    (l.len(), added)
                };
                if added { let _ = app.emit("batch-item", &item); }
                let _ = app.emit("batch-count", BatchCount { count, last: name, added });
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
    /// Switched off: listed, but left out of the filter.
    off: bool,
    /// The saved look it uses, if any.
    look: Option<String>,
}

fn rule_label(r: &Rule) -> String {
    match r {
        Rule::Override { label, .. } => label.clone(),
        Rule::MoveBase { type_path, base, to_tier, .. } if to_tier.is_empty() => {
            format!("Tier: {base} taken out of {}", type_path.join(" > "))
        }
        Rule::MoveBase { type_path, base, to_tier, .. } => {
            format!("Tier: {base} moved to {} {}", type_path.join(" > "), filter_core::tiers::tier_name(to_tier))
        }
        Rule::Tier { type_path, tier, show, actions, min_stack, min_item_tier, conds, .. } => {
            let what = match (show, actions, min_stack) {
                (Some(false), ..) => "hidden".to_string(),
                (_, Some(_), _) => "restyled".to_string(),
                (_, _, Some(n)) => format!("set to stacks of {n}+"),
                _ if min_item_tier.is_some() => format!("set to item tier {}+", min_item_tier.unwrap_or(0)),
                (Some(true), ..) => "shown".to_string(),
                _ if conds.is_some() => {
                    let c = conds.as_ref().unwrap();
                    let w: Vec<String> = c.iter().map(|(k, v)| if v.is_empty() { format!("no {} condition", filter_core::conds::name_of(k)) }
                        else { filter_core::conds::words(k, v).0 }).collect();
                    format!("set to {}", w.join(", "))
                }
                _ => "changed".to_string(),
            };
            format!("Tier: {} {} {what}", type_path.join(" > "), filter_core::tiers::tier_name(tier))
        }
    }
}

/// Tiers → Threads (24 Nov): every Thread poe.ninja prices (cheapest first), with the page's "hide if worth under"
/// line and "always show stacks of N+" (both kept in the profile's price settings, key "threads").
#[derive(Serialize)]
struct ThreadsPage { items: Vec<(String, f64)>, limit: Option<f64>, stack: Option<u32>, unit: String,
    /// What the filter really does with each: from what stack it shows (None = never), and what hides the rest.
    shows: std::collections::BTreeMap<String, ThreadShows> }
#[derive(Serialize)]
struct ThreadShows { from: Option<u32>, kind: String, label: String,
    /// Your edit hiding it, when it's in this filter (its place in My edits), else the filter it's in.
    rule: Option<usize>, parent: Option<String> }
#[tauri::command(async)]
fn get_threads(state: State<AppState>, id: String) -> Result<ThreadsPage, String> {
    let store = state.store.lock().unwrap();
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let prices = state.prices.lock().unwrap();
    let file = prices.get(&p.game);
    let key = filter_core::prices::THREADS_KEY;
    let items = file.map(filter_core::prices::thread_prices).unwrap_or_default();
    let mut shows = std::collections::BTreeMap::new();
    if let Ok((text, _)) = render_profile(&store, &id, &filter_dir(p.game), file, None) {
        let segs = parse_filter(&text);
        for (name, _) in &items {
            let (from, why) = filter_core::preview::shows_from(&segs, "Map Fragments", name, 83);
            let why = why.unwrap_or_default();
            let mine = |q: &filter_core::profiles::Profile| q.rules.iter().position(|r| !r.is_off() && rule_label(r) == why.label);
            let rule = (why.kind == "edit").then(|| mine(p)).flatten();
            let parent = (why.kind == "edit" && rule.is_none()).then(|| store.profiles.values().find(|q| q.id != id && mine(q).is_some()).map(|q| q.name.clone())).flatten();
            shows.insert(name.clone(), ThreadShows { from, kind: why.kind, label: why.label, rule, parent });
        }
    }
    Ok(ThreadsPage {
        items, shows,
        limit: p.prices.as_ref().and_then(|s| s.section_hide.get(key).copied()),
        stack: p.prices.as_ref().and_then(|s| s.section_stack.get(key).copied()),
        unit: file.map(|f| f.unit.clone()).unwrap_or_else(|| "chaos".into()),
    })
}

/// NeverSink's tier lists for a profile, as they'll be in game (your tier edits applied).
#[tauri::command]
fn get_tiers(state: State<AppState>, id: String) -> Result<Vec<filter_core::tiers::Section>, String> {
    let store = state.store.lock().unwrap();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?;
    let mut s = filter_core::tiers::sections(&parse_filter(&text));
    let rules = store.resolved_rules(&id);
    filter_core::tiers::mark_changed(&mut s, &rules);
    // Items your own edits hide, whatever the tiers say.
    let hidden = filter_core::tiers::hidden_by_edits(&s, &parse_filter(&text), &rules, if game == Game::Poe1 { 83 } else { 80 });
    for (base, (from, label)) in hidden {
        let mine = |q: &filter_core::profiles::Profile| q.rules.iter().position(|r| !r.is_off() && rule_label(r) == label);
        let rule = store.profiles.get(&id).and_then(mine);
        let parent = if rule.is_none() { store.profiles.values().find(|q| q.id != id && mine(q).is_some()).map(|q| q.name.clone()) } else { None };
        for sec in s.iter_mut().filter(|x| x.group != filter_core::tiers::LEVELLING && x.tiers.iter().any(|t| t.bases.contains(&base))) {
            sec.edit_hides.insert(base.clone(), filter_core::tiers::EditHide { from, label: label.clone(), rule, parent: parent.clone() });
        }
    }
    // poe.ninja worth beside each item, and each section's "hide if worth under" line.
    if let Some(file) = prices.get(&game) {
        let hides = store.profiles.get(&id).and_then(|p| p.prices.as_ref()).map(|p| p.section_hide.clone()).unwrap_or_default();
        filter_core::tiers::attach_prices(&mut s, file, &hides);
    }
    Ok(s)
}

/// One of NeverSink's rules that isn't a tier list ("Modify rule" on e.g. white 4-links).
#[tauri::command]
fn get_rule_section(state: State<AppState>, id: String, type_path: Vec<String>) -> Result<Option<filter_core::tiers::Section>, String> {
    let store = state.store.lock().unwrap();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?;
    let mut s: Vec<_> = filter_core::tiers::section_at(&parse_filter(&text), &type_path).into_iter().collect();
    filter_core::tiers::mark_changed(&mut s, &store.resolved_rules(&id));
    Ok(s.pop())
}

#[tauri::command]
fn get_map_tiers(state: State<AppState>, id: String) -> Result<filter_core::tiers::MapTiers, String> {
    let store = state.store.lock().unwrap();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let prices = state.prices.lock().unwrap();
    let (text, _) = render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?;
    Ok(filter_core::tiers::map_tiers(&parse_filter(&text)))
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
struct PobState { build: filter_core::pob::Build, stage: usize, link: String, follow: bool,
    /// What won't work in game, per stage (build checker).
    checks: Vec<filter_core::buildcheck::StageCheck> }

fn pob_load(state: &AppState) -> Option<PobState> {
    let code = std::fs::read_to_string(pob_file(state)).ok()?;
    let build = filter_core::pob::parse(&filter_core::pob::decode(&code).ok()?).ok()?;
    let s = state.store.lock().unwrap();
    let stage = s.settings.pob_stage.unwrap_or(build.active).min(build.stages.len().saturating_sub(1));
    let checks = filter_core::buildcheck::check(&build);
    Some(PobState { build, stage, link: s.settings.pob_link.clone(), follow: s.settings.pob_follow, checks })
}

/// Your saved builds (My build page): one is shown (its code is also in pob.txt), the rest wait to be swapped in.
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct PobBuilds { active: Option<String>, list: Vec<SavedPob> }

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct SavedPob {
    id: String,
    name: String,
    link: String,
    code: String,
    #[serde(default)] stage: Option<usize>,
    #[serde(default)] follow: bool,
    /// Ascendancy (or class) and level, for the list.
    #[serde(default)] class: String,
    #[serde(default)] level: u32,
}

fn pob_builds_file(state: &AppState) -> PathBuf { state.store_path.with_file_name("pob_builds.json") }

fn pob_new_id(b: &PobBuilds) -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut id = format!("b{secs}");
    while b.list.iter().any(|e| e.id == id) { id.push('x'); }
    id
}

fn pob_class(b: &filter_core::pob::Build) -> String { if b.ascendancy.is_empty() { b.class.clone() } else { b.ascendancy.clone() } }

/// A name to start with: the build's main skill and ascendancy ("Kinetic Fusillade Hierophant").
fn pob_name(b: &filter_core::pob::Build) -> String {
    let class = pob_class(b);
    let skill = b.stages.last().and_then(|st| st.groups.iter().filter(|g| g.enabled)
        .max_by_key(|g| g.gems.iter().filter(|x| x.enabled).count())
        .and_then(|g| g.gems.iter().find(|x| x.enabled && !x.support)).map(|x| x.name.clone()));
    match skill {
        Some(sk) if class.is_empty() => sk,
        Some(sk) => format!("{sk} {class}"),
        None if class.is_empty() => "My build".into(),
        None => class,
    }
}

/// The saved list. The first time, the build you already had becomes its first entry.
fn pob_builds_load(state: &AppState) -> PobBuilds {
    let mut b: PobBuilds = filter_core::keep::load(&pob_builds_file(state)).unwrap_or_default();
    if b.list.is_empty() {
        if let Ok(code) = std::fs::read_to_string(pob_file(state)) {
            if let Some(build) = filter_core::pob::decode(&code).ok().and_then(|x| filter_core::pob::parse(&x).ok()) {
                let id = pob_new_id(&b);
                let s = state.store.lock().unwrap();
                b.list.push(SavedPob { id: id.clone(), name: pob_name(&build), link: s.settings.pob_link.clone(), code: code.trim().into(),
                    stage: s.settings.pob_stage, follow: s.settings.pob_follow, class: pob_class(&build), level: build.level });
                drop(s);
                b.active = Some(id);
                let _ = filter_core::keep::save(&pob_builds_file(state), &b, true);
            }
        }
    }
    b
}

/// Before swapping away, the shown build keeps the stage you were on.
fn pob_remember_stage(state: &AppState, b: &mut PobBuilds) {
    let s = state.store.lock().unwrap();
    let active = b.active.clone();
    if let Some(e) = b.list.iter_mut().find(|e| Some(&e.id) == active.as_ref()) {
        e.stage = s.settings.pob_stage;
        e.follow = s.settings.pob_follow;
    }
}

/// Makes a saved build the one shown (My build, F6, Home, the levelling guide...).
fn pob_show(app: &AppHandle, state: &AppState, e: &SavedPob) -> Result<(), String> {
    std::fs::write(pob_file(state), &e.code).map_err(err)?;
    {
        let mut s = state.store.lock().unwrap();
        s.settings.pob_link = e.link.clone();
        s.settings.pob_stage = e.stage;
        s.settings.pob_follow = e.follow;
        s.save(&state.store_path).map_err(err)?;
    }
    let _ = app.emit("pob-imported", ());
    Ok(())
}

/// Imports a PoB link (pobb.in, pastebin, poe.ninja) or a pasted code and shows it. `replace`: the saved build it
/// updates (the Update button); otherwise the same link updates its build, and anything else is added to the list.
#[tauri::command]
fn pob_import(app: AppHandle, state: State<AppState>, link: String, replace: Option<String>) -> Result<PobState, String> {
    let code = match filter_core::pob::raw_url(&link) {
        Some(url) => ureq::get(&url).set("User-Agent", "poe-filter-editor").timeout(Duration::from_secs(20)).call()
            .map_err(|e| format!("Couldn't download that build: {e}"))?
            .into_string().map_err(err)?,
        None => link.trim().to_string(),
    };
    let build = filter_core::pob::parse(&filter_core::pob::decode(&code)?)?;
    let link = if filter_core::pob::raw_url(&link).is_some() { link.trim().to_string() } else { "(pasted code)".into() };
    let mut b = pob_builds_load(&state);
    pob_remember_stage(&state, &mut b);
    let at = replace.as_ref().and_then(|id| b.list.iter().position(|e| &e.id == id))
        .or_else(|| if link == "(pasted code)" { None } else { b.list.iter().position(|e| e.link == link) });
    let entry = match at {
        Some(i) => {
            let e = &mut b.list[i];
            e.link = link;
            e.code = code.trim().into();
            e.stage = Some(build.active);
            e.class = pob_class(&build);
            e.level = build.level;
            e.clone()
        }
        None => {
            let e = SavedPob { id: pob_new_id(&b), name: pob_name(&build), link, code: code.trim().into(), stage: Some(build.active),
                follow: false, class: pob_class(&build), level: build.level };
            b.list.push(e.clone());
            e
        }
    };
    b.active = Some(entry.id.clone());
    filter_core::keep::save(&pob_builds_file(&state), &b, true).map_err(err)?;
    pob_show(&app, &state, &entry)?;
    pob_load(&state).ok_or_else(|| "Couldn't read the build back".into())
}

#[derive(serde::Serialize)]
struct PobListItem { id: String, name: String, link: String, class: String, level: u32, active: bool }

/// The saved builds for the My build page.
#[tauri::command]
fn pob_list(state: State<AppState>) -> Vec<PobListItem> {
    let b = pob_builds_load(&state);
    b.list.iter().map(|e| PobListItem { id: e.id.clone(), name: e.name.clone(), link: e.link.clone(), class: e.class.clone(),
        level: e.level, active: Some(&e.id) == b.active.as_ref() }).collect()
}

/// Swaps to another saved build; the one you leave keeps its stage.
#[tauri::command]
fn pob_switch(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let mut b = pob_builds_load(&state);
    let e = b.list.iter().find(|e| e.id == id).cloned().ok_or("That build isn't saved any more")?;
    pob_remember_stage(&state, &mut b);
    b.active = Some(id);
    filter_core::keep::save(&pob_builds_file(&state), &b, true).map_err(err)?;
    pob_show(&app, &state, &e)
}

#[tauri::command]
fn pob_rename(state: State<AppState>, id: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() { return Err("Give it a name".into()); }
    let mut b = pob_builds_load(&state);
    if let Some(e) = b.list.iter_mut().find(|e| e.id == id) { e.name = name.chars().take(80).collect(); }
    filter_core::keep::save(&pob_builds_file(&state), &b, true).map_err(err)
}

/// Takes a build off the list. Removing the shown one shows the newest left (or none).
#[tauri::command]
fn pob_remove(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let mut b = pob_builds_load(&state);
    let was_shown = b.active.as_deref() == Some(id.as_str());
    b.list.retain(|e| e.id != id);
    if !was_shown { return filter_core::keep::save(&pob_builds_file(&state), &b, true).map_err(err); }
    b.active = b.list.last().map(|e| e.id.clone());
    filter_core::keep::save(&pob_builds_file(&state), &b, true).map_err(err)?;
    match b.list.last() {
        Some(e) => pob_show(&app, &state, e),
        None => {
            let _ = std::fs::remove_file(pob_file(&state));
            {
                let mut s = state.store.lock().unwrap();
                s.settings.pob_link.clear();
                s.settings.pob_stage = None;
                s.save(&state.store_path).map_err(err)?;
            }
            let _ = app.emit("pob-imported", ());
            Ok(())
        }
    }
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
            win_place::restore(app, &w);
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

#[tauri::command]
fn hide_build_window(app: AppHandle) {
    if let Some(w) = app.get_webview_window("build") { let _ = w.hide(); }
}

/// "Open when the passive skill tree is opened": (on, the game's tree key).
#[tauri::command]
fn build_tree_opt(state: State<AppState>) -> (bool, String) {
    let s = &state.store.lock().unwrap().settings;
    (s.build_with_tree, s.key_tree.clone())
}

#[tauri::command]
fn set_build_tree(app: AppHandle, state: State<AppState>, on: bool) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.settings.build_with_tree = on;
    store.save(&state.store_path).map_err(err)?;
    let _ = app.emit("build-tree-opt", on);
    Ok(())
}

/// Windows key code for a key name: letters, digits, F1-F24, Tab, Space.
#[cfg_attr(not(windows), allow(dead_code))]
fn vk_of(key: &str) -> Option<i32> {
    let k = key.trim().to_uppercase();
    let mut c = k.chars();
    match (c.next(), c.next()) {
        (Some(ch), None) if ch.is_ascii_alphanumeric() => return Some(ch as i32),
        _ => {}
    }
    if let Some(n) = k.strip_prefix('F').and_then(|n| n.parse::<i32>().ok()) {
        if (1..=24).contains(&n) { return Some(0x70 + n - 1); }
    }
    match k.as_str() { "TAB" => Some(0x09), "SPACE" => Some(0x20), _ => None }
}

/// Watches the game's passive tree key without taking it from the game (it only looks at
/// the keyboard, the game still gets the key): tree key with the game in front opens the build
/// window on its Tree tab without taking focus; the tree key or Esc closes it again.
/// Enter opens/sends chat, so the tree key is ignored while chat may be open.
#[cfg(windows)]
fn start_tree_watcher(app: AppHandle) {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    thread::spawn(move || {
        let down = |vk: i32| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
        let (mut was_key, mut was_esc, mut was_enter) = (false, false, false);
        let mut chat = false;
        let mut opened = false; // opened by the tree key (so Esc / the key closes it)
        loop {
            let (on, key) = {
                let st = app.state::<AppState>();
                let s = &st.store.lock().unwrap().settings;
                (s.build_with_tree, s.key_tree.clone())
            };
            let vk = vk_of(&key).filter(|_| on);
            let Some(vk) = vk else { opened = false; thread::sleep(Duration::from_millis(500)); continue };
            thread::sleep(Duration::from_millis(25));
            let (k, esc, enter) = (down(vk), down(0x1B), down(0x0D));
            let front = game_in_front();
            if !front { chat = false; }
            if enter && !was_enter && front { chat = !chat; }
            if esc && !was_esc { chat = false; }
            let Some(w) = app.get_webview_window("build") else { continue };
            let shown = w.is_visible().unwrap_or(false);
            if !shown { opened = false; }
            if k && !was_key && front && !chat {
                if !shown {
                    let _ = w.emit_to("build", "build-tree", ());
                    let _ = w.emit("build-shown", ());
                    win_place::restore(&app, &w);
                    let _ = w.show();
                    focus_game(); // keep the keyboard on the game
                    opened = true;
                } else if opened {
                    let _ = w.hide();
                    opened = false;
                }
            } else if esc && !was_esc && opened && shown && front {
                let _ = w.hide();
                opened = false;
            }
            (was_key, was_esc, was_enter) = (k, esc, enter);
        }
    });
}
#[cfg(not(windows))]
fn start_tree_watcher(_app: AppHandle) {}

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
    p.rules.push(Rule::MoveBase { type_path, base, to_tier, off: false });
    save_and_build(&state, &store, &id)
}

/// Changes several tiers of one section at once (the map panel: "hide tiers 1-5", one look for a band).
#[tauri::command]
fn set_tiers(state: State<AppState>, id: String, type_path: Vec<String>, tiers: Vec<String>, show: Option<bool>,
             style: Option<CustomStyle>, reset: bool, clear_style: Option<bool>) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("{} tiers", tiers.len()));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    for t in &tiers {
        tier_edit(&mut p.rules, &type_path, t, show, style.as_ref(), None, None, reset);
        // A highlight switched off: no look of its own, the tier rules decide.
        if clear_style == Some(true) {
            if let Some(Rule::Tier { actions, .. }) = p.rules.iter_mut().rev().find(|r| matches!(r, Rule::Tier { type_path: tp, tier, .. } if *tp == type_path && tier == t)) {
                *actions = Some(vec![]);
            }
        }
    }
    save_and_build(&state, &store, &id)
}

#[allow(clippy::too_many_arguments)]
fn tier_edit(rules: &mut Vec<Rule>, type_path: &[String], tier: &str, show: Option<bool>, style: Option<&CustomStyle>,
             min_stack: Option<u32>, min_item_tier: Option<u32>, reset: bool) {
    let same = |r: &Rule| matches!(r, Rule::Tier { type_path: tp, tier: t, .. } if tp.as_slice() == type_path && t == tier);
    let old = rules.iter().find(|r| same(r)).cloned();
    rules.retain(|r| !same(r));
    if reset {
        // Also undo items dragged into this tier.
        rules.retain(|r| !matches!(r, Rule::MoveBase { type_path: tp, to_tier, .. } if tp.as_slice() == type_path && to_tier == tier));
    } else {
        let (old_show, old_actions, old_stack, old_it, old_conds, old_look) = match old {
            Some(Rule::Tier { show, actions, min_stack, min_item_tier, conds, look, .. }) => (show, actions, min_stack, min_item_tier, conds, look), _ => (None, None, None, None, None, None) };
        rules.push(Rule::Tier { type_path: type_path.to_vec(), tier: tier.to_string(), show: show.or(old_show),
            actions: style.map(filter_core::rules::style_actions).or(old_actions), min_stack: min_stack.or(old_stack),
            min_item_tier: min_item_tier.or(old_it), conds: old_conds,
            // Any change switches it back on; a new look of its own stops it following a saved look.
            off: false, look: if style.is_some() { None } else { old_look } });
    }
}

/// Changes a step's conditions (Tiers page chips): `set` keyword -> new value ("" removes that condition),
/// `undo` puts those keywords back to NeverSink's.
#[tauri::command]
fn set_tier_conds(state: State<AppState>, id: String, type_path: Vec<String>, tier: String,
                  set: std::collections::BTreeMap<String, String>, undo: Vec<String>) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("Step {}", filter_core::tiers::tier_name(&tier)));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    tier_edit(&mut p.rules, &type_path, &tier, None, None, None, None, false);
    if let Some(Rule::Tier { conds, .. }) = p.rules.iter_mut().rev()
        .find(|r| matches!(r, Rule::Tier { type_path: tp, tier: t, .. } if *tp == type_path && *t == tier)) {
        let mut c = conds.take().unwrap_or_default();
        for k in &undo { c.remove(k); }
        c.extend(set);
        *conds = if c.is_empty() { None } else { Some(c) };
    }
    save_and_build(&state, &store, &id)
}

/// Show/hide a whole tier, give it a new look, or put it back (reset).
#[tauri::command]
fn set_tier(state: State<AppState>, id: String, type_path: Vec<String>, tier: String, show: Option<bool>,
            style: Option<CustomStyle>, min_stack: Option<u32>, min_item_tier: Option<u32>, reset: bool) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    remember(&state, &store, &id, &format!("Tier {}", filter_core::tiers::tier_name(&tier)));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    tier_edit(&mut p.rules, &type_path, &tier, show, style.as_ref(), min_stack, min_item_tier, reset);
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
                        off: r.is_off(),
                        look: r.look().map(str::to_string),
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

/// A share pack for a filter: its edits, saved looks, theme and price settings (never NeverSink's file).
#[derive(Serialize)]
struct PackOut { code: String, file_name: String, summary: filter_core::share::PackSummary }

#[tauri::command]
fn export_pack(state: State<AppState>, id: String) -> Result<PackOut, String> {
    let store = state.store.lock().unwrap();
    let pack = filter_core::share::export(&store, &id).ok_or("Filter not found")?;
    let safe: String = pack.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '_' }).collect();
    Ok(PackOut { code: filter_core::share::to_code(&pack), file_name: format!("{}.lodestone", safe.trim()), summary: filter_core::share::summary(&pack) })
}

/// Saves a filter's share pack as a file (where the save box said).
#[tauri::command]
fn save_pack_file(state: State<AppState>, id: String, path: String) -> Result<(), String> {
    let store = state.store.lock().unwrap();
    let pack = filter_core::share::export(&store, &id).ok_or("Filter not found")?;
    std::fs::write(&path, filter_core::share::to_file(&pack)).map_err(err)
}

/// A pack from a pasted code, or from a pack file.
fn read_pack_from(text: Option<String>, path: Option<String>) -> Result<filter_core::share::SharePack, String> {
    let text = match (text, path) {
        (Some(t), _) if !t.trim().is_empty() => t,
        (_, Some(p)) => {
            let meta = std::fs::metadata(&p).map_err(err)?;
            if meta.len() > 20_000_000 { return Err("That file is far too big to be a share pack".into()); }
            std::fs::read_to_string(&p).map_err(|_| "That file couldn't be read".to_string())?
        }
        _ => return Err("Paste a share code or pick a pack file".into()),
    };
    filter_core::share::from_text(&text)
}

/// What a pack holds, to show before importing.
#[tauri::command]
fn read_pack(text: Option<String>, path: Option<String>) -> Result<filter_core::share::PackSummary, String> {
    read_pack_from(text, path).map(|p| filter_core::share::summary(&p))
}

/// Adds a pack as a new filter on your own NeverSink file (the pack's strictness, downloaded fresh), then switches to it.
#[tauri::command]
fn import_pack(state: State<AppState>, text: Option<String>, path: Option<String>, name: String) -> Result<String, String> {
    let pack = read_pack_from(text, path)?;
    if state.store.lock().unwrap().profiles.values().any(|p| p.name.eq_ignore_ascii_case(name.trim())) {
        return Err(format!("You already have a filter called {}", name.trim()));
    }
    let (base_file, strictness) = match pack.strictness {
        Some(level) => (download_neversink(&state, pack.game, level.min(6))?.0.display().to_string(), Some(level.min(6))),
        None => {
            // Made on someone's own file: use yours for that game (the active filter's, else any).
            let store = state.store.lock().unwrap();
            let mine = store.active.as_ref().and_then(|id| store.profiles.get(id)).filter(|p| p.game == pack.game)
                .or_else(|| store.profiles.values().find(|p| p.game == pack.game))
                .ok_or("Set up a filter for that game first: the pack goes on top of your own NeverSink filter")?;
            (mine.base_file.clone(), mine.strictness)
        }
    };
    let mut store = state.store.lock().unwrap();
    let id = filter_core::share::import(&mut store, &pack, &name, &base_file, strictness)?;
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

/// F2 / My edits: "Only show 3+". Hides the smaller stacks and makes sure stacks of that size really show (25 Nov).
#[tauri::command]
fn only_show_stack(state: State<AppState>, item: Item, min: u32, opts: Option<MatchOpts>) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    let opts = opts.unwrap_or_default();
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let name = item.base_type.clone();
    remember(&state, &store, &id, &format!("Only show {name} in stacks of {min}+"));
    let label = filter_core::rules::only_show(&mut store.profiles.get_mut(&id).ok_or("Profile not found")?.rules, &item, min, &opts);
    let (text, _) = {
        let prices = state.prices.lock().unwrap();
        render_profile(&store, &id, &filter_dir(game), prices.get(&game), None).map_err(err)?
    };
    let level = match (opts.when, game) { (filter_core::rules::When::Levelling, _) => 60, (_, Game::Poe1) => 83, _ => 80 };
    if let Some(show) = filter_core::rules::show_hidden_stacks(&parse_filter(&text), &item, min, &opts, level) {
        // Below the "stacks under N" hide it belongs to, so that one stays the newest.
        let rules = &mut store.profiles.get_mut(&id).unwrap().rules;
        let hide = rules.pop();
        add_rule(rules, show);
        rules.extend(hide);
    }
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
}

/// Saves a full custom look (colours, beam, map icon, sound) for this item.
#[tauri::command]
fn apply_custom_style(state: State<AppState>, item: Item, style: Option<CustomStyle>, opts: Option<MatchOpts>, look: Option<String>) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    // With a saved look: its colours, and the edit keeps following it.
    let saved = look.as_deref().map(|n| store.find_look(n).cloned().ok_or_else(|| format!("There's no saved look called “{n}” any more"))).transpose()?;
    let style = saved.as_ref().map(|l| l.style.clone()).or(style).ok_or("Pick a look first")?;
    let mut rule = custom_rule(&item, &style, &opts.unwrap_or_default());
    if let Some(l) = &saved { rule.use_look(&l.name, &l.style); }
    let label = rule_label(&rule);
    remember(&state, &store, &id, &label);
    add_rule(&mut store.profiles.get_mut(&id).ok_or("Profile not found")?.rules, rule);
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
}

/// F2 with "All <group>" picked (23 Nov, he chose 2A): the change goes to the rule already in charge, the NeverSink
/// tier the Tiers / Map tiers pages show, instead of a new edit on top, so the page and the game always match.
/// `show` false hides the whole tier; `style` (or the saved look `look`) restyles it.
#[tauri::command]
fn apply_to_tier(state: State<AppState>, type_path: Vec<String>, tier: String, show: bool, style: Option<CustomStyle>,
                 look: Option<String>) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a profile first")?;
    let saved = look.as_deref().map(|n| store.find_look(n).cloned().ok_or_else(|| format!("There's no saved look called “{n}” any more"))).transpose()?;
    let style = saved.as_ref().map(|l| l.style.clone()).or(style);
    let label = format!("{} (whole group)", filter_core::tiers::tier_name(&tier));
    remember(&state, &store, &id, &label);
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    tier_edit(&mut p.rules, &type_path, &tier, Some(show), if show { style.as_ref() } else { None }, None, None, false);
    if let (Some(l), true) = (&saved, show) {
        if let Some(Rule::Tier { look, .. }) = p.rules.iter_mut().rev()
            .find(|r| matches!(r, Rule::Tier { type_path: tp, tier: t, .. } if *tp == type_path && *t == tier)) {
            *look = Some(l.name.clone());
        }
    }
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

/// The Themes page's cards: the same drops through this filter with every theme, so the colours are the real ones.
#[tauri::command(async)]
fn theme_cards(state: State<AppState>, id: String) -> Result<filter_core::preview::ThemeCards, String> {
    // Copies, so the app isn't held up while it works through every theme.
    let (store, game) = {
        let store = state.store.lock().unwrap();
        (store.clone(), store.profiles.get(&id).ok_or("Profile not found")?.game)
    };
    let prices = state.prices.lock().unwrap().get(&game).cloned();
    let level = if game == Game::Poe1 { 83 } else { 80 };
    filter_core::preview::theme_cards(&store, &id, &filter_dir(game), prices.as_ref(), level).map_err(err)
}

/// Item pictures for the active filter's game: name -> picture URL.
#[tauri::command]
fn get_icons(state: State<AppState>) -> std::collections::HashMap<String, String> {
    let game = active_game(&state);
    let mut m = state.icons.lock().unwrap().get(&game).cloned().unwrap_or_default();
    if game == Game::Poe1 { for (k, v) in learned_icons(&state) { m.entry(k).or_insert(v); } }
    m
}

/// Pictures poe.ninja doesn't have: Originator maps (the game marks their picture; 26 Nov he asked the price window to
/// show it). The T16 one is from his own map; other tiers are learnt from his bags as he picks them up.
const ORIGINATOR_T16: &str = "https://web.poecdn.com/gen/image/WzI4LDE0LHsiZiI6IjJESXRlbXMvTWFwcy9BdGxhczJNYXBzL05ldy9NYXBOdW1iZXJzMTYiLCJ3IjoxLCJoIjoxLCJzY2FsZSI6MSwibW4iOjI0LCJtdCI6MCwibW0iOnRydWV9XQ/29434e838e/MapNumbers16.png";

fn learned_icons(state: &AppState) -> std::collections::HashMap<String, String> {
    let mut m: std::collections::HashMap<String, String> = std::fs::read_to_string(state.data_dir.join("learned_icons.json")).ok()
        .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    m.entry("Originator Map (Tier 16)".into()).or_insert_with(|| ORIGINATOR_T16.into());
    m
}

/// Remembers the pictures of special maps seen in his bags ("Originator Map (Tier 14)" → its URL).
pub(crate) fn learn_icons(app: &AppHandle, seen: Vec<(String, String)>) {
    let state = app.state::<AppState>();
    let mut m = learned_icons(&state);
    let mut changed = false;
    for (name, url) in seen {
        if name.starts_with("Originator ") && url.starts_with("https://") && m.get(&name) != Some(&url) { m.insert(name, url); changed = true; }
    }
    if changed { if let Ok(t) = serde_json::to_string(&m) { let _ = std::fs::write(state.data_dir.join("learned_icons.json"), t); } }
}

/// Files of the app that were damaged when it started, and what was done about it.
#[tauri::command]
fn file_trouble() -> Vec<String> { filter_core::keep::trouble() }

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
    store.save(&state.store_path).map_err(err)?;
    rebuild_with_children(&state, &store, &[id]);
    Ok(())
}

/// Opens a saved edit for changing. With no index, finds the active
/// profile's newest edit with this label (used by the in-game pop-up).
/// `inherited`: also look in the filters this one is built on (Modify this rule, 26 Nov: his map edit was in
/// Red-Maps while he played Blasting, and it said "That edit lives in another profile"). The 4th value is the
/// name of the filter the edit was found in, when that isn't the one asked about.
#[tauri::command]
fn get_rule(state: State<AppState>, id: Option<String>, index: Option<usize>, label: Option<String>, inherited: Option<bool>)
    -> Result<(String, usize, RuleDetail, Option<String>), String> {
    let store = state.store.lock().unwrap();
    let id = id.or_else(|| store.active.clone()).ok_or("Set up a profile first")?;
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let (home, index) = match (index, label) {
        (Some(i), _) => (p, i),
        (None, Some(l)) => {
            let mut cur = Some(p);
            let mut found = None;
            for _ in 0..20 {
                let Some(c) = cur else { break };
                if let Some(i) = c.rules.iter().rposition(|r| rule_label(r) == l) { found = Some((c, i)); break; }
                if !inherited.unwrap_or(false) { break; }
                cur = c.parent.as_deref().and_then(|pid| store.profiles.get(pid));
            }
            found.ok_or("That edit lives in another profile")?
        }
        _ => return Err("Which edit?".into()),
    };
    let detail = home.rules.get(index).and_then(rule_detail).ok_or("This edit can't be opened")?;
    let other = (home.id != id).then(|| home.name.clone());
    Ok((home.id.clone(), index, detail, other))
}

/// Saves changes to an edit, keeping its place in the list.
#[tauri::command]
fn update_rule(state: State<AppState>, id: String, index: usize, edit: RuleEdit) -> Result<ActionResult, String> {
    let mut store = state.store.lock().unwrap();
    let old = store.profiles.get(&id).ok_or("Profile not found")?.rules.get(index).ok_or("That edit no longer exists")?;
    let mut new = edited_rule(old, &edit).ok_or("This edit can't be changed")?;
    if let Some(l) = edit.look.as_deref().filter(|_| edit.show).and_then(|n| store.find_look(n)) { new.use_look(&l.name, &l.style); }
    let label = rule_label(&new);
    remember(&state, &store, &id, &format!("Changed: {label}"));
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    p.rules[index] = new;
    store.save(&state.store_path).map_err(err)?;
    // Filters built on this one take the change too (the edit may be in the filter the active one is built on).
    let warnings = rebuild_with_children(&state, &store, &[id]);
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

/// F2's "Reset to NeverSink's" (24 Nov): what it would undo for this item, and how NeverSink alone shows it.
#[derive(Serialize)]
struct ResetReply { what: Vec<String>, note: Option<String>, after: String,
    /// The filter this one starts from (NeverSink's, or your own imported one), by its file name: what Reset goes back to.
    base: String }

/// Undoes the changes that decide how this item looks, so NeverSink's own rule does: your edit for it (or the next
/// one down), your change to the NeverSink step in charge (and the item moved within that section), and changes
/// inherited from the filter this one is built on (by adding a change here that puts NeverSink's step back).
/// `apply: false` only says what it would do.
#[tauri::command(async)]
fn reset_item(app: AppHandle, item: Item, from_screen: bool, apply: bool) -> Result<ResetReply, String> {
    let state = app.state::<AppState>();
    let level = area_level(&app);
    let fallback = fallback_area_level(&app);
    let mut store = state.store.lock().unwrap();
    let id = store.active.clone().ok_or("Set up a filter first")?;
    let game = store.profiles.get(&id).ok_or("Profile not found")?.game;
    let dir = filter_dir(game);
    let prices = state.prices.lock().unwrap();
    let file = prices.get(&game);
    let saved = store.profiles[&id].rules.clone();
    let parent = store.profiles[&id].parent.clone();
    let inherited = parent.as_deref().map(|pid| store.resolved_rules(pid)).unwrap_or_default();
    let verdict = |store: &Store| -> Result<filter_core::evaluate::Verdict, String> {
        let (text, _) = render_profile(store, &id, &dir, file, None).map_err(err)?;
        Ok(status_with_fallback(&parse_filter(&text), &item, level, fallback, from_screen).verdict)
    };
    // NeverSink's own steps (theme applied, no edits at all), for putting a step back.
    let orig = {
        let p = store.profiles.get_mut(&id).unwrap();
        let (rules, par) = (std::mem::take(&mut p.rules), p.parent.take());
        let r = render_profile(&store, &id, &dir, None, None).map(|(t, _)| parse_filter(&t));
        let p = store.profiles.get_mut(&id).unwrap();
        p.rules = rules; p.parent = par;
        r.map_err(err)?
    };
    let mut what = vec![];
    let mut note = None;
    for _ in 0..6 {
        let v = verdict(&store)?;
        let src = v.source.clone();
        let own = &mut store.profiles.get_mut(&id).unwrap().rules;
        match src.kind.as_str() {
            "edit" | "preset" => {
                let name = src.label.split_once(": ").map(|x| x.1).unwrap_or(&src.label).to_string();
                if let Some(i) = own.iter().position(|r| matches!(r, Rule::Override { label, .. } if *label == src.label)) {
                    own.remove(i);
                    what.push(format!("your edit “{name}”"));
                    continue;
                }
                if inherited.iter().any(|r| matches!(r, Rule::Override { label, .. } if *label == src.label)) {
                    let pname = parent.as_deref().and_then(|p| store.profiles.get(p)).map(|p| p.name.clone()).unwrap_or_default();
                    note = Some(format!("“{name}” is an edit in {pname}, the filter this one is built on: remove it there (My edits)."));
                }
                break;
            }
            "neversink" => {
                let tier = src.tier.clone().unwrap_or_default();
                let step = format!("{} › {}", filter_core::tiers::section_title(&src.type_path).unwrap_or_else(|| src.type_path.join(" › ")),
                    filter_core::tiers::step_name(&src.type_path.join("->"), &tier));
                let mine = filter_core::rules::touching(own, &src.type_path, &tier, &item.base_type);
                for i in mine.iter().rev() { own.remove(*i); }
                let back = filter_core::rules::restore(&orig, &inherited, &src.type_path, &tier, &item.base_type);
                if !mine.is_empty() || !back.is_empty() { what.push(format!("your change to {step}")); }
                own.extend(back);
                break;
            }
            _ => {
                if src.kind == "price" {
                    // A worth line isn't a price tier (24 Nov: he was sent to Price tiers and switched those off).
                    note = Some(match &src.section {
                        Some(s) => format!("The worth line on Tiers › {s} decides this one (Modify this rule opens it)."),
                        None => "Price tiers decide this one: change it on the Price tiers page.".into(),
                    });
                }
                break;
            }
        }
    }
    let after = verdict(&store)?;
    // Named after the filter it starts from: not always NeverSink's (24 Nov, he may use his own).
    let base = std::path::Path::new(&store.profiles[&id].base_file).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
    let after = if after.visibility == "Hide" { "hidden" } else { "shown" }.to_string();
    let new_rules = std::mem::replace(&mut store.profiles.get_mut(&id).unwrap().rules, saved);
    drop(prices);
    if apply && !what.is_empty() {
        remember(&state, &store, &id, &format!("Reset {} to NeverSink's", item.base_type));
        store.profiles.get_mut(&id).unwrap().rules = new_rules;
        save_and_build(&state, &store, &id)?;
    }
    Ok(ResetReply { what, note, after, base })
}

#[derive(Serialize)]
struct StatusReply { status: Status, area_level: Option<u32>, game_filter: Option<GameFilter>, stackable: bool }

#[derive(Serialize)]
struct PriceInfo { league: String, updated: String, unit: String, count: usize }

#[derive(Serialize)]
struct PricePage { settings: PriceSettings, info: Option<PriceInfo>, game: Game }

/// Tiers page "Hide if worth under": how many of this group's priced items (one per base) are under the line.
/// Cards that don't count because too few sell: (name, value, sold), dearest first.
#[tauri::command]
fn price_thin_cards(state: State<AppState>, id: String, min_sold: f64) -> Result<Vec<(String, f64, f64)>, String> {
    let store = state.store.lock().unwrap();
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    Ok(state.prices.lock().unwrap().get(&p.game).map(|f| filter_core::prices::thin_cards(f, min_sold)).unwrap_or_default())
}

#[tauri::command]
fn price_count_below(state: State<AppState>, id: String, group: filter_core::prices::Group, limit: f64) -> Result<(usize, usize), String> {
    let store = state.store.lock().unwrap();
    let p = store.profiles.get(&id).ok_or("Profile not found")?;
    let prices = state.prices.lock().unwrap();
    let Some(file) = prices.get(&p.game) else { return Ok((0, 0)) };
    let min = p.prices.as_ref().and_then(|s| s.groups.iter().find(|g| g.group == group)).map_or(filter_core::prices::THIN_SOLD, |g| g.really_sells());
    let values = filter_core::prices::values_by_base_at(file, group, min);
    Ok((values.values().filter(|v| **v < limit).count(), values.len()))
}

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

/// Every unique that drops on this base, with poe.ninja prices (for unidentified / ground-read uniques).
#[tauri::command]
fn base_uniques(state: State<AppState>, base: String) -> Vec<(String, f64)> {
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    prices.get(&game).map(|f| { let k = chaos_per_unit(f); filter_core::prices::uniques_on_base(f, &base).into_iter().map(|(n, v)| (n, v * k)).collect() }).unwrap_or_default()
}

/// Awakened-style "related items" for the Ctrl+D window: a fragment's set or parts, and the boss's drops, with prices.
#[tauri::command]
fn related_items(state: State<AppState>, name: String) -> Option<filter_core::related::Related> {
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    if game != Game::Poe1 { return None } // the data is PoE 1 only
    filter_core::related::related(prices.get(&game)?, &name)
}

/// Crafting page: base names for the search box.
#[tauri::command]
fn craft_bases() -> Vec<String> { filter_core::craft::base_names() }

/// Crafting page: a copied item's base, item level, influences and mods.
#[tauri::command]
fn craft_start(text: String) -> Option<filter_core::craft::Start> { filter_core::craft::start(&text) }

/// Crafting page: what can roll on a base, with essence / bench / fossil options and Chaos Orb odds.
#[tauri::command]
async fn craft_pool(state: State<'_, AppState>, base: String, ilvl: u32, influences: Vec<String>, have: Vec<String>) -> Result<Option<filter_core::craft::Pool>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::pool(&base, ilvl, &influences, &have, &price)).await.map_err(|e| e.to_string())
}

/// Crafting page: the ways to get one mod, cheapest first.
#[tauri::command]
async fn craft_ways(state: State<'_, AppState>, base: String, ilvl: u32, influences: Vec<String>, key: String, min_tier: u32, have: Vec<String>, rarity: String) -> Result<Vec<filter_core::craft::Way>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::ways(&base, ilvl, &influences, &key, min_tier, &have, &rarity, &price)).await.map_err(|e| e.to_string())
}

/// Crafting page: every route to make an item with these mods (lock, essence, magic, fracture, Eldritch…), cheapest first.
#[tauri::command]
async fn craft_routes(state: State<'_, AppState>, base: String, ilvl: u32, influences: Vec<String>, targets: Vec<(String, u32)>, have: Vec<String>) -> Result<Option<filter_core::craft::routes::Routes>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::routes::routes(&base, ilvl, &influences, &targets, &have, &price)).await.map_err(|e| e.to_string())
}

/// Crafting page, "What now?": the copied item against the plan: the next click, what's left to spend, warnings.
#[tauri::command]
async fn craft_now(state: State<'_, AppState>, text: String, targets: Vec<(String, u32)>) -> Result<Option<filter_core::craft::now::WhatNow>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::now::what_now(&text, &targets, &price)).await.map_err(|e| e.to_string())
}

/// Crafting page: a stash search that lights up the item when a planned mod rolls (craft in your stash tab).
#[tauri::command]
async fn craft_search(state: State<'_, AppState>, base: String, ilvl: u32, influences: Vec<String>, targets: Vec<(String, u32)>, all: bool) -> Result<Option<filter_core::craft::search::CraftSearch>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::pool(&base, ilvl, &influences, &[], &price)
        .map(|p| filter_core::craft::search::craft_search(&p, &targets, all))).await.map_err(|e| e.to_string())
}

/// Every mod a regular jewel can roll (Tiers → Jewels → "Jewels with my mods").
#[tauri::command]
fn jewel_mods() -> Vec<filter_core::craft::JewelMod> {
    filter_core::craft::jewel_mods()
}

/// Crafting page: what an item like the copied one costs on the trade site (the same search as Ctrl+D), to compare
/// with crafting it. One search, only when you click.
#[tauri::command(async)]
fn craft_buy(app: AppHandle, text: String) -> Result<TradeResult, String> {
    let (poe2, _) = trade_league(&app);
    let index = trade_api::stats(&app.state::<AppState>().data_dir, poe2)?;
    let setup = filter_core::trade::setup(&text, &index).ok_or("That doesn't look like an item")?;
    trade_search(app, setup)
}

/// Crafting page with no copied item: what an item with the planned mods costs to buy (each mod at its chosen tier's
/// lowest roll or better), to compare with crafting it.
#[tauri::command(async)]
fn craft_buy_plan(app: AppHandle, base: String, ilvl: u32, influences: Vec<String>, targets: Vec<(String, u32)>) -> Result<TradeResult, String> {
    let price = |_: &str| None;
    let pool = filter_core::craft::pool(&base, ilvl, &influences, &[], &price).ok_or("Unknown base")?;
    let text = filter_core::craft::search::plan_item_text(&pool, &targets);
    let (poe2, _) = trade_league(&app);
    let index = trade_api::stats(&app.state::<AppState>().data_dir, poe2)?;
    let mut setup = filter_core::trade::setup(&text, &index).ok_or("Couldn't make a search for these mods")?;
    // Every planned mod counts, at least the tier's lowest roll.
    for f in setup.filters.iter_mut() { if !f.junk { f.enabled = true; f.min = Some(f.value); } }
    trade_search(app, setup)
}

/// Crafting page calculator: the chance each method gives several mods at once.
#[tauri::command]
async fn craft_plan(state: State<'_, AppState>, base: String, ilvl: u32, influences: Vec<String>, targets: Vec<(String, u32)>) -> Result<Vec<filter_core::craft::Way>, String> {
    let f = state.prices.lock().unwrap().get(&Game::Poe1).cloned();
    let price = move |n: &str| f.as_ref().and_then(|f| price_of(f, None, n, ""));
    tauri::async_runtime::spawn_blocking(move || filter_core::craft::plan(&base, ilvl, &influences, &targets, &price)).await.map_err(|e| e.to_string())
}

/// What poe.ninja says this item is worth (for the pop-up).
#[tauri::command]
fn item_price(state: State<AppState>, item: Item) -> Option<(f64, String, Option<f64>)> {
    let game = active_game(&state);
    let prices = state.prices.lock().unwrap();
    let f = prices.get(&game)?;
    // A unique whose versions differ: the cheapest, and the most it could be.
    let high = item.name.as_deref().filter(|_| item.rarity == "Unique").and_then(|n| filter_core::prices::unique_price(f, n, &item.base_type, None)).and_then(|u| u.high);
    price_of(f, item.name.as_deref(), &item.base_type, &item.rarity).map(|v| (v, f.unit.clone(), high))
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

/// Rebuilds these filters and every filter that builds on one of them.
fn rebuild_with_children(state: &AppState, store: &Store, changed: &[String]) -> Vec<String> {
    let ids: Vec<String> = store.profiles.keys().filter(|id| {
        let mut cur = store.profiles.get(*id);
        let mut n = 0;
        while let Some(p) = cur {
            if changed.contains(&p.id) { return true; }
            n += 1;
            if n > 20 { break; }
            cur = p.parent.as_deref().and_then(|pid| store.profiles.get(pid));
        }
        false
    }).cloned().collect();
    ids.iter().flat_map(|id| save_and_build(state, store, id).unwrap_or_else(|e| vec![e])).collect()
}

#[derive(Serialize)]
struct LookInfo { name: String, style: CustomStyle, used_by: usize }

/// Your saved looks ("My chase look"), with how many edits use each.
#[tauri::command]
fn list_looks(state: State<AppState>) -> Vec<LookInfo> {
    let store = state.store.lock().unwrap();
    store.looks.iter().map(|l| LookInfo { name: l.name.clone(), style: l.style.clone(), used_by: store.look_uses(&l.name) }).collect()
}

/// Saves a look (new, changed, or renamed from `old_name`); every edit using it changes too.
#[tauri::command]
fn save_look(state: State<AppState>, look: SavedLook, old_name: Option<String>) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    let changed = store.save_look(look, old_name.as_deref())?;
    store.save(&state.store_path).map_err(err)?;
    Ok(rebuild_with_children(&state, &store, &changed))
}

/// Deletes a saved look. Edits that used it keep their colours.
#[tauri::command]
fn delete_look(state: State<AppState>, name: String) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
    store.delete_look(&name);
    store.save(&state.store_path).map_err(err)
}

/// Switches an edit off (kept in the list, left out of the filter) or back on.
#[tauri::command]
fn set_rule_off(state: State<AppState>, id: String, index: usize, off: bool) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    let label = rule_label(store.profiles.get(&id).ok_or("Profile not found")?.rules.get(index).ok_or("That edit no longer exists")?);
    remember(&state, &store, &id, &format!("{}: {label}", if off { "Switched off" } else { "Switched on" }));
    store.profiles.get_mut(&id).ok_or("Profile not found")?.rules[index].set_off(off);
    store.save(&state.store_path).map_err(err)?;
    Ok(rebuild_with_children(&state, &store, &[id]))
}

/// Points an edit at a saved look (it takes that look now and follows later changes), or with no look stops following one.
#[tauri::command]
fn set_rule_look(state: State<AppState>, id: String, index: usize, look: Option<String>) -> Result<Vec<String>, String> {
    let mut store = state.store.lock().unwrap();
    let saved = look.as_deref().map(|n| store.find_look(n).cloned().ok_or_else(|| format!("There's no saved look called “{n}” any more"))).transpose()?;
    let label = rule_label(store.profiles.get(&id).ok_or("Profile not found")?.rules.get(index).ok_or("That edit no longer exists")?);
    remember(&state, &store, &id, &format!("Look changed: {label}"));
    let r = &mut store.profiles.get_mut(&id).ok_or("Profile not found")?.rules[index];
    match &saved {
        Some(l) => if !r.use_look(&l.name, &l.style) { return Err("A hidden edit has no look to change".into()); },
        None => r.drop_look(),
    }
    store.save(&state.store_path).map_err(err)?;
    Ok(rebuild_with_children(&state, &store, &[id]))
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
    // The league is changed on its own (league_set): the page's copy may be older. So are the earnings tabs (they move).
    store.settings = Settings { fallback_area_level: settings.fallback_area_level.clamp(1, 86), leagues: old.leagues.clone(),
        wealth_tabs: old.wealth_tabs.clone(), ..settings };
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
    // The small in-game window says it's on and counts what you copy (no focus taken: you keep playing).
    if let Some(w) = app.get_webview_window("batch") {
        if on {
            let count = BATCH_LIST.lock().unwrap().len();
            let _ = app.emit_to("batch", "batch-count", BatchCount { count, last: String::new(), added: false });
            crate::win_place::show_over_game(&app, &w);
        } else { let _ = w.hide(); }
    }
}

/// Batch mode's collected items (the Batch page reads them when it opens).
#[tauri::command]
fn batch_list() -> Vec<Item> { BATCH_LIST.lock().unwrap().clone() }

/// Takes items off the collected list (by rarity|base|name key); none = clear it all.
#[tauri::command]
fn batch_forget(app: AppHandle, keys: Option<Vec<String>>) {
    let count = {
        let mut l = BATCH_LIST.lock().unwrap();
        match keys { Some(k) => l.retain(|i| !k.contains(&batch_key(i))), None => l.clear() }
        l.len()
    };
    let _ = app.emit("batch-count", BatchCount { count, last: String::new(), added: false });
}

// ---------- startup ----------

/// Everything the app keeps, loaded before any window opens (so a window can never
/// ask for it too early). Same folder Tauri uses: <AppData>/com.grobbs.poefiltereditor.
#[cfg(debug_assertions)]
async fn tokio_sleep(secs: u64) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(Duration::from_secs(secs))).await;
}

fn load_state() -> AppState {
    let data = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("com.grobbs.poefiltereditor");
    // Development builds only: a test copy can run beside his installed editor with its own settings folder.
    #[cfg(debug_assertions)]
    let data = std::env::var_os("LODESTONE_DATA_DIR").map(PathBuf::from).unwrap_or(data);
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Remembers the main window's size and position between runs (the pop-ups place themselves).
        // Size, position and maximised only: not the frame (it would bring back the Windows title bar under our own,
        // 21 Nov), nor visibility (win_place decides how the window opens).
        .plugin(tauri_plugin_window_state::Builder::default()
            .with_state_flags(tauri_plugin_window_state::StateFlags::SIZE | tauri_plugin_window_state::StateFlags::POSITION
                | tauri_plugin_window_state::StateFlags::MAXIMIZED)
            .with_denylist(&["overlay", "build", "trade", "trades", "earnings", "atlas", "batch", "level", "tracker", "searches", "sheet", "notes", "splits", "merc", "stashbox", "choice", "altar", "pricesearch"]).build())
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
                    } else if action == Some(HotAction::Atlas) {
                        atlas_app::toggle_atlas_window(app);
                    } else if let Some(HotAction::Play(label)) = action {
                        play_app::toggle(app, label);
                    } else if action == Some(HotAction::Wiki) {
                        play_app::wiki_lookup(app.clone());
                    } else if action == Some(HotAction::Editor) {
                        open_editor(app.clone(), None);
                    } else if action == Some(HotAction::Find) {
                        price_search_app::toggle(app);
                    } else if action == Some(HotAction::Read) {
                        play_app::read_choice(app.clone());
                    } else if action == Some(HotAction::Build) {
                        toggle_build_window(app);
                    } else if action == Some(HotAction::Batch) {
                        let on = !BATCH_ON.load(Ordering::Relaxed);
                        set_batch_mode(app.clone(), on);
                        // Turning it on no longer pops the editor over the game (the small batch window shows instead);
                        // turning it off with items collected opens the Batch page to choose what to do with them.
                        if !on && !BATCH_LIST.lock().unwrap().is_empty() { open_editor(app.clone(), Some("batch".into())); }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            let _ = APP.set(app.handle().clone());
            let settings = app.state::<AppState>().store.lock().unwrap().settings.clone();
            start_price_updates(app.handle().clone());
            price_tick::start(app.handle().clone());
            start_rebuild_old(app.handle().clone());
            if register_hotkeys(app.handle(), &settings).is_err() {
                // Fall back to the standard keys if saved ones can't be used.
                let _ = register_hotkeys(app.handle(), &Settings::default());
            }
            start_batch_watcher(app.handle().clone());
            start_hotkey_focus_watcher(app.handle().clone());
            start_whisper_watcher(app.handle().clone());
            splits_app::start(app.handle().clone());
            play_app::start_watchers(app.handle().clone());
            phone_app::start_shop_watch(app.handle().clone());
            phone_app::start_page(app.handle());
            start_tree_watcher(app.handle().clone());
            start_live_searches(app.handle().clone());
            wealth_app::start_session_timer(app.handle().clone());
            win_place::start_main(app.handle());
            updates::start(app.handle().clone());
            // Development builds only: open "Log in with pathofexile.com" by itself, the way the button does (an async
            // command, off the main thread), to check the window loads and the app stays responsive.
            #[cfg(debug_assertions)]
            if std::env::var_os("LODESTONE_TEST_LOGIN").is_some() {
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio_sleep(3).await;
                    eprintln!("[login-test] poe_login -> {:?}", wealth_app::poe_login(h.clone()).await);
                    for _ in 0..6 {
                        tokio_sleep(3).await;
                        let url = h.get_webview_window("poelogin").and_then(|w| w.url().ok()).map(|u| u.to_string());
                        eprintln!("[login-test] login window url: {url:?}");
                    }
                });
            }

            // Tray icon: the app lives here so hotkeys work while you play.
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            use tauri::menu::PredefinedMenuItem;
            let open = MenuItem::with_id(app, "open", "Open editor", true, None::<&str>)?;
            let update = MenuItem::with_id(app, "update", "Check for updates", true, None::<&str>)?;
            let restart = MenuItem::with_id(app, "restart", "Restart editor", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &update, &restart, &sep, &quit])?;
            // The updater renames "Check for updates" when one is waiting ("Update to 0.2.8 and restart").
            app.manage(updates::TrayItem(update));
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("PoE Lodestone")
                .menu(&menu)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.unminimize();
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => { win_place::on_quit(app); app.exit(0) }
                    // He asked for it: it comes back in front.
                    "restart" => { win_place::mark_front(app); app.restart(); }
                    "update" => updates::from_tray(app.clone()),
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            win_place::on_window_event(window, event);
            // Closing the main window just hides it; the hotkeys keep working.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            price_build_item, buy_build_item, buy_base_item, buy_build_merc, price_build_merc, trade_untick_one, trade_hidden, overview,
            craft_bases, craft_start, craft_pool, craft_ways, craft_plan, craft_routes, craft_now, craft_buy, craft_search, craft_buy_plan, jewel_mods,
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
            set_batch_mode, batch_list, batch_forget,
            get_price_settings,
            set_price_settings,
            refresh_prices,
            item_price, base_uniques, related_items,
            list_themes,
            set_profile_theme,
            set_base,
            save_theme,
            delete_theme,
            list_looks,
            save_look,
            delete_look,
            set_rule_off,
            set_rule_look,
            export_pack,
            save_pack_file,
            read_pack,
            import_pack,
            get_settings,
            league_app::league_get,
            league_app::league_set,
            save_settings,
            search_items,
            list_farm_presets,
            apply_farm_preset,
            list_level_presets,
            get_tiers, only_show_stack, get_threads, reset_item, get_rule_section, get_map_tiers, set_tiers, build_tree_opt, set_build_tree, trade_search, trade_bulk, map_dangers, map_suggest, map_tool_get, map_tool_save, map_tool_regex, map_tool_stats, file_trouble, map_regex_copy, map_tool_trade, trade_action, hide_trades_window, wealth_app::wealth_get, bulk_app::bulk_get, bulk_app::bulk_basket, bulk_app::bulk_live, wealth_app::wealth_setup, wealth_app::wealth_prefs, wealth_app::wealth_scan, wealth_app::wealth_profile_save, wealth_app::wealth_profile_delete, wealth_app::wealth_profile_use, wealth_app::wealth_tabs_refresh, wealth_app::wealth_price_rares, wealth_app::wealth_find, wealth_app::wealth_count_rares, wealth_app::wealth_guild, updates::update_status, open_editor, editor_to_game, price_count_below, price_thin_cards, updates::update_install, wealth_app::session_start, wealth_app::session_check, wealth_app::session_pause, wealth_app::session_stop, wealth_app::session_delete, wealth_app::earnings_overlay, saved_list, saved_add, saved_delete, saved_live, report_problem, send_whisper, copy_text, hide_trade_window, keep_on_screen,
            move_tier_item,
            stack_choices,
            gear_setup,
            pob_import,
            pob_list,
            pob_switch,
            pob_rename,
            pob_remove,
            pob_get,
            pob_set_stage,
            pob_follow_now,
            pob_set_follow,
            hide_build_window,
            atlas_app::atlas_list, atlas_app::atlas_import, atlas_app::atlas_rename, atlas_app::atlas_delete, atlas_app::atlas_use,
            atlas_app::atlas_own_notes, atlas_app::atlas_official_link, atlas_app::hide_atlas_window, atlas_app::atlas_results,
            atlas_app::atlas_optimise, atlas_app::atlas_goals, atlas_app::atlas_save_plan, atlas_app::atlas_save_tree,
            play_app::level_view, play_app::level_move, play_app::level_jump_act, play_app::level_options, play_app::play_view,
            farming_app::farm_now, farming_app::farm_start, farming_app::farm_costs, farming_app::farm_use_tree, farming_app::farm_filter, farming_app::farm_stop,
            deaths_app::deaths_get, deaths_app::deaths_avoid, deaths_app::deaths_for,
            tonight_app::tonight_get,
            bases_app::base_worth, bases_app::bases_get, bases_app::bases_save, worn_app::worn_compare, worn_app::worn_refresh, worn_app::worn_set, worn_app::upgrade_plans, worn_app::upgrade_search,
            play_app::maps_done, play_app::maps_history, play_app::maps_clear, play_app::watch_action, play_app::searches_get,
            play_app::searches_save, play_app::search_type, play_app::stash_highlight, play_app::sheets_list, play_app::sheets_add, play_app::lab_layout, play_app::show_lab,
            play_app::sheets_remove, play_app::notes_get, play_app::notes_save, play_app::play_toggle, play_app::play_hide,
            play_app::trade_price_check, play_app::show_stash_spot, play_app::hide_stash_spot, play_app::alerts_get, play_app::alerts_save, play_app::play_prefs, play_app::play_prefs_get, play_app::altar_auto_get, play_app::altar_auto_set, play_app::price_names,
            play_app::chat_prefs, play_app::chat_get, splits_app::splits_view, splits_app::splits_hide, merc_app::merc_view, merc_app::merc_remove, merc_app::merc_details, merc_app::merc_recruit_details, merc_app::merc_hide, phone_app::phone_get, phone_app::phone_save, phone_app::phone_test, phone_app::phone_check_shop, phone_app::phone_new_key, play_app::pause_setup, play_app::pause_prefs, play_app::tracker_over_panels, play_app::tracker_use_atlas, play_app::maps_set_atlas, play_app::loot_prefs_save, wealth_app::poe_login,
            open_rule,
            gear_picks,
            open_url,
            set_stack_tier,
            set_tier,
            set_tier_conds,
            apply_level_preset,
            ask,
            get_history,
            get_icons, apply_to_tier, price_search_app::price_search, price_search_app::price_search_hide,
            preview_drops,
            theme_cards,
            rebuilt_filters,
            farm_app::farm_find,
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
