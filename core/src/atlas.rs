//! Atlas strategies: atlas passive trees (plus notes) imported from a link, saved
//! under a name, and shown over the game.
//!
//! Links understood:
//! - maxroll's farming strategy planner (`maxroll.gg/poe/farming-strategy-planner/<id>`): JSON from
//!   `planners.maxroll.gg/profiles/poe/<id>`, with variants, steps and rich-text notes.
//! - pathofexile.com's atlas tree (`…/atlas-skill-tree/<version>/<code>`): the tree only.
//!
//! The tree drawing data is `public/atlas_poe1.json` (built by tools/build_atlas_data.mjs).

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::OnceLock;

const TREE_JSON: &str = include_str!("../../public/atlas_poe1.json");

/// The atlas tree as a graph, for filling in paths and naming nodes.
pub struct Graph {
    pub version: String,
    pub start: u32,
    adj: HashMap<u32, Vec<u32>>,
    /// node -> (kind, name): kind 0 small, 1 notable, 2 keystone, 4 gateway, 5 start.
    info: HashMap<u32, (u8, String)>,
}

pub fn graph() -> &'static Graph {
    static G: OnceLock<Graph> = OnceLock::new();
    G.get_or_init(|| {
        let v: Value = serde_json::from_str(TREE_JSON).expect("atlas tree data");
        let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut info = HashMap::new();
        for (id, n) in v["nodes"].as_object().into_iter().flatten() {
            let Ok(id) = id.parse::<u32>() else { continue };
            info.insert(id, (n[2].as_u64().unwrap_or(0) as u8, n[3].as_str().unwrap_or("").to_string()));
            adj.entry(id).or_default();
        }
        for e in v["edges"].as_array().into_iter().flatten() {
            let (Some(a), Some(b)) = (e[0].as_u64(), e[1].as_u64()) else { continue };
            let (a, b) = (a as u32, b as u32);
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
        Graph { version: v["version"].as_str().unwrap_or("").to_string(), start: v["start"].as_u64().unwrap_or(0) as u32, adj, info }
    })
}

impl Graph {
    pub fn has(&self, id: u32) -> bool { self.info.contains_key(&id) }
    pub fn kind(&self, id: u32) -> u8 { self.info.get(&id).map(|i| i.0).unwrap_or(0) }
    pub fn name(&self, id: u32) -> &str { self.info.get(&id).map(|i| i.1.as_str()).unwrap_or("") }

    /// Every node the targets need: the targets plus the shortest paths joining them to the
    /// start (never through `exclude`). Like maxroll's auto-pathing, where a strategy only
    /// stores the nodes picked by hand. Already-joined trees come back unchanged.
    /// The start node is included (points spent = len - 1).
    pub fn fill_paths(&self, targets: &[u32], exclude: &HashSet<u32>) -> Vec<u32> {
        let wanted: Vec<u32> = targets.iter().copied().filter(|t| self.has(*t)).collect();
        let want_set: HashSet<u32> = wanted.iter().copied().collect();
        let mut have: HashSet<u32> = HashSet::from([self.start]);
        let mut order = vec![self.start];
        // Takes in every wanted node already touching what we have.
        let absorb = |have: &mut HashSet<u32>, order: &mut Vec<u32>, from: Vec<u32>| {
            let mut queue: VecDeque<u32> = from.into();
            while let Some(n) = queue.pop_front() {
                for &m in self.adj.get(&n).into_iter().flatten() {
                    if want_set.contains(&m) && have.insert(m) { order.push(m); queue.push_back(m); }
                }
            }
        };
        absorb(&mut have, &mut order, vec![self.start]);
        for &t in &wanted {
            if have.contains(&t) { continue; }
            // Breadth-first from everything we have to t.
            let mut prev: HashMap<u32, u32> = HashMap::new();
            let mut queue: VecDeque<u32> = have.iter().copied().collect();
            let mut seen: HashSet<u32> = have.clone();
            let mut found = false;
            while let Some(n) = queue.pop_front() {
                if n == t { found = true; break; }
                for &m in self.adj.get(&n).into_iter().flatten() {
                    if (exclude.contains(&m) && m != t) || !seen.insert(m) { continue; }
                    prev.insert(m, n);
                    queue.push_back(m);
                }
            }
            if !found { continue; }
            let mut path = vec![];
            let mut cur = t;
            while !have.contains(&cur) { path.push(cur); cur = prev[&cur]; }
            path.reverse();
            for &p in &path { have.insert(p); order.push(p); }
            absorb(&mut have, &mut order, path);
        }
        order
    }
}

