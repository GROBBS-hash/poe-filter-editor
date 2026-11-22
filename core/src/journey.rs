//! Play helpers that follow the game's log (Client.txt): which zone you're in, the levelling route step you're on,
//! your map runs (time per map, maps per hour, deaths), XP per hour, and short shop/stash search strings.
//!
//! The levelling route, zones and quest rewards come from exile-leveling by HeartofPhos (MIT), rebuilt with
//! `node tools/build_levelling.mjs`. The XP table is the game's own (level 1..100 totals).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

// ---------- the game's log ----------

/// One useful line from Client.txt.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PlayEvent {
    /// `Generating level 83 area "MapWorldsStrand" with seed 3141592`: logged every time a zone loads.
    Area { time: i64, level: u32, id: String, seed: u64 },
    /// `: You have entered Strand.`
    Entered { time: i64, name: String },
    /// `: Bob (Witch) is now level 23`
    Level { time: i64, name: String, class: String, level: u32 },
    /// `: Bob has been slain.`
    Slain { time: i64, name: String },
}

/// "2026/11/18 19:48:01" -> seconds (as if UTC; only differences between lines are used).
pub fn log_time(line: &str) -> Option<i64> {
    let t = line.get(..19)?;
    let b = t.as_bytes();
    if b.get(4) != Some(&b'/') || b.get(10) != Some(&b' ') { return None; }
    let n = |r: std::ops::Range<usize>| t.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    // Days from 1970-01-01 (Howard Hinnant's civil-from-days, inverted).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hh * 3600 + mm * 60 + ss)
}

/// Reads one Client.txt line.
pub fn parse_line(line: &str) -> Option<PlayEvent> {
    let line = line.trim_end();
    let time = log_time(line).unwrap_or(0);
    if let Some(i) = line.find("Generating level ") {
        let rest = &line[i + "Generating level ".len()..];
        let (lvl, rest) = rest.split_once(" area \"")?;
        let (id, rest) = rest.split_once('"')?;
        let seed = rest.split("with seed ").nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        return Some(PlayEvent::Area { time, level: lvl.trim().parse().ok()?, id: id.to_string(), seed });
    }
    let body = &line[line.find("] ")? + 2..];
    let msg = body.strip_prefix(": ")?;
    if let Some(name) = msg.strip_prefix("You have entered ") {
        return Some(PlayEvent::Entered { time, name: name.trim_end_matches('.').trim().to_string() });
    }
    if let Some(i) = msg.find(" is now level ") {
        let level = msg[i + " is now level ".len()..].trim().parse().ok()?;
        let who = &msg[..i];
        let open = who.rfind(" (")?;
        return Some(PlayEvent::Level { time, name: who[..open].to_string(), class: who[open + 2..].trim_end_matches(')').to_string(), level });
    }
    if let Some(name) = msg.strip_suffix(" has been slain.") {
        if !name.trim().is_empty() { return Some(PlayEvent::Slain { time, name: name.trim().to_string() }); }
    }
    None
}

/// Map areas (PoE 1 "MapWorlds…", PoE 2 "Map…"); not hideouts, towns or the campaign.
pub fn is_map_area(id: &str) -> bool {
    id.starts_with("Map") && !id.contains("Hideout") && !id.starts_with("MapDevice")
}

/// Hideouts and towns (the places you go between maps).
pub fn is_rest_area(id: &str) -> bool {
    id.contains("Hideout") || id.ends_with("_town") || id.contains("Town") || id == "HeistHub"
}

// ---------- map runs ----------

/// One map, from first entering it until you start the next one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MapRun {
    /// "Strand" (as the game says on entering).
    pub name: String,
    pub area: String,
    pub level: u32,
    pub seed: u64,
    pub start: i64,
    /// When you last came into it (while inside).
    #[serde(default)]
    pub last_enter: i64,
    /// Time spent inside the map (hideout time between portals not counted).
    pub in_map: i64,
    /// When you last left it.
    pub left: i64,
    pub deaths: u32,
    /// Times you came back in through a portal.
    pub portals: u32,
    /// From a Ctrl+D map check just before you ran it: its mods.
    #[serde(default)]
    pub mods: Vec<String>,
    /// About how much your earnings session went up during the run (chaos), when one was running.
    #[serde(default)]
    pub made: Option<f64>,
    /// Earnings session value when the run started (to work out `made`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_at_start: Option<f64>,
}

