//! "Tonight" on Home: up to three things worth doing in the time you have: sell something, run something, fix
//! something, each with its reason. A card is left out rather than guessed.

use crate::deaths::Patterns;
use crate::farming::{FarmNow, Profit};
use crate::wealth::{Reprice, Valued};
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Card {
    /// "sell", "run" or "fix".
    pub kind: String,
    pub title: String,
    pub reason: String,
    /// About what it's worth, chaos (low, high).
    pub worth: Option<(f64, f64)>,
    /// (item name, picture or "") and mechanics, for the pictures.
    pub items: Vec<(String, String)>,
    pub mechs: Vec<String>,
    /// Everything it's about, for "Highlight in stash" (the pictures show the first few).
    pub names: Vec<String>,
    /// What the button does: "tab:wealth", "highlight", "avoid:<mod>".
    pub action: String,
    pub button: String,
    /// When what it's based on was read (unix seconds).
    pub as_of: Option<i64>,
}

/// Kept, not sold: what you trade and craft with.
const KEEP: [&str; 6] = ["Divine Orb", "Chaos Orb", "Exalted Orb", "Mirror of Kalandra", "Mirror Shard", "Hinekora's Lock"];

/// The most valuable things not listed yet: as many as you'd price in a third of the time, a minute each.
pub fn sell(missed: &[Valued], minutes: u32, keep: &[String], scanned: i64) -> Option<Card> {
    let n = (minutes / 3).clamp(4, 15) as usize;
    let pick: Vec<&Valued> = missed.iter().filter(|v| !KEEP.contains(&v.name.as_str()) && !keep.contains(&v.name)).take(n).collect();
    if pick.is_empty() { return None; }
    let total: f64 = pick.iter().map(|v| v.value).sum();
    Some(Card {
        kind: "sell".into(),
        title: if pick.len() == 1 { format!("Sell {}", pick[0].name) } else { format!("Sell {} things you haven't listed", pick.len()) },
        reason: format!("Most valuable: {}", pick.iter().take(3).map(|v| if v.stack > 1 { format!("{} × {}", v.stack, v.name) } else { v.name.clone() }).collect::<Vec<_>>().join(", ")),
        worth: Some((total, total)),
        items: pick.iter().take(6).map(|v| (v.name.clone(), v.icon.clone())).collect(),
        names: pick.iter().map(|v| v.name.clone()).collect(),
        action: "highlight".into(), button: "Highlight in stash".into(), as_of: Some(scanned), ..Default::default()
    })
}

/// The farm you picked, for as many maps as fit. `cycle` = seconds a map takes you, hideout included.
pub fn run(farm: &FarmNow, profit: Option<&Profit>, cycle: f64, minutes: u32, buy: &[(String, u32)]) -> Card {
    let maps = ((minutes as f64 * 60.0 / cycle.max(60.0)).floor() as u32).max(1);
    let hours = maps as f64 * cycle / 3600.0;
    let worth = profit.filter(|p| !p.few).map(|p| match p.likely { Some((lo, hi)) => (lo * hours, hi * hours), None => (p.net * maps as f64, p.net * maps as f64) });
    let s = if maps == 1 { "" } else { "s" };
    let mut reason = match profit {
        Some(p) if !p.few => format!("{} of your maps made about {:.0}c each after costs", p.maps, p.net),
        Some(p) => format!("Only {} map{} so far: run more to see what it makes", p.maps, if p.maps == 1 { "" } else { "s" }),
        None => "No maps on this tree yet: run some to see what it makes".into(),
    };
    if !buy.is_empty() { reason += &format!(". First buy {}", buy.iter().map(|(n, c)| format!("{c} {n}s")).collect::<Vec<_>>().join(" and ")); }
    Card { kind: "run".into(), title: format!("Run {maps} {} map{s}", farm.title), reason, worth, mechs: farm.keys.clone(),
        action: "tab:atlas".into(), button: "Show the plan".into(), ..Default::default() }
}

/// No farm picked yet: the best one right now.
pub fn pick_farm(title: &str, keys: &[String]) -> Card {
    Card { kind: "run".into(), title: format!("Farm {title}"), reason: "Top of What to farm today. Press Farm this on its card to set it up".into(),
        mechs: keys.to_vec(), action: "tab:atlas".into(), button: "Show it".into(), ..Default::default() }
}

