//! Campaign split timer (27 Nov): how long each act took a character, read from the game's log (Client.txt), against
//! your best run, like LiveSplit.
//!
//! - A run starts when a character reaches level 2 and ends when it finishes act 10 (Karui Shores, or a map).
//! - Act N is finished the first time you enter a zone of act N + 1 (going back to an earlier act changes nothing;
//!   labyrinths don't count).
//! - The clock is time played: the gaps between log lines, except a silence over [`IDLE`] (a break: AFK, the game
//!   shut, the PC off) and the time you're on another character. So runs spread over days still compare fairly.
//! - Playing another character is noticed from its level-ups, or from a zone far above the run's level (your main in
//!   a map or its hideout); the run picks up again at its next level-up.
//!
//! Everything works off log lines, so the whole log gives your past campaigns straight away.

use crate::journey::{self, PlayEvent};
use serde::{Deserialize, Serialize};

/// A longer silence in the log than this is a break, not play (active play writes a line at least every few minutes).
pub const IDLE: i64 = 600;
/// Acts in the PoE 1 campaign.
pub const ACTS: usize = 10;
/// A zone this many levels above the character is another character's (your main in a map or hideout).
const OTHER_CHAR_GAP: u32 = 8;
/// An act done faster than this is a log mix-up, not a split to beat.
const MIN_SEGMENT: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Run {
    pub name: String,
    pub class: String,
    /// The log's clock (local time, as if UTC) when it reached level 2.
    pub started: i64,
    /// Seconds played so far.
    pub played: i64,
    /// The log's clock at the last line counted for it.
    pub last: i64,
    /// `splits[i]`: seconds played when act i + 1 was finished.
    pub splits: Vec<i64>,
    /// Character level at each split.
    pub levels: Vec<u32>,
    pub level: u32,
    /// Hidden from the history and the bests (a test character, a mix-up).
    pub hidden: bool,
}

impl Run {
    pub fn done(&self) -> bool { self.splits.len() >= ACTS }
    /// Seconds act `i` (0-based) took.
    pub fn segment(&self, i: usize) -> Option<i64> {
        let end = *self.splits.get(i)?;
        Some(end - if i == 0 { 0 } else { self.splits[i - 1] })
    }
}

/// Every run in the log, plus where the reading is.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Splits {
    pub runs: Vec<Run>,
    /// The character the log says you're on (None: someone without a run, or not sure).
    pub cur: Option<String>,
    pub cur_level: u32,
    /// The log's clock at the last line read (0 after the game starts up).
    pub last_line: i64,
}

impl Splits {
    /// The run being played by `name` (its latest one, unless it's finished).
    fn active_mut(&mut self, name: &str) -> Option<&mut Run> {
        self.runs.iter_mut().rev().find(|r| r.name == name).filter(|r| !r.done())
    }

