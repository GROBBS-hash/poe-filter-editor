//! Atlas optimiser: a tree for the strategies you pick (e.g. Harvest + Breach), built by spending points where
//! they do the most for those strategies.
//!
//! Each node gets a value for the picked mechanics from three things:
//! - its text ("Breach" in the stats) and the area of the tree it sits in (the big area names, e.g. "Sacred Flora"
//!   is Harvest's);
//! - how many creator trees for those mechanics take it (`public/atlas_picks.json`, maxroll);
//! - how many creator trees take it at all (map sustain and other all-round nodes).
//!
//! Then, from the start (plus nodes you lock in, never through nodes you block), it keeps taking the path that
//! gives the most value per point, until the points run out or (for "fewest points") paths stop paying.

use crate::atlas::{graph, Step};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::OnceLock;

const TREE_JSON: &str = include_str!("../../public/atlas_poe1.json");
const PICKS_JSON: &str = include_str!("../../public/atlas_picks.json");

/// Atlas points you can spend (maxroll's planner counts to 150 in 3.30).
pub const MAX_POINTS: usize = 150;

/// Mechanic key, words in a node's text that mean it, and the tree's area names for it.
pub const MECHANICS: &[(&str, &[&str], &[&str])] = &[
    ("ritual", &["Ritual"], &["Ritual"]),
    ("harvest", &["Harvest", "Lifeforce", "Sacred Grove"], &["Sacred Flora"]),
    ("breach", &["Breach"], &["Breach"]),
    ("abyss", &["Abyss"], &["Abyss"]),
    ("legion", &["Legion"], &["Legion"]),
    ("blight", &["Blight"], &["Blight"]),
    ("essence", &["Essence"], &["Essence"]),
    ("betrayal", &["Syndicate", "Betrayal"], &["Betrayal"]),
    ("incursion", &["Incursion", "Alva", "Temple"], &["Incursion"]),
    ("expedition", &["Expedition", "Logbook", "Remnant"], &["Expedition"]),
    ("bestiary", &["Beast", "Einhar"], &["Bestiary"]),
    ("ultimatum", &["Ultimatum"], &["Ultimatum"]),
    ("delirium", &["Delirium", "Simulacrum"], &["Delirium"]),
    ("shrines", &["Shrine"], &["Shrines"]),
    ("bosses", &["Map Boss", "Unique Boss", "Conqueror"], &["Unique Monsters", "Conquerors", "The Shaper and Elder", "The Searing Exarch", "The Eater of Worlds"]),
    ("rare monsters", &["Rare Monster"], &["Rare Monsters"]),
    ("magic monsters", &["Magic Monster"], &["Magic Monsters"]),
    ("memories", &["Memor"], &["Atlas Memories"]),
    ("settlers", &["Kalguur", "Settler"], &["Settlers of Kalguur"]),
    ("mercenaries", &["Mercenar"], &["Mercenaries"]),
    ("strongboxes", &["Strongbox"], &["Strongboxes"]),
    ("rogue exiles", &["Rogue Exile"], &["Rogue Exiles"]),
    ("divination cards", &["Divination Card"], &["Divination Cards"]),
    ("torment", &["Torment", "Possess"], &["Torment"]),
    ("mirage", &["Mirage"], &["Mirage"]),
    ("gold", &[], &["Gold"]),
];

struct Node {
    kind: u8,
    name: String,
    text: String,
    /// Mechanics the text names (1.0) or whose area it sits in (0.7).
    tags: HashMap<&'static str, f32>,
}

struct PickTree {
    mechanics: Vec<String>,
    nodes: HashSet<u32>,
}

