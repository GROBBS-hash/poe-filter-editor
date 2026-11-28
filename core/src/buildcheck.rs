//! Build checker: goes through every stage of a Path of Building build and says what won't work in game.
//! Gem and item level / Str / Dex / Int requirements against what the stage's tree, gear and jewels give,
//! sockets, passive points for the level, mana left after auras, and Kinetic Fusillade's duration breakpoint.
//! Data: core/data/poe1_buildcheck.json (tools/build_check_data.mjs, from Path of Building's game data and the
//! official tree). Formulas follow Path of Building (CalcTools.getGemStatRequirement, CalcSetup mana, the
//! Kinetic Fusillade speed cap in Skills/act_int.lua). Written 27 Nov 2026 after checking a twink guide by hand.

use crate::pob::{Build, Gear, Stage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize, Default)]
struct GemData {
    #[serde(default)] s: u32, #[serde(default)] d: u32, #[serde(default)] i: u32, #[serde(default)] sup: bool,
    #[serde(default)] req: Vec<u32>,
    #[serde(default, rename = "resFlat")] res_flat: Vec<u32>,
    #[serde(default, rename = "resPct")] res_pct: Vec<u32>,
    #[serde(default)] cost: Vec<u32>,
    #[serde(default)] mm: Vec<i32>,
    #[serde(default, rename = "asMore")] as_more: Vec<f64>,
    #[serde(default, rename = "auraAs")] aura_as: Vec<f64>,
    #[serde(default, rename = "as")] attack_speed: Vec<f64>,
    #[serde(default, rename = "durMore")] dur_more: Vec<f64>,
    #[serde(default)] proj: u32,
    #[serde(default)] dur: f64,
    #[serde(default, rename = "projDelay")] proj_delay: f64,
}

#[derive(Deserialize, Default)]
struct NodeData {
    #[serde(default)] x: f64, #[serde(default)] y: f64,
    #[serde(default)] s: Vec<String>,
    #[serde(default)] m: HashMap<String, Vec<String>>,
    #[serde(default)] a: u8, #[serde(default)] c: u8, #[serde(default)] n: u8,
}

#[derive(Deserialize, Default)]
struct Data {
    gems: HashMap<String, GemData>,
    /// [level, str, dex, int, attacks per second, two-handed]
    bases: HashMap<String, [f64; 6]>,
    /// 0 = a plain passive with nothing the checker reads.
    nodes: HashMap<String, serde_json::Value>,
}

fn data() -> &'static (Data, HashMap<u32, NodeData>) {
    static D: OnceLock<(Data, HashMap<u32, NodeData>)> = OnceLock::new();
    D.get_or_init(|| {
        let d: Data = serde_json::from_str(include_str!("../data/poe1_buildcheck.json")).unwrap_or_default();
        let nodes = d.nodes.iter().filter_map(|(k, v)| Some((k.parse().ok()?, serde_json::from_value(v.clone()).unwrap_or_default()))).collect();
        (d, nodes)
    })
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    /// "bad" (won't work), "warn" (works later / works badly), "ok".
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Mana { pub total: i64, pub reserved: i64, pub free: i64, pub skill: String, pub cost: Option<i64> }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Speed { pub skill: String, pub fires_after: f64, pub attack_every: f64, pub spare_pct: i64 }

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StageCheck {
    pub title: String,
    pub level: u32,
    /// The title had no level in it: worked out from the tree's points.
    pub level_guessed: bool,
    pub str: i64, pub dex: i64, pub int: i64,
    pub points: u32,
    pub points_have: u32,
    /// Str, Dex, Int you're short of for the most demanding item or gem (0 when enough).
    pub short: [i64; 3],
    pub mana: Option<Mana>,
    pub speed: Option<Speed>,
    pub notes: Vec<Note>,
}

