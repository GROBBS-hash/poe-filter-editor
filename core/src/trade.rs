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
}

/// One stat to search on, with a tick box and a minimum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}

impl StatIndex {
    pub fn from_json(v: &Value) -> StatIndex {
        let mut by_text = HashMap::new();
        for group in v["result"].as_array().into_iter().flatten() {
            for e in group["entries"].as_array().into_iter().flatten() {
                let (Some(id), Some(text), Some(kind)) = (e["id"].as_str(), e["text"].as_str(), e["type"].as_str()) else { continue };
                // Stats with a fixed option list ("Allocates #") are searched differently: skip.
                if e.get("option").is_some() { continue; }
                by_text.entry((kind.to_string(), text.replace('\n', " ").to_lowercase())).or_insert_with(|| id.to_string());
            }
        }
        StatIndex { by_text }
    }
    pub fn len(&self) -> usize { self.by_text.len() }
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

/// The ranges as shown in the window: "25–29", or "8–12 / 18–25" for several numbers.
/// None when there are none or every number is fixed.
fn range_text(line: &str) -> Option<String> {
    let r = ranges(line);
    if r.is_empty() || r.iter().all(|(a, b)| a == b) { return None; }
    Some(r.iter().map(|(a, b)| { let (lo, hi) = if a.abs() <= b.abs() { (a, b) } else { (b, a) }; format!("{lo}–{hi}") }).collect::<Vec<_>>().join(" / "))
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
    for sec in &sections[start + 1..] {
        if sec.iter().any(|l| l.starts_with("Note:") || *l == "Corrupted" || *l == "Unidentified" || *l == "Mirrored"
            || l.starts_with("Place into") || l.starts_with("Right click") || l.starts_with("Travel to")) {
            continue;
        }
        let mut header_kind: Option<String> = None;
        let mut header_tier: Option<u32> = None;
        let mut header_affix: Option<String> = None;
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
                header_affix = if low.contains("prefix") { Some("prefix".into()) } else if low.contains("suffix") { Some("suffix".into()) } else { None };
                header_tier = low.find("tier:").or_else(|| low.find("rank:")).and_then(|i| {
                    low[i + 5..].trim_start().chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
                });
                continue;
            }
            if l.starts_with('(') { continue; } // reminder text
            let kind = if l.ends_with("(implicit)") { "implicit".to_string() } else if l.ends_with("(crafted)") { "crafted".into() }
                else if l.ends_with("(enchant)") { "enchant".into() } else if l.ends_with("(fractured)") { "fractured".into() }
                else if l.ends_with("(mutated)") { "mutated".into() }
                else { header_kind.clone().unwrap_or_else(|| "explicit".into()) };
            let t = clean(l);
            // Under a "{ … Modifier }" heading every line is a mod, numbers or not ("…is Lucky while on Low Life").
            let numberless_ok = header_kind.is_some() || t.contains("Cannot") || t.contains("Has ");
            if t.is_empty() || !t.chars().any(|c| c.is_ascii_digit()) && !numberless_ok { continue; }
            let (_, values) = template(&t);
            let (roll, range) = (roll_of(l, &values), range_text(l));
            out.push(ModLine { text: t, kind, tier: header_tier, values, roll, range, affix: header_affix.clone() });
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
        _ => return None,
    })
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
    let local_first = category(&item.item_class).is_some_and(|c| c.starts_with("armour.") || c.starts_with("weapon."));
    for (i, m) in lines.iter().enumerate() {
        if hidden.contains(&i) { continue; }
        // Foulborn uniques' special mods are on the site's "mutated" list.
        // Armour and weapons: "+# to Armour" on the item itself is the site's "+# to Armour (Local)", not the global one
        // (that's for belts and jewels); try the local one first.
        let local = || index.find(&ModLine { text: format!("{} (Local)", m.text), ..m.clone() }).filter(|_| local_first);
        // Foulborn: its mutated mod first, then like any armour/weapon (the local version, 21 Nov: Foulborn Ghostwrithe's
        // "+183 to maximum Energy Shield" went to the global stat and found nothing).
        let found = if foulborn { index.find(&ModLine { kind: "mutated".into(), ..m.clone() }).filter(|(id, _)| id.starts_with("mutated.")).or_else(local).or_else(|| index.find(m)) } else { local().or_else(|| index.find(m)) };
        // "Curse Enemies with Punishment on Hit" is the site's "#% chance to Curse Enemies with Punishment on Hit" at 100%
        // (21 Nov: Foulborn Asenath's Gentle Touch showed it as not searchable).
        let found = found.or_else(|| m.text.starts_with("Curse Enemies with ").then(|| index.find(&ModLine {
            text: format!("100% chance to {}", m.text), values: vec![100.0], ..m.clone() })).flatten());
        let Some((id, value)) = found else { unknown.push(m.text.clone()); unknown_kinds.push(m.kind.clone()); continue };
        // Found on the mutated list = a Foulborn mod (shown in its own colour).
        let kind = if id.starts_with("mutated.") { "mutated".to_string() } else { m.kind.clone() };
        let (t, _) = template(&m.text);
        let good = if has_tiers { m.tier.map(|t| t <= 2).unwrap_or(false) } else { VALUABLE.iter().any(|k| t.contains(k)) };
        let junk = is_junk(&t);
        // Maps: price by tier and rarity; mods are there to tick, never ticked for you.
        let enabled = !unique && !is_map(&item) && m.kind != "enchant" && good && !junk && !counted.contains(&i);
        // A mod with no number (or listed word for word, value NaN) is just "has it": no minimum.
        let whole = value.is_nan();
        let value = if whole { 0.0 } else { value };
        let min = if m.values.is_empty() || whole { None } else { Some(min_for(value)) };
        filters.push(StatFilter { id, label: m.text.clone(), value, min, enabled, kind, tier: m.tier, junk, roll: m.roll, range: m.range.clone(), affix: m.affix.clone() });
    }
    if is_map(&item) { filters.splice(0..0, crate::trade_rules::map_props(text, &item)); }
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
                tier: None, junk: false, roll: None, range: None, affix: None,
            });
        }
    }
    else { filters.extend(crate::trade_rules::open_affixes(text, &item, index)); }
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
    let base_item = base_item(text, &item, &lines, &filters);
    let base = plain_base(&item);
    let affix_max = Some(crate::trade_rules::affix_limits(text, &item));
    Some(TradeSetup {
        category: category(&item.item_class).map(|c| if item.base_type.ends_with("Cluster Jewel") { "jewel.cluster" } else { c }).map(String::from),
        base_only: !matches!(item.rarity.as_str(), "Rare" | "Magic") || !gear || nothing_picked,
        ilvl_min: (gear && nothing_picked && item.rarity != "Unique").then_some(item.item_level).flatten(),
        ilvl_max: (gear && nothing_picked && item.rarity != "Unique").then_some(item.item_level).flatten().and_then(|l| ilvl_band(l).1),
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
    if !(magic || item.rarity == "Rare") || item.unidentified || cat == "flask" { return None; }
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
        "explicit" => magic && !cluster && f.tier.is_some_and(|t| t <= 2),
        _ => false,
    }).map(|f| f.id.clone()).collect();
    // Item level: at least this one (most mod tiers need 68-86). Cluster jewels by bracket; other jewels don't care.
    let ilvl_min = if cluster { [84, 75, 68, 50, 1].into_iter().find(|b| ilvl >= *b) } else if jewel { None } else { Some(ilvl.min(86)) };
    let mut misc: Vec<String> = item.influences.iter().map(|i| format!("{}_item", i.to_lowercase())).collect();
    if synthesised { misc.push("synthesised_item".into()); }
    if fractured { misc.push("fractured_item".into()); }
    Some(BaseItem { why, ids, ilvl_min: ilvl_min.filter(|l| *l > 0), misc })
}

