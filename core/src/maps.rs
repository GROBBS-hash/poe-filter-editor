//! Map check (Ctrl+D on a map): which of its mods are ones you said you can't run,
//! and a starting list of those suggested from your Path of Building build.

use crate::pob::Build;
use serde::Serialize;

/// Map mods that are a problem for many builds: (words the mod contains, short name).
/// Matching ignores capitals; every word group must be in the mod.
pub const MAP_DANGER: &[(&[&str], &str)] = &[
    (&["reflect", "elemental damage"], "Elemental reflect"),
    (&["reflect", "physical damage"], "Physical reflect"),
    (&["cannot regenerate"], "No regen"),
    (&["cannot leech"], "No leech"),
    (&["leeched from"], "No leech"),
    (&["maximum resistances"], "Less max res"),
    (&["recovery rate of life and energy shield"], "Less recovery"),
    (&["avoid elemental ailments"], "Monsters avoid ailments"),
    (&["effect of curses"], "Less curse effect"),
    (&["hexproof"], "Hexproof"),
    (&["cannot be stunned"], "Monsters can't be stunned"),
    (&["temporal chains"], "Temp chains"),
    (&["elemental weakness"], "Elemental weakness"),
    (&["enfeeble"], "Enfeeble"),
    (&["vulnerability"], "Vulnerability"),
    (&["extra maximum energy shield"], "Monster ES"),
    (&["burning ground"], "Burning ground"),
    (&["shocked ground"], "Shocked ground"),
    (&["chilled ground"], "Chilled ground"),
    (&["consecrated ground"], "Monster consecrated ground"),
    (&["poison on hit"], "Monsters poison"),
    (&["bleeding"], "Monsters bleed"),
    (&["monsters", "as extra"], "Extra monster damage"),
    (&["monsters", "critical strike"], "Monster crits"),
    (&["flask charges"], "Fewer flask charges"),
    (&["chance to block"], "Less block"),
    (&["less armour"], "Less armour"),
    (&["less area of effect"], "Less area of effect"),
    (&["additional projectiles"], "Extra monster projectiles"),
    (&["action speed cannot be modified"], "Monsters can't be slowed"),
    (&["steal power, frenzy and endurance"], "Monsters steal charges"),
    (&["buffs on players expire"], "Buffs expire faster"),
    (&["point blank"], "Point blank"),
    (&["blind on hit"], "Monsters blind"),
    (&["monsters", "accuracy"], "Monster accuracy"),
    (&["impale"], "Monsters impale"),
    (&["cooldown recovery"], "Less cooldown recovery"),
    (&["non-curse auras"], "Less aura effect"),
];

/// Every danger's short name, in list order (for Settings).
pub fn danger_names() -> Vec<&'static str> {
    let mut v: Vec<&str> = Vec::new();
    for (_, n) in MAP_DANGER { if !v.contains(n) { v.push(n); } }
    v
}

fn danger_of(text: &str) -> Option<&'static str> {
    let t = text.to_lowercase();
    MAP_DANGER.iter().find(|(words, _)| words.iter().all(|w| t.contains(w))).map(|(_, n)| *n)
}

/// One mod line of a map, and the warning name if it's one you can't run.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MapMod {
    pub text: String,
    pub warn: Option<String>,
}

/// What the price window shows for a map.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MapInfo {
    pub mods: Vec<MapMod>,
    pub unidentified: bool,
    pub tier: Option<u32>,
    /// Item quantity / pack size etc. ("Item Quantity: +80%").
    pub stats: Vec<String>,
    /// You can run it but not sell it (Kirac's Maven invitations): no price search.
    #[serde(default)]
    pub no_sale: bool,
}

/// Invitations and other map-device items with map mods (Maven's, Screaming, Incandescent…): you run them like a map.
pub fn runs_like_map(item_class: &str, base: &str) -> bool {
    item_class == "Misc Map Items" && base.contains("Invitation")
}

