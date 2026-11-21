//! Smarter price-check stats (ideas from Awakened PoE Trade, written fresh here):
//! - totals ("pseudo" stats) that add up every source of a stat, e.g. all
//!   resistances, life from Strength, and hide the mods they already count;
//! - weapon DPS and armour / evasion / energy shield at 20% quality;
//! - "has an open prefix / suffix" for crafting bases.

use crate::item::Item;
use crate::trade::{template, ModLine, StatFilter, StatIndex};

/// Where a total's numbers come from: the mod's template (lower case), how much
/// each point counts, and whether the mod stays listed on its own (it also gives
/// something else, like "+#% to Fire and Chaos Resistances").
struct Source { t: &'static str, times: f64, keep: bool }

const fn src(t: &'static str, times: f64, keep: bool) -> Source { Source { t, times, keep } }

struct Rule {
    /// The total's text on the trade site, and its id if the stat list isn't loaded.
    text: &'static str,
    fallback_id: Option<&'static str>,
    sources: &'static [Source],
    /// This mod must be on the item for the total to mean anything (life needs a life mod).
    required: Option<&'static str>,
    /// Only worth a total when two or more mods add to it (otherwise the mod itself is the search).
    needs_two: bool,
    /// Ticked for you on rares when the total reaches this.
    tick_at: Option<f64>,
}

// Resistances like Awakened PoE Trade (pseudo/index.ts): elemental total always ticked, chaos total ticked (dropped
// when its only source is a crafted chaos mod), each element's own total there to tick. On rares every resistance mod
// is folded into these totals (their `keep` is false); uniques keep their mods anyway.
const ELE_RES: &[Source] = &[
    src("+#% to all resistances", 3.0, false), src("+#% to all elemental resistances", 3.0, false),
    src("+#% to fire resistance", 1.0, false), src("+#% to cold resistance", 1.0, false), src("+#% to lightning resistance", 1.0, false),
    src("+#% to fire and lightning resistances", 2.0, false), src("+#% to fire and cold resistances", 2.0, false),
    src("+#% to cold and lightning resistances", 2.0, false),
    src("+#% to fire and chaos resistances", 1.0, false), src("+#% to cold and chaos resistances", 1.0, false),
    src("+#% to lightning and chaos resistances", 1.0, false),
];
const FIRE_RES: &[Source] = &[
    src("+#% to all resistances", 1.0, false), src("+#% to all elemental resistances", 1.0, false), src("+#% to fire resistance", 1.0, false),
    src("+#% to fire and lightning resistances", 1.0, false), src("+#% to fire and cold resistances", 1.0, false),
    src("+#% to fire and chaos resistances", 1.0, false),
];
const COLD_RES: &[Source] = &[
    src("+#% to all resistances", 1.0, false), src("+#% to all elemental resistances", 1.0, false), src("+#% to cold resistance", 1.0, false),
    src("+#% to fire and cold resistances", 1.0, false), src("+#% to cold and lightning resistances", 1.0, false),
    src("+#% to cold and chaos resistances", 1.0, false),
];
const LIGHTNING_RES: &[Source] = &[
    src("+#% to all resistances", 1.0, false), src("+#% to all elemental resistances", 1.0, false), src("+#% to lightning resistance", 1.0, false),
    src("+#% to fire and lightning resistances", 1.0, false), src("+#% to cold and lightning resistances", 1.0, false),
    src("+#% to lightning and chaos resistances", 1.0, false),
];
const CHAOS_RES: &[Source] = &[
    src("+#% to chaos resistance", 1.0, false), src("+#% to all resistances", 1.0, false),
    src("+#% to fire and chaos resistances", 1.0, false), src("+#% to cold and chaos resistances", 1.0, false),
    src("+#% to lightning and chaos resistances", 1.0, false),
];
const LIFE: &[Source] = &[
    src("+# to maximum life", 1.0, false), src("+# to strength", 0.5, true), src("+# to all attributes", 0.5, true),
    src("+# to strength and intelligence", 0.5, true), src("+# to strength and dexterity", 0.5, true),
];
const MANA: &[Source] = &[
    src("+# to maximum mana", 1.0, false), src("+# to intelligence", 0.5, true), src("+# to all attributes", 0.5, true),
    src("+# to strength and intelligence", 0.5, true), src("+# to dexterity and intelligence", 0.5, true),
];
const STR: &[Source] = &[src("+# to strength", 1.0, true), src("+# to all attributes", 1.0, true),
    src("+# to strength and intelligence", 1.0, true), src("+# to strength and dexterity", 1.0, true)];
const DEX: &[Source] = &[src("+# to dexterity", 1.0, true), src("+# to all attributes", 1.0, true),
    src("+# to strength and dexterity", 1.0, true), src("+# to dexterity and intelligence", 1.0, true)];
const INT: &[Source] = &[src("+# to intelligence", 1.0, true), src("+# to all attributes", 1.0, true),
    src("+# to strength and intelligence", 1.0, true), src("+# to dexterity and intelligence", 1.0, true)];

const RULES: &[Rule] = &[
    Rule { text: "+# total maximum Life", fallback_id: Some("pseudo.pseudo_total_life"), sources: LIFE, required: Some("+# to maximum life"), needs_two: false, tick_at: Some(40.0) },
    Rule { text: "+#% total Elemental Resistance", fallback_id: Some("pseudo.pseudo_total_elemental_resistance"), sources: ELE_RES, required: None, needs_two: false, tick_at: Some(0.0) },
    Rule { text: "+#% total to Fire Resistance", fallback_id: Some("pseudo.pseudo_total_fire_resistance"), sources: FIRE_RES, required: None, needs_two: false, tick_at: None },
    Rule { text: "+#% total to Cold Resistance", fallback_id: Some("pseudo.pseudo_total_cold_resistance"), sources: COLD_RES, required: None, needs_two: false, tick_at: None },
    Rule { text: "+#% total to Lightning Resistance", fallback_id: Some("pseudo.pseudo_total_lightning_resistance"), sources: LIGHTNING_RES, required: None, needs_two: false, tick_at: None },
    Rule { text: "+#% total to Chaos Resistance", fallback_id: Some("pseudo.pseudo_total_chaos_resistance"), sources: CHAOS_RES, required: None, needs_two: false, tick_at: Some(0.0) },
    Rule { text: "+# total maximum Mana", fallback_id: None, sources: MANA, required: Some("+# to maximum mana"), needs_two: false, tick_at: None },
    Rule { text: "+# total to Strength", fallback_id: None, sources: STR, required: None, needs_two: true, tick_at: None },
    Rule { text: "+# total to Dexterity", fallback_id: None, sources: DEX, required: None, needs_two: true, tick_at: None },
    Rule { text: "+# total to Intelligence", fallback_id: None, sources: INT, required: None, needs_two: true, tick_at: None },
    Rule { text: "+#% total Attack Speed", fallback_id: None, sources: &[src("#% increased attack speed", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "+#% total Cast Speed", fallback_id: None, sources: &[src("#% increased cast speed", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "#% increased Movement Speed", fallback_id: None, sources: &[src("#% increased movement speed", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "+#% Global Critical Strike Chance", fallback_id: None, sources: &[src("#% increased global critical strike chance", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "+#% Global Critical Strike Multiplier", fallback_id: None, sources: &[src("+#% to global critical strike multiplier", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "#% increased Spell Damage", fallback_id: None, sources: &[src("#% increased spell damage", 1.0, false)], required: None, needs_two: true, tick_at: None },
    Rule { text: "#% increased Elemental Damage", fallback_id: None, sources: &[src("#% increased elemental damage", 1.0, false)], required: None, needs_two: true, tick_at: None },
];

/// The totals for these mods, and which mods they replace in the list.
pub fn totals(mods: &[ModLine], item: &Item, index: &StatIndex, skip: &[usize]) -> (Vec<StatFilter>, Vec<usize>) {
    let unique = item.rarity == "Unique";
    let mut out = Vec::new();
    let mut hide: Vec<usize> = Vec::new();
    for rule in RULES {
        let id = index.pseudo_id(rule.text).or_else(|| rule.fallback_id.map(String::from));
        let Some(id) = id else { continue };
        let mut total = 0.0;
        let mut used: Vec<(usize, bool)> = Vec::new();
        let mut explicit_sources = 0;
        for (i, m) in mods.iter().enumerate() {
            if m.kind == "enchant" || skip.contains(&i) { continue; }
            let (t, v) = template(&m.text);
            let t = t.to_lowercase();
            let Some(s) = rule.sources.iter().find(|s| s.t == t) else { continue };
            total += v.first().copied().unwrap_or(0.0) * s.times;
            used.push((i, s.keep));
            if m.kind == "explicit" || m.kind == "mutated" { explicit_sources += 1; }
        }
        if used.is_empty() || total <= 0.0 { continue; }
        if let Some(req) = rule.required {
            if !used.iter().any(|(i, _)| template(&mods[*i].text).0.to_lowercase() == req) { continue; }
        }
        // A unique's fixed mods are searched one by one; a total only helps when two of its mods add up.
        if (rule.needs_two || unique) && (if unique { explicit_sources } else { used.len() }) < 2 { continue; }
        // Chaos resistance from one crafted mod alone is anyone's bench craft: the total is there but not ticked.
        // (It used to be left out, but a "Fire and Chaos" craft's row was already counted into the elemental total
        // and hidden, so chaos vanished: 21 Nov, his Demon Cowl.)
        let bench_only = std::ptr::eq(rule.sources, CHAOS_RES) && used.len() == 1 && mods[used[0].0].kind == "crafted";
        let total = total.floor();
        out.push(StatFilter {
            id, label: rule.text.replace('#', &total.to_string()), value: total, min: Some(total),
            enabled: !unique && !bench_only && rule.tick_at.map(|t| total >= t).unwrap_or(false),
            kind: "pseudo".into(), tier: None, junk: false, roll: None, range: None, affix: None,
        });
        if !unique { hide.extend(used.iter().filter(|(_, keep)| !keep).map(|(i, _)| *i)); }
    }
    hide.sort();
    hide.dedup();
    (out, hide)
}

/// The numbers at the top of a weapon or armour's text.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Props {
    pub quality: f64,
    pub phys: Option<(f64, f64)>,
    /// Sum of each element's (min, max).
    pub ele: Option<(f64, f64)>,
    pub aps: Option<f64>,
    pub armour: Option<f64>,
    pub evasion: Option<f64>,
    pub es: Option<f64>,
    pub ward: Option<f64>,
}

fn first_num(s: &str) -> Option<f64> {
    let n: String = s.trim().trim_start_matches('+').chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    n.parse().ok()
}

fn ranges(s: &str) -> Vec<(f64, f64)> {
    s.split(',').filter_map(|part| {
        let p = part.trim().split(' ').next()?;
        let (a, b) = p.split_once('-')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    }).collect()
}

pub fn props(text: &str) -> Props {
    let mut p = Props::default();
    let text = text.replace('\r', "");
    for l in text.split("--------").take(3).flat_map(|s| s.lines()).map(str::trim) {
        let Some((k, v)) = l.split_once(": ") else { continue };
        match k {
            "Quality" => p.quality = first_num(v).unwrap_or(0.0),
            "Physical Damage" => p.phys = ranges(v).first().copied(),
            "Elemental Damage" => { let r = ranges(v); if !r.is_empty() { p.ele = Some(r.iter().fold((0.0, 0.0), |a, x| (a.0 + x.0, a.1 + x.1))); } }
            "Attacks per Second" => p.aps = first_num(v),
            "Armour" => p.armour = first_num(v),
            "Evasion Rating" => p.evasion = first_num(v),
            "Energy Shield" => p.es = first_num(v),
            "Ward" => p.ward = first_num(v),
            _ => {}
        }
    }
    p
}

/// Local mods that raise each defence (they're counted in the total, so the total replaces them).
const AR_INC: &[&str] = &["#% increased armour", "#% increased armour and energy shield", "#% increased armour and evasion", "#% increased armour, evasion and energy shield"];
const EV_INC: &[&str] = &["#% increased evasion rating", "#% increased armour and evasion", "#% increased evasion and energy shield", "#% increased armour, evasion and energy shield"];
const ES_INC: &[&str] = &["#% increased energy shield", "#% increased armour and energy shield", "#% increased evasion and energy shield", "#% increased armour, evasion and energy shield"];
const WEAPON_LOCAL: &[&str] = &["adds # to # physical damage", "#% increased physical damage", "#% increased attack speed",
    "adds # to # fire damage", "adds # to # cold damage", "adds # to # lightning damage"];
const DEFENCE_FLAT: &[&str] = &["+# to armour", "+# to evasion rating", "+# to maximum energy shield", "+# to ward"];

fn sum_inc(mods: &[ModLine], list: &[&str]) -> f64 {
    mods.iter().filter(|m| m.kind != "enchant").map(|m| {
        let (t, v) = template(&m.text);
        if list.contains(&t.to_lowercase().as_str()) { v.first().copied().unwrap_or(0.0) } else { 0.0 }
    }).sum()
}

/// Scales a defence or damage number from the item's quality to 20% (what the trade site searches by).
fn at_q20(value: f64, inc: f64, quality: f64) -> f64 {
    let q = quality.max(20.0);
    value * (100.0 + inc + q) / (100.0 + inc + quality)
}

/// Weapon DPS and armour / evasion / energy shield at 20% quality, and the local mods they replace.
pub fn property_filters(text: &str, item: &Item, mods: &[ModLine]) -> (Vec<StatFilter>, Vec<usize>) {
    let p = props(text);
    let unique = item.rarity == "Unique";
    let mut out = Vec::new();
    let mut replaced: Vec<&str> = Vec::new();
    let f = |id: &str, label: String, v: f64, on: bool| StatFilter {
        id: id.into(), label, value: v, min: Some(v), enabled: on && !unique, kind: "property".into(), tier: None, junk: false, roll: None, range: None, affix: None };
    // Armour: each defence at 20% quality; ticked when the base has only one kind.
    let defences = [p.armour, p.evasion, p.es, p.ward].iter().filter(|x| x.is_some()).count();
    for (val, id, name, inc) in [(p.armour, "prop.ar", "Armour", AR_INC), (p.evasion, "prop.ev", "Evasion Rating", EV_INC),
        (p.es, "prop.es", "Energy Shield", ES_INC), (p.ward, "prop.ward", "Ward", &["#% increased ward"][..])] {
        let Some(v) = val else { continue };
        let q20 = at_q20(v, sum_inc(mods, inc), p.quality).floor();
        out.push(f(id, format!("{name}: {q20} (at 20% quality)"), q20, defences == 1));
        replaced.extend(inc.iter().copied());
        replaced.extend(DEFENCE_FLAT.iter().copied());
    }
    // Weapons: physical, elemental and total damage per second.
    if let Some(aps) = p.aps {
        let pdps = p.phys.map(|(a, b)| at_q20((a + b) / 2.0, sum_inc(mods, &["#% increased physical damage"]), p.quality) * aps).unwrap_or(0.0);
        let edps = p.ele.map(|(a, b)| (a + b) / 2.0 * aps).unwrap_or(0.0);
        let dps = pdps + edps;
        if dps > 0.0 {
            let r = |x: f64| (x * 10.0).floor() / 10.0;
            let phys_weapon = matches!(item.item_class.as_str(), "One Hand Axes" | "Two Hand Axes" | "One Hand Swords" | "Two Hand Swords"
                | "Thrusting One Hand Swords" | "Bows" | "Warstaves");
            if pdps > 0.0 && edps > 0.0 { out.push(f("prop.dps", format!("Total DPS: {}", r(dps)), r(dps), true)); }
            if edps > 0.0 && edps / dps >= 0.67 { out.push(f("prop.edps", format!("Elemental DPS: {}", r(edps)), r(edps), true)); }
            if pdps > 0.0 && pdps / dps >= 0.67 {
                out.push(f("prop.pdps", format!("Physical DPS: {} (at 20% quality)", r(pdps)), r(pdps), phys_weapon || edps == 0.0));
            }
            replaced.extend(WEAPON_LOCAL.iter().copied());
        }
    }
    let hide = mods.iter().enumerate().filter(|(_, m)| m.kind != "implicit" && m.kind != "enchant"
        && replaced.contains(&template(&m.text).0.to_lowercase().as_str())).map(|(i, _)| i).collect();
    (out, hide)
}

/// Where a property filter goes in the trade search.
pub fn property_query(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "prop.ar" => ("armour_filters", "ar"), "prop.ev" => ("armour_filters", "ev"), "prop.es" => ("armour_filters", "es"),
        "prop.ward" => ("armour_filters", "ward"), "prop.pdps" => ("weapon_filters", "pdps"), "prop.edps" => ("weapon_filters", "edps"),
        "prop.dps" => ("weapon_filters", "dps"),
        "prop.map_iiq" => ("map_filters", "map_iiq"), "prop.map_iir" => ("map_filters", "map_iir"),
        "prop.map_bonus" => ("map_filters", "map_bonus"), "prop.map_packsize" => ("map_filters", "map_packsize"),
        _ => return None,
    })
}

/// "Has an open prefix / suffix" (from the advanced copy's mod headings), for crafting.
/// How many prefixes and suffixes the item can hold: jewels (abyss and cluster too) two of each, other rares three,
/// changed by implicits like the Cogwork Ring's "-1 Prefix Modifier allowed" / "+1 Suffix Modifier allowed".
pub fn affix_limits(text: &str, item: &Item) -> (u32, u32) {
    let base: i32 = if item.item_class.contains("Jewel") { 2 } else { 3 };
    let (mut pre, mut suf) = (base, base);
    for l in text.lines().map(str::trim) {
        let l = l.trim_end_matches(" (implicit)");
        let Some((n, rest)) = l.split_once(' ') else { continue };
        let Ok(n) = n.trim_start_matches('+').parse::<i32>() else { continue };
        if rest.starts_with("Prefix Modifier") && rest.contains("allowed") { pre += n; }
        if rest.starts_with("Suffix Modifier") && rest.contains("allowed") { suf += n; }
    }
    (pre.max(0) as u32, suf.max(0) as u32)
}

pub fn open_affixes(text: &str, item: &Item, index: &StatIndex) -> Vec<StatFilter> {
    if item.rarity != "Rare" || item.corrupted || text.contains("\nMirrored") { return vec![]; }
    let (mut pre, mut suf) = (0, 0);
    for l in text.lines().map(str::trim).filter(|l| l.starts_with('{')) {
        let low = l.to_lowercase();
        if low.contains("prefix modifier") { pre += 1; }
        if low.contains("suffix modifier") { suf += 1; }
    }
    if pre + suf == 0 { return vec![]; } // plain copy: can't tell
    let (max_pre, max_suf) = affix_limits(text, item);
    let mut out = Vec::new();
    for (n, text_id, label) in [(max_pre.saturating_sub(pre), "# Empty Prefix Modifiers", "prefix"), (max_suf.saturating_sub(suf), "# Empty Suffix Modifiers", "suffix")] {
        if n == 0 { continue; }
        let Some(id) = index.pseudo_id(text_id) else { continue };
        out.push(StatFilter { id, label: format!("Open {label} ({n} free)"), value: n as f64, min: Some(1.0), enabled: false,
            kind: "pseudo".into(), tier: None, junk: false, roll: None, range: None, affix: None });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trade::{mods, setup, query};
    use serde_json::json;

    fn index() -> StatIndex {
        StatIndex::from_json(&json!({ "result": [
            { "label": "Pseudo", "entries": [
                { "id": "pseudo.pseudo_total_life", "text": "+# total maximum Life", "type": "pseudo" },
                { "id": "pseudo.pseudo_total_elemental_resistance", "text": "+#% total Elemental Resistance", "type": "pseudo" },
                { "id": "pseudo.pseudo_total_chaos_resistance", "text": "+#% total to Chaos Resistance", "type": "pseudo" },
                { "id": "pseudo.pseudo_total_strength", "text": "+# total to Strength", "type": "pseudo" },
                { "id": "pseudo.pseudo_number_of_empty_prefix_mods", "text": "# Empty Prefix Modifiers", "type": "pseudo" },
                { "id": "pseudo.pseudo_number_of_empty_suffix_mods", "text": "# Empty Suffix Modifiers", "type": "pseudo" } ] },
            { "label": "Explicit", "entries": [
                { "id": "explicit.life", "text": "+# to maximum Life", "type": "explicit" },
                { "id": "explicit.fire", "text": "+#% to Fire Resistance", "type": "explicit" },
                { "id": "explicit.firechaos", "text": "+#% to Fire and Chaos Resistances", "type": "explicit" },
                { "id": "explicit.str", "text": "+# to Strength", "type": "explicit" },
                { "id": "explicit.es", "text": "+# to maximum Energy Shield", "type": "explicit" },
                { "id": "explicit.esinc", "text": "#% increased Energy Shield", "type": "explicit" },
                { "id": "explicit.ms", "text": "#% increased Movement Speed", "type": "explicit" } ] } ] }))
    }

    const BOOTS: &str = "Item Class: Boots\nRarity: Rare\nDoom Track\nSorcerer Boots\n--------\nQuality: +10% (augmented)\nEnergy Shield: 150 (augmented)\n--------\nItem Level: 84\n--------\n{ Prefix Modifier \"Hale\" (Tier: 3) — Life }\n+60(55-64) to maximum Life\n{ Prefix Modifier \"Shimmering\" (Tier: 4) — Defences }\n+30(27-33) to maximum Energy Shield\n{ Suffix Modifier \"of the Furnace\" (Tier: 2) — Elemental, Fire, Resistance }\n+40(36-41)% to Fire Resistance\n{ Suffix Modifier \"of the Lost\" (Tier: 3) — Chaos, Resistance }\n+15(11-15)% to Fire and Chaos Resistances\n{ Suffix Modifier \"of the Brute\" (Tier: 5) — Attribute }\n+20(18-22) to Strength\n";

    #[test]
    fn totals_replace_their_mods() {
        let s = setup(BOOTS, &index()).unwrap();
        let get = |id: &str| s.filters.iter().find(|f| f.id == id).cloned();
        assert_eq!(get("pseudo.pseudo_total_life").unwrap().value, 70.0); // 60 + 20 Str / 2
        assert_eq!(get("pseudo.pseudo_total_elemental_resistance").unwrap().value, 55.0);
        assert_eq!(get("pseudo.pseudo_total_chaos_resistance").unwrap().value, 15.0);
        // Like Awakened: elemental and chaos totals ticked, fire's own total (40 + 15) there to tick, no cold/lightning.
        assert!(get("pseudo.pseudo_total_elemental_resistance").unwrap().enabled && get("pseudo.pseudo_total_chaos_resistance").unwrap().enabled);
        let fire = get("pseudo.pseudo_total_fire_resistance").unwrap();
        assert_eq!((fire.value, fire.enabled), (55.0, false));
        assert!(get("pseudo.pseudo_total_cold_resistance").is_none() && get("pseudo.pseudo_total_resistance").is_none());
        // Folded into totals: gone from the list, fire+chaos too (both totals count it). Strength also gives life: kept.
        assert!(get("explicit.life").is_none() && get("explicit.fire").is_none() && get("explicit.firechaos").is_none());
        assert!(get("explicit.str").is_some());
        // Energy shield at 20% quality replaces the local ES mod: 150 * 120/110.
        let es = get("prop.es").unwrap();
        assert_eq!((es.value, es.enabled), (163.0, true));
        assert!(get("explicit.es").is_none());
        // Two prefixes and three suffixes: one open prefix.
        let open = get("pseudo.pseudo_number_of_empty_prefix_mods").unwrap();
        assert_eq!((open.value, open.enabled), (1.0, false));
        assert!(get("pseudo.pseudo_number_of_empty_suffix_mods").is_none());
        let q = query(&s);
        assert_eq!(q["query"]["filters"]["armour_filters"]["filters"]["es"]["min"], 163.0);
        assert!(q["query"]["stats"][0]["filters"].as_array().unwrap().iter().all(|f| !f["id"].as_str().unwrap().starts_with("prop.")));
    }

    #[test]
    fn weapon_dps() {
        let t = "Item Class: Two Hand Axes\nRarity: Rare\nX\nVaal Axe\n--------\nQuality: +0%\nPhysical Damage: 200-300 (augmented)\nElemental Damage: 10-20 (augmented)\nAttacks per Second: 1.50\n--------\nItem Level: 84\n--------\n{ Prefix Modifier \"Merciless\" }\n150% increased Physical Damage\n";
        let item = crate::item::parse_item(t).unwrap();
        let (f, hide) = property_filters(t, &item, &mods(t));
        let pdps = f.iter().find(|x| x.id == "prop.pdps").unwrap();
        // 250 avg * (100+150+20)/(100+150) * 1.5 = 405
        assert_eq!(pdps.value, 405.0);
        assert!(pdps.enabled);
        assert!(f.iter().any(|x| x.id == "prop.dps"));
        assert_eq!(hide, vec![0]);
    }

    #[test]
    fn unique_totals_need_two_mods() {
        let t = "Item Class: Body Armours\nRarity: Unique\nFoulborn Ghostwrithe\nSilken Vest\n--------\nItem Level: 83\n--------\n+90 to maximum Life\n";
        let s = setup(t, &index()).unwrap();
        assert!(s.filters.iter().all(|f| f.kind != "pseudo"));
        assert!(s.filters.iter().any(|f| f.id == "explicit.life"));
    }
}

/// What sells a map, like Awakened PoE Trade: its drop bonuses (Item Quantity, Atlas Drops, Item Rarity,
/// Pack Size, More Currency / Scarabs / Div Cards / Maps) and whether it has all 8 mods. Ticked as Awakened
/// does: Rarity is left off when the map has a "More …" drop bonus, and More Maps is never ticked for you.
pub fn map_props(text: &str, item: &Item) -> Vec<StatFilter> {
    if item.item_class != "Maps" || item.rarity == "Unique" { return vec![]; }
    let text = text.replace('\r', "");
    let read = |key: &str| text.lines().map(str::trim).find_map(|l| l.strip_prefix(key))
        .and_then(|v| v.trim().trim_start_matches('+').split('%').next()?.trim().parse::<f64>().ok());
    let more = |k: &str| read(&format!("More {k}: "));
    let valuable = more("Scarabs").is_some() || more("Currency").is_some() || more("Divination Cards").is_some();
    let f = |id: &str, label: String, v: f64, on: bool| StatFilter {
        id: id.into(), label, value: v, min: Some(v), enabled: on, kind: "property".into(), tier: None, junk: false, roll: None, range: None, affix: None };
    let mut out = Vec::new();
    for (key, id, on) in [("Item Quantity: ", "prop.map_iiq", true), ("Atlas Drops: ", "prop.map_bonus", true),
        ("Item Rarity: ", "prop.map_iir", !valuable), ("Monster Pack Size: ", "prop.map_packsize", true)] {
        if let Some(v) = read(key) { out.push(f(id, format!("{key}+{v}%"), v, on)); }
    }
    for (k, id, on) in [("Currency", "pseudo.pseudo_map_more_currency_drops", true), ("Scarabs", "pseudo.pseudo_map_more_scarab_drops", true),
        ("Divination Cards", "pseudo.pseudo_map_more_card_drops", true), ("Maps", "pseudo.pseudo_map_more_map_drops", false)] {
        if let Some(v) = more(k) { out.push(StatFilter { kind: "pseudo".into(), ..f(id, format!("More {k}: +{v}%"), v, on) }); }
    }
    // Eight-mod maps sell for more: counted from the advanced copy's mod headings.
    let mods = text.lines().map(str::trim).filter(|l| l.starts_with('{') && {
        let low = l.to_lowercase();
        low.contains("prefix modifier") || low.contains("suffix modifier")
    }).count();
    if mods >= 8 { out.push(StatFilter { kind: "pseudo".into(), ..f("pseudo.pseudo_number_of_affix_mods", format!("{mods} mods"), mods as f64, true) }); }
    out
}

#[cfg(test)]
mod buy_tests {
    use crate::trade::{buy_query, StatIndex};
    use serde_json::json;

    fn idx() -> StatIndex {
        StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [
                { "id": "explicit.life", "text": "+# to maximum Life", "type": "explicit" },
                { "id": "explicit.fire", "text": "+#% to Fire Resistance", "type": "explicit" },
                { "id": "explicit.ar_global", "text": "+# to Armour", "type": "explicit" },
                { "id": "explicit.ar_local", "text": "+# to Armour (Local)", "type": "explicit" },
                { "id": "explicit.overshock", "text": "1 Added Passive Skill is Overshock", "type": "explicit" } ] },
            { "label": "Implicit", "entries": [
                { "id": "implicit.unnerve", "text": "#% chance to Unnerve Enemies for 4 seconds on Hit", "type": "implicit" } ] },
            { "label": "Crafted", "entries": [ { "id": "crafted.dex", "text": "+# to Dexterity", "type": "crafted" } ] },
            { "label": "Enchant", "entries": [ { "id": "enchant.passives", "text": "Adds # Passive Skills", "type": "enchant" } ] } ] }))
    }

    #[test]
    fn like_poestash() {
        // PoB's gloves as the price window reads them: every mod asked for, no numbers, exact base, rare.
        let gloves = "Item Class: Gloves\nRarity: Rare\nEndgame Gloves\nPaladin Gloves\n--------\nItem Level: 1\n--------\n\
            20% chance to Unnerve Enemies for 4 seconds on Hit (implicit)\n--------\n+80 to maximum Life\n+40% to Fire Resistance\n\
            +68 to Armour\n+18 to Dexterity (crafted)\n";
        let (q, n) = buy_query(gloves, &idx()).unwrap();
        let ids: Vec<&str> = q["query"]["stats"][0]["filters"].as_array().unwrap().iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["implicit.unnerve", "explicit.life", "explicit.fire", "explicit.ar_local", "crafted.dex"]);
        assert_eq!(n, 4, "the crafted one is shown but not required");
        assert_eq!(q["query"]["stats"][0]["filters"][4]["disabled"], true);
        assert!(q["query"]["stats"][0]["filters"].as_array().unwrap().iter().all(|v| v.get("value").is_none()), "no numbers");
        assert_eq!(q["query"]["type"], "Paladin Gloves");
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"], "rare");

        // Cluster jewel: passive count exact, notables by name.
        let cluster = "Item Class: Jewels\nRarity: Rare\nEagle Glisten\nLarge Cluster Jewel\n--------\nItem Level: 1\n--------\n\
            Adds 8 Passive Skills (enchant)\n--------\n1 Added Passive Skill is Overshock\n";
        let (q, _) = buy_query(cluster, &idx()).unwrap();
        let f = &q["query"]["stats"][0]["filters"];
        assert_eq!(f[0], json!({ "id": "enchant.passives", "value": { "min": 8.0, "max": 8.0 } }));
        assert_eq!(f[1], json!({ "id": "explicit.overshock" }));

        // Pick-one stats are listed as "id|14" and searched as the id with option 14.
        assert_eq!(crate::trade::stat_json("enchant.stat_3948993189|14"), json!({ "id": "enchant.stat_3948993189", "value": { "option": 14 } }));

        // Magic flask: searched by its base, not its full name.
        let flask = "Item Class: Utility Flasks\nRarity: Magic\nPerpetual Sapphire Flask of Craft\n--------\nItem Level: 85\n--------\n+80 to maximum Life\n";
        let (q, _) = buy_query(flask, &idx()).unwrap();
        assert_eq!(q["query"]["type"], "Sapphire Flask");
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["rarity"]["option"], "magic");

        // Unique: by name, its fixed mods not asked for.
        let unique = "Item Class: Amulets\nRarity: Unique\nMarylene's Fallacy\nLapis Amulet\n--------\nItem Level: 1\n--------\n+80 to maximum Life\n";
        let (q, n) = buy_query(unique, &idx()).unwrap();
        assert_eq!((q["query"]["name"].as_str(), n), (Some("Marylene's Fallacy"), 0));
    }
}

#[cfg(test)]
mod pob_wording_tests {
    use crate::trade::{setup, ItemTypes, StatIndex};
    use serde_json::json;

    #[test]
    fn pob_items_find_their_trade_stats() {
        let idx = StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [
                { "id": "explicit.ar_global", "text": "+# to Armour", "type": "explicit" },
                { "id": "explicit.ar_local", "text": "+# to Armour (Local)", "type": "explicit" },
                { "id": "explicit.abyss", "text": "Has # Abyssal Sockets", "type": "explicit" },
                { "id": "explicit.solaris", "text": "Grants Level # Aspect of Solaris", "type": "explicit" },
                { "id": "explicit.chan", "text": "Channelling Skills have +# to Total Mana Cost", "type": "explicit" } ] },
            { "label": "Implicit", "entries": [
                { "id": "implicit.unnerve", "text": "While a Unique Enemy is in your Presence, #% chance to Unnerve Enemies for 4 seconds on Hit", "type": "implicit" },
                { "id": "implicit.expo", "text": "While a Unique Enemy is in your Presence, Inflict Lightning Exposure on Hit, applying #% to Lightning Resistance", "type": "implicit" },
                { "id": "implicit.flask", "text": "While a Unique Enemy is in your Presence, Flasks gain # Charges every 3 seconds", "type": "implicit" } ] } ] }));
        let gloves = "Item Class: Gloves\nRarity: Rare\nX\nPaladin Gloves\n--------\nItem Level: 1\n--------\n\
            While a Unique Enemy is in your Presence, 20% chance to Unnerve Enemies for 4 seconds on Hit (implicit)\n\
            While a Unique Enemy is in your Presence, Inflict Lightning Exposure on Hit, applying -12% to Lightning Resistance (implicit)\n\
            While a Unique Enemy is in your Presence, Flasks gain a Charge every 3 seconds (implicit)\n--------\n+68 to Armour\n\
            Channelling Skills have -3 to Total Mana Cost\nGrants Level 20 Aspect of Solaris Skill\nHas 1 Abyssal Socket\n";
        let s = setup(gloves, &idx).unwrap();
        assert!(s.unknown.is_empty(), "{:?}", s.unknown);
        let get = |id: &str| s.filters.iter().find(|f| f.id == id).map(|f| f.value);
        assert_eq!(get("implicit.unnerve"), Some(20.0)); // the "4 seconds" stays words
        assert_eq!(get("implicit.expo"), Some(-12.0));
        assert_eq!(get("implicit.flask"), Some(1.0));
        assert_eq!(get("explicit.ar_local"), Some(68.0)); // gloves' own armour, not the global stat
        assert_eq!(get("explicit.chan"), Some(-3.0));
        assert_eq!((get("explicit.solaris"), get("explicit.abyss")), (Some(20.0), Some(1.0)));
        // A unique renamed in PoB ("Endgame Marylene's Fallacy") is searched by its real name.
        let types = ItemTypes::from_json(&json!({ "result": [ { "entries": [
            { "name": "Marylene's Fallacy", "type": "Lapis Amulet", "text": "Marylene's Fallacy Lapis Amulet", "flags": { "unique": true } },
            { "type": "Lapis Amulet", "text": "Lapis Amulet" } ] } ] }));
        let mut q = json!({ "query": { "name": "Endgame Marylene's Fallacy", "type": "Lapis Amulet" } });
        types.fix_query(&mut q);
        assert_eq!(q["query"]["name"], "Marylene's Fallacy");
    }
}