impl MapRun {
    /// PoE 1 map tier from the area level (68 = tier 1).
    pub fn tier(&self) -> Option<u32> { (self.level >= 68).then(|| self.level - 67) }
    /// Time inside so far (counting the current visit if `inside` and `now` given).
    pub fn time_inside(&self, inside: bool, now: i64) -> i64 {
        self.in_map + if inside && now > self.last_enter { now - self.last_enter } else { 0 }
    }
}

/// Follows the log and keeps your map runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapTracker {
    pub current: Option<MapRun>,
    /// Inside the current map right now.
    pub inside: bool,
    /// Finished runs, newest last (kept to 500).
    pub history: Vec<MapRun>,
    /// Your character's name (from level-up lines), so a party member's death isn't counted as yours.
    pub character: Option<String>,
    #[serde(skip)]
    pending: Option<(String, u32, u64, i64)>,
}

/// What the tracker did with a line (the app uses it to attach map mods and earnings).
#[derive(Debug, Clone, PartialEq)]
pub enum MapChange { None, Started, Resumed, Left, Finished(MapRun), Death }

impl MapTracker {
    pub fn on_event(&mut self, ev: &PlayEvent) -> Vec<MapChange> {
        let mut out = vec![];
        match ev {
            PlayEvent::Area { time, level, id, seed } => {
                self.pending = Some((id.clone(), *level, *seed, *time));
                // Leaving a map shows up as the next zone loading.
                if self.inside && !is_map_area(id) {
                    if let Some(c) = self.current.as_mut() { c.in_map += (time - c.last_enter).max(0); c.left = *time; }
                    self.inside = false;
                    out.push(MapChange::Left);
                }
                if is_map_area(id) {
                    let same = self.current.as_ref().is_some_and(|c| c.area == *id && (c.seed == *seed || *seed == 0 || c.seed == 0));
                    if same {
                        if !self.inside {
                            let c = self.current.as_mut().unwrap();
                            c.last_enter = *time;
                            c.portals += 1;
                            self.inside = true;
                            out.push(MapChange::Resumed);
                        }
                    } else {
                        if let Some(done) = self.finish(*time) { out.push(MapChange::Finished(done)); }
                        self.current = Some(MapRun { area: id.clone(), level: *level, seed: *seed, start: *time, last_enter: *time, left: *time, name: map_name(id), ..Default::default() });
                        self.inside = true;
                        out.push(MapChange::Started);
                    }
                }
            }
            PlayEvent::Entered { name, .. } => {
                // The game's own name for the map ("Strand"), better than the area id.
                if let (Some((id, ..)), Some(c)) = (&self.pending, self.current.as_mut()) {
                    if self.inside && c.area == *id && !name.is_empty() { c.name = name.clone(); }
                }
            }
            PlayEvent::Level { name, .. } => { self.character = Some(name.clone()); }
            PlayEvent::Slain { name, .. } => {
                let mine = self.character.as_ref().map_or(true, |c| c == name);
                if self.inside && mine {
                    if let Some(c) = self.current.as_mut() { c.deaths += 1; out.push(MapChange::Death); }
                }
            }
        }
        if out.is_empty() { out.push(MapChange::None); }
        out
    }

    /// Ends the current run (when the next map starts, or you press Done).
    pub fn finish(&mut self, now: i64) -> Option<MapRun> {
        let mut c = self.current.take()?;
        if self.inside { c.in_map += (now - c.last_enter).max(0); c.left = now; }
        self.inside = false;
        self.history.push(c.clone());
        if self.history.len() > 500 { let n = self.history.len() - 500; self.history.drain(..n); }
        Some(c)
    }
}