// ---------- saved strategies ----------

/// One heading's worth of notes (e.g. "How to play").
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Notes {
    pub title: String,
    pub lines: Vec<NoteLine>,
}

/// A line of notes: "h" heading, "p" paragraph, "li" list item (depth 0, 1…).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NoteLine {
    pub kind: String,
    #[serde(default)]
    pub depth: u8,
    pub text: String,
}

/// The map device setup a strategy suggests.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Device {
    /// Map to run (e.g. "City Square Map").
    pub maps: Vec<String>,
    /// Threads in the device, e.g. "Ritual ×8".
    pub threads: Vec<String>,
    /// Map rolls to look for, e.g. "30% increased Pack size".
    pub rolls: Vec<String>,
    /// The site's one-line tip.
    pub text: String,
}

/// One atlas tree of a strategy (maxroll: a variant's step).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    pub name: String,
    /// Allocated nodes, start node included.
    pub nodes: Vec<u32>,
    /// Multiple-choice notables: (node, effect picked).
    pub choices: Vec<(u32, u32)>,
    /// Notes for this variant (setup, atlas tips, loot).
    pub notes: Vec<Notes>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Strategy {
    pub id: u64,
    pub name: String,
    /// "maxroll" or "official".
    pub site: String,
    pub link: String,
    pub author: String,
    pub imported: u64,
    pub device: Option<Device>,
    /// Notes for the whole strategy (intro, gameplay, summary).
    pub notes: Vec<Notes>,
    pub variants: Vec<Variant>,
    /// Your own notes, typed on the Atlas page.
    pub own_notes: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AtlasStore {
    pub list: Vec<Strategy>,
    /// The strategy shown in game, and which of its trees.
    pub active: Option<u64>,
    pub variant: usize,
}

impl AtlasStore {
    pub fn load(path: &Path) -> AtlasStore {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    /// Adds a strategy (name = yours if given, else the site's) and makes it the one shown in game.
    pub fn add(&mut self, mut s: Strategy, name: &str) -> u64 {
        let id = self.list.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        s.id = id;
        if !name.trim().is_empty() { s.name = name.trim().to_string(); }
        if s.name.trim().is_empty() { s.name = format!("Atlas strategy {id}"); }
        self.list.push(s);
        self.active = Some(id);
        self.variant = 0;
        id
    }

    pub fn rename(&mut self, id: u64, name: &str) -> Result<(), String> {
        if name.trim().is_empty() { return Err("Give it a name".into()); }
        let s = self.list.iter_mut().find(|s| s.id == id).ok_or("That strategy is gone")?;
        s.name = name.trim().to_string();
        Ok(())
    }

    pub fn set_own_notes(&mut self, id: u64, text: &str) {
        if let Some(s) = self.list.iter_mut().find(|s| s.id == id) { s.own_notes = text.to_string(); }
    }

    pub fn remove(&mut self, id: u64) {
        self.list.retain(|s| s.id != id);
        if self.active == Some(id) { self.active = self.list.first().map(|s| s.id); self.variant = 0; }
    }

    pub fn use_one(&mut self, id: u64, variant: usize) {
        if let Some(s) = self.list.iter().find(|s| s.id == id) {
            self.active = Some(id);
            self.variant = variant.min(s.variants.len().saturating_sub(1));
        }
    }

    pub fn current(&self) -> Option<&Strategy> {
        self.active.and_then(|id| self.list.iter().find(|s| s.id == id)).or(self.list.first())
    }
}

// ---------- links ----------

/// What a pasted link points at.
#[derive(Debug, Clone, PartialEq)]
pub enum Link {
    /// maxroll planner id.
    Maxroll(String),
    /// pathofexile.com atlas tree code (base64).
    Official(String),
}

/// Works out what a pasted link is. A bare maxroll id ("vg1ga30g") also works.
pub fn parse_link(text: &str) -> Result<Link, String> {
    let t = text.trim();
    if t.is_empty() { return Err("Paste a link first".into()); }
    let clean = t.split(['?', '#']).next().unwrap_or(t).trim_end_matches('/');
    let last = clean.rsplit('/').next().unwrap_or(clean).to_string();
    let lower = clean.to_lowercase();
    if lower.contains("maxroll.gg") {
        if last.is_empty() || last.contains('.') { return Err("That maxroll link doesn't name a strategy".into()); }
        return Ok(Link::Maxroll(last));
    }
    if lower.contains("pathofexile.com") {
        if !lower.contains("atlas") { return Err("That's a pathofexile.com link, but not to the atlas tree".into()); }
        return Ok(Link::Official(last));
    }
    if lower.contains("poe.ninja") || lower.contains("poeplanner") {
        return Err("poe.ninja / poeplanner links aren't supported yet: use their \"Export to pathofexile.com\" link, or maxroll".into());
    }
    if !t.contains('/') && !t.contains(' ') && (4..=16).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(Link::Maxroll(t.to_string()));
    }
    if !t.contains('/') && t.len() > 16 && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_=+/".contains(c)) {
        return Ok(Link::Official(t.to_string()));
    }
    Err("Paste a maxroll farming strategy link or a pathofexile.com atlas tree link".into())
}

