//! Levelling gear helper: uniques you can wear at your level that help your
//! main skill, best matches first, with a trade link for each.
//! Data built by tools/build_levelling_data.py (Path of Building + RePoE).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Unique {
    pub name: String,
    pub base: String,
    pub class: String,
    pub level: u32,
    pub mods: Vec<String>,
    pub stats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gem {
    pub tags: Vec<String>,
    pub stats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatInfo { pub id: String, pub name: String }

pub fn uniques() -> &'static Vec<Unique> {
    static D: OnceLock<Vec<Unique>> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_uniques.json")).unwrap_or_default())
}

pub fn gems() -> &'static HashMap<String, Gem> {
    static D: OnceLock<HashMap<String, Gem>> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/poe1_gems.json")).unwrap_or_default())
}

/// "Fireball of Radiation": a gem whose name is another gem's plus "of …" (not Herald of Ash).
pub fn is_transfigured(name: &str) -> bool {
    name.split_once(" of ").is_some_and(|(g, _)| gems().contains_key(g))
}

pub fn stat_list() -> &'static Vec<StatInfo> {
    static D: OnceLock<Vec<StatInfo>> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/levelling_stats.json")).unwrap_or_default())
}

/// Equipment slot a class goes in (for grouping).
pub fn slot_of(class: &str) -> &'static str {
    match class {
        "Helmets" => "Helmet", "Body Armours" => "Body", "Gloves" => "Gloves", "Boots" => "Boots",
        "Belts" => "Belt", "Rings" => "Ring", "Amulets" => "Amulet", "Shields" | "Quivers" => "Off hand",
        "Life Flasks" | "Mana Flasks" | "Hybrid Flasks" | "Utility Flasks" => "Flask",
        "Jewels" | "Abyss Jewels" => "Jewel",
        c if c.contains("Hand") || ["Bows", "Claws", "Daggers", "Rune Daggers", "Sceptres", "Staves", "Wands", "Warstaves"].contains(&c) => "Weapon",
        _ => "Other",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Pick {
    pub unique: Unique,
    pub slot: &'static str,
    /// Stats it has that you asked for.
    pub matched: Vec<String>,
    /// Can wear it now (otherwise it's for later).
    pub wearable: bool,
}

/// Uniques up to `level + ahead`, that have at least one wanted stat (or all if none are wanted),
/// best match first within each slot, then lowest level.
/// `skill` = the stats your skill itself wants (a subset of `wanted`): weapons need one of
/// those, and they count double when ranking (life on a sword doesn't make it a caster weapon).
pub fn picks(level: u32, ahead: u32, wanted: &[String], skill: &[String], classes: &[String]) -> Vec<Pick> {
    let mut out: Vec<Pick> = uniques().iter()
        .filter(|u| u.level <= level + ahead)
        .filter(|u| classes.is_empty() || classes.iter().any(|c| *c == u.class))
        .filter_map(|u| {
            let matched: Vec<String> = u.stats.iter().filter(|s| wanted.contains(s)).cloned().collect();
            if !wanted.is_empty() && matched.is_empty() { return None; }
            let slot = slot_of(&u.class);
            // A weapon has to help the kind of skill (spell/attack/minion), not just share an element.
            let core: Vec<&String> = skill.iter().filter(|s| CORE.contains(&s.as_str())).collect();
            if slot == "Weapon" && !core.is_empty() && !matched.iter().any(|m| core.contains(&m)) { return None; }
            Some(Pick { slot, matched, wearable: u.level <= level, unique: u.clone() })
        })
        .collect();
    out.sort_by(|a, b| a.slot.cmp(b.slot)
        .then(b.wearable.cmp(&a.wearable))
        .then(score(b, skill).cmp(&score(a, skill)))
        .then(b.unique.level.cmp(&a.unique.level)));
    out
}

/// Stats that say what kind of skill a weapon is for.
const CORE: [&str; 6] = ["gem_levels", "spell_damage", "cast_speed", "attack_speed", "added_attack", "minion"];

fn score(p: &Pick, skill: &[String]) -> usize {
    p.matched.iter().map(|m| if skill.contains(m) { 2 } else { 1 }).sum()
}

/// A pathofexile.com trade search for this unique, in a league ("Standard" if unknown).
pub fn trade_url(league: &str, name: &str, base: &str) -> String {
    let q = serde_json::json!({
        "query": { "status": { "option": "available" }, "name": name, "type": base },
        "sort": { "price": "asc" }
    });
    let enc = |s: &str| s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect::<String>();
    format!("https://www.pathofexile.com/trade/search/{}?q={}", enc(league), enc(&q.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caster_picks_at_level_10() {
        let fireball = gems().get("Fireball").unwrap();
        let mut wanted = fireball.stats.clone();
        wanted.push("life".into());
        let p = picks(10, 0, &wanted, &fireball.stats, &[]);
        assert!(!p.iter().any(|x| x.unique.name == "Oni-Goroshi"), "a sword with only life isn't a Fireball weapon");
        assert!(p.iter().any(|x| x.unique.name == "Lifesprig" && x.wearable), "Lifesprig is a classic caster levelling wand");
        assert!(p.iter().all(|x| x.unique.level <= 10));
        assert!(!p.iter().any(|x| x.unique.name == "Goldrim"), "no Fireball stats on Goldrim");
        // Plain movement speed boots for anyone.
        let mv = picks(10, 0, &["movement_speed".into()], &[], &[]);
        assert!(mv.iter().any(|x| x.unique.name == "Wanderlust" || x.unique.name == "Seven-League Step"));
        assert_eq!(slot_of("Two Hand Maces"), "Weapon");
    }

    #[test]
    fn trade_link() {
        let u = trade_url("Covens", "Goldrim", "Leather Cap");
        assert!(u.starts_with("https://www.pathofexile.com/trade/search/Covens?q=%7B"));
        assert!(u.contains("Goldrim") && u.contains("Leather%20Cap"));
    }
}
