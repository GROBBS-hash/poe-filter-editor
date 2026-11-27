//! "Find something better": for a slot you wear, one trade search that adds up the stats you need (the trade site's
//! weighted sum) and only lists items that beat what you have now, cheapest first.
//! What counts: life always, the elemental resistances your gear is short of the cap, the Str / Dex / Int the build
//! checker says you lack, movement speed on boots, energy shield when your gear is mostly ES. Each comes with a
//! reason, and can be switched off. The trade site's "pseudo" totals do the adding up (they fold "all elemental
//! resistances" into each resistance, and Strength into life), and `compare::totals` adds up the same way here so
//! what you wear gets the same score. Asked for 27 Nov 2026 ("SUM of values on the trade site").

use crate::compare;
use crate::pob::Gear;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Want {
    /// As `compare` calls it ("Life", "Fire res").
    pub stat: String,
    /// The trade site's pseudo stat.
    pub id: String,
    pub weight: f64,
    pub why: String,
    pub on: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub slot: String,
    /// What's there now ("" when the slot is empty).
    pub current: String,
    pub wants: Vec<Want>,
    /// What's there now scores this much with these weights.
    pub now: f64,
    /// Mods on what you wear that no weight counts (you'd lose them): shown so a cheap "better" item isn't a surprise.
    #[serde(default)]
    pub not_counted: Vec<String>,
}

/// The stat's trade pseudo id.
fn pseudo(stat: &str) -> Option<&'static str> {
    Some(match stat {
        "Life" => "pseudo.pseudo_total_life", "Energy Shield" => "pseudo.pseudo_total_energy_shield", "Mana" => "pseudo.pseudo_total_mana",
        "Fire res" => "pseudo.pseudo_total_fire_resistance", "Cold res" => "pseudo.pseudo_total_cold_resistance",
        "Lightning res" => "pseudo.pseudo_total_lightning_resistance", "Chaos res" => "pseudo.pseudo_total_chaos_resistance",
        "Strength" => "pseudo.pseudo_total_strength", "Dexterity" => "pseudo.pseudo_total_dexterity", "Intelligence" => "pseudo.pseudo_total_intelligence",
        "Movement speed" => "pseudo.pseudo_increased_movement_speed", "Attack speed" => "pseudo.pseudo_total_attack_speed",
        "Cast speed" => "pseudo.pseudo_total_cast_speed", "Spell damage" => "pseudo.pseudo_increased_spell_damage",
        _ => return None,
    })
}

/// The trade site's item category for a slot.
pub fn category(slot: &str) -> Option<&'static str> {
    Some(match slot {
        "Helmet" => "armour.helmet", "Body Armour" => "armour.chest", "Gloves" => "armour.gloves", "Boots" => "armour.boots",
        "Belt" => "accessory.belt", "Amulet" => "accessory.amulet", "Ring 1" | "Ring 2" | "Ring 3" => "accessory.ring",
        _ => return None, // weapons and off hands are about damage and block: the plain comparison is better there
    })
}

/// After the story your elemental resistances drop 60: 135% from gear, tree and jewels reaches the 75% cap.
const CAP: f64 = 135.0;

/// What to look for in a slot, given all the gear you wear and the Str / Dex / Int you're short of (from the build
/// checker; zeros when unknown).
pub fn plan(slot: &str, worn: &[Gear], short: [i64; 3]) -> Plan {
    let here = worn.iter().find(|g| g.slot == slot);
    let all: Vec<String> = worn.iter().flat_map(|g| g.lines.iter().cloned()).collect();
    let t = compare::totals(&all);
    let mut wants = vec![];
    let mut want = |stat: &str, weight: f64, why: String| {
        if let Some(id) = pseudo(stat) { wants.push(Want { stat: stat.into(), id: id.into(), weight, why, on: true }); }
    };
    let es_build = t.get("Energy Shield").copied().unwrap_or(0.0) > t.get("Life").copied().unwrap_or(0.0) * 1.5;
    if es_build { want("Energy Shield", 1.0, "Your gear is mostly energy shield".into()); }
    else { want("Life", 1.0, "Life keeps you alive (Strength counts half)".into()); }
    for r in ["Fire res", "Cold res", "Lightning res"] {
        let have = t.get(r).copied().unwrap_or(0.0);
        // What's on this slot now could be replaced: count the shortfall without it.
        let mine = here.map(|g| compare::totals(&g.lines).get(r).copied().unwrap_or(0.0)).unwrap_or(0.0);
        let short_by = CAP - (have - mine);
        if short_by > 0.0 {
            want(r, 1.0, format!("Your other gear has {}% {} ({}% short of the cap if your tree, jewels and auras add none: untick it if you're capped)", (have - mine).round(), r.trim_end_matches(" res").to_lowercase(), short_by.round()));
        }
    }
    for (i, a) in ["Strength", "Dexterity", "Intelligence"].iter().enumerate() {
        if short[i] > 0 { want(a, 2.0, format!("Your build is {} {a} short", short[i])); }
    }
    want("Chaos res", 0.5, "Chaos resistance, at half weight".into());
    if slot == "Boots" { want("Movement speed", 3.0, "Speed on boots: each 1% counts like 3 life".into()); }
    let now = here.map(|g| score(&g.lines, &wants)).unwrap_or(0.0);
    let not_counted = here.map(|g| g.lines.iter().filter(|l| {
        let t = compare::totals(std::slice::from_ref(*l));
        !wants.iter().any(|w| t.get(w.stat.as_str()).copied().unwrap_or(0.0) != 0.0)
    }).map(|l| plain(l)).collect()).unwrap_or_default();
    Plan { slot: slot.into(), current: here.map(|g| g.name.clone()).unwrap_or_default(), wants, now, not_counted }
}

