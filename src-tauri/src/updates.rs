//! Updates for the installed editor (not the dev build, which updates through git).
//! Every few hours it asks GitHub for the latest release (latest.json, signed with the key in
//! %USERPROFILE%\.tauri), downloads a newer one in the background, and tells the window. Nothing is
//! installed until he clicks "Restart to update": it never restarts by itself while he plays.

use super::*;
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    version: String,
    current: String,
    notes: String,
}

/// A downloaded update waiting for his click.
static READY: Mutex<Option<(Update, Vec<u8>)>> = Mutex::new(None);

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo { version: u.version.clone(), current: u.current_version.clone(), notes: u.body.clone().unwrap_or_default() }
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

async fn check(app: &AppHandle) -> Result<(), String> {
    if READY.lock().unwrap().is_some() { return Ok(()); }
    let Some(u) = app.updater().map_err(err)?.check().await.map_err(err)? else { return Ok(()) };
    let bytes = u.download(|_, _| {}, || {}).await.map_err(err)?;
    let i = info(&u);
    *READY.lock().unwrap() = Some((u, bytes));
    let _ = app.emit("update-ready", i);
    Ok(())
}

/// The update waiting to be installed, if any (the window asks when it opens).
#[tauri::command]
pub fn update_status() -> Option<UpdateInfo> { READY.lock().unwrap().as_ref().map(|(u, _)| info(u)) }

/// "Restart to update": installs the downloaded version and starts it.
#[tauri::command]
pub fn update_install(app: AppHandle) -> Result<(), String> {
    let Some((u, bytes)) = READY.lock().unwrap().take() else { return Err("No update downloaded".into()) };
    u.install(bytes).map_err(err)?;
    app.restart();
}
