//! Map tools: a stash-search regex that hides maps with mods you can't run (and
//! keeps only ones with the quantity, pack size… you want), and the same choice as
//! a trade site search to buy more maps like that.
//!
//! The list of map mods and each mod's short search code comes from poe.re's
//! generated data (credited in the app; see core/data/poere/README.md). The app
//! downloads the current copy; the copy in core/data is the fallback.

use crate::maps::MAP_DANGER;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One map mod (some mods are several lines, joined with " / ").
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MapModEntry {
    pub id: i64,
    pub text: String,
    /// Short text that only this mod has, for the stash search.
    pub token: String,
    /// How dangerous poe.re rates it (0–1001; higher = worse).
    pub scary: u32,
    /// Only on nightmare (T17-style) maps.
    pub nightmare: bool,
    /// The trade site's stat id, to leave it out of trade searches.
    pub trade_id: Option<String>,
    /// The map check's name for it, if it's one of the known dangers.
    pub danger: Option<String>,
}

/// The bundled fallback copy.
pub fn bundled() -> (Value, Value) {
    (serde_json::from_str(include_str!("../data/poere/mapmods.json")).unwrap_or_default(),
     serde_json::from_str(include_str!("../data/poere/trade_ids.json")).unwrap_or_default())
}

/// Reads poe.re's map mod file and trade id file into one list, scariest first.
pub fn load(mapmods: &Value, trade_ids: &Value) -> Vec<MapModEntry> {
    let mut out: Vec<MapModEntry> = mapmods["tokens"].as_array().into_iter().flatten().filter_map(|t| {
        let id = t["id"].as_i64()?;
        let text = t["rawText"].as_str()?.split('|').map(str::trim).collect::<Vec<_>>().join(" / ");
        let low = text.to_lowercase();
        let danger = MAP_DANGER.iter().find(|(w, _)| w.iter().all(|x| low.contains(x))).map(|(_, n)| n.to_string());
        Some(MapModEntry {
            id, token: t["regex"].as_str()?.to_string(),
            scary: t["options"]["scary"].as_u64().unwrap_or(0) as u32,
            nightmare: t["options"]["nm"].as_bool().unwrap_or(false),
            trade_id: trade_ids[id.to_string()].as_str().map(String::from),
            danger, text,
        })
    }).collect();
    out.sort_by(|a, b| b.scary.cmp(&a.scary).then(a.text.cmp(&b.text)));
    out
}

/// What you want in a map: minimums (in %), tier and kind.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MapWants {
    pub quantity: Option<u32>,
    pub pack_size: Option<u32>,
    pub rarity: Option<u32>,
    pub more_maps: Option<u32>,
    pub more_currency: Option<u32>,
    pub more_scarabs: Option<u32>,
    pub more_cards: Option<u32>,
    /// Trade only.
    pub tier_min: Option<u32>,
    /// "any", "conqueror", "elder", "shaper", "originator" (8-mod), "blighted", "ravaged" — trade only.
    pub kind: String,
    /// Trade: allow corrupted maps.
    pub corrupted_ok: bool,
    /// Trade: lines the map must have ("Area is dominated by Terror Shrines"), and whether any one of them is enough.
    pub extras: Vec<MapExtra>,
    pub extras_any: bool,
    /// Trade: at most this many chaos, and only instant buyout.
    pub price_max: Option<u32>,
    pub instant: bool,
}

/// A line to ask for when buying maps: the trade site's stat, its words, and the least it must be.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MapExtra { pub id: String, pub text: String, pub min: Option<f64> }

