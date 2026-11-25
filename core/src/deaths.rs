//! Death notes: where you died, and what your maps with deaths had in common. The log doesn't say what killed you, so
//! this only ever says what was around you, with how many maps that's from.

use crate::journey::MapRun;
use crate::maps::shape;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One death in a map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Death {
    pub time: i64,
    pub map: String,
    pub area: String,
    pub level: u32,
    pub char_level: Option<u32>,
    /// Seconds into the map (pauses and hideout time not counted).
    pub secs: i64,
    /// Its mods, when you pressed Ctrl+D on it before running it.
    pub mods: Vec<String>,
}

impl Death {
    pub fn of(run: &MapRun, time: i64, char_level: Option<u32>) -> Death {
        Death { time, map: run.name.clone(), area: run.area.clone(), level: run.level, char_level, secs: run.time_inside(true, time).max(0), mods: run.mods.clone() }
    }
}

/// Deaths with something (a map, a mod) and without it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    pub name: String,
    pub runs: usize,
    /// Maps you died in at least once, and all deaths in them.
    pub bad_runs: usize,
    pub deaths: u32,
    pub other_runs: usize,
    pub other_bad: usize,
    /// You die in these maps at least twice as often as in the rest, over enough maps to say so.
    pub stands_out: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Patterns {
    pub runs: usize,
    pub deaths: u32,
    pub bad_runs: usize,
    /// Maps whose mods are known.
    pub with_mods: usize,
    pub maps: Vec<Row>,
    pub mods: Vec<Row>,
}

/// This many maps with it before it says anything.
pub const ENOUGH: usize = 4;

fn row(name: String, with: (usize, usize, u32), all: (usize, usize)) -> Row {
    let (runs, bad_runs, deaths) = with;
    let (other_runs, other_bad) = (all.0 - runs, all.1 - bad_runs);
    let rate = |b: usize, n: usize| if n == 0 { 0.0 } else { b as f64 / n as f64 };
    let stands_out = runs >= ENOUGH && bad_runs >= 2 && other_runs >= ENOUGH && rate(bad_runs, runs) >= 2.0 * rate(other_bad, other_runs);
    Row { name, runs, bad_runs, deaths, other_runs, other_bad, stands_out }
}

/// What your maps with deaths have in common. Mods are only compared among maps whose mods are known.
pub fn patterns(runs: &[MapRun]) -> Patterns {
    let mut maps: BTreeMap<&str, (usize, usize, u32)> = BTreeMap::new();
    let mut mods: BTreeMap<String, (String, (usize, usize, u32))> = BTreeMap::new();
    let (mut all, mut known) = ((0, 0), (0, 0));
    let mut deaths = 0;
    let count = |e: &mut (usize, usize, u32), d: u32| { e.0 += 1; if d > 0 { e.1 += 1; e.2 += d; } };
    for r in runs {
        all.0 += 1;
        if r.deaths > 0 { all.1 += 1; deaths += r.deaths; }
        count(maps.entry(&r.name).or_default(), r.deaths);
        if r.mods.is_empty() { continue; }
        known.0 += 1;
        if r.deaths > 0 { known.1 += 1; }
        let mut seen = std::collections::BTreeSet::new();
        for m in &r.mods {
            let k = shape(m);
            if !seen.insert(k.clone()) { continue; }
            let e = mods.entry(k).or_insert_with(|| (plain(m), (0, 0, 0)));
            count(&mut e.1, r.deaths);
        }
    }
    // The ones that stand out first, then by how often you died in them.
    let order = |v: &mut Vec<Row>| {
        v.retain(|r| r.bad_runs > 0);
        v.sort_by(|a, b| (b.stands_out, b.bad_runs * a.runs, b.deaths).cmp(&(a.stands_out, a.bad_runs * b.runs, a.deaths)));
        v.truncate(12);
    };
    let mut map_rows: Vec<Row> = maps.into_iter().map(|(n, w)| row(n.to_string(), w, all)).collect();
    let mut mod_rows: Vec<Row> = mods.into_values().map(|(n, w)| row(n, w, known)).collect();
    order(&mut map_rows);
    order(&mut mod_rows);
    Patterns { runs: all.0, deaths, bad_runs: all.1, with_mods: known.0, maps: map_rows, mods: mod_rows }
}

/// A mod line without the copy's extras.
fn plain(m: &str) -> String { m.replace(" — Unscalable Value", "").trim().to_string() }

/// For the map check: how it went in maps with each of these mods, (maps, maps you died in). None when you never died
/// with it, or only one map had it.
pub fn history_of(runs: &[MapRun], mods: &[String]) -> Vec<Option<(usize, usize)>> {
    mods.iter().map(|m| {
        let k = shape(m);
        let with: Vec<&MapRun> = runs.iter().filter(|r| r.mods.iter().any(|x| shape(x) == k)).collect();
        let bad = with.iter().filter(|r| r.deaths > 0).count();
        (with.len() >= 2 && bad > 0).then_some((with.len(), bad))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, deaths: u32, mods: &[&str]) -> MapRun {
        MapRun { name: name.into(), deaths, mods: mods.iter().map(|m| m.to_string()).collect(), ..Default::default() }
    }

    #[test]
    fn what_maps_with_deaths_share() {
        let mut runs = vec![];
        for i in 0..5 { runs.push(run("Strand", if i < 3 { 2 } else { 0 }, &["Players are Cursed with Enfeeble", "26% increased Monster Movement Speed"])); }
        for _ in 0..10 { runs.push(run("Dunes", 0, &["31% increased Monster Movement Speed"])); }
        runs.push(run("Pit", 1, &[]));
        let p = patterns(&runs);
        assert_eq!((p.runs, p.deaths, p.bad_runs, p.with_mods), (16, 7, 4, 15));
        let curse = &p.mods[0];
        assert_eq!((curse.name.as_str(), curse.runs, curse.bad_runs, curse.stands_out), ("Players are Cursed with Enfeeble", 5, 3, true));
        let speed = p.mods.iter().find(|m| m.name.contains("Movement")).unwrap();
        assert_eq!((speed.runs, speed.bad_runs, speed.stands_out), (15, 3, false), "different numbers are the same mod, and every map had it");
        assert!(p.maps.iter().any(|m| m.name == "Strand" && m.stands_out));
        assert!(!p.maps.iter().any(|m| m.name == "Pit" && m.stands_out), "one map says nothing");
        let h = history_of(&runs, &["40% increased Monster Movement Speed".into(), "Players cannot Regenerate Life, Mana or Energy Shield".into()]);
        assert_eq!(h, [Some((15, 3)), None]);
    }

    #[test]
    fn a_death_keeps_the_moment() {
        let mut r = run("Strand", 1, &["x"]);
        (r.start, r.last_enter, r.in_map, r.level) = (100, 160, 30, 83);
        let d = Death::of(&r, 200, Some(91));
        assert_eq!((d.secs, d.level, d.char_level), (70, 83, Some(91)));
    }
}