struct Data {
    nodes: HashMap<u32, Node>,
    choices: HashMap<u32, Vec<(u32, String)>>,
    picks: Vec<PickTree>,
}

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| {
        let v: Value = serde_json::from_str(TREE_JSON).expect("atlas tree data");
        // Area names sit at the middle of their group: a node belongs to the nearest one close enough.
        let labels: Vec<(f64, f64, String)> = v["labels"].as_array().into_iter().flatten()
            .filter_map(|l| Some((l[0].as_f64()?, l[1].as_f64()?, l[2].as_str()?.to_string()))).collect();
        let mut nodes = HashMap::new();
        for (id, n) in v["nodes"].as_object().into_iter().flatten() {
            let Ok(id) = id.parse::<u32>() else { continue };
            let (x, y) = (n[0].as_f64().unwrap_or(0.0), n[1].as_f64().unwrap_or(0.0));
            let name = n[3].as_str().unwrap_or("").to_string();
            let stats: Vec<&str> = n[4].as_array().into_iter().flatten().filter_map(|s| s.as_str()).collect();
            let text = format!("{name}\n{}", stats.join("\n"));
            let region = labels.iter()
                .map(|(lx, ly, l)| ((lx - x).powi(2) + (ly - y).powi(2), l))
                .filter(|(d, _)| *d < 800.0 * 800.0)
                .min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, l)| l.clone());
            let mut tags = HashMap::new();
            for (m, words, areas) in MECHANICS {
                if words.iter().any(|w| text.contains(w)) { tags.insert(*m, 1.0); }
                else if region.as_ref().is_some_and(|r| areas.contains(&r.as_str())) { tags.insert(*m, 0.7); }
            }
            nodes.insert(id, Node { kind: n[2].as_u64().unwrap_or(0) as u8, name, text, tags });
        }
        let mut choices: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
        for (node, effects) in v["choices"].as_object().into_iter().flatten() {
            let Ok(node) = node.parse::<u32>() else { continue };
            let mut list: Vec<(u32, String)> = effects.as_object().into_iter().flatten()
                .filter_map(|(e, t)| Some((e.parse().ok()?, t.as_str()?.to_string()))).collect();
            list.sort();
            choices.insert(node, list);
        }
        let p: Value = serde_json::from_str(PICKS_JSON).unwrap_or(Value::Null);
        let g = graph();
        let ids = |x: &Value| -> Vec<u32> { x.as_array().into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| n as u32).collect() };
        let picks = p["picks"].as_array().into_iter().flatten().filter_map(|pk| {
            let targets = ids(&pk["targets"]);
            if targets.is_empty() { return None; }
            let exclude: HashSet<u32> = ids(&pk["exclude"]).into_iter().collect();
            let nodes: HashSet<u32> = g.fill_paths(&targets, &exclude).into_iter().collect();
            let mechanics = pk["mechanics"].as_array().into_iter().flatten().filter_map(|m| m.as_str()).map(String::from).collect();
            Some(PickTree { mechanics, nodes })
        }).collect();
        Data { nodes, choices, picks }
    })
}

/// "Most profit" fills every point; "balanced" and "fewest" stop once paths pay less per point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority { #[default] Profit, Balanced, Fewest }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Ask {
    pub mechanics: Vec<String>,
    #[serde(default)]
    pub priority: Priority,
    /// Points to spend at most (0 = all).
    #[serde(default)]
    pub budget: usize,
    /// Nodes you always want, and nodes you never want.
    #[serde(default)]
    pub lock: Vec<u32>,
    #[serde(default)]
    pub block: Vec<u32>,
}

/// A node doing work in the tree (or not), in plain words.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NodeWork {
    pub node: u32,
    pub name: String,
    pub value: f32,
    /// e.g. "Harvest node · taken by 4 of 5 Harvest trees".
    pub why: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    /// Start node first, then in the order taken (most useful first).
    pub nodes: Vec<u32>,
    pub steps: Vec<Step>,
    pub choices: Vec<(u32, u32)>,
    pub points: usize,
    pub budget: usize,
    /// Notables doing the most for your strategies.
    pub top: Vec<NodeWork>,
    /// Notables taken only because a path runs through them.
    pub weak: Vec<NodeWork>,
    /// Creator trees it learned from for these mechanics.
    pub trees_used: usize,
}

/// Values for every node, for the picked mechanics.
struct Values { v: HashMap<u32, f32>, rel: HashMap<u32, f32>, pop: HashMap<u32, (usize, usize)> }

fn values(mechs: &[String]) -> Values {
    let d = data();
    let wanted: Vec<&str> = mechs.iter().map(|m| m.as_str()).collect();
    let fits: Vec<&PickTree> = d.picks.iter().filter(|p| p.mechanics.iter().any(|m| wanted.contains(&m.as_str()))).collect();
    let all = d.picks.len().max(1) as f32;
    let (mut v, mut rel, mut pop) = (HashMap::new(), HashMap::new(), HashMap::new());
    for (&id, n) in &d.nodes {
        let r: f32 = wanted.iter().map(|m| n.tags.get(m).copied().unwrap_or(0.0)).sum();
        let taken = fits.iter().filter(|p| p.nodes.contains(&id)).count();
        let p = if fits.is_empty() { 0.0 } else { taken as f32 / fits.len() as f32 };
        let g = d.picks.iter().filter(|p| p.nodes.contains(&id)).count() as f32 / all;
        let weight = if matches!(n.kind, 1 | 2) { 3.0 } else { 1.0 };
        v.insert(id, weight * r + 2.0 * p + 0.3 * g);
        rel.insert(id, r);
        pop.insert(id, (taken, fits.len()));
    }
    Values { v, rel, pop }
}

