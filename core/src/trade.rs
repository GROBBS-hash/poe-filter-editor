//! Price checking on the official trade site, like Awakened PoE Trade: reads the
//! item text the game copies (Ctrl+Alt+C), works out which stats matter, builds
//! the trade search, and sums up what comes back. No network here: the app does
//! the requests (and keeps to the site's rate limits).

use crate::item::{parse_item, Item};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

/// One modifier line on the item.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModLine {
    /// As shown, without the "(implicit)" tag or roll ranges.
    pub text: String,
    /// "explicit", "implicit", "crafted", "enchant", "fractured".
    pub kind: String,
    /// From the advanced copy: tier 1 is the best roll of that mod.
    pub tier: Option<u32>,
    pub values: Vec<f64>,
    /// Where the roll sits in its range, 0 (lowest) to 1 (highest), from the advanced copy's "(30-39)".
    pub roll: Option<f64>,
    /// Its range as shown, "25–29" (several numbers: "8–12 / 18–25").
    pub range: Option<String>,
    /// "prefix" or "suffix", from the advanced copy's "{ Prefix Modifier … }" heading.
    pub affix: Option<String>,
    /// How wide its roll range is (averaged over its numbers that vary), from the advanced copy: "(80-100)" -> 20.
    #[serde(skip)]
    pub width: Option<f64>,
    /// Where it came from when special (eldritch, essence, delve, influence…): see `StatFilter::source`.
    pub source: Option<String>,
}

/// A mod's special source from its advanced-copy heading, e.g. { Prefix Modifier "Subterranean" (Tier: 1) } -> delve.
/// The mod names are the game's (the same lists Awakened PoE Trade uses).
fn mod_source(head: &str) -> Option<String> {
    let s = |x: &str| Some(x.to_string());
    if head.contains("Eater of Worlds Implicit") { return s("eater"); }
    if head.contains("Searing Exarch Implicit") { return s("exarch"); }
    // The game writes "{ Corruption Implicit Modifier — … }" (24 Nov: his corrupted Darkray Vectors' "+2 to Level of
    // Socketed Curse Gems" was taken for a fixed base mod and hidden, as this only looked for "Corrupted Implicit").
    if head.contains("Corrupted Implicit") || head.contains("Corruption Implicit") { return s("corrupted"); }
    if head.contains("Rare-only") { return s("rareonly"); }
    if head.contains("Synthesised") { return s("synthesised"); }
    let name = head.split('"').nth(1)?;
    const LISTS: [(&str, &[&str]); 11] = [
        ("shaper", &["of Shaping", "The Shaper's", "Elevated Shaper's", "of Elevated Shaping"]),
        ("elder", &["of the Elder", "The Elder's", "Elevated Elder's", "of the Elevated Elder"]),
        ("crusader", &["Crusader's", "of the Crusade", "Elevated Crusader's", "of the Elevated Crusade"]),
        ("hunter", &["Hunter's", "of the Hunt", "Elevated Hunter's", "of the Elevated Hunt"]),
        ("redeemer", &["of Redemption", "Redeemer's", "Elevated Redeemer's", "of Elevated Redemption"]),
        ("warlord", &["Warlord's", "of the Conquest", "Elevated Warlord's", "of the Elevated Conquest"]),
        ("delve", &["Subterranean", "of the Underground"]),
        ("veiled", &["Chosen", "of the Order", "Leo's", "Catarina's", "Elreon's", "Vorici's", "Haku's", "Tora's", "Vagan's", "Brinerot",
            "of Janus", "It's", "of Hillock", "Gravicius'", "Jorgin's", "Korell's", "Rin's", "of Cameria", "of Aisling", "of Riker",
            "of Catarina", "Zorath's", "of Zorath"]),
        ("incursion", &["Guatelitzi's", "Xopec's", "Topotante's", "Tacati's", "Matatl's", "of Matatl", "Citaqualotl's", "of Citaqualotl",
            "of Tacati", "of Guatelitzi", "of Puhuarte"]),
        ("essence", &["of the Essence", "Essences"]),
        ("infamous", &["of Infamy", "Infamous"]),
    ];
    LISTS.iter().find(|(_, names)| names.contains(&name)).map(|(k, _)| k.to_string())
}

/// One stat to search on, with a tick box and a minimum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StatFilter {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub min: Option<f64>,
    pub enabled: bool,
    /// "pseudo", "explicit", "implicit"...
    pub kind: String,
    pub tier: Option<u32>,
    /// Rarely affects the price (accuracy, thorns, light radius…): never ticked for you.
    #[serde(default)]
    pub junk: bool,
    /// Where the roll sits in its range (0 lowest, 1 highest), when the copy showed one.
    #[serde(default)]
    pub roll: Option<f64>,
    #[serde(default)]
    pub range: Option<String>,
    /// "prefix" or "suffix" (shown as P / S in the window).
    #[serde(default)]
    pub affix: Option<String>,
    /// An upper limit too (a cluster jewel's passive count is searched exactly: more passives is a worse jewel).
    #[serde(default)]
    pub max: Option<f64>,
    /// Where the mod came from when it needs something special, from the advanced copy's heading (like Awakened's
    /// tags): "exarch", "eater", "essence", "delve", "veiled", "incursion", "shaper", "elder", "crusader", "hunter",
    /// "redeemer", "warlord", "infamous", "rareonly", "synthesised", "corrupted".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Mods that almost never change what an item sells for.
const JUNK: &[&str] = &["Accuracy Rating", "Light Radius", "Stun and Block Recovery", "Reflects # Physical Damage",
    "reduced Attribute Requirements", "Stun Duration", "Stun Threshold", "gained for each Enemy hit", "gained on Kill",
    "increased Rarity of Items found", "Knockback", "reduced Enemy Stun", "increased Flask Charges gained",
    "Life gained when you Block", "Mana gained when you Block", "Thorns"];

pub fn is_junk(template_text: &str) -> bool {
    JUNK.iter().any(|j| template_text.contains(j))
}

/// Everything the price check window needs to start a search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSetup {
    pub item: Item,
    pub filters: Vec<StatFilter>,
    /// Same base only (otherwise any base of that kind, e.g. any helmet).
    pub base_only: bool,
    pub ilvl_min: Option<u32>,
    /// Top of the item-level band (see `ilvl_band`); None = no upper limit.
    #[serde(default)]
    pub ilvl_max: Option<u32>,
    pub links_min: Option<u32>,
    pub gem_level: Option<u32>,
    /// Mods the trade site's list doesn't know (shown, not searched).
    pub unknown: Vec<String>,
    /// Their kinds ("mutated" = Foulborn…), same order, for colouring.
    #[serde(default)]
    pub unknown_kinds: Vec<String>,
    /// Trade site category for the item class ("armour.helmet").
    pub category: Option<String>,
    /// The copied item text.
    #[serde(default)]
    pub raw: String,
    /// Extra "yes" switches for the search's misc filters ("shaper_item", "synthesised_item"…).
    #[serde(default)]
    pub misc: Vec<String>,
    /// The base as the trade site knows it ("Eternal Burgonet" for "Fecund Eternal Burgonet of the Genius").
    #[serde(default)]
    pub base: String,
    /// How many prefixes and suffixes this item can hold (3 / 3 on most gear, 2 / 2 on jewels, Cogwork Ring 2 / 4).
    #[serde(default)]
    pub affix_max: Option<(u32, u32)>,
    /// Awakened's "base item" search, when this looks like a crafting base rather than a finished item.
    #[serde(default)]
    pub base_item: Option<BaseItem>,
    /// Search corrupted (Some(true)) or not corrupted (Some(false)) only; None = either.
    #[serde(default)]
    pub corrupted: Option<bool>,
    /// Instant buyout only (Some(true)) or any listing (Some(false)) for this search; None = your Settings choice.
    #[serde(default)]
    pub buyout: Option<bool>,
}

/// How to price an item as a crafting base (Awakened PoE Trade's "base item" preset): same base, at least this
/// item level, its influences, and only the mods that make the base (fractured, enchants, implicits, a magic
/// item's top-tier mods).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaseItem {
    /// Why it's a base, in words ("Fractured", "Shaper", "Item level 84"…).
    pub why: Vec<String>,
    /// Filters to tick (by id); the rest are unticked.
    pub ids: Vec<String>,
    pub ilvl_min: Option<u32>,
    pub misc: Vec<String>,
}

/// The trade site's stat list (api/trade/data/stats), indexed by text.
#[derive(Debug, Default)]
pub struct StatIndex {
    by_text: HashMap<(String, String), String>,
    /// Every id for a text, where the site lists the same words more than once (Mercenary supports: two
    /// "Gilded Extra Targets (Tier 3)").
    all: HashMap<(String, String), Vec<String>>,
    /// The "Mercenary" stats' words as the site writes them (its skills, and supports like "Pierce (Tier 2)").
    pub merc: Vec<String>,
}

impl StatIndex {
    pub fn from_json(v: &Value) -> StatIndex {
        let mut by_text = HashMap::new();
        let mut all: HashMap<(String, String), Vec<String>> = HashMap::new();
        let mut merc = Vec::new();
        for group in v["result"].as_array().into_iter().flatten() {
            for e in group["entries"].as_array().into_iter().flatten() {
                let (Some(id), Some(text), Some(kind)) = (e["id"].as_str(), e["text"].as_str(), e["type"].as_str()) else { continue };
                // Stats with a fixed option list ("Allocates #") are searched differently: skip.
                // (Temple rooms are open / closed: searched as open, see temple_rows.)
                if e.get("option").is_some() && !id.starts_with("pseudo.pseudo_temple_") { continue; }
                let key = (kind.to_string(), text.replace('\n', " ").to_lowercase());
                if kind == "mercenary" && !all.contains_key(&key) { merc.push(text.to_string()); }
                all.entry(key.clone()).or_default().push(id.to_string());
                by_text.entry(key).or_insert_with(|| id.to_string());
            }
        }
        StatIndex { by_text, all, merc }
    }
    pub fn len(&self) -> usize { self.by_text.len() }
    /// Every trade id listed with exactly these words under this kind.
    pub fn ids(&self, kind: &str, text: &str) -> Vec<String> { self.all.get(&(kind.to_string(), text.to_lowercase())).cloned().unwrap_or_default() }
    /// The trade id of a total ("pseudo") stat by its text, e.g. "+# total maximum Life".
    pub fn pseudo_id(&self, text: &str) -> Option<String> { self.by_text.get(&("pseudo".to_string(), text.to_lowercase())).cloned() }
    pub fn is_empty(&self) -> bool { self.by_text.is_empty() }

    /// A timeless jewel's seed stat and its siblings: the same words under every conqueror of that jewel
    /// ("Denoted service of # dekhara in the akhara of Balbala" / "… of Nasima" / "… of Asenath" / "… of Deshret"),
    /// as (id, conqueror) pairs, sorted by name.
    pub fn timeless_family(&self, id: &str) -> Vec<(String, String)> {
        let Some(((_, text), _)) = self.by_text.iter().find(|(_, v)| v.as_str() == id) else { return vec![] };
        let Some((stem, _)) = text.rsplit_once(' ') else { return vec![] };
        let mut out: Vec<(String, String)> = self.by_text.iter()
            .filter(|((_, t), v)| v.starts_with("explicit.pseudo_timeless_jewel_") && t.rsplit_once(' ').is_some_and(|(s, _)| s == stem))
            .map(|((_, t), v)| { let who = t.rsplit(' ').next().unwrap_or(""); (v.clone(), who[..1].to_uppercase() + &who[1..]) })
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out.dedup();
        out
    }

    /// The trade id for a mod line, and the value to search with. Tries the
    /// mod's own kind first, then explicit; "reduced" is searched as negative "increased".
    pub fn find(&self, m: &ModLine) -> Option<(String, f64)> {
        let (t, vals) = template(&m.text);
        let value = if vals.len() == 2 && t.to_lowercase().contains("# to #") { (vals[0] + vals[1]) / 2.0 } else { vals.first().copied().unwrap_or(0.0) };
        let kinds = [m.kind.as_str(), "explicit"];
        // Some are listed word for word, numbers and all ("1 Added Passive Skill is Overshock"): no value to search.
        for k in kinds {
            if let Some(id) = self.by_text.get(&(k.to_string(), m.text.to_lowercase())) { return Some((id.clone(), f64::NAN)); }
        }
        for k in kinds {
            if let Some(id) = self.by_text.get(&(k.to_string(), t.to_lowercase())) { return Some((id.clone(), value)); }
            // The site writes a leading "+" for flat mods; the item sometimes doesn't, and vice versa.
            let alt = if let Some(x) = t.strip_prefix('+') { x.to_string() } else { format!("+{t}") };
            if let Some(id) = self.by_text.get(&(k.to_string(), alt.to_lowercase())) { return Some((id.clone(), value)); }
            // Armour and weapon mods that only affect the item itself are listed as "… (Local)".
            if let Some(id) = self.by_text.get(&(k.to_string(), format!("{} (local)", t.to_lowercase()))) { return Some((id.clone(), value)); }
            if t.contains("reduced") {
                let inc = t.replace("reduced", "increased");
                if let Some(id) = self.by_text.get(&(k.to_string(), inc.to_lowercase())) { return Some((id.clone(), -value)); }
            }
            // "applying -12% to Lightning Resistance": the site writes "applying #%" and searches -12.
            if t.contains("-#") {
                if let Some(id) = self.by_text.get(&(k.to_string(), t.replacen("-#", "#", 1).to_lowercase())) { return Some((id.clone(), -value)); }
            }
        }
        // Small wording differences: "-3 to Total Mana Cost" listed as "+# to Total Mana Cost", "Has 1 Abyssal Socket" as
        // "Has # Abyssal Sockets", "Grants Level 20 Aspect of Solaris Skill" as "Grants Level # Aspect of Solaris".
        let plural = t.split_once("# ").map(|(a, b)| { let (w, r) = b.split_once(' ').unwrap_or((b, "")); format!("{a}# {w}s {r}").trim().to_string() });
        let variants = [(t.contains("-#").then(|| t.replacen("-#", "+#", 1)), -value), (plural, value),
            (t.strip_suffix(" Skill").map(String::from), value), (Some(format!("{t}s")), value)];
        for (v, val) in variants {
            let Some(v) = v else { continue };
            for k in kinds {
                if let Some(id) = self.by_text.get(&(k.to_string(), v.to_lowercase())) { return Some((id.clone(), val)); }
            }
        }
        // "Flasks gain a Charge every # seconds" is listed as "Flasks gain # Charges every 3 seconds" (one, as a number).
        if let Some(i) = m.text.rfind(" a ") {
            let rest = &m.text[i + 3..];
            let (word, after) = rest.split_once(' ').unwrap_or((rest, ""));
            let plural = format!("{} # {word}s {after}", &m.text[..i]);
            for k in kinds {
                if let Some(id) = self.by_text.get(&(k.to_string(), plural.trim().to_lowercase())) { return Some((id.clone(), 1.0)); }
            }
        }
        // Some numbers are part of the stat's text ("#% chance to Unnerve Enemies for 4 seconds on Hit"):
        // try leaving each number as it is, searching on the others.
        let nums = vals.len().min(4);
        if nums >= 2 {
            for keep in 1..(1u32 << nums) - 1 {
                let (mut text, mut n, mut first) = (String::new(), 0usize, None);
                for (i, part) in t.split('#').enumerate() {
                    if i > 0 {
                        let v = vals.get(n).copied().unwrap_or(0.0);
                        if n < nums && keep & (1 << n) != 0 { text.push_str(&fmt_num(v)); } else { text.push('#'); first.get_or_insert(v); }
                        n += 1;
                    }
                    text.push_str(part);
                }
                for k in kinds {
                    if let Some(id) = self.by_text.get(&(k.to_string(), text.to_lowercase())) { return Some((id.clone(), first.unwrap_or(value))); }
                }
            }
        }
        None
    }
}

