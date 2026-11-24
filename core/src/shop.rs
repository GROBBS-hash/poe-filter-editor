//! Your shop (24 Nov 2026): what's listed in your stash, and what sold since the last look. PoE has no "sold" event, so
//! a sale is an item with a price note that's gone (or a stack that got smaller) between two reads of your trade tabs.
//! Moving a listed item out by hand looks the same: the phone message says "sold (or moved)".
//!
//! Also the short texts the phone alerts send.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A listed item in a stash tab.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Listed {
    pub id: String,
    pub name: String,
    pub stack: u32,
    /// "~price 5 chaos" (the item's note, or the tab's name for a priced tab).
    pub note: String,
    pub tab: String,
}

/// One sale (or an item moved out of a trade tab).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sale {
    /// Unix seconds.
    pub time: i64,
    pub name: String,
    pub count: u32,
    pub note: String,
    pub tab: String,
    /// The price note in chaos for the whole sale, when it could be read.
    pub chaos: Option<f64>,
}

fn clean(s: &str) -> String {
    // The stash API wraps some names in <<set:MS>>-style markers.
    let mut out = s.to_string();
    while let (Some(a), Some(b)) = (out.find("<<"), out.find(">>")) {
        if b < a { break; }
        out.replace_range(a..b + 2, "");
    }
    out.trim().to_string()
}

fn is_price(n: &str) -> bool { n.starts_with("~price") || n.starts_with("~b/o") }

/// The listed items in one stash tab's JSON (`items` array).
pub fn listed_in_tab(items: &Value, tab: &str) -> Vec<Listed> {
    items.as_array().into_iter().flatten().filter_map(|it| {
        let note = it["note"].as_str().unwrap_or("");
        let note = if is_price(note) { note } else if is_price(tab) { tab } else { return None };
        let name = clean(it["name"].as_str().unwrap_or(""));
        let type_line = clean(it["typeLine"].as_str().unwrap_or(""));
        Some(Listed {
            id: it["id"].as_str()?.to_string(),
            name: if name.is_empty() { type_line } else { format!("{name} {type_line}") },
            stack: it["stackSize"].as_u64().unwrap_or(1).max(1) as u32,
            note: note.to_string(),
            tab: tab.to_string(),
        })
    }).collect()
}

/// What's gone between two reads of the same tabs: whole items gone, or stacks that got smaller.
/// `chaos_of(note, count)`: the note's price in chaos for that many.
pub fn sold(before: &[Listed], now: &[Listed], time: i64, chaos_of: &dyn Fn(&str, u32) -> Option<f64>) -> Vec<Sale> {
    let mut out = vec![];
    for b in before {
        let count = match now.iter().find(|n| n.id == b.id) {
            None => b.stack,
            Some(n) if n.stack < b.stack => b.stack - n.stack,
            _ => continue,
        };
        out.push(Sale { time, name: b.name.clone(), count, note: b.note.clone(), tab: b.tab.clone(), chaos: chaos_of(&b.note, count) });
    }
    out
}

/// The note's price for `count` of the item: "~price 5 chaos" is per item; "~price 10/20 chaos" is 10 chaos for 20.
/// `rate`: chaos per currency id.
pub fn note_chaos(note: &str, count: u32, rate: &dyn Fn(&str) -> Option<f64>) -> Option<f64> {
    let p = note.trim_start_matches("~price").trim_start_matches("~b/o").trim();
    let (amount, cur) = p.split_once(' ')?;
    let (a, per) = match amount.split_once('/') {
        Some((a, n)) => (a.parse::<f64>().ok()?, n.parse::<f64>().ok()?),
        None => (amount.parse::<f64>().ok()?, 1.0),
    };
    let (_, id) = crate::whispers::read_price(&format!("1 {cur}"))?;
    Some(a / per.max(1e-9) * count as f64 * rate(&id)?)
}

/// The phone message for a buyer's whisper: ("Buyer: Bob", "Tabula Rasa for 5 chaos · Fair price").
pub fn whisper_text(from: &str, item: &str, price: Option<&str>, verdict: Option<&str>) -> (String, String) {
    let v = match verdict { Some("low") => " · Underpriced?", Some("high") => " · Good sale", Some("fair") => " · Fair price", _ => "" };
    (format!("Buyer: {from}"), format!("{item}{}{v}", price.map(|p| format!(" for {p}")).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn finds_sales() {
        let tab = json!([
            { "id": "a", "typeLine": "Divine Orb", "stackSize": 5, "note": "~price 180 chaos" },
            { "id": "b", "name": "<<set:MS>>Tabula Rasa", "typeLine": "Simple Robe", "note": "~b/o 10 chaos" },
            { "id": "c", "typeLine": "Chaos Orb", "stackSize": 40 },
        ]);
        let before = listed_in_tab(&tab, "Shop");
        assert_eq!(before.len(), 2);
        assert_eq!(before[1].name, "Tabula Rasa Simple Robe");
        let after = listed_in_tab(&json!([{ "id": "a", "typeLine": "Divine Orb", "stackSize": 3, "note": "~price 180 chaos" }]), "Shop");
        let rate = |id: &str| if id == "chaos" { Some(1.0) } else { None };
        let s = sold(&before, &after, 100, &|n, c| note_chaos(n, c, &rate));
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].name.as_str(), s[0].count, s[0].chaos), ("Divine Orb", 2, Some(360.0)));
        assert_eq!((s[1].count, s[1].chaos), (1, Some(10.0)));
        // A priced tab prices everything in it.
        assert_eq!(listed_in_tab(&json!([{ "id": "x", "typeLine": "Exalted Orb" }]), "~price 15 chaos").len(), 1);
        assert_eq!(note_chaos("~price 10/20 chaos", 4, &rate), Some(2.0));
        assert_eq!(whisper_text("Bob", "Tabula Rasa", Some("5 chaos"), Some("fair")).1, "Tabula Rasa for 5 chaos · Fair price");
    }
}
