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

/// Your character from the newest level-up line: ": Name (Class) is now level 23".
pub fn last_character(log: &str) -> Option<(String, String, u32)> {
    log.lines().rev().find_map(|l| {
        let i = l.find(" is now level ")?;
        let level: u32 = l[i + " is now level ".len()..].trim().parse().ok()?;
        let head = &l[..i];
        let start = head.rfind(" : ")? + 3;
        let who = &head[start..];
        let open = who.rfind(" (")?;
        let class = who[open + 2..].trim_end_matches(')').to_string();
        Some((who[..open].to_string(), class, level))
    })
}

/// Reads the last ~8 MB of the log for your newest level-up (they're rarer than area changes).
pub fn read_character(log_path: &Path) -> Option<(String, String, u32)> {
    let mut f = std::fs::File::open(log_path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(8 * 1024 * 1024))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    last_character(&String::from_utf8_lossy(&buf))
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

/// Where Steam is installed, as Windows records it (handles custom spots like C:\\STEAM).
#[cfg(windows)]
fn steam_from_registry() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    for (key, value) in [(r"HKCU\Software\Valve\Steam", "SteamPath"), (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath")] {
        let out = std::process::Command::new("reg")
            .args(["query", key, "/v", value])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(path) = text.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, p)| p.trim().to_string())) {
            return Some(PathBuf::from(path.replace('/', "\\")));
        }
    }
    None
}
#[cfg(not(windows))]
fn steam_from_registry() -> Option<PathBuf> { None }

/// Likely places for the game's log: every Steam library (wherever Steam is
/// installed) and the standalone install, on any drive.
pub fn find_log(poe2: bool, extra_roots: &[PathBuf]) -> Option<PathBuf> {
    let game = if poe2 { "Path of Exile 2" } else { "Path of Exile" };
    let mut steam_roots: Vec<PathBuf> = steam_from_registry().into_iter().collect();
    let mut ggg_roots: Vec<PathBuf> = Vec::new();
    for d in b'C'..=b'Z' {
        let drive = PathBuf::from(format!("{}:\\", d as char));
        if !drive.exists() { continue; }
        for sub in ["Program Files (x86)\\Steam", "Program Files\\Steam", "Steam", "SteamLibrary", "Games\\Steam", "Games\\SteamLibrary"] {
            steam_roots.push(drive.join(sub));
        }
        for sub in ["Program Files (x86)\\Grinding Gear Games", "Program Files\\Grinding Gear Games", "Grinding Gear Games", "Games\\Grinding Gear Games"] {
            ggg_roots.push(drive.join(sub));
        }
    }
    // Every Steam install lists its other library folders (e.g. D:\\SteamLibrary).
    for s in steam_roots.clone() {
        if let Ok(vdf) = std::fs::read_to_string(s.join("steamapps").join("libraryfolders.vdf")) {
            for line in vdf.lines() {
                if let Some(rest) = line.trim().strip_prefix("\"path\"") {
                    steam_roots.push(PathBuf::from(rest.trim().trim_matches('"').replace("\\\\", "\\")));
                }
            }
        }
    }
    let mut dirs: Vec<PathBuf> = steam_roots.iter().map(|r| r.join("steamapps").join("common").join(game).join("logs")).collect();
    dirs.extend(ggg_roots.iter().map(|r| r.join(game).join("logs")));
    dirs.extend(extra_roots.iter().cloned());
    dirs.into_iter()
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

#[cfg(test)]
mod character_tests {
    #[test]
    fn reads_level_up_line() {
        let log = "2026/11/18 12:00:00 123 abc [INFO Client 1] : Grobbs_Witch (Witch) is now level 22\n\
                   2026/11/18 12:05:00 124 abc [INFO Client 1] : Grobbs_Witch (Witch) is now level 23\n\
                   2026/11/18 12:06:00 125 abc [DEBUG Client 1] Generating level 24 area \"1_2_3\"\n";
        assert_eq!(super::last_character(log), Some(("Grobbs_Witch".into(), "Witch".into(), 23)));
    }
}
