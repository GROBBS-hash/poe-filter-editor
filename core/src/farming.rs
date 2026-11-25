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

/// One map run: chaos of loot, seconds until the next one began, deaths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunMade { pub loot: f64, pub secs: f64 }

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
    /// Fewer than 10 maps: luck still decides it.
    pub few: bool,
}

/// Your map runs (start, seconds in it, loot or None when it couldn't be read, tree) as the runs on one tree.
/// The time of a map goes on to the start of the next one (hideout time counts), up to an hour.
pub fn runs_on(runs: &[(i64, i64, Option<f64>, Option<u64>)], tree: u64) -> Vec<RunMade> {
    runs.iter().enumerate().filter(|(_, r)| r.3 == Some(tree)).filter_map(|(i, r)| {
        let next = runs.get(i + 1).map(|n| n.0 - r.0).filter(|g| *g > 0 && *g < 3600).unwrap_or(r.1);
        Some(RunMade { loot: r.2?, secs: next.max(60) as f64 })
    }).collect()
}

pub fn profit(runs: &[RunMade], cost: f64) -> Option<Profit> {
    if runs.is_empty() { return None; }
    let n = runs.len() as f64;
    let (loot, secs): (f64, f64) = runs.iter().fold((0.0, 0.0), |a, r| (a.0 + r.loot, a.1 + r.secs));
    let hour = |loot: f64, secs: f64, n: f64| (loot - cost * n) / (secs / 3600.0);
    Some(Profit {
        maps: runs.len(), loot: loot / n, cost, net: loot / n - cost, hour: hour(loot, secs, n),
        likely: (runs.len() >= 5).then(|| {
            // Draw as many maps again from your own, many times over: the middle 8 in 10 of what comes out.
            let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
            let mut pick = || { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; (seed % runs.len() as u64) as usize };
            let mut got: Vec<f64> = (0..400).map(|_| {
                let (l, s) = (0..runs.len()).map(|_| runs[pick()]).fold((0.0, 0.0), |a, r| (a.0 + r.loot, a.1 + r.secs));
                hour(l, s, n)
            }).collect();
            got.sort_by(f64::total_cmp);
            (got[40], got[359])
        }),
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
        let runs: Vec<_> = (0..6).map(|i| (i * 300, 250, Some(100.0), Some(7))).collect();
        let mine = runs_on(&runs, 7);
        assert_eq!(mine.len(), 6);
        let p = profit(&mine[..5], 40.0).unwrap();
        assert_eq!((p.maps, p.net, p.few), (5, 60.0, true));
        assert!((p.hour - 720.0).abs() < 0.01);
        assert_eq!(p.likely, Some((p.hour, p.hour)), "maps all alike leave no doubt");
        assert!(profit(&mine[..4], 40.0).unwrap().likely.is_none());
        assert!(profit(&[], 1.0).is_none());
    }

    #[test]
    fn unread_loot_and_other_trees_are_left_out() {
        let runs = vec![(0, 250, None, Some(1)), (300, 250, Some(10.0), Some(1)), (600, 250, Some(99.0), Some(2)), (9000, 100, Some(5.0), None)];
        assert_eq!(runs_on(&runs, 1), vec![RunMade { loot: 10.0, secs: 300.0 }]);
        assert_eq!(runs_on(&runs, 2), vec![RunMade { loot: 99.0, secs: 250.0 }], "an hour's break isn't map time");
    }

    #[test]
    fn likely_range_is_wider_when_maps_differ() {
        let runs: Vec<RunMade> = (0..20).map(|i| RunMade { loot: if i % 10 == 0 { 900.0 } else { 40.0 }, secs: 300.0 }).collect();
        let (lo, hi) = profit(&runs, 20.0).unwrap().likely.unwrap();
        let mid = profit(&runs, 20.0).unwrap().hour;
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