/// Is this item a map (PoE 1 maps, PoE 2 waystones)?
pub fn is_map(item_class: &str) -> bool {
    matches!(item_class, "Maps" | "Waystones" | "Memories")
}

/// Removes roll ranges "25(20-25)" -> "25" and drops the trailing "(implicit)" style tags.
fn tidy(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    let mut buf = String::new();
    for c in line.chars() {
        match c {
            '(' => { depth += 1; buf.clear(); }
            ')' if depth > 0 => {
                depth -= 1;
                let numeric = buf.chars().all(|c| c.is_ascii_digit() || "-.+ ".contains(c));
                let tag = ["implicit", "crafted", "enchant", "fractured", "augmented"].contains(&buf.trim());
                if !numeric && !tag { out.push('('); out.push_str(&buf); out.push(')'); }
            }
            _ if depth > 0 => buf.push(c),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads a copied map (plain Ctrl+C or advanced Ctrl+Alt+C). `avoid` = None warns about every known danger.
pub fn map_info(text: &str, avoid: Option<&[String]>) -> MapInfo {
    let text = text.replace('\r', "");
    let sections: Vec<Vec<&str>> = text.split("--------")
        .map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).collect()).collect();
    let mut info = MapInfo { mods: vec![], unidentified: false, tier: None, stats: vec![], no_sale: false };
    // Invitations have flavour text after their mods; in the advanced copy every mod section has a { … } heading.
    let invitation = sections.first().is_some_and(|s| s.contains(&"Item Class: Misc Map Items"));
    let headed = text.contains("\n{ ");
    let ilvl_at = sections.iter().position(|s| s.iter().any(|l| l.starts_with("Item Level:")));
    for (i, sec) in sections.iter().enumerate() {
        for l in sec {
            if let Some(t) = l.strip_prefix("Map Tier: ").or_else(|| l.strip_prefix("Waystone Tier: ")) {
                info.tier = t.trim().parse().ok();
            }
            // Quantity, pack size, more maps… sit above the item level ("Item Quantity: +80%").
            if ilvl_at.map(|a| i < a).unwrap_or(false) && l.contains(": ") && l.contains('%') && !l.starts_with("Map Tier") {
                info.stats.push(tidy(l));
            }
        }
        if *sec == ["Unidentified"] || sec.contains(&"Unidentified") { info.unidentified = true; }
        let Some(start) = ilvl_at else { continue };
        if i <= start { continue; }
        if invitation && headed && !sec.iter().any(|l| l.starts_with('{')) { continue; }
        // Not mods: the "how to use" text, flavour, corrupted/unidentified markers.
        if sec.iter().any(|l| l.starts_with("Travel to") || l.starts_with("Place into") || l.starts_with("Right click")
            || l.starts_with("Note:") || *l == "Corrupted" || *l == "Twice Corrupted" || *l == "Unidentified" || *l == "Mirrored" || l.starts_with("Can be used")) {
            continue;
        }
        for l in sec {
            if l.starts_with('{') || l.starts_with('(') || l.starts_with("Monster Level:") || l.starts_with("Area Level:") { continue; }
            let t = tidy(&l.replace(" — Unscalable Value", ""));
            if t.is_empty() || t.starts_with("Modifiers to Item Quantity") { continue; }
            let warn = danger_of(&t).filter(|n| avoid.map(|a| a.iter().any(|x| x == n)).unwrap_or(true)).map(String::from);
            info.mods.push(MapMod { text: t, warn });
        }
    }
    info
}

/// A suggested "can't run" mod and why.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Suggestion {
    pub name: String,
    pub why: String,
}

const CURSES: &[&str] = &["despair", "flammability", "frostbite", "conductivity", "vulnerability", "elemental weakness",
    "enfeeble", "temporal chains", "punishment", "assassin's mark", "poacher's mark", "warlord's mark", "sniper's mark", "hextouch", "blasphemy", "curse on hit"];