/// 4.0 -> "4", 0.15 -> "0.15" (as the number reads on the item).
fn fmt_num(v: f64) -> String { if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") } }

/// "+52 to maximum Life" -> ("+# to maximum Life", [52]).
pub fn template(line: &str) -> (String, Vec<f64>) {
    let mut out = String::new();
    let mut vals: Vec<f64> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())) { i += 1; }
            let s: String = chars[start..i].iter().collect();
            if let Ok(v) = s.parse() { vals.push(v); }
            out.push('#');
            continue;
        }
        out.push(c);
        i += 1;
    }
    (out, vals)
}

/// The "(25-29)" ranges on an advanced-copy line, in order.
fn ranges(line: &str) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find('(') {
        let Some(j) = rest[i..].find(')').map(|j| i + j) else { break };
        let inner = &rest[i + 1..j];
        // "-" splits the two ends; a leading "-" belongs to the number ("(-15--5)").
        if let Some((k, _)) = inner.char_indices().skip(1).find(|(_, c)| *c == '-') {
            if let (Ok(a), Ok(b)) = (inner[..k].trim().parse::<f64>(), inner[k + 1..].trim().parse::<f64>()) { out.push((a, b)); }
        }
        rest = &rest[j + 1..];
    }
    out
}

/// How high the line rolled, 0 (lowest) to 1 (highest), averaged over its numbers.
/// None when the copy shows no ranges or every number is fixed.
fn roll_of(line: &str, values: &[f64]) -> Option<f64> {
    let r = ranges(line);
    if r.is_empty() || r.len() != values.len() { return None; }
    let spots: Vec<f64> = r.iter().zip(values).filter(|((a, b), _)| a != b)
        // By size: "29(30-25)% reduced …" is written high to low, and the bigger number is the better roll.
        .map(|((a, b), v)| { let (lo, hi) = (a.abs().min(b.abs()), a.abs().max(b.abs())); ((v.abs() - lo) / (hi - lo)).clamp(0.0, 1.0) }).collect();
    if spots.is_empty() { return None; }
    Some(spots.iter().sum::<f64>() / spots.len() as f64)
}

/// How wide the line's roll range is, averaged over its numbers that vary ("(80-100)" -> 20). None when fixed or unknown.
fn width_of(line: &str) -> Option<f64> {
    let w: Vec<f64> = ranges(line).iter().filter(|(a, b)| a != b).map(|(a, b)| (a.abs() - b.abs()).abs()).collect();
    (!w.is_empty()).then(|| w.iter().sum::<f64>() / w.len() as f64)
}

/// The ranges as shown in the window: "25–29", or "8–12 / 18–25" for several numbers.
/// None when there are none or every number is fixed.
fn range_text(line: &str) -> Option<String> {
    let r = ranges(line);
    if r.is_empty() || r.iter().all(|(a, b)| a == b) { return None; }
    Some(r.iter().map(|(a, b)| { let (lo, hi) = if a.abs() <= b.abs() { (a, b) } else { (b, a) }; format!("{lo}–{hi}") }).collect::<Vec<_>>().join(" / "))
}

