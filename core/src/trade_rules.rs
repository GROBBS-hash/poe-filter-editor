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

const ELE_RES: &[Source] = &[
    src("+#% to all resistances", 3.0, true), src("+#% to all elemental resistances", 3.0, false),
    src("+#% to fire resistance", 1.0, false), src("+#% to cold resistance", 1.0, false), src("+#% to lightning resistance", 1.0, false),
    src("+#% to fire and lightning resistances", 2.0, false), src("+#% to fire and cold resistances", 2.0, false),
    src("+#% to cold and lightning resistances", 2.0, false),
    src("+#% to fire and chaos resistances", 1.0, true), src("+#% to cold and chaos resistances", 1.0, true),
    src("+#% to lightning and chaos resistances", 1.0, true),
];
const CHAOS_RES: &[Source] = &[
    src("+#% to chaos resistance", 1.0, false), src("+#% to all resistances", 1.0, true),
    src("+#% to fire and chaos resistances", 1.0, true), src("+#% to cold and chaos resistances", 1.0, true),
    src("+#% to lightning and chaos resistances", 1.0, true),
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
    Rule { text: "+#% total Elemental Resistance", fallback_id: Some("pseudo.pseudo_total_elemental_resistance"), sources: ELE_RES, required: None, needs_two: false, tick_at: Some(36.0) },
    Rule { text: "+#% total to Chaos Resistance", fallback_id: Some("pseudo.pseudo_total_chaos_resistance"), sources: CHAOS_RES, required: None, needs_two: false, tick_at: Some(15.0) },
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
        let total = total.floor();
        out.push(StatFilter {
            id, label: rule.text.replace('#', &total.to_string()), value: total, min: Some(total),
            enabled: !unique && rule.tick_at.map(|t| total >= t).unwrap_or(false),
            kind: "pseudo".into(), tier: None, junk: false,
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
        id: id.into(), label, value: v, min: Some(v), enabled: on && !unique, kind: "property".into(), tier: None, junk: false };
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
        _ => return None,
    })
}

/// "Has an open prefix / suffix" (from the advanced copy's mod headings), for crafting.
pub fn open_affixes(text: &str, item: &Item, index: &StatIndex) -> Vec<StatFilter> {
    if item.rarity != "Rare" || item.corrupted || text.contains("\nMirrored") { return vec![]; }
    let (mut pre, mut suf) = (0, 0);
    for l in text.lines().map(str::trim).filter(|l| l.starts_with('{')) {
        let low = l.to_lowercase();
        if low.contains("prefix modifier") { pre += 1; }
        if low.contains("suffix modifier") { suf += 1; }
    }
    if pre + suf == 0 { return vec![]; } // plain copy: can't tell
    let mut out = Vec::new();
    for (n, text_id, label) in [(3 - pre.min(3), "# Empty Prefix Modifiers", "prefix"), (3 - suf.min(3), "# Empty Suffix Modifiers", "suffix")] {
        if n == 0 { continue; }
        let Some(id) = index.pseudo_id(text_id) else { continue };
        out.push(StatFilter { id, label: format!("Open {label} ({n} free)"), value: n as f64, min: Some(1.0), enabled: false,
            kind: "pseudo".into(), tier: None, junk: false });
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
        // Folded into totals: gone from the list. Fire+chaos and Strength also give other things: kept.
        assert!(get("explicit.life").is_none() && get("explicit.fire").is_none());
        assert!(get("explicit.firechaos").is_some() && get("explicit.str").is_some());
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