fn is_map(it: &Item) -> bool { it.item_class == "Maps" }

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
    if let Some(n) = s.gem_level { misc["gem_level"] = json!({ "min": n }); }
    if foulborn { misc["mutated"] = json!({ "option": "true" }); }
    for k in &s.misc { misc[k.as_str()] = json!({ "option": "true" }); }
    if it.item_class.contains("Gem") {
        if let Some(qual) = it.quality { misc["quality"] = json!({ "min": qual }); }
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
        if let Some(ids) = f.id.strip_prefix("timeless_any:") {
            let v = f.min.unwrap_or(f.value);
            groups.push(json!({ "type": "count", "value": { "min": 1 },
                "filters": ids.split(',').map(|id| json!({ "id": id, "value": seed(v) })).collect::<Vec<_>>() }));
        }
    }
    let stats: Vec<Value> = s.filters.iter().filter(|f| f.enabled && !f.id.starts_with("prop.") && !f.id.starts_with("timeless_any:")).map(|f| {
        let id = match f.id.strip_prefix("fractured.") { Some(rest) if !as_base => format!("explicit.{rest}"), _ => f.id.clone() };
        let mut v = stat_json(&id);
        if let (Some(m), None) = (f.min, v.get("value")) { v["value"] = json!({ "min": m }); }
        if f.id.starts_with("explicit.pseudo_timeless_jewel_") { v["value"] = seed(f.min.unwrap_or(f.value)); }
        v
    }).collect();
    q["stats"] = json!([json!({ "type": "and", "filters": stats })].into_iter().chain(groups).collect::<Vec<_>>());
    json!({ "query": q, "sort": { "price": "asc" } })
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
}

/// Reads the trade site's fetch answer.
pub fn listings(fetched: &Value, rates: &HashMap<String, f64>) -> Vec<Listing> {
    fetched["result"].as_array().into_iter().flatten().filter_map(|r| {
        let l = &r["listing"];
        let p = &l["price"];
        let amount = p["amount"].as_f64()?;
        let currency = p["currency"].as_str()?.to_string();
        Some(Listing {
            chaos: rates.get(&currency).map(|r| r * amount),
            amount, currency,
            account: l["account"]["lastCharacterName"].as_str().or(l["account"]["name"].as_str()).unwrap_or("").to_string(),
            indexed: l["indexed"].as_str().unwrap_or("").to_string(),
            whisper: l["whisper"].as_str().unwrap_or("").to_string(),
            ilvl: r["item"]["ilvl"].as_u64().map(|v| v as u32),
            instant: l.get("fee").is_some() || l["whisper"].as_str().map(str::is_empty).unwrap_or(true),
        })
    }).collect()
}

