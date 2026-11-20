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
    pub links_min: Option<u32>,
    pub gem_level: Option<u32>,
    /// Mods the trade site's list doesn't know (shown, not searched).
    pub unknown: Vec<String>,
    /// Their kinds ("mutated" = Foulborn…), same order, for colouring.
    #[serde(default)]
    pub unknown_kinds: Vec<String>,
    /// Trade site category for the item class ("armour.helmet").
    pub category: Option<String>,
    /// The copied item text (for poeprices.info).
    #[serde(default)]
    pub raw: String,
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

    /// The trade id for a mod line, and the value to search with. Tries the
    /// mod's own kind first, then explicit; "reduced" is searched as negative "increased".
    pub fn find(&self, m: &ModLine) -> Option<(String, f64)> {
        let (t, vals) = template(&m.text);
        let value = if vals.len() == 2 && t.to_lowercase().contains("# to #") { (vals[0] + vals[1]) / 2.0 } else { vals.first().copied().unwrap_or(0.0) };
        let kinds = [m.kind.as_str(), "explicit"];
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
        }
        None
    }
}

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
            let l = l.as_str();
            if l.starts_with('{') {
                // { Prefix Modifier "Hale" (Tier: 7) — Life }
                let low = l.to_lowercase();
                header_kind = Some(if low.contains("implicit") { "implicit" } else if low.contains("crafted") { "crafted" }
                    else if low.contains("enchant") { "enchant" } else if low.contains("fractured") { "fractured" }
                    else if low.contains("foulborn") || low.contains("mutated") { "mutated" } else { "explicit" }.to_string());
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
            out.push(ModLine { text: t, kind, tier: header_tier, values, roll, range });
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
    for (i, m) in lines.iter().enumerate() {
        if hidden.contains(&i) { continue; }
        // Foulborn uniques' special mods are on the site's "mutated" list.
        let found = if foulborn { index.find(&ModLine { kind: "mutated".into(), ..m.clone() }).or_else(|| index.find(m)) } else { index.find(m) };
        let Some((id, value)) = found else { unknown.push(m.text.clone()); unknown_kinds.push(m.kind.clone()); continue };
        // Found on the mutated list = a Foulborn mod (shown in its own colour).
        let kind = if id.starts_with("mutated.") { "mutated".to_string() } else { m.kind.clone() };
        let (t, _) = template(&m.text);
        let good = if has_tiers { m.tier.map(|t| t <= 2).unwrap_or(false) } else { VALUABLE.iter().any(|k| t.contains(k)) };
        let junk = is_junk(&t);
        // Maps: price by tier and rarity; mods are there to tick, never ticked for you.
        let enabled = !unique && !is_map(&item) && m.kind != "enchant" && good && !junk && !counted.contains(&i);
        // A mod with no number is just "has it": no minimum.
        let min = if m.values.is_empty() { None } else { Some(min_for(value)) };
        filters.push(StatFilter { id, label: m.text.clone(), value, min, enabled, kind, tier: m.tier, junk, roll: m.roll, range: m.range.clone() });
    }
    if is_map(&item) { filters.splice(0..0, crate::trade_rules::map_props(text, &item)); }
    else { filters.extend(crate::trade_rules::open_affixes(text, &item, index)); }
    let links = item.sockets.as_deref().map(crate::item::max_links).unwrap_or(0);
    let gem_level = text.replace('\r', "").split("--------").nth(1)
        .and_then(|s| s.lines().find_map(|l| l.trim().strip_prefix("Level: ")))
        .and_then(|v| v.split_whitespace().next()?.parse().ok())
        .filter(|_| item.item_class.contains("Gem"));
    // Nothing worth searching on (white/unidentified items, or only weak mods): it's
    // the base that's for sale, so search that base at this item level or better.
    let nothing_picked = !filters.iter().any(|f| f.enabled);
    let gear = category(&item.item_class).is_some();
    Some(TradeSetup {
        category: category(&item.item_class).map(String::from),
        base_only: !matches!(item.rarity.as_str(), "Rare" | "Magic") || !gear || nothing_picked,
        ilvl_min: (gear && nothing_picked && item.rarity != "Unique").then_some(item.item_level).flatten(),
        links_min: (links >= 5).then_some(links),
        gem_level,
        filters, unknown, unknown_kinds, item, raw: text.to_string(),
    })
}