fn class_attrs(class: &str) -> (i64, i64, i64) {
    match class {
        "Marauder" => (32, 14, 14), "Ranger" => (14, 32, 14), "Witch" => (14, 14, 32), "Duelist" => (23, 23, 14),
        "Templar" => (23, 14, 23), "Shadow" => (14, 23, 23), _ => (20, 20, 20),
    }
}

/// Passive points from quests by the end of each act (kill-all bandits), roughly.
fn quest_points(level: u32) -> u32 {
    const BY_ACT: [u32; 11] = [0, 2, 5, 7, 8, 9, 12, 14, 16, 17, 22];
    BY_ACT[(crate::pob::act_of(level) as usize).min(10)]
}

/// The character level in a stage title: "Level 25", "Lvl 12-25", "lv35", "Act 3 (lvl 30)".
pub fn title_level(t: &str) -> Option<u32> {
    let l = t.to_lowercase();
    for key in ["level", "lvl", "lv"] {
        if let Some(i) = l.find(key) {
            let rest = l[i + key.len()..].trim_start_matches([' ', '.', ':']);
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(v) = n.parse::<u32>() { if (1..=100).contains(&v) { return Some(v); } }
        }
    }
    None
}

/// "+5 to Intelligence" style numbers from a line, signed by increased/reduced.
fn num(l: &str) -> f64 {
    let start = l.find(|c: char| c.is_ascii_digit()).unwrap_or(l.len());
    let s: String = l[start..].chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let v = s.parse::<f64>().unwrap_or(0.0);
    if l[..start].ends_with('-') || l.contains(" reduced ") || l.contains(" less ") { -v } else { v }
}

#[derive(Default, Clone, Copy)]
struct Attr { s: f64, d: f64, i: f64, s_pct: f64, d_pct: f64, i_pct: f64 }

fn add_attr(a: &mut Attr, l: &str) {
    let v = num(l);
    if l.contains("% increased") || l.contains("% reduced") {
        if l.ends_with("increased Attributes") || l.ends_with("reduced Attributes") { a.s_pct += v; a.d_pct += v; a.i_pct += v; }
        else if l.ends_with(" Strength") { a.s_pct += v } else if l.ends_with(" Dexterity") { a.d_pct += v } else if l.ends_with(" Intelligence") { a.i_pct += v }
        return;
    }
    if !l.starts_with('+') && !l.starts_with('-') { return; }
    let what = l.split(" to ").nth(1).unwrap_or("");
    match what {
        "all Attributes" => { a.s += v; a.d += v; a.i += v }
        "Strength" => a.s += v, "Dexterity" => a.d += v, "Intelligence" => a.i += v,
        "Strength and Dexterity" => { a.s += v; a.d += v }
        "Strength and Intelligence" => { a.s += v; a.i += v }
        "Dexterity and Intelligence" => { a.d += v; a.i += v }
        _ => {}
    }
}

fn radius_of(lines: &[String]) -> f64 {
    match lines.iter().find_map(|l| l.strip_prefix("Radius: ")).unwrap_or("") {
        "Small" => 960.0, "Medium" => 1440.0, "Large" => 1800.0, "Very Large" => 2400.0, "Massive" => 2880.0, _ => 0.0,
    }
}

fn base_of<'a>(g: &'a Gear) -> Option<(&'a str, [f64; 6])> {
    let bases = &data().0.bases;
    if let Some(b) = bases.get(g.base.as_str()) { return Some((g.base.as_str(), *b)); }
    // Magic items: "Constant Quicksilver Flask of the Hare" holds the base.
    bases.iter().filter(|(k, _)| g.base.contains(k.as_str())).max_by_key(|(k, _)| k.len()).map(|(_, v)| (g.base.as_str(), *v))
}

/// The character level an item needs to wear (its base, or a unique's own); 0 when unknown.
pub fn wear_level(base: &str, rarity: &str, name: &str) -> u32 {
    let bases = &data().0.bases;
    let b = bases.get(base).or_else(|| bases.iter().filter(|(k, _)| base.contains(k.as_str())).max_by_key(|(k, _)| k.len()).map(|x| x.1));
    (b.map(|b| b[0] as u32).unwrap_or(0)).max(if rarity.eq_ignore_ascii_case("unique") { unique_level(name) } else { 0 })
}