/// Every number on a line times `f`, rounded down like the game does ("+46(43-50)" x1.25 -> "+57(53-62)").
/// For mods boosted by the item's own "25% increased Suffix Modifier magnitudes" (Cogwork Ring): the advanced copy
/// shows the numbers before the boost, the game and the trade site show them after.
fn boost_numbers(line: &str, f: f64) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())) { i += 1; }
            let s: String = chars[start..i].iter().collect();
            match s.parse::<f64>() {
                Ok(v) if s.contains('.') => { let b = (v * f * 10.0 + 1e-9).floor() / 10.0; out.push_str(&format!("{b:.1}")); }
                Ok(v) => out.push_str(&((v * f + 1e-9).floor() as i64).to_string()),
                Err(_) => out.push_str(&s),
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Removes roll ranges "(25-29)" and tags like "(augmented)" from an advanced-copy line.
fn clean(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let mut buf = String::new();
    for c in line.chars() {
        match c {
            '(' => { depth += 1; buf.clear(); }
            ')' if depth > 0 => {
                depth -= 1;
                // Keep words in brackets that are part of the mod; drop ranges and tags.
                let keep = !buf.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.' || c == ' ' || c == '+')
                    && !["implicit", "crafted", "enchant", "fractured", "augmented", "unmet", "scourge", "crucible"].contains(&buf.trim());
                if keep { out.push('('); out.push_str(&buf); out.push(')'); }
            }
            _ if depth > 0 => buf.push(c),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The mod lines of a copied item (plain Ctrl+C or advanced Ctrl+Alt+C).
pub fn mods(text: &str) -> Vec<ModLine> {
    let text = text.replace('\r', "");
    let sections: Vec<Vec<&str>> = text.split("--------")
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect()).collect();
    // Mods come after the "Item Level" section.
    let Some(start) = sections.iter().position(|s| s.iter().any(|l| l.starts_with("Item Level:"))) else { return vec![] };
    let mut out = Vec::new();
    // A Wickerman body part's own mod is an implicit on the trade site, but the copy doesn't mark it.
    let body_part = sections[0].contains(&"Item Class: Viscera");
    for sec in &sections[start + 1..] {
        if sec.iter().any(|l| l.starts_with("Note:") || *l == "Corrupted" || *l == "Twice Corrupted" || *l == "Unidentified" || *l == "Mirrored"
            || l.starts_with("Place into") || l.starts_with("Right click") || l.starts_with("Travel to")) {
            continue;
        }
        let mut header_kind: Option<String> = None;
        let mut header_tier: Option<u32> = None;
        let mut header_affix: Option<String> = None;
        // "{ Suffix Modifier … — 25% Increased }": the item boosts this mod (Cogwork Ring); its numbers need the boost.
        let mut header_boost: Option<f64> = None;
        let mut header_source: Option<String> = None;
        // A mod written over two lines: the second starts in lower case ("…in Radius" / "also grant…").
        let mut joined: Vec<String> = Vec::new();
        for l in sec {
            let starts_low = l.chars().next().map(|c| c.is_lowercase()).unwrap_or(false);
            match joined.last_mut() {
                Some(prev) if starts_low && !prev.starts_with('{') && !prev.starts_with('(') => { prev.push(' '); prev.push_str(l); }
                _ => joined.push(l.to_string()),
            }
        }
        for l in &joined {
            // The advanced copy marks fixed numbers: "Has 1 Abyssal Socket — Unscalable Value".
            // ...and puts an invisible zero-width space before granted skills ("Grants Level 20 <here>Aspect of the Spider").
            let l = l.replace(" — Unscalable Value", "").replace('\u{200b}', "");
            let l = l.as_str();
            if l.starts_with('{') {
                // { Prefix Modifier "Hale" (Tier: 7) — Life }
                let low = l.to_lowercase();
                header_kind = Some(if low.contains("implicit") { "implicit" } else if low.contains("crafted") { "crafted" }
                    else if low.contains("enchant") { "enchant" } else if low.contains("fractured") { "fractured" }
                    else if low.contains("foulborn") || low.contains("mutated") { "mutated" } else { "explicit" }.to_string());
                header_source = mod_source(l);
                header_affix = if low.contains("prefix") { Some("prefix".into()) } else if low.contains("suffix") { Some("suffix".into()) } else { None };
                header_boost = low.rfind("% increased").and_then(|i| {
                    let digits: String = low[..i].chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>().into_iter().rev().collect();
                    digits.parse::<f64>().ok()
                }).filter(|p| *p > 0.0).map(|p| 1.0 + p / 100.0);
                header_tier = low.find("tier:").or_else(|| low.find("rank:")).and_then(|i| {
                    low[i + 5..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
                });
                continue;
            }
            if l.starts_with('(') { continue; } // reminder text
            let boosted = header_boost.map(|f| boost_numbers(l, f));
            let l = boosted.as_deref().unwrap_or(l);
            let kind = if l.ends_with("(implicit)") { "implicit".to_string() } else if l.ends_with("(crafted)") { "crafted".into() }
                else if l.ends_with("(enchant)") { "enchant".into() } else if l.ends_with("(fractured)") { "fractured".into() }
                else if l.ends_with("(mutated)") { "mutated".into() }
                else { header_kind.clone().unwrap_or_else(|| "explicit".into()) };
            let kind = if body_part && kind == "explicit" { "implicit".to_string() } else { kind };
            let t = clean(l);
            // Under a "{ … Modifier }" heading every line is a mod, numbers or not ("…is Lucky while on Low Life").
            // ...and so is a line the copy marks itself (26 Nov: "Area is dominated by Terror Shrines (enchant)" on his
            // Originator map was dropped, so it couldn't be searched).
            let marked = ["(implicit)", "(crafted)", "(enchant)", "(fractured)", "(mutated)"].iter().any(|s| l.ends_with(s));
            let numberless_ok = header_kind.is_some() || marked || t.contains("Cannot") || t.contains("Has ") || t.starts_with("Allocates ");
            if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) && !numberless_ok { continue; }
            let (_, values) = template(&t);
            let (roll, range) = (roll_of(l, &values), range_text(l));
            out.push(ModLine { text: t, kind, tier: header_tier, values, roll, range, affix: header_affix.clone(), width: width_of(l), source: header_source.clone() });
        }
    }
    out
}

/// Trade site category for an item class.
pub fn category(class: &str) -> Option<&'static str> {
    Some(match class {
        "Helmets" => "armour.helmet", "Body Armours" => "armour.chest", "Gloves" => "armour.gloves", "Boots" => "armour.boots",
        "Shields" => "armour.shield", "Quivers" => "armour.quiver",
        "Rings" => "accessory.ring", "Amulets" => "accessory.amulet", "Belts" => "accessory.belt",
        "Wands" => "weapon.wand", "Sceptres" => "weapon.sceptre", "Staves" => "weapon.staff", "Warstaves" => "weapon.warstaff",
        "Daggers" => "weapon.dagger", "Rune Daggers" => "weapon.runedagger", "Claws" => "weapon.claw", "Bows" => "weapon.bow",
        "One Hand Swords" | "Thrusting One Hand Swords" => "weapon.onesword", "Two Hand Swords" => "weapon.twosword",
        "One Hand Axes" => "weapon.oneaxe", "Two Hand Axes" => "weapon.twoaxe",
        "One Hand Maces" => "weapon.onemace", "Two Hand Maces" => "weapon.twomace",
        "Jewels" => "jewel.base", "Abyss Jewels" => "jewel.abyss", "Flasks" | "Life Flasks" | "Mana Flasks" | "Hybrid Flasks" | "Utility Flasks" => "flask",
        "Tinctures" => "tincture", "Trinkets" => "accessory.trinket",
        "Heist Gear" => "heistequipment.heistweapon", "Heist Tools" => "heistequipment.heisttool",
        "Heist Cloaks" => "heistequipment.heistutility", "Heist Brooches" => "heistequipment.heistreward",
        "Contracts" => "heistmission.contract", "Blueprints" => "heistmission.blueprint",
        _ => return None,
    })
}

fn is_flask(it: &Item) -> bool { it.item_class.contains("Flask") || it.item_class == "Tinctures" }

/// Whether a mod can be the item's own ("Local") stat on this kind of item: a weapon's damage, speed and crit, an
/// armour piece's defences. Energy shield on a staff (Xirgil's Crank, 21 Nov) is the global stat, not armour's local one.
fn local_fits(category: Option<&str>, text: &str) -> bool {
    let t = text.to_lowercase();
    let defence = ["armour", "evasion", "energy shield", "ward"].iter().any(|w| t.contains(w));
    let weapon = ["damage", "attack speed", "attacks per second", "critical strike chance", "accuracy"].iter().any(|w| t.contains(w));
    match category {
        Some(c) if c.starts_with("weapon.") => !defence,
        Some(c) if c.starts_with("armour.") => !weapon,
        _ => true,
    }
}

/// Mods that usually set an item's price when tiers aren't known.
const VALUABLE: &[&str] = &["to Level of", "Movement Speed", "Spell Damage", "Critical Strike Multiplier", "increased Physical Damage",
    "Attack Speed", "Cast Speed", "maximum Energy Shield", "Suppress", "Damage over Time Multiplier", "to Level of all", "Minion",
    "Adds # to # Physical", "Life Regeneration", "Mana Regeneration", "Elemental Damage with Attack"];

/// The search minimum starts at the item's own roll (you can lower it in the window).
fn min_for(v: f64) -> f64 {
    v
}

/// Reads a copied item and picks the search: stats that matter ticked, others there to tick.
pub fn setup(text: &str, index: &StatIndex) -> Option<TradeSetup> {
    let item = parse_item(text)?;
    let lines = mods(text);
    let mut filters = Vec::new();
    let mut unknown = Vec::new();
    let mut unknown_kinds = Vec::new();
    let unique = item.rarity == "Unique";
    let foulborn = item.name.as_deref().map(|n| n.starts_with("Foulborn ")).unwrap_or(false);
    // Weapon DPS / armour at 20% quality, then totals (life, resistances…), each replacing the mods it counts.
    let (props_f, prop_hide) = crate::trade_rules::property_filters(text, &item, &lines);
    let prop_hide = if unique { vec![] } else { prop_hide };
    let (totals_f, total_hide) = crate::trade_rules::totals(&lines, &item, index, &prop_hide);
    // Fractured mods always keep their own row (for "fractured only" searches on crafting bases), even when a total
    // or the defence at 20% quality counts them too; those rows start unticked so nothing is counted twice.
    let counted: Vec<usize> = prop_hide.iter().chain(total_hide.iter()).copied().collect();
    let hidden: Vec<usize> = counted.iter().copied().filter(|i| lines[*i].kind != "fractured").collect();
    filters.extend(totals_f);
    filters.extend(props_f);
    let has_tiers = lines.iter().any(|m| m.tier.is_some());
    let flask = is_flask(&item);
    let cluster = item.base_type.ends_with("Cluster Jewel");
    let local_first = category(&item.item_class).is_some_and(|c| c.starts_with("armour.") || c.starts_with("weapon."));
    let mut skip_to = 0;
    for (i, m) in lines.iter().enumerate() {
        if i < skip_to || hidden.contains(&i) { continue; }
        // A mod written over two or three lines ("Enemies near corpses … are Blinded" / "Enemies Killed near corpses …
        // explode, dealing" / "3% of their Life as Physical Damage") is one stat on the site, its lines joined.
        let joined = [3usize, 2].into_iter().find_map(|n| {
            let part = lines.get(i..i + n)?;
            if part.iter().enumerate().any(|(k, l)| l.kind != m.kind || hidden.contains(&(i + k))) { return None; }
            let j = ModLine { text: part.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join(" "), ..m.clone() };
            index.find(&j).map(|_| (j, n))
        });
        let merged;
        let m = match joined { Some((j, n)) => { skip_to = i + n; merged = j; &merged } None => m };
        // Foulborn uniques' special mods are on the site's "mutated" list.
        // Armour and weapons: "+# to Armour" on the item itself is the site's "+# to Armour (Local)", not the global one
        // (that's for belts and jewels); try the local one first.
        let local = || index.find(&ModLine { text: format!("{} (Local)", m.text), ..m.clone() })
            .filter(|_| local_first && local_fits(category(&item.item_class), &m.text));
        // Foulborn: its mutated mod first, then like any armour/weapon (the local version, 21 Nov: Foulborn Ghostwrithe's
        // "+183 to maximum Energy Shield" went to the global stat and found nothing).
        let found = if foulborn { index.find(&ModLine { kind: "mutated".into(), ..m.clone() }).filter(|(id, _)| id.starts_with("mutated.")).or_else(local).or_else(|| index.find(m)) } else { local().or_else(|| index.find(m)) };
        // "Curse Enemies with Punishment on Hit" is the site's "#% chance to Curse Enemies with Punishment on Hit" at 100%
        // (21 Nov: Foulborn Asenath's Gentle Touch showed it as not searchable).
        // Same for map mods: "Monsters Hinder on Hit with Spells" is "Monsters have #% chance to Hinder on Hit with
        // Spells" at 100% (21 Nov, his tier 16 maps).
        let always = |t: String| index.find(&ModLine { text: t, values: vec![100.0], ..m.clone() });
        let found = found.or_else(|| (!m.text.contains("chance")).then(|| match m.text.split_once(' ') {
            Some((who @ ("Monsters" | "Players" | "Minions"), rest)) => always(format!("{who} have 100% chance to {rest}")),
            _ => always(format!("100% chance to {}", m.text)),
        }).flatten());
        let Some((id, value)) = found else { unknown.push(m.text.clone()); unknown_kinds.push(m.kind.clone()); continue };
        // Found on the mutated list = a Foulborn mod (shown in its own colour).
        let kind = if id.starts_with("mutated.") { "mutated".to_string() } else { m.kind.clone() };
        let (t, _) = template(&m.text);
        // Flasks: both of a magic flask's mods are what you pay for, so without tiers every one counts.
        let good = if has_tiers { m.tier.map(|t| t <= 2).unwrap_or(false) } else { (flask && !(value < 0.0)) || VALUABLE.iter().any(|k| t.contains(k)) };
        let junk = is_junk(&t);
        // Maps: price by tier and rarity; mods are there to tick, never ticked for you.
        let mut enabled = !unique && !is_map(&item) && !is_invitation(&item) && m.kind != "enchant" && good && !junk && !counted.contains(&i);
        // A mod with no number (or listed word for word, value NaN) is just "has it": no minimum.
        let whole = value.is_nan();
        let value = if whole { 0.0 } else { value };
        let mut min = if m.values.is_empty() || whole { None } else { Some(min_for(value)) };
        let mut max = None;
        // Uniques: every mod that rolls is ticked at exactly your roll (23 Nov: he wanted the price window to open with the
        // unique's own numbers; it used to start 10% of each range below, Awakened-style; the -10% / -20% buttons loosen it).
        // Fixed mods and plain copies (no ranges) stay unticked.
        if unique && !whole && value > 0.0 && m.kind != "enchant" {
            if let (Some(roll), Some(w)) = (m.roll, m.width) {
                let _ = roll;
                let slack = 0.0;
                let lo = value - slack;
                min = Some(if value.fract() == 0.0 && w >= 5.0 { lo.floor() } else { (lo * 10.0).floor() / 10.0 });
                enabled = !junk;
            }
        }
        // A corruption implicit (Vaal Orb) is a big part of the price: ticked, at least your roll.
        if m.source.as_deref() == Some("corrupted") && !whole { enabled = true; min = Some(value); }
        if m.source.as_deref() == Some("corrupted") && whole { enabled = true; }
        // Anointed amulets and rings: the oil's notable sets the price as much as the item does.
        if m.kind == "enchant" && m.text.starts_with("Allocates ") && matches!(item.item_class.as_str(), "Amulets" | "Rings") { enabled = true; }
        // Cluster jewels (Awakened's rule): the passive count exactly (8 passives is a better large jewel than 10) and
        // what the small passives grant (it decides which notables can roll). Its jewel sockets come with the count.
        if cluster && !unique && m.kind == "enchant" {
            if t.starts_with("Adds # Passive Skills") { enabled = true; max = min; }
            if t.starts_with("Added Small Passive Skills grant") { enabled = true; }
        }
        // Maps: what kind of map it is (Elder guardian, conqueror, Shaper, Originator) and its enchants (Terror Shrines)
        // set the price far more than its mods: ticked (26 Nov: his Elder and Originator maps priced as plain T16s).
        if is_map(&item) && (m.kind == "enchant" || m.kind == "implicit" && ["influenced by", "occupied by", "Citadel", "Originator"].iter().any(|k| m.text.contains(k))) {
            enabled = true;
        }
        // Wickerman body parts: the one mod is what you pay for.
        if item.item_class == "Viscera" { enabled = true; }
        filters.push(StatFilter { id, label: m.text.clone(), value, min, max, enabled, kind, tier: m.tier, junk, roll: m.roll, range: m.range.clone(), affix: m.affix.clone(), source: m.source.clone() });
    }
    if is_map(&item) { filters.splice(0..0, crate::trade_rules::map_props(text, &item)); }
    // Heist contracts and blueprints: area level, wings, the job they need.
    filters.splice(0..0, crate::trade_rules::heist_props(text, &item));
    filters.extend(temple_rows(text, index));
    filters.extend(mercenary_rows(text, index));
    filters.extend(vessel_rows(text));
    // Influences ("Has Shaper Influence"), there to tick: the mods they bring are usually what's ticked.
    for inf in &item.influences {
        let id = index.pseudo_id(&format!("Has {inf} Influence")).unwrap_or_else(|| format!("pseudo.pseudo_has_{}_influence", inf.to_lowercase()));
        filters.push(StatFilter { id, label: format!("{inf} item"), kind: "pseudo".into(), ..Default::default() });
    }
    // Timeless jewels (Awakened's rule): the seed decides which passives change, the conqueror the keystone. The jewel's
    // own line is ticked with the exact seed; one more row, unticked, takes the same seed under any of its conquerors.
    if let Some(i) = filters.iter().position(|f| f.id.starts_with("explicit.pseudo_timeless_jewel_")) {
        filters[i].enabled = true;
        filters[i].min = Some(filters[i].value);
        let family = index.timeless_family(&filters[i].id);
        if family.len() > 1 {
            let names: Vec<&str> = family.iter().map(|(_, n)| n.as_str()).collect();
            filters.insert(i + 1, StatFilter {
                id: format!("timeless_any:{}", family.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>().join(",")),
                label: format!("Seed {} under any conqueror ({})", filters[i].value, names.join(", ")),
                value: filters[i].value, min: Some(filters[i].value), enabled: false, kind: "explicit".into(),
                ..Default::default()
            });
        }
    }
    else if !is_invitation(&item) { filters.extend(crate::trade_rules::open_affixes(text, &item, index)); }
    // The game's own order: defences / damage first, then totals, enchants, implicits, prefixes, suffixes,
    // and the "open prefix / suffix" rows last. Same-group rows keep the item's order.
    let affix_of = |f: &StatFilter| f.affix.clone();
    let group = |f: &StatFilter| -> u8 {
        if f.id.starts_with("prop.") { return 0 }
        if f.kind == "pseudo" { return if f.id.contains("empty") { 9 } else { 1 } }
        match f.kind.as_str() {
            "enchant" => 2, "implicit" => 3,
            _ => match affix_of(f).as_deref() { Some("prefix") => 4, Some("suffix") => 5, _ => 6 },
        }
    };
    filters.sort_by_key(|f| group(f));
    let links = item.sockets.as_deref().map(crate::item::max_links).unwrap_or(0);
    let gem_level = text.replace('\r', "").split("--------").nth(1)
        .and_then(|s| s.lines().find_map(|l| l.trim().strip_prefix("Level: ")))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .filter(|_| item.item_class.contains("Gem"));
    // Nothing worth searching on (white/unidentified items, or only weak mods): it's
    // the base that's for sale, so search that base at this item level or better.
    let nothing_picked = !filters.iter().any(|f| f.enabled);
    let gear = category(&item.item_class).is_some();
    let heist_mission = matches!(item.item_class.as_str(), "Contracts" | "Blueprints");
    let base_item = base_item(text, &item, &lines, &filters);
    let base = plain_base(&item);
    let affix_max = Some(crate::trade_rules::affix_limits(text, &item));
    // Gems: their level and quality as rows he can untick or change (26 Nov: "I can't click the 23% quality"; they were
    // fixed parts of the search, shown only as "Gem level 1+" / "Quality 23+%").
    let mut filters = filters;
    if item.item_class.contains("Gem") {
        let mut rows = vec![];
        if let Some(l) = gem_level { rows.push(StatFilter { id: "prop.gem_level".into(), label: format!("Gem level: {l}"), value: l as f64, min: Some(l as f64), enabled: true, kind: "property".into(), ..Default::default() }); }
        if let Some(q) = item.quality.filter(|q| *q > 0) { rows.push(StatFilter { id: "prop.quality".into(), label: format!("Quality: {q}%"), value: q as f64, min: Some(q as f64), enabled: true, kind: "property".into(), ..Default::default() }); }
        filters.splice(0..0, rows);
    }
    Some(TradeSetup {
        category: category(&item.item_class).map(|c| if item.base_type.ends_with("Cluster Jewel") { "jewel.cluster" } else { c }).map(String::from),
        // Flasks and tinctures: a Quicksilver Flask's price has nothing to do with a Diamond Flask's, so always this base.
        base_only: !matches!(item.rarity.as_str(), "Rare" | "Magic") || !gear || nothing_picked || flask || heist_mission,
        // Uniques: corrupted or not, like yours (a corrupted one can't be changed, and its implicit is part of the price).
        corrupted: (item.rarity == "Unique" && !item.item_class.contains("Gem")).then_some(item.corrupted),
        buyout: None,
        ilvl_min: (gear && nothing_picked && !heist_mission && item.rarity != "Unique").then_some(item.item_level).flatten(),
        ilvl_max: (gear && nothing_picked && !heist_mission && item.rarity != "Unique").then_some(item.item_level).flatten().and_then(|l| ilvl_band(l).1),
        links_min: (links >= 5).then_some(links),
        gem_level,
        filters, unknown, unknown_kinds, item, raw: text.to_string(), misc: vec![], base, base_item, affix_max,
    })
}

/// Awakened PoE Trade's rule (create-presets.ts) for offering a "base item" search: a magic or rare piece of gear or
/// jewel that isn't finished (no crafted mod, not exactly 20% quality, not corrupted or mirrored) and is worth
/// something to crafters (fractured, synthesised, influenced, a cluster jewel, a magic jewel, or gear of item level
/// 82+). The search keeps fractured mods and enchants, implicits when not influenced or fractured, and on magic
/// items their tier 1-2 mods.
fn base_item(text: &str, item: &Item, lines: &[ModLine], filters: &[StatFilter]) -> Option<BaseItem> {
    let cat = category(&item.item_class)?;
    let has_line = |l: &str| text.lines().any(|x| x.trim() == l);
    let magic = item.rarity == "Magic";
    if !(magic || item.rarity == "Rare") || item.unidentified || cat == "flask" || cat == "tincture"
        || cat.starts_with("heist") || cat == "accessory.trinket" { return None; }
    let fractured = has_line("Fractured Item") || lines.iter().any(|m| m.kind == "fractured");
    let synthesised = has_line("Synthesised Item");
    if item.corrupted || item.mirrored { return None; }
    // Awakened counts a crafted mod or exactly 20% quality as "finished"; a fractured or synthesised item is still a
    // base (crafted mods come off), so those always get the base search.
    let finished = lines.iter().any(|m| m.kind == "crafted") || item.quality == Some(20);
    if finished && !fractured && !synthesised { return None; }
    let cluster = item.base_type.ends_with("Cluster Jewel");
    let jewel = cat.starts_with("jewel.");
    let ilvl = item.item_level.unwrap_or(0);
    let mut why: Vec<String> = Vec::new();
    if fractured { why.push("Fractured".into()); }
    if synthesised { why.push("Synthesised".into()); }
    why.extend(item.influences.iter().cloned());
    if cluster { why.push("Cluster jewel".into()); }
    if jewel && !cluster && cat != "jewel.abyss" && magic { why.push("Magic jewel".into()); }
    if !jewel && ilvl >= 82 { why.push(format!("Item level {ilvl}")); }
    if why.is_empty() { return None; }
    let keep_implicits = item.influences.is_empty() && !fractured;
    let ids = filters.iter().filter(|f| match f.kind.as_str() {
        "fractured" | "enchant" => true,
        "implicit" => keep_implicits,
        // Influences are searched as "Has Shaper Influence" stats (the trade site dropped its "Shaper Item" switch).
        "pseudo" => f.id.ends_with("_influence"),
        "explicit" => magic && !cluster && f.tier.is_some_and(|t| t <= 2),
        _ => false,
    }).map(|f| f.id.clone()).collect();
    // Item level: at least this one (most mod tiers need 68-86). Cluster jewels by bracket; other jewels don't care.
    let ilvl_min = if cluster { [84, 75, 68, 50, 1].into_iter().find(|b| ilvl >= *b) } else if jewel { None } else { Some(ilvl.min(86)) };
    let mut misc: Vec<String> = Vec::new();
    if synthesised { misc.push("synthesised_item".into()); }
    if fractured { misc.push("fractured_item".into()); }
    Some(BaseItem { why, ids, ilvl_min: ilvl_min.filter(|l| *l > 0), misc })
}

/// What's caught in a Blood-filled Vessel: (its unique monsters by name, how many others).
pub fn vessel(text: &str) -> Option<(Vec<String>, u32)> {
    let text = text.replace('\r', "");
    if !text.lines().any(|l| l.trim() == "Blood-filled Vessel") { return None; }
    let num = |s: &str| s.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse::<u32>().ok();
    let sec = text.split("--------").find(|s| s.trim_start().starts_with("Monsters:"))?;
    let (mut names, mut others) = (vec![], 0);
    for l in sec.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(n) = l.strip_prefix("Monsters:") { others = num(n).unwrap_or(0); }
        else if l.ends_with("Other Monsters") { others = num(l).unwrap_or(0); }
        else if l.starts_with("Monster Level:") || l.starts_with("From:") { break; }
        else { names.push(l.to_string()); }
    }
    Some((names, others))
}

/// A vessel's uniques and others as search rows (the site counts them, it can't search them by name): uniques ticked.
fn vessel_rows(text: &str) -> Vec<StatFilter> {
    let Some((names, others)) = vessel(text) else { return vec![] };
    let row = |id: &str, label: String, n: f64, enabled: bool| StatFilter { id: id.into(), label, value: n, min: Some(n), enabled, kind: "pseudo".into(), ..Default::default() };
    let mut out = vec![];
    if !names.is_empty() { out.push(row("pseudo.pseudo_ritual_unique_monsters", format!("Unique monsters: {}", names.join(" + ")), names.len() as f64, true)); }
    if others > 0 { out.push(row("pseudo.pseudo_ritual_other_monsters", format!("Other monsters: {others}"), others as f64, false)); }
    out
}

/// The listings with all of these unique monsters first (cheapest first within each), marked; how many there are.
pub fn same_monsters(ls: &mut [Listing], names: &[String]) -> usize {
    if names.is_empty() { return 0; }
    for l in ls.iter_mut() { l.same = names.iter().all(|n| l.monsters.iter().any(|m| m.eq_ignore_ascii_case(n))); }
    ls.sort_by_key(|l| !l.same);
    ls.iter().filter(|l| l.same).count()
}

/// A Chronicle of Atzoatl's open rooms: the ones worth something ticked.
/// A Mercenary Warrant's skills and supports, as the trade site's "Mercenary" stats ("Greater Multiple Projectiles
/// (Tier 3)"). 26 Nov: warrants were searched by name only and all came out at 1c, though some sell for a fortune.
/// Tier 3 supports come ticked: they set the price (26 Nov, live: his Cicero warrant's five Tier 3 supports matched 7 for
/// sale at 35c to 1 div+, its name alone 10,000 at 1c). Skills and lower tiers are there to tick. Words listed twice on the site are
/// searched as "either of them" (`any_of:`).
fn mercenary_rows(text: &str, index: &StatIndex) -> Vec<StatFilter> {
    let text = text.replace('\r', "");
    if !text.contains("Mercenary Level:") { return vec![] }
    let mut out = vec![];
    let mut after = false;
    for sec in text.split("--------") {
        let lines: Vec<&str> = sec.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.iter().any(|l| l.starts_with("Mercenary Level:")) { after = true; continue; }
        if !after { continue; }
        if lines.iter().any(|l| l.starts_with("Right click") || l.starts_with("Open a portal")) { break; }
        for l in lines {
            let (name, tier) = match l.rsplit_once(" (Tier: ") {
                Some((n, t)) => (n, t.trim_end_matches(')').trim().parse::<u32>().ok()),
                None => (l, None),
            };
            let label = match tier { Some(t) => format!("{name} (Tier {t})"), None => name.to_string() };
            // A support on two skills ("Critical Chance (Tier 2)" twice) is one row.
            if out.iter().any(|f: &StatFilter| f.label == label) { continue; }
            let ids = index.ids("mercenary", &label);
            if ids.is_empty() { continue; }
            let id = if ids.len() == 1 { ids[0].clone() } else { format!("any_of:{}", ids.join(",")) };
            out.push(StatFilter { id, label, kind: "mercenary".into(), enabled: tier == Some(3), tier,
                junk: tier.is_some_and(|t| t < 3), ..Default::default() });
        }
    }
    out
}

fn temple_rows(text: &str, index: &StatIndex) -> Vec<StatFilter> {
    let Some((rooms, level)) = crate::temple::read(text) else { return vec![] };
    let t = crate::temple::Temples::shipped();
    let mut out: Vec<StatFilter> = rooms.iter().filter_map(|r| {
        let id = index.pseudo_id(&format!("Has Room: {r}"))?;
        let good = t.good(&id);
        // "|1" = the room is open.
        Some(StatFilter { id: format!("{id}|1"), label: r.clone(), kind: "pseudo".into(), enabled: good, junk: !r.contains("(Tier 3)") && !good, ..Default::default() })
    }).collect();
    out.sort_by_key(|f| (!f.enabled, f.junk));
    if let Some(l) = level { out.push(StatFilter { id: "prop.area_level".into(), label: format!("Area level: {l}"), value: l as f64, min: Some(l.min(crate::temple::MIN_LEVEL) as f64), enabled: true, kind: "property".into(), ..Default::default() }); }
    out
}

fn is_map(it: &Item) -> bool { it.item_class == "Maps" }
fn is_invitation(it: &Item) -> bool { crate::maps::runs_like_map(&it.item_class, &it.base_type) }

/// The base as the trade site knows it. Magic items only have a full name ("Demon Dagger of Finesse",
/// "Perpetual Sapphire Flask of Craft"): their base is the longest one inside it.
pub fn plain_base(it: &Item) -> String {
    if it.rarity != "Magic" { return it.base_type.clone(); }
    crate::item::base_classes().keys().filter(|b| it.base_type.contains(b.as_str())).max_by_key(|b| b.len()).cloned()
        .unwrap_or_else(|| it.base_type.clone())
}

/// The map's tier: from "Map Tier: 16", or from the name "Map (Tier 2)".
fn map_tier_of(it: &Item) -> Option<u32> {
    it.map_tier.or_else(|| it.base_type.split("(Tier ").nth(1)?.trim_end_matches(')').trim().parse().ok())
}

/// Item-level bands where base prices jump (better mod tiers unlock at 75, 82, 84 and 86): a base is
/// compared with others in its own band, e.g. 73 -> 68..=74, 84 -> 84..=85, 86 -> 86 and up (None).
pub fn ilvl_band(ilvl: u32) -> (u32, Option<u32>) {
    match ilvl {
        0..=67 => (1, Some(67)), 68..=74 => (68, Some(74)), 75..=81 => (75, Some(81)),
        82..=83 => (82, Some(83)), 84..=85 => (84, Some(85)), _ => (86, None),
    }
}

/// The search to send to the trade site.
pub fn query(s: &TradeSetup) -> Value {
    let it = &s.item;
    // "Instant Buyout and In Person", like the trade site's default: instant buyout
    // listings are real prices the seller can't refuse.
    let mut q = json!({ "status": { "option": "available" } });
    let mut foulborn = false;
    if it.rarity == "Unique" {
        // "Foulborn Ghostwrithe" is listed as Ghostwrithe with the Foulborn switch on.
        if let Some(n) = &it.name {
            let plain = n.strip_prefix("Foulborn ");
            foulborn = plain.is_some();
            q["name"] = json!(plain.unwrap_or(n));
        }
        q["type"] = json!(it.base_type);
    } else if s.base_only || s.category.is_none() {
        q["type"] = json!(plain_base(it));
    }
    let mut filters = json!({});
    let mut type_f = json!({});
    if matches!(it.rarity.as_str(), "Rare" | "Magic" | "Normal") && s.category.is_some() {
        type_f["rarity"] = json!({ "option": "nonunique" });
        if !s.base_only { type_f["category"] = json!({ "option": s.category }); }
    }
    if type_f.as_object().map(|o| !o.is_empty()).unwrap_or(false) { filters["type_filters"] = json!({ "filters": type_f }); }
    let mut misc = json!({});
    if let Some(n) = s.ilvl_min { misc["ilvl"] = json!({ "min": n }); }
    if let (Some(_), Some(m)) = (s.ilvl_min, s.ilvl_max) { misc["ilvl"]["max"] = json!(m); }
    if it.unidentified && it.rarity != "Unique" { misc["identified"] = json!({ "option": "false" }); }
    if foulborn { misc["mutated"] = json!({ "option": "true" }); }
    if let Some(c) = s.corrupted { misc["corrupted"] = json!({ "option": if c { "true" } else { "false" } }); }
    if it.rarity == "Unique" && it.twice_corrupted && s.corrupted == Some(true) { misc["twice_corrupted"] = json!({ "option": "true" }); }
    for k in &s.misc { misc[k.as_str()] = json!({ "option": "true" }); }
    if it.item_class.contains("Gem") {
        misc["corrupted"] = json!({ "option": if it.corrupted { "true" } else { "false" } });
    }
    if misc.as_object().map(|o| !o.is_empty()).unwrap_or(false) { filters["misc_filters"] = json!({ "filters": misc }); }
    if let Some(n) = s.links_min { filters["socket_filters"] = json!({ "filters": { "links": { "min": n } } }); }
    // "Map (Tier 2)" is listed as "Map" with the tier as a filter.
    if is_map(it) && it.rarity != "Unique" {
        if it.base_type.starts_with("Map (Tier") { q["type"] = json!({ "option": "Map", "discriminator": "map" }); }
        if let Some(t) = map_tier_of(it) { filters["map_filters"] = json!({ "filters": { "map_tier": { "min": t, "max": t } } }); }
        filters["type_filters"] = json!({ "filters": { "rarity": { "option": "nonunique" } } });
    }
    // Weapon DPS / defences go in their own filter groups, not the stat list.
    for f in s.filters.iter().filter(|f| f.enabled) {
        if let Some((group, key)) = crate::trade_rules::property_query(&f.id) {
            if filters.get(group).is_none() { filters[group] = json!({ "filters": {} }); }
            filters[group]["filters"][key] = json!({ "min": f.min.unwrap_or(f.value) });
            if let Some(m) = f.max { filters[group]["filters"][key]["max"] = json!(m); }
        }
    }
    if filters.as_object().map(|o| !o.is_empty()).unwrap_or(false) { q["filters"] = filters; }
    // A fractured mod only has to be fractured when pricing the item as a base (fractured switch on); otherwise any item
    // with that mod counts, so it's searched as the normal mod (the trade site gives both the same number).
    let as_base = s.misc.iter().any(|m| m == "fractured_item");
    // A timeless jewel's seed is searched exactly; "any conqueror" is a group needing one of them.
    let seed = |v: f64| json!({ "min": v, "max": v });
    let mut groups: Vec<Value> = Vec::new();
    for f in s.filters.iter().filter(|f| f.enabled) {
        if let Some(ids) = f.id.strip_prefix("any_of:") {
            groups.push(json!({ "type": "count", "value": { "min": 1 }, "filters": ids.split(',').map(|id| json!({ "id": id })).collect::<Vec<_>>() }));
        }
        if let Some(ids) = f.id.strip_prefix("timeless_any:") {
            let v = f.min.unwrap_or(f.value);
            groups.push(json!({ "type": "count", "value": { "min": 1 },
                "filters": ids.split(',').map(|id| json!({ "id": id, "value": seed(v) })).collect::<Vec<_>>() }));
        }
    }
    let stats: Vec<Value> = s.filters.iter().filter(|f| f.enabled && !f.id.starts_with("prop.") && !f.id.starts_with("timeless_any:") && !f.id.starts_with("any_of:")).map(|f| {
        let id = match f.id.strip_prefix("fractured.") { Some(rest) if !as_base => format!("explicit.{rest}"), _ => f.id.clone() };
        let mut v = stat_json(&id);
        if let (Some(m), None) = (f.min, v.get("value")) { v["value"] = json!({ "min": m }); }
        if let (Some(m), Some(_)) = (f.max, v.get("value").and_then(|x| x.get("min"))) { v["value"]["max"] = json!(m); }
        if f.id.starts_with("explicit.pseudo_timeless_jewel_") { v["value"] = seed(f.min.unwrap_or(f.value)); }
        v
    }).collect();
    q["stats"] = json!([json!({ "type": "and", "filters": stats })].into_iter().chain(groups).collect::<Vec<_>>());
    json!({ "query": q, "sort": { "price": "asc" } })
}

/// Everything a search asks for besides the ticked stat rows, in plain words (the "hidden filters" Awakened shows):
/// listing status, name or base, rarity, kind of item, item level, corrupted, links, map tier and so on.
pub fn describe(q: &Value) -> Vec<String> {
    let q = &q["query"];
    let mut out = Vec::new();
    out.push(match q["status"]["option"].as_str() {
        Some("securable") => "Instant buyout only".to_string(),
        Some("available") => "Instant buyout and in person".to_string(),
        Some("online") | Some("onlineleague") => "Sellers online".to_string(),
        _ => "Any listing".to_string(),
    });
    if let Some(n) = q["name"].as_str() { out.push(format!("Named {n}")); }
    match &q["type"] {
        Value::String(t) => out.push(format!("Base: {t}")),
        Value::Object(o) => if let Some(t) = o.get("option").and_then(|x| x.as_str()) { out.push(format!("Base: {t}")); },
        _ => {}
    }
    let f = &q["filters"];
    match f["type_filters"]["filters"]["rarity"]["option"].as_str() {
        Some("nonunique") => out.push("Not unique".into()),
        Some(r) => out.push(format!("Rarity: {r}")),
        None => {}
    }
    if let Some(c) = f["type_filters"]["filters"]["category"]["option"].as_str() { out.push(format!("Any {}", category_words(c))); }
    let range = |v: &Value| match (v["min"].as_f64(), v["max"].as_f64()) {
        (Some(a), Some(b)) if a == b => format!("{a}"), (Some(a), Some(b)) => format!("{a}–{b}"),
        (Some(a), None) => format!("{a}+"), (None, Some(b)) => format!("up to {b}"), _ => String::new(),
    };
    let yes = |v: &Value| v["option"].as_str() == Some("true");
    if let Some(m) = f["misc_filters"]["filters"].as_object() {
        for (k, v) in m {
            out.push(match k.as_str() {
                "ilvl" => format!("Item level {}", range(v)),
                "gem_level" => format!("Gem level {}", range(v)),
                "quality" => format!("Quality {}%", range(v)),
                "corrupted" => if yes(v) { "Corrupted".into() } else { "Not corrupted".into() },
                "identified" => if yes(v) { "Identified".into() } else { "Unidentified".into() },
                "mutated" => "Foulborn".into(),
                "fractured_item" => "Fractured".into(),
                "synthesised_item" => "Synthesised".into(),
                other => format!("{}: {}", other.replace('_', " "), if v.get("option").is_some() { if yes(v) { "yes".into() } else { "no".into() } } else { range(v) }),
            });
        }
    }
    if let Some(l) = f["socket_filters"]["filters"]["links"].as_object() { out.push(format!("{} links", range(&Value::Object(l.clone())))); }
    if let Some(t) = f["map_filters"]["filters"].get("map_tier") { out.push(format!("Map tier {}", range(t))); }
    out
}

/// "armour.helmet" -> "helmet", "heistmission.contract" -> "contract".
fn category_words(c: &str) -> String {
    let last = c.rsplit('.').next().unwrap_or(c);
    match last {
        "chest" => "body armour".into(), "onesword" => "one-handed sword".into(), "twosword" => "two-handed sword".into(),
        "oneaxe" => "one-handed axe".into(), "twoaxe" => "two-handed axe".into(), "onemace" => "one-handed mace".into(),
        "twomace" => "two-handed mace".into(), "runedagger" => "rune dagger".into(), "base" => "jewel".into(),
        "abyss" => "abyss jewel".into(), "cluster" => "cluster jewel".into(), "heistweapon" => "heist gear".into(),
        "heisttool" => "heist tool".into(), "heistutility" => "heist cloak".into(), "heistreward" => "heist brooch".into(),
        x => x.to_string(),
    }
}

/// A stat filter for a search. The stat list writes pick-one stats as "enchant.stat_3948993189|14"
/// ("Added Small Passive Skills grant: 12% increased Lightning Damage"); the search wants the id and option 14.
pub fn stat_json(id: &str) -> Value {
    match id.split_once('|') {
        Some((stat, opt)) => json!({ "id": stat, "value": { "option": opt.parse::<u64>().map(Value::from).unwrap_or_else(|_| json!(opt)) } }),
        None => json!({ "id": id }),
    }
}

/// One listing from the trade site.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Listing {
    pub amount: f64,
    pub currency: String,
    /// In chaos, when we know the currency's value.
    pub chaos: Option<f64>,
    pub account: String,
    pub indexed: String,
    pub whisper: String,
    pub ilvl: Option<u32>,
    /// Instant buyout (from the seller's merchant tab): buy it from the trade site or in game, no whisper.
    pub instant: bool,
    /// The listed item's mods (implicit, fractured, explicit, crafted), to compare with yours.
    #[serde(default)]
    pub mods: Vec<String>,
    /// Blood-filled Vessels: the unique monsters in it, how many others, and whether it has all of yours.
    #[serde(default)]
    pub monsters: Vec<String>,
    #[serde(default)]
    pub others: Option<u32>,
    #[serde(default)]
    pub same: bool,
}

