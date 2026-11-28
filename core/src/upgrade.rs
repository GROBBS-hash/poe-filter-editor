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
    /// A rare or magic attack weapon: searched by damage per second instead of a weighted sum (28 Nov 2026).
    #[serde(default)]
    pub dps: Option<Dps>,
}

/// A weapon's damage per second, worked out like the trade site does (base damage from the game's data, the item's
/// local mods and quality).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Dps {
    pub pdps: f64,
    pub edps: f64,
    /// Physical + elemental + chaos.
    pub total: f64,
    pub aps: f64,
    /// What the search beats: "pdps", "edps" or "dps" (the trade site's weapon filters). Picked by what most of
    /// the damage is; the page can switch it.
    pub kind: String,
    /// The trade site's category for this kind of weapon ("weapon.wand").
    pub category: String,
}

impl Dps {
    /// The number for `kind`.
    pub fn value(&self, kind: &str) -> f64 { match kind { "pdps" => self.pdps, "edps" => self.edps, _ => self.total } }
}

/// Weapon bases: (physical min, max, attack time ms, item class), from core/data/poe1_weapons.json (tools/build_weapons.mjs).
fn weapons() -> &'static std::collections::HashMap<String, (f64, f64, f64, String)> {
    static D: std::sync::OnceLock<std::collections::HashMap<String, (f64, f64, f64, String)>> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        let v: std::collections::HashMap<String, Vec<Value>> = serde_json::from_str(include_str!("../data/poe1_weapons.json")).unwrap_or_default();
        v.into_iter().filter_map(|(k, a)| Some((k, (a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?, a.get(4)?.as_str()?.to_string())))).collect()
    })
}

fn weapon_category(class: &str) -> Option<&'static str> {
    Some(match class {
        "One Hand Sword" | "Thrusting One Hand Sword" => "weapon.onesword", "One Hand Axe" => "weapon.oneaxe", "One Hand Mace" => "weapon.onemace",
        "Sceptre" => "weapon.sceptre", "Claw" => "weapon.claw", "Dagger" => "weapon.dagger", "Rune Dagger" => "weapon.runedagger",
        "Wand" => "weapon.wand", "Bow" => "weapon.bow", "Staff" => "weapon.staff", "Warstaff" => "weapon.warstaff",
        "Two Hand Sword" => "weapon.twosword", "Two Hand Axe" => "weapon.twoaxe", "Two Hand Mace" => "weapon.twomace",
        _ => return None,
    })
}

/// The numbers in a line ("Adds 12 to 24 Fire Damage" -> [12, 24]).
fn nums(l: &str) -> Vec<f64> {
    l.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|s| !s.is_empty() && *s != ".").filter_map(|s| s.parse().ok()).collect()
}