/// "MapWorldsStrand" -> "Strand", "MapWorldsArachnidTomb" -> "Arachnid Tomb" (until the game names it).
pub fn map_name(id: &str) -> String {
    let core = id.trim_start_matches("MapWorlds").trim_start_matches("Map");
    let mut s = String::new();
    for (i, c) in core.chars().enumerate() {
        if i > 0 && c.is_uppercase() { s.push(' '); }
        if c != '_' { s.push(c); } else { s.push(' '); }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Summary of recent runs.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct MapStats {
    pub runs: usize,
    /// Average seconds inside a map.
    pub avg_inside: i64,
    /// Average seconds from one map's start to the next (includes hideout time).
    pub avg_cycle: i64,
    pub per_hour: f64,
    pub deaths: u32,
    /// Average chaos per map, from runs that have it.
    pub avg_made: Option<f64>,
}

/// Stats over the runs that started since `since` (0 = all).
pub fn map_stats(runs: &[MapRun], since: i64) -> MapStats {
    let rs: Vec<&MapRun> = runs.iter().filter(|r| r.start >= since).collect();
    if rs.is_empty() { return MapStats::default(); }
    let n = rs.len() as i64;
    let avg_inside = rs.iter().map(|r| r.in_map).sum::<i64>() / n;
    // Cycle: start-to-start where the next run follows within an hour; else the run's own span.
    let mut cycles = vec![];
    for w in rs.windows(2) {
        let gap = w[1].start - w[0].start;
        if gap > 0 && gap < 3600 { cycles.push(gap); }
    }
    let avg_cycle = if cycles.is_empty() { rs.iter().map(|r| (r.left - r.start).max(r.in_map)).sum::<i64>() / n } else { cycles.iter().sum::<i64>() / cycles.len() as i64 };
    let made: Vec<f64> = rs.iter().filter_map(|r| r.made).collect();
    MapStats {
        runs: rs.len(), avg_inside, avg_cycle,
        per_hour: if avg_cycle > 0 { 3600.0 / avg_cycle as f64 } else { 0.0 },
        deaths: rs.iter().map(|r| r.deaths).sum(),
        avg_made: (!made.is_empty()).then(|| made.iter().sum::<f64>() / made.len() as f64),
    }
}

// ---------- experience ----------

/// Total experience needed for each level (index 0 = level 1).
pub const XP_TABLE: [u64; 100] = [
    0, 525, 1760, 3781, 7184, 12186, 19324, 29377, 43181, 61693,
    85990, 117506, 157384, 207736, 269997, 346462, 439268, 551295, 685171, 843709,
    1030734, 1249629, 1504995, 1800847, 2142652, 2535122, 2984677, 3496798, 4080655, 4742836,
    5490247, 6334393, 7283446, 8384398, 9541110, 10874351, 12361842, 14018289, 15859432, 17905634,
    20171471, 22679999, 25456123, 28517857, 31897771, 35621447, 39721017, 44225461, 49176560, 54607467,
    60565335, 67094245, 74247659, 82075627, 90631041, 99984974, 110197515, 121340161, 133497202, 146749362,
    161191120, 176922628, 194049893, 212684946, 232956711, 255001620, 278952403, 304972236, 333233648, 363906163,
    397194041, 433312945, 472476370, 514937180, 560961898, 610815862, 664824416, 723298169, 786612664, 855129128,
    929261318, 1009443795, 1096169525, 1189918242, 1291270350, 1400795257, 1519130326, 1646943474, 1784977296, 1934009687,
    2094900291, 2268549086, 2455921256, 2658074992, 2876116901, 3111280300, 3364828162, 3638186694, 3932818530, 4250334444,
];

/// Level for a total experience.
pub fn level_of(xp: u64) -> u32 { XP_TABLE.iter().rposition(|&t| xp >= t).map(|i| i as u32 + 1).unwrap_or(1) }

/// (experience still needed for the next level, how far through this level 0..1). None at 100.
pub fn to_next(xp: u64) -> Option<(u64, f64)> {
    let l = level_of(xp) as usize;
    if l >= 100 { return None; }
    let (lo, hi) = (XP_TABLE[l - 1], XP_TABLE[l]);
    Some((hi.saturating_sub(xp), (xp - lo) as f64 / (hi - lo) as f64))
}

/// Experience per hour from (time, total xp) samples over the last `window` seconds.
pub fn xp_per_hour(samples: &[(i64, u64)], window: i64) -> Option<f64> {
    let last = samples.last()?;
    let first = samples.iter().find(|s| s.0 >= last.0 - window)?;
    let secs = last.0 - first.0;
    (secs >= 60 && last.1 >= first.1).then(|| (last.1 - first.1) as f64 * 3600.0 / secs as f64)
}

/// PoE 1's experience penalty for being too far above or below the zone: the share of XP you keep (0..1).
pub fn xp_multiplier(player: u32, area: u32) -> f64 {
    let safe = 3 + player as i64 / 16;
    let diff = ((player as i64 - area as i64).abs() - safe).max(0) as f64;
    if diff == 0.0 { return 1.0; }
    let p = player as f64 + 5.0;
    (p / (p + diff.powf(2.5))).powf(1.5).max(0.01)
}

// ---------- levelling route ----------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Zone {
    /// Name, act, area level, town.
    pub n: String,
    pub a: u32,
    pub l: u32,
    pub t: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Quest {
    pub name: String,
    pub act: u32,
    pub npc: String,
    /// (gem, classes that can take it; empty = everyone).
    pub rewards: Vec<(String, Vec<String>)>,
    /// (gem, classes, npc) you can buy after the quest.
    pub vendor: Vec<(String, Vec<String>, String)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Step {
    pub act: u32,
    /// (kind, value): "text", "enter" (area id), "waypoint" (area id, or "" = any), "kill", "quest" (quest id),
    /// "quest_text", "generic", "dir" (degrees), "arena", "logout", "trial", "crafting", "ascend", "portal", "waypoint_get".
    pub parts: Vec<(String, String)>,
    pub subs: Vec<String>,
    /// "LEAGUE_START", "!LEAGUE_START", "BANDIT_KILL"… all must hold.
    #[serde(default)]
    pub cond: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Route {
    pub credit: String,
    pub areas: HashMap<String, Zone>,
    pub quests: HashMap<String, Quest>,
    pub gems: Vec<String>,
    pub steps: Vec<Step>,
}

pub fn route() -> &'static Route {
    static R: OnceLock<Route> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("../data/levelling.json")).expect("levelling.json"))
}

/// Your levelling choices that change the route.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RouteOptions {
    /// First character of the league: the route grabs extra waypoints and does side areas.
    pub league_start: bool,
    /// "kill" (kill all three, the usual), "alira", "oak", "kraityn".
    pub bandit: String,
    /// Do the Library quest in act 3.
    pub library: bool,
}

