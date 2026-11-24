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

/// Areas the map device opens that aren't named "Map…": Originator memories and Venarius
/// ("Synthesis_MapGuardian4" = Rewritten Distant Memory, 23 Nov), conquerors and Sirus, the Maven,
/// breachstones and league bosses. Not the Maven's Crucible hub or sub-areas inside a map.
const DEVICE_AREAS: &[&str] = &["Synthesis_Map", "AtlasExilesBoss", "MavenBoss", "BreachBoss", "AbyssLeagueBoss",
    "AzmeriLeagueBoss", "FaridunLeagueBoss", "ChayulaLeagueTowerBoss"];

/// Map areas (PoE 1 "MapWorlds…", PoE 2 "Map…", plus [`DEVICE_AREAS`]); not hideouts, towns or the campaign.
pub fn is_map_area(id: &str) -> bool {
    (id.starts_with("Map") && !id.contains("Hideout") && !id.starts_with("MapDevice"))
        || DEVICE_AREAS.iter().any(|p| id.starts_with(p))
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
    /// Seconds the timer stood still while the game was paused (Escape menu) or the skill tree was open.
    #[serde(default)]
    pub paused: i64,
    /// What you picked up in it (from your inventory before and after each visit), priced.
    #[serde(default)]
    pub loot: Vec<crate::loot::LootLine>,
    /// The atlas tree in use when it started (Atlas page id), so loot can be compared per tree.
    #[serde(default)]
    pub atlas: Option<u64>,
}

impl MapRun {
    /// PoE 1 map tier from the area level (68 = tier 1).
    pub fn tier(&self) -> Option<u32> { (self.level >= 68).then(|| self.level - 67) }
    /// Time inside so far (counting the current visit if `inside` and `now` given). Pass `inside` = false while
    /// paused: the visit up to the pause is already counted in `in_map`.
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
    /// The timer is stopped (Escape menu or skill tree open) since then.
    #[serde(default)]
    pub paused_since: Option<i64>,
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
                // A zone loading means no menu is open any more.
                self.set_paused(false, *time);
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
        self.set_paused(false, now);
        let mut c = self.current.take()?;
        if self.inside { c.in_map += (now - c.last_enter).max(0); c.left = now; }
        self.inside = false;
        self.history.push(c.clone());
        if self.history.len() > 500 { let n = self.history.len() - 500; self.history.drain(..n); }
        Some(c)
    }

    /// Stops or restarts the clock (the game paused / a menu covering the game). Only counts while you're inside a
    /// map. Returns true when something changed.
    pub fn set_paused(&mut self, paused: bool, now: i64) -> bool {
        match (paused, self.paused_since) {
            (true, None) if self.inside => {
                let Some(c) = self.current.as_mut() else { return false };
                let now = now.max(c.last_enter);
                c.in_map += now - c.last_enter;
                c.last_enter = now;
                self.paused_since = Some(now);
                true
            }
            (false, Some(since)) => {
                self.paused_since = None;
                if let Some(c) = self.current.as_mut() {
                    let now = now.max(since);
                    c.paused += now - since;
                    // The visit goes on from now (the paused stretch is already banked out of in_map).
                    if self.inside { c.last_enter = now; }
                }
                true
            }
            _ => false,
        }
    }

    /// Seconds inside the current map right now (0 without one), standing still while paused.
    pub fn current_inside(&self, now: i64) -> i64 {
        self.current.as_ref().map(|c| c.time_inside(self.inside && self.paused_since.is_none(), now)).unwrap_or(0)
    }
}

// ---------- is a menu covering the game? ----------

/// Points to sample on the bottom of the game's screen (the life/mana bar, flasks and skill bar frame): the passive tree
/// and the Escape menu cover or darken them. `rect` = the game's drawing area (x, y, width, height) on screen.
pub fn hud_points(rect: (i32, i32, i32, i32)) -> Vec<(i32, i32)> {
    let (x, y, w, h) = rect;
    let mut out = vec![];
    for row in 0..4 {
        let fy = 0.955 + row as f64 * 0.012; // bottom 4.5% of the screen
        for col in 0..28 {
            let fx = 0.14 + col as f64 * (0.72 / 27.0);
            out.push((x + (fx * w as f64) as i32, y + (fy * h as f64) as i32));
        }
    }
    out
}

