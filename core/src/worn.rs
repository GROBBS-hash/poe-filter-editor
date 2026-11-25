//! "Better than mine?": the gear you're wearing, from your character on pathofexile.com (the same reply the map
//! tracker reads your bags from) or, without a login, kept by hand with "I'm wearing this" in the price window.

use crate::compare::{self, Comparison};
use crate::pob::Gear;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One thing you're wearing.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Piece {
    /// PoB's slot names ("Ring 1", "Weapon 2").
    pub slot: String,
    pub rarity: String,
    pub name: String,
    pub base: String,
    /// As the game's Ctrl+C would copy it.
    pub text: String,
    pub icon: String,
    pub class: String,
}

/// What you were wearing when it was last read.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Worn {
    pub character: String,
    /// Unix seconds: when the site saved it (your last zone change), or when you said so.
    pub at: i64,
    pub poe2: bool,
    /// Kept by hand: not from the site.
    pub by_hand: bool,
    pub pieces: Vec<Piece>,
}

/// The site's inventory names as PoB's slots. Weapon swap, flasks and the like aren't compared.
pub fn slot_of(inventory: &str, x: u64) -> Option<&'static str> {
    Some(match inventory {
        "Helm" => "Helmet", "BodyArmour" => "Body Armour", "Gloves" => "Gloves", "Boots" => "Boots", "Belt" => "Belt",
        "Amulet" => "Amulet", "Ring" => if x == 1 { "Ring 2" } else { "Ring 1" }, "Ring2" => "Ring 2", "Ring3" => "Ring 3",
        "Weapon" => "Weapon 1", "Offhand" => "Weapon 2",
        _ => return None,
    })
}

/// Everything worn in a character reply's items.
pub fn from_items(items: &Value) -> Vec<Piece> {
    items.as_array().into_iter().flatten().filter_map(|i| {
        let slot = slot_of(i["inventoryId"].as_str()?, i["x"].as_u64().unwrap_or(0))?;
        let text = crate::wealth::item_text(i);
        let base = i["baseType"].as_str().or(i["typeLine"].as_str()).unwrap_or("").to_string();
        let name = i["name"].as_str().filter(|n| !n.is_empty()).unwrap_or(i["typeLine"].as_str().unwrap_or("")).to_string();
        let rarity = match i["frameType"].as_u64() { Some(0) => "normal", Some(1) => "magic", Some(3) | Some(9) => "unique", _ => "rare" };
        Some(Piece { slot: slot.into(), rarity: rarity.into(), class: class_of(&text), name, base, text, icon: i["icon"].as_str().unwrap_or("").to_string() })
    }).collect()
}

fn class_of(text: &str) -> String {
    text.lines().find_map(|l| l.trim().strip_prefix("Item Class: ")).unwrap_or("").trim().to_string()
}

const TWO_HANDED: [&str; 6] = ["Two Hand Swords", "Two Hand Axes", "Two Hand Maces", "Staves", "Warstaves", "Bows"];

impl Worn {
    /// As the gear the comparison reads.
    pub fn gear(&self) -> Vec<Gear> {
        self.pieces.iter().map(|p| Gear {
            slot: p.slot.clone(), rarity: p.rarity.clone(), name: p.name.clone(), base: p.base.clone(),
            lines: crate::trade::mods(&p.text).into_iter().map(|m| m.text).collect(), copy: p.text.clone(),
        }).collect()
    }

    /// "I'm wearing this": the item you checked takes that slot.
    pub fn wear(&mut self, text: &str, slot: &str, name: &str, base: &str, rarity: &str, at: i64) {
        let p = Piece { slot: slot.into(), rarity: rarity.to_lowercase(), name: name.into(), base: base.into(), text: text.into(), class: class_of(text), icon: String::new() };
        self.pieces.retain(|x| x.slot != slot);
        self.pieces.push(p);
        self.at = at;
    }

    /// Where an item of this class could go: its slots, and a weapon's off hand too when you wield two.
    fn slots(&self, class: &str) -> Vec<&'static str> {
        let mut s: Vec<&'static str> = compare::slots_for(class).to_vec();
        if s == ["Weapon 1"] && !TWO_HANDED.contains(&class)
            && self.pieces.iter().any(|p| p.slot == "Weapon 2" && compare::slots_for(&p.class) == ["Weapon 1"]) {
            s.push("Weapon 2");
        }
        if s.contains(&"Ring 1") && self.pieces.iter().any(|p| p.slot == "Ring 3") { s.push("Ring 3"); }
        s
    }

    /// The checked item against what you wear, the best swap first.
    pub fn compare(&self, text: &str, class: &str) -> Vec<Comparison> {
        let lines: Vec<String> = crate::trade::mods(text).into_iter().map(|m| m.text).collect();
        let mut out = compare::compare_slots(&lines, text, &self.slots(class), &self.gear());
        // What you're wearing already isn't an upgrade on itself.
        out.retain(|c| !self.pieces.iter().any(|p| p.slot == c.slot && same_item(&p.text, text)));
        for c in &mut out {
            c.worn = true;
            c.icon = self.pieces.iter().find(|p| p.slot == c.slot).map(|p| p.icon.clone()).unwrap_or_default();
            // A two-hander takes the place of both (a bow keeps its quiver).
            if TWO_HANDED.contains(&class) && c.slot == "Weapon 1" {
                if let Some(off) = self.pieces.iter().find(|p| p.slot == "Weapon 2" && !(class == "Bows" && p.class == "Quivers")) {
                    c.note = format!("Needs both hands: you'd also lose {}", off.name);
                }
            }
            if !self.poe2 { c.resists = self.resists_after(&lines, &c.slot); }
        }
        let score = |c: &Comparison| c.rows.iter().map(|r| if r.this > r.now { 1 } else { -1 }).sum::<i32>();
        out.sort_by_key(|c| -score(c));
        out
    }

    /// Fire, cold, lightning and chaos from all your gear: now, and with the checked item in that slot.
    fn resists_after(&self, lines: &[String], slot: &str) -> Vec<compare::Resist> {
        let gear = self.gear();
        let total = |skip: Option<&str>| compare::resists(&gear.iter().filter(|g| Some(g.slot.as_str()) != skip).flat_map(|g| g.lines.iter().cloned()).collect::<Vec<_>>());
        let (now, rest, this) = (total(None), total(Some(slot)), compare::resists(lines));
        compare::RESISTS.iter().enumerate().map(|(i, n)| compare::Resist { name: n.to_string(), now: now[i].floor(), after: (rest[i] + this[i]).floor() })
            .filter(|r| r.now != r.after).collect()
    }
}

