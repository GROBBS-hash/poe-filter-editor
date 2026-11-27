//! Reads a Path of Building build code (pobb.in / pastebin / raw export) into
//! stages: each with its passive tree, gem links and gear, like PoB shows them.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gem {
    pub name: String,
    pub level: u32,
    pub quality: u32,
    pub enabled: bool,
    pub support: bool,
    /// "r" / "g" / "b" / "w" (white).
    pub colour: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub label: String,
    /// Where it's socketed, e.g. "Body Armour" (empty if not set).
    pub slot: String,
    pub enabled: bool,
    pub gems: Vec<Gem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gear {
    pub slot: String,
    pub rarity: String,
    pub name: String,
    pub base: String,
    /// Item lines as PoB has them, tidied (sockets, implicits, mods...).
    pub lines: Vec<String>,
    /// The item written like the game's Ctrl+C copy, for the price window.
    #[serde(default)]
    pub copy: String,
}

/// A jewel: in a passive tree socket (`node`) or an abyssal socket of an item (`node` None, slot "Belt Abyssal Socket 1").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Jewel {
    pub node: Option<u32>,
    pub item: Gear,
    /// Cluster jewels: the notables they add ("1 Added Passive Skill is Feed the Fury").
    pub notables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tree {
    pub version: String,
    pub class_id: u32,
    pub ascend_id: u32,
    pub nodes: Vec<u32>,
    /// Official tree link for this stage (opens on pathofexile.com).
    pub url: String,
    /// Mastery choices: (mastery node, effect picked).
    #[serde(default)]
    pub masteries: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stage {
    pub title: String,
    pub tree: Option<Tree>,
    pub groups: Vec<Group>,
    pub gear: Vec<Gear>,
    /// Tree and abyssal jewels.
    #[serde(default)]
    pub jewels: Vec<Jewel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Build {
    pub class: String,
    pub ascendancy: String,
    pub level: u32,
    pub stages: Vec<Stage>,
    /// The stage PoB had open.
    pub active: usize,
}

fn colours() -> &'static HashMap<String, String> {
    static D: OnceLock<HashMap<String, String>> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/gem_colours.json")).unwrap_or_default())
}

/// Export code -> PoB's XML.
pub fn decode(code: &str) -> Result<String, String> {
    let clean: String = code.chars().filter(|c| !c.is_whitespace()).map(|c| match c { '-' => '+', '_' => '/', c => c }).collect();
    let clean = clean.trim_end_matches('=');
    let bytes = base64::engine::general_purpose::STANDARD_NO_PAD.decode(clean)
        .map_err(|_| "That doesn't look like a Path of Building code or link".to_string())?;
    let mut xml = String::new();
    flate2::read::ZlibDecoder::new(&bytes[..]).read_to_string(&mut xml)
        .map_err(|_| "Couldn't unpack that Path of Building code".to_string())?;
    Ok(xml)
}

/// Where to download the code for a share link (None = it's already a code).
pub fn raw_url(link: &str) -> Option<String> {
    let l = link.trim().trim_end_matches('/');
    let id = |prefix: &str| l.split(prefix).nth(1).map(|s| s.split(['/', '?', '#']).next().unwrap_or("").to_string()).filter(|s| !s.is_empty());
    if l.contains("pobb.in/") { return id("pobb.in/").map(|i| format!("https://pobb.in/{i}/raw")); }
    if l.contains("pastebin.com/") {
        let i = l.split("pastebin.com/").nth(1)?.trim_start_matches("raw/").split(['/', '?']).next()?.to_string();
        return Some(format!("https://pastebin.com/raw/{i}"));
    }
    if l.contains("poe.ninja/pob/") { return id("poe.ninja/pob/").map(|i| format!("https://poe.ninja/pob/raw/{i}")); }
    None
}

fn item_gear(slot: &str, text: &str) -> Gear {
    let mut lines: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let mut rarity = "NORMAL".to_string();
    if let Some(r) = lines.first().and_then(|l| l.strip_prefix("Rarity: ")) {
        rarity = r.to_string();
        lines.remove(0);
    }
    let two_names = matches!(rarity.as_str(), "RARE" | "UNIQUE" | "RELIC");
    let name = if lines.is_empty() { String::new() } else { lines.remove(0) };
    let base = if two_names && !lines.is_empty() { lines.remove(0) } else { name.clone() };
    // Uniques with variants: PoB lists every variant's lines, tagged {variant:1,3}; keep the chosen one's.
    let pick = |key: &str| lines.iter().find_map(|l| l.strip_prefix(key)).and_then(|v| v.trim().parse::<u32>().ok());
    let variant = pick("Selected Variant: ");
    // "Selected Alt Variant: 3", "Selected Alt Variant Two: 1"… (Watcher's Eye and other multi-variant uniques).
    let alts: Vec<u32> = lines.iter().filter(|l| l.starts_with("Selected Alt Variant"))
        .filter_map(|l| l.split(": ").nth(1)?.trim().parse().ok()).collect();
    let lines: Vec<String> = lines.iter().filter(|l| {
        let Some(tag) = l.find("{variant:").map(|i| &l[i + 9..]).and_then(|t| t.split('}').next()) else { return true };
        let ids: Vec<u32> = tag.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        variant.is_none_or(|v| ids.contains(&v)) || alts.iter().any(|a| ids.contains(a))
    }).cloned().collect();
    let shown: Vec<String> = lines.iter().filter(|l| !bookkeeping(l)).map(|l| tidy(l)).filter(|l| !l.is_empty()).collect();
    // PoB calls items you made up "New Item": show the base instead.
    let name = if name == "New Item" { base.clone() } else { name };
    let copy = game_copy(&rarity, &name, &base, &lines);
    Gear { slot: slot.to_string(), rarity: rarity.to_lowercase(), name, base, lines: shown, copy }
}

/// A PoB mod line without its {tags}, with "(60-80)" ranges rolled where PoB's {range:0.25} says
/// (the middle when it doesn't).
fn tidy(l: &str) -> String {
    let range = l.find("{range:").and_then(|i| l[i + 7..].split('}').next()).and_then(|r| r.parse::<f64>().ok()).unwrap_or(0.5);
    let mut out = String::new();
    let mut depth = 0;
    for c in l.chars() { match c { '{' => depth += 1, '}' => depth -= 1, _ if depth == 0 => out.push(c), _ => {} } }
    roll_ranges(out.trim(), range)
}

/// PoB-only bookkeeping ("Prefix: …", "Crafted: true", "ArmourBasePercentile: 1", "Has Alt Variant Two: true"…),
/// not something you'd see on the item.
fn bookkeeping(l: &str) -> bool {
    const DROP: [&str; 15] = ["Unique ID", "Item Level", "LevelReq", "Implicits", "Prefix", "Suffix", "Crafted", "Variant",
        "Selected Variant", "Catalyst", "CatalystQuality", "League", "Source", "Talisman Tier", "Intangibility"];
    let Some((key, _)) = l.split_once(": ") else { return false };
    DROP.contains(&key) || key.ends_with("BasePercentile") || key.starts_with("Has Alt Variant") || key.starts_with("Selected Alt Variant")
}

/// A PoB item written like the game's Ctrl+C copy (class, rarity, name, properties, item level, implicits,
/// mods), so the price window reads it the same way as an item hovered in game.
fn game_copy(rarity: &str, name: &str, base: &str, lines: &[String]) -> String {
    let rarity = match rarity { "RARE" => "Rare", "UNIQUE" | "RELIC" => "Unique", "MAGIC" => "Magic", _ => "Normal" };
    // The game's own spelling ("Maelström Staff", PoB writes "Maelstrom Staff") and class. Magic items only have
    // a full name ("Perpetual Sapphire Flask of Craft"): their base is the longest one inside it.
    let fold = |s: &str| s.replace('ö', "o").to_lowercase();
    let classes = crate::item::base_classes();
    let found = classes.get_key_value(base).or_else(|| {
        let b = fold(base);
        classes.iter().filter(|(k, _)| { let k = fold(k); k == b || (!matches!(rarity, "Rare" | "Unique") && b.contains(&k)) })
            .max_by_key(|(k, _)| k.len())
    });
    let (base, class) = found.map(|(b, c)| (b.as_str(), c.as_str())).unwrap_or((base, ""));
    let name = if base.contains('ö') { name.replace(&base.replace('ö', "o"), base) } else { name.to_string() };
    let mut head = vec![format!("Item Class: {class}"), format!("Rarity: {rarity}"), name];
    if matches!(rarity, "Rare" | "Unique") { head.push(base.to_string()); }
    let (mut props, mut sockets, mut ilvl) = (vec![], None, None);
    let (mut implicits, mut mods, mut tail): (Vec<String>, Vec<String>, Vec<String>) = (vec![], vec![], vec![]);
    let mut implicit_left: Option<usize> = None;
    for l in lines {
        if let Some(n) = l.strip_prefix("Implicits: ") { implicit_left = n.trim().parse().ok(); continue; }
        let Some(left) = implicit_left else {
            // Before "Implicits: N": the item's properties.
            if let Some(v) = l.strip_prefix("Item Level: ") { ilvl = Some(v.trim().to_string()); }
            else if let Some(v) = l.strip_prefix("Sockets: ") { sockets = Some(v.trim().to_string()); }
            else if let Some(v) = l.strip_prefix("Quality: ") { if v.trim() != "0" { props.push(format!("Quality: +{}% (augmented)", v.trim())); } }
            // Not "Armour: 96" etc.: in PoB those are the base's roll, not the finished item's number.
            else if ["Limited to: ", "Radius: "].iter().any(|k| l.starts_with(k)) {
                props.push(l.clone());
            } else if l.ends_with(" Item") && !l.contains(": ") { tail.push(l.clone()); }
            continue;
        };
        if bookkeeping(l) { continue; }
        let t = tidy(l);
        if t.is_empty() { continue; }
        if matches!(t.as_str(), "Corrupted" | "Mirrored" | "Split") || (t.ends_with(" Item") && !t.contains(char::is_numeric)) { tail.push(t); continue; }
        // An anoint ("{crafted}Allocates Fleetfoot", counted with the implicits in PoB) is an enchant in game.
        let anoint = left > 0 && l.contains("{crafted}") && t.starts_with("Allocates ");
        let tag = if l.contains("{enchant}") || anoint || (left > 0 && base.ends_with("Cluster Jewel")) { " (enchant)" } else if left > 0 { " (implicit)" } else if l.contains("{crafted}") { " (crafted)" }
            else if l.contains("{fractured}") { " (fractured)" } else { "" };
        if left > 0 { implicit_left = Some(left - 1); implicits.push(format!("{t}{tag}")); } else { mods.push(format!("{t}{tag}")); }
    }
    let mut sections = vec![head.join("\n")];
    if !props.is_empty() { sections.push(props.join("\n")); }
    if let Some(s) = sockets { sections.push(format!("Sockets: {s}")); }
    sections.push(format!("Item Level: {}", ilvl.unwrap_or_else(|| "1".into())));
    if !implicits.is_empty() { sections.push(implicits.join("\n")); }
    if !mods.is_empty() { sections.push(mods.join("\n")); }
    if !tail.is_empty() { sections.push(tail.join("\n")); }
    sections.join("\n--------\n") + "\n"
}

/// A gem written like the game's copy, for the price window.
pub fn gem_copy(g: &Gem) -> String {
    let class = if g.support { "Support Gems" } else { "Skill Gems" };
    let q = if g.quality > 0 { format!("\nQuality: +{}% (augmented)", g.quality) } else { String::new() };
    // Past level 20 or 20% quality only happens by corrupting (PoB guides often list 21/20): search it as corrupted.
    let corrupted = if g.level > 20 || g.quality > 20 { "--------\nCorrupted\n" } else { "" };
    // PoB writes supports without the word the game (and trade site) ends their name with: "Arcane Surge" -> "Arcane Surge Support".
    let name = if g.support && !g.name.ends_with(" Support") { format!("{} Support", g.name) } else { g.name.clone() };
    format!("Item Class: {class}\nRarity: Gem\n{name}\n--------\nLevel: {}{q}\n{corrupted}", g.level)
}

/// "(60-80)% increased Spell Damage" at 0.5 -> "70% increased Spell Damage".
fn roll_ranges(l: &str, at: f64) -> String {
    let mut out = String::new();
    let mut rest = l;
    while let Some(i) = rest.find('(') {
        let Some(j) = rest[i..].find(')').map(|j| i + j) else { break };
        let inner = &rest[i + 1..j];
        // "-" splits the two ends; a leading "-" belongs to the number ("(-10--5)").
        let parsed = inner.char_indices().skip(1).find(|(_, c)| *c == '-')
            .and_then(|(k, _)| Some((inner[..k].parse::<f64>().ok()?, inner[k + 1..].parse::<f64>().ok()?)));
        out.push_str(&rest[..i]);
        match parsed {
            Some((a, b)) => {
                let v = a + (b - a) * at;
                out.push_str(&if inner.contains('.') { format!("{}", (v * 100.0).round() / 100.0) } else { format!("{}", v.round()) });
            }
            None => out.push_str(&rest[i..=j]),
        }
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

fn attr<'a>(n: &roxmltree::Node<'a, 'a>, k: &str) -> &'a str { n.attribute(k).unwrap_or("") }

fn child<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<roxmltree::Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(tag))
}

/// PoB XML -> build with stages (tree, gems, gear), paired by title where they match.
pub fn parse(xml: &str) -> Result<Build, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("Couldn't read the build: {e}"))?;
    let root = doc.root_element();
    let build_el = child(root, "Build").ok_or("Not a Path of Building build")?;

    // Every item by its id (gear sets and tree sockets point at them).
    let by_id: HashMap<String, String> = child(root, "Items").map(|items| items.children().filter(|c| c.has_tag_name("Item"))
        .map(|i| (attr(&i, "id").to_string(), i.text().unwrap_or("").to_string())).collect()).unwrap_or_default();
    let jewel_of = |node: Option<u32>, slot: &str, text: &str| {
        let item = item_gear(slot, text);
        let notables = item.lines.iter().filter_map(|l| l.strip_prefix("1 Added Passive Skill is ").map(String::from)).collect();
        Jewel { node, item, notables }
    };

    // Trees, and the jewels in each tree's sockets.
    let mut trees: Vec<(String, Tree)> = vec![];
    let mut tree_jewels: Vec<Vec<Jewel>> = vec![];
    if let Some(t) = child(root, "Tree") {
        for s in t.children().filter(|c| c.has_tag_name("Spec")) {
            let url = child(s, "URL").and_then(|u| u.text()).unwrap_or("").trim().to_string();
            tree_jewels.push(child(s, "Sockets").map(|so| so.children().filter(|c| c.has_tag_name("Socket"))
                .filter_map(|c| {
                    let node: u32 = attr(&c, "nodeId").parse().ok()?;
                    let text = by_id.get(attr(&c, "itemId"))?;
                    Some(jewel_of(Some(node), &format!("Jewel socket {node}"), text))
                }).collect()).unwrap_or_default());
            trees.push((attr(&s, "title").to_string(), Tree {
                version: attr(&s, "treeVersion").to_string(),
                class_id: attr(&s, "classId").parse().unwrap_or(0),
                ascend_id: attr(&s, "ascendClassId").parse().unwrap_or(0),
                nodes: attr(&s, "nodes").split(',').filter_map(|n| n.trim().parse().ok()).collect(),
                masteries: attr(&s, "masteryEffects").split('}').filter_map(|p| {
                    let (a, b) = p.trim_start_matches([',', '{']).split_once(',')?;
                    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
                }).collect(),
                url,
            }));
        }
    }
    let tree_active = child(root, "Tree").and_then(|t| t.attribute("activeSpec")).and_then(|a| a.parse::<usize>().ok()).unwrap_or(1);

    // Gem setups. Older builds have <Skill> straight under <Skills> (one set).
    let mut skill_sets: Vec<(String, Vec<Group>)> = vec![];
    if let Some(sk) = child(root, "Skills") {
        let read = |n: roxmltree::Node| -> Vec<Group> {
            n.children().filter(|c| c.has_tag_name("Skill")).map(|s| Group {
                label: attr(&s, "label").to_string(),
                slot: attr(&s, "slot").to_string(),
                enabled: attr(&s, "enabled") != "false",
                gems: s.children().filter(|g| g.has_tag_name("Gem")).map(|g| {
                    let id = attr(&g, "gemId");
                    let name = attr(&g, "nameSpec").to_string();
                    let colour = colours().get(id).or_else(|| colours().get(&name)).cloned().unwrap_or_else(|| "w".into());
                    Gem { support: id.contains("/SupportGem") || attr(&g, "skillId").starts_with("Support"),
                        level: attr(&g, "level").parse().unwrap_or(1), quality: attr(&g, "quality").parse().unwrap_or(0),
                        enabled: attr(&g, "enabled") != "false", colour, name }
                }).collect(),
            }).filter(|g: &Group| !g.gems.is_empty()).collect()
        };
        let sets: Vec<roxmltree::Node> = sk.children().filter(|c| c.has_tag_name("SkillSet")).collect();
        if sets.is_empty() { skill_sets.push((String::new(), read(sk))); }
        for s in sets { skill_sets.push((attr(&s, "title").to_string(), read(s))); }
    }

    // Gear.
    let mut item_sets: Vec<(String, Vec<Gear>)> = vec![];
    if let Some(items) = child(root, "Items") {
        let read = |n: roxmltree::Node| -> Vec<Gear> {
            n.children().filter(|c| c.has_tag_name("Slot"))
                .filter(|s| attr(s, "itemId") != "0" && !attr(s, "name").contains("Swap"))
                .filter_map(|s| by_id.get(attr(&s, "itemId")).map(|t| item_gear(attr(&s, "name"), t)))
                .collect()
        };
        let sets: Vec<roxmltree::Node> = items.children().filter(|c| c.has_tag_name("ItemSet")).collect();
        if sets.is_empty() { item_sets.push((String::new(), read(items))); }
        for s in sets { item_sets.push((attr(&s, "title").to_string(), read(s))); }
    }

    // Stages: one per tree (or per gem set if there are more), matched by title, else by position.
    let n = trees.len().max(skill_sets.len()).max(1);
    // PoB's loadouts: sets with the same name, or sharing a tag ("Endgame {2}" with gems "{2}" and gear "Gear {1,2}").
    // By position only when the sets aren't named to match at all (a wrong set is worse than none).
    let tree_titles: Vec<String> = trees.iter().map(|t| t.0.clone()).collect();
    let pick_by = |title: &str, i: usize, titles: &[String]| -> Option<usize> {
        let by_name = |title: &str| titles.iter().position(|t| !t.is_empty() && t == title)
            .or_else(|| { let tags = loadout_tags(title); titles.iter().position(|t| loadout_tags(t).iter().any(|x| tags.contains(x))) })
            .or_else(|| { let c = plain_title(title).to_lowercase(); titles.iter().position(|t| !c.is_empty() && plain_title(t).to_lowercase() == c) });
        if let Some(j) = by_name(title) { return Some(j); }
        let matched = tree_titles.iter().any(|t| by_name(t).is_some());
        // One set not named after any tree: it goes with all of them.
        if titles.len() == 1 && !matched { return Some(0); }
        (!matched || titles.len() == tree_titles.len()).then_some(i).filter(|i| *i < titles.len())
    };
    let skill_titles: Vec<String> = skill_sets.iter().map(|s| s.0.clone()).collect();
    let item_titles: Vec<String> = item_sets.iter().map(|s| s.0.clone()).collect();
    let mut stages = vec![];
    for i in 0..n {
        let (title, tree) = trees.get(i).map(|(t, tr)| (t.clone(), Some(tr.clone())))
            .unwrap_or_else(|| (skill_titles.get(i).cloned().unwrap_or_default(), None));
        let groups = pick_by(&title, i, &skill_titles).map(|j| skill_sets[j].1.clone()).unwrap_or_default();
        let gear: Vec<Gear> = pick_by(&title, i, &item_titles).map(|j| item_sets[j].1.clone()).unwrap_or_default();
        // Abyss jewels sit in items' "… Abyssal Socket N" slots: list them with the other jewels.
        let (abyss, gear): (Vec<Gear>, Vec<Gear>) = gear.into_iter().partition(|g| g.slot.contains("Abyssal Socket"));
        let mut jewels: Vec<Jewel> = tree_jewels.get(i).filter(|_| i < trees.len()).cloned().unwrap_or_default();
        jewels.extend(abyss.into_iter().map(|g| Jewel { node: None, notables: vec![], item: g }));
        let shown = plain_title(&title);
        let title = if shown.is_empty() { format!("Setup {}", i + 1) } else { shown };
        stages.push(Stage { title, tree, groups, gear, jewels });
    }
    Ok(Build {
        class: attr(&build_el, "className").to_string(),
        ascendancy: attr(&build_el, "ascendClassName").to_string(),
        level: attr(&build_el, "level").parse().unwrap_or(1),
        active: tree_active.saturating_sub(1).min(stages.len().saturating_sub(1)),
        stages,
    })
}