pub fn maxroll_api_url(id: &str) -> String { format!("https://planners.maxroll.gg/profiles/poe/{id}") }

/// The pathofexile.com atlas tree code (as in the site's links): allocated nodes and picks.
/// Version 6: u32 version, u8 class, u8 ascendancy, u8 count + u16 nodes,
/// u8 count + u16 cluster nodes, u8 count + u32 (effect << 16 | node).
pub fn decode_official(code: &str) -> Result<(Vec<u32>, Vec<(u32, u32)>), String> {
    let code = code.trim().trim_end_matches('=');
    let cfg = base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let url = base64::engine::GeneralPurpose::new(&base64::alphabet::URL_SAFE, cfg);
    let std = base64::engine::GeneralPurpose::new(&base64::alphabet::STANDARD, cfg);
    let bytes = url.decode(code).or_else(|_| std.decode(code)).map_err(|_| "That atlas tree link looks broken (can't read its code)".to_string())?;
    if bytes.len() < 6 { return Err("That atlas tree link is too short".into()); }
    let version = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let u16_at = |i: usize| -> Option<u32> { Some(u16::from_be_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]) as u32) };
    let mut nodes = vec![];
    let mut choices = vec![];
    if version <= 4 {
        // u32 version, u8 class, u8 ascendancy, u8 fullscreen, then u16 nodes to the end.
        let mut i = 7;
        while let Some(n) = u16_at(i) { nodes.push(n); i += 2; }
    } else {
        let mut i = 6;
        let count = bytes.get(i).copied().unwrap_or(0) as usize;
        i += 1;
        for _ in 0..count { if let Some(n) = u16_at(i) { nodes.push(n); } i += 2; }
        let clusters = bytes.get(i).copied().unwrap_or(0) as usize;
        i += 1 + clusters * 2;
        let picks = bytes.get(i).copied().unwrap_or(0) as usize;
        i += 1;
        for _ in 0..picks {
            if let (Some(lo), Some(hi)) = (u16_at(i + 2), u16_at(i)) { choices.push((lo, hi)); }
            i += 4;
        }
    }
    if nodes.is_empty() { return Err("That atlas tree link has no points in it".into()); }
    Ok((nodes, choices))
}

