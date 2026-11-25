//! "Farm this": the farm you picked (tree, Threads, maps, what stands out in the filter), and what it makes after
//! paying for the maps and Threads. The picking itself is in farm.rs.

use crate::item::base_classes;
use crate::rules::{style_actions, CustomStyle, Rule};
use serde::{Deserialize, Serialize};

/// The farm you're running. Kept in farm_now.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FarmNow {
    pub title: String,
    pub keys: Vec<String>,
    /// The saved atlas strategy it uses: maps you run with that tree in use count for it.
    pub strategy: u64,
    pub strategy_name: String,
    /// Threads a map: (name, how many).
    pub threads: Vec<(String, u32)>,
    pub maps: Vec<String>,
    pub rolls: Option<String>,
    /// What it drops that sells (for the filter).
    pub drops: Vec<String>,
    pub started: i64,
    /// Plan this many maps (for the Thread shopping list).
    pub plan: u32,
    /// What one map costs you, chaos: the map itself, and anything else (rolling it, Maven's chisels…).
    pub map_cost: f64,
    pub other_cost: f64,
    /// Your own number for Threads a map (None = today's prices).
    pub thread_cost: Option<f64>,
    /// Its drops stand out in your filter.
    pub in_filter: bool,
}

/// One map run: chaos of loot, seconds until the next one began, and what a divine was worth then (None = not known).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunMade { pub loot: f64, pub secs: f64, pub divine: Option<f64> }

/// What a farm makes after costs.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Profit {
    pub maps: usize,
    /// Chaos a map.
    pub loot: f64,
    pub cost: f64,
    pub net: f64,
    /// Chaos an hour, after costs.
    pub hour: f64,
    /// Where it likely lies going by how much your maps differ (chaos an hour, 8 times in 10). None under 5 maps.
    pub likely: Option<(f64, f64)>,
    /// The same in divines: each map's loot at its own day's rate, costs at today's.
    pub hour_div: Option<f64>,
    pub likely_div: Option<(f64, f64)>,
    /// Fewer than 10 maps: luck still decides it.
    pub few: bool,
}

/// A map run as (start, seconds in it, loot or None when it couldn't be read, tree, chaos per divine then).
pub type Run = (i64, i64, Option<f64>, Option<u64>, Option<f64>);

/// Your map runs as the runs on one tree.
/// The time of a map goes on to the start of the next one (hideout time counts), up to an hour.
pub fn runs_on(runs: &[Run], tree: u64) -> Vec<RunMade> {
    runs.iter().enumerate().filter(|(_, r)| r.3 == Some(tree)).filter_map(|(i, r)| {
        let next = runs.get(i + 1).map(|n| n.0 - r.0).filter(|g| *g > 0 && *g < 3600).unwrap_or(r.1);
        Some(RunMade { loot: r.2?, secs: next.max(60) as f64, divine: r.4 })
    }).collect()
}

pub fn profit(runs: &[RunMade], cost: f64, divine: Option<f64>) -> Option<Profit> {
    if runs.is_empty() { return None; }
    let n = runs.len() as f64;
    let divine = divine.filter(|d| *d > 0.0);
    let hour = |made: &[(f64, f64)], cost: f64| {
        let (l, s) = made.iter().fold((0.0, 0.0), |a, r| (a.0 + r.0, a.1 + r.1));
        (l - cost * n) / (s / 3600.0)
    };
    let chaos: Vec<(f64, f64)> = runs.iter().map(|r| (r.loot, r.secs)).collect();
    let divs: Option<Vec<(f64, f64)>> = divine.map(|d| runs.iter().map(|r| (r.loot / r.divine.filter(|x| *x > 0.0).unwrap_or(d), r.secs)).collect());
    let div_cost = divine.map_or(0.0, |d| cost / d);
    let likely = |made: &[(f64, f64)], cost: f64| (runs.len() >= 5).then(|| {
        // Draw as many maps again from your own, many times over: the middle 8 in 10 of what comes out.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut pick = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % made.len() as u64) as usize };
        let mut got: Vec<f64> = (0..400).map(|_| hour(&(0..made.len()).map(|_| made[pick()]).collect::<Vec<_>>(), cost)).collect();
        got.sort_by(f64::total_cmp);
        (got[40], got[359])
    });
    let loot = chaos.iter().map(|r| r.0).sum::<f64>() / n;
    Some(Profit {
        maps: runs.len(), loot, cost, net: loot - cost, hour: hour(&chaos, cost), likely: likely(&chaos, cost),
        hour_div: divs.as_ref().map(|d| hour(d, div_cost)), likely_div: divs.as_ref().and_then(|d| likely(d, div_cost)),
        few: runs.len() < 10,
    })
}

