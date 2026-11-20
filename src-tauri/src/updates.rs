//! Updates for the installed editor (not the dev build, which updates through git).
//! Every few hours it asks GitHub for the latest release (latest.json, signed with the key in
//! %USERPROFILE%\.tauri), downloads a newer one in the background, and tells the window and the tray menu.
//! Nothing is installed until he asks: "Restart to update" in the window, or the tray's "Update to x and
//! restart". It never restarts by itself while he plays. The tray's "Check for updates" checks right away.

use super::*;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    version: String,
    current: String,
    notes: String,
}

/// The tray's "Check for updates" item, renamed to say what's going on.
pub struct TrayItem(pub tauri::menu::MenuItem<tauri::Wry>);

/// A downloaded update waiting for his click.
static READY: Mutex<Option<(Update, Vec<u8>)>> = Mutex::new(None);
/// A tray check is running (a second click does nothing).
static CHECKING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone().unwrap_or_default() }
}

fn tray_text(app: &AppHandle, text: &str) {
    if let Some(t) = app.try_state::<TrayItem>() { let _ = t.0.set_text(text); }
}

pub fn start(app: AppHandle) {
    if cfg!(debug_assertions) { return; }
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(20));
        loop {
            if let Err(e) = tauri::async_runtime::block_on(check(&app)) { eprintln!("update check: {e}"); }
            thread::sleep(Duration::from_secs(3 * 3600));
        }
    });
}

/// Looks for a newer version and downloads it. Ok(true) when one is ready to install.
async fn check(app: &AppHandle) -> Result<bool, String> {
    if READY.lock().unwrap().is_some() { return Ok(true); }
    let Some(u) = app.updater().map_err(err)?.check().await.map_err(err)? else { return Ok(false) };
    tray_text(app, &format!("Downloading {}…", u.version));
    let bytes = u.download(|_, _| {}, || {}).await.map_err(err)?;
    let i = info(&u);
    tray_text(app, &format!("Update to {} and restart", i.version));
    *READY.lock().unwrap() = Some((u, bytes));
    let _ = app.emit("update-ready", i);
    Ok(true)
}

/// Tray "Check for updates" (or "Update to x and restart"): installs a waiting or newly found update and
/// restarts; otherwise says it's up to date for a few seconds.
pub fn from_tray(app: AppHandle) {
    if CHECKING.swap(true, std::sync::atomic::Ordering::SeqCst) { return; }
    thread::spawn(move || {
        let current = app.package_info().version.to_string();
        if cfg!(debug_assertions) {
            tray_text(&app, "Updates come with git here (dev build)");
        } else {
            tray_text(&app, "Checking for updates…");
            match tauri::async_runtime::block_on(check(&app)) {
                Ok(true) => {
                    if let Err(e) = install(&app) { tray_text(&app, &format!("Update failed: {e}")); }
                }
                Ok(false) => tray_text(&app, &format!("Up to date ({current})")),
                Err(e) => { eprintln!("update check: {e}"); tray_text(&app, "Couldn't check: no internet?"); }
            }
        }
        thread::sleep(Duration::from_secs(6));
        if READY.lock().unwrap().is_none() { tray_text(&app, "Check for updates"); }
        CHECKING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// Installs the downloaded version and starts it, in front (he asked for it).
fn install(app: &AppHandle) -> Result<(), String> {
    let Some((u, bytes)) = READY.lock().unwrap().take() else { return Err("No update downloaded".into()) };
    crate::win_place::mark_front(app);
    u.install(bytes).map_err(err)?;
    app.restart();
}

/// The update waiting to be installed, if any (the window asks when it opens).
#[tauri::command]
pub fn update_status() -> Option<UpdateInfo> { READY.lock().unwrap().as_ref().map(|(u, _)| info(u)) }

/// "Restart to update": installs the downloaded version and starts it.
#[tauri::command]
pub fn update_install(app: AppHandle) -> Result<(), String> { install(&app) }