/// Reads the trade site's fetch answer.
pub fn listings(fetched: &Value, rates: &HashMap<String, f64>) -> Vec<Listing> {
    fetched["result"].as_array().into_iter().flatten().filter_map(|r| {
        let l = &r["listing"];
        let p = &l["price"];
        let amount = p["amount"].as_f64()?;
        let currency = p["currency"].as_str()?.to_string();
        // Vessels: "Monsters:\n{0}" = the uniques, one a line; "{0} Other Monsters" / "Monsters" = how many (others).
        let vessel = r["item"]["baseType"] == "Blood-filled Vessel";
        let prop = |f: &dyn Fn(&str) -> bool| r["item"]["properties"].as_array().into_iter().flatten().filter(|_| vessel)
            .find(|p| p["name"].as_str().is_some_and(f)).and_then(|p| p["values"][0][0].as_str()).map(String::from);
        Some(Listing {
            monsters: prop(&|n| n.starts_with("Monsters:")).map(|v| v.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default(),
            others: prop(&|n| n.ends_with("Other Monsters") || n == "Monsters").and_then(|v| v.parse().ok()),
            same: false,
            chaos: rates.get(&currency).map(|r| r * amount),
            amount, currency,
            account: l["account"]["lastCharacterName"].as_str().or(l["account"]["name"].as_str()).unwrap_or("").to_string(),
            indexed: l["indexed"].as_str().unwrap_or("").to_string(),
            whisper: l["whisper"].as_str().unwrap_or("").to_string(),
            ilvl: r["item"]["ilvl"].as_u64().map(|v| v as u32),
            instant: l.get("fee").is_some() || l["whisper"].as_str().map(str::is_empty).unwrap_or(true),
            mods: ["implicitMods", "fracturedMods", "explicitMods", "craftedMods"].iter()
                .flat_map(|k| r["item"][*k].as_array().into_iter().flatten()
                    // Plain text, or (since Nov 2026) { "description": "Has 1 Abyssal Socket", … }.
                    .filter_map(|m| m.as_str().or_else(|| m["description"].as_str()).map(String::from))).collect(),
        })
    }).collect()
}

/// What the cheapest listings say it's worth: (lowest, typical) in chaos.
pub fn summary(ls: &[Listing]) -> Option<(f64, f64)> {
    // Price fixers list a few very cheap copies (often the same seller many times) to drag the price down.
    // Count each seller once, at their cheapest. Then "sells for about" is the 3rd cheapest seller: near enough the bottom
    // to be what it really sells for, while one or two suspiciously cheap listings don't decide it. (23 Nov: it was the
    // middle of the cheapest ten, 90c on his Xirgil's Crank with instant 60c, 60c, 70c and 74c ones listed.)
    // With fewer than five sellers, the middle one.
    let mut by_seller: Vec<(&str, f64)> = Vec::new();
    for l in ls {
        let Some(c) = l.chaos else { continue };
        match by_seller.iter_mut().find(|(a, _)| *a == l.account) {
            Some(s) => if c < s.1 { s.1 = c },
            None => by_seller.push((&l.account, c)),
        }
    }
    let mut v: Vec<f64> = by_seller.into_iter().map(|(_, c)| c).collect();
    if v.is_empty() { return None; }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let typical = if v.len() >= 5 { v[2] } else { v[v.len() / 2] };
    Some((v[0], typical))
}

/// Trade site currency ids -> names poe.ninja uses.
pub const CURRENCY_NAMES: &[(&str, &str)] = &[
    ("divine", "Divine Orb"), ("exalted", "Exalted Orb"), ("alch", "Orb of Alchemy"), ("fusing", "Orb of Fusing"),
    ("alt", "Orb of Alteration"), ("jewellers", "Jeweller's Orb"), ("chrome", "Chromatic Orb"), ("chance", "Orb of Chance"),
    ("regal", "Regal Orb"), ("vaal", "Vaal Orb"), ("scour", "Orb of Scouring"), ("gcp", "Gemcutter's Prism"),
    ("mirror", "Mirror of Kalandra"), ("annul", "Orb of Annulment"), ("aug", "Orb of Augmentation"), ("transmute", "Orb of Transmutation"), ("regret", "Orb of Regret"), ("blessed", "Blessed Orb"),
];

/// Chaos value of each trade currency, from poe.ninja prices.
pub fn rates(price_of: impl Fn(&str) -> Option<f64>) -> HashMap<String, f64> {
    let mut m: HashMap<String, f64> = CURRENCY_NAMES.iter().filter_map(|(id, name)| price_of(name).map(|v| (id.to_string(), v))).collect();
    m.insert("chaos".into(), 1.0);
    m
}

// ---------- bulk exchange (currency, fragments, scarabs…) ----------

/// The trade site's item ids for bulk exchange ("divine", "chaos"…), from
/// api/trade/data/static, by name.
#[derive(Debug, Default)]
pub struct StaticIndex { by_name: HashMap<String, String> }

impl StaticIndex {
    pub fn from_json(v: &Value) -> StaticIndex {
        let mut by_name = HashMap::new();
        for group in v["result"].as_array().into_iter().flatten() {
            for e in group["entries"].as_array().into_iter().flatten() {
                if let (Some(id), Some(text)) = (e["id"].as_str(), e["text"].as_str()) {
                    by_name.entry(text.to_lowercase()).or_insert_with(|| id.to_string());
                }
            }
        }
        StaticIndex { by_name }
    }
    pub fn id(&self, name: &str) -> Option<String> { self.by_name.get(&name.to_lowercase()).cloned() }
}

/// Item names the trade site searches differently from how the game writes them:
/// transfigured gems ("Scourge Arrow of Menace" = type "Scourge Arrow" + a
/// discriminator), from api/trade/data/items.
#[derive(Debug, Default)]
pub struct ItemTypes { by_text: HashMap<String, (String, String)>, /// Every base type the trade site knows (lower case), to tell a base from extra words in front of it.
    bases: std::collections::HashSet<String>,
    /// Every unique's name (lower case -> as the site writes it), for renamed ones ("Endgame Marylene's Fallacy" in a PoB guide).
    uniques: HashMap<String, String> }

impl ItemTypes {
    pub fn from_json(v: &Value) -> ItemTypes {
        let mut by_text = HashMap::new();
        let mut bases = std::collections::HashSet::new();
        let mut uniques = HashMap::new();
        for group in v["result"].as_array().into_iter().flatten() {
            for e in group["entries"].as_array().into_iter().flatten() {
                if e.get("name").is_none() { if let Some(t) = e["type"].as_str() { bases.insert(t.to_lowercase()); } }
                if let Some(n) = e["name"].as_str() { uniques.insert(n.to_lowercase(), n.to_string()); }
                if let (Some(t), Some(text), Some(disc)) = (e["type"].as_str(), e["text"].as_str(), e["disc"].as_str()) {
                    if e.get("name").is_none() { by_text.entry(text.to_lowercase()).or_insert_with(|| (t.to_string(), disc.to_string())); }
                }
            }
        }
        ItemTypes { by_text, bases, uniques }
    }

    /// Rewrites a search's plain type ("Scourge Arrow of Menace") into the form the
    /// trade site wants ({option: "Scourge Arrow", discriminator: "alt_x"}).
    /// Does the trade site list this base at all? (true when its list hasn't loaded)
    pub fn sells(&self, base: &str) -> bool { self.bases.is_empty() || self.bases.contains(&base.to_lowercase()) }

    pub fn fix_query(&self, q: &mut Value) {
        // A unique with words in front of its name (renamed in PoB): drop words until it's a unique the site knows.
        if let Some(n) = q["query"]["name"].as_str().map(str::to_string) {
            if !self.uniques.is_empty() && !self.uniques.contains_key(&n.to_lowercase()) {
                let words: Vec<&str> = n.split_whitespace().collect();
                if let Some(real) = (1..words.len()).find_map(|i| self.uniques.get(&words[i..].join(" ").to_lowercase())) {
                    q["query"]["name"] = json!(real);
                }
            }
        }
        let Some(t) = q["query"]["type"].as_str().map(str::to_string) else { return };
        // "Fated Composite Ring" (unidentified, with a word in front): drop leading words until it's a base the site knows.
        let mut t = t;
        if !self.bases.is_empty() && !self.bases.contains(&t.to_lowercase()) && !self.by_text.contains_key(&t.to_lowercase()) {
            let words: Vec<&str> = t.split_whitespace().collect();
            if let Some(i) = (1..words.len()).find(|i| self.bases.contains(&words[*i..].join(" ").to_lowercase())) {
                t = words[i..].join(" ");
                q["query"]["type"] = json!(t);
            }
        }
        if let Some((ty, disc)) = self.by_text.get(&t.to_lowercase()) {
            q["query"]["type"] = json!({ "option": ty, "discriminator": disc });
        }
    }
}

/// A bulk exchange search: people selling `want` for `have`.
pub fn exchange_query(have: &str, want: &str, minimum: Option<u32>) -> Value {
    let mut q = json!({ "status": { "option": "online" }, "have": [have], "want": [want] });
    if let Some(m) = minimum { q["minimum"] = json!(m); }
    json!({ "query": q, "sort": { "have": "asc" }, "engine": "new" })
}

/// One bulk offer: they give `get` of the item for `pay` of your currency.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Offer {
    pub pay: f64,
    pub pay_currency: String,
    pub get: f64,
    pub get_currency: String,
    /// Your currency per one item.
    pub each: f64,
    pub stock: u32,
    pub account: String,
    pub whisper: String,
}