/// Points in the middle of the screen, where the Escape menu's panel sits.
pub fn centre_points(rect: (i32, i32, i32, i32)) -> Vec<(i32, i32)> {
    let (x, y, w, h) = rect;
    let mut out = vec![];
    for row in 0..10 {
        for col in 0..8 {
            let fx = 0.42 + col as f64 * (0.16 / 7.0);
            let fy = 0.30 + row as f64 * (0.40 / 9.0);
            out.push((x + (fx * w as f64) as i32, y + (fy * h as f64) as i32));
        }
    }
    out
}

/// Share (0..1) of points whose colour changed clearly between two samples.
pub fn changed_share(a: &[[u8; 3]], b: &[[u8; 3]]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 { return 0.0; }
    let changed = a.iter().zip(b).filter(|(p, q)| (0..3).any(|i| (p[i] as i32 - q[i] as i32).abs() > 40)).count();
    changed as f64 / n as f64
}

/// What the game looks like with nothing open (and, if set up, with the Escape menu open), saved by "Set up".
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MenuLook {
    /// The bottom bar with no menu open.
    pub hud: Vec<[u8; 3]>,
    /// The middle of the screen with the Escape menu open (when set up).
    pub esc: Vec<[u8; 3]>,
    /// The game's size when it was set up (a different size needs setting up again).
    pub size: (i32, i32),
}

impl MenuLook {
    /// Is a menu covering the game? `hud`/`centre` = the same points sampled now.
    pub fn covered(&self, hud: &[[u8; 3]], centre: &[[u8; 3]]) -> bool {
        // Most of the bottom bar looks different: the skill tree (or the Escape menu darkening everything) is up.
        // Flask charges and skill cooldowns only ever change a few of the points.
        let bar_gone = !self.hud.is_empty() && changed_share(&self.hud, hud) >= 0.55;
        // The middle matches the Escape menu's panel.
        let esc_up = !self.esc.is_empty() && !centre.is_empty() && changed_share(&self.esc, centre) <= 0.2;
        bar_gone || esc_up
    }

    /// Like `covered`, but a menu also has to hold still: the skill tree and the Escape menu don't move, while play
    /// does. Points that land on the game world instead of the bar (his 4K screen, 23 Nov: Forge of the Phoenix
    /// counted 12 s with 159 s "paused") change every second while you play, so they never stop the clock.
    /// `prev` = the bar sampled a second earlier (empty at first).
    pub fn covered_still(&self, hud: &[[u8; 3]], prev: &[[u8; 3]], centre: &[[u8; 3]]) -> bool {
        let still = prev.is_empty() || changed_share(prev, hud) < 0.15;
        still && self.covered(hud, centre)
    }
}

/// A map that was "paused" for most of the time you were in it: the normal look of the bar was learnt wrong, so it's
/// learnt again on the next map. Alt-tabbed stretches count too, which at worst means learning it again.
pub fn pause_look_suspect(in_map: i64, paused: i64) -> bool {
    paused >= 60 && paused > in_map * 2
}

/// Smooths the once-a-second check: the clock only stops or starts after two readings in a row agree.
#[derive(Debug, Clone, Default)]
pub struct Steady { shown: bool, streak: u32 }