/// The pathofexile.com link for a tree (to open it on the site and copy it in game).
pub fn official_link(nodes: &[u32], choices: &[(u32, u32)]) -> String {
    let g = graph();
    let pts: Vec<u32> = nodes.iter().copied().filter(|n| *n != g.start).take(255).collect();
    let mut b: Vec<u8> = 6u32.to_be_bytes().to_vec();
    b.extend([0, 0, pts.len() as u8]);
    for n in &pts { b.extend((*n as u16).to_be_bytes()); }
    b.push(0);
    b.push(choices.len().min(255) as u8);
    for (node, eff) in choices.iter().take(255) { b.extend((*eff as u16).to_be_bytes()); b.extend((*node as u16).to_be_bytes()); }
    let code = base64::engine::general_purpose::URL_SAFE.encode(b);
    format!("https://www.pathofexile.com/atlas-skill-tree/{code}")
}

/// A strategy from a pathofexile.com atlas tree code.
pub fn from_official(code: &str, link: &str) -> Result<Strategy, String> {
    let (nodes, choices) = decode_official(code)?;
    let g = graph();
    let known = nodes.iter().filter(|n| g.has(**n)).count();
    if known * 2 < nodes.len() { return Err("That atlas tree is from a different version of the game (most of its points aren't on today's atlas)".into()); }
    let all = g.fill_paths(&nodes, &HashSet::new());
    Ok(Strategy { site: "official".into(), link: link.to_string(), name: String::new(),
        variants: vec![Variant { name: "Atlas tree".into(), nodes: all, choices, notes: vec![] }], ..Default::default() })
}

// ---------- maxroll ----------

