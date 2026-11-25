//! Keeps the shared prices hourly while the owner's editor is open: GitHub drops most of its own hourly runs, so when
//! the prices are getting old this asks GitHub to run the price job (about a minute) and fetches the new file.
//! Only on a PC with `ask_github.txt` in the data folder and a signed-in GitHub command line (`gh`), so nobody else's
//! editor asks. The file keeps when it last asked and how that went. poe.ninja is still only asked by GitHub, hourly at most.

use super::*;

const MARK: &str = "ask_github.txt";
/// Prices older than this are due (the job itself skips anything younger).
const DUE: i64 = 50 * 60;
/// GitHub needs about a minute, and its file server keeps the old copy for up to five.
const READY: Duration = Duration::from_secs(7 * 60);

fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

/// "2026-11-18T02:17:00Z" as unix seconds.
fn unix(t: &str) -> Option<i64> {
    let n: Vec<i64> = t.trim_end_matches('Z').split(['-', 'T', ':']).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let [y, m, d, h, min, s] = n[..] else { return None };
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let days = y * 365 + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) + (m * 153 + 2) / 5 + d - 1 - 719468;
    Some(days * 86400 + h * 3600 + min * 60 + s)
}

/// Is it time to ask? `last` is when it last asked.
fn due(updated: Option<i64>, last: i64, now: i64) -> bool {
    updated.is_some_and(|u| now - u >= DUE) && now - last >= DUE
}

/// Asks GitHub to run the price job. Quiet: no window.
fn ask() -> Result<(), String> {
    let installed = std::path::Path::new(r"C:\Program Files\GitHub CLI\gh.exe");
    let mut c = std::process::Command::new(if installed.exists() { installed } else { std::path::Path::new("gh") });
    c.args(["workflow", "run", "prices.yml", "-R", "GROBBS-hash/poe-filter-editor"]);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // CREATE_NO_WINDOW
    let o = c.output().map_err(|e| e.to_string())?;
    if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).trim().to_string()) }
}

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(90)); // the start's own price check comes first
        loop {
            let state = app.state::<AppState>();
            let mark = state.data_dir.join(MARK);
            if let Ok(text) = std::fs::read_to_string(&mark) {
                let last = text.split_whitespace().next().and_then(|w| w.parse().ok()).unwrap_or(0);
                let updated = state.prices.lock().unwrap().get(&Game::Poe1).and_then(|p| unix(&p.updated));
                if due(updated, last, now()) {
                    let r = ask();
                    let how = match &r { Ok(()) => "asked".to_string(), Err(e) => format!("couldn't ask: {e}") };
                    let _ = std::fs::write(&mark, format!("{} {how}\n", now()));
                    if r.is_ok() {
                        thread::sleep(READY);
                        take_new_prices(&app);
                    }
                }
            }
            thread::sleep(Duration::from_secs(5 * 60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_to_ask() {
        assert_eq!(unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix("2024-02-29T12:00:01Z"), Some(1709208001));
        assert_eq!(unix("now"), None);
        assert!(due(Some(0), 0, 3000));
        assert!(!due(Some(0), 0, 2999)); // prices still fresh
        assert!(!due(Some(0), 1000, 3000)); // asked already
        assert!(!due(None, 0, 9999)); // no prices loaded
    }
}
