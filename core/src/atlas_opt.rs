//! Atlas builder: a tree for what you want to farm (mechanics like Harvest + Breach, or goals like boss rush).
//!
//! - What a tree is worth: `atlas_model.rs` (every node's real effect, how effects multiply, what people farming it take).
//! - Finding one: `atlas_search.rs`.
//! - Here: what you can ask for, one-thing-per-map choices (Eater / Exarch / Maven, Unstable Breaches / Hives), the reasons shown
//!   for each notable, and which mechanic a node belongs to (`points_in`, used by the farm finder).
//!
//! The rules and numbers come from ATLAS_KNOWLEDGE.md (25 Nov 2026).

use crate::atlas::{graph, Step};
use crate::atlas_model::{self, model, Problem, Setup, Shares};
use crate::atlas_search::Search;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const TREE_JSON: &str = include_str!("../../public/atlas_poe1.json");

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
    // Travel nodes (he asked 23 Nov): all-round map juice you path through on the way to your mechanics.
    ("pack size", &["increased Pack Size in your Maps", "Monster Packs"], &[]),
    ("explicit effect", &["effect of Explicit Modifiers on your Maps"], &[]),
    ("threads", &["Threads found"], &[]),
    ("maps found", &["Maps found in your Maps", "Map Tier"], &[]),
    ("rarity", &["Rarity of Items found in your Maps"], &[]),
];

/// The travel options, not mechanics.
pub const TRAVEL: &[&str] = &["pack size", "explicit effect", "threads", "maps found", "rarity"];

#[allow(dead_code)] // name / text: read by the tests and the dev printout
struct Node {
    kind: u8,
    name: String,
    text: String,
    /// Mechanics the text names (1.0) or whose area it sits in (0.7).
    tags: HashMap<&'static str, f32>,
}

struct Data {
    nodes: HashMap<u32, Node>,
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
        Data { nodes }
    })
}

/// "Most profit" fills every point; "balanced" and "fewest" spend about two thirds and two fifths of them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Priority { #[default] Profit, Balanced, Fewest }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ask {
    pub mechanics: Vec<String>,
    /// Goals that aren't a mechanic: "currency", "threads", "maps", "terror", "destructive", "rush" (atlas_model.json).
    pub goals: Vec<String>,
    pub priority: Priority,
    /// Points to spend at most (0 = as the priority says).
    pub budget: usize,
    /// Nodes you always want, and nodes you never want.
    pub lock: Vec<u32>,
    pub block: Vec<u32>,
    /// Threads per map ("legion", 8). Empty: eight of each picked one, and cheap ones up to three kinds.
    pub threads: Vec<(String, u32)>,
    /// "eater", "exarch", "maven", "none"; anything else = whichever pays more.
    pub altar: String,
    /// What every map has anyway: "mirage", "delirium"…
    pub map_has: Vec<String>,
    /// "plain", "rolled" (default), "juiced".
    pub map: String,
    /// "weak", "normal" (default), "strong".
    pub build: String,
    /// Today's prices by item name, put in by the app ("Legion Thread", "Deafening Essence of Zeal"…).
    #[serde(skip)]
    pub prices: HashMap<String, f64>,
}

/// A node doing work in the tree (or not), in plain words.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NodeWork {
    pub node: u32,
    pub name: String,
    pub value: f32,
    /// How much less the whole plan would make without it, by the maths (0.11 = 11%).
    #[serde(default)]
    pub gain: f32,
    /// Share of trees for these mechanics that take it.
    #[serde(default)]
    pub share: f32,
    /// "both" (maths and players agree), "maths", "players" or "path".
    #[serde(default)]
    pub sure: String,
    /// e.g. "more monsters, better drops: Harvest pays 11% more".
    pub why: String,
}