fn unique_level(name: &str) -> u32 {
    let n = name.strip_prefix("Foulborn ").unwrap_or(name);
    crate::gear::uniques().iter().find(|u| u.name == n).map(|u| u.level).unwrap_or(0)
}

/// Requirement for a gem at a character-level requirement (PoB's CalcTools.getGemStatRequirement).
fn gem_stat(level_req: u32, support: bool, multi: u32) -> i64 {
    if multi == 0 { return 0; }
    let r = ((20.0 + (level_req as f64 - 3.0) * 3.0) * (multi as f64 / 100.0).powf(0.9) * if support { 0.5 } else { 0.7 }).round() as i64;
    if r < 14 { 0 } else { r }
}

fn at<T: Copy + Default>(v: &[T], level: u32) -> T { v.get((level.clamp(1, 21) - 1) as usize).or(v.last()).copied().unwrap_or_default() }

fn gem_data(name: &str) -> Option<&'static GemData> {
    let g = &data().0.gems;
    g.get(name).or_else(|| g.get(name.trim_end_matches(" Support")))
}

fn sockets(g: &Gear) -> usize {
    g.lines.iter().find_map(|l| l.strip_prefix("Sockets: ")).map(|s| s.chars().filter(|c| "RGBW".contains(*c)).count()).unwrap_or(0)
}

/// Lines that count for this stage: tree nodes and masteries, gear (not weapons' local lines), jewels.
struct Pool { tree: Vec<String>, gear: Vec<String> }

fn stage_lines(st: &Stage, alloc: &[u32], active: &[String]) -> Pool {
    let nodes = &data().1;
    let mut tree: Vec<String> = vec![];
    // Unnatural Instinct and friends change which small passives count.
    let mut skip: Vec<u32> = vec![];
    let mut extra: Vec<u32> = vec![];
    for j in &st.jewels {
        let Some(sock) = j.node.filter(|n| alloc.contains(n)) else { continue };
        let r = radius_of(&j.item.lines);
        let Some(sn) = nodes.get(&sock) else { continue };
        let in_r = |id: &u32| nodes.get(id).is_some_and(|n| n.n == 0 && n.a == 0 && n.c == 0 && ((n.x - sn.x).powi(2) + (n.y - sn.y).powi(2)).sqrt() <= r);
        if j.item.lines.iter().any(|l| l == "Allocated Small Passive Skills in Radius grant nothing") { skip.extend(alloc.iter().filter(|i| in_r(i))); }
        if j.item.lines.iter().any(|l| l == "Grants all bonuses of Unallocated Small Passive Skills in Radius") { extra.extend(nodes.keys().filter(|i| !alloc.contains(i) && in_r(i))); }
    }
    for id in alloc.iter().filter(|i| !skip.contains(i)).chain(extra.iter()) {
        if let Some(n) = nodes.get(id) { tree.extend(n.s.iter().cloned()); }
    }
    if let Some(t) = &st.tree {
        for (node, eff) in &t.masteries {
            if let Some(s) = nodes.get(node).and_then(|n| n.m.get(&eff.to_string())) { tree.extend(s.iter().cloned()); }
        }
    }
    let mut gear: Vec<String> = vec![];
    let keep = |l: &String| -> Option<String> {
        // Watcher's Eye: "… while affected by Clarity" only counts with that aura up.
        match l.split_once(" while affected by ") { Some((a, aura)) => active.iter().any(|g| g == aura).then(|| a.to_string()), None => Some(l.clone()) }
    };
    for g in st.gear.iter().filter(|g| !g.slot.starts_with("Flask")) {
        // A weapon's attack speed is its own (local), not everyone's.
        let weapon = g.slot.starts_with("Weapon");
        gear.extend(g.lines.iter().filter(|l| !(weapon && (l.ends_with("% increased Attack Speed") || l.ends_with("% reduced Attack Speed")))).filter_map(keep));
    }
    for j in &st.jewels { if j.node.is_none_or(|n| alloc.contains(&n)) { gear.extend(j.item.lines.iter().filter_map(keep)); } }
    Pool { tree, gear }
}