fn is_map(it: &Item) -> bool { it.item_class == "Maps" }

/// The map's tier: from "Map Tier: 16", or from the name "Map (Tier 2)".
fn map_tier_of(it: &Item) -> Option<u32> {
    it.map_tier.or_else(|| it.base_type.split("(Tier ").nth(1)?.trim_end_matches(')').trim().parse().ok())
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
        q["type"] = json!(it.base_type);
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
    if it.unidentified && it.rarity != "Unique" { misc["identified"] = json!({ "option": "false" }); }
    if let Some(n) = s.gem_level { misc["gem_level"] = json!({ "min": n }); }
    if foulborn { misc["mutated"] = json!({ "option": "true" }); }
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
    let stats: Vec<Value> = s.filters.iter().filter(|f| f.enabled && !f.id.starts_with("prop.")).map(|f| {
        let mut v = json!({ "id": f.id });
        if let Some(m) = f.min { v["value"] = json!({ "min": m }); }
        v
    }).collect();
    q["stats"] = json!([{ "type": "and", "filters": stats }]);
    json!({ "query": q, "sort": { "price": "asc" } })
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
    let mut v: Vec<f64> = ls.iter().filter_map(|l| l.chaos).collect();
    if v.is_empty() { return None; }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    // Skip the odd too-cheap listing (price fixers): typical = middle of the cheapest ten.
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
pub struct ItemTypes { by_text: HashMap<String, (String, String)> }

impl ItemTypes {
    pub fn from_json(v: &Value) -> ItemTypes {
        let mut by_text = HashMap::new();
        for group in v["result"].as_array().into_iter().flatten() {
            for e in group["entries"].as_array().into_iter().flatten() {
                if let (Some(t), Some(text), Some(disc)) = (e["type"].as_str(), e["text"].as_str(), e["disc"].as_str()) {
                    if e.get("name").is_none() { by_text.entry(text.to_lowercase()).or_insert_with(|| (t.to_string(), disc.to_string())); }
                }
            }
        }
        ItemTypes { by_text }
    }

    /// Rewrites a search's plain type ("Scourge Arrow of Menace") into the form the
    /// trade site wants ({option: "Scourge Arrow", discriminator: "alt_x"}).
    pub fn fix_query(&self, q: &mut Value) {
        let Some(t) = q["query"]["type"].as_str() else { return };
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

// ---------- poeprices.info (rough price for rares) ----------

/// poeprices.info's estimate: (min, max, currency, confidence %).
pub fn poeprices(v: &Value) -> Option<(f64, f64, String, f64)> {
    if v["error"].as_i64().unwrap_or(0) != 0 { return None; }
    Some((v["min"].as_f64()?, v["max"].as_f64()?, v["currency"].as_str().unwrap_or("chaos").to_string(),
        v["pred_confidence_score"].as_f64().unwrap_or(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(m[0], ModLine { text: "+8% to Cold Resistance".into(), kind: "implicit".into(), tier: None, values: vec![8.0], roll: None, range: None });
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
        // Life 28 + 10 from strength = 38 (not enough to tick); res 34 (no); T1 movement speed: yes;
        // evasion-only boots: evasion at 20% quality, ticked.
        assert_eq!(on, ["prop.ev", "explicit.stat_ms"]);
        let life = s.filters.iter().find(|f| f.id == "pseudo.pseudo_total_life").unwrap();
        assert_eq!(life.value, 38.0);
        let res = s.filters.iter().find(|f| f.id == "pseudo.pseudo_total_elemental_resistance").unwrap();
        assert_eq!(res.value, 34.0);
        assert!(s.unknown.is_empty());
        assert_eq!(s.category.as_deref(), Some("armour.boots"));
        let q = query(&s);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "armour.boots");
        assert_eq!(q["query"]["stats"][0]["filters"][0]["id"], "explicit.stat_ms");
        assert!(q["query"]["filters"]["armour_filters"]["filters"]["ev"]["min"].is_number());
        assert_eq!(q["query"]["stats"][0]["filters"][0]["value"]["min"], 30.0);
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
    }
}