/// Where the plan's income comes from.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Part { pub key: String, pub label: String, pub share: f32 }

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    /// Start node first, then in the order taken (most useful first).
    pub nodes: Vec<u32>,
    pub steps: Vec<Step>,
    pub choices: Vec<(u32, u32)>,
    pub points: usize,
    pub budget: usize,
    /// Notables doing the most for your plan.
    pub top: Vec<NodeWork>,
    /// Notables taken only because a path runs through them.
    pub weak: Vec<NodeWork>,
    /// Trees for these mechanics it learned from.
    pub trees_used: usize,
    /// Plain-words notes, e.g. "Eater of Worlds altars: a map has Eater, Exarch or the Maven".
    #[serde(default)]
    pub notes: Vec<String>,
    /// Traps it left out: (name, why).
    #[serde(default)]
    pub left_out: Vec<(String, String)>,
    /// How to play it.
    #[serde(default)]
    pub tips: Vec<String>,
    /// Threads the plan runs, and "eater" / "exarch" / "maven" / "".
    #[serde(default)]
    pub threads: Vec<(String, u32)>,
    #[serde(default)]
    pub altar: String,
    #[serde(default)]
    pub parts: Vec<Part>,
}

/// The goals there are: (key, label).
pub fn goals() -> Vec<(String, String)> { model().goals.iter().map(|(k, g)| (k.clone(), g.label.clone())).collect() }

/// The Threads a plan runs when none are given: eight of each picked one (three kinds at most), the rest cheap fillers.
fn threads_for(mechs: &[String]) -> Vec<(String, u32)> {
    let m = model();
    let mut t: Vec<(String, u32)> = mechs.iter().filter(|x| m.primaries.contains(x)).take(3).map(|x| (x.clone(), 8)).collect();
    for c in &m.cheap_threads { if t.len() < 3 && !t.iter().any(|x| &x.0 == c) { t.push((c.clone(), 1)); } }
    t
}

fn setup(ask: &Ask) -> Setup {
    let m = model();
    let goals: Vec<_> = ask.goals.iter().filter_map(|g| m.goals.get(g)).collect();
    let mut mechs: Vec<String> = ask.mechanics.iter().filter(|x| !TRAVEL.contains(&x.as_str())).map(|x| atlas_model::shares_key(x).to_string()).collect();
    for g in &goals { for p in &g.prior { if !mechs.contains(p) { mechs.push(p.clone()); } } }
    let mut pick: HashSet<String> = ask.mechanics.iter().flat_map(|x| atlas_model::sources_of(x)).collect();
    let mut boost: HashMap<String, f64> = HashMap::new();
    let mut lock: Vec<(u32, Option<u32>)> = ask.lock.iter().map(|id| (*id, None)).collect();
    for g in &goals {
        pick.extend(g.pick.iter().cloned());
        for (k, v) in &g.boost { let b = boost.entry(k.clone()).or_insert(1.0); *b = b.max(*v); }
        lock.extend(g.lock.iter().filter(|id| !ask.block.contains(id)).map(|id| (*id, None)));
    }
    let threads: Vec<(String, u32)> = if ask.threads.is_empty() { threads_for(&ask.mechanics) } else { ask.threads.iter().filter(|t| t.1 > 0).take(3).cloned().collect() };
    let mut block: HashSet<u32> = ask.block.iter().copied().filter(|b| *b != graph().start).collect();
    // Unwavering Vision (one kind of Thread) only suits a plan with exactly one main kind.
    if threads.iter().filter(|t| t.1 >= 6).count() != 1 && !ask.lock.contains(&65225) { block.insert(65225); }
    Setup {
        pick, mechs,
        wants: ask.mechanics.clone(),
        threads,
        map_has: ask.map_has.clone(),
        map: m.maps.get(&ask.map).or(m.maps.get("rolled")).copied().expect("map kinds"),
        eldritch: None,
        maven: ask.altar == "maven" || goals.iter().any(|g| g.maven),
        terror: goals.iter().any(|g| g.terror),
        rush: goals.iter().any(|g| g.rush),
        strength: match ask.build.as_str() { "weak" => 0.3, "strong" => 1.0, _ => 0.7 },
        boost, lock,
        block,
        prices: m.thread_prices.keys().filter_map(|t| Some((t.clone(), *ask.prices.get(&format!("{t} Thread")).filter(|p| **p > 0.0)?))).collect(),
    }
}

/// Builds a tree for the strategies asked for.
pub fn optimise(ask: &Ask) -> Plan { optimise_with(ask, atlas_model::shares()) }