/// Reads a bulk exchange answer (cheapest first).
pub fn offers(v: &Value) -> Vec<Offer> {
    let mut out: Vec<Offer> = v["result"].as_object().into_iter().flat_map(|m| m.values()).filter_map(|r| {
        let l = &r["listing"];
        let o = l["offers"].as_array()?.first()?;
        let (ex, it) = (&o["exchange"], &o["item"]);
        let pay = ex["amount"].as_f64()?;
        let get = it["amount"].as_f64()?;
        if get <= 0.0 { return None; }
        let fill = |w: &str, n: f64| w.replace("{0}", &format!("{}", n));
        // The site gives a whisper with gaps for the amounts: one lot of the offer.
        let whisper = l["whisper"].as_str().map(|w| w
            .replace("{0}", &fill(it["whisper"].as_str().unwrap_or("{0}"), get))
            .replace("{1}", &fill(ex["whisper"].as_str().unwrap_or("{0}"), pay))).unwrap_or_default();
        Some(Offer {
            pay, pay_currency: ex["currency"].as_str().unwrap_or("").into(),
            get, get_currency: it["currency"].as_str().unwrap_or("").into(),
            each: pay / get, stock: it["stock"].as_u64().unwrap_or(0) as u32,
            account: l["account"]["lastCharacterName"].as_str().or(l["account"]["name"].as_str()).unwrap_or("").into(),
            whisper,
        })
    }).collect();
    out.sort_by(|a, b| a.each.partial_cmp(&b.each).unwrap_or(std::cmp::Ordering::Equal));
    out
}

// ---------- "Buy" from your PoB build ----------