const MINION: &[&str] = &["summon", "raise ", "animate", "spectre", "zombie", "golem", "skeleton", "absolution", "dominating blow"];
const DOT: &[&str] = &["righteous fire", "essence drain", "contagion", "bane", "soulrend", "vortex", "toxic rain", "caustic arrow",
    "death aura", "blight", "scorching ray", "wither", "poisonous concoction"];
const ELEMENTAL: &[&str] = &["fire", "cold", "ice", "frost", "lightning", "spark", "arc", "flame", "glacial", "freez", "shock", "storm",
    "burn", "elemental", "volcanic", "winter", "cremation", "crackling", "galvanic", "hydrosphere", "comet", "orb of storms", "armageddon"];
const AILMENT: &[&str] = &["ignite", "burning damage", "unbound ailments", "deadly ailments", "combustion", "immolate", "elemental overload"];

/// Suggests the map mods to warn about, from the build's endgame stage (the last one).
pub fn suggest(build: &Build) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    let mut add = |name: &str, why: &str| if !out.iter().any(|s| s.name == name) { out.push(Suggestion { name: name.into(), why: why.into() }) };
    // Levelling stages are often listed last: the last one that isn't for the campaign and has gems, else the one PoB has open.
    let has_gems = |s: &&crate::pob::Stage| s.groups.iter().any(|g| g.enabled && !g.gems.is_empty());
    let Some(stage) = build.stages.iter().filter(has_gems).filter(|s| crate::pob::stage_act(&s.title).is_none_or(|a| a >= 11)).last()
        .or_else(|| build.stages.get(build.active).filter(|s| has_gems(s)))
        .or_else(|| build.stages.iter().filter(has_gems).last()).or_else(|| build.stages.last()) else { return out };
    let gems: Vec<String> = stage.groups.iter().filter(|g| g.enabled).flat_map(|g| g.gems.iter())
        .filter(|g| g.enabled).map(|g| g.name.to_lowercase()).collect();
    // Parts of a name only count from the start of a word; some only as the whole word ("Arcane Surge" isn't Arc,
    // "Shockwave" isn't Shock), and a damage-over-time skill only by its own name ("Blade Vortex" isn't Vortex).
    let word = |n: &str, k: &str| {
        let whole = matches!(k, "arc" | "ice" | "shock" | "spark" | "comet" | "cold");
        n.match_indices(k).any(|(i, _)| (i == 0 || !n.as_bytes()[i - 1].is_ascii_alphanumeric())
            && (!whole || n[i + k.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric())))
    };
    let has = |list: &[&str], names: &[String]| names.iter().any(|n| list.iter().any(|k| word(n, k)));
    let is = |list: &[&str], names: &[String]| names.iter().any(|n| {
        let n = n.strip_prefix("vaal ").unwrap_or(n);
        list.iter().any(|k| n == *k || n.strip_prefix(k).is_some_and(|r| r.starts_with(" of ")))
    });
    // Main skill: the biggest link group's active gems.
    let main = stage.groups.iter().filter(|g| g.enabled).max_by_key(|g| g.gems.len());
    let main_gems: Vec<String> = main.map(|g| g.gems.iter().filter(|x| x.enabled).map(|x| x.name.to_lowercase()).collect()).unwrap_or_default();
    let main_active: Vec<String> = main.map(|g| g.gems.iter().filter(|x| x.enabled && !x.support).map(|x| x.name.to_lowercase()).collect()).unwrap_or_default();
    let gear: String = stage.gear.iter().flat_map(|g| g.lines.iter()).map(|l| l.to_lowercase()).collect::<Vec<_>>().join("\n");

    add("Less max res", "Hurts every build: you take a lot more elemental damage");
    add("No regen", "Most builds need regen for mana (and life)");
    if has(MINION, &main_active) {
        // Minions take the reflected damage, not you.
    } else if is(DOT, &main_active) {
        // Damage over time isn't reflected.
    } else if has(ELEMENTAL, &main_gems) {
        add("Elemental reflect", "Your main skill hits with elemental damage");
    } else if !main_active.is_empty() {
        add("Physical reflect", "Your main skill hits with physical damage");
    } else {
        add("Elemental reflect", "Reflect kills most hit builds");
        add("Physical reflect", "Reflect kills most hit builds");
    }
    if gear.contains("leech") || gems.iter().any(|g| g.contains("leech")) {
        add("No leech", "Your build uses life or mana leech");
    }
    if has(CURSES, &gems) {
        add("Hexproof", "Your curses won't work on hexproof monsters");
        add("Less curse effect", "Your curses get much weaker");
    }
    if has(AILMENT, &gems) {
        add("Monsters avoid ailments", "Your build relies on ignite or other ailments");
    }
    if gear.contains("flask") || gems.iter().any(|g| g.contains("flask")) {
        add("Fewer flask charges", "Your build leans on flasks");
    }
    out
}