fn label(m: &str) -> String {
    let mut c = m.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Builds a tree for the strategies asked for.
pub fn optimise(ask: &Ask) -> Plan {
    let g = graph();
    let d = data();
    let vals = values(&ask.mechanics);
    let budget = if ask.budget == 0 { MAX_POINTS } else { ask.budget.min(MAX_POINTS) };
    let block: HashSet<u32> = ask.block.iter().copied().filter(|b| *b != g.start).collect();
    // Locked nodes first (joined up the shortest way).
    let mut order = g.fill_paths(&ask.lock, &block);
    let mut have: HashSet<u32> = order.iter().copied().collect();
    let locked = order.len();
    loop {
        let left = (budget + 1).saturating_sub(order.len());
        if left == 0 { break; }
        // Shortest ways from everything taken to every other node.
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut dist: HashMap<u32, usize> = have.iter().map(|n| (*n, 0)).collect();
        let mut queue: VecDeque<u32> = have.iter().copied().collect();
        while let Some(n) = queue.pop_front() {
            let dn = dist[&n];
            if dn >= left { continue; }
            for &m in g.neighbours(n) {
                if block.contains(&m) || dist.contains_key(&m) { continue; }
                dist.insert(m, dn + 1);
                prev.insert(m, n);
                queue.push_back(m);
            }
        }
        // The path with the most value per point.
        let mut best: Option<(f32, f32, u32)> = None;
        for (&t, &dt) in &dist {
            if dt == 0 || vals.v.get(&t).copied().unwrap_or(0.0) <= 0.0 { continue; }
            let mut gain = 0.0;
            let mut cur = t;
            while !have.contains(&cur) { gain += vals.v.get(&cur).copied().unwrap_or(0.0); cur = prev[&cur]; }
            let ratio = gain / dt as f32;
            let better = match best { None => true, Some((r, gn, id)) => ratio > r + 1e-6 || ((ratio - r).abs() <= 1e-6 && (gain > gn || (gain == gn && t < id))) };
            if better { best = Some((ratio, gain, t)); }
        }
        let Some((ratio, _, t)) = best else { break };
        if ratio <= 0.0 { break; }
        let mut path = vec![];
        let mut cur = t;
        while !have.contains(&cur) { path.push(cur); cur = prev[&cur]; }
        path.reverse();
        for p in path { have.insert(p); order.push(p); }
    }
    // Taken best first, so "fewest points" keeps the start that gives 60% of the value and "balanced" 85%.
    // Any start of the order is joined up (each node joins one taken before it).
    let keep = match ask.priority { Priority::Profit => 1.0, Priority::Balanced => 0.85, Priority::Fewest => 0.6 };
    if keep < 1.0 {
        let total: f32 = order[locked..].iter().map(|n| vals.v.get(n).copied().unwrap_or(0.0)).sum();
        let mut sum = 0.0;
        for i in locked..order.len() {
            sum += vals.v.get(&order[i]).copied().unwrap_or(0.0);
            if sum >= keep * total { order.truncate(i + 1); break; }
        }
    }

    let wanted: Vec<&str> = ask.mechanics.iter().map(|m| m.as_str()).collect();
    let names: Vec<String> = wanted.iter().map(|m| label(m)).collect();
    let why = |id: u32| -> String {
        let n = &d.nodes[&id];
        let mut parts = vec![];
        let hits: Vec<String> = wanted.iter().filter(|m| n.tags.contains_key(**m)).map(|m| label(m)).collect();
        if !hits.is_empty() { parts.push(format!("{} node", hits.join(" + "))); }
        let (taken, of) = vals.pop[&id];
        if of > 0 && taken > 0 { parts.push(format!("taken by {taken} of {of} {} trees", names.join("/"))); }
        if parts.is_empty() { "on the way to better nodes".into() } else { parts.join(" · ") }
    };
    let mut top: Vec<NodeWork> = order.iter().filter(|id| matches!(g.kind(**id), 1 | 2))
        .map(|&id| NodeWork { node: id, name: g.name(id).to_string(), value: vals.v[&id], why: why(id) }).collect();
    top.sort_by(|a, b| b.value.total_cmp(&a.value));
    let weak: Vec<NodeWork> = top.iter().filter(|w| vals.rel[&w.node] == 0.0 && {
        let (t, of) = vals.pop[&w.node]; of == 0 || (t as f32) < 0.25 * of as f32 }).cloned().collect();
    top.retain(|w| !weak.iter().any(|x| x.node == w.node));
    top.truncate(12);

    // Multiple-choice notables: the effect that names a picked mechanic.
    let choices = order.iter().filter_map(|id| {
        let list = d.choices.get(id)?;
        let pick = list.iter().max_by_key(|(_, t)| {
            MECHANICS.iter().filter(|(m, ..)| wanted.contains(m)).filter(|(_, words, _)| words.iter().any(|w| t.contains(w))).count()
        })?;
        let named = MECHANICS.iter().any(|(m, words, _)| wanted.contains(m) && words.iter().any(|w| pick.1.contains(w)));
        named.then_some((*id, pick.0))
    }).collect();

    let fits = d.picks.iter().filter(|p| p.mechanics.iter().any(|m| wanted.contains(&m.as_str()))).count();
    Plan { steps: g.steps(&order), points: order.len() - 1, nodes: order, choices, budget, top, weak, trees_used: fits }
}

/// Every mechanic the optimiser knows (for the picker), with how many atlas nodes it has.
pub fn mechanic_sizes() -> Vec<(String, usize)> {
    let d = data();
    MECHANICS.iter().map(|(m, ..)| (m.to_string(), d.nodes.values().filter(|n| n.tags.contains_key(m)).count())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(m: &[&str], p: Priority) -> Ask { Ask { mechanics: m.iter().map(|s| s.to_string()).collect(), priority: p, ..Default::default() } }

    #[test]
    fn areas_and_words_tag_nodes() {
        let d = data();
        let find = |name: &str| d.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(find("Bountiful Harvest").tags.get("harvest"), Some(&1.0));
        assert!(find("Cassia's Pride").tags.contains_key("blight"));
        assert!(d.picks.len() >= 15, "creator trees load");
        for (m, n) in mechanic_sizes() { assert!(n > 0, "{m} has no nodes"); }
        let _ = find("Bountiful Harvest").text.len();
    }

    #[test]
    fn harvest_breach_tree() {
        let g = graph();
        let p = optimise(&ask(&["harvest", "breach"], Priority::Profit));
        assert_eq!(p.points, MAX_POINTS);
        assert_eq!(p.nodes[0], g.start);
        // Every point joins one taken before it.
        for (i, n) in p.nodes.iter().enumerate().skip(1) {
            assert!(p.nodes[..i].iter().any(|m| g.neighbours(*n).contains(m)), "{n} isn't joined");
        }
        let d = data();
        let tagged = |m: &str| p.nodes.iter().filter(|n| d.nodes[n].tags.contains_key(m)).count();
        assert!(tagged("harvest") >= 5 && tagged("breach") >= 10, "harvest {} breach {}", tagged("harvest"), tagged("breach"));
        assert!(!p.top.is_empty());
        let fewest = optimise(&ask(&["harvest", "breach"], Priority::Fewest));
        let balanced = optimise(&ask(&["harvest", "breach"], Priority::Balanced));
        assert!(fewest.points <= balanced.points && balanced.points <= p.points, "{} {} {}", fewest.points, balanced.points, p.points);
        assert!(fewest.points >= 20, "fewest still builds something: {}", fewest.points);
    }

    #[test]
    fn lock_and_block() {
        let g = graph();
        let base = optimise(&ask(&["breach"], Priority::Profit));
        let some_breach = base.top[0].node;
        let mut a = ask(&["breach"], Priority::Profit);
        a.block = vec![some_breach];
        let blocked = optimise(&a);
        assert!(!blocked.nodes.contains(&some_breach));
        // Lock a far-away Blight keystone: it's in, joined up.
        let cassia = data().nodes.iter().find(|(_, n)| n.name == "Cassia's Pride").map(|(id, _)| *id).unwrap();
        let mut b = ask(&["breach"], Priority::Fewest);
        b.lock = vec![cassia];
        let locked = optimise(&b);
        assert!(locked.nodes.contains(&cassia));
        assert_eq!(locked.nodes[0], g.start);
        let mut c = ask(&["breach"], Priority::Profit);
        c.budget = 40;
        assert_eq!(optimise(&c).points, 40);
    }
}

/// Dev look at a plan: `ATLAS_OPT=harvest,breach cargo test -p filter-core --lib atlas_opt::show -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn show() {
    let m: Vec<String> = std::env::var("ATLAS_OPT").unwrap_or("harvest,breach".into()).split(',').map(String::from).collect();
    for pr in [Priority::Fewest, Priority::Balanced, Priority::Profit] {
        let p = optimise(&Ask { mechanics: m.clone(), priority: pr, ..Default::default() });
        println!("{pr:?}: {} points, {} trees", p.points, p.trees_used);
        if let Ok(dir) = std::env::var("ATLAS_OPT_OUT") {
            let _ = std::fs::write(format!("{dir}/plan_{pr:?}.json"), serde_json::to_string(&p).unwrap());
        }
        for w in &p.top { println!("  + {} ({:.1}) {}", w.name, w.value, w.why); }
        for w in &p.weak { println!("  - {} {}", w.name, w.why); }
    }
}
