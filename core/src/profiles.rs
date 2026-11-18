//! Profiles: one NeverSink base + your own edits = one output filter.
//! "League Start", "Strict 1", "Strict 2"... each can build on a parent.

use crate::filter::{parse_filter, render_filter};
use crate::rules::{apply, Rule};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Game { Poe1, Poe2 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game: Game,
    pub base_file: String,      // NeverSink/FilterBlade file it builds on
    pub output_file: String,    // what the game loads, e.g. "League Start.filter"
    pub parent: Option<String>, // inherits this profile's edits first
    pub rules: Vec<Rule>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Store {
    pub profiles: BTreeMap<String, Profile>,
    pub active: Option<String>,
}

pub fn slug(name: &str) -> String {
    name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

impl Store {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    /// Writes to a temp file then swaps it in, so a crash never leaves a half file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(tmp, path)
    }

    pub fn create(&mut self, name: &str, game: Game, base_file: &str) -> String {
        let id = slug(name);
        self.profiles.insert(id.clone(), Profile {
            id: id.clone(), name: name.into(), game, base_file: base_file.into(),
            output_file: format!("{name}.filter"), parent: None, rules: vec![],
        });
        self.active.get_or_insert(id.clone());
        id
    }

    /// "Save as new". `inherit` = the copy keeps following the original's edits.
    pub fn fork(&mut self, from: &str, new_name: &str, inherit: bool) -> Option<String> {
        let src = self.profiles.get(from)?.clone();
        let id = slug(new_name);
        let p = Profile {
            id: id.clone(),
            name: new_name.into(),
            output_file: format!("{new_name}.filter"),
            parent: inherit.then(|| from.to_string()),
            rules: if inherit { vec![] } else { src.rules.clone() },
            ..src
        };
        self.profiles.insert(id.clone(), p);
        Some(id)
    }

    /// Parent edits first, then this profile's own, so the child wins.
    pub fn resolved_rules(&self, id: &str) -> Vec<Rule> {
        let mut chain = Vec::new();
        let mut cur = self.profiles.get(id);
        while let Some(p) = cur {
            if chain.len() > 20 { break; }
            chain.push(p);
            cur = p.parent.as_deref().and_then(|pid| self.profiles.get(pid));
        }
        chain.iter().rev().flat_map(|p| p.rules.clone()).collect()
    }
}

/// Rebuilds a profile's filter from its base file plus all edits, backing up
/// the previous output first. Returns warnings to show the user.
pub fn build(store: &Store, id: &str, filter_dir: &Path) -> io::Result<Vec<String>> {
    let p = store.profiles.get(id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "profile not found"))?;
    let base = fs::read_to_string(filter_dir.join(&p.base_file))?;
    let nl = if base.contains("\r\n") { "\r\n" } else { "\n" };
    let mut segs = parse_filter(&base);
    let warnings = apply(&mut segs, &store.resolved_rules(id), nl);

    let out = filter_dir.join(&p.output_file);
    if out.exists() {
        let backups = filter_dir.join("LiveEditorBackups");
        fs::create_dir_all(&backups)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        fs::copy(&out, backups.join(format!("{}.{stamp}.filter", p.id)))?;
        prune_backups(&backups, &p.id, 30)?;
    }
    let tmp = out.with_extension("tmp");
    fs::write(&tmp, render_filter(&segs))?;
    fs::rename(tmp, out)?;
    Ok(warnings)
}

fn prune_backups(dir: &Path, id: &str, keep: usize) -> io::Result<()> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{id}.")))
        .collect();
    files.sort_by_key(|e| e.file_name());
    while files.len() > keep {
        fs::remove_file(files.remove(0).path())?;
    }
    Ok(())
}