/// Every line the trade site knows that a map can carry (from its stat list): what "Must have" searches through.
pub fn map_stats(v: &Value) -> Vec<MapExtra> {
    let mut out: Vec<MapExtra> = Vec::new();
    for e in v["result"].as_array().into_iter().flatten().flat_map(|g| g["entries"].as_array().into_iter().flatten()) {
        let (Some(id), Some(text), Some(kind)) = (e["id"].as_str(), e["text"].as_str(), e["type"].as_str()) else { continue };
        let t = text.replace('\n', " ").split_whitespace().collect::<Vec<_>>().join(" ");
        // Not the ones on tablets, idols, logbooks and gear ("Your Maps have…", "While a Pinnacle Atlas Boss…").
        if t.contains("our Maps") || t.contains("Pinnacle") || t.contains("Lesser ") || t.contains("Shrine Buffs on you") || e.get("option").is_some() { continue; }
        let l = t.to_lowercase();
        let map_line = match kind {
            "enchant" | "implicit" => l.starts_with("area") || l.starts_with("map ") || l.starts_with("this map") || l.contains(" in area") || l.starts_with("players in area")
                || l.starts_with("players and monsters") || l.starts_with("unique boss") || l.starts_with("unique monsters drop") || l.starts_with("strongboxes in area")
                || l.starts_with("gain a random shrine buff") || l.starts_with("rare monsters") || l.contains("monster packs") || l.contains("blight")
                || l.starts_with("natural inhabitants") || l.contains("explicit modifier magnitudes") || l.contains("additional enchantment mod")
                || l.starts_with("area's inhabitants") || l.contains("atlas passives") || l == "+#% pack size" || l == "+#% item rarity",
            "explicit" => l.starts_with("area ") || l.contains(" in area") || l.starts_with("unique boss") || l.starts_with("map bosses") || l.starts_with("rare monsters each") || l.starts_with("players "),
            "pseudo" => id.starts_with("pseudo.pseudo_map_more_") || id.starts_with("pseudo.pseudo_map_conversion_") || id.starts_with("pseudo.pseudo_map_quality_"),
            _ => false,
        };
        if map_line && !out.iter().any(|o| o.id == id) { out.push(MapExtra { id: id.into(), text: t, min: None }); }
    }
    // What's added to a map (Terror Shrines…) before its own mods.
    out.sort_by_key(|o| ["enchant.", "implicit.", "pseudo."].iter().position(|k| o.id.starts_with(k)).unwrap_or(9));
    out
}

/// The saved map tool choices.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MapTool {
    /// Mods you can't run (by id). None = take them from the map check list.
    pub bad: Option<Vec<i64>>,
    pub wants: MapWants,
}

fn range(a: u8, b: u8) -> String {
    match (a, b) { (0, 9) => r"\d".into(), _ if a == b => a.to_string(), _ => format!("[{a}-{b}]") }
}

/// Fixed-length digit strings at least `d` (e.g. "85" -> ["8[5-9]", "9\d"]).
fn at_least_fixed(d: &[u8]) -> Vec<String> {
    if d.len() == 1 { return vec![range(d[0], 9)]; }
    let rest = r"\d".repeat(d.len() - 1);
    // "80" and up = 8x or 9x = [8-9]\d.
    if d[1..].iter().all(|x| *x == 0) { return vec![format!("{}{rest}", range(d[0], 9))]; }
    let mut out: Vec<String> = at_least_fixed(&d[1..]).into_iter().map(|s| format!("{}{s}", d[0])).collect();
    if d[0] < 9 { out.push(format!("{}{rest}", range(d[0] + 1, 9))); }
    out
}

/// A regex for a whole number of at least `n` (up to 3 digits, like map percentages).
pub fn at_least(n: u32) -> String {
    if n <= 1 { return r"\d+".into(); }
    let digits: Vec<u8> = n.to_string().bytes().map(|b| b - b'0').collect();
    let mut alts = at_least_fixed(&digits);
    for len in digits.len() + 1..=3 { alts.push(format!(r"\d{{{len}}}")); }
    if alts.len() == 1 { alts.remove(0) } else { format!("({})", alts.join("|")) }
}