/// The loadout tags in a set's title: "Endgame {2}", "Gear {1,2}", "Tree {L3}".
fn loadout_tags(title: &str) -> Vec<String> {
    title.split('{').skip(1).filter_map(|p| p.split_once('}')).flat_map(|(t, _)| t.split(',').map(|x| x.trim().to_lowercase()).collect::<Vec<_>>())
        .filter(|t| !t.is_empty()).collect()
}

/// A set's title without its loadout tags.
fn plain_title(title: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in title.chars() {
        match c { '{' => depth += 1, '}' => depth = (depth - 1i32).max(0), _ if depth == 0 => out.push(c), _ => {} }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Campaign act for an area level (approximate, PoE 1). 11 = maps.
pub fn act_of(area_level: u32) -> u32 {
    match area_level {
        0..=12 => 1, 13..=22 => 2, 23..=32 => 3, 33..=39 => 4, 40..=44 => 5,
        45..=49 => 6, 50..=54 => 7, 55..=60 => 8, 61..=63 => 9, 64..=67 => 10, _ => 11,
    }
}

/// The act a stage's title is for: "Act 2 RF Swap" -> 2, "Start of Maps" -> 11, "Animate Guardian" -> None.
pub fn stage_act(title: &str) -> Option<u32> {
    let t = title.to_lowercase();
    // "Act 3", "Acts 1-3", "act3", "A3" (its first act); whole words, so "Compact" isn't an act.
    let words: Vec<&str> = t.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let num = |w: Option<&&str>| w.and_then(|w| w.parse::<u32>().ok());
    for (i, w) in words.iter().enumerate() {
        let rest = w.strip_prefix("acts").or_else(|| w.strip_prefix("act")).or_else(|| w.strip_prefix('a'));
        let n = match rest {
            Some("") if *w != "a" => num(words.get(i + 1)),
            Some(r) if !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()) => r.parse().ok(),
            _ => None,
        };
        if let Some(n) = n.filter(|n| (1..=10).contains(n)) { return Some(n); }
    }
    if ["map", "endgame", "red ", "yellow", "white map", "t16", "t17"].iter().any(|w| t.contains(w)) { return Some(11); }
    // "Level 12", "Lvl 30", "Levelling 1-30" (from its first level).
    let level = words.iter().position(|w| matches!(*w, "level" | "levels" | "lvl" | "lv" | "levelling" | "leveling"));
    if let Some(i) = level {
        if let Some(n) = num(words.get(i + 1)) { return Some(act_of(n)); }
        if words[i].starts_with("levell") || words[i].starts_with("leveling") { return Some(1); }
    }
    words.iter().any(|w| *w == "campaign").then_some(1)
}

/// The stage to show for where you are: the last one for your act (or earlier),
/// and in maps the first maps stage (it won't jump ahead to a big-budget setup).
pub fn stage_for(stages: &[Stage], area_level: u32) -> Option<usize> {
    let act = act_of(area_level);
    let acts: Vec<Option<u32>> = stages.iter().map(|s| stage_act(&s.title)).collect();
    if act >= 11 {
        return acts.iter().position(|a| *a == Some(11)).or_else(|| acts.iter().rposition(|a| a.is_some()));
    }
    acts.iter().enumerate().filter(|(_, a)| a.map(|a| a <= act).unwrap_or(false)).map(|(i, _)| i).last()
        .or_else(|| acts.iter().position(|a| a.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn stage_names_and_loadout_tags() {
        for (t, a) in [("Act 3", Some(3)), ("Acts 1-3", Some(1)), ("act10", Some(10)), ("A4 twink", Some(4)), ("Compact tree", None), ("Character", None),
            ("Level 12", Some(act_of(12))), ("Lvl 30", Some(act_of(30))), ("Levelling 1-30", Some(1)), ("Leveling", Some(1)), ("Campaign", Some(1)),
            ("Early Mapping", Some(11)), ("Level 90 Endgame", Some(11)), ("Endgame {2}", Some(11)), ("Uber", None)] {
            assert_eq!(stage_act(t), a, "{t}");
        }
        assert_eq!(loadout_tags("Gear {1,L2}"), ["1", "l2"]);
        assert_eq!(plain_title("Endgame {L2}"), "Endgame");
        assert_eq!(plain_title("{1} Acts"), "Acts");
    }

    // A trimmed copy of a real export (an Elementalist levelling guide), two stages.
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
	<Build ascendClassName="Elementalist" className="Witch" level="50" mainSocketGroup="1" targetVersion="3_0"/>
	<Tree activeSpec="2">
		<Spec title="Act 1" treeVersion="3_29" classId="3" ascendClassId="0" nodes="739,1031,1957">
			<URL>https://www.pathofexile.com/passive-skill-tree/AAAABgMADwLjBAc</URL>
			<Sockets>
				<Socket nodeId="7960" itemId="3"/>
				<Socket nodeId="9408" itemId="0"/>
			</Sockets>
		</Spec>
		<Spec title="Act 3" treeVersion="3_29" classId="3" ascendClassId="2" nodes="739,885,1031,1957,4184" masteryEffects="{12125,34242},{63268,55308}">
			<URL>https://www.pathofexile.com/passive-skill-tree/AAAABgMAIgLjA3U</URL>
		</Spec>
	</Tree>
	<Skills activeSkillSet="1">
		<SkillSet id="1" title="Act 1">
			<Skill enabled="true" label="" mainActiveSkill="1">
				<Gem nameSpec="Lightning Tendrils" gemId="Metadata/Items/Gems/SkillGemLightningTendrils" skillId="LightningTendrils" level="1" quality="0" enabled="true"/>
				<Gem nameSpec="Added Lightning Damage" gemId="Metadata/Items/Gems/SupportGemAddedLightningDamage" skillId="SupportAddedLightningDamage" level="1" quality="0" enabled="true"/>
				<Gem nameSpec="Arcane Surge" gemId="Metadata/Items/Gems/SupportGemArcaneSurge" skillId="SupportArcaneSurge" level="1" quality="0" enabled="true"/>
			</Skill>
			<Skill enabled="true" label="" mainActiveSkill="1">
				<Gem nameSpec="Shield Charge" gemId="Metadata/Items/Gems/SkillGemShieldCharge" skillId="ShieldCharge" level="1" quality="0" enabled="true"/>
				<Gem nameSpec="Momentum" gemId="Metadata/Items/Gems/SupportGemOnslaught" skillId="SupportMomentum" level="1" quality="0" enabled="true"/>
			</Skill>
		</SkillSet>
		<SkillSet id="2" title="Act 3">
			<Skill enabled="true" label="Auras" slot="Helmet">
				<Gem nameSpec="Vitality" gemId="Metadata/Items/Gems/SkillGemVitality" skillId="Vitality" level="5" quality="0" enabled="true"/>
			</Skill>
		</SkillSet>
	</Skills>
	<Items activeItemSet="1">
		<Item id="1">
Rarity: RARE
Ghoul Spell
Imperial Staff
Unique ID: 6469839fd0e08d
Shaper Item
Item Level: 84
Crafted: true
Prefix: {range:0.5}GlobalLightningSpellGemsLevel1
Suffix: None
ArmourBasePercentile: 1
Quality: 20
Sockets: B-R-R-R-B-B
LevelReq: 66
Implicits: 1
{crafted}8% increased Explicit Caster Damage Modifier magnitudes
147% increased Spell Damage
{range:0.5}43% increased Cast Speed
		</Item>
		<Item id="2">
Rarity: UNIQUE
Goldrim
Leather Cap
+10% to Item Rarity
		</Item>
		<Item id="3">
Rarity: RARE
8 Passive Only! Not 9!
Large Cluster Jewel
Adds 8 Passive Skills
Added Small Passive Skills grant: 12% increased Lightning Damage
1 Added Passive Skill is Doryani's Lesson
1 Added Passive Skill is Overshock
		</Item>
		<Item id="4">
Rarity: RARE
Bleed + CB Avoidance Jewel
Hypnotic Eye Jewel
Adds 10 to 20 Lightning Damage to Spells
		</Item>
		<ItemSet id="1" title="Act 1">
			<Slot name="Belt Abyssal Socket 1" itemId="4"/>
			<Slot name="Weapon 1" itemId="1"/>
			<Slot name="Weapon 1 Swap" itemId="0"/>
			<Slot name="Helmet" itemId="2"/>
		</ItemSet>
	</Items>
</PathOfBuilding>"#;

    fn encode(xml: &str) -> String {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(xml.as_bytes()).unwrap();
        base64::engine::general_purpose::URL_SAFE.encode(z.finish().unwrap())
    }

    #[test]
    fn reads_a_build_code() {
        let b = parse(&decode(&encode(XML)).unwrap()).unwrap();
        assert_eq!((b.class.as_str(), b.ascendancy.as_str(), b.level, b.active), ("Witch", "Elementalist", 50, 1));
        assert_eq!(b.stages.len(), 2);
        let s = &b.stages[0];
        assert_eq!(s.title, "Act 1");
        assert_eq!(s.tree.as_ref().unwrap().nodes, vec![739, 1031, 1957]);
        let g = &s.groups[0].gems;
        assert_eq!((g[0].name.as_str(), g[0].support, g[0].colour.as_str()), ("Lightning Tendrils", false, "b"));
        assert_eq!((g[2].name.as_str(), g[2].support), ("Arcane Surge", true));
        assert_eq!(s.groups[1].gems[1].colour, "g", "Momentum is green");
        let staff = &s.gear[0];
        assert_eq!((staff.slot.as_str(), staff.rarity.as_str(), staff.name.as_str(), staff.base.as_str()), ("Weapon 1", "rare", "Ghoul Spell", "Imperial Staff"));
        assert!(staff.lines.contains(&"43% increased Cast Speed".to_string()) && staff.lines.contains(&"Sockets: B-R-R-R-B-B".to_string()), "{:?}", staff.lines);
        assert!(!staff.lines.iter().any(|l| l.starts_with("Unique ID") || l.contains('{') || l.starts_with("Prefix") || l.starts_with("Crafted") || l.contains("Percentile")), "{:?}", staff.lines);
        assert!(staff.lines.contains(&"Quality: 20".to_string()));
        assert_eq!(roll_ranges("Adds (10-15) to (20-30) Cold Damage", 1.0), "Adds 15 to 30 Cold Damage");
        assert_eq!(roll_ranges("(-10--5)% to Chaos Resistance (Hidden)", 0.0), "-10% to Chaos Resistance (Hidden)");
        let crank = item_gear("Weapon 1", "Rarity: UNIQUE\nXirgil's Crank\nCoiled Staff\nVariant: Pre 3.0.0\nVariant: Current\nSelected Variant: 2\n\
            {variant:1}+18% Chance to Block Attack Damage while wielding a Staff\n{variant:2}+20% Chance to Block Attack Damage while wielding a Staff\n\
            {range:0.25}(60-80)% increased Spell Damage\n(25-35)% chance for Energy Shield Recharge to start when you Block");
        assert_eq!(crank.lines, ["+20% Chance to Block Attack Damage while wielding a Staff", "65% increased Spell Damage",
            "30% chance for Energy Shield Recharge to start when you Block"]);
        assert_eq!(s.gear.len(), 2, "swap and empty slots skipped, abyss jewel listed with the jewels");
        // Jewels: the tree socket (empty ones skipped) and the abyss jewel.
        assert_eq!(s.jewels.len(), 2);
        assert_eq!((s.jewels[0].node, s.jewels[0].item.base.as_str()), (Some(7960), "Large Cluster Jewel"));
        assert_eq!(s.jewels[0].notables, ["Doryani's Lesson", "Overshock"]);
        assert_eq!((s.jewels[1].node, s.jewels[1].item.slot.as_str()), (None, "Belt Abyssal Socket 1"));
        assert!(b.stages[1].jewels.is_empty());
        assert_eq!(b.stages[1].tree.as_ref().unwrap().masteries, vec![(12125, 34242), (63268, 55308)]);
        // Second stage: its own gems, gear falls back to nothing (only one item set).
        assert_eq!(b.stages[1].groups[0].slot, "Helmet");
        // Codes may arrive with line breaks or padding.
        assert!(decode(&format!(" {}\n", encode(XML))).is_ok());
    }

    #[test]
    fn follows_your_act() {
        let titles = ["Act 1", "Act 2 RF Swap", "Act 3", "Act 4 First Lab", "Act 7 Right Before Staff Swap", "Act 7 Staff Swap",
            "Start of Maps", "End of Early Maps", "Mageblood No Whispers", "Animate Guardian"];
        let st: Vec<Stage> = titles.iter().map(|t| Stage { title: t.to_string(), tree: None, groups: vec![], gear: vec![], jewels: vec![] }).collect();
        let at = |lvl| stage_for(&st, lvl).map(|i| titles[i]);
        assert_eq!(at(5), Some("Act 1"));
        assert_eq!(at(18), Some("Act 2 RF Swap"));
        assert_eq!(at(36), Some("Act 4 First Lab"));
        assert_eq!(at(47), Some("Act 4 First Lab"), "act 6: nothing newer yet");
        assert_eq!(at(52), Some("Act 7 Staff Swap"));
        assert_eq!(at(75), Some("Start of Maps"));
    }

    #[test]
    fn share_links() {
        assert_eq!(raw_url("https://pobb.in/CuCqXO9ftu49").as_deref(), Some("https://pobb.in/CuCqXO9ftu49/raw"));
        assert_eq!(raw_url("pastebin.com/abc123").as_deref(), Some("https://pastebin.com/raw/abc123"));
        assert_eq!(raw_url("eNrsfWmT20ay4OfHX4G"), None);
    }

    #[test]
    fn copy_for_price_window() {
        // From his build: a fractured ring with a crafted mod and an influence-free "Fractured Item" line.
        let ring = item_gear("Ring 1", "Rarity: RARE\nMorbid Twirl\nUnset Ring\nIntangibility: 72%\nUnique ID: 2d58\nCatalyst: Fertile\n\
            CatalystQuality: 20\nItem Level: 83\nSockets: B\nLevelReq: 5\nImplicits: 1\nHas 1 Socket\n+61 to maximum Mana\n\
            {crafted}Channelling Skills have -3 to Total Mana Cost\n{fractured}+3 to Level of Socketed Gems\n\
            {tags:resource,mana}{range:0.5}(70-76)% increased Mana Regeneration Rate\nFractured Item");
        assert_eq!(ring.copy, "Item Class: Rings\nRarity: Rare\nMorbid Twirl\nUnset Ring\n--------\nSockets: B\n--------\nItem Level: 83\n\
            --------\nHas 1 Socket (implicit)\n--------\n+61 to maximum Mana\nChannelling Skills have -3 to Total Mana Cost (crafted)\n\
            +3 to Level of Socketed Gems (fractured)\n73% increased Mana Regeneration Rate\n--------\nFractured Item\n");
        assert!(!ring.lines.iter().any(|l| l.starts_with("Intangibility")));
        let s = crate::trade::setup(&ring.copy, &Default::default()).expect("the price window reads it");
        assert_eq!((s.item.item_class.as_str(), s.item.base_type.as_str(), s.item.rarity.as_str(), s.item.item_level), ("Rings", "Unset Ring", "Rare", Some(83)));
        assert!(s.filters.iter().any(|f| f.kind == "fractured") || s.unknown_kinds.iter().any(|k| k == "fractured"), "fractured mod kept");
        assert_eq!(crate::trade::mods(&ring.copy).len(), 5);

        // Watcher's Eye style: PoB's "Has Alt Variant Two" bookkeeping hidden, both alt variants' mods kept.
        let eye = item_gear("", "Rarity: UNIQUE\nWatcher's Eye\nPrismatic Jewel\nVariant: a\nVariant: b\nVariant: c\nSelected Variant: 1\n\
            Has Alt Variant: true\nSelected Alt Variant: 2\nHas Alt Variant Two: true\nSelected Alt Variant Two: 3\nLimited to: 1\nImplicits: 0\n\
            {range:0.5}(4-6)% increased maximum Energy Shield\n{variant:1}Gain 25 Energy Shield per Enemy Hit while affected by Discipline\n\
            {variant:2}Damage Penetrates 13% Lightning Resistance while affected by Wrath\n{variant:3}+1 to Level of Socketed Gems while affected by Zealotry");
        assert_eq!(eye.lines, ["Limited to: 1", "5% increased maximum Energy Shield", "Gain 25 Energy Shield per Enemy Hit while affected by Discipline",
            "Damage Penetrates 13% Lightning Resistance while affected by Wrath", "+1 to Level of Socketed Gems while affected by Zealotry"]);
        assert!(eye.copy.starts_with("Item Class: Jewels\nRarity: Unique\nWatcher's Eye\nPrismatic Jewel\n--------\nLimited to: 1\n--------\nItem Level: 1\n"), "{}", eye.copy);

        let gem = Gem { name: "Arc".into(), level: 20, quality: 20, enabled: true, support: false, colour: "b".into() };
        let s = crate::trade::setup(&gem_copy(&gem), &Default::default()).unwrap();
        assert_eq!((s.item.base_type.as_str(), s.gem_level, s.item.corrupted), ("Arc", Some(20), false));
        // His Lightning Tendrils of Eccentricity at 21/20: only exists corrupted.
        let g21 = Gem { name: "Lightning Tendrils of Eccentricity".into(), level: 21, quality: 20, enabled: true, support: false, colour: "b".into() };
        let s = crate::trade::setup(&gem_copy(&g21), &Default::default()).unwrap();
        assert_eq!((s.gem_level, s.item.corrupted, s.item.item_level), (Some(21), true, None));
        assert_eq!(crate::trade::query(&s)["query"]["filters"]["misc_filters"]["filters"]["corrupted"]["option"], "true");
        // Supports: PoB's "Arcane Surge" is the trade site's "Arcane Surge Support".
        let sup = Gem { name: "Arcane Surge".into(), level: 20, quality: 20, enabled: true, support: true, colour: "b".into() };
        assert_eq!(crate::trade::setup(&gem_copy(&sup), &Default::default()).unwrap().item.base_type, "Arcane Surge Support");
    }
}
