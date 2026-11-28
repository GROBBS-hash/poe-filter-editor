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
    pub fn ids(&self) -> Vec<u32> { self.info.keys().copied().collect() }
    pub fn has(&self, id: u32) -> bool { self.info.contains_key(&id) }
    pub fn kind(&self, id: u32) -> u8 { self.info.get(&id).map(|i| i.0).unwrap_or(0) }
    pub fn name(&self, id: u32) -> &str { self.info.get(&id).map(|i| i.1.as_str()).unwrap_or("") }
    pub fn neighbours(&self, id: u32) -> &[u32] { self.adj.get(&id).map(|v| v.as_slice()).unwrap_or(&[]) }

    /// Every node the targets need, in the order you'd take them: each target in turn, after
    /// the shortest path joining it to what's taken so far (never through `exclude`). Like
    /// maxroll's auto-pathing, where a strategy only stores the nodes picked by hand; a tree
    /// listed in the order it was taken comes back unchanged. Start node first (points = len - 1).
    pub fn fill_paths(&self, targets: &[u32], exclude: &HashSet<u32>) -> Vec<u32> {
        let wanted: Vec<u32> = targets.iter().copied().filter(|t| self.has(*t)).collect();
        let mut have: HashSet<u32> = HashSet::from([self.start]);
        let mut order = vec![self.start];
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
        }
        order
    }

    /// The points in an order you could take them (each joined to one before it), keeping the given order where
    /// it already works; points not joined to the start through the others are left out. Start node first.
    pub fn joined_order(&self, nodes: &[u32]) -> Vec<u32> {
        let want: HashSet<u32> = nodes.iter().copied().filter(|n| self.has(*n)).collect();
        let mut have: HashSet<u32> = HashSet::from([self.start]);
        let mut order = vec![self.start];
        let mut left: Vec<u32> = nodes.iter().copied().filter(|n| *n != self.start && want.contains(n)).collect();
        left.dedup();
        loop {
            // The first one in the list that joins what we have.
            let Some(i) = left.iter().position(|n| self.neighbours(*n).iter().any(|m| have.contains(m))) else { break };
            let n = left.remove(i);
            if have.insert(n) { order.push(n); }
        }
        order
    }

    /// The tree in steps, like maxroll's "Leveling Path": a step ends at each notable or keystone,
    /// or where the next point isn't joined to this one (a new branch). Named after its last point.
    pub fn steps(&self, nodes: &[u32]) -> Vec<Step> {
        let mut out = vec![];
        let mut cur: Vec<u32> = vec![];
        let pts: Vec<u32> = nodes.iter().copied().filter(|n| *n != self.start).collect();
        let mut total = 0;
        for (i, &n) in pts.iter().enumerate() {
            cur.push(n);
            let next = pts.get(i + 1);
            let joined = next.is_some_and(|m| self.adj.get(&n).is_some_and(|a| a.contains(m)));
            if next.is_none() || !joined || matches!(self.kind(n), 1 | 2) {
                total += cur.len();
                out.push(Step { name: self.name(n).to_string(), node: n, nodes: std::mem::take(&mut cur), total });
            }
        }
        out
    }
}