/// A rare / magic / normal attack weapon's DPS. Uniques are left out: they're picked for what they do, and many
/// scale with your level in ways a search can't match.
pub fn dps_of(g: &Gear) -> Option<Dps> {
    if g.rarity.eq_ignore_ascii_case("unique") { return None; }
    let w = weapons();
    let (min, max, time, class) = w.get(g.base.as_str()).or_else(|| w.iter().filter(|(k, _)| g.base.contains(k.as_str())).max_by_key(|(k, _)| k.len()).map(|x| x.1))?;
    let category = weapon_category(class)?;
    let (mut flat_p, mut inc_p, mut ias, mut q, mut ele, mut chaos) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for l in &g.lines {
        let low = l.to_lowercase();
        let n = nums(&low);
        if low.starts_with("quality:") { q = n.first().copied().unwrap_or(0.0); continue; }
        // Local mods only: "Adds 10 to 20 Physical Damage" (not "... to Attacks", "... to Spells").
        if low.starts_with("adds ") && low.ends_with(" damage") && n.len() == 2 {
            let avg = (n[0] + n[1]) / 2.0;
            if low.contains("physical") { flat_p += avg } else if low.contains("chaos") { chaos += avg }
            else if low.contains("fire") || low.contains("cold") || low.contains("lightning") { ele += avg }
            continue;
        }
        if low.ends_with("% increased physical damage") { inc_p += n.first().copied().unwrap_or(0.0); }
        if low.ends_with("% increased attack speed") { ias += n.first().copied().unwrap_or(0.0); }
    }
    // The trade site counts every weapon at 20% quality ("dps_aug" on its listings, checked 28 Nov 2026), so yours too.
    let q = q.max(20.0);
    let aps = 1000.0 / time * (1.0 + ias / 100.0);
    let pdps = ((min + max) / 2.0 + flat_p) * (1.0 + (inc_p + q) / 100.0) * aps;
    let edps = ele * aps;
    let total = pdps + edps + chaos * aps;
    let kind = if pdps >= total * 0.67 { "pdps" } else if edps >= total * 0.67 { "edps" } else { "dps" };
    let r = |x: f64| (x * 10.0).round() / 10.0;
    Some(Dps { pdps: r(pdps), edps: r(edps), total: r(total), aps: (aps * 100.0).round() / 100.0, kind: kind.into(), category: category.into() })
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
    if slot.starts_with("Weapon") {
        if let Some(d) = here.and_then(dps_of) {
            return Plan { slot: slot.into(), current: here.map(|g| g.name.clone()).unwrap_or_default(), wants: vec![], now: d.value(&d.kind), not_counted: vec![], dps: Some(d) };
        }
    }
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
    Plan { slot: slot.into(), current: here.map(|g| g.name.clone()).unwrap_or_default(), wants, now, not_counted, dps: None }
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
    if let Some(d) = &p.dps {
        // A weapon: the same kind, more damage per second than yours.
        let mut q = json!({
            "query": {
                "status": { "option": "securable" },
                "filters": {
                    "type_filters": { "filters": { "category": { "option": d.category } } },
                    "weapon_filters": { "filters": { d.kind.as_str(): { "min": (d.value(&d.kind) + 1.0).floor() } } },
                },
            },
            "sort": { "price": "asc" },
        });
        if let Some(l) = max_level { q["query"]["filters"]["req_filters"] = json!({ "filters": { "lvl": { "max": l } } }); }
        return Some(q);
    }
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
    fn weapons_by_damage() {
        // A rare Imbued Wand (27-50, 1.6 a second): 20% quality, 30% increased physical, adds 10-20 physical and
        // 5-15 fire, 10% attack speed.
        let w = Gear { slot: "Weapon 1".into(), rarity: "rare".into(), name: "Doom Song".into(), base: "Imbued Wand".into(), copy: String::new(),
            lines: ["Quality: 20", "Adds 10 to 20 Physical Damage", "Adds 5 to 15 Fire Damage", "30% increased Physical Damage", "10% increased Attack Speed",
                "Adds 2 to 4 Cold Damage to Spells"].iter().map(|s| s.to_string()).collect() };
        let d = dps_of(&w).unwrap();
        // aps 1.6 * 1.1 = 1.76; phys (38.5 + 15) * 1.5 = 80.25 -> 141.2; fire 10 * 1.76 = 17.6.
        assert_eq!((d.aps, d.pdps, d.edps, d.total), (1.76, 141.2, 17.6, 158.8));
        assert_eq!((d.kind.as_str(), d.category.as_str()), ("pdps", "weapon.wand"));
        let p = plan("Weapon 1", &[w.clone()], [0, 0, 0]);
        assert!(p.wants.is_empty());
        assert_eq!(p.now, 141.2);
        let q = query(&p, Some(68)).unwrap();
        assert_eq!(q["query"]["filters"]["weapon_filters"]["filters"]["pdps"]["min"], 142.0);
        assert_eq!(q["query"]["filters"]["type_filters"]["filters"]["category"]["option"], "weapon.wand");
        assert_eq!(q["query"]["filters"]["req_filters"]["filters"]["lvl"]["max"], 68);
        // Uniques and shields aren't searched by damage.
        assert!(dps_of(&Gear { rarity: "unique".into(), ..w.clone() }).is_none());
        assert!(dps_of(&Gear { base: "Archon Kite Shield".into(), ..w }).is_none());
    }

    #[test]
    fn plans_from_a_build_stage() {
        // My build's Upgrades improve on the shown stage's gear: PoB's slot names must match.
        let code = include_str!("../data/test_pob_kf_fixed.txt");
        let b = crate::pob::parse(&crate::pob::decode(code.trim()).unwrap()).unwrap();
        let st = b.stages.iter().find(|s| s.title.contains("50")).unwrap();
        let slots: Vec<&str> = st.gear.iter().map(|g| g.slot.as_str()).collect();
        let plans: Vec<Plan> = ["Helmet", "Body Armour", "Gloves", "Boots", "Belt", "Amulet", "Ring 1", "Ring 2"].iter()
            .filter(|s| slots.contains(s)).map(|s| plan(s, &st.gear, [0, 0, 0])).collect();
        assert!(plans.len() >= 6, "{slots:?}");
        assert!(plans.iter().any(|p| p.now > 0.0));
    }

    #[test]
    fn no_search_for_weapons() {
        assert!(query(&plan("Weapon 1", &[], [0, 0, 0]), None).is_none());
    }
}