impl Steady {
    /// Feeds one reading; returns Some(new state) when it flips.
    pub fn feed(&mut self, covered: bool) -> Option<bool> {
        if covered == self.shown { self.streak = 0; return None; }
        self.streak += 1;
        if self.streak >= 2 { self.shown = covered; self.streak = 0; return Some(covered); }
        None
    }
    pub fn reset(&mut self) { *self = Steady::default(); }
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

/// Where the play session you're in began, when it began before `now` with maps already run: walks back through the
/// runs while each one ended less than `gap` seconds before the next began (or before now). Returns (start, deaths in
/// those runs). Used when the editor starts mid-session, so the session card counts the maps you'd already done.
pub fn session_start(history: &[MapRun], current: Option<&MapRun>, now: i64, gap: i64) -> Option<(i64, u32)> {
    let mut runs: Vec<&MapRun> = history.iter().collect();
    if let Some(c) = current { if !history.iter().any(|h| h.start == c.start && h.area == c.area) { runs.push(c); } }
    let mut next = now;
    let mut found: Option<(i64, u32)> = None;
    for r in runs.iter().rev() {
        let end = r.left.max(r.start);
        if next - end > gap { break; }
        found = Some((r.start, found.map_or(0, |f| f.1) + r.deaths));
        next = r.start;
    }
    found
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

/// PoE 2's campaign route (from Exile UI's leveling guide, MIT; `node tools/build_levelling_poe2.mjs`). Its area ids are
/// lower case ("g1_4"); the game logs "G1_4", so look them up with `area_key`.
pub fn route_poe2() -> &'static Route {
    static R: OnceLock<Route> = OnceLock::new();
    R.get_or_init(|| serde_json::from_str(include_str!("../data/levelling_poe2.json")).expect("levelling_poe2.json"))
}

pub fn route_for(poe2: bool) -> &'static Route { if poe2 { route_poe2() } else { route() } }

/// The id a route knows an area by (PoE 2's are lower case).
pub fn area_key(poe2: bool, id: &str) -> String { if poe2 { id.to_lowercase() } else { id.to_string() } }

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
    fn map_areas() {
        for id in ["MapWorldsStrand", "Synthesis_MapGuardian4", "Synthesis_MapBoss", "AtlasExilesBoss5", "MavenBoss", "MapAtziri"] {
            assert!(is_map_area(id), "{id}");
        }
        for id in ["HideoutSpace", "MapWorldsHideout", "MavenHub", "2_11_endgame_town", "KalguuranSettlersLeague", "DeepwaterEncounter"] {
            assert!(!is_map_area(id), "{id}");
        }
    }

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
    fn session_carries_on() {
        // His 22 Nov evening: Colonnade, Cursed Crypt, then the editor restarted, then Park. The session should
        // still count both earlier maps (it said 1 while Today said 2).
        let run = |start: i64, left: i64, deaths: u32| MapRun { start, left, in_map: left - start, deaths, ..Default::default() };
        let hist = vec![run(0, 400, 0), run(10_000, 10_400, 3), run(10_620, 10_860, 0)];
        let cur = run(11_570, 11_720, 0);
        assert_eq!(session_start(&hist, Some(&cur), 11_800, 1800), Some((10_000, 3)));
        // Long after the last map: no session to carry on.
        assert_eq!(session_start(&hist, None, 20_000, 1800), None);
    }

    #[test]
    fn menu_must_hold_still() {
        let normal: Vec<[u8; 3]> = (0..100).map(|i| [i as u8, 50, 50]).collect();
        let look = MenuLook { hud: normal.clone(), ..Default::default() };
        // Playing with points on the moving world: very different from the learnt look, and from a second ago.
        let a: Vec<[u8; 3]> = (0..100).map(|i| [(i * 2) as u8, 200, 10]).collect();
        let b: Vec<[u8; 3]> = (0..100).map(|i| [(i * 2) as u8, 10, 200]).collect();
        assert!(look.covered(&b, &[]), "the old check called this a menu");
        assert!(!look.covered_still(&b, &a, &[]), "moving: not a menu");
        // A real menu: different from the normal bar, and the same as a second ago.
        assert!(look.covered_still(&b, &b, &[]));
        // The normal bar: never a menu.
        assert!(!look.covered_still(&normal, &normal, &[]));
        // His runs: 12 s in, 159 s paused → learn again; a 30 s Escape break in a 3 minute map → keep it.
        assert!(pause_look_suspect(12, 159));
        assert!(!pause_look_suspect(150, 30));
        assert!(!pause_look_suspect(10, 40));
    }

    #[test]
    fn pauses_in_menus() {
        let mut m = MapTracker::default();
        m.on_event(&area(0, "MapWorldsStrand", 5));
        assert_eq!(m.current_inside(100), 100);
        assert!(m.set_paused(true, 100)); // Escape menu at 1:40
        assert!(!m.set_paused(true, 120)); // still open
        assert_eq!(m.current_inside(160), 100); // clock stands still
        assert!(m.set_paused(false, 160));
        assert_eq!(m.current_inside(200), 140);
        // Paused, then portal out: the hideout load ends the pause and the visit.
        m.set_paused(true, 200);
        m.on_event(&area(230, "HideoutFelled", 1));
        assert!(!m.inside && m.paused_since.is_none());
        assert_eq!(m.current.as_ref().unwrap().in_map, 140);
        assert_eq!(m.current.as_ref().unwrap().paused, 90);
        // Not in a map: nothing to pause.
        assert!(!m.set_paused(true, 240));
        m.on_event(&area(250, "MapWorldsStrand", 5));
        m.set_paused(true, 300);
        let done = m.finish(400).unwrap();
        assert_eq!((done.in_map, done.paused), (190, 190));
    }