#[cfg(test)]
mod base_item_tests {
    use crate::trade::{query, setup, StatIndex};
    use serde_json::json;

    fn idx() -> StatIndex {
        StatIndex::from_json(&json!({ "result": [
            { "label": "Explicit", "entries": [
                { "id": "explicit.life", "text": "+# to maximum Life", "type": "explicit" },
                { "id": "explicit.ms", "text": "#% increased Movement Speed", "type": "explicit" },
                { "id": "explicit.fire", "text": "+#% to Fire Resistance", "type": "explicit" } ] },
            { "label": "Implicit", "entries": [ { "id": "implicit.str", "text": "+# to Strength", "type": "implicit" } ] },
            { "label": "Fractured", "entries": [ { "id": "fractured.ms", "text": "#% increased Movement Speed", "type": "fractured" } ] },
            { "label": "Crafted", "entries": [ { "id": "crafted.fire", "text": "+#% to Fire Resistance", "type": "crafted" } ] } ] }))
    }

    #[test]
    fn like_awakened() {
        // Magic boots, item level 84: a base for crafters. Its tier 1 speed mod counts, the tier 5 life roll doesn't.
        let magic = "Item Class: Boots\nRarity: Magic\nFleet Two-Toned Boots of Life\n--------\nItem Level: 84\n--------\n\
            { Prefix Modifier \"Fleet\" (Tier: 1) — Speed }\n30% increased Movement Speed\n\
            { Suffix Modifier \"of Life\" (Tier: 5) — Life }\n+40 to maximum Life\n";
        let b = setup(magic, &idx()).unwrap().base_item.expect("a base");
        assert_eq!((b.why.clone(), b.ids.clone(), b.ilvl_min), (vec!["Item level 84".to_string()], vec!["explicit.ms".to_string()], Some(84)));

        // Fractured rare, Hunter influence: only the fractured mod, both switches on; no implicits (influenced).
        let fract = "Item Class: Boots\nRarity: Rare\nX\nTwo-Toned Boots\n--------\nItem Level: 75\n--------\n\
            +20 to Strength (implicit)\n--------\n30% increased Movement Speed (fractured)\n+40 to maximum Life\n--------\nFractured Item\nHunter Item\n";
        let s = setup(fract, &idx()).unwrap();
        let b = s.base_item.clone().unwrap();
        assert_eq!(b.why, ["Fractured", "Hunter"]);
        assert_eq!(b.ids, ["fractured.ms"]);
        assert_eq!(b.misc, ["hunter_item", "fractured_item"]);
        // What the window searches when you press it: same base, item level 75+, the switches, only those mods.
        let mut s2 = s.clone();
        s2.base_only = true; s2.ilvl_min = b.ilvl_min; s2.misc = b.misc.clone();
        for f in &mut s2.filters { f.enabled = b.ids.contains(&f.id); }
        let q = query(&s2);
        assert_eq!(q["query"]["type"], "Two-Toned Boots");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["hunter_item"]["option"], "true");
        assert_eq!(q["query"]["filters"]["misc_filters"]["filters"]["ilvl"]["min"], 75);
        assert_eq!(q["query"]["stats"][0]["filters"].as_array().unwrap().len(), 1);
        assert_eq!(q["query"]["stats"][0]["filters"][0]["id"], "fractured.ms", "as a base: must be fractured");
        // "This item": the same mod on any item, fractured or not.
        let mut s3 = s.clone();
        for f in &mut s3.filters { f.enabled = f.id == "fractured.ms"; }
        assert_eq!(query(&s3)["query"]["stats"][0]["filters"][0]["id"], "explicit.ms");