/// One step of a tree: the points to take, ending at `node` (usually a notable).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Step {
    pub name: String,
    pub node: u32,
    pub nodes: Vec<u32>,
    /// Points spent once this step is done.
    pub total: usize,
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
    /// What to take in order (empty in strategies saved before 20 Nov: refreshed on load).
    #[serde(default)]
    pub steps: Vec<Step>,
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
    /// When you last changed its tree in the editor (0 = never).
    pub edited: u64,
    /// A tree from Build a tree: what it was built for (goals, mechanics, Threads, maps, Always / Never), so it can
    /// be opened there again and changed (HANDOFF "save his Threads / goal with a strategy").
    pub ask: Option<crate::atlas_opt::Ask>,
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
        crate::keep::load(path).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        crate::keep::save(path, self, true).map_err(|e| e.to_string())
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

    /// Saves an edited tree: over the strategy's tree, or (with a name) as a new strategy, leaving the old one
    /// as it was. Points are put in an order where each joins one before it; points cut off from the start are
    /// dropped. Returns the id it's saved under (shown in game from now on).
    pub fn set_tree(&mut self, id: u64, variant: usize, nodes: &[u32], choices: &[(u32, u32)], as_new: Option<&str>, now: u64) -> Result<u64, String> {
        let g = graph();
        let st = self.list.iter().find(|s| s.id == id).ok_or("That strategy is gone")?;
        if variant >= st.variants.len() { return Err("That tree is gone".into()); }
        let order = g.joined_order(nodes);
        let kept: HashSet<u32> = order.iter().copied().collect();
        let choices: Vec<(u32, u32)> = choices.iter().copied().filter(|(n, _)| kept.contains(n)).collect();
        let mut s = st.clone();
        let v = &mut s.variants[variant];
        v.steps = g.steps(&order);
        v.nodes = order;
        v.choices = choices;
        s.edited = now;
        match as_new {
            Some(name) => Ok(self.add(s, name)),
            None => {
                let slot = self.list.iter_mut().find(|x| x.id == id).unwrap();
                *slot = s;
                self.active = Some(id);
                self.variant = variant;
                Ok(id)
            }
        }
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
    /// "Atlas Tree Planner" (atlas-blush-beta.vercel.app and copies): the tree is in the part after '#', read
    /// against the planner's own data.json (its link format walks that file's graph).
    Planner { data_url: String, hash: String },
    /// poeplanner.com atlas tree code (`/atlas-tree/<code>`).
    PoePlanner(String),
    /// poeplanner.com short link id (`/a/<id>`): `api.poeplanner.com/api/b/<id>` gives the code.
    PoePlannerShort(String),
}

pub fn poeplanner_short_url(id: &str) -> String { format!("https://api.poeplanner.com/api/b/{id}") }

/// poeplanner's atlas tree code: little-endian u16 format version, u16 tree version, (version 5+) a "PoE 2" byte,
/// u16 count and that many u16 node ids, then gzipped notes. It keeps no option picks.
pub fn decode_poeplanner(code: &str) -> Result<Vec<u32>, String> {
    let code = code.trim().trim_end_matches('=');
    let cfg = base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let bytes = base64::engine::GeneralPurpose::new(&base64::alphabet::URL_SAFE, cfg).decode(code)
        .or_else(|_| base64::engine::GeneralPurpose::new(&base64::alphabet::STANDARD, cfg).decode(code))
        .map_err(|_| "That poeplanner link looks broken (can't read its code)".to_string())?;
    let u16_at = |i: usize| -> Option<u16> { Some(u16::from_le_bytes([*bytes.get(i)?, *bytes.get(i + 1)?])) };
    let version = u16_at(0).ok_or("That poeplanner link is too short")?;
    if version > 5 { return Err("That poeplanner link is newer than this app understands".into()); }
    let mut i = 4;
    if version >= 5 {
        if bytes.get(i).copied().unwrap_or(0) != 0 { return Err("That's a Path of Exile 2 atlas tree".into()); }
        i += 1;
    }
    let count = u16_at(i).ok_or("That poeplanner link is too short")? as usize;
    i += 2;
    let ids: Vec<u32> = (0..count).filter_map(|k| u16_at(i + 2 * k)).map(u32::from).collect();
    if ids.is_empty() { return Err("That atlas tree link has no points in it".into()); }
    Ok(ids)
}

