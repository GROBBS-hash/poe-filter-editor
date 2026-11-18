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
    // Drop PoB-only bookkeeping ("Prefix: …", "Crafted: true", "ArmourBasePercentile: 1"…), keep what you'd see on the item.
    const DROP: [&str; 16] = ["Unique ID", "Item Level", "LevelReq", "Implicits", "Prefix", "Suffix", "Crafted", "Variant",
        "Selected Variant", "Has Alt Variant", "Selected Alt Variant", "Catalyst", "CatalystQuality", "League", "Source", "Talisman Tier"];
    let lines = lines.into_iter()
        .filter(|l| {
            let key = l.split(": ").next().unwrap_or("");
            !(l.contains(": ") && (DROP.contains(&key) || key.ends_with("BasePercentile")))
        })
        .map(|l| {
            let mut out = String::new();
            let mut depth = 0;
            for c in l.chars() { match c { '{' => depth += 1, '}' => depth -= 1, _ if depth == 0 => out.push(c), _ => {} } }
            out.trim().to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    // PoB calls items you made up "New Item": show the base instead.
    let name = if name == "New Item" { base.clone() } else { name };
    Gear { slot: slot.to_string(), rarity: rarity.to_lowercase(), name, base, lines }
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

    // Trees.
    let mut trees: Vec<(String, Tree)> = vec![];
    if let Some(t) = child(root, "Tree") {
        for s in t.children().filter(|c| c.has_tag_name("Spec")) {
            let url = child(s, "URL").and_then(|u| u.text()).unwrap_or("").trim().to_string();
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
        let by_id: HashMap<&str, &str> = items.children().filter(|c| c.has_tag_name("Item"))
            .map(|i| (attr(&i, "id"), i.text().unwrap_or(""))).collect();
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
    let pick_by = |title: &str, i: usize, titles: &[String]| -> Option<usize> {
        titles.iter().position(|t| !t.is_empty() && t == title).or((i < titles.len()).then_some(i))
    };
    let skill_titles: Vec<String> = skill_sets.iter().map(|s| s.0.clone()).collect();
    let item_titles: Vec<String> = item_sets.iter().map(|s| s.0.clone()).collect();
    let mut stages = vec![];
    for i in 0..n {
        let (title, tree) = trees.get(i).map(|(t, tr)| (t.clone(), Some(tr.clone())))
            .unwrap_or_else(|| (skill_titles.get(i).cloned().unwrap_or_default(), None));
        let groups = pick_by(&title, i, &skill_titles).map(|j| skill_sets[j].1.clone()).unwrap_or_default();
        let gear = pick_by(&title, i, &item_titles).map(|j| item_sets[j].1.clone()).unwrap_or_default();
        let title = if title.is_empty() { format!("Setup {}", i + 1) } else { title };
        stages.push(Stage { title, tree, groups, gear });
    }
    Ok(Build {
        class: attr(&build_el, "className").to_string(),
        ascendancy: attr(&build_el, "ascendClassName").to_string(),
        level: attr(&build_el, "level").parse().unwrap_or(1),
        active: tree_active.saturating_sub(1).min(stages.len().saturating_sub(1)),
        stages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // A trimmed copy of a real export (an Elementalist levelling guide), two stages.
    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<PathOfBuilding>
	<Build ascendClassName="Elementalist" className="Witch" level="50" mainSocketGroup="1" targetVersion="3_0"/>
	<Tree activeSpec="2">
		<Spec title="Act 1" treeVersion="3_29" classId="3" ascendClassId="0" nodes="739,1031,1957">
			<URL>https://www.pathofexile.com/passive-skill-tree/AAAABgMADwLjBAc</URL>
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
		<ItemSet id="1" title="Act 1">
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
        assert_eq!(s.gear.len(), 2, "swap and empty slots skipped");
        assert_eq!(b.stages[1].tree.as_ref().unwrap().masteries, vec![(12125, 34242), (63268, 55308)]);
        // Second stage: its own gems, gear falls back to nothing (only one item set).
        assert_eq!(b.stages[1].groups[0].slot, "Helmet");
        // Codes may arrive with line breaks or padding.
        assert!(decode(&format!(" {}\n", encode(XML))).is_ok());
    }

    #[test]
    fn share_links() {
        assert_eq!(raw_url("https://pobb.in/CuCqXO9ftu49").as_deref(), Some("https://pobb.in/CuCqXO9ftu49/raw"));
        assert_eq!(raw_url("pastebin.com/abc123").as_deref(), Some("https://pastebin.com/raw/abc123"));
        assert_eq!(raw_url("eNrsfWmT20ay4OfHX4G"), None);
    }
}
