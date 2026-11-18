//! A per-filter history of changes: before each change the filter's whole
//! setup (edits, prices, theme, base) is saved, so you can go back to any
//! earlier point. Going back is itself recorded, so it can be undone too.

use crate::profiles::Profile;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, path::Path};

const KEEP: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Seconds since 1970 (shown as a local time in the app).
    pub at: u64,
    /// What changed, e.g. "Hide: Gold (stacks under 500)".
    pub what: String,
    /// The filter's setup just before that change.
    pub before: Profile,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    /// Profile id -> changes, oldest first.
    pub entries: HashMap<String, Vec<Entry>>,
}

impl History {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_string(self)?)?;
        fs::rename(tmp, path)
    }

    /// Remember a filter's setup before a change.
    pub fn record(&mut self, before: &Profile, what: &str) {
        let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let list = self.entries.entry(before.id.clone()).or_default();
        list.push(Entry { at, what: what.to_string(), before: before.clone() });
        if list.len() > KEEP {
            let extra = list.len() - KEEP;
            list.drain(..extra);
        }
    }

    pub fn list(&self, id: &str) -> &[Entry] {
        self.entries.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The setup from before change `index`; recording the current setup first
    /// so going back can be undone.
    pub fn go_back(&mut self, current: &Profile, index: usize) -> Option<Profile> {
        let entry = self.list(&current.id).get(index)?.clone();
        self.record(current, &format!("Went back to before: {}", entry.what));
        Some(Profile { id: current.id.clone(), ..entry.before })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{Game, Store};

    #[test]
    fn record_and_go_back() {
        let mut store = Store::default();
        let id = store.create("League Start", Game::Poe1, "base.filter");
        let mut h = History::default();

        h.record(&store.profiles[&id], "Theme: Ember");
        store.profiles.get_mut(&id).unwrap().theme = Some("Ember".into());
        h.record(&store.profiles[&id], "Theme: Frost");
        store.profiles.get_mut(&id).unwrap().theme = Some("Frost".into());

        // Go back to before "Theme: Ember": no theme.
        let back = h.go_back(&store.profiles[&id], 0).unwrap();
        assert_eq!(back.theme, None);
        store.profiles.insert(id.clone(), back);
        // ...and that's undoable: the latest entry holds the Frost setup.
        let last = h.list(&id).last().unwrap();
        assert!(last.what.starts_with("Went back to before: Theme: Ember"));
        assert_eq!(last.before.theme.as_deref(), Some("Frost"));

        for i in 0..80 { h.record(&store.profiles[&id], &format!("change {i}")); }
        assert_eq!(h.list(&id).len(), KEEP);
    }
}
