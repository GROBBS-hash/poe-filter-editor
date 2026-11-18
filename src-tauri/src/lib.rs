mod screen;

use filter_core::{
    evaluate::{status, Status},
    filter::{parse_filter, Segment},
    area::{find_log, read_area_level, selected_filter},
    ground::{ground_item, known_names, match_base, LabelColours},
    item::{is_stackable, parse_item, resolve_magic_base, Item},
    prices::{default_settings, price_of, PriceFile, PriceSettings},
    profiles::{build_with_prices, Game, Store},
    rules::{add_rule, custom_rule, edited_rule, known_bases, locate, quick_rule, rule_detail, Action, CustomStyle, Location, MatchOpts, Rule, RuleDetail, RuleEdit},
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
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

// ---------- state ----------

struct AppState {
    store: Mutex<Store>,
    store_path: PathBuf,
    /// Latest prices per game, downloaded hourly from the app's own price cache.
    prices: Mutex<std::collections::HashMap<Game, PriceFile>>,
    data_dir: PathBuf,
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
        if prices.get(&g).map(|p| p.updated != file.updated).unwrap_or(true) {
            let _ = std::fs::write(state.data_dir.join(format!("prices_{}.json", game_key(g))), &body);
            prices.insert(g, file);
            changed.push(g);
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

/// Hourly price updates in the background.
fn start_price_updates(app: AppHandle) {
    thread::spawn(move || loop {
        let changed = fetch_prices(&app);
        if !changed.is_empty() {
            rebuild_priced(&app, &changed);
            let _ = app.emit("prices-updated", ());
        }
        thread::sleep(Duration::from_secs(60 * 60));
    });
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
}

/// No item text on the clipboard: read the ground label under the mouse instead.
fn read_from_ground(app: &AppHandle) -> Result<Picked, String> {
    let read = screen::read_label_under_cursor()?;
    if read.text.trim().is_empty() {
        return Err("Couldn't see an item label under your mouse".into());
    }
    let (live, base) = active_filters(app).ok_or("Set up a filter first")?;
    let names = known_names(&known_bases(&base));
    let m = match_base(&read.text, &names)
        .ok_or_else(|| format!("Read \"{}\" but couldn't match it to an item", read.text))?;
    let colours = read.colours.map(|(text, background)| LabelColours { text, background });
    let item = ground_item(&live, &m, colours, area_level(app).unwrap_or(83));
    Ok(Picked { item, from_screen: true, screen_text: Some(read.text), confidence: m.confidence })
}

fn show_overlay(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    if let Some(w) = app.get_webview_window("overlay") {
        if let Ok(pos) = w.cursor_position() {
            let _ = w.set_position(PhysicalPosition::new(pos.x as i32 + 12, pos.y as i32 + 12));
        }
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
            Some(item) => Ok(Picked { item, from_screen: false, screen_text: None, confidence: 1.0 }),
            None => read_from_ground(&app),
        };
        match picked {
            Ok(p) => show_overlay(&app, "item-picked", p),
            Err(e) => show_overlay(&app, "pick-failed", e),
        }
    });
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
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let name = active_filter_name(app).ok_or("Set up a filter first")?;
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
        e.text(&format!("/itemfilter {name}"))?;
        thread::sleep(Duration::from_millis(30));
        e.key(Key::Return, Direction::Click)                // send
    };
    step().map_err(err)
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
}

#[derive(Serialize)]
struct RuleInfo {
    index: usize,
    label: String,
}

fn rule_label(r: &Rule) -> String {
    match r {
        Rule::Override { label, .. } => label.clone(),
        Rule::MoveBase { type_path, base, to_tier } => {
            format!("Move {base} to {} {to_tier}", type_path.join(" > "))
        }
    }
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
                rules: p
                    .rules
                    .iter()
                    .enumerate()
                    .map(|(index, r)| RuleInfo { index, label: rule_label(r) })
                    .collect(),
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

#[tauri::command]
fn create_profile(state: State<AppState>, name: String, game: Game, base_path: String) -> Result<String, String> {
    let dir = filter_dir(game);
    let src = Path::new(&base_path);
    let file_name = src.file_name().ok_or("That isn't a file")?.to_string_lossy().to_string();
    // Keep the base next to the game's filters so rebuilds always find it.
    if src.parent() != Some(dir.as_path()) {
        std::fs::create_dir_all(&dir).map_err(err)?;
        std::fs::copy(src, dir.join(&file_name)).map_err(err)?;
    }
    let mut store = state.store.lock().unwrap();
    if store.profiles.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("You already have a profile called {name}"));
    }
    let id = store.create(&name, game, &file_name);
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
    let removed = store.profiles.get_mut(&id).ok_or("Profile not found")?.rules.pop();
    save_and_build(&state, &store, &id)?;
    Ok(removed.as_ref().map(rule_label))
}

#[tauri::command]
fn delete_rule(state: State<AppState>, id: String, index: usize) -> Result<(), String> {
    let mut store = state.store.lock().unwrap();
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
    let p = store.profiles.get_mut(&id).ok_or("Profile not found")?;
    let old = p.rules.get(index).ok_or("That edit no longer exists")?;
    let new = edited_rule(old, &edit).ok_or("This edit can't be changed")?;
    let label = rule_label(&new);
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
    let st = status(&live, &item, level, from_screen);
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

pub fn run() {
    let f2 = Shortcut::new(None, Code::F2);
    let f3 = Shortcut::new(None, Code::F3);
    let f4 = Shortcut::new(None, Code::F4);

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, sc, ev| {
                    if ev.state() != ShortcutState::Pressed {
                        return;
                    }
                    if sc == &f2 {
                        quick_edit(app.clone());
                    } else if sc == &f4 {
                        let app = app.clone();
                        thread::spawn(move || {
                            if let Err(e) = reload_in_game(&app, 0) {
                                let _ = app.emit("reload-failed", e);
                            }
                        });
                    } else if sc == &f3 {
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
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let store_path = data.join("profiles.json");
            // Start from the last downloaded prices, then keep them fresh.
            let mut cached = std::collections::HashMap::new();
            for g in [Game::Poe1, Game::Poe2] {
                if let Some(f) = std::fs::read_to_string(data.join(format!("prices_{}.json", game_key(g)))).ok()
                    .and_then(|t| serde_json::from_str::<PriceFile>(&t).ok()) {
                    cached.insert(g, f);
                }
            }
            app.manage(AppState { store: Mutex::new(Store::load(&store_path)), store_path,
                prices: Mutex::new(cached), data_dir: data.clone() });
            start_price_updates(app.handle().clone());
            app.global_shortcut().register(f2)?;
            app.global_shortcut().register(f3)?;
            app.global_shortcut().register(f4)?;
            start_batch_watcher(app.handle().clone());

            // Tray icon: the app lives here so hotkeys work while you play.
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            let open = MenuItem::with_id(app, "open", "Open editor", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("PoE Filter Editor (F2 quick edit, F3 batch, F4 reload filter)")
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
            item_price
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