    /// One line of Client.txt. Returns true when a run started or an act was finished.
    pub fn feed(&mut self, line: &str) -> bool {
        if line.contains("***** LOG FILE OPENING *****") { self.last_line = 0; return false; }
        let Some(t) = journey::log_time(line) else { return false };
        let gap = t - self.last_line;
        if self.last_line > 0 && gap > 0 && gap <= IDLE {
            if let Some(name) = self.cur.clone() {
                if let Some(r) = self.active_mut(&name) { r.played += gap; r.last = t; }
            }
        }
        self.last_line = t;
        match journey::parse_line(line) {
            Some(PlayEvent::Level { name, class, level, .. }) => {
                let mut changed = false;
                if level == 2 {
                    self.runs.push(Run { name: name.clone(), class, started: t, last: t, level: 2, ..Default::default() });
                    changed = true;
                }
                if let Some(r) = self.active_mut(&name) { r.level = level; }
                self.cur = Some(name);
                self.cur_level = level;
                changed
            }
            Some(PlayEvent::Area { id, level, .. }) => {
                let Some(name) = self.cur.clone() else { return false };
                let zone = journey::route().areas.get(&id);
                if zone.is_none() && journey::is_rest_area(&id) { return false; } // hideouts: no clue who you are
                if level > self.cur_level + OTHER_CHAR_GAP {
                    // Towns and hideouts are often above you (Lioneye's Watch is 13 on a level 2 twink, hideouts
                    // are 60) and you visit them mid-run; a map or campaign zone that far up is another character.
                    if !zone.is_some_and(|z| z.t) && (zone.is_some() || journey::is_map_area(&id)) { self.cur = None; }
                    return false;
                }
                let act = match zone {
                    Some(z) if !id.contains("Labyrinth") => z.a,
                    None if journey::is_map_area(&id) => ACTS as u32 + 1,
                    _ => return false,
                };
                let lvl = self.cur_level;
                let Some(r) = self.active_mut(&name) else { return false };
                if act as usize == r.splits.len() + 2 {
                    r.splits.push(r.played);
                    r.levels.push(lvl);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// Time played on a run up to `now` (the log's clock): the clock runs on between lines while it's yours.
    pub fn elapsed(&self, i: usize, now: i64) -> i64 {
        let r = &self.runs[i];
        let live = !r.done() && self.cur.as_deref() == Some(r.name.as_str()) && r.last == self.last_line;
        let gap = now - r.last;
        r.played + if live && gap > 0 && gap <= IDLE { gap } else { 0 }
    }

    /// The run to show: the one you're on, else the last one that got anywhere.
    pub fn current(&self) -> Option<usize> {
        if let Some(name) = &self.cur {
            if let Some(i) = self.runs.iter().rposition(|r| &r.name == name && !r.hidden) {
                if !self.runs[i].done() || self.runs[i].splits.len() == ACTS { return Some(i); }
            }
        }
        self.runs.iter().rposition(|r| !r.hidden && !r.splits.is_empty())
    }

    /// Your personal best: the fastest finished campaign (not counting `except`).
    pub fn best_run(&self, except: Option<usize>) -> Option<usize> {
        self.runs.iter().enumerate()
            .filter(|(i, r)| Some(*i) != except && !r.hidden && r.done() && valid(r))
            .min_by_key(|(_, r)| r.splits[ACTS - 1]).map(|(i, _)| i)
    }

    /// Your fastest time for each act on its own, over every run but `except`.
    pub fn best_segments(&self, except: Option<usize>) -> Vec<Option<i64>> {
        (0..ACTS).map(|a| {
            self.runs.iter().enumerate()
                .filter(|(i, r)| Some(*i) != except && !r.hidden)
                .filter_map(|(_, r)| r.segment(a)).filter(|s| *s >= MIN_SEGMENT).min()
        }).collect()
    }

    /// The times to beat at each act's end: your best run's, or (no finished run yet) your fastest time to each act.
    pub fn compare(&self, except: Option<usize>) -> Vec<Option<i64>> {
        if let Some(b) = self.best_run(except) { return self.runs[b].splits.iter().map(|s| Some(*s)).collect(); }
        (0..ACTS).map(|a| {
            self.runs.iter().enumerate().filter(|(i, r)| Some(*i) != except && !r.hidden && valid(r))
                .filter_map(|(_, r)| r.splits.get(a).copied()).min()
        }).collect()
    }
}

/// No act done impossibly fast (a log mix-up).
fn valid(r: &Run) -> bool { (0..r.splits.len()).all(|i| r.segment(i).is_some_and(|s| s >= MIN_SEGMENT)) }

/// What the act is called on the timer.
pub fn act_label(i: usize) -> String {
    match i { 4 => "Act 5 · Kitava".into(), 9 => "Act 10 · Kitava".into(), _ => format!("Act {}", i + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(mins: i64, secs: i64) -> String {
        let t = 1_800_000_000 + mins * 60 + secs;
        // Back into the log's "YYYY/MM/DD HH:MM:SS"; the date doesn't matter, only differences.
        let (d, s) = (t.div_euclid(86400), t.rem_euclid(86400));
        let (y, m, dd) = civil(d);
        format!("{y:04}/{m:02}/{dd:02} {:02}:{:02}:{:02} 1 abc", s / 3600, s / 60 % 60, s % 60)
    }
    fn civil(z: i64) -> (i64, i64, i64) {
        let z = z + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
    }
    fn level(s: &mut Splits, m: i64, name: &str, lvl: u32) {
        s.feed(&format!("{} [INFO Client 1] : {name} (Scion) is now level {lvl}", at(m, 0)));
    }
    fn zone(s: &mut Splits, m: i64, lvl: u32, id: &str) -> bool {
        s.feed(&format!("{} [DEBUG Client 1] Generating level {lvl} area \"{id}\" with seed 1", at(m, 30)))
    }
    fn tick(s: &mut Splits, m: i64) { s.feed(&format!("{} [INFO Client 1] [SCENE] tick", at(m, 0))); }

    #[test]
    fn acts_breaks_and_other_characters() {
        let mut s = Splits::default();
        level(&mut s, 0, "Racer", 2);
        for m in 1..=10 { tick(&mut s, m); }
        assert!(!zone(&mut s, 11, 3, "1_1_2"), "act 1 zones don't split");
        level(&mut s, 12, "Racer", 12);
        zone(&mut s, 12, 60, "HideoutSpace");
        assert_eq!(s.cur.as_deref(), Some("Racer"), "your hideout (level 60) mid-run is still you");
        assert!(zone(&mut s, 14, 13, "1_2_1"), "first act 2 zone ends act 1");
        assert_eq!(s.runs[0].splits, vec![14 * 60 + 30]);
        assert_eq!(s.runs[0].levels, vec![12]);
        assert!(!zone(&mut s, 15, 12, "1_1_town"), "going back doesn't split");
        // A 3-hour break: none of it counts.
        tick(&mut s, 200);
        tick(&mut s, 201);
        assert_eq!(s.runs[0].played, 15 * 60 + 30 + 60);
        // Over to the main character in a map: its time doesn't count, and its map doesn't finish the campaign.
        zone(&mut s, 202, 83, "MapWorldsStrand");
        assert_eq!(s.cur, None);
        for m in 203..=230 { tick(&mut s, m); }
        let before = s.runs[0].played;
        level(&mut s, 240, "Racer", 13);
        assert_eq!(s.runs[0].played, before, "the gap before coming back is a break");
        tick(&mut s, 241);
        assert_eq!(s.runs[0].played, before + 60);
        // Acts 2 to 10 in order, then Karui Shores ends the run.
        let ids = ["1_3_1", "1_4_1", "1_5_1", "2_6_1", "2_7_1", "2_8_1", "2_9_1", "2_10_1"];
        for (k, id) in ids.iter().enumerate() {
            let m = 250 + k as i64 * 5;
            tick(&mut s, m - 1);
            level(&mut s, m, "Racer", 20 + k as u32 * 6);
            assert!(zone(&mut s, m, 15 + k as u32 * 6, id), "{id}");
        }
        assert!(!s.runs[0].done());
        level(&mut s, 300, "Racer", 68);
        assert!(zone(&mut s, 300, 69, "2_11_endgame_town"));
        assert!(s.runs[0].done());
        let done_at = s.runs[0].played;
        tick(&mut s, 301);
        assert_eq!(s.runs[0].played, done_at, "a finished run's clock stops");
    }

    #[test]
    fn best_run_and_segments() {
        let run = |name: &str, splits: Vec<i64>| Run { name: name.into(), splits, ..Default::default() };
        let full = |k: i64| (1..=10).map(|a| a * k).collect::<Vec<_>>();
        let mut s = Splits::default();
        s.runs.push(run("Slow", full(900)));
        s.runs.push(run("Fast", full(800)));
        let mut odd = full(1000);
        odd[2] = odd[1] + 300; // one great act 3
        s.runs.push(run("Odd", odd));
        s.runs.push(run("Bugged", vec![30, 900])); // act 1 in 30 s: a log mix-up
        s.runs.push(run("Now", vec![700]));
        assert_eq!(s.best_run(Some(4)), Some(1));
        let segs = s.best_segments(Some(4));
        assert_eq!(segs[0], Some(800), "the 30 s act is ignored");
        assert_eq!(segs[2], Some(300));
        assert_eq!(s.compare(Some(4))[0], Some(800));
        s.runs[1].hidden = true;
        assert_eq!(s.best_run(Some(4)), Some(0));
        // No finished run: the fastest time to each act.
        let mut t = Splits::default();
        t.runs.push(run("A", vec![900, 2000]));
        t.runs.push(run("B", vec![1000, 1900, 2800]));
        assert_eq!(t.compare(None), [Some(900), Some(1900), Some(2800)].into_iter().chain(std::iter::repeat(None).take(7)).collect::<Vec<_>>());
    }

    #[test]
    fn live_clock_and_current_run() {
        let mut s = Splits::default();
        level(&mut s, 0, "Racer", 2);
        tick(&mut s, 5);
        let now = journey::log_time(&at(7, 0)).unwrap();
        assert_eq!(s.elapsed(0, now), 7 * 60);
        let later = journey::log_time(&at(30, 0)).unwrap();
        assert_eq!(s.elapsed(0, later), 5 * 60, "a long silence stops the clock");
        assert_eq!(s.current(), Some(0));
    }

    #[test]
    fn label() { assert_eq!(act_label(4), "Act 5 · Kitava"); assert_eq!(act_label(0), "Act 1"); }
}

/// `SPLITS_LOG=<Client.txt> cargo test -p filter-core --lib real_log -- --ignored --nocapture`: every run in a real log.
#[cfg(test)]
#[test]
#[ignore]
fn real_log() {
    let path = std::env::var("SPLITS_LOG").expect("SPLITS_LOG");
    let bytes = std::fs::read(path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let t0 = std::time::Instant::now();
    let mut s = Splits::default();
    for l in text.lines() { s.feed(l); }
    println!("read in {:?}", t0.elapsed());
    for r in &s.runs {
        println!("{} {} {:?} level {}", r.name, r.class, r.splits.iter().map(|x| x / 60).collect::<Vec<_>>(), r.level);
    }
    let cur = s.current();
    println!("current {:?} best {:?} segs {:?}", cur.map(|i| &s.runs[i].name), s.best_run(cur).map(|i| &s.runs[i].name),
        s.best_segments(cur).iter().map(|x| x.map(|v| v / 60)).collect::<Vec<_>>());
}