impl Default for RouteOptions {
    fn default() -> Self { Self { league_start: false, bandit: "kill".into(), library: false } }
}

impl RouteOptions {
    fn flag(&self, f: &str) -> bool {
        match f {
            "LEAGUE_START" => self.league_start,
            "LIBRARY" => self.library,
            "BANDIT_KILL" => self.bandit == "kill",
            "BANDIT_ALIRA" => self.bandit == "alira",
            "BANDIT_OAK" => self.bandit == "oak",
            "BANDIT_KRAITYN" => self.bandit == "kraityn",
            _ => false,
        }
    }
    pub fn keeps(&self, s: &Step) -> bool {
        s.cond.iter().all(|c| match c.strip_prefix('!') { Some(f) => !self.flag(f), None => self.flag(c) })
    }
}

/// The steps for these options (indexes into `route().steps`).
pub fn steps_for(r: &Route, o: &RouteOptions) -> Vec<usize> {
    (0..r.steps.len()).filter(|&i| o.keeps(&r.steps[i])).collect()
}

/// Does this step take you into `area` (walking in, a waypoint, or logging out to that act's town)?
pub fn step_enters(r: &Route, s: &Step, area: &str) -> bool {
    s.parts.iter().any(|(k, v)| match k.as_str() {
        "enter" | "waypoint" => v == area,
        "logout" => r.areas.get(area).is_some_and(|z| z.t && z.a == s.act),
        _ => false,
    })
}

