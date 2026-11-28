//! Mercenary checker (28 Nov 2026; he asked: "price check Mercenaries as they show up in our maps … sometimes you want
//! to recruit, sometimes you want loot from them").
//!
//! How Mercenaries work (3.30, from his own Client.txt and guides): one walks up and talks ("Oi - Exile! Over here"),
//! you duel it, and once it's beaten you choose: take ONE of its items (or its pockets), recruit it for gold, or send it
//! packing. So what killing is worth is its single best item, not all of them added up. What recruiting is worth is
//! roughly what a Mercenary Warrant with the same skills and Tier 3 supports sells for on the trade site.
//!
//! Everything it says goes into the game's log as "Name: line", which is how the app knows one is there.

use serde::Serialize;

/// What a Mercenary said, as far as the checker cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MercEvent {
    /// It turned up (or spoke for the first time): the strip comes up.
    Met(String),
    /// You beat it and it's waiting for your choice.
    Beaten(String),
    /// You recruited it.
    Joined(String),
    /// You took an item.
    Took(String),
}

/// Lines only Mercenaries say (all from his log, 3.26 to 3.30). Any line with "Trarth" in it counts as well.
const MET: &[&str] = &[
    "Oi - Exile! Over here", "Let's have a word, eh?", "Better dead than in chains", "Don't poke around too long",
    "Hurry up! I'm bored", "My allegiance is won, not bought", "Never could stay away from the Death Trades",
    "Come on. Let's see what passes for strength in Wraeclast", "Gonna take more than that! Come on",
];
/// Said once beaten, while it waits for you to choose.
const BEATEN: &[&str] = &[
    "Choose your fate", "go ahead - choose", "Loser pays up", "You beat me, you claim it", "Make your choice",
    "Make a call", "What'll it be, then", "Choose your next move", "Claim your prize", "Defeat warrants tribute",
    "I lost. That means it's yours", "you know the rules", "Choose quickly", "I know what I'd choose",
    "Haven't got all day", "Clock's ticking", "My allegiance is won, not bought",
];
const JOINED: &[&str] = &["You lead, I'll follow", "I'll cover you", "You lead, I'll cover"];
const TOOK: &[&str] = &[
    "Hope it's an upgrade", "Had this one for years... but you earned it", "Try not to embarrass yourself with it",
    "Trarthan Code's clear. It's yours", "Fair's fair. It's yours now", "It's yours to ruin", "If it means that much to you",
];

/// The speaker and their words, for an NPC line ("[INFO Client 34964] Elvira Tarika: Oi - exile!").
/// Chat (#global, $trade, %party, &guild, @whispers) and the game's own messages (": You have entered…") aren't.
fn npc_line(line: &str) -> Option<(&str, &str)> {
    let rest = &line[line.find("] ")? + 2..];
    let (who, said) = rest.split_once(": ")?;
    let who = who.trim();
    if who.is_empty() || who.starts_with(['#', '$', '%', '&', '@', '<']) || who.contains(']') { return None; }
    Some((who, said.trim()))
}

/// Is this log line a Mercenary speaking? `known` are the names already seen saying something only a Mercenary says
/// (their other lines, like "Marked!", count too).
pub fn log_event(line: &str, known: &[String]) -> Option<MercEvent> {
    let (who, said) = npc_line(line)?;
    let has = |list: &[&str]| list.iter().any(|p| said.to_lowercase().contains(&p.to_lowercase()));
    let name = who.to_string();
    if has(JOINED) { return Some(MercEvent::Joined(name)); }
    if has(TOOK) { return Some(MercEvent::Took(name)); }
    if has(BEATEN) { return Some(MercEvent::Beaten(name)); }
    if has(MET) || said.contains("Trarth") || known.iter().any(|k| k == who) { return Some(MercEvent::Met(name)); }
    None
}

/// Skills that make a Mercenary worth keeping at your side: auras, heralds and buffs that also help you, links,
/// curses and marks. (A Luminary keeps one for good, so these matter more than its damage.)
const HELPS_YOU: &[&str] = &[
    "Wrath", "Zealotry", "Anger", "Hatred", "Grace", "Determination", "Discipline", "Haste", "Malevolence", "Pride",
    "Envy", "Precision", "Purity of Fire", "Purity of Ice", "Purity of Lightning", "Vitality", "Clarity", "Vaal Grace",
    "Battlemage's Cry", "Rallying Cry", "Inspiring Cry", "Intimidating Cry", "Flame Link", "Herald of Purity",
];
const CURSES: &[&str] = &[
    "Despair", "Temporal Chains", "Elemental Weakness", "Flammability", "Frostbite", "Conductivity", "Assassin's Mark",
    "Poacher's Mark", "Alchemist's Mark", "Punishment", "Enfeeble", "Vulnerability", "Bane",
];

