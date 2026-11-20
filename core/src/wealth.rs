//! Wealth tracker: what your stash is worth (poe.ninja prices), how that changes
//! over time, valuable items you haven't put up for sale, and "earnings
//! sessions": name a farming strategy, and see chaos / divines per hour.
//! No network here: the app reads the stash and hands the items over.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A stash tab from the tab list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tab {
    pub index: usize,
    pub name: String,
    pub kind: String,
    /// "#rrggbb"
    pub colour: String,
}

pub fn tabs_from_json(v: &Value) -> Vec<Tab> {
    v["tabs"].as_array().into_iter().flatten().filter(|t| !t["hidden"].as_bool().unwrap_or(false)).map(|t| {
        let c = &t["colour"];
        let ch = |k: &str| c[k].as_u64().unwrap_or(128) as u8;
        Tab {
            index: t["i"].as_u64().unwrap_or(0) as usize,
            name: t["n"].as_str().unwrap_or("").to_string(),
            kind: t["type"].as_str().unwrap_or("").to_string(),
            colour: format!("#{:02x}{:02x}{:02x}", ch("r"), ch("g"), ch("b")),
        }
    }).collect()
}

/// Strips the "<<set:MS>><<set:M>><<set:S>>" markup the stash API puts in names.
fn clean(s: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '<' && b.get(i + 1) == Some(&'<') { depth += 1; i += 2; continue; }
        if b[i] == '>' && b.get(i + 1) == Some(&'>') && depth > 0 { depth -= 1; i += 2; continue; }
        if depth == 0 { out.push(b[i]); }
        i += 1;
    }
    out.trim().to_string()
}

/// How to price one thing: (unique name, base / item name, kind) -> chaos each.
/// kind: "unique", "gem" or "" (currency, fragments, cards, maps…).
pub type Lookup<'a> = &'a dyn Fn(Option<&str>, &str, &str) -> Option<f64>;

/// One stash item's worth, if poe.ninja prices it (rares and plain gear aren't).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Valued {
    pub tab: String,
    pub name: String,
    pub stack: u32,
    /// Chaos for the whole stack.
    pub value: f64,
    pub x: u32,
    pub y: u32,
    /// Already up for sale (has a "~price" note).
    pub listed: bool,
    pub icon: String,
    /// "Worth selling" adds up the same item from several stacks / slots: how many places it's in.
    #[serde(default)]
    pub places: u32,
}

pub fn value_item(item: &Value, tab: &str, lookup: Lookup) -> Option<Valued> {
    let frame = item["frameType"].as_u64().unwrap_or(0);
    let name = clean(item["name"].as_str().unwrap_or(""));
    let type_line = clean(item["typeLine"].as_str().unwrap_or(""));
    let base = item["baseType"].as_str().map(clean).unwrap_or_else(|| type_line.clone());
    let stack = item["stackSize"].as_u64().unwrap_or(1).max(1) as u32;
    let each = match frame {
        3 | 9 => lookup(Some(&name), &base, "unique")?,
        4 => lookup(None, &type_line, "gem")?,
        0..=2 if item["frameType"].is_number() && is_gear(item) => return None,
        _ => if base == "Chaos Orb" { 1.0 } else { lookup(None, &base, "").or_else(|| lookup(None, &type_line, ""))? },
    };
    if each <= 0.0 { return None; }
    let shown = if name.is_empty() { type_line.clone() } else { format!("{name} {base}") };
    Some(Valued {
        tab: tab.to_string(), name: shown, stack, value: each * stack as f64,
        x: item["x"].as_u64().unwrap_or(0) as u32, y: item["y"].as_u64().unwrap_or(0) as u32,
        // A price note on the item, or on the whole tab (a tab named "~price 5 chaos" prices everything in it).
        listed: [item["note"].as_str().unwrap_or(""), tab].iter().any(|n| n.starts_with("~price") || n.starts_with("~b/o")),
        icon: item["icon"].as_str().unwrap_or("").to_string(), places: 1,
    })
}

/// Normal/magic/rare gear and jewels (worth what the trade site says, not poe.ninja).
fn is_gear(item: &Value) -> bool {
    // Maps have tiers and item levels too, but poe.ninja prices them.
    let map = item["typeLine"].as_str().map(|t| t.ends_with(" Map") || t.contains("Waystone")).unwrap_or(false)
        || item["properties"].as_array().into_iter().flatten().any(|p| p["name"].as_str().map(|n| n.starts_with("Map Tier")).unwrap_or(false));
    if map { return false; }
    item["properties"].as_array().map(|p| !p.is_empty()).unwrap_or(false) && item["stackSize"].is_null()
        || item["ilvl"].as_u64().unwrap_or(0) > 0 && item["stackSize"].is_null() && item["frameType"].as_u64().unwrap_or(0) <= 2
}

/// Everything poe.ninja prices in one tab's items.
pub fn value_tab(items: &Value, tab: &str, lookup: Lookup) -> Vec<Valued> {
    items.as_array().into_iter().flatten().filter_map(|it| value_item(it, tab, lookup)).collect()
}

