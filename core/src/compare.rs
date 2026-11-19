//! "Is this an upgrade?": an item you price check against the gear in the same
//! slot of your Path of Building build. Adds up the stats people compare by
//! (life, resistances, attributes, speeds…) on both and shows what changes, plus
//! the mods only one of them has. Not a full PoB calculation: a quick read.

use crate::pob::Gear;
use crate::trade::template;
use serde::Serialize;

/// (what to call it, mod templates in lower case with how much each point counts).
const STATS: &[(&str, &[(&str, f64)])] = &[
    ("Life", &[("+# to maximum life", 1.0), ("+# to strength", 0.5), ("+# to all attributes", 0.5),
        ("+# to strength and intelligence", 0.5), ("+# to strength and dexterity", 0.5)]),
    ("Energy Shield", &[("+# to maximum energy shield", 1.0)]),
    ("Mana", &[("+# to maximum mana", 1.0), ("+# to intelligence", 0.5), ("+# to all attributes", 0.5),
        ("+# to strength and intelligence", 0.5), ("+# to dexterity and intelligence", 0.5)]),
    ("Fire res", &[("+#% to fire resistance", 1.0), ("+#% to all elemental resistances", 1.0), ("+#% to all resistances", 1.0),
        ("+#% to fire and cold resistances", 1.0), ("+#% to fire and lightning resistances", 1.0), ("+#% to fire and chaos resistances", 1.0)]),
    ("Cold res", &[("+#% to cold resistance", 1.0), ("+#% to all elemental resistances", 1.0), ("+#% to all resistances", 1.0),
        ("+#% to fire and cold resistances", 1.0), ("+#% to cold and lightning resistances", 1.0), ("+#% to cold and chaos resistances", 1.0)]),
    ("Lightning res", &[("+#% to lightning resistance", 1.0), ("+#% to all elemental resistances", 1.0), ("+#% to all resistances", 1.0),
        ("+#% to fire and lightning resistances", 1.0), ("+#% to cold and lightning resistances", 1.0), ("+#% to lightning and chaos resistances", 1.0)]),
    ("Chaos res", &[("+#% to chaos resistance", 1.0), ("+#% to all resistances", 1.0), ("+#% to fire and chaos resistances", 1.0),
        ("+#% to cold and chaos resistances", 1.0), ("+#% to lightning and chaos resistances", 1.0)]),
    ("Strength", &[("+# to strength", 1.0), ("+# to all attributes", 1.0), ("+# to strength and intelligence", 1.0), ("+# to strength and dexterity", 1.0)]),
    ("Dexterity", &[("+# to dexterity", 1.0), ("+# to all attributes", 1.0), ("+# to strength and dexterity", 1.0), ("+# to dexterity and intelligence", 1.0)]),
    ("Intelligence", &[("+# to intelligence", 1.0), ("+# to all attributes", 1.0), ("+# to strength and intelligence", 1.0), ("+# to dexterity and intelligence", 1.0)]),
    ("Movement speed", &[("#% increased movement speed", 1.0)]),
    ("Spell suppression", &[("+#% chance to suppress spell damage", 1.0)]),
    ("Attack speed", &[("#% increased attack speed", 1.0)]),
    ("Cast speed", &[("#% increased cast speed", 1.0)]),
    ("Crit multiplier", &[("+#% to global critical strike multiplier", 1.0)]),
    ("Spell damage", &[("#% increased spell damage", 1.0)]),
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Row {
    pub stat: String,
    /// On your current gear in that slot.
    pub now: f64,
    /// On the item you're checking.
    pub this: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Comparison {
    /// PoB's slot ("Ring 1"), and the item there now.
    pub slot: String,
    pub current: String,
    pub rows: Vec<Row>,
    /// Mods only the checked item has (not counted above), and ones you'd lose.
    pub gain: Vec<String>,
    pub lose: Vec<String>,
}

/// PoB slots an item of this class goes in.
pub fn slots_for(item_class: &str) -> &'static [&'static str] {
    match item_class {
        "Helmets" => &["Helmet"], "Body Armours" => &["Body Armour"], "Gloves" => &["Gloves"], "Boots" => &["Boots"],
        "Belts" => &["Belt"], "Amulets" => &["Amulet"], "Rings" => &["Ring 1", "Ring 2"],
        "Shields" | "Quivers" => &["Weapon 2"],
        "Wands" | "Sceptres" | "Staves" | "Warstaves" | "Daggers" | "Rune Daggers" | "Claws" | "Bows" | "One Hand Swords"
        | "Thrusting One Hand Swords" | "Two Hand Swords" | "One Hand Axes" | "Two Hand Axes" | "One Hand Maces" | "Two Hand Maces" => &["Weapon 1"],
        _ => &[],
    }
}

/// Lines that are item details, not mods ("Quality: 20", "Sockets: R-G").
fn is_mod_line(l: &str) -> bool {
    let l = l.trim();
    !l.is_empty() && l.chars().any(|c| c.is_ascii_digit() || c.is_alphabetic())
        && !(l.contains(": ") && !l.contains('%') && l.split(": ").next().map(|k| !k.contains(' ') || k.len() < 20).unwrap_or(false))
}

/// Each tracked stat's total over these mod lines, and the lines no stat counted.
fn add_up(lines: &[String]) -> (Vec<f64>, Vec<String>) {
    let mut totals = vec![0.0; STATS.len()];
    let mut other = Vec::new();
    for l in lines.iter().filter(|l| is_mod_line(l)) {
        let (t, v) = template(l);
        let t = t.to_lowercase();
        let mut used = false;
        for (i, (_, srcs)) in STATS.iter().enumerate() {
            if let Some((_, times)) = srcs.iter().find(|(s, _)| *s == t) {
                totals[i] += v.first().copied().unwrap_or(0.0) * times;
                used = true;
            }
        }
        if !used { other.push(l.trim().to_string()); }
    }
    (totals, other)
}

/// The checked item's mod lines against the gear in each slot it could go in.
pub fn compare(this_lines: &[String], item_class: &str, gear: &[Gear]) -> Vec<Comparison> {
    let (mine, my_other) = add_up(this_lines);
    slots_for(item_class).iter().filter_map(|slot| {
        let g = gear.iter().find(|g| g.slot == *slot)?;
        let (now, their_other) = add_up(&g.lines);
        let rows = STATS.iter().enumerate()
            .filter(|(i, _)| (mine[*i] - now[*i]).abs() >= 0.5)
            .map(|(i, (name, _))| Row { stat: name.to_string(), now: now[i].floor(), this: mine[i].floor() })
            .collect();
        let tmpl = |s: &String| template(s).0.to_lowercase();
        let gain = my_other.iter().filter(|l| !their_other.iter().any(|o| tmpl(o) == tmpl(l))).cloned().collect();
        let lose = their_other.iter().filter(|l| !my_other.iter().any(|o| tmpl(o) == tmpl(l))).cloned().collect();
        Some(Comparison { slot: slot.to_string(), current: if g.rarity == "unique" || g.name != g.base { g.name.clone() } else { g.base.clone() },
            rows, gain, lose })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gear(slot: &str, name: &str, lines: &[&str]) -> Gear {
        Gear { slot: slot.into(), rarity: "rare".into(), name: name.into(), base: "Two-Stone Ring".into(), lines: lines.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn ring_against_both_rings() {
        let gear = vec![
            gear("Ring 1", "Old Loop", &["+12% to Fire and Cold Resistances", "+50 to maximum Life", "+30 to Strength", "Quality: 20"]),
            gear("Ring 2", "Other Loop", &["+70 to maximum Life", "Adds 5 to 10 Fire Damage to Attacks"]),
            gear("Helmet", "Hat", &["+90 to maximum Life"]),
        ];
        let this = vec!["+16% to Fire and Cold Resistances".to_string(), "+80 to maximum Life".into(), "+40% to Lightning Resistance".into(),
            "Adds 5 to 10 Fire Damage to Attacks".into()];
        let c = compare(&this, "Rings", &gear);
        assert_eq!(c.len(), 2);
        let r1 = &c[0];
        assert_eq!(r1.slot, "Ring 1");
        let row = |c: &Comparison, s: &str| c.rows.iter().find(|r| r.stat == s).map(|r| (r.now, r.this));
        assert_eq!(row(r1, "Life"), Some((65.0, 80.0))); // 50 + 30 Str / 2
        assert_eq!(row(r1, "Fire res"), Some((12.0, 16.0)));
        assert_eq!(row(r1, "Lightning res"), Some((0.0, 40.0)));
        assert_eq!(row(r1, "Strength"), Some((30.0, 0.0)));
        assert_eq!(r1.gain, ["Adds 5 to 10 Fire Damage to Attacks"]);
        assert!(r1.lose.is_empty(), "Quality line isn't a mod");
        assert!(c[1].gain.is_empty());
        assert!(compare(&this, "Jewels", &gear).is_empty());
    }
}