/// "Allocates [templar_x|Divine Judgement]" -> "Allocates Divine Judgement" (the site's link markup).
fn plain(l: &str) -> String {
    let mut out = String::new();
    let mut rest = l;
    while let Some(i) = rest.find('[') {
        let Some(j) = rest[i..].find(']').map(|j| i + j) else { break };
        out.push_str(&rest[..i]);
        let inner = &rest[i + 1..j];
        out.push_str(inner.rsplit('|').next().unwrap_or(inner));
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

/// An item's score: its stats times the weights that are on.
pub fn score(lines: &[String], wants: &[Want]) -> f64 {
    let t = compare::totals(lines);
    wants.iter().filter(|w| w.on).map(|w| t.get(w.stat.as_str()).copied().unwrap_or(0.0) * w.weight).sum()
}

/// The trade search: this slot's items whose weighted sum beats what you have now, cheapest first, buyout only.
pub fn query(p: &Plan, max_level: Option<u32>) -> Option<Value> {
    let cat = category(&p.slot)?;
    let filters: Vec<Value> = p.wants.iter().filter(|w| w.on).map(|w| json!({ "id": w.id, "value": { "weight": w.weight } })).collect();
    if filters.is_empty() { return None; }
    let mut q = json!({
        "query": {
            "status": { "option": "securable" },
            "stats": [{ "type": "weight", "value": { "min": (p.now + 1.0).floor() }, "filters": filters }],
            "filters": { "type_filters": { "filters": { "category": { "option": cat } } } },
        },
        "sort": { "price": "asc" },
    });
    if let Some(l) = max_level {
        q["query"]["filters"]["req_filters"] = json!({ "filters": { "lvl": { "max": l } } });
    }
    Some(q)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn g(slot: &str, lines: &[&str]) -> Gear {
        Gear { slot: slot.into(), rarity: "rare".into(), name: format!("{slot} thing"), base: String::new(), lines: lines.iter().map(|s| s.to_string()).collect(), copy: String::new() }
    }

    #[test]
    fn asks_for_what_you_lack_and_beats_what_you_wear() {
        let worn = vec![
            g("Helmet", &["+60 to maximum Life", "+30% to Fire Resistance"]),
            g("Boots", &["+40 to maximum Life", "25% increased Movement Speed", "+40% to Cold Resistance"]),
            g("Ring 1", &["+45% to Fire Resistance", "+45% to Lightning Resistance"]),
            g("Amulet", &["+16% to all Elemental Resistances"]),
        ];
        let p = plan("Helmet", &worn, [0, 0, 20]);
        let names: Vec<&str> = p.wants.iter().map(|w| w.stat.as_str()).collect();
        assert_eq!(names, ["Life", "Fire res", "Cold res", "Lightning res", "Intelligence", "Chaos res"]);
        // 60 life + 30 fire.
        assert_eq!(p.now, 90.0);
        let q = query(&p, Some(68)).unwrap();
        assert_eq!(q["query"]["stats"][0]["type"], "weight");
        assert_eq!(q["query"]["stats"][0]["value"]["min"], 91.0);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "armour.helmet");
        assert_eq!(q["query"]["filters"]["req_filters"]["filters"]["lvl"]["max"], 68);
        let boots = plan("Boots", &worn, [0, 0, 0]);
        assert!(boots.wants.iter().any(|w| w.stat == "Movement speed"));
        // 40 life + 40 cold + 25 * 3 speed.
        assert_eq!(boots.now, 155.0);
        // All resistances counted in each one, like the trade site's totals.
        assert_eq!(score(&["+10% to all Elemental Resistances".to_string()], &p.wants), 30.0);
    }

    #[test]
    fn no_search_for_weapons() {
        assert!(query(&plan("Weapon 1", &[], [0, 0, 0]), None).is_none());
    }
}
