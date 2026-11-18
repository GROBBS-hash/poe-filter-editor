use filter_core::{
    filter::parse_filter,
    item::{parse_item, resolve_magic_base, Item},
    profiles::{build, Game, Store},
    rules::{known_bases, locate, quick_rule, Action, Location, MatchOpts, Rule},
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
        let Some(item) = read_item(&app, &text) else { return };
        if let Some(w) = app.get_webview_window("overlay") {
            if let Ok(pos) = w.cursor_position() {
                let _ = w.set_position(PhysicalPosition::new(pos.x as i32 + 12, pos.y as i32 + 12));
            }
            let _ = w.emit("item-picked", &item);
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
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
    build(store, id, &filter_dir(game)).map_err(err)
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
    store.profiles.get_mut(&id).ok_or("Profile not found")?.rules.push(rule);
    let warnings = save_and_build(&state, &store, &id)?;
    Ok(ActionResult { label, warnings })
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
            app.manage(AppState { store: Mutex::new(Store::load(&store_path)), store_path });
            app.global_shortcut().register(f2)?;
            app.global_shortcut().register(f3)?;
            start_batch_watcher(app.handle().clone());

            // Tray icon: the app lives here so hotkeys work while you play.
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::TrayIconBuilder;
            let open = MenuItem::with_id(app, "open", "Open editor", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("PoE Filter Editor (F2 quick edit, F3 batch)")
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
            undo_last,
            delete_rule,
            rebuild,
            locate_item,
            set_batch_mode
        ])
        .run(tauri::generate_context!())
        .expect("error while running the app");
}
