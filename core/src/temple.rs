//! Temples of Atzoatl (24 Nov 2026, he asked: "each room sells for x amount, some setups are worth heaps").
//! The trade site finds Chronicles by their open rooms, so a temple's worth is what temples with its best rooms ask.
//! `data/temples.json` is a survey of every top-tier room (`tools/build_temples.mjs`); the app re-reads the ones
//! worth something now and then (`farm_app`), leaving the trade site's limits to your own price checks.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub const CHRONICLE: &str = "Chronicle of Atzoatl";
/// Temples under this area level sell for less: the survey and the searches leave them out.
pub const MIN_LEVEL: u32 = 75;
/// A room is worth ticking (and re-reading) when temples with it ask this many times a plain temple.
const WORTH: f64 = 2.0;

/// What temples with a room (or two together) ask, in chaos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoomPrice {
    /// "Locus of Corruption (Tier 3)", or "… + …".
    pub label: String,
    /// The trade site's stat for each room.
    pub stats: Vec<String>,
    pub listed: u32,
    /// The middle of the ten cheapest.
    pub typical: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Temples {
    /// When they were read (ISO).
    pub checked: String,
    /// A temple with nothing special.
    pub plain: f64,
    pub rooms: Vec<RoomPrice>,
}

impl Temples {
    pub fn shipped() -> &'static Temples {
        static D: OnceLock<Temples> = OnceLock::new();
        D.get_or_init(|| serde_json::from_str(include_str!("../data/temples.json")).expect("temples.json"))
    }
    /// Rooms and pairs worth having, dearest first.
    pub fn worth(&self) -> Vec<&RoomPrice> {
        let mut v: Vec<&RoomPrice> = self.rooms.iter().filter(|r| r.listed > 0 && r.typical >= (self.plain * WORTH).max(5.0)).collect();
        v.sort_by(|a, b| b.typical.total_cmp(&a.typical));
        v
    }
    /// Is this room's stat worth ticking?
    pub fn good(&self, stat: &str) -> bool { self.worth().iter().any(|r| r.stats.len() == 1 && r.stats[0] == stat) }
}

/// A search for temples with these rooms open.
pub fn query(stats: &[String]) -> serde_json::Value {
    let open: Vec<serde_json::Value> = stats.iter().map(|s| serde_json::json!({ "id": s, "value": { "option": 1 } })).collect();
    serde_json::json!({ "query": { "status": { "option": "available" }, "type": CHRONICLE, "stats": [{ "type": "and", "filters": open }],
        "filters": { "map_filters": { "filters": { "area_level": { "min": MIN_LEVEL } } } } }, "sort": { "price": "asc" } })
}

/// The middle of the cheapest listings (chaos, cheapest first).
pub fn typical(prices: &[f64]) -> Option<f64> { (!prices.is_empty()).then(|| prices[(prices.len() / 2).min(prices.len() - 1)]) }

/// Unix seconds as "2026-11-24T06:00:00Z".
pub fn iso(t: i64) -> String {
    let (days, s) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", s / 3600, s / 60 % 60, s % 60)
}

/// A copied Chronicle's open rooms and area level.
pub fn read(text: &str) -> Option<(Vec<String>, Option<u32>)> {
    let text = text.replace('\r', "");
    if !text.lines().any(|l| l.trim() == CHRONICLE) { return None; }
    let level = text.lines().find_map(|l| l.trim().strip_prefix("Area Level: ")).and_then(|v| v.split_whitespace().next()?.parse().ok());
    let rooms = text.split("--------").find(|s| s.trim_start().starts_with("Open Rooms:"))?
        .lines().map(str::trim).filter(|l| !l.is_empty() && *l != "Open Rooms:" && *l != "Obstructed Rooms:")
        .map(String::from).collect();
    // Obstructed rooms are listed under their own heading and can't be reached: only the open ones count.
    Some((rooms, level))
}

#[cfg(test)]
mod tests {
    use super::*;

    const COPY: &str = "Item Class: Misc Map Items\r\nRarity: Currency\r\nChronicle of Atzoatl\r\n--------\r\nArea Level: 80\r\n--------\r\nOpen Rooms:\r\nLocus of Corruption (Tier 3)\r\nHalls\r\nApex of Atzoatl\r\n--------\r\nObstructed Rooms:\r\nDoryani's Institute (Tier 3)\r\n--------\r\nRight click this item to view the Temple of Atzoatl.\r\n";

    #[test]
    fn reads_open_rooms() {
        let (rooms, level) = read(COPY).unwrap();
        assert_eq!(rooms, ["Locus of Corruption (Tier 3)", "Halls", "Apex of Atzoatl"]);
        assert_eq!(level, Some(80));
        assert!(read("Item Class: Maps\nRarity: Rare\nx").is_none());
    }

    #[test]
    fn dates() {
        assert_eq!(iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso(1_795_488_737), "2026-11-24T02:52:17Z");
    }

    #[test]
    fn survey_is_usable() {
        let t = Temples::shipped();
        assert!(t.plain > 0.0 && t.rooms.len() > 10);
        let w = t.worth();
        assert!(!w.is_empty() && w.windows(2).all(|x| x[0].typical >= x[1].typical));
        assert!(t.rooms.iter().all(|r| r.stats.iter().all(|s| s.starts_with("pseudo.pseudo_temple_"))));
        assert_eq!(query(&w[0].stats)["query"]["stats"][0]["filters"][0]["value"]["option"], 1);
    }
}