/// One stash scan's total.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    /// Unix seconds.
    pub time: u64,
    pub league: String,
    pub total: f64,
    /// Chaos per divine then.
    pub divine: Option<f64>,
    pub tabs: Vec<(String, f64)>,
    /// Which scan profile it was ("" = the whole stash).
    #[serde(default)]
    pub profile: String,
}

pub fn snapshot(time: u64, league: &str, divine: Option<f64>, valued: &[Valued]) -> Snapshot {
    let mut tabs: Vec<(String, f64)> = Vec::new();
    for v in valued {
        match tabs.iter_mut().find(|t| t.0 == v.tab) { Some(t) => t.1 += v.value, None => tabs.push((v.tab.clone(), v.value)) }
    }
    tabs.sort_by(|a, b| b.1.total_cmp(&a.1));
    Snapshot { time, league: league.into(), total: valued.iter().map(|v| v.value).sum(), divine, tabs, profile: String::new() }
}

/// Worth selling and not listed yet, most valuable first. The same item in several stacks or tabs is
/// added up first: ten Blight threads together can be worth listing when three alone aren't.
pub fn missed(valued: &[Valued], at_least: f64) -> Vec<Valued> {
    let mut groups: Vec<Valued> = Vec::new();
    for it in valued.iter().filter(|v| !v.listed && v.name != "Chaos Orb") {
        match groups.iter_mut().find(|g| g.name == it.name) {
            Some(g) => {
                g.stack += it.stack;
                g.value += it.value;
                g.places += 1;
                if !g.tab.split(", ").any(|t| t == it.tab) { g.tab = format!("{}, {}", g.tab, it.tab); }
            }
            None => groups.push(Valued { places: 1, ..it.clone() }),
        }
    }
    let mut v: Vec<Valued> = groups.into_iter().filter(|g| g.value >= at_least).collect();
    v.sort_by(|a, b| b.value.total_cmp(&a.value));
    v.truncate(150);
    v
}

/// A named farming run: what the tracked tabs were worth at the start and now.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    pub name: String,
    pub league: String,
    pub tabs: Vec<usize>,
    pub start: u64,
    pub end: Option<u64>,
    pub start_value: f64,
    pub last_value: f64,
    /// Seconds spent paused (excluded from the hourly rate).
    #[serde(default)]
    pub paused: u64,
    #[serde(default)]
    pub paused_at: Option<u64>,
}

impl Session {
    pub fn profit(&self) -> f64 { self.last_value - self.start_value }
    pub fn seconds(&self, now: u64) -> u64 {
        let end = self.paused_at.or(self.end).unwrap_or(now);
        end.saturating_sub(self.start).saturating_sub(self.paused)
    }
    /// Chaos per hour (0 for the first minute, it means nothing yet).
    pub fn per_hour(&self, now: u64) -> f64 {
        let s = self.seconds(now);
        if s < 60 { 0.0 } else { self.profit() * 3600.0 / s as f64 }
    }
}

/// Each strategy's average over its finished sessions: (name, sessions, hours, chaos per hour).
pub fn by_strategy(sessions: &[Session]) -> Vec<(String, usize, f64, f64)> {
    let mut out: Vec<(String, usize, f64, f64)> = Vec::new();
    for s in sessions.iter().filter(|s| s.end.is_some()) {
        let hours = s.seconds(0) as f64 / 3600.0;
        match out.iter_mut().find(|o| o.0.eq_ignore_ascii_case(&s.name)) {
            Some(o) => { o.1 += 1; o.2 += hours; o.3 += s.profit(); }
            None => out.push((s.name.clone(), 1, hours, s.profit())),
        }
    }
    for o in &mut out { o.3 = if o.2 > 0.0 { o.3 / o.2 } else { 0.0 }; }
    out.sort_by(|a, b| b.3.total_cmp(&a.3));
    out
}

/// Everything the wealth tracker keeps (wealth.json in the app's data folder).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WealthStore {
    pub tabs: Vec<Tab>,
    pub snapshots: Vec<Snapshot>,
    pub sessions: Vec<Session>,
    pub current: Option<Session>,
    pub missed: Vec<Valued>,
    /// Every priced item from the last scan (most valuable first, at most 3000), for the "By tab" breakdown.
    pub last_items: Vec<Valued>,
    /// Named sets of tabs to scan (like WealthyExile's profiles), and the one in use (None = whole stash).
    pub profiles: Vec<ScanProfile>,
    pub active: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanProfile {
    pub name: String,
    pub tabs: Vec<usize>,
}