        // Finished items (a crafted mod) and ordinary low-level rares: no base search.
        let crafted = "Item Class: Boots\nRarity: Rare\nX\nTwo-Toned Boots\n--------\nItem Level: 85\n--------\n+40 to maximum Life\n+30% to Fire Resistance (crafted)\n";
        assert!(setup(crafted, &idx()).unwrap().base_item.is_none());
        // A fractured item with a crafted mod (his Woe Buckle) is still a base: crafted mods come off.
        let belt = "Item Class: Belts
Rarity: Rare
Woe Buckle
Stygian Vise
--------
Item Level: 85
--------
            Has 1 Abyssal Socket — Unscalable Value (implicit)
--------
30% increased Movement Speed (fractured)
+30% to Fire Resistance (crafted)
--------
Fractured Item
";
        let b = setup(belt, &idx()).unwrap().base_item.expect("fractured: still a base");
        assert_eq!((b.ids.clone(), b.ilvl_min), (vec!["fractured.ms".to_string()], Some(85)));
        assert_eq!(crate::trade::mods(belt)[0].text, "Has 1 Abyssal Socket");
        assert_eq!(crate::trade::mods("Item Level: 86\n--------\nGrants Level 20 \u{200b}Aspect of the Spider — Unscalable Value\n")[0].text,
            "Grants Level 20 Aspect of the Spider");
        let plain = "Item Class: Boots\nRarity: Rare\nX\nTwo-Toned Boots\n--------\nItem Level: 70\n--------\n+40 to maximum Life\n";
        assert!(setup(plain, &idx()).unwrap().base_item.is_none());
    }
}