    #[test]
    fn spots_menus() {
        let rect = (0, 0, 1920, 1080);
        let hp = hud_points(rect);
        assert_eq!(hp.len(), 112);
        assert!(hp.iter().all(|p| p.1 >= 1030 && p.1 < 1080 && p.0 > 260 && p.0 < 1660));
        assert!(centre_points(rect).iter().all(|p| p.0 > 800 && p.0 < 1120));
        let bar: Vec<[u8; 3]> = (0..112).map(|i| [(i * 2) as u8, 60, 90]).collect();
        let look = MenuLook { hud: bar.clone(), esc: vec![], size: (1920, 1080) };
        // A few flasks / cooldowns changed: still playing.
        let mut busy = bar.clone();
        for p in busy.iter_mut().take(20) { *p = [250, 250, 250]; }
        assert!(!look.covered(&busy, &[]));
        // Everything darkened (Escape menu) or replaced (skill tree): covered.
        let dark: Vec<[u8; 3]> = bar.iter().map(|p| [p[0] / 3, p[1] / 3, p[2] / 3]).collect();
        assert!(look.covered(&dark, &[]));
        let tree: Vec<[u8; 3]> = (0..112).map(|_| [10, 10, 10]).collect();
        assert!(look.covered(&tree, &[]));
        // The Escape panel in the middle, set up: seen even if the bar didn't change.
        let panel: Vec<[u8; 3]> = (0..80).map(|i| [40 + (i % 7) as u8 * 20, 30, 20]).collect();
        let look = MenuLook { esc: panel.clone(), ..look };
        assert!(look.covered(&bar, &panel));
        let world: Vec<[u8; 3]> = (0..80).map(|i| [200, (i * 3) as u8, 100]).collect();
        assert!(!look.covered(&bar, &world));
        // Two readings in a row before the clock stops or starts.
        let mut s = Steady::default();
        assert_eq!(s.feed(true), None);
        assert_eq!(s.feed(true), Some(true));
        assert_eq!(s.feed(true), None);
        assert_eq!(s.feed(false), None);
        assert_eq!(s.feed(true), None); // a blip doesn't count
        assert_eq!(s.feed(false), None);
        assert_eq!(s.feed(false), Some(false));
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
    fn poe2_route_follows_zones() {
        let r = route_poe2();
        assert!(r.steps.len() > 200);
        let vis = steps_for(r, &RouteOptions::default());
        let p = advance(r, &vis, 0, &area_key(true, "G1_2")).expect("Clearfell");
        assert!(p > 0);
        assert!(r.areas.contains_key("g1_town"));
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

// ---------- chat commands (24 Nov) ----------

/// A command you typed in game chat: "!next" in local / party / guild chat, or whispered to yourself
/// ("@YourName !next", which nobody else sees). Only your own lines count (`me` = your character's name).
/// Returns the command without the "!" ("next", "filter strict").
pub fn chat_command(line: &str, me: &str) -> Option<String> {
    if me.is_empty() { return None; }
    let body = &line.trim_end()[line.find("] ")? + 2..];
    let (who, msg) = if let Some(rest) = body.strip_prefix("@To ") {
        rest.split_once(": ")?
    } else {
        if body.starts_with(": ") || body.starts_with("@From ") { return None; }
        let (who, msg) = body.split_once(": ")?;
        (who.trim_start_matches(['#', '%', '&', '$']), msg)
    };
    // "<GUILD> Name" -> "Name".
    let who = match who.strip_prefix('<') { Some(r) => r.split_once("> ").map(|x| x.1).unwrap_or(r), None => who };
    if who.trim() != me { return None; }
    let cmd = msg.trim().strip_prefix('!')?.trim();
    (!cmd.is_empty()).then(|| cmd.to_lowercase())
}

#[cfg(test)]
mod chat_tests {
    use super::chat_command;
    const T: &str = "2026/11/24 19:48:01 1 a [INFO Client 9]";
    #[test]
    fn own_commands_only() {
        assert_eq!(chat_command(&format!("{T} Bob_Witch: !next"), "Bob_Witch").as_deref(), Some("next"));
        assert_eq!(chat_command(&format!("{T} %<GLD> Bob_Witch: !filter Strict"), "Bob_Witch").as_deref(), Some("filter strict"));
        assert_eq!(chat_command(&format!("{T} @To Bob_Witch: !done"), "Bob_Witch").as_deref(), Some("done"));
        assert_eq!(chat_command(&format!("{T} @From Bob_Witch: !done"), "Bob_Witch"), None);
        assert_eq!(chat_command(&format!("{T} Someone: !next"), "Bob_Witch"), None);
        assert_eq!(chat_command(&format!("{T} Bob_Witch: hello"), "Bob_Witch"), None);
        assert_eq!(chat_command(&format!("{T} : Bob_Witch has joined the area."), "Bob_Witch"), None);
        assert_eq!(chat_command(&format!("{T} Bob_Witch: !next"), ""), None);
    }
}

// ---------- race splits (24 Nov) ----------

/// One character's run through the campaign: when it reached each act and level, from level 2 on.
/// (A live ladder rank would need GGG's ladder, which is only open to approved OAuth apps, and pathofexile.com no longer
/// sends experience; so this races you against your own earlier characters.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Race {
    pub class: String,
    /// When it reached level 2 (unix seconds): the race clock starts there.
    pub start: i64,
    /// (act, seconds since start) the first time you entered it; act 11 = maps.
    pub acts: Vec<(u32, i64)>,
    /// (level, seconds since start).
    pub levels: Vec<(u32, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Races { pub chars: std::collections::BTreeMap<String, Race> }

impl Races {
    /// A level-up line. Level 2 starts (or restarts) the character's race.
    pub fn level(&mut self, name: &str, class: &str, level: u32, time: i64) {
        if level == 2 { self.chars.insert(name.to_string(), Race { class: class.to_string(), start: time, ..Default::default() }); }
        if let Some(r) = self.chars.get_mut(name) {
            if !r.levels.iter().any(|l| l.0 == level) { r.levels.push((level, time - r.start)); }
        }
    }
    /// Entering a zone of `act` (11 = a map).
    pub fn act(&mut self, name: &str, act: u32, time: i64) {
        if let Some(r) = self.chars.get_mut(name) {
            if !r.acts.iter().any(|a| a.0 >= act) { r.acts.push((act, time - r.start)); }
        }
    }
    /// Your fastest time to each act and level over your other characters.
    pub fn best_except(&self, name: &str) -> (Vec<(u32, i64)>, Vec<(u32, i64)>) {
        let mut acts: Vec<(u32, i64)> = vec![];
        let mut levels: Vec<(u32, i64)> = vec![];
        for (n, r) in &self.chars {
            if n == name { continue; }
            for &(a, t) in &r.acts { match acts.iter_mut().find(|x| x.0 == a) { Some(x) => x.1 = x.1.min(t), None => acts.push((a, t)) } }
            for &(l, t) in &r.levels { match levels.iter_mut().find(|x| x.0 == l) { Some(x) => x.1 = x.1.min(t), None => levels.push((l, t)) } }
        }
        acts.sort(); levels.sort();
        (acts, levels)
    }
}

#[cfg(test)]
mod race_tests {
    use super::Races;
    #[test]
    fn splits_against_your_best() {
        let mut r = Races::default();
        r.level("Old", "Witch", 2, 0);
        r.act("Old", 2, 1200);
        r.act("Old", 2, 1300); // only the first entry counts
        r.level("Old", "Witch", 10, 900);
        r.level("New", "Ranger", 2, 5000);
        r.act("New", 2, 5000 + 1000);
        r.act("New", 1, 5000 + 1100); // going back doesn't add an earlier act
        let (acts, levels) = r.best_except("New");
        assert_eq!(acts, vec![(2, 1200)]);
        assert_eq!(levels, vec![(2, 0), (10, 900)]);
        assert_eq!(r.chars["New"].acts, vec![(2, 1000)]);
        r.level("New", "Ranger", 2, 9000); // a new character with the same name starts over
        assert!(r.chars["New"].acts.is_empty());
    }
}