fn optimise_with(ask: &Ask, shares: &Shares) -> Plan {
    let g = graph();
    let m = model();
    let base = setup(ask);
    let budget = if ask.budget > 0 { ask.budget.min(MAX_POINTS) } else { match ask.priority { Priority::Profit => MAX_POINTS, Priority::Balanced => 100, Priority::Fewest => 60 } };

    // One thing per map: Eater, Exarch or the Maven, and Unstable Breaches or Hives. Each is tried, the best wins.
    let sides: Vec<Option<&str>> = if base.maven { vec![None] } else { match ask.altar.as_str() { "eater" => vec![Some("eater")], "exarch" => vec![Some("exarch")], "none" => vec![None], _ => vec![Some("eater"), Some("exarch")] } };
    let modes: Vec<Option<&atlas_model::Mode>> = {
        let v: Vec<_> = m.modes.iter().filter(|x| ask.mechanics.contains(&x.when) && !base.block.contains(&x.node)).map(Some).collect();
        if v.is_empty() { vec![None] } else { v }
    };
    let mut best: Option<(Problem, crate::atlas_search::Tree, Option<&atlas_model::Mode>)> = None;
    for side in &sides {
        for mode in &modes {
            let mut s = base.clone();
            s.eldritch = side.map(String::from);
            if let Some(x) = mode { s.lock.retain(|l| l.0 != x.node); s.lock.push((x.node, Some(x.option))); }
            let p = Problem::new(s, shares);
            let t = Search::new(&p).solve(budget, 10);
            if best.as_ref().is_none_or(|b| t.u > b.1.u + 1e-9) { best = Some((p, t, *mode)); }
        }
    }
    let (p, t, mode) = best.expect("at least one way");
    let mut s = Search::new(&p);
    let t = s.improve(t, budget, 40);
    let nodes = s.order(&t);

    // Why each notable is in: the plan with and without it.
    let names: Vec<String> = p.setup.mechs.iter().map(|x| atlas_model::cap(x)).collect();
    let with = p.detail(&t.x);
    let pc = p.prior_channel();
    let maths = |x: &[f64]| p.eval(x) - Problem::lambda() * x[pc];
    let u0 = maths(&t.x);
    let share_of = |id: u32| p.setup.mechs.iter().filter_map(|k| shares.mechs.get(k)).map(|k| k.n.get(&id).copied().unwrap_or(0.0)).fold(0.0f64, f64::max)
        .max(if p.setup.mechs.is_empty() { shares.all.get(&id).copied().unwrap_or(0.0) } else { 0.0 });
    let ix: HashMap<u32, usize> = p.ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut works: Vec<NodeWork> = nodes.iter().filter(|id| matches!(g.kind(**id), 1 | 2)).map(|&id| {
        let mut w = t.clone();
        s.remove(&mut w, ix[&id]);
        let without = p.detail(&w.x);
        let gain = (u0 - maths(&w.x)).exp() - 1.0;
        let share = share_of(id);
        let mut parts = vec![];
        let said = atlas_model::said(&with, &without);
        if !said.is_empty() { parts.push(said.iter().take(2).copied().collect::<Vec<_>>().join(", ")); }
        // The source it does most for.
        let top = with.src.iter().map(|a| { let b = without.src.iter().find(|b| b.key == a.key); (a, b.map(|b| b.income).unwrap_or(0.0), b.map(|b| b.n).unwrap_or(0.0)) })
            .max_by(|a, b| (a.0.income - a.1).total_cmp(&(b.0.income - b.1)));
        if let Some((a, was, n)) = top.filter(|(a, was, _)| a.income - was > with.total * 0.001) {
            if (a.n - n).abs() > 0.02 { parts.push(if a.n < 1.0 { format!("{}: {:.0}% of maps instead of {:.0}%", a.label, a.n * 100.0, n * 100.0) } else { format!("{}: {:.1} a map instead of {:.1}", a.label, a.n, n) }); }
            else if was > 0.0 && a.income / was >= 1.01 { parts.push(format!("{} +{:.0}%", a.label, (a.income / was - 1.0) * 100.0)); }
        }
        let paying = gain >= 0.004;
        if share >= 0.15 { parts.push(format!("{:.0}% of {}trees take it", share * 100.0, if names.is_empty() { String::new() } else { names.join(" / ") + " " })); }
        let sure = if paying && share >= 0.45 { "both" } else if paying { "maths" } else if share >= 0.35 { "players" } else { "path" };
        let value = gain as f32 + share as f32 * 0.04;
        NodeWork { node: id, name: g.name(id).to_string(), value, gain: gain as f32, share: share as f32, sure: sure.into(), why: if parts.is_empty() { "on the way to better nodes".into() } else { parts.join(" · ") } }
    }).collect();
    works.sort_by(|a, b| b.value.total_cmp(&a.value));
    let (weak, top): (Vec<_>, Vec<_>) = works.into_iter().partition(|w| w.sure == "path" && !ask.lock.contains(&w.node));

    // Options: the model's, or the one whose item sells for most today.
    let choices: Vec<(u32, u32)> = nodes.iter().filter_map(|id| {
        let i = ix[id];
        let a = &p.alts[i];
        if a.len() < 2 { return None; }
        let mut o = a[t.opt[i]].id;
        if let Some(items) = m.option_items.get(id) {
            let dear = items.iter().filter_map(|(opt, item)| Some((*opt, *ask.prices.get(item)?))).max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
            if let Some((opt, price)) = dear { if price > 0.0 { o = opt; } }
        }
        Some((*id, o))
    }).collect();

    let altar = if p.setup.maven { "maven" } else { p.setup.eldritch.as_deref().unwrap_or("") }.to_string();
    let mut notes = vec![];
    let uses = |f: &str| nodes.iter().any(|id| atlas_model::family().get(id).copied() == Some(f));
    match altar.as_str() {
        "eater" if uses("eater") => notes.push("Eater of Worlds altars: a map has Eater, Exarch or the Maven, so the others are left out".to_string()),
        "exarch" if uses("exarch") => notes.push("Searing Exarch altars: a map has Eater, Exarch or the Maven, so the others are left out".to_string()),
        "maven" => notes.push("The Maven watches these maps, so no Eldritch altars".to_string()),
        _ => {}
    }
    if let Some(x) = mode { notes.push(format!("{}: a map has Unstable Breaches or Hives, never both", x.label)); }
    let mut fams: HashSet<&str> = p.setup.mechs.iter().map(|x| x.as_str()).collect();
    fams.extend(ask.mechanics.iter().map(|x| x.as_str()));
    let mut left_out: Vec<(String, String)> = m.never.iter().filter(|(id, _)| !nodes.contains(id) && atlas_model::family().get(id).is_some_and(|f| fams.contains(f))).map(|(id, why)| (g.name(*id).to_string(), why.clone())).collect();
    left_out.sort();
    let tips = m.tips.iter().filter(|(ms, _)| ms.iter().all(|x| ask.mechanics.contains(x))).map(|(_, t)| t.clone()).collect();
    let mut parts: Vec<Part> = with.src.iter().filter(|x| x.income > 0.0).map(|x| Part { key: x.key.into(), label: x.label.into(), share: (x.income / with.total) as f32 }).collect();
    parts.sort_by(|a, b| b.share.total_cmp(&a.share));
    parts.retain(|x| x.share >= 0.03);
    let trees_used = p.setup.mechs.iter().filter_map(|k| shares.mechs.get(k)).map(|k| k.w).sum::<f64>().ceil() as usize;
    Plan { steps: g.steps(&nodes), points: nodes.len() - 1, choices, budget, top, weak, trees_used, notes, left_out, tips, threads: p.setup.threads.clone(), altar, parts, nodes }
}