/// The trade search behind "Buy" (like PoEStash's build links): the exact base, the rarity, and every mod the
/// item has, each just "has this mod" with no numbers, so you see everything with that set of mods and add
/// minimums on the site. Uniques: by name (Watcher's Eye style mods ticked too). Cluster jewels keep their
/// passive count exact. Returns the search and how many mods it asks for.
pub fn buy_query(text: &str, index: &StatIndex) -> Option<(Value, usize)> {
    let item = parse_item(text)?;
    let unique = item.rarity == "Unique";
    let rolls = if unique { item.name.as_deref().map(|n| unique_rolls(n.strip_prefix("Foulborn ").unwrap_or(n))).unwrap_or_default() } else { vec![] };
    let cat = category(&item.item_class);
    let local_first = cat.is_some_and(|c| c.starts_with("armour.") || c.starts_with("weapon."));
    let mut stats: Vec<Value> = Vec::new();
    // A rare or magic item (28 Nov 2026, he asked: resistances "and max life etc are PSeudo tags, so we can filter by how
    // much"): life, fire / cold / lightning resistance, Str / Dex / Int and movement speed become the trade site's totals,
    // with NO number (he: "IM NOT SAYING TO PUT A MINIMUM IN": he types his own on the site), for the ones this item has (its own rolled mods; crafted and implicit ones left out: you craft those, and a
    // swapped base has other implicits). The mods that make them up aren't asked for one by one. Chaos stays as it is.
    const TOTALS: [&str; 8] = ["Life", "Fire res", "Cold res", "Lightning res", "Strength", "Dexterity", "Intelligence", "Movement speed"];
    let all_mods = mods(text);
    let rolled: Vec<String> = all_mods.iter().filter(|m| m.kind == "explicit" || m.kind == "fractured").map(|m| m.text.clone()).collect();
    let totals = if unique { Default::default() } else { crate::compare::totals(&rolled) };
    let in_totals = |line: &str| !unique && { let t = crate::compare::totals(&[line.to_string()]); TOTALS.iter().any(|k| t.get(k).copied().unwrap_or(0.0) != 0.0) };
    for k in TOTALS {
        let v = totals.get(k).copied().unwrap_or(0.0);
        if v >= 1.0 { if let Some(id) = crate::upgrade::pseudo(k) { stats.push(json!({ "id": id })); } }
    }
    for m in all_mods {
        if (m.kind == "explicit" || m.kind == "fractured") && in_totals(&m.text) { continue; }
        let local = || index.find(&ModLine { text: format!("{} (Local)", m.text), ..m.clone() }).filter(|_| local_first && local_fits(cat, &m.text));
        // A cluster jewel's "2 Added Passive Skills are Jewel Sockets": comes with its passive count, and asking for it
        // (as enchant or explicit, any number) finds nothing on the trade site (tested 21 Nov, his large lightning
        // cluster: 0 with it, 234 without). Leave it out.
        if m.text.contains("Jewel Socket") && item.base_type.ends_with("Cluster Jewel") { continue; }
        let Some((id, value)) = local().or_else(|| index.find(&m)) else { continue };
        // A unique's fixed mods come with the name; only mods that differ between copies are worth asking for
        // (Watcher's Eye auras, a timeless jewel's seed, its anoint, and the mods that roll).
        let timeless = id.starts_with("explicit.pseudo_timeless_jewel_");
        let anoint = m.text.starts_with("Allocates ") && m.kind == "enchant";
        let roll = rolls.iter().find(|r| r.0 == template(&m.text).0).filter(|_| !timeless);
        // A corrupted unique's implicit is what the Vaal Orb gave it (a Tabula Rasa with "+2 to Level of Socketed
        // Projectile Gems"): asked for, at least that number.
        let corrupt_implicit = unique && item.corrupted && m.kind == "implicit";
        if unique && !m.text.contains("while affected by") && !timeless && !anoint && roll.is_none() && !corrupt_implicit { continue; }
        if stats.iter().any(|v| v["id"] == id.as_str()) { continue; }
        let mut v = stat_json(&id);
        if corrupt_implicit && roll.is_none() && value.is_finite() && value > 0.0 && v.get("value").is_none() { v["value"] = json!({ "min": value }); }
        // A unique's rolling mod (27 Nov, his KF build: Timeclasp's duration is the whole point of the ring): at
        // least the PoB guide's roll ("reduced" is searched as negative "increased", so at most). Ticked only when
        // the roll can land either side of zero, where a bad copy does the opposite (Timeclasp's -20 to 20%);
        // the rest are on the page, unticked, one click to ask for.
        if let Some(&(_, lo, hi)) = roll {
            if value.is_finite() && v.get("value").is_none() {
                v["value"] = if value < 0.0 { json!({ "max": value }) } else { json!({ "min": value }) };
                if !(lo < 0.0 && hi > 0.0) { v["disabled"] = json!(true); }
            }
        }
        // Crafted mods: you can craft them yourself, so they're on the page but not required.
        if m.kind == "crafted" { v["disabled"] = json!(true); }
        // "Adds 8 Passive Skills" / "2 Added Passive Skills are Jewel Sockets": that number is the jewel.
        if (timeless || m.kind == "enchant" && m.text.contains("Passive Skill")) && value.is_finite() && v.get("value").is_none() { v["value"] = json!({ "min": value, "max": value }); }
        stats.push(v);
    }
    // The base as the trade site knows it: magic items only have a full name ("Perpetual Sapphire Flask of Craft").
    let base = plain_base(&item);
    let mut q = json!({ "status": { "option": "securable" }, "type": base, "stats": [{ "type": "and", "filters": stats }] });
    if unique {
        if let Some(n) = &item.name { q["name"] = json!(n.strip_prefix("Foulborn ").unwrap_or(n)); }
        if item.corrupted { q["filters"] = json!({ "misc_filters": { "filters": { "corrupted": { "option": "true" } } } }); }
    } else if cat.is_some() {
        let rarity = match item.rarity.as_str() { "Rare" => "rare", "Magic" => "magic", "Normal" => "normal", _ => "nonunique" };
        q["filters"] = json!({ "type_filters": { "filters": { "rarity": { "option": rarity } } } });
    }
    let n = q["stats"][0]["filters"].as_array().map(|a| a.iter().filter(|v| v.get("disabled").is_none()).count()).unwrap_or(0);
    Some((json!({ "query": q, "sort": { "price": "asc" } }), n))
}

/// A unique's mods that roll, from core/data/poe1_uniques.json: (the line's template as `template` writes it, low
/// end, high end). Only lines with one range ("(-20-20)% reduced Skill Effect Duration"); "Adds (1-2) to (3-4)" is left out.
fn unique_rolls(name: &str) -> Vec<(String, f64, f64)> {
    let Some(u) = crate::gear::uniques().iter().find(|u| u.name.eq_ignore_ascii_case(name)) else { return vec![] };
    u.mods.iter().filter_map(|m| {
        if m.matches('(').count() != 1 { return None; }
        let i = m.find('(')?;
        let j = i + m[i..].find(')')?;
        let inner = &m[i + 1..j];
        // "-" splits the two ends; a leading "-" belongs to the number ("(-20-20)").
        let k = inner.char_indices().skip(1).find(|(_, c)| *c == '-')?.0;
        let (lo, hi) = (inner[..k].parse::<f64>().ok()?, inner[k + 1..].parse::<f64>().ok()?);
        Some((template(&format!("{}1{}", &m[..i], &m[j + 1..])).0, lo, hi))
    }).collect()
}

/// Only items you can wear at `level` (28 Nov 2026: his "Swap at Merciless Lab (level 68-90)" stage's Buy
/// links found rares needing level 84).
pub fn max_level(q: &mut Value, level: u32) {
    let query = &mut q["query"];
    if !query["filters"].is_object() { query["filters"] = json!({}); }
    query["filters"]["req_filters"] = json!({ "filters": { "lvl": { "max": level } } });
}

/// The same search on another base (one you can wear sooner): its implicit mods are the old base's, so they're
/// left out; the explicit mods stay. Returns how many mods it now asks for.
pub fn swap_base(q: &mut Value, base: &str) -> usize {
    q["query"]["type"] = json!(base);
    if let Some(f) = q["query"]["stats"][0]["filters"].as_array_mut() {
        f.retain(|v| !v["id"].as_str().unwrap_or("").starts_with("implicit."));
    }
    q["query"]["stats"][0]["filters"].as_array().map(|a| a.iter().filter(|v| v.get("disabled").is_none()).count()).unwrap_or(0)
}

/// The item level a cluster jewel notable needs to roll (core/data/cluster_notables.json, from the game's mod data
/// via tools/build_cluster_levels.mjs; checked against poedb).
pub fn notable_ilvl(name: &str) -> Option<u32> {
    static D: std::sync::OnceLock<HashMap<String, u32>> = std::sync::OnceLock::new();
    let d = D.get_or_init(|| serde_json::from_str(include_str!("../data/cluster_notables.json")).unwrap_or_default());
    d.get(&name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>()).copied()
}

/// "Buy base" on a cluster jewel: the base to craft on (Harvest reforge, Alchemy...) instead of buying the finished
/// jewel. Same passive count and small-passive enchant, item level high enough for every notable it has
/// (Inspired Oppression needs 75), any rarity but unique, cheapest first.
/// Returns the search, the item level asked for, and each notable with the level it needs.
pub fn cluster_base_query(text: &str, index: &StatIndex) -> Option<(Value, u32, Vec<(String, u32)>)> {
    let item = parse_item(text)?;
    if !item.base_type.ends_with("Cluster Jewel") { return None; }
    let mut stats: Vec<Value> = Vec::new();
    let mut levels: Vec<(String, u32)> = Vec::new();
    for m in mods(text) {
        if let Some(n) = m.text.strip_prefix("1 Added Passive Skill is ") {
            levels.push((n.to_string(), notable_ilvl(n).unwrap_or(1)));
            continue;
        }
        if m.kind != "enchant" || m.text.contains("Jewel Socket") { continue; }
        let Some((id, value)) = index.find(&m) else { continue };
        let mut v = stat_json(&id);
        if m.text.contains("Passive Skill") && value.is_finite() && v.get("value").is_none() { v["value"] = json!({ "min": value, "max": value }); }
        stats.push(v);
    }
    let ilvl = levels.iter().map(|l| l.1).max().unwrap_or(1);
    let mut filters = json!({ "type_filters": { "filters": { "rarity": { "option": "nonunique" } } } });
    if ilvl > 1 { filters["misc_filters"] = json!({ "filters": { "ilvl": { "min": ilvl } } }); }
    let q = json!({ "query": { "status": { "option": "securable" }, "type": plain_base(&item),
        "stats": [{ "type": "and", "filters": stats }], "filters": filters }, "sort": { "price": "asc" } });
    Some((q, ilvl, levels))
}

/// "Buy" found nothing with every mod: the same search with `drop` fewer of the mods that can vary required
/// (any of them), while what makes the item what it is stays required: enchants (a cluster jewel's passive count
/// and small passives), its jewel sockets, a timeless jewel's seed. Guides list several good notables for a
/// cluster; a jewel has some of them. Returns (search, mods that vary, how many of them it needs); None when
/// there's nothing left to loosen.
pub fn buy_loosen(q: &Value, drop: usize) -> Option<(Value, usize, usize)> {
    let filters = q["query"]["stats"][0]["filters"].as_array()?;
    let fixed = |v: &Value| {
        let id = v["id"].as_str().unwrap_or("");
        id.starts_with("enchant.") || id.contains("pseudo_timeless") || id.ends_with("stat_4079888060")
    };
    let (keep, vary): (Vec<Value>, Vec<Value>) = filters.iter().filter(|v| v.get("disabled").is_none()).cloned().partition(fixed);
    let n = vary.len();
    let need = n.saturating_sub(drop);
    if n < 2 || drop == 0 || need == 0 { return None; }
    let mut groups = vec![json!({ "type": "count", "value": { "min": need }, "filters": vary })];
    if !keep.is_empty() { groups.insert(0, json!({ "type": "and", "filters": keep })); }
    let mut out = q.clone();
    out["query"]["stats"] = json!(groups);
    Some((out, n, need))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cluster_base_asks_for_the_item_level_the_notables_need() {
        let t = "Item Class: Jewels
Rarity: Rare
8 Passive Only
Large Cluster Jewel
--------
Item Level: 84
--------
Adds 8 Passive Skills (enchant)
2 Added Passive Skills are Jewel Sockets (enchant)
Added Small Passive Skills grant: 12% increased Lightning Damage (enchant)
--------
1 Added Passive Skill is Doryani's Lesson
1 Added Passive Skill is Inspired Oppression
1 Added Passive Skill is Overshock
";
        let (q, ilvl, levels) = cluster_base_query(t, &Default::default()).unwrap();
        assert_eq!(ilvl, 75, "Inspired Oppression needs 75: {levels:?}");
        assert_eq!(levels.iter().map(|l| l.1).collect::<Vec<_>>(), [68, 75, 50]);
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["ilvl"]["min"], 75);
        assert_eq!(q["query"]["type"], "Large Cluster Jewel");
        assert!(cluster_base_query(BOOTS, &Default::default()).is_none(), "only cluster jewels");
    }

    #[test]
    fn buy_loosens_cluster_notables() {
        // His large lightning cluster (21 Nov): nobody sold all 3 notables, any 2 found 234.
        let q = json!({ "query": { "type": "Large Cluster Jewel", "stats": [{ "type": "and", "filters": [
            { "id": "enchant.stat_3086156145", "value": { "min": 8, "max": 8 } },
            { "id": "enchant.stat_3948993189", "value": { "option": 14 } },
            { "id": "explicit.stat_228455793" }, { "id": "explicit.stat_3872380586" }, { "id": "explicit.stat_3777170562" },
            { "id": "crafted.stat_1", "disabled": true } ] }] } });
        let (l, of, need) = buy_loosen(&q, 1).unwrap();
        assert_eq!((of, need), (3, 2));
        let groups = l["query"]["stats"].as_array().unwrap();
        assert_eq!(groups[0]["type"], "and");
        assert_eq!(groups[0]["filters"].as_array().unwrap().len(), 2, "passives and the enchant stay required");
        assert_eq!((groups[1]["type"].as_str(), groups[1]["value"]["min"].as_u64()), (Some("count"), Some(2)));
        assert_eq!(buy_loosen(&q, 2).unwrap().2, 1);
        assert!(buy_loosen(&q, 3).is_none(), "can't need 0 of them");
    }

    const BOOTS: &str = "Item Class: Boots