/// A strategy from a poeplanner atlas tree code.
pub fn from_poeplanner(code: &str, link: &str) -> Result<Strategy, String> {
    let nodes = decode_poeplanner(code)?;
    let g = graph();
    let known = nodes.iter().filter(|n| g.has(**n)).count();
    if known * 2 < nodes.len() { return Err("That atlas tree is from a different version of the game (most of its points aren't on today's atlas)".into()); }
    let nodes: Vec<u32> = nodes.into_iter().filter(|n| g.has(*n)).collect();
    let all = g.fill_paths(&nodes, &HashSet::new());
    Ok(Strategy { site: "poeplanner".into(), link: link.trim().to_string(), name: String::new(),
        variants: vec![Variant { name: "Atlas tree".into(), steps: g.steps(&all), nodes: all, choices: vec![], notes: vec![] }], ..Default::default() })
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
    // Atlas Tree Planner (he asked 24 Nov): "https://atlas-blush-beta.vercel.app/#<code>". Its data.json sits next
    // to the page.
    if let Some((page, hash)) = t.split_once('#') {
        if page.to_lowercase().contains("atlas-blush") && !hash.trim().is_empty() {
            let base = page.split('?').next().unwrap_or(page);
            let dir = if base.ends_with('/') { base.to_string() } else if base.rsplit('/').next().is_some_and(|l| l.contains('.')) {
                format!("{}/", base.rsplit_once('/').map(|x| x.0).unwrap_or(base))
            } else { format!("{base}/") };
            return Ok(Link::Planner { data_url: format!("{dir}data.json"), hash: hash.trim().to_string() });
        }
    }
    // poeplanner (24 Nov): mobalytics' PoE 1 guides link their atlas trees there. "/atlas-tree/<code>" has the
    // tree; "/a/<id>" is a short link poeplanner's API turns into that code.
    if lower.contains("poeplanner.com") {
        if lower.contains("/poe2") { return Err("That's a Path of Exile 2 atlas tree".into()); }
        if lower.contains("/atlas-tree/") && !last.is_empty() { return Ok(Link::PoePlanner(last)); }
        if lower.contains("/a/") && !last.is_empty() { return Ok(Link::PoePlannerShort(last)); }
        return Err("That poeplanner link isn't to an atlas tree (it should have /atlas-tree/ or /a/ in it)".into());
    }
    if lower.contains("mobalytics.gg") {
        return Err("mobalytics pages can't be read directly (they block apps). Their PoE 1 guides link each atlas tree on poeplanner.com: right-click that link, copy it, and paste it here".into());
    }
    if lower.contains("poe.ninja") {
        // poe.ninja's atlas trees (24 Nov): ".../poe1/atlas-trees/<league>/tree/<pathofexile.com code>#i=0".
        if lower.contains("/atlas-trees/") && lower.contains("/tree/") && !last.is_empty() { return Ok(Link::Official(last)); }
        if lower.contains("/poe2/") { return Err("That's a Path of Exile 2 link".into()); }
        return Err("That poe.ninja link isn't to one atlas tree: open a tree on poe.ninja's Atlas Trees page and copy that page's link".into());
    }
    if !t.contains('/') && !t.contains(' ') && (4..=16).contains(&t.len()) && t.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(Link::Maxroll(t.to_string()));
    }
    if !t.contains('/') && t.len() > 16 && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_=+/".contains(c)) {
        return Ok(Link::Official(t.to_string()));
    }
    Err("Paste a maxroll farming strategy link, a pathofexile.com atlas tree link or an Atlas Tree Planner link".into())
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
        variants: vec![Variant { name: "Atlas tree".into(), steps: g.steps(&all), nodes: all, choices, notes: vec![] }], ..Default::default() })
}

// ---------- Atlas Tree Planner ----------