/// What the cheapest listings say it's worth: (lowest, typical) in chaos.
pub fn summary(ls: &[Listing]) -> Option<(f64, f64)> {
    // Price fixers list a few very cheap copies (often the same seller many times) to drag the price down.
    // Count each seller once, at their cheapest, then take the middle of the cheapest ten sellers.
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
    let first = &v[..v.len().min(10)];
    Some((v[0], first[first.len() / 2]))
}

/// Trade site currency ids -> names poe.ninja uses.
pub const CURRENCY_NAMES: &[(&str, &str)] = &[
    ("divine", "Divine Orb"), ("exalted", "Exalted Orb"), ("alch", "Orb of Alchemy"), ("fusing", "Orb of Fusing"),
    ("alt", "Orb of Alteration"), ("jewellers", "Jeweller's Orb"), ("chrome", "Chromatic Orb"), ("chance", "Orb of Chance"),
    ("regal", "Regal Orb"), ("vaal", "Vaal Orb"), ("scour", "Orb of Scouring"), ("gcp", "Gemcutter's Prism"),
    ("mirror", "Mirror of Kalandra"), ("annul", "Orb of Annulment"), ("regret", "Orb of Regret"), ("blessed", "Blessed Orb"),
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
    let cat = category(&item.item_class);
    let local_first = cat.is_some_and(|c| c.starts_with("armour.") || c.starts_with("weapon."));
    let mut stats: Vec<Value> = Vec::new();
    for m in mods(text) {
        let local = || index.find(&ModLine { text: format!("{} (Local)", m.text), ..m.clone() }).filter(|_| local_first);
        // A cluster jewel's "2 Added Passive Skills are Jewel Sockets": comes with its passive count, and asking for it
        // (as enchant or explicit, any number) finds nothing on the trade site (tested 21 Nov, his large lightning
        // cluster: 0 with it, 234 without). Leave it out.
        if m.text.contains("Jewel Socket") && item.base_type.ends_with("Cluster Jewel") { continue; }
        let Some((id, value)) = local().or_else(|| index.find(&m)) else { continue };
        // A unique's fixed mods come with the name; only mods that differ between copies are worth asking for
        // (Watcher's Eye auras, a timeless jewel's seed).
        let timeless = id.starts_with("explicit.pseudo_timeless_jewel_");
        if unique && !m.text.contains("while affected by") && !timeless { continue; }
        if stats.iter().any(|v| v["id"] == id.as_str()) { continue; }
        let mut v = stat_json(&id);
        // Crafted mods: you can craft them yourself, so they're on the page but not required.
        if m.kind == "crafted" { v["disabled"] = json!(true); }
        // "Adds 8 Passive Skills" / "2 Added Passive Skills are Jewel Sockets": that number is the jewel.
        if (timeless || m.kind == "enchant" && m.text.contains("Passive Skill")) && value.is_finite() && v.get("value").is_none() { v["value"] = json!({ "min": value, "max": value }); }
        stats.push(v);
    }
    // The base as the trade site knows it: magic items only have a full name ("Perpetual Sapphire Flask of Craft").
    let base = plain_base(&item);
    let mut q = json!({ "status": { "option": "available" }, "type": base, "stats": [{ "type": "and", "filters": stats }] });
    if unique {
        if let Some(n) = &item.name { q["name"] = json!(n.strip_prefix("Foulborn ").unwrap_or(n)); }
    } else if cat.is_some() {
        let rarity = match item.rarity.as_str() { "Rare" => "rare", "Magic" => "magic", "Normal" => "normal", _ => "nonunique" };
        q["filters"] = json!({ "type_filters": { "filters": { "rarity": { "option": rarity } } } });
    }
    let n = q["stats"][0]["filters"].as_array().map(|a| a.iter().filter(|v| v.get("disabled").is_none()).count()).unwrap_or(0);
    Some((json!({ "query": q, "sort": { "price": "asc" } }), n))
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
    let q = json!({ "query": { "status": { "option": "available" }, "type": plain_base(&item),
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
    fn reads_advanced_copy() {
        let m = mods(BOOTS);
        assert_eq!(m.len(), 5);
        assert_eq!(m[0], ModLine { text: "+8% to Cold Resistance".into(), kind: "implicit".into(), tier: None, values: vec![8.0], roll: None, range: None, affix: None });
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
            whisper: String::new(), ilvl: None, instant: false };
        let mut fixed: Vec<Listing> = (0..6).map(|_| l("fixer", 0.1)).collect();
        fixed.extend([l("a", 3.0), l("b", 3.0), l("c", 4.0), l("d", 5.0)]);
        assert_eq!(summary(&fixed), Some((0.1, 3.0)));
    }
}