pub const RULE_PREFIX: &str = "Preset: Farm";

/// The filter edit that makes a farm's drops stand out. Only names the game knows (one it doesn't would stop the whole
/// filter from loading).
pub fn rule(f: &FarmNow, style: &CustomStyle) -> Option<Rule> {
    let mut names: Vec<&String> = f.drops.iter().chain(f.threads.iter().map(|t| &t.0)).filter(|n| base_classes().contains_key(n.as_str())).collect();
    names.sort();
    names.dedup();
    if names.is_empty() { return None; }
    let list = names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ");
    Some(Rule::Override { label: format!("{RULE_PREFIX} ({}) drops stand out", f.title), show: true,
        conditions: vec![format!("BaseType == {list}")], actions: style_actions(style), off: false, look: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profit_after_costs() {
        // Six 5-minute maps at 100c, 40c each to run: 60c net, 12 an hour = 720c.
        let runs: Vec<_> = (0..6).map(|i| (i * 300, 250, Some(100.0), Some(7), Some(100.0))).collect();
        let mine = runs_on(&runs, 7);
        assert_eq!(mine.len(), 6);
        let p = profit(&mine[..5], 40.0, Some(200.0)).unwrap();
        assert_eq!((p.maps, p.net, p.few), (5, 60.0, true));
        assert!((p.hour - 720.0).abs() < 0.01);
        assert_eq!(p.likely, Some((p.hour, p.hour)), "maps all alike leave no doubt");
        // 1 div of loot a map at that day's 100c, 0.2 div of costs at today's 200c: 0.8 div x 12 an hour.
        assert!((p.hour_div.unwrap() - 9.6).abs() < 1e-9 && p.likely_div.is_some());
        assert!(profit(&mine[..5], 40.0, None).unwrap().hour_div.is_none());
        assert!(profit(&mine[..4], 40.0, None).unwrap().likely.is_none());
        assert!(profit(&[], 1.0, None).is_none());
    }

    #[test]
    fn unread_loot_and_other_trees_are_left_out() {
        let runs = vec![(0, 250, None, Some(1), None), (300, 250, Some(10.0), Some(1), None), (600, 250, Some(99.0), Some(2), None), (9000, 100, Some(5.0), None, None)];
        assert_eq!(runs_on(&runs, 1), vec![RunMade { loot: 10.0, secs: 300.0, divine: None }]);
        assert_eq!(runs_on(&runs, 2), vec![RunMade { loot: 99.0, secs: 250.0, divine: None }], "an hour's break isn't map time");
    }

    #[test]
    fn likely_range_is_wider_when_maps_differ() {
        let runs: Vec<RunMade> = (0..20).map(|i| RunMade { loot: if i % 10 == 0 { 900.0 } else { 40.0 }, secs: 300.0, divine: None }).collect();
        let (lo, hi) = profit(&runs, 20.0, None).unwrap().likely.unwrap();
        let mid = profit(&runs, 20.0, None).unwrap().hour;
        assert!(lo < mid && mid < hi && hi - lo > 300.0, "{lo} {mid} {hi}");
    }

    #[test]
    fn filter_rule_has_only_real_names() {
        let f = FarmNow { title: "Ritual".into(), drops: vec!["Omen of Amelioration".into(), "Not A Real Thing".into(), "Headhunter".into()], ..Default::default() };
        let style = crate::bases::default_style("chaos");
        let Some(Rule::Override { conditions, label, .. }) = rule(&f, &style) else { panic!("a rule") };
        assert_eq!(conditions, ["BaseType == \"Omen of Amelioration\""]);
        assert!(label.starts_with("Preset:"));
        assert!(rule(&FarmNow::default(), &style).is_none());
    }
}