/// The start of each map stat line the stash search looks for (English client).
const STAT_TEXT: &[(&str, &str)] = &[
    ("quantity", "as d"), ("pack_size", "r pa"), ("rarity", "m rar"), ("more_maps", "re maps"),
    ("more_currency", "cy:"), ("more_scarabs", "e thr"), ("more_cards", "e div"),
];

/// The stash search text: hides maps with any mod you can't run, and (when set)
/// keeps only maps with at least the quantity / pack size / etc. you asked for.
pub fn regex(mods: &[MapModEntry], bad: &[i64], wants: &MapWants) -> String {
    let mut tokens: Vec<&str> = Vec::new();
    for m in mods.iter().filter(|m| bad.contains(&m.id)) {
        if !tokens.contains(&m.token.as_str()) { tokens.push(&m.token); }
    }
    let mut parts: Vec<String> = Vec::new();
    if !tokens.is_empty() { parts.push(format!("\"!{}\"", tokens.join("|"))); }
    let w = wants;
    for (key, text) in STAT_TEXT {
        let v = match *key { "quantity" => w.quantity, "pack_size" => w.pack_size, "rarity" => w.rarity, "more_maps" => w.more_maps,
            "more_currency" => w.more_currency, "more_scarabs" => w.more_scarabs, _ => w.more_cards };
        if let Some(n) = v.filter(|n| *n > 0) { parts.push(format!("\"{text}.*{}%\"", at_least(n))); }
    }
    parts.join(" ")
}

/// The trade site search for maps like that: none of the bad mods, your minimums.
pub fn trade_query(mods: &[MapModEntry], bad: &[i64], w: &MapWants) -> Value {
    let mut not: Vec<Value> = Vec::new();
    for m in mods.iter().filter(|m| bad.contains(&m.id)) {
        if let Some(id) = &m.trade_id { if !not.iter().any(|v| v["id"] == *id) { not.push(json!({ "id": id })); } }
    }
    let mut map_f = json!({ "map_tier": { "min": w.tier_min.unwrap_or(16) } });
    if let Some(n) = w.quantity { map_f["map_bonus"] = json!({ "min": n }); }
    if let Some(n) = w.pack_size { map_f["map_packsize"] = json!({ "min": n }); }
    if let Some(n) = w.rarity { map_f["map_iir"] = json!({ "min": n }); }
    let mut misc = json!({});
    if !w.corrupted_ok { misc["corrupted"] = json!({ "option": "false" }); }
    // A map's mods only show once it's identified.
    if !not.is_empty() { misc["identified"] = json!({ "option": "true" }); }
    let mut stats: Vec<Value> = Vec::new();
    if !not.is_empty() { stats.push(json!({ "type": "not", "filters": not })); }
    let mut and: Vec<Value> = Vec::new();
    for (v, id) in [(w.more_maps, "pseudo.pseudo_map_more_map_drops"), (w.more_currency, "pseudo.pseudo_map_more_currency_drops"),
        (w.more_scarabs, "pseudo.pseudo_map_more_scarab_drops"), (w.more_cards, "pseudo.pseudo_map_more_card_drops")] {
        if let Some(n) = v { and.push(json!({ "id": id, "value": { "min": n } })); }
    }
    match w.kind.as_str() {
        "originator" => and.push(json!({ "id": "implicit.stat_2696470877" })),
        "shaper" => and.push(json!({ "id": "implicit.stat_1792283443", "value": { "option": 1 } })),
        "elder" => and.push(json!({ "id": "implicit.stat_1792283443", "value": { "option": 2 } })),
        "blighted" => map_f["map_blighted"] = json!({ "option": "true" }),
        "ravaged" => map_f["map_uberblighted"] = json!({ "option": "true" }),
        "conqueror" => stats.push(json!({ "type": "count", "value": { "min": 1 },
            "filters": (1..=4).map(|o| json!({ "id": "implicit.stat_2563183002", "value": { "option": o } })).collect::<Vec<_>>() })),
        _ => {}
    }
    // Lines the map must have. An id like "implicit.stat_1792283443|2" is a stat and its option.
    let extras: Vec<Value> = w.extras.iter().filter(|x| !x.id.is_empty()).map(|x| {
        let mut f = match x.id.split_once('|') {
            Some((id, o)) => json!({ "id": id, "value": { "option": o.parse::<u32>().unwrap_or(0) } }),
            None => json!({ "id": x.id }),
        };
        if let Some(n) = x.min.filter(|_| !x.id.contains('|')) { f["value"] = json!({ "min": n }); }
        f
    }).collect();
    if w.extras_any && extras.len() > 1 { stats.push(json!({ "type": "count", "value": { "min": 1 }, "filters": extras })); }
    else { and.extend(extras); }
    if !and.is_empty() { stats.push(json!({ "type": "and", "filters": and })); }
    if stats.is_empty() { stats.push(json!({ "type": "and", "filters": [] })); }
    let mut filters = json!({
        "type_filters": { "filters": { "category": { "option": "map" }, "rarity": { "option": "nonunique" } } },
        "map_filters": { "filters": map_f },
    });
    if let Some(n) = w.price_max.filter(|n| *n > 0) { filters["trade_filters"] = json!({ "filters": { "price": { "max": n } } }); }
    if misc.as_object().map(|o| !o.is_empty()).unwrap_or(false) { filters["misc_filters"] = json!({ "filters": misc }); }
    let status = if w.instant { "securable" } else { "available" };
    json!({ "query": { "status": { "option": status }, "filters": filters, "stats": stats }, "sort": { "price": "asc" } })
}