#[cfg(test)]
mod affix_limit_tests {
    use crate::trade::{setup, StatIndex};
    use serde_json::json;

    #[test]
    fn cogwork_ring() {
        // His Dragon Turn: Cogwork Ring holds 2 prefixes and 4 suffixes; it has 1 and 3, so one of each is free.
        let text = "Item Class: Rings\nRarity: Rare\nDragon Turn\nCogwork Ring\n--------\nItem Level: 84\n--------\n{ Implicit Modifier }\n\
            -1 Prefix Modifier allowed\n+1 Suffix Modifier allowed\nImplicit Modifiers Cannot Be Changed — Unscalable Value\n--------\n\
            { Prefix Modifier \"Mazarine\" (Tier: 4) — Mana }\n+60(60-64) to maximum Mana\n\
            { Suffix Modifier \"of the Rainbow\" (Tier: 1) — Elemental, Resistance  — 25% Increased }\n+15(15-16)% to all Elemental Resistances\n\
            { Suffix Modifier \"of the Virtuoso\" (Tier: 2) — Attribute  — 25% Increased }\n+46(43-50) to Intelligence\n\
            { Suffix Modifier \"of Haast\" (Tier: 1) — Elemental, Cold, Resistance  — 25% Increased }\n+46(46-48)% to Cold Resistance\n";
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Pseudo", "entries": [
            { "id": "pseudo.pseudo_number_of_empty_prefix_mods", "text": "# Empty Prefix Modifiers", "type": "pseudo" },
            { "id": "pseudo.pseudo_number_of_empty_suffix_mods", "text": "# Empty Suffix Modifiers", "type": "pseudo" } ] } ] }));
        let s = setup(text, &idx).unwrap();
        assert_eq!(s.affix_max, Some((2, 4)));
        let open: Vec<&str> = s.filters.iter().filter(|f| f.id.contains("empty")).map(|f| f.label.as_str()).collect();
        assert_eq!(open, ["Open prefix (1 free)", "Open suffix (1 free)"]);
    }
}