/// Reads an Atlas Tree Planner link code against that planner's data.json. Three kinds:
/// - compact (first byte 0xA1): from the start point, a breadth-first walk of its graph (neighbours in number
///   order, masteries left out); one bit per newly reached neighbour says "taken", then 3 bits per taken
///   multiple-choice notable for the option picked (index into its `variantEffects`);
/// - old "id,id.option" lists;
/// - a pathofexile.com tree code.
pub fn decode_planner(hash: &str, data: &Value) -> Result<(Vec<u32>, Vec<(u32, u32)>), String> {
    let hash = hash.trim().trim_start_matches('#');
    let nodes = data["nodes"].as_object().ok_or("The planner's atlas data looks wrong")?;
    let effect = |id: u32, index: usize| -> Option<u32> {
        nodes.get(&id.to_string())?["variantEffects"].get(index)?["effect"].as_u64().map(|e| e as u32)
    };
    if hash.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        let (mut ids, mut picks) = (vec![], vec![]);
        for part in hash.split(',').filter(|p| !p.is_empty()) {
            let mut it = part.split('.');
            let Some(Ok(id)) = it.next().map(|x| x.parse::<u32>()) else { continue };
            ids.push(id);
            if let Some(Ok(i)) = it.next().map(|x| x.parse::<usize>()) { if let Some(e) = effect(id, i) { picks.push((id, e)); } }
        }
        if ids.is_empty() { return Err("That atlas tree link has no points in it".into()); }
        return Ok((ids, picks));
    }
    let cfg = base64::engine::GeneralPurposeConfig::new()
        .with_decode_padding_mode(base64::engine::DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true);
    let bytes = base64::engine::GeneralPurpose::new(&base64::alphabet::URL_SAFE, cfg).decode(hash.trim_end_matches('='))
        .map_err(|_| "That atlas tree link looks broken (can't read its code)".to_string())?;
    if bytes.first() != Some(&0xA1) { return decode_official(hash); }

    // The planner's graph: links both ways between real nodes, none to or from masteries.
    let mastery = |id: &str| nodes.get(id).is_some_and(|n| n["isMastery"].as_bool() == Some(true));
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for (id, n) in nodes {
        if id == "root" { continue; }
        let Ok(a) = id.parse::<u32>() else { continue };
        for t in n["out"].as_array().into_iter().flatten().chain(n["in"].as_array().into_iter().flatten()) {
            let Some(t) = t.as_str() else { continue };
            if t == "root" || !nodes.contains_key(t) || mastery(id) || mastery(t) { continue; }
            let Ok(b) = t.parse::<u32>() else { continue };
            adj.entry(a).or_default().push(b);
            adj.entry(b).or_default().push(a);
        }
    }
    for v in adj.values_mut() { v.sort_unstable(); v.dedup(); }
    let root: u32 = data["nodes"]["root"]["out"][0].as_str().and_then(|s| s.parse().ok()).ok_or("The planner's atlas data has no start point")?;
    let mut pos = 8usize;
    let mut bit = || -> bool { let b = bytes.get(pos >> 3).copied().unwrap_or(0) >> (7 - (pos & 7)) & 1; pos += 1; b == 1 };
    let mut seen: HashSet<u32> = HashSet::from([root]);
    let mut order = vec![root];
    let mut i = 0;
    while i < order.len() {
        for &nb in adj.get(&order[i]).map(|v| v.as_slice()).unwrap_or(&[]) {
            if !seen.insert(nb) { continue; }
            if bit() { order.push(nb); }
        }
        i += 1;
    }
    let mut picks = vec![];
    for &id in &order {
        if nodes.get(&id.to_string()).is_some_and(|n| n["isMultipleChoiceVariant"].as_bool() == Some(true)) {
            let index = (bit() as usize) * 4 + (bit() as usize) * 2 + bit() as usize;
            if let Some(e) = effect(id, index) { picks.push((id, e)); }
        }
    }
    let ids: Vec<u32> = order.into_iter().skip(1).collect();
    if ids.is_empty() { return Err("That atlas tree link has no points in it".into()); }
    Ok((ids, picks))
}