/// Where you are on the route after entering `area`, given you were at `pos` (an index into `visible`).
/// Looks ahead first (up to 30 steps), then a little behind (you went back); None if the zone isn't nearby.
/// Returns the step after the one that entered the zone.
pub fn advance(r: &Route, visible: &[usize], pos: usize, area: &str) -> Option<usize> {
    let ahead = (pos..visible.len().min(pos + 30)).find(|&p| step_enters(r, &r.steps[visible[p]], area));
    let found = ahead.or_else(|| (pos.saturating_sub(12)..pos).rev().find(|&p| step_enters(r, &r.steps[visible[p]], area)))?;
    Some((found + 1).min(visible.len().saturating_sub(1)))
}

/// A step written out in plain words (zone ids replaced by their names).
pub fn step_text(r: &Route, s: &Step) -> String {
    let mut out = String::new();
    for (k, v) in &s.parts {
        let zone = |id: &str| r.areas.get(id).map(|z| z.n.clone()).unwrap_or_else(|| id.to_string());
        match k.as_str() {
            "text" => out.push_str(v),
            "enter" => out.push_str(&zone(v)),
            "waypoint" => out.push_str(&if v.is_empty() { "Waypoint".into() } else { format!("Waypoint to {}", zone(v)) }),
            "waypoint_get" => out.push_str("the waypoint"),
            "quest" => out.push_str(&r.quests.get(v).map(|q| q.name.clone()).unwrap_or_else(|| v.clone())),
            "logout" => out.push_str("Log out (back to town)"),
            "trial" => out.push_str("the Trial of Ascendancy"),
            "crafting" => out.push_str("the crafting recipe"),
            "ascend" => out.push_str(&format!("Do the {} Labyrinth", if v.is_empty() { "normal" } else { v })),
            "portal" => out.push_str(if v == "set" { "a portal" } else { "your portal" }),
            "dir" => {
                let d: i32 = v.parse().unwrap_or(0);
                const N: [&str; 8] = ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"];
                out.push_str(N[(((d % 360 + 360) % 360 + 22) / 45 % 8) as usize]);
            }
            _ => out.push_str(v),
        }
    }
    out.trim().to_string()
}

/// Quest gems from your build that this quest offers your class: (gem, true = a reward you pick, false = buy after).
pub fn build_rewards(q: &Quest, class: &str, build_gems: &[String]) -> Vec<(String, bool, String)> {
    let wants = |g: &str| build_gems.iter().any(|b| b.eq_ignore_ascii_case(g));
    let ok = |cs: &[String]| cs.is_empty() || cs.iter().any(|c| c.eq_ignore_ascii_case(class));
    let mut out: Vec<(String, bool, String)> = q.rewards.iter().filter(|(g, cs)| wants(g) && ok(cs)).map(|(g, _)| (g.clone(), true, q.npc.clone())).collect();
    for (g, cs, npc) in &q.vendor {
        if wants(g) && ok(cs) && !out.iter().any(|(o, ..)| o == g) { out.push((g.clone(), false, npc.clone())); }
    }
    out
}

// ---------- search strings ----------

/// The game's search boxes take up to 250 characters.
pub const SEARCH_MAX: usize = 250;

fn regex_safe(s: &str) -> String {
    s.chars().filter(|c| !"\"\\".contains(*c)).map(|c| if "^$.|?*+()[]{}".contains(c) { format!("\\{c}") } else { c.to_string() }).collect()
}

/// A short piece of `name` found in no other name of `all` (lowercase), or the whole name.
pub fn short_token(name: &str, all: &[String]) -> String {
    let n = name.to_lowercase();
    let others: Vec<String> = all.iter().map(|a| a.to_lowercase()).filter(|a| *a != n).collect();
    let chars: Vec<char> = n.chars().collect();
    for len in 3..=chars.len() {
        for start in 0..=chars.len() - len {
            let t: String = chars[start..start + len].iter().collect();
            if t.starts_with(' ') || t.ends_with(' ') || t.contains('\'') { continue; }
            if !others.iter().any(|o| o.contains(&t)) { return t; }
        }
    }
    n
}

