//! Saved price searches (from the Ctrl+D window), and which listings a live
//! search has already seen so only new ones pop up.

use crate::trade::TradeSetup;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Live searches running at once (each one re-checks the trade site every ~90s).
pub const MAX_LIVE: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSearch {
    pub id: u64,
    pub name: String,
    pub setup: TradeSetup,
    #[serde(default)]
    pub live: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Saved {
    pub list: Vec<SavedSearch>,
    /// Listing ids each live search has seen (not kept on disk).
    #[serde(skip)]
    seen: HashMap<u64, HashSet<String>>,
}

impl Saved {
    pub fn load(path: &Path) -> Saved {
        crate::keep::load(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        crate::keep::save(path, self, true).map_err(|e| e.to_string())
    }

    pub fn add(&mut self, name: &str, setup: TradeSetup) -> u64 {
        let id = self.list.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        let name = if name.trim().is_empty() { format!("Search {id}") } else { name.trim().to_string() };
        self.list.push(SavedSearch { id, name, setup, live: false });
        id
    }

    pub fn remove(&mut self, id: u64) {
        self.list.retain(|s| s.id != id);
        self.seen.remove(&id);
    }

    /// Turns live on or off. Errors when too many are already live.
    pub fn set_live(&mut self, id: u64, on: bool) -> Result<(), String> {
        if on && !self.list.iter().any(|s| s.id == id && s.live) && self.list.iter().filter(|s| s.live).count() >= MAX_LIVE {
            return Err(format!("Up to {MAX_LIVE} live searches at once (so price checks stay quick): turn one off first"));
        }
        if let Some(s) = self.list.iter_mut().find(|s| s.id == id) { s.live = on; }
        if !on { self.seen.remove(&id); }
        Ok(())
    }

    /// The listings not seen before. The first check only remembers what's already
    /// listed (returns nothing), so you're told about new ones only.
    pub fn fresh(&mut self, id: u64, ids: &[String]) -> Vec<String> {
        let first = !self.seen.contains_key(&id);
        let seen = self.seen.entry(id).or_default();
        let new: Vec<String> = ids.iter().filter(|x| !seen.contains(*x)).cloned().collect();
        seen.extend(new.iter().cloned());
        if first { vec![] } else { new }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::parse_item;

    fn setup() -> TradeSetup {
        let item = parse_item("Item Class: Rings\nRarity: Rare\nDoom Loop\nRuby Ring\n--------\nItem Level: 84\n--------\n+40 to maximum Life\n").unwrap();
        TradeSetup { item, filters: vec![], base_only: false, corrupted: None, ilvl_min: None, ilvl_max: None, links_min: None, gem_level: None, unknown: vec![], unknown_kinds: vec![], category: None, raw: String::new(), misc: vec![], base: String::new(), base_item: None, affix_max: None, buyout: None }
    }

    #[test]
    fn saves_and_tracks_new_listings() {
        let mut s = Saved::default();
        let a = s.add("Life ring", setup());
        let b = s.add("", setup());
        assert_eq!(s.list[1].name, "Search 2");
        s.set_live(a, true).unwrap();
        assert!(s.fresh(a, &["x".into(), "y".into()]).is_empty(), "first look only remembers");
        assert_eq!(s.fresh(a, &["x".into(), "z".into()]), ["z"]);
        for i in 0..MAX_LIVE { let id = s.add("", setup()); if i < MAX_LIVE - 1 { s.set_live(id, true).unwrap(); } }
        assert!(s.set_live(b, true).is_err());
        s.remove(a);
        assert!(s.set_live(b, true).is_ok());

        let dir = std::env::temp_dir().join("pfe_saved_test.json");
        s.save(&dir).unwrap();
        let back = Saved::load(&dir);
        assert_eq!(back.list.len(), s.list.len());
        assert_eq!(back.list[0].setup.item.base_type, "Ruby Ring");
        let _ = std::fs::remove_file(dir);
    }
}