Rarity: Rare
Dusk Spur
Sharkskin Boots
--------
Evasion Rating: 120 (augmented)
--------
Requirements:
Level: 44
Dex: 79
--------
Sockets: G-G B
--------
Item Level: 75
--------
{ Implicit Modifier }
+8% to Cold Resistance (implicit)
--------
{ Prefix Modifier \"Hale\" (Tier: 7) — Life }
+28(25-29) to maximum Life
{ Suffix Modifier \"of the Rainstorm\" (Tier: 4) — Elemental, Cold, Resistance }
+26(24-29)% to Cold Resistance
{ Prefix Modifier \"Cheetah's\" (Tier: 1) — Speed }
30% increased Movement Speed
{ Suffix Modifier \"of the Bear\" (Tier: 5) — Attribute }
+20(18-22) to Strength
--------
";

    fn index() -> StatIndex {
        StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [
                { "id": "explicit.stat_life", "text": "+# to maximum Life", "type": "explicit" },
                { "id": "explicit.stat_cold", "text": "+#% to Cold Resistance", "type": "explicit" },
                { "id": "explicit.stat_ms", "text": "#% increased Movement Speed", "type": "explicit" },
                { "id": "explicit.stat_str", "text": "+# to Strength", "type": "explicit" },
            ]},
            { "label": "Implicit", "entries": [ { "id": "implicit.stat_cold", "text": "+#% to Cold Resistance", "type": "implicit" } ]},
        ]}))
    }

    #[test]
    fn corruption_implicit_counts() {
        // 24 Nov, his corrupted Darkray Vectors: the Vaal implicit must be shown and ticked.
        let text = "Item Class: Boots\nRarity: Unique\nDarkray Vectors\nDragonscale Boots\n--------\nItem Level: 85\n--------\n{ Corruption Implicit Modifier — Caster, Gem, Curse }\n+2 to Level of Socketed Curse Gems\n--------\n{ Unique Modifier — Elemental, Lightning, Resistance }\n+39(20-40)% to Lightning Resistance\n--------\nCorrupted\n";
        let idx = StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [ { "id": "explicit.stat_light", "text": "+#% to Lightning Resistance", "type": "explicit" } ]},
            { "label": "Implicit", "entries": [ { "id": "implicit.stat_curse", "text": "+# to Level of Socketed Curse Gems", "type": "implicit" } ]},
        ]}));
        let s = setup(text, &idx).unwrap();
        let f = s.filters.iter().find(|f| f.id == "implicit.stat_curse").expect("corruption implicit row");
        assert_eq!(f.source.as_deref(), Some("corrupted"));
        assert!(f.enabled && f.min == Some(2.0), "{f:?}");
    }

    #[test]
    fn vessels_by_their_uniques() {
        let t = "Item Class: Map Fragments\r\nRarity: Normal\r\nBlood-filled Vessel\r\n--------\r\nMonsters:\r\nPileah, Corpse Burner\r\nPileah, Burning Corpse\r\n39 Other Monsters\r\nMonster Level: 83\r\nFrom: City Square\r\n--------\r\nRituals have 20% improved Favour Rewards\r\n";
        assert_eq!(vessel(t).unwrap(), (vec!["Pileah, Corpse Burner".to_string(), "Pileah, Burning Corpse".into()], 39));
        assert_eq!(vessel(&t.replace("Monsters:\r\nPileah, Corpse Burner\r\nPileah, Burning Corpse\r\n39 Other Monsters", "Monsters: 50")).unwrap(), (vec![], 50));
        let s = setup(t, &StatIndex::default()).unwrap();
        let on: Vec<(&str, Option<f64>)> = s.filters.iter().filter(|f| f.enabled).map(|f| (f.id.as_str(), f.min)).collect();
        assert_eq!(on, [("pseudo.pseudo_ritual_unique_monsters", Some(2.0))], "{:?}", s.filters);
        let q = query(&s);
        assert_eq!((q["query"]["type"].as_str(), q["query"]["stats"][0]["filters"][0]["value"]["min"].as_f64()), (Some("Blood-filled Vessel"), Some(2.0)));
        // Listings as the site sends them (24 Nov): the ones with both of yours come first.
        let l = |price: f64, names: &str| json!({ "listing": { "price": { "amount": price, "currency": "chaos" }, "account": { "name": format!("a{price}") }, "indexed": "" },
            "item": { "baseType": "Blood-filled Vessel", "properties": [{ "name": "Monsters:\n{0}", "values": [[names, 0]] }, { "name": "{0} Other Monsters", "values": [["38", 0]] }] } });
        let mut ls = listings(&json!({ "result": [l(35.0, "Master of the Blade\nMassier"), l(38.0, "Carius, the Unnatural\nPileah, Corpse Burner\nPileah, Burning Corpse"), l(40.0, "Pileah, Corpse Burner\nPileah, Burning Corpse")] }), &[("chaos".to_string(), 1.0)].into());
        assert_eq!(same_monsters(&mut ls, &vessel(t).unwrap().0), 2);
        assert_eq!(ls.iter().map(|x| (x.amount, x.same)).collect::<Vec<_>>(), [(38.0, true), (40.0, true), (35.0, false)]);
        assert_eq!((ls[0].monsters.len(), ls[0].others), (3, Some(38)));
    }

    #[test]
    fn temple_rooms_are_searched() {
        let t = "Item Class: Misc Map Items\r\nRarity: Currency\r\nChronicle of Atzoatl\r\n--------\r\nArea Level: 80\r\n--------\r\nOpen Rooms:\r\nLocus of Corruption (Tier 3)\r\nHalls\r\n--------\r\nRight click this item to view the Temple of Atzoatl.\r\n";
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Pseudo", "entries": [
            { "id": "pseudo.pseudo_temple_corruption_room_3", "text": "Has Room: Locus of Corruption (Tier 3)", "type": "pseudo", "option": { "options": [{ "id": 1, "text": "Open Room" }] } },
            { "id": "pseudo.pseudo_temple_halls", "text": "Has Room: Halls", "type": "pseudo", "option": { "options": [] } } ]} ]}));
        let mut s = setup(t, &idx).unwrap();
        assert_eq!(s.filters.iter().filter(|f| f.kind == "pseudo").count(), 2, "{:?}", s.filters);
        for f in &mut s.filters { f.enabled = true; }
        let q = query(&s);
        assert_eq!(q["query"]["type"], "Chronicle of Atzoatl");
        let locus = q["query"]["stats"][0]["filters"].as_array().unwrap().iter().find(|f| f["id"] == "pseudo.pseudo_temple_corruption_room_3").unwrap().clone();
        assert_eq!(locus["value"]["option"], 1);
        assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["area_level"]["min"], 75.0, "{q}");
    }

    #[test]
    fn wickerman_body_part() {
        // 24 Nov: 3.30's body parts are copied without a Rarity line, so the price check said "No item was copied".
        let text = "Item Class: Viscera
Blackened Lungs
--------
Torso
--------
Item Level: 77
--------
25% increased Warcry Speed
--------
Take this to the Wickerman in a town or hideout to attach it.
--------
Note: ~b/o 1 alch
";
        let idx = StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [ { "id": "explicit.stat_1316278494", "text": "#% increased Warcry Speed", "type": "explicit" } ]},
            { "label": "Implicit", "entries": [ { "id": "implicit.stat_1316278494", "text": "#% increased Warcry Speed", "type": "implicit" } ]},
        ]}));
        let s = setup(text, &idx).unwrap();
        assert_eq!((s.item.base_type.as_str(), s.item.rarity.as_str(), s.item.item_level), ("Blackened Lungs", "Normal", Some(77)));
        assert_eq!(s.filters.len(), 1, "{:?}", s.filters);
        assert!(s.filters[0].id == "implicit.stat_1316278494" && s.filters[0].enabled && s.filters[0].min == Some(25.0));
        assert_eq!(query(&s)["query"]["type"], "Blackened Lungs");
        // The advanced copy, with the roll's range.
        let adv = text.replace("25% increased", "{ Implicit Modifier — Speed }
25(20-30)% increased");
        assert_eq!(setup(&adv, &idx).unwrap().filters[0].id, "implicit.stat_1316278494");
        assert!(parse_item("just some text I copied").is_none());
    }

    #[test]
    fn reads_advanced_copy() {
        let m = mods(BOOTS);
        assert_eq!(m.len(), 5);
        assert_eq!(m[0], ModLine { text: "+8% to Cold Resistance".into(), kind: "implicit".into(), tier: None, values: vec![8.0], roll: None, range: None, affix: None, width: None, source: None });
        assert_eq!((m[1].text.as_str(), m[1].tier, m[1].values[0]), ("+28 to maximum Life", Some(7), 28.0));
        assert_eq!(m[3].tier, Some(1));
        assert!(m[1].roll.is_some(), "life has a range: {:?}", m[1]);
        assert_eq!(m[1].range.as_deref(), Some("25–29"));
        assert_eq!(range_text("Adds 8(8-12) to 25(18-25) Fire Damage").as_deref(), Some("8–12 / 18–25"));
    }

    #[test]
    fn picks_what_matters() {
        let s = setup(BOOTS, &index()).unwrap();
        let on: Vec<&str> = s.filters.iter().filter(|f| f.enabled).map(|f| f.id.as_str()).collect();
        // Life 28 + 10 from strength = 38 (not enough to tick); elemental res always ticked, like Awakened; T1 movement speed: yes;
        // evasion-only boots: evasion at 20% quality, ticked.
        assert_eq!(on, ["prop.ev", "pseudo.pseudo_total_elemental_resistance", "explicit.stat_ms"]);
        let life = s.filters.iter().find(|f| f.id == "pseudo.pseudo_total_life").unwrap();
        assert_eq!(life.value, 38.0);
        let res = s.filters.iter().find(|f| f.id == "pseudo.pseudo_total_elemental_resistance").unwrap();
        assert_eq!(res.value, 34.0);
        assert!(s.unknown.is_empty());
        assert_eq!(s.category.as_deref(), Some("armour.boots"));
        let q = query(&s);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "armour.boots");
        assert_eq!(q["query"]["stats"][0]["filters"][1]["id"], "explicit.stat_ms");
        assert!(q["query"]["filters"]["armour_filters"]["filters"]["ev"]["min"].is_number());
        assert_eq!(q["query"]["stats"][0]["filters"][1]["value"]["min"], 30.0);
        assert!(q["query"].get("type").is_none());
    }


    #[test]
    fn rare_map_by_tier() {
        let t = concat!("Item Class: Maps\nRarity: Rare\nRemote Grind\nMap (Tier 2)\n--------\n",
            "Atlas Drops: +78% (augmented)\nItem Rarity: +46% (augmented)\nMonster Pack Size: +30% (augmented)\nMore Currency: +12% (augmented)\n",
            "--------\nItem Level: 70\n--------\nMonster Level: 69\n--------\n",
            "{ Prefix Modifier \"Resistant\" (Tier: 1) — Elemental, Chaos, Resistance }\n+15% Monster Chaos Resistance\n+20% Monster Elemental Resistances\n",
            "--------\nTravel to a Map of this tier or lower by using this in a personal Map Device.\n");
        let s = setup(t, &index()).unwrap();
        let on: Vec<&str> = s.filters.iter().filter(|f| f.enabled).map(|f| f.id.as_str()).collect();
        // Like Awakened: drop bonuses ticked, Rarity left off when there's a More Currency bonus; map mods never.
        assert_eq!(on, ["prop.map_bonus", "prop.map_packsize", "pseudo.pseudo_map_more_currency_drops"]);
        let q = query(&s);
        assert_eq!(q["query"]["type"]["option"], "Map");
        assert_eq!(q["query"]["type"]["discriminator"], "map");
        assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["map_tier"]["max"], 2);
        assert_eq!(q["query"]["filters"]["map_filters"]["filters"]["map_bonus"]["min"], 78.0);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"], "nonunique");
        assert_eq!(q["query"]["stats"][0]["filters"][0]["id"], "pseudo.pseudo_map_more_currency_drops");
    }


    #[test]
    fn fractured_mods_keep_their_row() {
        let t = concat!("Item Class: Shields\nRarity: Rare\nVictory Charm\nFossilised Spirit Shield\n--------\nChance to Block: 22%\nEnergy Shield: 65 (augmented)\n",
            "--------\nItem Level: 73\n--------\n",
            "{ Fractured Prefix Modifier \"Chosen\" (Tier: 1) — Life, Defences, Energy Shield }\n27(24-28)% increased Energy Shield\n+19(19-22) to maximum Life\n",
            "{ Prefix Modifier \"Sanguine\" (Tier: 9) — Life }\n+25(25-39) to maximum Life\n",
            "{ Suffix Modifier \"of the Volcano\" (Tier: 3) — Elemental, Fire, Resistance }\n+41(36-41)% to Fire Resistance\n--------\nFractured Item\n");
        let s = setup(t, &index()).unwrap();
        let fr: Vec<&StatFilter> = s.filters.iter().filter(|f| f.kind == "fractured").collect();
        assert!(fr.iter().any(|f| f.label == "+19 to maximum Life"), "{:?}", s.filters);
        assert!(fr.iter().all(|f| !f.enabled), "counted in the life total, so not ticked twice");
    }

    #[test]
    fn rows_in_game_order() {
        // Boots sample: a suffix (Rainstorm) is listed before the prefix (Cheetah's) in the copy; rows come out as
        // defences, totals, then the movement speed prefix.
        let s = setup(BOOTS, &index()).unwrap();
        let ids: Vec<&str> = s.filters.iter().map(|f| f.id.as_str()).collect();
        let pos = |p: &str| ids.iter().position(|i| i.starts_with(p)).unwrap();
        assert!(pos("prop.") < pos("pseudo.") && pos("pseudo.") < pos("explicit.stat_ms"), "{ids:?}");
        let m = mods(BOOTS);
        assert_eq!(m.iter().find(|l| l.text.contains("Movement Speed")).unwrap().affix.as_deref(), Some("prefix"));
        assert_eq!(m.iter().find(|l| l.text.contains("Strength")).unwrap().affix.as_deref(), Some("suffix"));
    }

    #[test]
    fn fated_unidentified_ring() {
        let t = "Item Class: Rings\nRarity: Rare\nFated Composite Ring\n--------\nItem Level: 85\n--------\n{ Implicit Modifier }\n+3 Prefix Modifiers allowed\n--------\nUnidentified (Tier 4)\n";
        let s = setup(t, &index()).unwrap();
        assert!(s.item.unidentified, "Unidentified (Tier 4) counts as unidentified");
        let types = ItemTypes::from_json(&json!({ "result": [ { "entries": [ { "type": "Composite Ring", "text": "Composite Ring" } ] } ] }));
        let mut q = json!({ "query": { "type": "Fated Composite Ring" } });
        types.fix_query(&mut q);
        assert_eq!(q["query"]["type"], "Composite Ring");
        let mut known = json!({ "query": { "type": "Composite Ring" } });
        types.fix_query(&mut known);
        assert_eq!(known["query"]["type"], "Composite Ring");
    }
    #[test]
    fn plain_copy_and_unique() {
        let t = "Item Class: Helmets\nRarity: Unique\nGoldrim\nLeather Cap\n--------\nEvasion Rating: 30\n--------\nItem Level: 40\n--------\n+38% to all Elemental Resistances\n--------\nNo metal slips as easily through the fingers.\n";
        let s = setup(t, &index()).unwrap();
        assert!(s.filters.iter().all(|f| !f.enabled), "uniques search by name");
        let q = query(&s);
        assert_eq!((q["query"]["name"].as_str(), q["query"]["type"].as_str()), (Some("Goldrim"), Some("Leather Cap")));
    }

    #[test]
    fn junk_and_bulk() {
        assert!(is_junk("+# to Accuracy Rating") && !is_junk("+# to maximum Life"));
        let st = StaticIndex::from_json(&json!({ "result": [ { "entries": [ { "id": "divine", "text": "Divine Orb" } ] } ] }));
        assert_eq!(st.id("divine orb").as_deref(), Some("divine"));
        let v = json!({ "result": { "a": { "listing": { "whisper": "@Bob Hi, I'd like to buy your {0} for my {1} in Keepers.",
            "account": { "name": "Bob" }, "offers": [ { "exchange": { "currency": "chaos", "amount": 360, "whisper": "{0} Chaos Orb" },
            "item": { "currency": "divine", "amount": 2, "stock": 10, "whisper": "{0} Divine Orb" } } ] } } } });
        let o = offers(&v);
        assert_eq!((o[0].each, o[0].stock), (180.0, 10));
        assert_eq!(o[0].whisper, "@Bob Hi, I'd like to buy your 2 Divine Orb for my 360 Chaos Orb in Keepers.");
    }

    #[test]
    fn foulborn_two_line_mod() {
        let text = "Item Class: Jewels\nRarity: Unique\nFoulborn The Red Dream\nCrimson Jewel\n--------\nLimited to: 1\nRadius: Large\n--------\nItem Level: 85\n--------\n{ Unique Modifier — Damage, Elemental, Fire, Chaos }\nGain 6(6-10)% of Fire Damage as Extra Chaos Damage\n{ Foulborn Unique Modifier — Life, Elemental, Fire }\nPassives granting Fire Resistance or all Elemental Resistances in Radius\nalso grant increased Maximum Life at 50% of its value\n--------\nWe flow like blood into Chayula's open mouth,\nand spill into the land we have watched forever.\n--------\nPlace into an allocated Jewel Socket on the Passive Skill Tree. Right click to remove from the Socket.\n";
        let m = mods(text);
        assert_eq!(m.len(), 2);
        assert_eq!(m[1].text, "Passives granting Fire Resistance or all Elemental Resistances in Radius also grant increased Maximum Life at 50% of its value");
        assert_eq!(m[1].kind, "mutated");
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Mutated", "entries": [
            { "id": "mutated.stat_rd", "text": "Passives granting Fire Resistance or all Elemental Resistances in Radius\nalso grant increased Maximum Life at #% of its value", "type": "mutated" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        assert!(s.filters.iter().any(|f| f.id == "mutated.stat_rd" && f.kind == "mutated"));
        // Not on the list: still kept, marked Foulborn.
        let s = setup(text, &StatIndex::default()).unwrap();
        assert_eq!(s.unknown_kinds, ["explicit", "mutated"]);
    }

    #[test]
    fn numberless_foulborn_mod() {
        let text = "Item Class: Rings\nRarity: Unique\nFoulborn Lori's Lantern\nPrismatic Ring\n--------\nItem Level: 83\n--------\n{ Implicit Modifier — Elemental, Resistance }\n+9(8-10)% to all Elemental Resistances\n--------\n{ Unique Modifier }\n31% increased Light Radius\n{ Foulborn Unique Modifier — Life }\nYour Damage with Hits is Lucky while on Low Life\n(You are on Low Life if you have 50% of your Maximum Life or less)\n--------\nBy its light, Lori led her young charges to safety\nand bound up their wounds.\n";
        let m = mods(text);
        assert_eq!(m.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
            ["+9% to all Elemental Resistances", "31% increased Light Radius", "Your Damage with Hits is Lucky while on Low Life"]);
        assert_eq!(m[2].kind, "mutated");
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Mutated", "entries": [
            { "id": "mutated.stat_lucky", "text": "Your Damage with Hits is Lucky while on Low Life", "type": "mutated" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        let f = s.filters.iter().find(|f| f.id == "mutated.stat_lucky").unwrap();
        assert_eq!(f.min, None);
    }

    #[test]
    fn foulborn_uniques() {
        let text = "Item Class: Body Armours\nRarity: Unique\nFoulborn Ghostwrithe\nSilken Vest\n--------\nItem Level: 83\n--------\n+90 to maximum Life\nYour Maximum Energy Shield is Equal to 35% of Your Maximum Life\n";
        let idx = StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [ { "id": "explicit.stat_life", "text": "+# to maximum Life", "type": "explicit" } ] },
            { "label": "Mutated", "entries": [ { "id": "mutated.stat_es", "text": "Your Maximum Energy Shield is Equal to #% of Your Maximum Life", "type": "mutated" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        assert!(s.filters.iter().any(|f| f.id == "mutated.stat_es" && f.kind == "mutated"));
        let q = query(&s);
        assert_eq!(q["query"]["name"], "Ghostwrithe");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["mutated"]["option"], "true");
    }

    #[test]
    fn local_only_where_it_fits() {
        // Energy shield on a staff is the global stat; attack speed on gloves too.
        assert!(!local_fits(Some("weapon.staff"), "+137 to maximum Energy Shield"));
        assert!(local_fits(Some("weapon.staff"), "Adds 10 to 20 Physical Damage"));
        assert!(!local_fits(Some("armour.gloves"), "16% increased Attack Speed"));
        assert!(local_fits(Some("armour.gloves"), "+50 to maximum Energy Shield"));
    }

    #[test]
    fn listing_mods_both_shapes() {
        let v = json!({ "result": [ { "listing": { "price": { "amount": 4.0, "currency": "divine" }, "account": { "name": "a" }, "whisper": "@a hi" },
            "item": { "explicitMods": [ { "description": "Has 1 Abyssal Socket", "domain": "explicit" }, "+47 to maximum Energy Shield" ] } } ] });
        let l = listings(&v, &HashMap::new());
        assert_eq!(l[0].mods, ["Has 1 Abyssal Socket", "+47 to maximum Energy Shield"]);
    }

    #[test]
    fn mod_over_several_lines() {
        let text = "Item Class: Gloves\nRarity: Unique\nFoulborn Asenath's Gentle Touch\nSilk Gloves\n--------\nItem Level: 83\n--------\n{ Unique Modifier }\n+61 to maximum Mana\n{ Foulborn Unique Modifier }\nEnemies near corpses affected by your Curses are Blinded\nEnemies Killed near corpses affected by your Curses explode, dealing\n3% of their Life as Physical Damage\n";
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Explicit", "entries": [
            { "id": "explicit.stat_mana", "text": "+# to maximum Mana", "type": "explicit" },
            { "id": "explicit.stat_corpse", "text": "Enemies near corpses affected by your Curses are Blinded\nEnemies Killed near corpses affected by your Curses explode, dealing\n#% of their Life as Physical Damage", "type": "explicit" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        assert!(s.unknown.is_empty(), "{:?} {:?}", s.unknown, mods(text));
        let f = s.filters.iter().find(|f| f.id == "explicit.stat_corpse").unwrap();
        assert_eq!(f.value, 3.0, "{}", f.label);
        assert!(s.filters.iter().any(|f| f.id == "explicit.stat_mana"));
    }

    #[test]
    fn gem_level_and_quality_are_rows() {
        let text = "Item Class: Skill Gems
Rarity: Gem
Mirror Arrow
--------
Attack, Minion
Level: 1
Quality: +23% (augmented)
--------
Requires Level 10
--------
Corrupted
";
        let mut s = setup(text, &StatIndex::default()).unwrap();
        let rows: Vec<(&str, bool)> = s.filters.iter().filter(|f| f.kind == "property").map(|f| (f.label.as_str(), f.enabled)).collect();
        assert_eq!(rows, [("Gem level: 1", true), ("Quality: 23%", true)]);
        assert!(query(&s)["query"]["filters"]["misc_filters"]["filters"]["quality"]["min"] == 23.0, "{}", query(&s));
        s.filters.iter_mut().for_each(|f| f.enabled = false);
        assert!(!query(&s).to_string().contains("quality"), "unticked: any quality");
    }

    #[test]
    fn mercenary_warrant_supports() {
        let text = "Item Class: Map Fragments
Rarity: Normal
Mercenary Warrant
--------
Cicero, the Abyssal
--------
Build: Chaplain of the Well
Mercenary Level: 83
--------
Vaal Smite
Cooldown Recovery (Tier: 2)
Gilded Extra Targets (Tier: 3)
--------
Flame Dash
Cooldown Recovery (Tier: 2)
--------
Right click this item to view Mercenary details.
";
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Mercenary", "entries": [
            { "id": "mercenary.skill_40606", "text": "Vaal Smite", "type": "mercenary" },
            { "id": "mercenary.support_48875", "text": "Cooldown Recovery (Tier 2)", "type": "mercenary" },
            { "id": "mercenary.support_58471", "text": "Gilded Extra Targets (Tier 3)", "type": "mercenary" },
            { "id": "mercenary.support_37259", "text": "Gilded Extra Targets (Tier 3)", "type": "mercenary" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        let rows: Vec<(&str, bool)> = s.filters.iter().filter(|f| f.kind == "mercenary").map(|f| (f.label.as_str(), f.enabled)).collect();
        assert_eq!(rows, [("Vaal Smite", false), ("Cooldown Recovery (Tier 2)", false), ("Gilded Extra Targets (Tier 3)", true)]);
        let q = query(&s).to_string();
        assert!(q.contains("count") && q.contains("support_58471") && q.contains("support_37259"), "{q}");
    }

    #[test]
    fn map_kind_and_enchants_are_ticked() {
        // 26 Nov: his Originator map's second enchant was dropped, and Elder / Originator lines came unticked.
        let text = "Item Class: Maps\nRarity: Rare\nCryptic Inscription\nMap (Tier 16)\n--------\nItem Level: 85\n--------\nMonster Level: 83\n--------\nArea contains 8 additional Shrines (enchant)\nArea is dominated by Terror Shrines (enchant)\n--------\n{ Implicit Modifier }\nArea is Influenced by the Originator's Memories — Unscalable Value\n--------\n{ Suffix Modifier \"of Endurance\" }\nMonsters gain an Endurance Charge on Hit\n";
        let idx = StatIndex::from_json(&json!({ "result": [
            { "label": "Implicit", "entries": [ { "id": "implicit.stat_2696470877", "text": "Area is Influenced by the Originator's Memories", "type": "implicit" } ] },
            { "label": "Enchant", "entries": [ { "id": "enchant.stat_667284445", "text": "Area is dominated by Terror Shrines", "type": "enchant" } ] },
            { "label": "Explicit", "entries": [ { "id": "explicit.stat_687813731", "text": "Monsters gain an Endurance Charge on Hit", "type": "explicit" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        let on = |id: &str| s.filters.iter().find(|f| f.id == id).map(|f| f.enabled);
        assert_eq!(on("enchant.stat_667284445"), Some(true));
        assert_eq!(on("implicit.stat_2696470877"), Some(true));
        assert_eq!(on("explicit.stat_687813731"), Some(false), "ordinary map mods stay unticked");
    }

    #[test]
    fn map_mods_that_always_happen() {
        let text = "Item Class: Maps\nRarity: Rare\nFoe Nexus\nCrimson Township Map\n--------\nMap Tier: 16\n--------\nItem Level: 83\n--------\n{ Prefix Modifier \"Hindering\" (Tier: 1) }\nMonsters Hinder on Hit with Spells\n{ Suffix Modifier \"of Withering\" (Tier: 1) }\nMonsters Inflict Withered for 2 seconds on Hit\n";
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Explicit", "entries": [
            { "id": "explicit.stat_hinder", "text": "Monsters have #% chance to Hinder on Hit with Spells", "type": "explicit" },
            { "id": "explicit.stat_wither", "text": "Monsters have #% chance to inflict Withered for 2 seconds on Hit", "type": "explicit" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        assert!(s.unknown.is_empty(), "{:?} {:?}", s.unknown, mods(text));
        assert!(s.filters.iter().any(|f| f.id == "explicit.stat_hinder"), "{:?}", s.filters.iter().map(|f| (&f.id, &f.label)).collect::<Vec<_>>());
        assert!(s.filters.iter().any(|f| f.id == "explicit.stat_wither"));
    }

    #[test]
    fn transfigured_gems() {
        let items = json!({ "result": [{ "label": "Gems", "entries": [
            { "type": "Scourge Arrow", "text": "Scourge Arrow" },
            { "type": "Scourge Arrow", "text": "Scourge Arrow of Menace", "disc": "alt_x" } ] }] });
        let types = ItemTypes::from_json(&items);
        let mut q = json!({ "query": { "type": "Scourge Arrow of Menace" } });
        types.fix_query(&mut q);
        assert_eq!(q["query"]["type"], json!({ "option": "Scourge Arrow", "discriminator": "alt_x" }));
        let mut q = json!({ "query": { "type": "Scourge Arrow" } });
        types.fix_query(&mut q);
        assert_eq!(q["query"]["type"], "Scourge Arrow");
    }

    #[test]
    fn templates_and_listings() {
        assert_eq!(template("Adds 10 to 20 Physical Damage"), ("Adds # to # Physical Damage".into(), vec![10.0, 20.0]));
        assert_eq!(clean("+28(25-29) to maximum Life"), "+28 to maximum Life");
        assert_eq!(roll_of("+28(25-29) to maximum Life", &[28.0]), Some(0.75));
        assert_eq!(roll_of("Adds 8(8-12) to 25(18-25) Fire Damage", &[8.0, 25.0]), Some(0.5));
        assert_eq!(roll_of("+1 to Level of all Spell Skill Gems", &[1.0]), None);
        assert_eq!(roll_of("+25(25-25)% Chance to Block", &[25.0]), None);
        // Negative ranges: further from zero counts as higher.
        assert_eq!(roll_of("-10(-15--5)% to Chaos Resistance", &[-10.0]), Some(0.5));
        assert_eq!(roll_of("29(30-25)% reduced Effect of Curses on you", &[29.0]), Some(0.8));
        assert_eq!(range_text("29(30-25)% reduced Effect of Curses on you").as_deref(), Some("25–30"));
        assert_eq!(clean("+10% to Cold Resistance (implicit)"), "+10% to Cold Resistance");
        let r = rates(|n| (n == "Divine Orb").then_some(200.0));
        let f = json!({ "result": [
            { "listing": { "price": { "amount": 1.0, "currency": "divine" }, "account": { "name": "a" }, "whisper": "@a hi", "indexed": "2026-11-18T00:00:00Z" }, "item": { "ilvl": 80 } },
            { "listing": { "price": { "amount": 50.0, "currency": "chaos" }, "account": { "name": "b" }, "whisper": "", "indexed": "" }, "item": {} },
        ]});
        let ls = listings(&f, &r);
        assert_eq!(ls[0].chaos, Some(200.0));
        assert_eq!(summary(&ls), Some((50.0, 200.0)));
        // One price fixer listing six cheap copies counts once: the typical price comes from the real sellers.
        let l = |acc: &str, c: f64| Listing { amount: c, currency: "chaos".into(), chaos: Some(c), account: acc.into(), indexed: String::new(),
            whisper: String::new(), ilvl: None, instant: false, mods: vec![], monsters: vec![], others: None, same: false };
        let mut fixed: Vec<Listing> = (0..6).map(|_| l("fixer", 0.1)).collect();
        fixed.extend([l("a", 3.0), l("b", 3.0), l("c", 4.0), l("d", 5.0)]);
        assert_eq!(summary(&fixed), Some((0.1, 3.0)));
        // His Xirgil's Crank (23 Nov): instant 60, 60, 70, 74, 75, 90… sells for about 70, not the middle (90).
        let crank: Vec<Listing> = [60.0, 60.0, 70.0, 74.0, 75.0, 90.0, 100.0, 145.0, 150.0].iter().enumerate().map(|(i, c)| l(&format!("s{i}"), *c)).collect();
        assert_eq!(summary(&crank), Some((60.0, 70.0)));
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    #[test]
    fn eldritch_essence_and_delve_sources() {
        let item = "Item Class: Boots\nRarity: Rare\nFoe Urge\nCrusader Boots\n--------\nItem Level: 85\n--------\n\
{ Searing Exarch Implicit Modifier (Lesser) }\n5% chance to Block Attack Damage\n--------\n\
{ Prefix Modifier \"Subterranean\" (Tier: 1) }\n+86(80-89) to maximum Life\n\
{ Suffix Modifier \"of the Essence\" (Tier: 1) }\n16(14-16)% increased Stun and Block Recovery\n\
{ Suffix Modifier \"of the Lightning\" (Tier: 2) }\n+42(36-41)% to Lightning Resistance\n";
        let m = mods(item);
        let src = |t: &str| m.iter().find(|x| x.text.contains(t)).and_then(|x| x.source.clone());
        assert_eq!(src("Block Attack").as_deref(), Some("exarch"));
        assert_eq!(src("maximum Life").as_deref(), Some("delve"));
        assert_eq!(src("Stun and Block").as_deref(), Some("essence"));
        assert_eq!(src("Lightning Resistance"), None);
    }
}
