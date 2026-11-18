//! Works out the current area level from the game's own log file
//! (Client.txt), which logs "Generating level 83 area ..." on every zone.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The level of the most recent zone in a chunk of log text.
pub fn last_area_level(log: &str) -> Option<u32> {
    log.lines().rev().find_map(|l| {
        let rest = &l[l.find("Generating level ")? + "Generating level ".len()..];
        rest.split_whitespace().next()?.parse().ok()
    })
}

/// Reads just the end of the log (it can be hundreds of MB).
pub fn read_area_level(log_path: &Path) -> Option<u32> {
    let mut f = std::fs::File::open(log_path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(512 * 1024);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    last_area_level(&String::from_utf8_lossy(&buf))
}

/// Likely places for the game's log: Steam libraries and the standalone install.
pub fn find_log(poe2: bool, extra_steam_libraries: &[PathBuf]) -> Option<PathBuf> {
    let game = if poe2 { "Path of Exile 2" } else { "Path of Exile" };
    let mut roots: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
    ];
    // Other Steam library folders (e.g. D:\SteamLibrary) are listed here.
    for s in roots.clone() {
        if let Ok(vdf) = std::fs::read_to_string(s.join("steamapps").join("libraryfolders.vdf")) {
            for line in vdf.lines() {
                let t = line.trim();
                if let Some(rest) = t.strip_prefix("\"path\"") {
                    let p = rest.trim().trim_matches('"').replace("\\\\", "\\");
                    roots.push(PathBuf::from(p));
                }
            }
        }
    }
    roots.extend(extra_steam_libraries.iter().cloned());
    let mut candidates: Vec<PathBuf> = roots
        .iter()
        .map(|r| r.join("steamapps").join("common").join(game).join("logs"))
        .collect();
    for pf in [r"C:\Program Files (x86)\Grinding Gear Games", r"C:\Program Files\Grinding Gear Games"] {
        candidates.push(Path::new(pf).join(game).join("logs"));
    }
    candidates
        .into_iter()
        .flat_map(|d| [d.join("LatestClient.txt"), d.join("Client.txt")])
        .filter(|p| p.is_file())
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// Which filter the game has selected, from its settings file (it saves
/// this when you change filters or close the game).
pub fn selected_filter(config_text: &str) -> Option<String> {
    config_text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "item_filter").then(|| v.trim().to_string()).filter(|v| !v.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_latest_zone() {
        let log = "2026/11/18 10:01:02 1234 abc [DEBUG Client 99] Generating level 45 area \"1_3_5\" with seed 1\n\
                   2026/11/18 10:05:00 1234 abc [INFO Client 99] : You have entered The Docks.\n\
                   2026/11/18 10:20:02 1234 abc [DEBUG Client 99] Generating level 81 area \"MapWorldsStrand\" with seed 9\n\
                   2026/11/18 10:21:00 1234 abc [INFO Client 99] : You have entered Strand.\n";
        assert_eq!(last_area_level(log), Some(81));
        assert_eq!(last_area_level("nothing here"), None);
    }

    #[test]
    fn reads_selected_filter() {
        let cfg = "[UI]\nitem_filter_loaded_successfully=Giga juice.filter\nitem_filter=Giga juice.filter\n";
        assert_eq!(selected_filter(cfg).as_deref(), Some("Giga juice.filter"));
    }
}