/// What its skills do for you, in a few words ("Wrath, Haste: helps you"), if anything.
pub fn helps(skills: &[String]) -> Vec<String> {
    let on = |list: &[&str]| -> Vec<String> {
        skills.iter().filter(|s| list.iter().any(|a| s.trim_start_matches("Vaal ").eq_ignore_ascii_case(a) || s.eq_ignore_ascii_case(a)))
            .cloned().collect()
    };
    let mut out = vec![];
    let buffs = on(HELPS_YOU);
    if !buffs.is_empty() { out.push(format!("{}: helps you", buffs.join(", "))); }
    let curses = on(CURSES);
    if !curses.is_empty() { out.push(format!("{}: curses enemies", curses.join(", "))); }
    out
}

/// One of its skill gems as the game copies it. `support` and the tier come from the copy ("Item Class: Support
/// Gems", "Tier: 3"); a support's name loses " Support" so it matches the trade site's Mercenary stats.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MercGem { pub name: String, pub support: bool, pub tier: Option<u32> }

/// A copied gem, read for the checker (None when the copy isn't a gem).
pub fn gem(text: &str) -> Option<MercGem> {
    let text = text.replace('\r', "");
    let class = text.lines().find_map(|l| l.strip_prefix("Item Class: "))?.trim().to_string();
    if !class.contains("Gem") { return None; }
    let name = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.contains(": ") && *l != "--------")?.to_string();
    let support = class.contains("Support") || name.ends_with(" Support");
    let name = name.trim_end_matches(" Support").to_string();
    let tier = text.lines().find_map(|l| l.trim().strip_prefix("Tier: ").and_then(|t| t.trim().parse().ok()))
        .or_else(|| name.rsplit_once(" (Tier ").and_then(|(_, t)| t.trim_end_matches(')').parse().ok()));
    let name = name.rsplit_once(" (Tier ").map(|(n, _)| n.to_string()).unwrap_or(name);
    Some(MercGem { name, support, tier })
}

/// A support copied without its tier: the trade site's Mercenary stats say which tier that support is
/// ("Pierce (Tier 2)", "Greater Pierce (Tier 3)": the name decides it).
pub fn fill_tier(g: &mut MercGem, index: &crate::trade::StatIndex) {
    if !g.support || g.tier.is_some() { return; }
    g.tier = (1..=3).find(|t| !index.ids("mercenary", &format!("{} (Tier {t})", g.name)).is_empty());
}

/// Its skills and supports written like a Mercenary Warrant's copy, so the price window's warrant search
/// (`trade::setup`, its "Mercenary" rows) can price "a Mercenary like this one". Supports with no tier on the copy
/// are taken as Tier 3 (the ones that set the price) and can be unticked in the price window.
pub fn warrant_text(name: &str, gems: &[MercGem]) -> String {
    let mut out = format!("Item Class: Map Fragments\nRarity: Normal\nMercenary Warrant\n--------\n{name}\n--------\nMercenary Level: 83\n--------\n");
    for g in gems {
        if g.support { out.push_str(&format!("{} (Tier: {})\n", g.name, g.tier.unwrap_or(3))); } else { out.push_str(&format!("{}\n", g.name)); }
    }
    out.push_str("--------\nRight click this item to view Mercenary details.\n");
    out
}

/// "A Mercenary like this" for a build's Mercenary stage (28 Nov: My build had no way to search for the Mercenary itself).
/// `text` is a warrant copy for the price window; `query` asks the trade site for a warrant with the main attack and
/// at least `need` of its supports, in any tier.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildWarrant { pub text: String, pub query: serde_json::Value, pub skill: String, pub supports: Vec<String> }