#[cfg(test)]
mod timeless_tests {
    use crate::trade::{query, setup, StatIndex};
    use serde_json::json;

    #[test]
    fn brutal_restraint_by_seed() {
        let idx = StatIndex::from_json(&json!({ "result": [ { "label": "Explicit", "entries": [
            { "id": "explicit.pseudo_timeless_jewel_balbala", "text": "Denoted service of # dekhara in the akhara of Balbala", "type": "explicit" },
            { "id": "explicit.pseudo_timeless_jewel_nasima", "text": "Denoted service of # dekhara in the akhara of Nasima", "type": "explicit" },
            { "id": "explicit.pseudo_timeless_jewel_asenath", "text": "Denoted service of # dekhara in the akhara of Asenath", "type": "explicit" },
            { "id": "explicit.pseudo_timeless_jewel_kaom", "text": "Commanded leadership over # warriors under Kaom", "type": "explicit" } ] } ] }));
        // His build's jewel, as the game copies it.
        let text = "Item Class: Jewels\nRarity: Unique\nBrutal Restraint\nTimeless Jewel\n--------\nLimited to: 1 Historic\nRadius: Large\n--------\n\
            Item Level: 84\n--------\nDenoted service of 7000 dekhara in the akhara of Balbala\nPassives in radius are Conquered by the Maraketh\n\
            Historic\n--------\nThey belonged to the land.\n";
        let s = setup(text, &idx).unwrap();
        let own = s.filters.iter().find(|f| f.id == "explicit.pseudo_timeless_jewel_balbala").unwrap();
        assert!(own.enabled && own.min == Some(7000.0), "own seed ticked");
        let any = s.filters.iter().find(|f| f.id.starts_with("timeless_any:")).unwrap();
        assert!(!any.enabled);
        assert_eq!(any.label, "Seed 7000 under any conqueror (Asenath, Balbala, Nasima)");
        // Default search: name + the exact seed under Balbala.
        let q = query(&s);
        assert_eq!(q["query"]["name"], "Brutal Restraint");
        assert_eq!(q["query"]["stats"][0]["filters"][0], json!({ "id": "explicit.pseudo_timeless_jewel_balbala", "value": { "min": 7000.0, "max": 7000.0 } }));
        // "Any conqueror": a group where one of the three must have seed 7000.
        let mut s2 = s.clone();
        for f in &mut s2.filters { f.enabled = f.id.starts_with("timeless_any:"); }
        let q = query(&s2);
        assert_eq!(q["query"]["stats"][1]["type"], "count");
        assert_eq!(q["query"]["stats"][1]["filters"].as_array().unwrap().len(), 3);
        assert_eq!(q["query"]["stats"][1]["filters"][0]["value"], json!({ "min": 7000.0, "max": 7000.0 }));
        // Buy from the build: the same jewel by name and its exact seed.
        let (q, n) = crate::trade::buy_query(text, &idx).unwrap();
        assert_eq!((q["query"]["name"].as_str(), n), (Some("Brutal Restraint"), 1));
        assert_eq!(q["query"]["stats"][0]["filters"][0]["value"], json!({ "min": 7000.0, "max": 7000.0 }));
    }
}