/// A strategy from maxroll's planner JSON (`planners.maxroll.gg/profiles/poe/<id>`).
pub fn from_maxroll(v: &Value, link: &str) -> Result<Strategy, String> {
    let data: Value = match &v["data"] {
        Value::String(s) => serde_json::from_str(s).map_err(|_| "maxroll sent something unexpected".to_string())?,
        d @ Value::Object(_) => d.clone(),
        _ => return Err("maxroll sent no strategy (is the link right?)".into()),
    };
    let meta: Value = match &v["metadata"] {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        m => m.clone(),
    };
    let g = graph();
    let profiles = data["profiles"].as_array().cloned().unwrap_or_default();
    let mut variants = vec![];
    for p in &profiles {
        let pname = p["name"].as_str().unwrap_or("").trim().to_string();
        let mut pnotes = vec![];
        for (key, title) in [("setup", "Map device & setup"), ("atlas", "Atlas tree tips")] {
            push_notes(&mut pnotes, title, &p["widgetNotes"][key]);
        }
        for l in p["loot"].as_array().into_iter().flatten() {
            let t = l["title"].as_str().unwrap_or("").trim();
            push_notes(&mut pnotes, &if t.is_empty() { "Loot".to_string() } else { format!("Loot: {t}") }, &l["text"]);
        }
        let steps = p["atlas"]["variants"].as_array().cloned().unwrap_or_default();
        for s in &steps {
            let ids = |x: &Value| -> Vec<u32> { x.as_array().into_iter().flatten().filter_map(|n| n.as_u64()).map(|n| n as u32).collect() };
            let mut targets = ids(&s["history"]);
            targets.extend(ids(&s["pathing"]["include"]));
            let exclude: HashSet<u32> = ids(&s["pathing"]["exclude"]).into_iter().collect();
            let nodes = g.fill_paths(&targets, &exclude);
            let mut choices: Vec<(u32, u32)> = s["masteries"].as_object().into_iter().flatten()
                .filter_map(|(k, e)| Some((k.parse().ok()?, e.as_u64()? as u32))).collect();
            choices.sort();
            let sname = s["name"].as_str().unwrap_or("").trim().to_string();
            let name = match (profiles.len() > 1, steps.len() > 1) {
                (true, true) => format!("{pname} · {sname}"),
                (true, false) => pname.clone(),
                (false, true) => sname.clone(),
                (false, false) => if pname.is_empty() { "Atlas tree".into() } else { pname.clone() },
            };
            let mut notes = vec![];
            push_notes(&mut notes, "This step", &s["notes"]);
            notes.extend(pnotes.iter().cloned());
            variants.push(Variant { name: if name.is_empty() { format!("Tree {}", variants.len() + 1) } else { name }, nodes, choices, notes });
        }
    }
    if variants.iter().all(|x| x.nodes.len() <= 1) {
        return Err("That maxroll page has no atlas tree (is it a farming strategy?)".into());
    }
    let mut notes = vec![];
    for (key, title) in [("intro", "About"), ("gameplay", "How to play"), ("summary", "Summary")] {
        push_notes(&mut notes, title, &data["globalNotes"][key]);
    }
    let st = &meta["strategy"];
    let device = if st.is_object() {
        let maps: Vec<String> = v["search_metadata"]["items"].as_array().into_iter().flatten()
            .filter_map(|x| x.as_str()).map(String::from).collect();
        let maps = if maps.is_empty() {
            st["device"].as_array().into_iter().flatten().filter_map(|d| d["base"].as_str()).map(map_from_metadata).collect()
        } else { maps };
        let threads = st["threads"].as_array().into_iter().flatten().filter_map(|t| {
            let m = t["mechanic"].as_str()?;
            let m = m.split(['_', ' ']).map(cap).collect::<Vec<_>>().join(" ");
            Some(match t["count"].as_u64() { Some(c) if c > 1 => format!("{m} ×{c}"), _ => m })
        }).collect();
        let rolls = st["mapRolls"].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(String::from).collect();
        let d = Device { maps, threads, rolls, text: meta["description"].as_str().unwrap_or("").trim().to_string() };
        (d != Device::default()).then_some(d)
    } else { None };
    let author = meta["author"]["contentCreator"].as_str().or(meta["author"]["name"].as_str()).unwrap_or("").to_string();
    Ok(Strategy { name: v["name"].as_str().unwrap_or("").trim().to_string(), site: "maxroll".into(), link: link.to_string(),
        author, device, notes, variants, ..Default::default() })
}

