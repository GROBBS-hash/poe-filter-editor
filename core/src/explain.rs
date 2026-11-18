//! Turns NeverSink's internal tags ("hidelayer › normalmagicendgame") into
//! plain English ("plain normal/magic gear, once you're in maps").

use crate::filter::Block;

/// What a NeverSink section is about, in everyday words.
fn section(path: &[String], tier_name: Option<&str>) -> String {
    // Some details live in the tier name ("hidelayer" + "normalmagicendgame").
    let p = match tier_name { Some(t) => format!("{}>{t}", path.join(">")), None => path.join(">") };
    let first = path.first().map(String::as_str).unwrap_or("");
    let specific = [
        ("hidelayer>normalmagicendgame", "plain normal/magic gear"),
        ("hidelayer>raresendgame", "ordinary rare gear"),
        ("hidelayer>corruptedrares", "corrupted rare gear"),
        ("hidelayer>mirroredrares", "mirrored rare gear"),
        ("hidelayer>final", "gear nothing else picked out"),
        ("uniques>replicas", "replica uniques"),
        ("uniques>foulborn", "foulborn uniques"),
        ("currency>stackedsupplieswisdom", "stacks of Scrolls of Wisdom"),
        ("currency>twisdom", "Scrolls of Wisdom"),
        ("currency>tportal", "Portal Scrolls"),
        ("currency>scrollfragments", "scroll fragments"),
        ("currency>t9armour", "cheap armourer's scraps and whetstones"),
        ("currency>stackedsuppliesportal", "stacks of Portal Scrolls"),
        ("currency>levelingstacked", "levelling currency stacks"),
        ("currency>leveling", "levelling currency"),
        ("currency>splinter", "splinters"),
        ("currency>stackedsplintershigh", "valuable splinter stacks"),
        ("currency>stackedsplinterslow", "cheap splinter stacks"),
        ("currency>essence", "essences"),
        ("currency>fossil", "fossils"),
        ("currency>oil", "oils"),
        ("currency>deliriumorbs", "delirium orbs"),
        ("currency>trial>omen", "omens"),
        ("currency>trial>tattoo", "tattoos"),
        ("currency>harvest", "harvest lifeforce"),
        ("currency>heist", "heist currency"),
        ("gems>exceptional", "exceptional gems"),
        ("gems>special", "special gems"),
        ("gems>generic", "skill and support gems"),
        ("gear>tiered", "tiered gear"),
        ("gear>memorystrand", "gear with memory strands"),
        ("exotic>fractured", "fractured items"),
        ("exotic>enchanted", "enchanted items"),
        ("leveling>flasks", "levelling flasks"),
        ("leveling>rare", "useful rares while levelling"),
        ("leveling>normalmagic", "useful normal/magic gear while levelling"),
        ("leveling>firstlevels", "early-game gear"),
        ("leveling>weaponprogression", "weapon upgrades while levelling"),
        ("leveling>wandprogression", "wand upgrades while levelling"),
        ("jewels>cluster", "cluster jewels"),
        ("jewels>clustereco", "cluster jewels"),
        ("jewels>abyss", "abyss jewels"),
        ("maps>blighted", "blighted maps"),
        ("maps>influenced", "influenced maps"),
    ];
    if let Some((_, s)) = specific.iter().filter(|(k, _)| p.starts_with(k)).max_by_key(|(k, _)| k.len()) {
        return s.to_string();
    }
    match first {
        "currency" => "currency",
        "uniques" => "uniques",
        "divination" => "divination cards",
        "gems" => "gems",
        "maps" | "exoticmap" => "maps",
        "fragments" => "map fragments",
        "gold" => "gold",
        "rareid" | "rareblendid" | "rare" | "rr" => "rares worth checking",
        "magicid" => "magic items worth checking",
        "corruptedid" => "corrupted items worth checking",
        "jewels" => "jewels",
        "influenced" => "influenced items",
        "crafting" => "good crafting bases",
        "exoticbases" => "rare and valuable base types",
        "exoticmods" | "exotic" => "items with special mods",
        "endgameflasks" => "endgame flasks",
        "endgametinctures" => "tinctures",
        "socketslinks" => "items with useful sockets or links",
        "6l" => "6-linked items",
        "heist" | "heisttarget" => "heist items",
        "artefact" => "league artefacts",
        "event" => "league event items",
        "questlike" | "questlikeexception" => "quest items",
        "decorators" => "an extra style on top",
        "leveling" => "levelling items",
        "viscera" | "corpses" => "graveyard items",
        "runesgrafts" => "runegrafts",
        "mapenchantments" => "map enchantments",
        "miscmapitems" | "miscmapitemsextra" => "map items",
        "charts" => "charts",
        "expedition" => "expedition logbooks",
        "enshrouded" => "enshrouded items",
        "anyremaining" => "anything not covered elsewhere",
        "hidelayer" => "low-value items",
        other => return other.replace('_', " "),
    }
    .to_string()
}

