//! Reading and writing the app's own files (settings, history, saved searches, stash worth…) without ever losing them.
//! (25 Nov: a file that couldn't be read counted as empty, and the next save wrote that emptiness over it.)

use serde::{de::DeserializeOwned, Serialize};
use std::{fs, io, path::{Path, PathBuf}, sync::Mutex, time::{Duration, SystemTime}};

/// What happened to damaged files since the app started, in plain words, for the main window to show.
static TROUBLE: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub fn trouble() -> Vec<String> { TROUBLE.lock().unwrap().clone() }

fn with_ext(path: &Path, ext: &str) -> PathBuf {
    let mut n = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    n.push(ext);
    path.with_file_name(n)
}

/// The kept good copy of a file ("profiles.json.good").
pub fn good_copy(path: &Path) -> PathBuf { with_ext(path, ".good") }

/// Reads a file that may be briefly held by something else (a virus scanner, a sync program).
fn read(path: &Path) -> io::Result<String> {
    let mut tries = 0;
    loop {
        match fs::read_to_string(path) {
            Err(e) if tries < 5 && !matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::InvalidData) => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(80));
            }
            r => return r,
        }
    }
}

/// Loads a file: None when there isn't one yet. A file that can't be read is set aside ("….broken-<time>") and the
/// last good copy is used instead; a good one is kept as the new good copy.
pub fn load<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = match read(path) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Gone, but there was one: a crash between two steps, or it was deleted by hand.
            let t = read(&good_copy(path)).ok()?;
            let v = serde_json::from_str(&t).ok()?;
            note(path, "was missing: the last good copy is back");
            return Some(v);
        }
        Err(_) => String::new(),
    };
    if let Ok(v) = serde_json::from_str::<T>(&text) {
        let _ = fs::copy(path, good_copy(path));
        return Some(v);
    }
    let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let aside = with_ext(path, &format!(".broken-{secs}"));
    // Copied as well as moved away from: whatever happens next, the damaged file itself stays.
    if fs::rename(path, &aside).is_err() { let _ = fs::copy(path, &aside); }
    match read(&good_copy(path)).ok().and_then(|t| serde_json::from_str::<T>(&t).ok()) {
        Some(v) => {
            note(path, "was damaged: the last good copy is back (changes since then may be missing)");
            Some(v)
        }
        None => {
            note(path, "was damaged and had no good copy: it starts empty. The damaged file is kept beside it");
            None
        }
    }
}

fn note(path: &Path, what: &str) {
    let name = match path.file_stem().and_then(|s| s.to_str()).unwrap_or("") {
        "profiles" => "Your filters and settings file", "history" => "The file with your recent changes",
        "saved_searches" => "Your saved searches file", "wealth" => "Your stash worth file", "phone" => "Your phone page file",
        "play" => "Your map history file", "atlas" => "Your atlas trees file", other => { TROUBLE.lock().unwrap().push(format!("{other} {what}.")); return; }
    };
    TROUBLE.lock().unwrap().push(format!("{name} {what}."));
}

/// Writes to a temp file, makes sure it's really on the disk, then swaps it in: a crash or power cut never leaves half a file.
pub fn save_text(path: &Path, text: &str) -> io::Result<()> {
    use io::Write;
    if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
    let tmp = with_ext(path, ".tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(tmp, path)
}

pub fn save<T: Serialize>(path: &Path, v: &T, pretty: bool) -> io::Result<()> {
    let text = if pretty { serde_json::to_string_pretty(v) } else { serde_json::to_string(v) }?;
    save_text(path, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("keep-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_damaged_file_comes_back() {
        let d = dir("damaged");
        let f = d.join("profiles.json");
        assert_eq!(load::<Vec<u32>>(&f), None, "nothing yet");
        save(&f, &vec![1u32, 2, 3], true).unwrap();
        assert_eq!(load::<Vec<u32>>(&f), Some(vec![1, 2, 3]));
        // Half a file, zeros after a power cut, empty.
        for bad in [&b"[1, 2,"[..], &[0u8; 40][..], b""] {
            fs::write(&f, bad).unwrap();
            assert_eq!(load::<Vec<u32>>(&f), Some(vec![1, 2, 3]));
            assert!(!f.exists(), "the damaged one is moved aside, so a save can't hide it");
            save(&f, &vec![1u32, 2, 3], true).unwrap();
            assert_eq!(load::<Vec<u32>>(&f), Some(vec![1, 2, 3]));
        }
        let broken: Vec<_> = fs::read_dir(&d).unwrap().filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().contains(".broken-")).collect();
        assert!(!broken.is_empty(), "kept");
        assert!(trouble().iter().any(|t| t.starts_with("Your filters and settings file was damaged")));
        // Deleted by hand: the good copy is used.
        fs::remove_file(&f).unwrap();
        assert_eq!(load::<Vec<u32>>(&f), Some(vec![1, 2, 3]));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn damaged_with_no_copy_is_kept() {
        let d = dir("nocopy");
        let f = d.join("x.json");
        fs::write(&f, "{ half").unwrap();
        assert_eq!(load::<Vec<u32>>(&f), None);
        save(&f, &vec![9u32], false).unwrap();
        let kept = fs::read_dir(&d).unwrap().filter_map(Result::ok).find(|e| e.file_name().to_string_lossy().contains(".broken-")).expect("kept");
        assert_eq!(fs::read_to_string(kept.path()).unwrap(), "{ half");
        let _ = fs::remove_dir_all(d);
    }
}