fn cap(w: &str) -> String {
    let mut c = w.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// "Metadata/Items/Maps/MapWorldsCitySquare" -> "City Square Map".
fn map_from_metadata(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    let core = last.trim_start_matches("MapWorlds").trim_start_matches("Map");
    let mut out = String::new();
    for (i, ch) in core.chars().enumerate() {
        if i > 0 && ch.is_uppercase() { out.push(' '); }
        out.push(ch);
    }
    format!("{out} Map")
}

fn push_notes(out: &mut Vec<Notes>, title: &str, rich: &Value) {
    let lines = lexical(rich);
    if !lines.is_empty() { out.push(Notes { title: title.to_string(), lines }); }
}

/// maxroll's notes (Lexical editor JSON) as plain lines.
pub fn lexical(v: &Value) -> Vec<NoteLine> {
    let mut out = vec![];
    let root = if v["root"].is_object() { &v["root"] } else { v };
    for c in root["children"].as_array().into_iter().flatten() { block(c, 0, &mut out); }
    // Drop blank lines at the ends.
    while out.last().is_some_and(|l| l.text.is_empty()) { out.pop(); }
    out
}

fn block(n: &Value, depth: u8, out: &mut Vec<NoteLine>) {
    let line = |kind: &str, text: String, out: &mut Vec<NoteLine>| {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() { out.push(NoteLine { kind: kind.into(), depth, text }); }
    };
    match n["type"].as_str().unwrap_or("") {
        "heading" => line("h", inline(n), out),
        "list" => {
            for item in n["children"].as_array().into_iter().flatten() {
                let kids = item["children"].as_array().cloned().unwrap_or_default();
                let (lists, rest): (Vec<Value>, Vec<Value>) = kids.into_iter().partition(|k| k["type"] == "list");
                line("li", inline(&serde_json::json!({ "children": rest })), out);
                for l in &lists { block(l, depth + 1, out); }
            }
        }
        "paragraph" | "quote" => line("p", inline(n), out),
        _ if n["children"].is_array() => { for c in n["children"].as_array().into_iter().flatten() { block(c, depth, out); } }
        _ => line("p", inline_one(n, &mut false), out),
    }
}

fn inline(n: &Value) -> String {
    let mut s = String::new();
    let mut after_embed = false;
    for c in n["children"].as_array().into_iter().flatten() { s.push_str(&inline_one(c, &mut after_embed)); }
    s
}

fn inline_one(c: &Value, after_embed: &mut bool) -> String {
    match c["type"].as_str().unwrap_or("") {
        "embed" => {
            let t = c["text"].as_str().unwrap_or("").to_string();
            // Embeds next to each other run together ("Breach ThreadRitual Thread").
            let s = if *after_embed { format!(" · {t}") } else { t };
            *after_embed = true;
            s
        }
        "linebreak" => { *after_embed = false; " ".into() }
        "text" => { *after_embed = false; c["text"].as_str().unwrap_or("").to_string() }
        _ => { *after_embed = false; inline(c) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_loads() {
        let g = graph();
        assert_eq!(g.start, 29045);
        assert!(g.has(21453));
        assert_eq!(g.kind(29045), 5);
    }

    #[test]
    fn links() {
        assert_eq!(parse_link("https://maxroll.gg/poe/farming-strategy-planner/vg1ga30g").unwrap(), Link::Maxroll("vg1ga30g".into()));
        assert_eq!(parse_link("https://maxroll.gg/poe/farming-strategy-planner/vg1ga30g#2").unwrap(), Link::Maxroll("vg1ga30g".into()));
        assert_eq!(parse_link("vg1ga30g").unwrap(), Link::Maxroll("vg1ga30g".into()));
        assert_eq!(parse_link("https://www.pathofexile.com/fullscreen-atlas-skill-tree/3.30.0/AAAABgAAAQBx").unwrap(), Link::Official("AAAABgAAAQBx".into()));
        assert!(parse_link("https://www.pathofexile.com/trade/search/Standard").is_err());
        assert!(parse_link("https://poe.ninja/builds").is_err());
        assert!(parse_link("").is_err());
    }

    #[test]
    fn official_round_trip() {
        let g = graph();
        let nodes = g.fill_paths(&[21453], &HashSet::new());
        assert!(nodes.len() > 2 && nodes.contains(&21453));
        let link = official_link(&nodes, &[(712, 8618)]);
        assert!(link.starts_with("https://www.pathofexile.com/atlas-skill-tree/AAAABg"));
        let Link::Official(code) = parse_link(&link).unwrap() else { panic!() };
        let (back, choices) = decode_official(&code).unwrap();
        let want: Vec<u32> = nodes.iter().copied().filter(|n| *n != g.start).collect();
        assert_eq!(back, want);
        assert_eq!(choices, vec![(712, 8618)]);
        let s = from_official(&code, &link).unwrap();
        assert_eq!(s.variants[0].nodes.len(), nodes.len());
        assert!(decode_official("!!!").is_err());
    }

    #[test]
    fn fill_paths_joins_and_avoids() {
        let g = graph();
        let a = g.fill_paths(&[21453], &HashSet::new());
        // Already joined: nothing new.
        let b = g.fill_paths(&a, &HashSet::new());
        assert_eq!(a.len(), b.len());
        // Every node touches an earlier one.
        for (i, n) in a.iter().enumerate().skip(1) {
            assert!(a[..i].iter().any(|m| g.adj[n].contains(m)), "{n} isn't joined");
        }
        // Avoiding the first step forces another way (or none).
        let ex: HashSet<u32> = HashSet::from([a[1]]);
        let c = g.fill_paths(&[21453], &ex);
        assert!(!c.contains(&a[1]));
    }

    #[test]
    fn maxroll_strategy() {
        let v: Value = serde_json::from_str(include_str!("../data/test_maxroll_vg1ga30g.json")).unwrap();
        let s = from_maxroll(&v, "https://maxroll.gg/poe/farming-strategy-planner/vg1ga30g").unwrap();
        assert_eq!(s.name, "Leaguestart Shrine/Magic Atlas");
        assert_eq!(s.author, "palsteron");
        assert_eq!(s.variants.len(), 2);
        assert_eq!(s.variants[0].name, "Variant 1");
        // Variant 1 is a short path to Cassia's Pride; variant 2 is a full 105-point tree.
        let pts = |i: usize| s.variants[i].nodes.len() - 1;
        assert_eq!(pts(0), 14);
        assert_eq!(pts(1), 105);
        assert_eq!(s.variants[1].choices, vec![(12887, 20818), (28029, 36379)]);
        for x in [36137, 55117, 57739, 64048, 8132] { assert!(!s.variants[0].nodes.contains(&x)); }
    }

    #[test]
    fn notes_from_lexical() {
        let v = serde_json::json!({"root": {"children": [
            {"type": "heading", "tag": "h1", "children": [{"type": "text", "text": "Setup"}]},
            {"type": "paragraph", "children": [{"type": "text", "text": "Run "}, {"type": "embed", "text": "City Square Map"}, {"type": "text", "text": " maps."}]},
            {"type": "paragraph", "children": []},
            {"type": "list", "children": [
                {"type": "listitem", "children": [{"type": "text", "text": "Threads:"},
                    {"type": "list", "children": [{"type": "listitem", "children": [{"type": "embed", "text": "Breach Thread"}, {"type": "embed", "text": "Ritual Thread"}]}]}]}
            ]}
        ]}});
        let l = lexical(&v);
        assert_eq!(l.len(), 4);
        assert_eq!((l[0].kind.as_str(), l[0].text.as_str()), ("h", "Setup"));
        assert_eq!(l[1].text, "Run City Square Map maps.");
        assert_eq!((l[2].kind.as_str(), l[2].depth), ("li", 0));
        assert_eq!((l[3].text.as_str(), l[3].depth), ("Breach Thread · Ritual Thread", 1));
    }

    #[test]
    fn store() {
        let mut st = AtlasStore::default();
        let a = st.add(Strategy { name: "Harvest".into(), variants: vec![Variant::default(), Variant::default()], ..Default::default() }, "");
        let b = st.add(Strategy::default(), "My essence");
        assert_eq!(st.active, Some(b));
        assert_eq!(st.current().unwrap().name, "My essence");
        st.use_one(a, 5);
        assert_eq!((st.active, st.variant), (Some(a), 1));
        st.rename(a, "  Lifeforce ").unwrap();
        assert!(st.rename(a, " ").is_err());
        st.remove(a);
        assert_eq!(st.active, Some(b));
        assert_eq!(map_from_metadata("Metadata/Items/Maps/MapWorldsCitySquare"), "City Square Map");
    }
}

/// Dev check against a saved maxroll response: `ATLAS_SAMPLE=path cargo test -p filter-core sample -- --ignored --nocapture`.
#[cfg(test)]
#[test]
#[ignore]
fn sample() {
    let path = std::env::var("ATLAS_SAMPLE").expect("ATLAS_SAMPLE");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let s = from_maxroll(&v, "").unwrap();
    println!("{} by {} · device {:?}", s.name, s.author, s.device);
    for n in &s.notes { println!("## {} ({} lines) {:?}", n.title, n.lines.len(), n.lines.first()); }
    for x in &s.variants {
        println!("{}: {} points, {} picks, notes {:?}", x.name, x.nodes.len() - 1, x.choices.len(), x.notes.iter().map(|n| n.title.clone()).collect::<Vec<_>>());
    }
}