/// A search for these gems at a vendor: `"arc|flame da|frostbl"`. Returns (string, gems that didn't fit).
pub fn gem_search(want: &[String], all: &[String]) -> (String, Vec<String>) {
    let mut toks: Vec<String> = vec![];
    let mut left = vec![];
    for g in want {
        let t = regex_safe(&short_token(g, all));
        let len = toks.iter().map(|x| x.len() + 1).sum::<usize>() + t.len() + 2;
        if toks.contains(&t) { continue; }
        if len > SEARCH_MAX { left.push(g.clone()); continue; }
        toks.push(t);
    }
    (if toks.is_empty() { String::new() } else { format!("\"{}\"", toks.join("|")) }, left)
}

/// A stash search lighting up these items by name: `"divine orb|tabula rasa"`. Returns (string, how many fit).
pub fn names_search(names: &[String]) -> (String, usize) {
    let mut toks: Vec<String> = vec![];
    for n in names {
        let t = regex_safe(&n.to_lowercase());
        if toks.contains(&t) { continue; }
        if toks.iter().map(|x| x.len() + 1).sum::<usize>() + t.len() + 2 > SEARCH_MAX { break; }
        toks.push(t);
    }
    let fit = toks.len();
    (if toks.is_empty() { String::new() } else { format!("\"{}\"", toks.join("|")) }, fit)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "2026/11/18 19:48:01 1234567 cff94598 [INFO Client 9876]";

    #[test]
    fn reads_log_lines() {
        assert_eq!(parse_line(&format!("2026/11/18 19:48:01 1 a [DEBUG Client 9] Generating level 83 area \"MapWorldsStrand\" with seed 31415")),
            Some(PlayEvent::Area { time: log_time("2026/11/18 19:48:01").unwrap(), level: 83, id: "MapWorldsStrand".into(), seed: 31415 }));
        assert!(matches!(parse_line(&format!("{T} : You have entered Strand.")), Some(PlayEvent::Entered { name, .. }) if name == "Strand"));
        assert!(matches!(parse_line(&format!("{T} : Bob_Witch (Necromancer) is now level 71")), Some(PlayEvent::Level { name, class, level: 71, .. }) if name == "Bob_Witch" && class == "Necromancer"));
        assert!(matches!(parse_line(&format!("{T} : Bob_Witch has been slain.")), Some(PlayEvent::Slain { name, .. }) if name == "Bob_Witch"));
        assert_eq!(parse_line(&format!("{T} @From Bob: hi")), None);
        assert_eq!(log_time("2026/11/18 19:48:01").unwrap() - log_time("2026/11/18 19:47:00").unwrap(), 61);
        assert_eq!(log_time("2026/03/01 00:00:00").unwrap() - log_time("2026/02/28 00:00:00").unwrap(), 86400);
        assert_eq!(log_time("1970/01/01 00:00:10"), Some(10));
    }

    fn area(t: i64, id: &str, seed: u64) -> PlayEvent { PlayEvent::Area { time: t, level: 80, id: id.into(), seed } }

    #[test]
    fn counts_map_runs() {
        let mut m = MapTracker::default();
        m.on_event(&PlayEvent::Level { time: 0, name: "Me".into(), class: "Witch".into(), level: 90 });
        m.on_event(&area(0, "HideoutFelled", 1));
        m.on_event(&area(10, "MapWorldsStrand", 5));
        m.on_event(&PlayEvent::Entered { time: 10, name: "Strand".into() });
        assert!(m.inside);
        m.on_event(&area(130, "HideoutFelled", 1)); // portal out after 2 min
        assert!(!m.inside);
        m.on_event(&PlayEvent::Slain { time: 140, name: "Friend".into() }); // not in the map, not me
        m.on_event(&area(160, "MapWorldsStrand", 5)); // back in
        m.on_event(&PlayEvent::Slain { time: 170, name: "Me".into() });
        m.on_event(&area(200, "HideoutFelled", 1));
        let ch = m.on_event(&area(260, "MapWorldsBurial_Chambers", 9)); // next map
        assert!(matches!(&ch[0], MapChange::Finished(r) if r.name == "Strand" && r.in_map == 160 && r.portals == 1 && r.deaths == 1));
        assert_eq!(m.current.as_ref().unwrap().name, "Burial Chambers");
        assert_eq!(m.history[0].tier(), Some(13));
        let s = map_stats(&m.history, 0);
        assert_eq!((s.runs, s.avg_inside, s.deaths), (1, 160, 1));
        m.finish(400);
        let s = map_stats(&m.history, 0);
        assert_eq!(s.avg_cycle, 250);
        assert!((s.per_hour - 14.4).abs() < 0.01);
    }

    #[test]
    fn experience() {
        assert_eq!(level_of(0), 1);
        assert_eq!(level_of(525), 2);
        assert_eq!(level_of(4_250_334_444), 100);
        assert_eq!(to_next(4_250_334_444), None);
        let (left, frac) = to_next(1000).unwrap();
        assert_eq!(left, 760);
        assert!(frac > 0.38 && frac < 0.39);
        assert_eq!(xp_per_hour(&[(0, 0), (1800, 1_000_000)], 3600), Some(2_000_000.0));
        assert_eq!(xp_per_hour(&[(0, 0), (30, 10)], 3600), None);
        assert_eq!(xp_multiplier(20, 20), 1.0);
        assert_eq!(xp_multiplier(20, 24), 1.0);
        assert!(xp_multiplier(20, 30) < 0.5);
    }

    #[test]
    fn route_follows_zones() {
        let r = route();
        assert!(r.steps.len() > 300);
        let o = RouteOptions::default();
        let vis = steps_for(r, &o);
        assert!(vis.len() < r.steps.len()); // league-start-only steps are left out
        // Entering The Coast moves you past "➞ The Coast".
        let p = advance(r, &vis, 0, "1_1_2").unwrap();
        assert!(step_text(r, &r.steps[vis[p - 1]]).contains("The Coast"));
        // Mud Flats next.
        let p2 = advance(r, &vis, p, "1_1_3").unwrap();
        assert!(p2 > p);
        // A zone nowhere near: stay put.
        assert_eq!(advance(r, &vis, p2, "1_10_1"), None);
        assert!(step_text(r, &r.steps[0]).contains("Hillock"));
    }

    #[test]
    fn quest_rewards_for_your_build() {
        let q = &route().quests["a1q1"];
        let got = build_rewards(q, "Witch", &["Freezing Pulse".into(), "Cleave".into()]);
        assert_eq!(got.first().map(|g| (g.0.as_str(), g.1)), Some(("Freezing Pulse", true)));
        assert!(build_rewards(q, "Marauder", &["Freezing Pulse".into()]).iter().all(|g| !g.1));
    }

    #[test]
    fn search_strings() {
        let all: Vec<String> = ["Fireball", "Vaal Fireball", "Frostblink", "Frost Bomb", "Arc", "Arcane Surge Support"].iter().map(|s| s.to_string()).collect();
        let t = short_token("Frostblink", &all);
        assert!(t.len() <= 4 && "frostblink".contains(&t), "{t}");
        assert_eq!(short_token("Arc", &all), "arc"); // inside "Arcane", nothing shorter: whole name
        let (s, left) = gem_search(&["Frostblink".into(), "Frost Bomb".into()], &all);
        assert!(s.starts_with('"') && s.contains('|') && left.is_empty(), "{s}");
        let (s, n) = names_search(&["Divine Orb".into(), "Tabula Rasa".into()]);
        assert_eq!((s.as_str(), n), ("\"divine orb|tabula rasa\"", 2));
        let many: Vec<String> = (0..60).map(|i| format!("Some Long Item Name {i}")).collect();
        let (s, n) = names_search(&many);
        assert!(s.len() <= SEARCH_MAX && n < 60);
    }
}

#[cfg(test)]
mod real_gems {
    #[test]
    fn a_typical_build_fits() {
        let want: Vec<String> = ["Frostblink", "Arcane Surge Support", "Ball Lightning", "Flame Dash", "Fireball", "Added Lightning Damage Support", "Orb of Storms", "Clarity"].iter().map(|s| s.to_string()).collect();
        let (s, left) = super::gem_search(&want, &super::route().gems);
        assert!(left.is_empty() && s.len() < 80, "{s}");
    }
}