/// A map mod without its numbers, to tell whether two lines are the same mod. poe.re writes ranges "(20-40)%" where the
/// map says "40%": each "(a-b)" counts as one number.
pub fn shape(text: &str) -> String {
    let low = text.replace(" — Unscalable Value", "").to_lowercase();
    let mut out = String::new();
    let mut rest = low.as_str();
    while let Some(i) = rest.find('(') {
        let Some(j) = rest[i..].find(')').map(|j| i + j) else { break };
        let inner = &rest[i + 1..j];
        let numeric = !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.');
        out.push_str(&rest[..i]);
        if numeric { out.push('1') } else { out.push_str(&rest[i..=j]) }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    crate::trade::template(&out).0.replace('+', "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Ctrl+D map check against the one list of mods you can't run (the Maps page's ticks, poe.re's mod list):
/// a map mod line is flagged when it matches any line of a ticked mod, numbers ignored.
pub fn map_info_bad(text: &str, mods: &[crate::mapregex::MapModEntry], bad: &[i64]) -> MapInfo {
    let ticked: Vec<(String, String)> = mods.iter().filter(|m| bad.contains(&m.id)).flat_map(|m| {
        let name = m.danger.clone().unwrap_or_else(|| "can't run".into());
        m.text.split(" / ").map(move |part| (shape(part), name.clone())).collect::<Vec<_>>()
    }).collect();
    let mut info = map_info(text, Some(&[]));
    for m in &mut info.mods {
        let s = shape(&m.text);
        m.warn = ticked.iter().find(|(t, _)| *t == s).map(|(_, n)| n.clone());
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitations_are_checked_like_maps() {
        // 24 Nov, Kirac's "Fate Offer": priced as an item ("None listed"), never checked against the map mods.
        let t = "Item Class: Misc Map Items\r\nRarity: Rare\r\nFate Offer\r\nMaven's Invitation: The Forgotten\r\n--------\r\nAtlas Drops: +56% (augmented)\r\nItem Rarity: +34% (augmented)\r\nThe Maven must Witness: \r\nTwisted Distant Memory\r\n--------\r\nItem Level: 83\r\n--------\r\n{ Implicit Modifier }\r\nModifiers to Item Quantity will affect the number of encounter rewards dropped — Unscalable Value\r\n--------\r\n{ Prefix Modifier \"Impervious\" (Tier: 1) }\r\nMonsters have a 50% chance to avoid Poison, Impale, and Bleeding\r\n{ Suffix Modifier \"of Impotence\" (Tier: 1) }\r\nPlayers have 25% less Area of Effect\r\n{ Suffix Modifier \"of Toughness\" (Tier: 1) }\r\nMonsters take 36(36-40)% reduced Extra Damage from Critical Strikes\r\n--------\r\nFor those who must not endure it,\r\nthere may be found a perverse joy \r\n--------\r\nOpen portals to The Maven's Crucible by using this item in a personal Map Device.\r\n";
        assert!(runs_like_map("Misc Map Items", "Maven's Invitation: The Forgotten") && !runs_like_map("Misc Map Items", "Chronicle of Atzoatl"));
        let i = map_info(t, None);
        let mods: Vec<&str> = i.mods.iter().map(|m| m.text.as_str()).collect();
        assert_eq!(mods, ["Monsters have a 50% chance to avoid Poison, Impale, and Bleeding", "Players have 25% less Area of Effect", "Monsters take 36% reduced Extra Damage from Critical Strikes"]);
        assert_eq!(i.mods[1].warn.as_deref(), Some("Less area of effect"));
        assert_eq!(i.stats, ["Atlas Drops: +56%", "Item Rarity: +34%"]);
        let e = crate::mapregex::MapModEntry { id: 7, text: "Players have (15-25)% less Area of Effect".into(), token: String::new(), scary: 0, nightmare: false, trade_id: None, danger: None };
        assert_eq!(map_info_bad(t, &[e], &[7]).mods.iter().filter(|m| m.warn.is_some()).count(), 1);
    }

    #[test]
    fn warns_from_the_maps_page_list() {
        let e = |id: i64, text: &str| crate::mapregex::MapModEntry { id, text: text.into(), token: String::new(), scary: 0, nightmare: false,
            trade_id: None, danger: None };
        let mods = vec![e(1, "Monsters reflect 18% of Elemental Damage"), e(2, "Monsters Poison on Hit / Monsters have 100% increased Poison Duration"),
            e(3, "Players have (20-40)% reduced Chance to Block / Players have (20-30)% less Armour")];
        let t = "Item Class: Maps\nRarity: Rare\nX\nMap (Tier 14)\n--------\nItem Level: 83\n--------\nMonsters reflect 15% of Elemental Damage\nMonsters Poison on Hit\nArea has patches of Burning Ground\n--------\nTravel to this Map\n";
        let only_poison = map_info_bad(t, &mods, &[2]);
        let flagged: Vec<&str> = only_poison.mods.iter().filter(|m| m.warn.is_some()).map(|m| m.text.as_str()).collect();
        assert_eq!(flagged, ["Monsters Poison on Hit"], "numbers ignored, only ticked mods: {:?}", only_poison.mods);
        assert_eq!(map_info_bad(t, &mods, &[1, 2]).mods.iter().filter(|m| m.warn.is_some()).count(), 2);
        // poe.re's ranges "(20-40)%" match the map's "40%" (his Victory Compass).
        let t2 = "Item Class: Maps
Rarity: Rare
Victory Compass
Map (Tier 16)
--------
Item Level: 85
--------
Players have 30% less Armour
Players have 40% reduced Chance to Block
--------
Travel to this Map
";
        assert_eq!(map_info_bad(t2, &mods, &[3]).mods.iter().filter(|m| m.warn.is_some()).count(), 2);
    }
    use crate::pob::{Gem, Group, Stage};

    const MAP: &str = "Item Class: Maps
Rarity: Rare
Grim Haunt
Strand Map
--------
Map Tier: 16
Item Quantity: +80% (augmented)
Monster Pack Size: +30% (augmented)
--------
Item Level: 83
--------
Monster Level: 83
--------
{ Prefix Modifier \"Hexwarded\" (Tier: 1) }
60(55-60)% less effect of Curses on Monsters
{ Suffix Modifier \"of Exposure\" }
Players cannot Regenerate Life, Mana or Energy Shield
{ Prefix Modifier \"Fleet\" }
Monsters have 25(20-25)% increased Movement Speed
Monsters reflect 18% of Elemental Damage
--------
Travel to this Map by using it in a personal Map Device. Maps can only be used once.
";

    #[test]
    fn reads_map_and_flags() {
        let m = map_info(MAP, None);
        assert_eq!(m.tier, Some(16));
        assert_eq!(m.stats, ["Item Quantity: +80%", "Monster Pack Size: +30%"]);
        let t: Vec<(&str, Option<&str>)> = m.mods.iter().map(|x| (x.text.as_str(), x.warn.as_deref())).collect();
        assert_eq!(t, [
            ("60% less effect of Curses on Monsters", Some("Less curse effect")),
            ("Players cannot Regenerate Life, Mana or Energy Shield", Some("No regen")),
            ("Monsters have 25% increased Movement Speed", None),
            ("Monsters reflect 18% of Elemental Damage", Some("Elemental reflect")),
        ]);
        let only = map_info(MAP, Some(&["No regen".to_string()]));
        assert_eq!(only.mods.iter().filter(|x| x.warn.is_some()).count(), 1);
        let unid = map_info("Item Class: Maps\nRarity: Rare\nStrand Map\n--------\nMap Tier: 16\n--------\nItem Level: 83\n--------\nUnidentified\n--------\nTravel to this Map\n", None);
        assert!(unid.unidentified && unid.mods.is_empty());
        assert!(is_map("Maps") && !is_map("Rings"));
        assert!(danger_names().contains(&"Hexproof"));
    }

    #[test]
    fn suggestions_go_by_whole_names() {
        let gem = |n: &str, s: bool| Gem { name: n.into(), level: 20, quality: 0, enabled: true, support: s, colour: "b".into() };
        let group = |g: Vec<Gem>| Group { label: String::new(), slot: String::new(), enabled: true, gems: g };
        let stage = |t: &str, g: Vec<Gem>| Stage { title: t.into(), tree: None, gear: vec![], jewels: vec![], groups: if g.is_empty() { vec![] } else { vec![group(g)] } };
        let names = |stages: Vec<Stage>| suggest(&Build { class: "Scion".into(), ascendancy: String::new(), level: 90, active: 0, stages }).into_iter().map(|s| s.name).collect::<Vec<_>>();
        // Blade Vortex isn't Vortex, and Arcane Surge isn't Arc.
        let n = names(vec![stage("Endgame", vec![gem("Blade Vortex", false), gem("Arcane Surge", true), gem("Unleash", true)])]);
        assert!(n.contains(&"Physical reflect".to_string()) && !n.contains(&"Elemental reflect".to_string()), "{n:?}");
        let n = names(vec![stage("Endgame", vec![gem("Vortex of Projection", false)])]);
        assert!(!n.iter().any(|x| x.contains("reflect")), "{n:?}");
        // Levelling stages listed last don't decide.
        let n = names(vec![stage("Red Maps", vec![gem("Frostbolt", false)]), stage("Levelling 1", vec![gem("Cleave", false)]), stage("Levelling 2", vec![])]);
        assert!(n.contains(&"Elemental reflect".to_string()) && !n.contains(&"Physical reflect".to_string()), "{n:?}");
    }

    #[test]
    fn suggests_from_build() {
        let gem = |n: &str, s: bool| Gem { name: n.into(), level: 20, quality: 0, enabled: true, support: s, colour: "b".into() };
        let b = Build { class: "Witch".into(), ascendancy: "Elementalist".into(), level: 90, active: 0, stages: vec![Stage {
            title: "Endgame".into(), tree: None, gear: vec![], jewels: vec![],
            groups: vec![
                Group { label: String::new(), slot: String::new(), enabled: true,
                    gems: vec![gem("Arc", false), gem("Spell Echo", true), gem("Lightning Penetration", true)] },
                Group { label: String::new(), slot: String::new(), enabled: true, gems: vec![gem("Conductivity", false)] },
            ],
        }]};
        let names: Vec<String> = suggest(&b).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"Elemental reflect".to_string()));
        assert!(!names.contains(&"Physical reflect".to_string()));
        assert!(names.contains(&"Hexproof".to_string()));
        assert!(names.iter().all(|n| danger_names().contains(&n.as_str())));
    }
}