/// A strategy from an Atlas Tree Planner link (`data` is that planner's data.json).
pub fn from_planner(hash: &str, data: &Value, link: &str) -> Result<Strategy, String> {
    let (nodes, choices) = decode_planner(hash, data)?;
    let g = graph();
    let known = nodes.iter().filter(|n| g.has(**n)).count();
    if known * 2 < nodes.len() { return Err("That atlas tree is from a different version of the game (most of its points aren't on today's atlas)".into()); }
    let nodes: Vec<u32> = nodes.into_iter().filter(|n| g.has(*n)).collect();
    let all = g.fill_paths(&nodes, &HashSet::new());
    let kept: HashSet<u32> = all.iter().copied().collect();
    let choices = choices.into_iter().filter(|(n, _)| kept.contains(n)).collect();
    Ok(Strategy { site: "planner".into(), link: link.trim().to_string(), name: String::new(),
        variants: vec![Variant { name: "Atlas tree".into(), steps: g.steps(&all), nodes: all, choices, notes: vec![] }], ..Default::default() })
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
            variants.push(Variant { name: if name.is_empty() { format!("Tree {}", variants.len() + 1) } else { name }, steps: g.steps(&nodes), nodes, choices, notes });
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
    fn planner_links() {
        let l = parse_link("https://atlas-blush-beta.vercel.app/#oYL77Yu0pm3sd8UfPl_8J74326baPkjlfLb2XcTOVftZ").unwrap();
        assert_eq!(l, Link::Planner { data_url: "https://atlas-blush-beta.vercel.app/data.json".into(), hash: "oYL77Yu0pm3sd8UfPl_8J74326baPkjlfLb2XcTOVftZ".into() });
        let l = parse_link("https://atlas-blush-beta.vercel.app/index.html#abc").unwrap();
        assert!(matches!(l, Link::Planner { data_url, .. } if data_url == "https://atlas-blush-beta.vercel.app/data.json"));
        // A small made-up planner graph: root → 1, 1 – 2, 1 – 3 (a choice notable), 3 – 4 (a mastery: left out).
        let data = serde_json::json!({ "nodes": {
            "root": { "out": ["1"] },
            "1": { "out": ["2", "3"], "in": [] },
            "2": { "out": [], "in": ["1"] },
            "3": { "out": ["4"], "in": ["1"], "isMultipleChoiceVariant": true, "variantEffects": [{ "effect": 70 }, { "effect": 71 }, { "effect": 72 }] },
            "4": { "out": [], "in": ["3"], "isMastery": true },
        }});
        // Walk from 1: neighbours 2 (bit 0), 3 (bit 1); from 3 nothing new. Then node 3's pick: 010 = option 2.
        // Bits after the marker: 0 1 0 1 0 → 0b0101_0000.
        let code = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([0xA1, 0b0101_0000]);
        assert_eq!(decode_planner(&code, &data).unwrap(), (vec![3], vec![(3, 72)]));
        assert_eq!(decode_planner("2,3.1", &data).unwrap(), (vec![2, 3], vec![(3, 71)]));
    }

    /// His link against the live planner data: `PLANNER_DATA=<path to data.json> cargo test -p filter-core planner_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn planner_real() {
        let data: Value = serde_json::from_str(&std::fs::read_to_string(std::env::var("PLANNER_DATA").unwrap()).unwrap()).unwrap();
        let code = std::env::var("PLANNER_CODE").unwrap_or("oYL77Yu0pm3sd8UfPI_8J74326baPkjlfLb2XcTOVftZ".into());
        // Read off a screenshot: try the look-alike letters too and list codes that give a whole 150-point tree.
        if std::env::var("PLANNER_SWAPS").is_ok() {
            let alike = |c: char| -> &'static [char] { match c { 'l' | 'I' | '1' => &['l', 'I', '1'], 'O' | '0' => &['O', '0'], _ => &[] } };
            let spots: Vec<usize> = code.char_indices().filter(|(_, c)| !alike(*c).is_empty()).map(|(i, _)| i).collect();
            let mut tries = vec![code.clone()];
            for &i in &spots {
                tries = tries.into_iter().flat_map(|t| alike(t.as_bytes()[i] as char).iter().map(move |r| { let mut s = t.clone(); s.replace_range(i..i + 1, &r.to_string()); s }).collect::<Vec<_>>()).collect();
            }
            for t in &tries {
                if let Ok((ids, picks)) = decode_planner(t, &data) { if ids.len() >= 140 { println!("{t}: {} points, {} picks", ids.len(), picks.len()); } }
            }
            println!("{} tried", tries.len());
        }
        let s = from_planner(&code, &data, "x").unwrap();
        let v = &s.variants[0];
        let g = graph();
        println!("{} points, picks {:?}", v.nodes.len() - 1, v.choices);
        let mut names: Vec<&str> = v.nodes.iter().filter(|n| matches!(g.kind(**n), 1 | 2)).map(|n| g.name(*n)).collect();
        names.sort();
        println!("{}", names.join(", "));
    }

    #[test]
    fn poe_ninja_links() {
        // A Covens tree from poe.ninja's Atlas Trees page (24 Nov): the code is pathofexile.com's.
        let code = "AAAABgAAlwDOAlkCyAMJA2IHwwg_CLcKGQqqCwoLcQzgDO8O0w7mD2MQ4RFXFNoXFBfLGMQZdhmCHXQeWx9mJucovyjQKsArzivXMDUz5DYJNhI4IThvO9w8Sj4xPlk-Xz-qP9VAakFfQmhC_0RUR89M21b-WptazltLW8tb81zZXeheKl_rYrljtWWXZ-loQGp9bz1wyXErcXVzGHOlc7V1_3Y6dkV5w3oIewp7In0TgPCC4oQchEyGWYbGh2aIIojkiWiOapIslBmUxJZBlySY65nUnbGeKqBdoYei3qSspm2ngqgsqkevda_Bsfu0hrX9twK3EbvrvLy9FcC6xU_F_sYzxtDHL8dGyXTPydEk0vzV_tq93T_eHN8j4N7hi-Kn4zfjcOTM9Uz35Ph4-rL62vxhAAA";
        assert_eq!(parse_link(&format!("https://poe.ninja/poe1/atlas-trees/covens/tree/{code}#i=0")).unwrap(), Link::Official(code.into()));
        assert!(parse_link("https://poe.ninja/poe1/atlas-trees/covens").is_err());
        let s = from_official(code, "x").unwrap();
        assert!(s.variants[0].nodes.len() > 140, "{}", s.variants[0].nodes.len());
    }

    #[test]
    fn poeplanner_links() {
        // From a mobalytics guide (bigducks, 3.30 early atlas): poeplanner.com/a/6Rtl → this code.
        let code = "BQAeAAAlAGEfIWAo58aGtWZ9WiKIT_cdfXQdt12L4X2RDkre_p7Ye2B_q9U_WnkF8d2D4RAZCszkfbWCiOtfxIVOG3M-ru8Yiwtx2VypQ4fUFAAfiwgAAAAAAAADAwAAAAAAAAAAAA==";
        assert_eq!(parse_link(&format!("https://poeplanner.com/atlas-tree/{code}")).unwrap(), Link::PoePlanner(code.into()));
        assert_eq!(parse_link("https://poeplanner.com/a/6Rtl").unwrap(), Link::PoePlannerShort("6Rtl".into()));
        assert!(parse_link("https://mobalytics.gg/poe/profile/bigducks/guides/3-30-early-atlas-progression").unwrap_err().contains("poeplanner"));
        let ids = decode_poeplanner(code).unwrap();
        assert_eq!(ids.len(), 37);
        let s = from_poeplanner(code, "x").unwrap();
        let g = graph();
        assert!(s.variants[0].nodes.iter().any(|n| g.name(*n) == "Shaping the World"));
        assert!(s.variants[0].nodes.len() >= 38);
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
        // Same steps as maxroll's "Leveling Path" for variant 2.
        let st = &s.variants[1].steps;
        let first: Vec<(&str, usize, usize)> = st.iter().take(13).map(|x| (x.name.as_str(), x.nodes.len(), x.total)).collect();
        assert_eq!(first, vec![("Supplication", 5, 5), ("Shaping the Mountains", 8, 13), ("Shaping the Skies", 8, 21),
            ("Shaping the World", 8, 29), ("Path of Worship", 6, 35), ("Drawn to Power", 5, 40), ("Syncretism", 2, 42),
            ("False Prophets", 11, 53), ("Deep Bloodlines", 5, 58), ("Group Solidarity", 6, 64), ("Magic Monsters", 1, 65),
            ("Shrine Chance", 3, 68), ("Blended Realities", 4, 72)]);
        assert_eq!(st.last().unwrap().total, 105);
        assert_eq!(s.variants[0].steps.last().unwrap().name, "Cassia's Pride");
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
    fn edit_tree() {
        let g = graph();
        let a = g.fill_paths(&[21453], &HashSet::new());
        let mut st = AtlasStore::default();
        let id = st.add(Strategy { name: "Shrines".into(), site: "maxroll".into(), variants: vec![Variant { name: "T".into(), nodes: a.clone(), ..Default::default() }], ..Default::default() }, "");
        // Reversed order and a stray far-away point: back in a joined order, stray dropped.
        let mut edited: Vec<u32> = a.iter().rev().copied().collect();
        let stray = *g.info.keys().find(|n| !a.contains(n) && g.neighbours(**n).iter().all(|m| !a.contains(m))).unwrap();
        edited.push(stray);
        let new_id = st.set_tree(id, 0, &edited, &[(stray, 1)], Some("Shrines (mine)"), 5).unwrap();
        assert_ne!(new_id, id);
        let mine = st.list.iter().find(|s| s.id == new_id).unwrap();
        assert_eq!(mine.variants[0].nodes.len(), a.len());
        assert!(!mine.variants[0].nodes.contains(&stray) && mine.variants[0].choices.is_empty());
        assert_eq!(mine.edited, 5);
        assert_eq!(mine.variants[0].steps.last().unwrap().total, a.len() - 1);
        // The original is untouched.
        assert_eq!(st.list.iter().find(|s| s.id == id).unwrap().variants[0].nodes, a);
        // Saving over: only the start left when everything is removed.
        st.set_tree(id, 0, &[], &[], None, 6).unwrap();
        assert_eq!(st.list.iter().find(|s| s.id == id).unwrap().variants[0].nodes, vec![g.start]);
        assert_eq!(st.active, Some(id));
        assert!(st.set_tree(id, 3, &[], &[], None, 7).is_err());
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
        println!("  steps: {}", x.steps.iter().map(|s| format!("{} {}/{}", s.name, s.nodes.len(), s.total)).collect::<Vec<_>>().join(", "));
    }
}