/// "top tier", "tier 3", "low value"... from NeverSink's tier name.
fn tier(t: &str) -> Option<String> {
    let t = t.to_lowercase();
    let digits: String = t.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    if t.contains("hide") || t.contains("restex") || t == "rest" {
        return Some("low value".into());
    }
    if (t.starts_with('t') || t.starts_with("ex")) && !digits.is_empty() {
        return Some(match digits.as_str() { "1" | "0" => "top tier".into(), d => format!("tier {d}") });
    }
    None
}

/// A short phrase for the conditions that matter to a player.
fn conditions(b: &Block) -> Vec<String> {
    let mut out = Vec::new();
    for line in &b.lines {
        let l = line.trim();
        let mut parts = l.split_whitespace();
        let key = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        let (op, val) = match rest.as_slice() {
            [op, v, ..] if ["<", "<=", ">", ">=", "=", "==", "!="].contains(op) => (*op, v.trim_matches('"').to_string()),
            [v, ..] => ("=", v.trim_matches('"').to_string()),
            _ => continue,
        };
        let n: Option<u32> = val.parse().ok();
        let phrase = match (key, op, n) {
            ("AreaLevel", ">=", Some(v)) if v >= 68 => Some("once you're in maps".to_string()),
            ("AreaLevel", "<=", Some(v)) if v <= 67 => Some("while levelling".to_string()),
            ("AreaLevel", "<", Some(v)) if v <= 68 => Some("while levelling".to_string()),
            ("AreaLevel", ">=", Some(v)) if v > 2 => Some(format!("from area level {v}")),
            ("ItemLevel", ">=", Some(v)) => Some(format!("item level {v}+")),
            ("ItemLevel", "<", Some(v)) => Some(format!("item level under {v}")),
            ("ItemLevel", "<=", Some(v)) => Some(format!("item level {v} or lower")),
            ("StackSize", ">=", Some(v)) => Some(format!("in stacks of {v}+")),
            ("StackSize", "<", Some(v)) => Some(format!("in stacks under {v}")),
            ("Quality", ">=", Some(v)) if v > 0 => Some(format!("{v}%+ quality")),
            ("GemLevel", ">=", Some(v)) => Some(format!("gem level {v}+")),
            ("LinkedSockets", ">=", Some(v)) | ("LinkedSockets", "=", Some(v)) => Some(format!("{v}-linked")),
            ("Sockets", ">=", Some(v)) => Some(format!("{v}+ sockets")),
            ("MapTier", ">=", Some(v)) => Some(format!("map tier {v}+")),
            ("UnidentifiedItemTier", ">=", Some(v)) => Some(format!("item tier {v}+")),
            ("HasExplicitMod", _, _) => Some("with certain mods".to_string()),
            ("HasInfluence", _, _) if val != "None" => Some("influenced".to_string()),
            ("Corrupted", _, _) if val == "True" => Some("corrupted".to_string()),
            ("FracturedItem", _, _) if val == "True" => Some("fractured".to_string()),
            ("SynthesisedItem", _, _) if val == "True" => Some("synthesised".to_string()),
            ("Identified", _, _) if val == "True" => Some("identified".to_string()),
            ("DropLevel", ">=", Some(_)) => Some("high-level bases".to_string()),
            _ => None,
        };
        if let Some(p) = phrase {
            if !out.contains(&p) { out.push(p); }
        }
    }
    out
}

/// One plain sentence about why a NeverSink rule applies, e.g.
/// "Plain normal/magic gear, once you're in maps".
pub fn explain(b: &Block) -> String {
    if b.type_path.is_empty() {
        return "a rule without a NeverSink label".into();
    }
    let mut s = section(&b.type_path, b.tier.as_deref());
    if let Some(t) = b.tier.as_deref().and_then(tier) {
        s = format!("{s} ({t})");
    }
    let c = conditions(b);
    if !c.is_empty() {
        s = format!("{s}, {}", c.join(", "));
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{parse_filter, Segment};

    fn first(f: &str) -> String {
        parse_filter(f).iter().find_map(|s| if let Segment::Block(b) = s { Some(explain(b)) } else { None }).unwrap()
    }

    #[test]
    fn readable() {
        assert_eq!(first("Hide # $type->hidelayer $tier->normalmagicendgame\n    Rarity Normal Magic\n    AreaLevel >= 68\n"),
            "Plain normal/magic gear, once you're in maps");
        assert_eq!(first("Show # $type->currency->stackedsupplieswisdom $tier->t3\n    StackSize >= 3\n"),
            "Stacks of Scrolls of Wisdom (tier 3), in stacks of 3+");
        assert_eq!(first("Show # %D3 $type->gems->generic $tier->qt4 !gems_d\n    Quality >= 1\n"),
            "Skill and support gems, 1%+ quality");
        assert_eq!(first("Show # $type->uniques $tier->t1\n    Rarity Unique\n"), "Uniques (top tier)");
        assert_eq!(first("Show # $type->uniques $tier->hideable2\n"), "Uniques (low value)");
    }

    #[test]
    fn every_real_rule_gets_a_sentence() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        let mut n = 0;
        for s in parse_filter(&t) {
            if let Segment::Block(b) = s {
                let e = explain(&b);
                assert!(!e.is_empty() && !e.contains("->"), "{e}");
                n += 1;
            }
        }
        assert_eq!(n, 786);
    }
}