impl WealthStore {
    pub fn load(path: &std::path::Path) -> WealthStore {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
    /// Keeps the history from growing forever (newest 400 scans).
    pub fn add_snapshot(&mut self, s: Snapshot) {
        self.snapshots.push(s);
        let n = self.snapshots.len();
        if n > 400 { self.snapshots.drain(..n - 400); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lookup(name: Option<&str>, base: &str, kind: &str) -> Option<f64> {
        match (name, base, kind) {
            (Some("Headhunter"), _, "unique") => Some(30000.0),
            (_, "Divine Orb", "") => Some(210.0),
            (_, "Orb of Alchemy", "") => Some(0.3),
            (_, "Vaal Grace", "gem") => Some(5.0),
            (_, "Strand Map", "") => Some(2.0),
            _ => None,
        }
    }

    #[test]
    fn values_a_tab() {
        let items = json!([
            { "frameType": 5, "typeLine": "Divine Orb", "baseType": "Divine Orb", "stackSize": 3, "x": 0, "y": 0 },
            { "frameType": 5, "typeLine": "Chaos Orb", "baseType": "Chaos Orb", "stackSize": 40, "x": 1, "y": 0 },
            { "frameType": 5, "typeLine": "Orb of Alchemy", "baseType": "Orb of Alchemy", "stackSize": 10, "x": 2, "y": 0 },
            { "frameType": 3, "name": "<<set:MS>><<set:M>><<set:S>>Headhunter", "typeLine": "Leather Belt", "baseType": "Leather Belt", "ilvl": 80, "x": 3, "y": 0, "note": "~price 150 divine" },
            { "frameType": 2, "name": "Doom Loop", "typeLine": "Ruby Ring", "baseType": "Ruby Ring", "ilvl": 84, "x": 4, "y": 0 },
            { "frameType": 0, "typeLine": "Strand Map", "baseType": "Strand Map", "ilvl": 83, "x": 6, "y": 0, "properties": [ { "name": "Map Tier" } ] },
            { "frameType": 4, "typeLine": "Vaal Grace", "baseType": "Vaal Grace", "ilvl": 0, "x": 5, "y": 0, "properties": [ { "name": "Level" } ] },
        ]);
        let f = |n: Option<&str>, b: &str, k: &str| lookup(n, b, k);
        let v = value_tab(&items, "Dump", &f);
        assert_eq!(v.len(), 6, "rare ring not priced, the map is");
        assert_eq!(v[0].value, 630.0);
        assert_eq!(v[1].value, 40.0);
        assert!((v[2].value - 3.0).abs() < 1e-9);
        assert_eq!((v[3].name.as_str(), v[3].listed), ("Headhunter Leather Belt", true));
        // Everything in a tab named "~price 5 chaos" is up for sale.
        let priced_tab = value_tab(&json!([{ "frameType": 5, "typeLine": "Divine Orb", "baseType": "Divine Orb", "stackSize": 1, "x": 0, "y": 0 }]), "~price 5 chaos", &f);
        assert!(priced_tab[0].listed);
        let snap = snapshot(100, "Covens", Some(210.0), &v);
        assert!((snap.total - 30680.0).abs() < 1e-6);
        assert_eq!(snap.tabs, vec![("Dump".to_string(), snap.total)]);
        let m = missed(&v, 10.0);
        assert_eq!(m.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["Divine Orb"], "chaos and listed items aren't 'missed'");
        let tabs = tabs_from_json(&json!({ "tabs": [ { "n": "Dump", "i": 0, "type": "PremiumStash", "colour": { "r": 255, "g": 0, "b": 16 } },
            { "n": "Hidden", "i": 1, "type": "MapStash", "hidden": true, "colour": {} } ] }));
        assert_eq!(tabs, vec![Tab { index: 0, name: "Dump".into(), kind: "PremiumStash".into(), colour: "#ff0010".into() }]);
    }


    #[test]
    fn worth_selling_adds_up_stacks() {
        let it = |tab: &str, stack: u32, value: f64| Valued { tab: tab.into(), name: "Blighted Thread".into(), stack, value, x: 0, y: 0,
            listed: false, icon: String::new(), places: 1 };
        // 3 + 7 threads at 1.2c each: neither stack reaches 10c, together they do.
        let v = vec![it("Dump 1", 3, 3.6), it("Dump 2", 7, 8.4), it("Dump 2", 0, 0.0)];
        let m = missed(&v, 10.0);
        assert_eq!(m.len(), 1);
        assert_eq!((m[0].stack, m[0].places, m[0].tab.as_str()), (10, 3, "Dump 1, Dump 2"));
        assert!((m[0].value - 12.0).abs() < 1e-9);
        assert!(missed(&v[..1], 10.0).is_empty());
    }
    #[test]
    fn sessions_per_hour() {
        let mut s = Session { name: "Strand alch & go".into(), league: "Covens".into(), tabs: vec![0], start: 1000, end: None,
            start_value: 500.0, last_value: 500.0, paused: 0, paused_at: None };
        assert_eq!(s.per_hour(1030), 0.0);
        s.last_value = 800.0; // +300 in 30 minutes
        assert_eq!(s.per_hour(1000 + 1800), 600.0);
        s.paused = 600; // 10 of those minutes paused
        assert_eq!(s.per_hour(1000 + 1800), 900.0);
        s.end = Some(1000 + 1800);
        let other = Session { name: "strand ALCH & go".into(), end: Some(1000 + 3600), last_value: 1400.0, paused: 0, ..s.clone() };
        let b = by_strategy(&[s, other]);
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].1, 2);
        // 300 + 900 chaos over 20 min + 60 min.
        assert!((b[0].3 - 900.0).abs() < 1e-6);
    }
}