/// From a Mercenary stage's gem groups (Path of Building names: "Hypothermia", or "Hypothermia Support"). The main
/// attack is the group with the most supports whose skill is a Mercenary skill on the trade site; other Mercenary
/// skills go in the warrant text too. A support counts in any tier: "Hypothermia (Tier 2)" or "Greater Hypothermia (Tier 3)".
pub fn from_build(groups: &[crate::pob::Group], index: &crate::trade::StatIndex, need: usize) -> Option<BuildWarrant> {
    let merc = |n: &str| index.ids("mercenary", n);
    let plain = |n: &str| n.trim_end_matches(" Support").to_string();
    // Every tier of a support, best first: (label, ids).
    let tiers = |n: &str| -> Vec<(String, Vec<String>)> {
        [format!("Greater {n} (Tier 3)"), format!("{n} (Tier 3)"), format!("{n} (Tier 2)"), format!("{n} (Tier 1)")].into_iter()
            .map(|l| { let ids = merc(&l); (l, ids) }).filter(|(_, ids)| !ids.is_empty()).collect()
    };
    let skills: Vec<(&crate::pob::Gem, Vec<&crate::pob::Gem>)> = groups.iter().filter(|g| g.enabled).filter_map(|g| {
        let on: Vec<&crate::pob::Gem> = g.gems.iter().filter(|x| x.enabled).collect();
        let skill = on.iter().copied().find(|x| !x.support && !merc(&x.name).is_empty())?;
        Some((skill, on.into_iter().filter(|x| x.support && !tiers(&plain(&x.name)).is_empty()).collect()))
    }).collect();
    let (main, sups) = skills.iter().max_by_key(|(_, s)| s.len())?;
    let mut gems = vec![];
    for (sk, ss) in &skills {
        gems.push(MercGem { name: sk.name.clone(), support: false, tier: None });
        for s in ss {
            let (label, _) = tiers(&plain(&s.name)).into_iter().next()?;
            let (name, tier) = label.rsplit_once(" (Tier ").map(|(n, t)| (n.to_string(), t.trim_end_matches(')').parse().ok()))?;
            gems.push(MercGem { name, support: true, tier });
        }
    }
    let skill_ids = merc(&main.name);
    let mut stats = vec![if skill_ids.len() == 1 { serde_json::json!({ "type": "and", "filters": [{ "id": skill_ids[0] }] }) }
        else { serde_json::json!({ "type": "count", "value": { "min": 1 }, "filters": skill_ids.iter().map(|i| serde_json::json!({ "id": i })).collect::<Vec<_>>() }) }];
    let sup_ids: Vec<serde_json::Value> = sups.iter().flat_map(|s| tiers(&plain(&s.name)).into_iter().flat_map(|(_, ids)| ids))
        .map(|i| serde_json::json!({ "id": i })).collect();
    let need = need.min(sups.len());
    if need > 0 { stats.push(serde_json::json!({ "type": "count", "value": { "min": need }, "filters": sup_ids })); }
    Some(BuildWarrant {
        text: warrant_text("A Mercenary like this", &gems),
        // Instant buyout only, like Buy on gear (28 Nov: he wants no whispering for a warrant).
        query: serde_json::json!({ "query": { "status": { "option": "securable" }, "type": "Mercenary Warrant", "stats": stats }, "sort": { "price": "asc" } }),
        skill: main.name.clone(),
        supports: sups.iter().map(|s| plain(&s.name)).collect(),
    })
}

/// The readout at the top of the window.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Verdict {
    /// "kill", "recruit" or "check" (nothing priced yet).
    pub kind: String,
    /// Which item to take when you kill it (index into the items), the dearest one.
    pub best: Option<usize>,
    /// What the best item sells for, in chaos.
    pub kill: Option<f64>,
    /// What a Mercenary like it sells for, in chaos.
    pub recruit: Option<f64>,
    /// One line on why.
    pub why: String,
}