/// The same item, whatever way it was copied (advanced copy or the site's).
fn same_item(a: &str, b: &str) -> bool {
    let key = |t: &str| {
        let mut l: Vec<String> = crate::trade::mods(t).into_iter().map(|m| m.text.to_lowercase()).collect();
        l.sort();
        l
    };
    let name = |t: &str| t.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("Item Class:") && !l.starts_with("Rarity:")).unwrap_or("").to_string();
    name(a) == name(b) && !key(a).is_empty() && key(a) == key(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn items() -> Value {
        json!([
            { "inventoryId": "Ring", "frameType": 2, "identified": true, "name": "Old Loop", "typeLine": "Two-Stone Ring", "baseType": "Two-Stone Ring", "ilvl": 80, "icon": "r1.png",
              "implicitMods": ["+16% to Fire and Cold Resistances"], "explicitMods": ["+70 to maximum Life", "+30% to Cold Resistance"] },
            { "inventoryId": "Ring2", "frameType": 2, "identified": true, "name": "Bad Loop", "typeLine": "Iron Ring", "baseType": "Iron Ring", "ilvl": 80,
              "explicitMods": ["+10 to maximum Life"] },
            { "inventoryId": "Weapon", "frameType": 2, "identified": true, "name": "Grim Edge", "typeLine": "Ambusher", "baseType": "Ambusher", "ilvl": 80,
              "properties": [{ "name": "Physical Damage", "values": [["50-150", 1]] }, { "name": "Attacks per Second", "values": [["1.50", 0]] }],
              "explicitMods": ["20% increased Attack Speed"] },
            { "inventoryId": "Offhand", "frameType": 2, "identified": true, "name": "Doom Ward", "typeLine": "Titanium Spirit Shield", "baseType": "Titanium Spirit Shield", "ilvl": 80,
              "explicitMods": ["+50 to maximum Life"] },
            { "inventoryId": "MainInventory", "frameType": 5, "typeLine": "Chaos Orb" },
            { "inventoryId": "Flask", "frameType": 3, "name": "Dying Sun", "typeLine": "Ruby Flask" }
        ])
    }

    const RING: &str = "Item Class: Rings\nRarity: Rare\nNew Loop\nTopaz Ring\n--------\nItem Level: 84\n--------\n+30% to Lightning Resistance (implicit)\n--------\n+60 to maximum Life\n+10% to Cold Resistance\n";

    #[test]
    fn worn_gear_is_compared_best_swap_first() {
        let w = Worn { pieces: from_items(&items()), ..Default::default() };
        assert_eq!(w.pieces.iter().map(|p| p.slot.as_str()).collect::<Vec<_>>(), ["Ring 1", "Ring 2", "Weapon 1", "Weapon 2"]);
        let c = w.compare(RING, "Rings");
        assert_eq!(c.iter().map(|x| x.slot.as_str()).collect::<Vec<_>>(), ["Ring 2", "Ring 1"], "replacing the bad ring is the better swap");
        assert!(c.iter().all(|x| x.worn));
        assert_eq!(c[1].icon, "r1.png");
        let cold = c[1].resists.iter().find(|r| r.name == "Cold").unwrap();
        assert_eq!((cold.now, cold.after), (46.0, 10.0));
        assert!(w.compare(&w.pieces[0].text, "Rings").iter().all(|x| x.slot != "Ring 1"), "not against itself");
    }

    #[test]
    fn weapons_compare_damage_and_hands() {
        let w = Worn { pieces: from_items(&items()), ..Default::default() };
        let staff = "Item Class: Warstaves\nRarity: Rare\nDoom Spire\nEzomyte Staff\n--------\nWarstaff\nPhysical Damage: 100-300 (augmented)\nAttacks per Second: 1.20\n--------\nItem Level: 84\n--------\n+1 to maximum Mana\n";
        let c = w.compare(staff, "Warstaves");
        let dps = c[0].rows.iter().find(|r| r.stat == "Weapon DPS").unwrap();
        assert_eq!((dps.now, dps.this), (150.0, 240.0));
        assert!(c[0].note.contains("Doom Ward"));
        assert_eq!(w.compare(&staff.replace("Warstaves", "Wands"), "Wands").len(), 1, "a shield isn't replaced by a wand");
    }

    #[test]
    fn kept_by_hand() {
        let mut w = Worn::default();
        w.wear(RING, "Ring 1", "New Loop", "Topaz Ring", "Rare", 5);
        w.wear(RING, "Ring 1", "New Loop", "Topaz Ring", "Rare", 6);
        assert_eq!((w.pieces.len(), w.at), (1, 6));
        assert!(w.compare(RING, "Rings").is_empty());
    }
}