pub fn check(b: &Build) -> Vec<StageCheck> {
    // A levelling guide's tree is what you should have by the time the next stage starts.
    b.stages.iter().enumerate().map(|(i, s)| check_stage(b, s, b.stages.get(i + 1).and_then(|n| title_level(&n.title)))).collect()
}

fn check_stage(b: &Build, st: &Stage, next_level: Option<u32>) -> StageCheck {
    // A gear set with no tree, or a Mercenary's stage: it isn't worn by you, so your level and attributes don't apply.
    if st.tree.is_none() && (st.groups.is_empty() || st.title.to_lowercase().contains("mercenar")) {
        let note = Note { kind: "ok".into(), text: "Gear for your Mercenary or a minion, not you: nothing to check against your level.".into() };
        return StageCheck { title: st.title.clone(), level: b.level.max(1), level_guessed: true, str: 0, dex: 0, int: 0, short: [0; 3],
            points: 0, points_have: 0, mana: None, speed: None, notes: vec![note] };
    }
    let nodes = &data().1;
    let mut notes: Vec<Note> = vec![];
    let mut note = |kind: &str, text: String| notes.push(Note { kind: kind.into(), text });
    let alloc: Vec<u32> = st.tree.as_ref().map(|t| t.nodes.clone()).unwrap_or_default();
    let points = alloc.iter().filter(|i| nodes.get(i).is_none_or(|n| n.a == 0 && n.c == 0)).count() as u32;
    let (level, level_guessed) = match title_level(&st.title) {
        Some(l) => (l, false),
        None if st.tree.is_some() => ((1..=100).find(|l| l - 1 + quest_points(*l) >= points).unwrap_or(100), true),
        None => (b.level.max(1), true),
    };
    let points_have = level - 1 + quest_points(level);
    let end = next_level.filter(|n| *n > level).unwrap_or(level);
    let have_end = end - 1 + quest_points(end);
    if points > have_end + 1 {
        note("warn", format!("The tree uses {points} points; by level {end} you'd have about {have_end}."));
    }

    // Auras and buffs switched on (for Watcher's Eye lines and aura speed).
    let gems_on: Vec<(&crate::pob::Group, &crate::pob::Gem)> = st.groups.iter().filter(|g| g.enabled)
        .flat_map(|g| g.gems.iter().filter(|x| x.enabled).map(move |x| (g, x))).collect();
    let active: Vec<String> = gems_on.iter().map(|(_, g)| g.name.clone()).collect();
    let pool = stage_lines(st, &alloc, &active);

    // ---- attributes
    let (bs, bd, bi) = class_attrs(&b.class);
    let mut a = Attr { s: bs as f64, d: bd as f64, i: bi as f64, ..Default::default() };
    for l in pool.tree.iter().chain(pool.gear.iter()) { add_attr(&mut a, l); }
    // "Dexterity from Passives in Radius is Transformed to Intelligence" (Fertile Mind and friends).
    for j in &st.jewels {
        let Some(sock) = j.node.filter(|n| alloc.contains(n)) else { continue };
        let Some(sn) = nodes.get(&sock) else { continue };
        let r = radius_of(&j.item.lines);
        for l in &j.item.lines {
            let Some((from, to)) = l.split_once(" from Passives in Radius is Transformed to ") else { continue };
            let mut moved = Attr::default();
            for id in alloc.iter().filter(|i| **i != sock) {
                let Some(n) = nodes.get(id) else { continue };
                if ((n.x - sn.x).powi(2) + (n.y - sn.y).powi(2)).sqrt() > r { continue; }
                for s in &n.s { add_attr(&mut moved, s); }
            }
            let v = match from { "Strength" => moved.s, "Dexterity" => moved.d, "Intelligence" => moved.i, _ => 0.0 };
            match from { "Strength" => a.s -= v, "Dexterity" => a.d -= v, "Intelligence" => a.i -= v, _ => {} }
            match to { "Strength" => a.s += v, "Dexterity" => a.d += v, "Intelligence" => a.i += v, _ => {} }
        }
    }
    let total = |a: &Attr| ((a.s * (1.0 + a.s_pct / 100.0)).floor() as i64, (a.d * (1.0 + a.d_pct / 100.0)).floor() as i64, (a.i * (1.0 + a.i_pct / 100.0)).floor() as i64);
    let (ts, td, ti) = total(&a);

    // ---- gear
    let mut later: Vec<(u32, String)> = vec![];
    // How much Str / Dex / Int you're short of at worst (the upgrade finder asks for it on gear).
    let mut short_by = [0i64; 3];
    for g in &st.gear {
        let Some((_, base)) = base_of(g) else { continue };
        let lvl = (base[0] as u32).max(if g.rarity == "unique" { unique_level(&g.name) } else { 0 });
        let short_name = g.name.split(" of ").next().unwrap_or(&g.name).trim_start_matches("Constant ").to_string();
        if lvl > level && !later.iter().any(|(_, n)| *n == short_name) { later.push((lvl, short_name)); }
        if g.lines.iter().any(|l| l == "Has no Attribute Requirements") { continue; }
        // An item's own attributes don't count towards its own requirement.
        let mut own = Attr::default();
        for l in &g.lines { add_attr(&mut own, l); }
        let rows = [("Str", base[1], ts - own.s as i64), ("Dex", base[2], td - own.d as i64), ("Int", base[3], ti - own.i as i64)];
        for (i, (_, need, have)) in rows.iter().enumerate() { short_by[i] = short_by[i].max(*need as i64 - *have); }
        let short: Vec<String> = rows
            .iter().filter(|(_, need, have)| *need as i64 > *have).map(|(k, need, have)| format!("{need} {k} (you'd have {have})")).collect();
        if !short.is_empty() { note("bad", format!("{} needs {}.", g.name, short.join(" and "))); }
    }

    if !later.is_empty() {
        later.sort();
        note("warn", format!("Not wearable at level {level} yet: {}.", later.iter().map(|(l, n)| format!("{n} at {l}")).collect::<Vec<_>>().join(", ")));
    }

    // ---- gems
    let mut per_slot: HashMap<&str, usize> = HashMap::new();
    for (grp, g) in &gems_on {
        if !grp.slot.is_empty() { *per_slot.entry(grp.slot.as_str()).or_default() += 1; }
        let Some(d) = gem_data(&g.name) else { continue };
        let req = at(&d.req, g.level);
        if req > level { note("warn", format!("{} level {} needs character level {req} (this stage starts at {level}).", g.name, g.level)); }
        let needs = [gem_stat(req, d.sup, d.s), gem_stat(req, d.sup, d.d), gem_stat(req, d.sup, d.i)];
        for (i, have) in [ts, td, ti].iter().enumerate() { short_by[i] = short_by[i].max(needs[i] - have); }
        let short: Vec<String> = [("Str", d.s, ts), ("Dex", d.d, td), ("Int", d.i, ti)].iter()
            .map(|(k, m, have)| (k, gem_stat(req, d.sup, *m), have)).filter(|(_, need, have)| need > *have)
            .map(|(k, need, have)| format!("{need} {k} (you'd have {have})")).collect();
        if !short.is_empty() { note("bad", format!("{} level {} needs {}.", g.name, g.level, short.join(" and "))); }
    }
    for (slot, n) in &per_slot {
        if let Some(it) = st.gear.iter().find(|g| g.slot == *slot) {
            let have = sockets(it);
            if have > 0 && *n > have { note("bad", format!("{} gems in {slot} ({}), which has {have} sockets.", n, it.name)); }
        }
    }

    // ---- mana
    let sum = |lines: &[String], f: &dyn Fn(&str) -> bool| lines.iter().filter(|l| f(l)).map(|l| num(l)).sum::<f64>();
    let all: Vec<String> = pool.tree.iter().chain(pool.gear.iter()).cloned().collect();
    let flat_mana = sum(&all, &|l| l.starts_with('+') && l.ends_with(" to maximum Mana"));
    let inc_mana = sum(&all, &|l| l.ends_with("% increased maximum Mana") || l.ends_with("% reduced maximum Mana"));
    let mana = ((34.0 + 6.0 * level as f64 + (ti / 2) as f64 + flat_mana) * (1.0 + inc_mana / 100.0)).floor();
    let eff = sum(&all, &|l| l.contains("Reservation Efficiency of Skills") && !l.contains("Life Reservation"));
    let mut reserved = 0.0;
    for (_, g) in &gems_on {
        let Some(d) = gem_data(&g.name) else { continue };
        if !d.res_pct.is_empty() { reserved += (mana * at(&d.res_pct, g.level) as f64 / 100.0 / (1.0 + eff / 100.0)).ceil(); }
        else if !d.res_flat.is_empty() { reserved += (at(&d.res_flat, g.level) as f64 / (1.0 + eff / 100.0)).ceil(); }
    }
    // The main skill: the active gem with the most supports (ties: the body armour's).
    let main = st.groups.iter().filter(|g| g.enabled)
        .filter_map(|g| g.gems.iter().find(|x| x.enabled && !x.support && gem_data(&x.name).is_some_and(|d| !d.cost.is_empty() && d.res_flat.is_empty() && d.res_pct.is_empty())).map(|m| (g, m)))
        .max_by_key(|(g, _)| (g.gems.len(), g.slot == "Body Armour"));
    let mut mana_out = None;
    if reserved > 0.0 || main.is_some() {
        let mut cost = None;
        if let Some((grp, m)) = main {
            let d = gem_data(&m.name).unwrap();
            let mut c = at(&d.cost, m.level) as f64;
            for s in grp.gems.iter().filter(|x| x.enabled && x.support) { if let Some(sd) = gem_data(&s.name) { c *= 1.0 + at(&sd.mm, s.level) as f64 / 100.0; } }
            let red = sum(&all, &|l| l.ends_with("% reduced Mana Cost of Skills") || l.ends_with("% increased Mana Cost of Skills"));
            let ce = sum(&all, &|l| l.ends_with("% increased Mana Cost Efficiency") || l.ends_with("% reduced Mana Cost Efficiency"));
            let flat_off = sum(&all, &|l| l.contains("to Total Mana Cost") && l.starts_with("Non-Channelling Skills have -"));
            cost = Some(((c * (1.0 + red / 100.0) / (1.0 + ce / 100.0)).floor() + flat_off).max(0.0) as i64);
        }
        let free = (mana - reserved) as i64;
        if free < 0 { note("bad", format!("Your auras reserve {} mana but you only have {}: something won't switch on.", reserved as i64, mana as i64)); }
        else if let Some(c) = cost.filter(|c| *c > 0 && free < *c * 2) {
            note("warn", format!("Only {free} mana left after auras, and {} costs {c}: you'll run dry fast. Drop an aura or lower its level.", main.map(|m| m.1.name.as_str()).unwrap_or("your skill")));
        }
        mana_out = Some(Mana { total: mana as i64, reserved: reserved as i64, free, skill: main.map(|m| m.1.name.clone()).unwrap_or_default(), cost });
    }

    // ---- Kinetic Fusillade style breakpoint: duration has to be shorter than the time between attacks.
    let mut speed = None;
    if let Some((grp, m)) = main {
        let d = gem_data(&m.name).unwrap();
        if d.proj_delay > 0.0 && d.dur > 0.0 {
            let weapon = |slot: &str| st.gear.iter().find(|g| g.slot == slot).and_then(|g| base_of(g).map(|(_, b)| (g, b))).filter(|(_, b)| b[4] > 0.0);
            if let Some((w1, b1)) = weapon("Weapon 1") {
                let local = |g: &Gear| g.lines.iter().filter(|l| l.ends_with("% increased Attack Speed") || l.ends_with("% reduced Attack Speed")).map(|l| num(l)).sum::<f64>();
                let w2 = weapon("Weapon 2");
                let dual = w2.is_some();
                let rate1 = b1[4] * (1.0 + local(w1) / 100.0);
                let rate = match w2 { Some((g2, b2)) => (rate1 + b2[4] * (1.0 + local(g2) / 100.0)) / 2.0, None => rate1 };
                let mut inc = sum(&all, &|l| l.ends_with("% increased Attack Speed") || l.ends_with("% reduced Attack Speed"));
                if dual { inc += sum(&all, &|l| l.ends_with("% increased Attack Speed while Dual Wielding")); }
                for (_, g) in &gems_on { if let Some(gd) = gem_data(&g.name) { inc += at(&gd.aura_as, g.level); } }
                for s in grp.gems.iter().filter(|x| x.enabled && x.support) { if let Some(sd) = gem_data(&s.name) { inc += at(&sd.attack_speed, s.level); } }
                let aps = rate * (1.0 + at(&d.as_more, m.level) / 100.0) * if dual { 1.1 } else { 1.0 } * (1.0 + inc / 100.0);
                let proj = 1 + grp.gems.iter().filter(|x| x.enabled && x.support).filter_map(|s| gem_data(&s.name)).map(|s| s.proj).sum::<u32>();
                let dur_inc = sum(&all, &|l| l.ends_with("% increased Skill Effect Duration") || l.ends_with("% reduced Skill Effect Duration"));
                let mut more = 1.0;
                for l in all.iter().filter(|l| l.ends_with("% less Skill Effect Duration") || l.ends_with("% more Skill Effect Duration")) { more *= 1.0 + num(l) / 100.0; }
                for s in grp.gems.iter().filter(|x| x.enabled && x.support) { if let Some(sd) = gem_data(&s.name) { more *= 1.0 + at(&sd.dur_more, s.level) / 100.0; } }
                let delay = (d.dur + d.proj_delay * (proj - 1) as f64) * (1.0 + dur_inc / 100.0).max(0.0) * more;
                const TICK: f64 = 0.033;
                let fires = ((delay / TICK) - 1e-9).ceil() * TICK;
                let max_aps = 1.0 / fires;
                let spare = (((max_aps / aps) * (1.0 + inc / 100.0) - 1.0) * 100.0 - inc).floor() as i64;
                if aps > max_aps {
                    note("bad", format!("{} fires after {:.2} s but you attack every {:.2} s: attacking that fast resets it and wastes damage. Lose about {}% attack speed or get more reduced duration.", m.name, fires, 1.0 / aps, -spare));
                }
                speed = Some(Speed { skill: m.name.clone(), fires_after: (fires * 1000.0).round() / 1000.0, attack_every: ((1.0 / aps) * 1000.0).round() / 1000.0, spare_pct: spare });
            }
        }
    }
    // Two of the same item (both Poet's Pens) say the same thing once.
    let mut seen = std::collections::HashSet::new();
    notes.retain(|n| seen.insert(n.text.clone()));
    if notes.is_empty() { notes.push(Note { kind: "ok".into(), text: "Everything in this stage can be worn and used at this level.".into() }); }
    StageCheck { title: st.title.clone(), level, level_guessed, str: ts, dex: td, int: ti, short: short_by, points, points_have, mana: mana_out, speed, notes }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn build(code: &str) -> Build { crate::pob::parse(&crate::pob::decode(code).unwrap()).unwrap() }

    #[test]
    fn wear_levels_for_buy() {
        // His level 68 stage's helmet (the trade site found none wearable at 80, all of them at 90).
        let l = wear_level("Torturer's Mask", "Rare", "");
        assert!((81..=90).contains(&l), "{l}");
        assert!(wear_level("Vaal Gauntlets", "Rare", "") > 0);
        assert_eq!(wear_level("Not A Base", "Rare", ""), 0);
        assert_eq!(title_level("Swap at Merciless Lab (level 68-90)"), Some(68));
    }

    #[test]
    fn mercenary_gear_is_not_checked_against_you() {
        let b = crate::pob::parse(r#"<PathOfBuilding><Build className="Scion" ascendClassName="Luminary" level="40"/>
<Items><Item id="1">Rarity: UNIQUE
Paradoxica
Vaal Rapier</Item><ItemSet id="1" title="Mercenary: budget"><Slot name="Weapon 1" itemId="1"/></ItemSet></Items></PathOfBuilding>"#).unwrap();
        let c = check(&b);
        assert_eq!(c.len(), 1);
        assert!(c[0].notes.iter().all(|n| n.kind == "ok") && c[0].short == [0; 3], "{:?}", c[0].notes);
    }

    #[test]
    fn old_kf_guide_catches_the_int_and_mana_problems() {
        let c = check(&build(include_str!("../data/test_pob_kf_old.txt")));
        let lv12 = c.iter().find(|s| s.title == "Level 12").unwrap();
        assert_eq!(lv12.int, 43, "Scion 20 + Doedre's 23, no Int passive");
        assert!(lv12.notes.iter().any(|n| n.kind == "bad" && n.text.starts_with("The Poet's Pen needs 47 Int")), "{:?}", lv12.notes);
        assert!(lv12.notes.iter().any(|n| n.kind == "bad" && n.text.starts_with("Doedre's Tenure needs 21 Int")), "{:?}", lv12.notes);
        let lv25 = c.iter().find(|s| s.title == "Level 25").unwrap();
        assert!(lv25.notes.iter().any(|n| n.text.contains("mana")), "Haste + Spellslinger + Clarity: {:?} {:?}", lv25.notes, lv25.mana);
        let lv35 = c.iter().find(|s| s.title == "Level 35").unwrap();
        assert!(lv35.notes.iter().any(|n| n.text.starts_with("Volley level 10 needs character level 36")), "{:?}", lv35.notes);
    }

    #[test]
    fn fixed_kf_guide_passes() {
        let c = check(&build(include_str!("../data/test_pob_kf_fixed.txt")));
        assert_eq!(c.len(), 6);
        for s in &c {
            let bad: Vec<_> = s.notes.iter().filter(|n| n.kind == "bad").collect();
            assert!(bad.is_empty(), "{}: {:?}", s.title, bad);
        }
        let lv12 = c.iter().find(|s| s.title == "Level 12").unwrap();
        assert_eq!(lv12.int, 48);
        let lv35 = c.iter().find(|s| s.title == "Level 35").unwrap();
        assert!(lv35.int >= 116, "Eclipse Solaris: {}", lv35.int);
        let lv25 = c.iter().find(|s| s.title == "Level 25").unwrap();
        let sp = lv25.speed.as_ref().expect("speed check");
        assert!((sp.fires_after - 0.231).abs() < 0.001 && sp.fires_after < sp.attack_every, "{sp:?}");
        let m = lv25.mana.as_ref().unwrap();
        assert!(m.free > 0, "{m:?}");
    }

    #[test]
    fn levels_from_titles() {
        assert_eq!(title_level("Level 25"), Some(25));
        assert_eq!(title_level("Lvl 12-25 Poet's Pen"), Some(12));
        assert_eq!(title_level("Level 70 (maps)"), Some(70));
        assert_eq!(title_level("Endgame"), None);
    }
}