/// The most useful thing to put right: listed too cheap, a mod you keep dying to, or a stash not read for days.
pub fn fix(reprice: &[Reprice], deaths: &Patterns, avoided: &dyn Fn(&str) -> bool, scanned: Option<i64>, now: i64, login: bool) -> Option<Card> {
    // Listed well under its worth: someone will buy it at that price.
    let cheap: Vec<&Reprice> = reprice.iter().filter(|r| !r.high && r.worth - r.asking >= 5.0).collect();
    if !cheap.is_empty() {
        let gain: f64 = cheap.iter().map(|r| r.worth - r.asking).sum();
        return Some(Card { kind: "fix".into(), title: if cheap.len() == 1 { format!("{} looks listed too cheap", cheap[0].name) } else { format!("{} things look listed too cheap", cheap.len()) },
            reason: format!("{}: asking {:.0}c, worth about {:.0}c", cheap[0].name, cheap[0].asking, cheap[0].worth), worth: Some((gain, gain)),
            items: cheap.iter().take(6).map(|r| (r.name.clone(), String::new())).collect(), action: "tab:wealth".into(), button: "Show them".into(), as_of: scanned, ..Default::default() });
    }
    if let Some(m) = deaths.mods.iter().find(|m| m.stands_out && !avoided(&m.name)) {
        return Some(Card { kind: "fix".into(), title: format!("Skip maps with “{}”", m.name),
            reason: format!("You died in {} of {} maps with it, and in {} of {} without", m.bad_runs, m.runs, m.other_bad, m.other_runs),
            action: format!("avoid:{}", m.name), button: "Put it on my can't-run list".into(), ..Default::default() });
    }
    if login && scanned.is_none_or(|t| now - t > 3 * 86400) {
        return Some(Card { kind: "fix".into(), title: "Read your stash again".into(),
            reason: match scanned { Some(t) => format!("Last read {} days ago, so what's worth selling is out of date", (now - t) / 86400), None => "Then this page can say what's worth selling".into() },
            action: "tab:wealth".into(), button: "Open Stash worth".into(), ..Default::default() });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deaths::patterns;

    fn valued(name: &str, value: f64) -> Valued {
        serde_json::from_value(serde_json::json!({ "tab": "Dump", "name": name, "stack": 2, "value": value, "x": 0, "y": 0, "listed": false, "icon": "" })).unwrap()
    }

    #[test]
    fn sells_what_isnt_kept() {
        let missed = vec![valued("Divine Orb", 400.0), valued("Ritual Thread", 60.0), valued("The Doctor", 50.0), valued("The Nurse", 20.0)];
        let c = sell(&missed, 15, &["Ritual Thread".into()], 5).unwrap();
        assert_eq!(c.worth, Some((70.0, 70.0)));
        assert_eq!(c.title, "Sell 2 things you haven't listed");
        assert!(sell(&missed[..1], 60, &[], 5).is_none());
    }

    #[test]
    fn maps_that_fit() {
        let f = FarmNow { title: "Ritual".into(), keys: vec!["ritual".into()], ..Default::default() };
        let p = Profit { maps: 23, net: 89.0, likely: Some((1000.0, 2000.0)), ..Default::default() };
        let c = run(&f, Some(&p), 360.0, 30, &[("Ritual Thread".into(), 64)]);
        assert_eq!(c.title, "Run 5 Ritual maps");
        assert_eq!(c.worth, Some((500.0, 1000.0)));
        assert!(c.reason.contains("First buy 64 Ritual Threads"));
        assert!(run(&f, Some(&Profit { few: true, maps: 3, ..p }), 360.0, 15, &[]).worth.is_none(), "too few maps to promise anything");
        assert_eq!(run(&f, None, 5000.0, 15, &[]).title, "Run 1 Ritual map");
    }

    #[test]
    fn fixes_come_in_order() {
        let none = patterns(&[]);
        let never = |_: &str| false;
        let cheap = Reprice { name: "Headhunter".into(), tab: "x".into(), note: String::new(), asking: 100.0, worth: 900.0, high: false, x: 0, y: 0 };
        assert_eq!(fix(&[cheap], &none, &never, Some(0), 0, true).unwrap().worth, Some((800.0, 800.0)));
        assert!(fix(&[], &none, &never, Some(100), 200, true).is_none());
        assert!(fix(&[], &none, &never, Some(0), 4 * 86400, true).unwrap().reason.contains("4 days"));
        assert!(fix(&[], &none, &never, None, 0, false).is_none(), "can't read it without a login");
    }
}