/// Kill (take its dearest item) or recruit (keep it: worth what a warrant like it sells for)?
/// `prices` are its items' prices in chaos (None: not priced yet or no price found).
/// `helps` is what its skills do for you (see `helps`): with no warrant price, a cheap Mercenary that helps you is
/// worth keeping rather than taking a few chaos of loot.
pub fn verdict(prices: &[Option<f64>], recruit: Option<f64>, helps: &[String]) -> Verdict {
    let best = prices.iter().enumerate().filter_map(|(i, p)| p.map(|p| (i, p)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    let kill = best.map(|b| b.1);
    let v = |kind: &str, why: String| Verdict { kind: kind.into(), best: best.map(|b| b.0), kill, recruit, why };
    match (kill, recruit) {
        (None, None) if !helps.is_empty() => v("recruit", "Nothing priced yet, but its skills help you".into()),
        (None, None) => v("check", "Hover its items and press Ctrl+D".into()),
        (Some(k), Some(r)) if r > k => v("recruit", "A Mercenary like it sells for more than its best item".into()),
        (Some(_), Some(_)) => v("kill", "Its best item is worth more than a Mercenary like it".into()),
        (None, Some(_)) => v("recruit", "None of its items has a price yet".into()),
        (Some(k), None) if k < 20.0 && !helps.is_empty() => v("recruit", "Its gear is cheap and its skills help you".into()),
        (Some(_), None) => v("kill", "Take its best item (copy its skill gems with Ctrl+D to price recruiting)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_his_log() {
        // From his Client.txt, 14 Nov 2026 (Elvira Tarika, recruited).
        let l = |s: &str| format!("2026/11/14 09:57:54 203169859 cffb06be [INFO Client 34964] {s}");
        assert_eq!(log_event(&l("Elvira Tarika: Oi - exile! Over here. Let's have a word, eh?"), &[]), Some(MercEvent::Met("Elvira Tarika".into())));
        assert_eq!(log_event(&l("Elvira Tarika: Beat me, and the choice is yours: take my gear, recruit me, or send me packing. That's the Trarthan Code."), &[]),
            Some(MercEvent::Met("Elvira Tarika".into())));
        assert_eq!(log_event(&l("Elvira Tarika: Marked!"), &[]), None);
        assert_eq!(log_event(&l("Elvira Tarika: Marked!"), &["Elvira Tarika".into()]), Some(MercEvent::Met("Elvira Tarika".into())));
        assert_eq!(log_event(&l("Arikelle, the Greedy: I still abide by the Trarthan Code. Choose your fate."), &[]), Some(MercEvent::Beaten("Arikelle, the Greedy".into())));
        assert_eq!(log_event(&l("Elvira Tarika: If there's one thing us Trarthans respect, it's strength - and you showed yours. You lead, I'll follow."), &[]),
            Some(MercEvent::Joined("Elvira Tarika".into())));
        assert_eq!(log_event(&l("Nia: Had this one for years... but you earned it."), &[]), Some(MercEvent::Took("Nia".into())));
        // The game's own messages and chat aren't Mercenaries.
        assert_eq!(log_event(&l(": You have entered The Docks."), &[]), None);
        assert_eq!(log_event(&l("#<UwU9> Ramsi_____: Mercenary Warrant is it good? Trarthan"), &[]), None);
        assert_eq!(log_event(&l("$Someone: WTS Trarthan stuff"), &[]), None);
    }

    /// `MERC_LOG=<Client.txt> cargo test -p filter-core --lib merc::tests::real_log -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_log() {
        let text = String::from_utf8_lossy(&std::fs::read(std::env::var("MERC_LOG").unwrap()).unwrap()).to_string();
        let mut known: Vec<String> = vec![];
        let mut counts = std::collections::BTreeMap::<String, [u32; 4]>::new();
        for l in text.lines() {
            let Some(e) = log_event(l, &known) else { continue };
            let (n, i) = match &e { MercEvent::Met(n) => (n, 0), MercEvent::Beaten(n) => (n, 1), MercEvent::Joined(n) => (n, 2), MercEvent::Took(n) => (n, 3) };
            if !known.contains(n) { known.push(n.clone()); }
            counts.entry(n.clone()).or_default()[i] += 1;
        }
        for (n, c) in &counts { println!("{n}: {c:?}"); }
        println!("{} Mercenaries", counts.len());
    }

    #[test]
    fn kill_takes_the_dearest_one_item() {
        let v = verdict(&[Some(40.0), None, Some(600.0), Some(3.0)], Some(35.0), &[]);
        assert_eq!((v.kind.as_str(), v.best, v.kill), ("kill", Some(2), Some(600.0)));
        let v = verdict(&[Some(40.0), Some(3.0)], Some(150.0), &[]);
        assert_eq!(v.kind, "recruit");
        let v = verdict(&[], None, &[]);
        assert_eq!(v.kind, "check");
        // Cheap gear, no warrant price, but it runs Wrath: keep it.
        let h = helps(&["Wrath".into(), "Arrow Nova".into()]);
        assert_eq!(h, ["Wrath: helps you"]);
        assert_eq!(verdict(&[Some(5.0)], None, &h).kind, "recruit");
        assert_eq!(verdict(&[Some(500.0)], None, &h).kind, "kill");
    }

    #[test]
    fn a_build_mercenary_becomes_a_warrant_search() {
        use crate::pob::{Gem, Group};
        let g = |n: &str, s: bool| Gem { name: n.into(), level: 20, quality: 0, enabled: true, support: s, colour: "w".into() };
        let groups = vec![
            Group { label: "".into(), slot: "".into(), enabled: true, gems: vec![g("Destructive Link", false)] },
            Group { label: "".into(), slot: "".into(), enabled: true, gems: vec![g("Frost Blades", false), g("Hypothermia Support", true), g("Elemental Damage with Attacks", true), g("Brutality", true)] },
            Group { label: "".into(), slot: "".into(), enabled: true, gems: vec![g("Static Strike", false), g("More Duration", true)] }];
        let idx = crate::trade::StatIndex::from_json(&serde_json::json!({ "result": [ { "label": "Mercenary", "entries": [
            { "id": "mercenary.skill_fb", "text": "Frost Blades", "type": "mercenary" },
            { "id": "mercenary.skill_ss", "text": "Static Strike", "type": "mercenary" },
            { "id": "mercenary.support_h2", "text": "Hypothermia (Tier 2)", "type": "mercenary" },
            { "id": "mercenary.support_h3", "text": "Greater Hypothermia (Tier 3)", "type": "mercenary" },
            { "id": "mercenary.support_e3", "text": "Greater Elemental Damage with Attacks (Tier 3)", "type": "mercenary" },
            { "id": "mercenary.support_d2", "text": "More Duration (Tier 2)", "type": "mercenary" } ] } ] }));
        let w = from_build(&groups, &idx, 9).unwrap();
        assert_eq!((w.skill.as_str(), w.supports.clone()), ("Frost Blades", vec!["Hypothermia".to_string(), "Elemental Damage with Attacks".to_string()]));
        assert_eq!(w.query["query"]["status"]["option"], "securable", "instant buyout only");
        let stats = &w.query["query"]["stats"];
        assert_eq!(stats[0]["filters"][0]["id"], "mercenary.skill_fb");
        assert_eq!(stats[1]["value"]["min"], 2);
        let ids: Vec<&str> = stats[1]["filters"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["mercenary.support_h3", "mercenary.support_h2", "mercenary.support_e3"]);
        assert!(w.text.contains("Frost Blades\nGreater Hypothermia (Tier: 3)\n") && w.text.contains("Static Strike\nMore Duration (Tier: 2)\n"), "{}", w.text);
        assert_eq!(from_build(&groups[..1], &idx, 2), None, "no Mercenary skill");
    }

    #[test]
    fn gems_become_a_warrant_search() {
        let skill = "Item Class: Skill Gems\nRarity: Gem\nWrath\n--------\nAura, Spell, AoE, Lightning\nLevel: 20\n";
        let sup = "Item Class: Support Gems\nRarity: Gem\nGreater Multiple Projectiles Support\n--------\nTier: 3\n";
        let g: Vec<MercGem> = [skill, sup].iter().filter_map(|t| gem(t)).collect();
        assert_eq!(g, [MercGem { name: "Wrath".into(), support: false, tier: None },
            MercGem { name: "Greater Multiple Projectiles".into(), support: true, tier: Some(3) }]);
        assert_eq!(gem("Item Class: Rings\nRarity: Rare\nDoom Loop\nRuby Ring\n"), None);
        let mut p = MercGem { name: "Pierce".into(), support: true, tier: None };
        let text = warrant_text("Elvira Tarika", &g);
        let idx = crate::trade::StatIndex::from_json(&serde_json::json!({ "result": [ { "label": "Mercenary", "entries": [
            { "id": "mercenary.skill_1", "text": "Wrath", "type": "mercenary" },
            { "id": "mercenary.support_2", "text": "Greater Multiple Projectiles (Tier 3)", "type": "mercenary" },
            { "id": "mercenary.support_3", "text": "Pierce (Tier 2)", "type": "mercenary" } ] } ] }));
        fill_tier(&mut p, &idx);
        assert_eq!(p.tier, Some(2));
        let s = crate::trade::setup(&text, &idx).unwrap();
        let rows: Vec<(&str, bool)> = s.filters.iter().filter(|f| f.kind == "mercenary").map(|f| (f.label.as_str(), f.enabled)).collect();
        assert_eq!(rows, [("Wrath", false), ("Greater Multiple Projectiles (Tier 3)", true)]);
    }
}