/// Every mechanic the optimiser knows (for the picker), with how many atlas nodes it has.
pub fn mechanic_sizes() -> Vec<(String, usize)> {
    let d = data();
    MECHANICS.iter().map(|(m, ..)| (m.to_string(), d.nodes.values().filter(|n| n.tags.contains_key(m)).count())).collect()
}

/// How many of these nodes are each mechanic's (its text names it, or it sits in its area), most first.
pub fn points_in(nodes: &[u32]) -> Vec<(&'static str, usize)> {
    let d = data();
    let mut n: HashMap<&'static str, usize> = HashMap::new();
    for id in nodes {
        for (m, w) in d.nodes.get(id).into_iter().flat_map(|x| x.tags.iter()) { if *w >= 0.7 { *n.entry(*m).or_default() += 1; } }
    }
    let mut v: Vec<(&'static str, usize)> = n.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(m: &[&str], p: Priority) -> Ask { Ask { mechanics: m.iter().map(|s| s.to_string()).collect(), priority: p, ..Default::default() } }
    fn joined(p: &Plan) {
        let g = graph();
        assert_eq!(p.nodes[0], g.start);
        for (i, n) in p.nodes.iter().enumerate().skip(1) {
            assert!(p.nodes[..i].iter().any(|m| g.neighbours(*n).contains(m)), "{n} isn't joined");
        }
    }
    fn fam(id: u32) -> &'static str { atlas_model::family().get(&id).copied().unwrap_or("") }
    fn notables(p: &Plan) -> Vec<u32> { p.nodes.iter().copied().filter(|id| matches!(graph().kind(*id), 1 | 2)).collect() }

    #[test]
    fn areas_and_words_tag_nodes() {
        let d = data();
        let find = |name: &str| d.nodes.values().find(|n| n.name == name).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(find("Bountiful Harvest").tags.get("harvest"), Some(&1.0));
        assert!(find("Cassia's Pride").tags.contains_key("blight"));
        for (m, n) in mechanic_sizes() { assert!(n > 0, "{m} has no nodes"); }
    }

    #[test]
    fn a_whole_tree_the_same_every_time() {
        let p = optimise(&ask(&["harvest"], Priority::Profit));
        assert_eq!(p.points, MAX_POINTS);
        joined(&p);
        assert_eq!(p.nodes, optimise(&ask(&["harvest"], Priority::Profit)).nodes, "same ask, same tree");
        let (few, mid) = (optimise(&ask(&["harvest"], Priority::Fewest)), optimise(&ask(&["harvest"], Priority::Balanced)));
        assert!(few.points < mid.points && mid.points < p.points);
        joined(&few);
        // What every Harvest farmer takes, and no trap.
        for id in [39276, 45664, 54874] { assert!(few.nodes.contains(&id), "missing {}", graph().name(id)); }
        assert!(!p.nodes.contains(&57163), "Heart of the Grove earns less");
        // The wild points have the plan's multipliers, not only its words.
        assert!(p.nodes.contains(&34393), "Mounting Modifiers");
        assert!(!p.top.is_empty() && p.top[0].why.len() > 5, "{:?}", p.top.first());
    }

    #[test]
    fn one_thing_per_map() {
        for a in [ask(&["breach"], Priority::Profit), ask(&["legion", "abyss"], Priority::Profit)] {
            let p = optimise(&a);
            let n = notables(&p);
            assert!(!(n.contains(&33008) && n.contains(&48336)), "Gaze and Wrath together");
            let (u, h) = ([3204u32, 14287, 21096], [27459u32, 38684, 14402]);
            assert!(!(n.iter().any(|x| u.contains(x)) && n.iter().any(|x| h.contains(x))), "Unstable and Hive notables: {:?}", p.notes);
        }
        // The Maven's extra bosses and Eldritch altars can't share a map.
        let mut m = ask(&[], Priority::Profit);
        m.goals = vec!["destructive".into()];
        let p = optimise(&m);
        assert!(p.nodes.contains(&34384) && p.altar == "maven");
        assert!(![33008u32, 48336, 27261, 41053].iter().any(|x| p.nodes.contains(x)), "altar notables in a Maven tree");
        // Exarch asked for: no Eater notables.
        let mut e = ask(&["legion"], Priority::Profit);
        e.altar = "exarch".into();
        let q = optimise(&e);
        assert!(!q.nodes.contains(&33008) && q.altar == "exarch");
    }

    #[test]
    fn what_the_map_has_and_what_it_lacks() {
        // No Abyss Thread in the plan: no Abyss notables. A Delirium mirror in every map: none of its chance nodes.
        let p = optimise(&ask(&["essence"], Priority::Profit));
        assert!(!notables(&p).iter().any(|id| fam(*id) == "abyss"), "{:?}", notables(&p).iter().filter(|i| fam(**i) == "abyss").map(|i| graph().name(*i)).collect::<Vec<_>>());
        let mut d = ask(&["delirium"], Priority::Profit);
        d.map_has = vec!["delirium".into()];
        let q = optimise(&d);
        assert!(!q.nodes.contains(&15324) && !q.nodes.contains(&42605));
        assert_eq!(optimise(&ask(&["legion"], Priority::Fewest)).threads[0], ("legion".to_string(), 8));
    }

    #[test]
    fn goals_change_the_tree() {
        let goal = |g: &str| { let mut a = Ask::default(); a.goals = vec![g.into()]; optimise(&a) };
        let (maps, terror, rush) = (goal("maps"), goal("terror"), goal("rush"));
        assert!(maps.nodes.contains(&43903), "5% maps per map mod");
        assert!(terror.nodes.contains(&18476) && terror.nodes.contains(&20503), "{:?}", terror.top.iter().map(|w| &w.name).collect::<Vec<_>>());
        assert_eq!(terror.choices.iter().find(|c| c.0 == 27261).map(|c| c.1), Some(15113), "altars pay on bosses");
        assert!(rush.nodes.contains(&64222), "vaults from anomalies");
        for p in [&maps, &terror, &rush] { joined(p); }
    }

    #[test]
    fn options_follow_prices() {
        let mut a = ask(&["harvest"], Priority::Profit);
        a.lock = vec![12826];
        a.prices = [("Wild Crystallised Lifeforce".to_string(), 0.9), ("Vivid Crystallised Lifeforce".to_string(), 0.1)].into();
        assert!(optimise(&a).choices.contains(&(12826, 47548)), "the dearest lifeforce");
    }

    #[test]
    fn lock_and_block() {
        let base = optimise(&ask(&["breach"], Priority::Fewest));
        let some = base.top.iter().find(|w| w.node != 19729).unwrap().node;
        let mut a = ask(&["breach"], Priority::Fewest);
        a.block = vec![some];
        assert!(!optimise(&a).nodes.contains(&some));
        // Lock a far-away Blight keystone: it's in, joined up.
        let cassia = data().nodes.iter().find(|(_, n)| n.name == "Cassia's Pride").map(|(id, _)| *id).unwrap();
        let mut b = ask(&["breach"], Priority::Fewest);
        b.lock = vec![cassia];
        let locked = optimise(&b);
        assert!(locked.nodes.contains(&cassia));
        joined(&locked);
        let mut c = ask(&["breach"], Priority::Profit);
        c.budget = 40;
        assert_eq!(optimise(&c).points, 40);
    }

    /// How close it gets to real trees it hasn't learned from (.claude/atlas-research/data/eval.json, GROBBS's PC only):
    /// `cargo test -p filter-core --lib atlas_opt::tests::score -- --ignored --nocapture`. EVAL_SRC=creators (default) / players.
    #[test]
    #[ignore]
    fn score() {
        let f = concat!(env!("CARGO_MANIFEST_DIR"), "/../.claude/atlas-research/data/eval.json");
        #[derive(Deserialize)]
        struct Row { id: String, src: String, ask: Vec<String>, points: usize, nodes: Vec<u32>, all: HashMap<u32, f64>, mechs: HashMap<String, atlas_model::MechShare> }
        let rows: Vec<Row> = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        let players = std::env::var("EVAL_SRC").is_ok_and(|s| s == "players");
        let show = std::env::var("EVAL_SHOW").is_ok();
        let big = |id: &u32| matches!(graph().kind(*id), 1 | 2);
        let (mut sums, mut n) = ([0.0f64; 4], 0.0);
        let (mut miss, mut extra): (HashMap<u32, usize>, HashMap<u32, usize>) = Default::default();
        let start = std::time::Instant::now();
        for r in rows.iter().filter(|r| (r.src == "player") == players) {
            let m: Vec<String> = r.ask.iter().map(|x| match x.as_str() { "conquerors" => "bosses".into(), x => x.to_string() }).collect();
            let a = Ask { mechanics: m, budget: r.points, ..Default::default() };
            let sh = Shares { all: r.all.clone(), mechs: r.mechs.clone(), ..Default::default() };
            let p = optimise_with(&a, &sh);
            let (ours, theirs): (HashSet<u32>, HashSet<u32>) = (p.nodes.iter().copied().collect(), r.nodes.iter().copied().collect());
            let idf = |id: &u32| (1.0 / r.all.get(id).copied().unwrap_or(0.01).max(0.01)).ln() + 0.3;
            let w = |s: &HashSet<u32>, other: Option<&HashSet<u32>>| s.iter().filter(|i| big(i) && other.is_none_or(|o| o.contains(i))).map(idf).sum::<f64>();
            let wf1 = 2.0 * w(&ours, Some(&theirs)) / (w(&ours, None) + w(&theirs, None));
            let both = ours.intersection(&theirs).count() as f64;
            let jac = both / (ours.len() + theirs.len()) as f64 / (1.0 - both / (ours.len() + theirs.len()) as f64);
            let gen: Vec<u32> = r.nodes.iter().copied().filter(|i| big(i) && !r.ask.iter().any(|a| a == fam(*i))).collect();
            let own: Vec<u32> = r.nodes.iter().copied().filter(|i| big(i) && r.ask.iter().any(|a| a == fam(*i))).collect();
            let rec = |v: &[u32]| if v.is_empty() { 1.0 } else { v.iter().filter(|i| ours.contains(i)).count() as f64 / v.len() as f64 };
            for (k, x) in [wf1, jac, rec(&own), rec(&gen)].iter().enumerate() { sums[k] += x; }
            for i in theirs.difference(&ours).filter(|i| big(i)) { *miss.entry(*i).or_default() += 1; }
            for i in ours.difference(&theirs).filter(|i| big(i)) { *extra.entry(*i).or_default() += 1; }
            if show { println!("{:40} {:30} wf1 {wf1:.2} jac {jac:.2} {}", r.id, r.ask.join("+"), p.altar); }
            n += 1.0;
        }
        println!("{n} trees: wF1 {:.3}, same points {:.3}, own notables {:.3}, all-round notables {:.3} in {:?}", sums[0] / n, sums[1] / n, sums[2] / n, sums[3] / n, start.elapsed());
        for (what, m) in [("misses", miss), ("extra", extra)] {
            let mut v: Vec<_> = m.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            println!("{what}: {}", v.iter().take(22).map(|(i, c)| format!("{} {i} x{c}", graph().name(*i))).collect::<Vec<_>>().join("; "));
        }
    }
}

/// Dev look at a plan: `ATLAS_OPT=harvest,breach cargo test -p filter-core --lib atlas_opt::show -- --ignored --nocapture`
/// (ATLAS_GOAL=terror for a goal, ATLAS_HAS=mirage for what the map has).
#[cfg(test)]
#[test]
#[ignore]
fn show() {
    let list = |k: &str| -> Vec<String> { std::env::var(k).map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect()).unwrap_or_default() };
    for pr in [Priority::Fewest, Priority::Balanced, Priority::Profit] {
        let start = std::time::Instant::now();
        let p = optimise(&Ask { mechanics: list("ATLAS_OPT"), goals: list("ATLAS_GOAL"), map_has: list("ATLAS_HAS"), priority: pr, ..Default::default() });
        println!("{pr:?}: {} points, {} trees, altar {}, threads {:?}, {:?}, {:?}", p.points, p.trees_used, p.altar, p.threads, p.notes, start.elapsed());
        println!("  income {}", p.parts.iter().map(|x| format!("{} {:.0}%", x.label, x.share * 100.0)).collect::<Vec<_>>().join(", "));
        if let Ok(dir) = std::env::var("ATLAS_OPT_OUT") { let _ = std::fs::write(format!("{dir}/plan_{pr:?}.json"), serde_json::to_string(&p).unwrap()); }
        if pr != Priority::Profit { continue; }
        for w in &p.top { println!("  + {} ({:.1}%, {}) {}", w.name, w.gain * 100.0, w.sure, w.why); }
        for w in &p.weak { println!("  - {} ({:.1}%) {}", w.name, w.gain * 100.0, w.why); }
        println!("  left out {:?}\n  tips {:?}", p.left_out, p.tips);
    }
}