/// The mods to tick by default: those matching your map check list (None = all known dangers).
pub fn default_bad(mods: &[MapModEntry], avoid: Option<&[String]>) -> Vec<i64> {
    mods.iter().filter(|m| m.danger.as_ref().map(|d| avoid.map(|a| a.contains(d)).unwrap_or(true)).unwrap_or(false)).map(|m| m.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(at_least(80), r"([8-9]\d|\d{3})");
        assert_eq!(at_least(85), r"(8[5-9]|9\d|\d{3})");
        assert_eq!(at_least(100), r"[1-9]\d\d");
        assert_eq!(at_least(110), r"(1[1-9]\d|[2-9]\d\d)");
        assert_eq!(at_least(5), r"([5-9]|\d{2}|\d{3})");
        assert_eq!(at_least(9), r"(9|\d{2}|\d{3})");
        // Check against every number 0..999 with a tiny matcher.
        for n in [7u32, 23, 50, 80, 99, 100, 120, 345, 500] {
            let alts: Vec<String> = {
                let r = at_least(n);
                let r = r.strip_prefix('(').and_then(|x| x.strip_suffix(')')).map(String::from).unwrap_or(r);
                r.split('|').map(String::from).collect()
            };
            for v in 0..1000u32 {
                let s = v.to_string();
                let hit = alts.iter().any(|a| full_match(a, &s));
                assert_eq!(hit, v >= n, "n={n} v={v}");
            }
        }
    }

    /// Matches our tiny regex subset (\d, [a-b], digits, \d{k}) against a whole string.
    fn full_match(pat: &str, s: &str) -> bool {
        let mut classes: Vec<(u8, u8)> = Vec::new();
        let p = pat.as_bytes();
        let mut i = 0;
        while i < p.len() {
            if p[i] == b'\\' {
                i += 2;
                if i < p.len() && p[i] == b'{' { let end = pat[i..].find('}').unwrap() + i; let k: usize = pat[i + 1..end].parse().unwrap();
                    for _ in 0..k { classes.push((b'0', b'9')); } i = end + 1; } else { classes.push((b'0', b'9')); }
            } else if p[i] == b'[' { classes.push((p[i + 1], p[i + 3])); i += 5; }
            else { classes.push((p[i], p[i])); i += 1; }
        }
        classes.len() == s.len() && s.bytes().zip(classes).all(|(c, (a, b))| c >= a && c <= b)
    }

    #[test]
    fn regex_and_trade() {
        let (m, t) = bundled();
        let mods = load(&m, &t);
        assert!(mods.len() > 50, "bundled data loads");
        let regen = mods.iter().find(|x| x.text.contains("cannot Regenerate")).unwrap();
        assert_eq!(regen.danger.as_deref(), Some("No regen"));
        assert!(regen.trade_id.is_some());
        let bad = default_bad(&mods, Some(&["No regen".to_string()]));
        assert_eq!(bad, vec![regen.id]);
        let w = MapWants { quantity: Some(80), pack_size: Some(30), ..Default::default() };
        let r = regex(&mods, &bad, &w);
        assert_eq!(r, format!(r#""!{}" "as d.*([8-9]\d|\d{{3}})%" "r pa.*([3-9]\d|\d{{3}})%""#, regen.token));
        let q = trade_query(&mods, &bad, &MapWants { kind: "conqueror".into(), more_maps: Some(50), ..Default::default() });
        assert_eq!(q["query"]["stats"][0]["type"], "not");
        assert_eq!(q["query"]["stats"][0]["filters"][0]["id"], regen.trade_id.clone().unwrap().as_str());
        assert_eq!(q["query"]["stats"][1]["type"], "count");
        assert_eq!(q["query"]["stats"][2]["filters"][0]["id"], "pseudo.pseudo_map_more_map_drops");
        assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["map_tier"]["min"], 16);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["identified"]["option"], "true");
    }

    #[test]
    fn must_have_lines() {
        let x = |id: &str, min| MapExtra { id: id.into(), text: String::new(), min };
        // Terror Shrines (25 Nov), and at least 40% delirious.
        let mut w = MapWants { extras: vec![x("enchant.stat_667284445", None), x("enchant.stat_1715784068", Some(40.0))], price_max: Some(120), instant: true, ..Default::default() };
        let q = trade_query(&[], &[], &w);
        assert_eq!(q["query"]["stats"][0], json!({ "type": "and", "filters": [{ "id": "enchant.stat_667284445" }, { "id": "enchant.stat_1715784068", "value": { "min": 40.0 } }] }));
        assert_eq!(q["query"]["filters"]["trade_filters"]["filters"]["price"]["max"], 120);
        assert_eq!(q["query"]["status"]["option"], "securable");
        w.extras_any = true;
        assert_eq!(trade_query(&[], &[], &w)["query"]["stats"][0]["type"], "count");
        let w = MapWants { kind: "blighted".into(), extras: vec![x("implicit.stat_3624393862|4", Some(1.0))], ..Default::default() };
        let q = trade_query(&[], &[], &w);
        assert_eq!(q["query"]["stats"][0]["filters"][0], json!({ "id": "implicit.stat_3624393862", "value": { "option": 4 } }));
        assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["map_blighted"]["option"], "true");
        // Choices saved before these existed still load.
        assert!(serde_json::from_str::<MapWants>(r#"{"quantity":80,"kind":"","corrupted_ok":false}"#).unwrap().extras.is_empty());
        let list = json!({ "result": [{ "entries": [
            { "id": "enchant.stat_667284445", "text": "Area is dominated by Terror Shrines", "type": "enchant" },
            { "id": "explicit.stat_1", "text": "Your Maps have +#% chance to contain an Abyss", "type": "explicit" },
            { "id": "implicit.stat_2", "text": "While a Pinnacle Atlas Boss is in your Presence, #% increased Damage in Area", "type": "implicit" },
            { "id": "explicit.stat_3", "text": "+# to maximum Life", "type": "explicit" }] }] });
        assert_eq!(map_stats(&list).iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["enchant.stat_667284445"]);
    }
}