// ---------- what each tree earned (24 Nov, plan step 4 option 3) ----------

/// One saved tree's results: from map runs (loot the map tracker counted) and earnings sessions started with it.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct TreeResult {
    pub id: u64,
    pub maps: usize,
    /// Chaos of loot per map, and per hour (loot / time from each map's start to the next one's).
    pub loot_per_map: Option<f64>,
    pub loot_per_hour: Option<f64>,
    /// All of it: chaos, and seconds.
    pub loot: f64,
    pub secs: f64,
    pub sessions: usize,
    pub session_hours: f64,
    /// Chaos per hour over its earnings sessions.
    pub session_per_hour: Option<f64>,
}

/// Results per atlas tree, best per hour first. `runs`: map runs (start, seconds, loot chaos or None when it couldn't be read, tree); `sessions`: (hours, profit
/// chaos, tree) of finished earnings sessions.
pub fn tree_results(runs: &[(i64, i64, Option<f64>, Option<u64>)], sessions: &[(f64, f64, Option<u64>)]) -> Vec<TreeResult> {
    let mut out: Vec<TreeResult> = vec![];
    let get = |out: &mut Vec<TreeResult>, id: u64| -> usize {
        match out.iter().position(|r| r.id == id) { Some(i) => i, None => { out.push(TreeResult { id, ..Default::default() }); out.len() - 1 } }
    };
    // Maps: (start, seconds inside, loot). Each map counts its time inside plus your usual hideout turnaround, so breaks
    // don't lower a tree's div an hour (26 Nov, he picked it; it used to run on to the next map's start, up to an hour).
    let spans: Vec<(i64, i64)> = runs.iter().map(|r| (r.0, r.0 + r.1)).collect();
    let turn = crate::journey::usual_turnaround(&spans);
    let mut by_tree: std::collections::HashMap<u64, (usize, f64, f64)> = Default::default();
    for (_, own, loot, tree) in runs.iter() {
        // Loot that couldn't be read (login run out) isn't "no loot": the map doesn't count.
        let (Some(t), Some(loot)) = (tree, loot) else { continue };
        let e = by_tree.entry(*t).or_default();
        e.0 += 1; e.1 += (own + turn).max(60) as f64; e.2 += loot;
    }
    for (t, (n, secs, loot)) in by_tree {
        let i = get(&mut out, t);
        out[i].maps = n;
        out[i].loot = loot;
        out[i].secs = secs;
        if loot > 0.0 {
            out[i].loot_per_map = Some(loot / n as f64);
            out[i].loot_per_hour = Some(loot / (secs / 3600.0));
        }
    }
    for (hours, profit, tree) in sessions {
        let Some(t) = tree else { continue };
        let i = get(&mut out, *t);
        out[i].sessions += 1;
        out[i].session_hours += hours;
        let total = out[i].session_per_hour.unwrap_or(0.0) * (out[i].session_hours - hours) + profit;
        out[i].session_per_hour = (out[i].session_hours > 0.0).then(|| total / out[i].session_hours);
    }
    let rate = |r: &TreeResult| r.session_per_hour.or(r.loot_per_hour).unwrap_or(0.0);
    out.sort_by(|a, b| rate(b).total_cmp(&rate(a)));
    out
}

#[cfg(test)]
mod result_tests {
    #[test]
    fn per_tree() {
        // Tree 1: two 5-minute map cycles with 100c each = 1200c/h. Tree 2: one session, 2 hours, 600c profit.
        let runs = vec![(0, 280, Some(100.0), Some(1)), (300, 280, Some(100.0), Some(1)), (600, 280, None, Some(1)), (5000, 290, Some(0.0), None)];
        let r = super::tree_results(&runs, &[(2.0, 600.0, Some(2)), (1.0, 50.0, None)]);
        let t1 = r.iter().find(|x| x.id == 1).unwrap();
        assert_eq!(t1.maps, 2);
        // 280 s inside + the 90 s default turnaround (only two short gaps), twice: 200c in 740 s.
        assert!((t1.loot_per_hour.unwrap() - 200.0 / (740.0 / 3600.0)).abs() < 1.0, "{t1:?}");
        let t2 = r.iter().find(|x| x.id == 2).unwrap();
        assert_eq!(t2.session_per_hour, Some(300.0));
        assert_eq!(r[0].id, 1);
    }
}
