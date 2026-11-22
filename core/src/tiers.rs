//! NeverSink's tier lists (currency, div cards, uniques...) for the Tiers tab:
//! like FilterBlade, each section is a stack of tiers you can show/hide,
//! restyle, and drag items between.

use crate::evaluate::{style_of, Style};
use crate::filter::Segment;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TierRow {
    pub tier: String,
    /// "Tier 1", "Tier 2 · about a Divine Orb", "Exotic: hidden"...
    pub name: String,
    pub visibility: String,
    pub style: Style,
    pub bases: Vec<String>,
    /// Extra conditions in plain words, e.g. "stacks of 3+".
    pub note: String,
    /// "StackSize >= N" tiers (stacks, gold): N, so the app can offer a slider.
    pub min_stack: Option<u32>,
    /// "UnidentifiedItemTier >= N" (3.30 item tiers on rare gear).
    #[serde(default)]
    pub min_item_tier: Option<u32>,
    /// The step's conditions as chips (item level, corrupted, mods…), as they'll be in game.
    #[serde(default)]
    pub conds: Vec<crate::conds::Cond>,
    /// Which of them you changed (keys), so the page can mark them and offer "Back to NeverSink's".
    #[serde(default)]
    pub changed: Vec<String>,
}

/// Marks the conditions you changed on each step (from your tier rules).
pub fn mark_changed(sections: &mut [Section], rules: &[crate::rules::Rule]) {
    for s in sections.iter_mut() {
        for t in s.tiers.iter_mut() {
            t.changed = rules.iter().filter_map(|r| match r {
                crate::rules::Rule::Tier { type_path, tier, conds: Some(c), .. } if *type_path == s.type_path && *tier == t.tier => Some(c.keys().cloned().collect::<Vec<_>>()),
                _ => None,
            }).flatten().collect();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    /// e.g. ["currency", "stackedthree"].
    pub type_path: Vec<String>,
    pub title: String,
    pub group: String,
    pub tiers: Vec<TierRow>,
    /// The same base sits in several tiers (a grid, like rare gear by item tier):
    /// dragging would pull it out of the others, so the app doesn't offer it.
    #[serde(default)]
    pub shared_bases: bool,
}

/// The group of levelling rules (the campaign, before maps): flasks, weapon upgrades, links, magic items…
/// Many aren't item lists, just show / hide and a look per step; they're listed anyway, with plain names.
pub const LEVELLING: &str = "Before maps";

/// NeverSink's step names in the levelling rules, in plain words ("r03" -> "Step 3", "earlybodyarmours" -> …).
fn levelling_tier_name(t: &str) -> Option<String> {
    let step = |prefix: char| t.strip_prefix(prefix).and_then(|n| n.trim_end_matches("bas").parse::<u32>().ok()).map(|n| format!("Step {n}"));
    if let Some(s) = step('r').or_else(|| step('w')).or_else(|| step('l')) { return Some(s); }
    Some(match t {
        "earlybodyarmours" => "Early body armours", "threesocketedgear" => "Gear with 3 sockets", "firstareas" => "Anything in the first areas",
        "casterweapons" => "Caster weapons", "castercraftrings" => "Rings to craft (casters)", "general" => "General", "boots" => "Boots",
        "quivers" => "Quivers", "physical" => "Physical (attack) bases", "jewellery" => "Jewellery",
        "largemagicblocker" => "Big magic items", "mediummagicblocker" => "Medium magic items", "rest" => "Everything else",
        "act1" => "Act 1", "act2" => "Act 2", "untilmudflats" => "Until the Mud Flats", "firstlevels" => "First levels",
        "quicksilver" => "Quicksilver", "asortedutility" => "Other utility flasks",
        "rgblarge" => "Big R-G-B items", "rgbmedium" => "Medium R-G-B items", "rgbsmall1" | "rgbsmall2" => "Small R-G-B items",
        "essences" => "Essences", "t2chance" => "Orb of Alteration", "t3transearly" => "Transmutation (early acts)",
        "t3trans" => "Transmutation", "t4misc" => "Scraps and whetstones", "t5aug" => "Augmentation", "portal" => "Portal scrolls",
        "wisdom" => "Wisdom scrolls",
        // More levelling rules (23 Nov).
        "earlylevels" => "First levels", "bootsfocus" => "Boots first", "bodyarmours" => "Body armours",
        "shields" => "Shields", "movementbootshigh" => "Movement speed boots", "movementbootslow" => "Movement speed boots (early)",
        "anyveiled" => "Anything with a veiled mod", "4linkrares" => "Rares with 4 links", "earlythreelinks" => "3-links (early)",
        "miniongear" => "Minion gear", "highphysquivers" => "Physical damage quivers", "fireresistgear" => "Fire resistance rings",
        "generalcrafting" => "Bases to craft", "max" => "20% quality", "largerares" => "Big rares", "mediumrares1" | "mediumrares2" => "Medium rares",
        "tinyrares" => "Small rares", "fatedgear" => "Fated gear", "medium1" | "medium2" => "Medium magic items", "tiny" => "Small magic items",
        "noticeearly" => "Magic items in the first areas", "underlevel68" => "Rares, item level up to 68",
        "underlevel42" => "Rares, item level up to 44", "underlevel24" => "Rares, item level up to 26", "underlevel16" => "Rares, item level up to 18",
        _ => return None,
    }.to_string())
}


/// Known sections: (type path, group, title). Order here is the order shown.
const KNOWN: &[(&str, &str, &str)] = &[
    ("currency", "Currency", "Currency"),
    ("currency->stackedthree", "Currency", "Stacks of 3+"),
    ("currency->stackedsix", "Currency", "Stacks of 6+"),
    ("currency->stackedsupplieswisdom", "Currency", "Wisdom scroll stacks"),
    ("currency->stackedsuppliesportal", "Currency", "Portal scroll stacks"),
    // Transmutation / augmentation stacks are left out (he found them not worth a menu entry).
    ("currency->leveling", LEVELLING, "Campaign currency"),
    ("currency->levelingstacked", LEVELLING, "Campaign currency stacks"),
    ("leveling->flasks->life", LEVELLING, "Life flasks"),
    ("leveling->flasks->mana", LEVELLING, "Mana flasks"),
    ("leveling->flasks->hybrid", LEVELLING, "Hybrid flasks"),
    ("leveling->flasks->utility", LEVELLING, "Utility flasks"),
    ("leveling->wandprogression", LEVELLING, "Wand upgrades"),
    ("leveling->weaponprogression", LEVELLING, "Weapon upgrades"),
    ("leveling->firstlevels", LEVELLING, "First areas"),
    ("leveling->normalmagic->act1", LEVELLING, "Act 1 gear"),
    ("leveling->normalmagic->rgb", LEVELLING, "R-G-B linked items"),
    ("leveling->normalmagic->4l", LEVELLING, "4-linked items"),
    ("leveling->magic->remaining", LEVELLING, "Other magic items"),
    ("leveling->normalmagic->3l", LEVELLING, "3-linked items"),
    ("leveling->normalmagic->act2", LEVELLING, "Act 2 gear"),
    ("leveling->normalmagic->otheracts", LEVELLING, "Later acts gear"),
    ("leveling->normalmagic->minion", LEVELLING, "Minion gear"),
    ("leveling->flasks->quality", LEVELLING, "Quality flasks"),
    ("leveling->tincture", LEVELLING, "Tinctures while levelling"),
    ("leveling->rare->socketslinks", LEVELLING, "Rare 4-links"),
    ("leveling->rare->exotics", LEVELLING, "Movement boots and veiled"),
    ("leveling->rare->universal", LEVELLING, "Rare jewellery"),
    ("leveling->rare->armours", LEVELLING, "Rare armour"),
    ("leveling->rare->minion", LEVELLING, "Rare minion gear"),
    ("leveling->rare->caster", LEVELLING, "Rare caster weapons"),
    ("leveling->rare->archer", LEVELLING, "Rare bows"),
    ("leveling->rare->melee1h", LEVELLING, "Rare one-hand weapons"),
    ("leveling->rare->melee2h", LEVELLING, "Rare two-hand weapons"),
    ("leveling->rare->remaining", LEVELLING, "Other rares"),
    ("decorators->leveling->rare", LEVELLING, "Rares by size"),
    ("decorators->leveling->magic", LEVELLING, "Magic items by size"),
    ("animatedweapons", LEVELLING, "Animate Weapon bases"),
    ("divination", "Divination cards", "Divination cards"),
    ("uniques", "Uniques", "Uniques"),
    ("uniques->replicas", "Uniques", "Replica uniques"),
    ("uniques->foulborn", "Uniques", "Foulborn uniques"),
    ("uniques->maps", "Uniques", "Unique maps"),
    ("fragments", "Fragments", "Fragments"),
    ("currency->splinter->simulacrum", "Fragments", "Simulacrum splinters"),
    ("currency->splinter", "Fragments", "Timeless splinters"),
    ("currency->stackedsplintershigh", "Fragments", "Splinter stacks (high)"),
    ("currency->stackedsplinterslow", "Fragments", "Splinter stacks (low)"),
    ("currency->essence", "League currency", "Essences"),
    ("currency->fossil", "League currency", "Fossils"),
    ("currency->oil", "League currency", "Oils"),
    ("currency->deliriumorbs", "League currency", "Delirium orbs"),
    ("currency->trial->omen", "League currency", "Omens"),
    ("currency->trial->tattoo", "League currency", "Tattoos"),
    ("currency->harvest", "League currency", "Lifeforce"),
    ("currency->deadmansulphur", "League currency", "Sulphur"),
    ("runesgrafts", "League currency", "Runegrafts"),
    ("corpses", "League currency", "Corpses"),
    ("viscera", "League currency", "Viscera"),
    ("currency->ducats", "League currency", "Ducats"),
    ("exotic->wombgifts", "League currency", "Wombgifts"),
    ("mapenchantments", "League currency", "Allflame embers"),
    ("currency->others", "League currency", "Other currency"),
    ("currency->leagueexclusive", "League currency", "League-only currency"),
    // Flasks and gems in maps (23 Nov: every NeverSink rule on the page).
    ("endgameflasks", "Flasks and gems", "Flasks"),
    ("endgametinctures", "Flasks and gems", "Tinctures"),
    ("gems->exceptional", "Flasks and gems", "Exceptional gems"),
    ("gems->special", "Flasks and gems", "Quality and level gems"),
    ("gems->generic", "Flasks and gems", "Other gems"),
    // Heist.
    ("heist->contract", "Heist", "Contracts"),
    ("heist->blueprint", "Heist", "Blueprints"),
    ("heist->cloak", "Heist", "Cloaks"),
    ("heist->brooch", "Heist", "Brooches"),
    ("heist->gear", "Heist", "Heist gear"),
    ("heist->tool", "Heist", "Tools"),
    ("currency->heist", "Heist", "Rogue's Markers"),
    ("heisttarget", "Heist", "Heist targets"),
    ("uniques->heist", "Heist", "Unique contracts and blueprints"),
    // Other league items.
    ("event->idols", "League items", "Idols"),
    ("expedition->logbook", "League items", "Expedition logbooks"),
    ("artefact->misc", "League items", "Voidstones, trinkets and other"),
    ("artefact->sanctifiedrelics", "League items", "Sanctum relics"),
    ("exoticmap->sanctum", "League items", "Sanctum research"),
    ("charts", "League items", "Charts"),
    ("miscmapitemsextra", "League items", "Reliquary keys and league items"),
    ("questlike", "League items", "Invitations and Labyrinth items"),
    ("enshrouded", "League items", "Enshrouded gear"),
    ("gold", "Currency", "Gold"),
    ("gear->tiered", "Gear", "Rare gear by item tier"),
    // The rest of NeverSink's gear rules (23 Nov: "revise every section, his is very thorough"), in his order.
    ("6l", "Gear", "6-linked items"),
    ("influenced->all", "Gear", "Influenced gear"),
    ("rare->exarch", "Gear", "Searing Exarch implicits"),
    ("rare->eater", "Gear", "Eater of Worlds implicits"),
    ("exoticmods", "Gear", "Special mods (veiled, Incursion…)"),
    ("crafting->qualityperfection", "Gear", "High-quality bases"),
    ("crafting->generalgear", "Gear", "Crafting bases"),
    ("exotic->corruptions", "Gear", "Corrupted gear"),
    ("exotic->enchanted", "Gear", "Enchanted and anointed"),
    ("socketslinks", "Gear", "Sockets and links"),
    ("exotic->fractured", "Gear", "Fractured bases"),
    ("rare->exotic->veiled", "Gear", "Veiled items"),
    ("exoticbases", "Gear", "Exotic bases"),
    ("rr", "Gear", "Rare gear by base"),
    ("rr->amuring", "Gear", "Rare rings and amulets"),
    ("rr->belts", "Gear", "Rare belts"),
    ("rareid", "Gear", "Identified rares with good mods"),
    ("rareblendid", "Gear", "Identified rares by base"),
    ("corruptedid", "Gear", "Identified corrupted gear"),
    ("magicid", "Gear", "Identified magic items with good mods"),
    ("gear->memorystrand", "Gear", "Memory strand gear"),
    ("decorators->rareeg", "Gear", "Rare gear by size"),
    ("exotic->crucible", "Gear", "Crucible gear"),
    ("exotic->sockets", "Gear", "Abyss socket gear"),
    ("chancing", "Gear", "Bases to chance"),
    ("normalcraft->extra", "Gear", "White bases to craft"),
    ("endgamergb", "Gear", "R-G-B items in maps"),
    // Jewels (21 Nov: he looked for the jewel rules here and couldn't find them).
    ("jewels->generic", "Jewels", "Jewels"),
    ("jewels->abyss", "Jewels", "Abyss jewels"),
    ("jewels->cluster", "Jewels", "Cluster jewels"),
    ("jewels->clustereco", "Jewels", "Cluster jewels (cheap)"),
    ("jewels->special", "Jewels", "Special jewels"),
    ("jewels->leveling", LEVELLING, "Jewels while levelling"),
];

/// "t2divine" -> "Tier 2 (divine)", "exhide" -> "Hidden extras".
pub fn tier_name(t: &str) -> String {
    if let Some(n) = map_rule_name(t) { return n; }
    // "t5r1": item tier 5 and up, on the best (rank 1) bases.
    if let Some(rest) = t.strip_prefix('t') {
        if let Some((a, b)) = rest.split_once('r') {
            if let (Ok(tier), Ok(rank)) = (a.parse::<u32>(), b.parse::<u32>()) {
                let r = match rank { 1 => "top bases", 2 => "good bases", 3 => "decent bases", _ => "other bases" };
                return format!("Item tier {tier}+ · {r}");
            }
        }
    }
    if let Some(n) = special_tier_name(t) { return n; }
    // Cheap cluster jewels: "n12_i84_t1" = 12 passives, item level 84+, NeverSink's tier 1.
    if let Some(rest) = t.strip_prefix('n') {
        let parts: Vec<&str> = rest.split('_').collect();
        if let [n, i, tier] = parts.as_slice() {
            if let (Ok(n), Some(i), Some(tier)) = (n.parse::<u32>(), i.strip_prefix('i'), tier.strip_prefix('t')) {
                let _ = i; // the item level shows beside the name (the rule's note)
                return format!("Tier {tier} · {n} passives");
            }
        }
    }
    match t {
        "strandedfractured" => return "Fractured, memory-stranded".into(),
        "strandedveiled" => return "Veiled, memory-stranded".into(),
        "exhide" => return "Hidden extras".into(),
        "tportal" => return "Portal scrolls".into(),
        "twisdom" => return "Wisdom scrolls".into(),
        "scrollfragments" => return "Scroll fragments".into(),
        "anyother" | "any" => return "Any other".into(),
        _ => {}
    }
    if let Some(rest) = t.strip_prefix('t') {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let tail = rest[digits.len()..].trim_start_matches('_');
            // What NeverSink's word after the number means: the currency each tier is worth about, or what's special.
            let tail = match tail {
                "exalted" => "worth an Exalted Orb or more", "divine" => "about a Divine Orb", "annul" => "about an Orb of Annulment",
                "chaos" => "about a Chaos Orb", "alchemy" => "about an Orb of Alchemy", "chrom" => "Chromatic Orbs",
                "chance" => "Orbs of Chance", "trans" => "Orbs of Transmutation", "armour" => "Armourer's Scraps and the like",
                "c" => "currency cards", "boss" => "boss drops", "stackedex" => "stacks of 3+", "lifeforce" | "divination" => "",
                other => other,
            };
            return if tail.is_empty() { format!("Tier {digits}") } else { format!("Tier {digits} · {tail}") };
        }
    }
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// NeverSink's one-word tier names that don't follow "t1, t2…", in plain words (checked against what each rule
/// catches in the filter, 21 Nov).
fn special_tier_name(t: &str) -> Option<String> {
    Some(match t {
        // Safety net in most sections: anything of that kind NeverSink hasn't tiered yet (new league items), shown loudly.
        "restex" => "New items, not tiered yet",
        // Divination cards.
        "excustomstack" => "Special: stacks of The Fortunate",
        // Uniques.
        "exuberimpresence" => "Special: Impresence with Shaper and Elder", "exkaom" => "Special: Kaom's Heart",
        "extabula" => "Special: 6-linked Tabula Rasa", "exforgesword" => "Special: influenced Infernal Sword",
        "exrationaljewel" => "Special: synthesised unique Cobalt Jewel", "exsynth" => "Special: synthesised unique rings",
        "ex6link" => "6-linked uniques", "4xabysshelmet" => "Special: 4 abyss sockets", "3xabyss" => "Special: 3 abyss sockets",
        "2xabyss" => "2 abyss sockets", "vestige" => "Vestigial uniques", "exuniqueidols" => "Unique idols",
        "2xcorrupteduniques" => "Twice-corrupted uniques", "excrucibleunique" => "Crucible uniques",
        "multispecialhigh" => "Could be a pricey one (several uniques share the base)",
        "multispecial" | "multi" => "Could be worth something (several uniques share the base)",
        "overqual" => "Over 20% quality", "5link" => "5-linked uniques", "earlyleague" => "Good early in the league",
        "exjewelscorrupted" => "Corrupted unique jewels", "exjewels" => "Unique jewels",
        "hideable2" => "Cheap", "hideable" => "Cheapest",
        // Fractured and exotic bases.
        "fractt1" => "Best fractured bases", "fractt2" => "Other fractured bases",
        "exoticstiered" => "Any exotic base, item tier 5+", "exoticsmemory" => "Any exotic base, memory strands 70+",
        "exoticsfract" => "Any fractured exotic base", "stygianmirage" => "Stygian Vise with an abyss mod",
        // Their item level shows beside the name (the rule's note), so the names don't repeat it.
        "stygian86" => "Stygian Vise, high item level",
        "v86" | "v85" | "v84" | "v83" => "High item level bases",
        // Jewels.
        "anyrare" => "Rare jewels", "anymagic" => "White and magic jewels",
        "optimal1highlarge" => "Large, 12 passives", "optimalhighlarge" => "Large, 8 passives (best item level)",
        "highlarge" => "Large, 8 passives", "large" => "Other large",
        "optimalhighmedium" => "Medium, up to 5 passives (best item level)", "highmedium" => "Medium, up to 5 passives",
        "medium" => "Other medium", "highsmall" => "Small (best item level)", "small" => "Other small",
        "extramodcorruptedabyssjewel" => "Corrupted rare, 5+ mods", "corruptedabyssjewel" => "Corrupted rare",
        "veryhighrare" => "Rare, very high item level", "veryhighmagic" => "White or magic, very high item level",
        "highrare" => "Rare, high item level", "highmagic" => "White or magic, high item level",
        "abyss" => "Abyss jewels while levelling", "generic" => "Jewels while levelling",
        "1modcorrupted" => "Corrupted, with a corruption mod", "1moduncorrupted" => "With a top mod (Shimmering, Vivid…)",
        _ => return None,
    }.to_string())
}

/// Step names in the gear sections, in plain words (NeverSink's own words only make sense to him). `key` is the
/// section ("influenced->all"), checked against what each rule catches (23 Nov).
fn gear_tier_name(key: &str, t: &str) -> Option<String> {
    let inf = |s: &str| { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
    Some(match key {
        "influenced->all" => {
            if let Some(i) = t.strip_prefix("t1bases") { return Some(format!("Tier 1 · best {} bases", inf(i))); }
            if t == "t2classesinfluencedshared" { return Some("Tier 2 · item types any influence wants".into()); }
            if let Some(i) = t.strip_prefix("t2classes") { return Some(format!("Tier 2 · item types {} wants", inf(i))); }
            match t { "t1exotic" => "Tier 1 · exotic bases", "t1top" => "Tier 1 · top bases", "t2high" => "Tier 2 · good bases, high item level",
                "any86" => "Any influenced, item level 86+", "any" => "Any other influenced", _ => return None }
        }
        "rare->exarch" | "rare->eater" => match t { "anyhigh" => "Good bases with the implicit", "any" => "Any with the implicit", _ => return None },
        "6l" => match t { "hightier" => "Good bases", "others" => "Any other 6-link", _ => return None },
        "crafting->generalgear" => {
            let (tier, lvl) = t.strip_prefix('t')?.split_once('_')?;
            return Some(format!("Tier {tier} bases, item level {lvl}+"));
        }
        "crafting->qualityperfection" => match t {
            "qualchancing" => "Bases to chance (high quality)", "armors1q1" => "Top armour bases, best quality", "armors1q2" => "Top armour bases, high quality",
            "weapont1q1" => "Top weapons, best quality", "weapont2q1" => "Good weapons, best quality", "weapont1q2" => "Top weapons, high quality",
            "weapont2q2" => "Good weapons, high quality", "anyoverqualityperf" => "Anything with very high quality", "anyoverqualityhigh" => "Anything with high quality",
            _ => return None },
        "socketslinks" => match t {
            "5linksleveling" => "5-links while levelling", "5link6sockets" => "5-link with 6 sockets", "5links" => "5-links in maps",
            "rgbsmall1" | "rgbsmall2" => "Small R-G-B items (vendor for a Chromatic Orb)", "rgbmedium" => "Medium R-G-B items", "rgblarge" => "Big R-G-B items",
            "6sockets4h" => "6 sockets, small items", "6sockets" => "6 sockets (vendor for 7 Jeweller's Orbs)", _ => return None },
        "exotic->corruptions" => match t {
            "doublecorruptedjwlry" => "Twice-corrupted jewellery", "doublecorruptedany" => "Twice-corrupted rares",
            "singlecorruptedquivers" => "Corrupted quivers", "singlecorrupteditems" => "Corrupted rings", _ => return None },
        "exotic->enchanted" => match t { "exotic" => "Enchanted gear", "anointedamulets" => "Anointed amulets", "anointedrings" => "Anointed rings", _ => return None },
        "exoticmods" => match t {
            "fracturedveiled" => "Fractured, with a veiled mod", "fracturedincursion" => "Fractured, with an Incursion mod", "duoveil" => "Two veiled mods",
            "exoticveiled" => "Veiled jewels", "incursionspeedtraps" => "Incursion: speed and trap boots", "incursioncaster" => "Incursion: caster mods",
            "incursionelemental" => "Incursion: elemental mods", "incursionjwlry" => "Incursion: jewellery", "incursionglovehelm" => "Incursion: gloves and helmets",
            "incursionattack" => "Incursion: attack mods", "incursionlifechest" => "Incursion: life body armours", "incursionminion" => "Incursion: minion mods",
            "incursionmagic" => "Incursion: magic items", "incursionrandom" => "Incursion: any other", "necropoliscraft" => "Necropolis crafted mods",
            "bestiaryvaluable" => "Beast mods (valuable)", "bestiaryother" => "Beast mods (other)", "delvefractured" => "Delve mods, fractured",
            "delve" => "Delve mods", "mercenaryothers" => "Mercenary mods", "mercenaryweapons" => "Mercenary weapons", "warband" => "Warband mods",
            "essence" => "Essence-only mods", "crafting" => "Crafted mods", _ => return None },
        "exotic->fractured" => match t { "fractt3" => "Lower fractured bases", "fractspecial" => "Fractured, special item types",
            "fractothers" => "Any other fractured", _ => return None },
        "rareid" | "corruptedid" | "magicid" => return Some(mods_step_name(t)),
        "rareblendid" => match t {
            "wcastermin" => "Caster weapons, 3+ good mods", "wsmallarmorsblend" => "Boots, gloves and helmets, 3+ good mods",
            "wringsblend" => "Rings, 3+ good mods", "wamuletsblend" => "Amulets, 3+ good mods", "wbeltsblend" => "Belts, 3+ good mods",
            "weleattackblend" => "Elemental attack weapons, 3+ good mods",
            _ => { let n: u32 = t.strip_prefix('t')?.parse().ok()?; return Some(format!("Tier {n} bases")); } },
        "uniques" => match t { "6s" => "6-socket uniques", "corrupteduniques" => "Corrupted uniques with a corruption mod", _ => return None },
        "divination" => match t { "exstack" => "Special: stacks of cheap cards", _ => return None },
        "gear->tiered" => match t { "any5" | "any4" => "Any item type, high item tier", "specialclasses" => "Special item types", _ => return None },
        "rr" if t.starts_with("fated") => match t { "fatedspecial" => "Fated, special item types", _ => "Fated gear" },
        "gear->memorystrand" => match t {
            "t1high" => "Tier 1 bases, many memory strands", "t1" => "Tier 1 bases", "t2high" => "Tier 2 bases, many memory strands", "t2" => "Tier 2 bases",
            "t3high" => "Tier 3 bases, many memory strands", "t3" => "Tier 3 bases", "anyhigh" => "Any base, many memory strands", "any" => "Any other", _ => return None },
        "decorators->rareeg" => match t {
            "largerares" => "Big rares", "mediumrares1" | "mediumrares2" => "Medium rares", "tinyrares" => "Small rares (rings, amulets…)",
            "fourlinkedrares" => "Rares with 4+ links", "topilvl83" | "topilvl84" | "topilvl85" | "topilvl86" => "Top item level for the item type",
            "corruptedrares" => "Corrupted rares", "corruptedraresimplicit" => "Corrupted rares with a corruption mod", "fatedgear" => "Fated gear", _ => return None },
        "exotic->crucible" => "Gear with a Crucible tree",
        "exotic->sockets" => "Gear with an abyss socket",
        "endgameflasks" => match t {
            "overqualcorrupted" => "Corrupted, 30% quality", "overqualutil1" => "Utility, very high quality", "overqualutil2" => "Utility, high quality",
            "overquallife1" => "Life/mana, very high quality", "overquallife2" => "Life/mana, high quality", "utility85" | "utility84" | "utility82" => "Utility, high item level",
            "lifemana82" => "Life/mana, high item level", "any20qualflask" => "Any white flask with 20% quality", "earlymappingflasks" => "Utility flasks (early maps)",
            "anyutility" => "Any utility flask", "earlymappinglifemana" => "Life/mana flasks (early maps)", _ => return None },
        "endgametinctures" => match t {
            "overqual1" => "Very high quality", "overqual2" => "High quality", "tinc85" | "tinc82" => "High item level", "any" => "Any other", _ => return None },
        "gems->exceptional" => match t {
            "highexceptionalbadcorrupt" => "Corrupted, low quality", "t1" => "Tier 1", "t2" => "Tier 2", "t3" => "Tier 3", "t4" => "Tier 4",
            "anyexceptionallvl" => "Any exceptional, high level", "anyexceptionalqual" => "Any exceptional, with quality", _ => return None },
        "gems->special" => match t {
            "altany" => "Any transfigured gem", "2020z" => "Level 20, 20% quality", "xx20z" => "Valuable, 20% quality", "2120" => "Level 21, 20% quality",
            "2023" => "Level 20, 23% quality", "2123any" => "Level 21, 23% quality", "xx23" => "Valuable, 23% quality", "21xx" => "Level 21",
            "vaal20" => "Vaal, 20% quality", _ => return None },
        "gems->generic" => match t {
            "lt1" => "Level tier 1", "lt2" => "Level tier 2", "lt3" => "Level tier 3", "qt1" => "Quality tier 1", "qt2" => "Quality tier 2",
            "qt3" => "Quality tier 3", "qt4" => "Some quality", "qt4lvl" => "Some quality (levelling)", "ex6lvlgems" => "Special gems",
            "decogemsvaalbgdrop" => "Vaal gems", "firstzone" => "First area gems", "levelingvaal" => "Vaal gems (levelling)", "leveling" => "Gems while levelling",
            "corruptedvaalany" => "Corrupted Vaal gems", "any" => "Any other", _ => return None },
        "heist->cloak" | "heist->brooch" | "heist->gear" | "heist->tool" => match t {
            "t1highlevel" => "Tier 1, high item level", "t1" => "Tier 1", "t2" => "Tier 2", "t3any" => "Any other", _ => return None },
        "heist->contract" | "heist->blueprint" => match t { "enchanted" => "Enchanted", "handpicked" => "Hand-picked targets", "any" => "Any other", _ => return None },
        "currency->heist" => match t { "highstack" => "Big stacks", _ => return None },
        "currency->others" => match t { "mischigh" => "Valuable", "misclow" => "Cheap", _ => return None },
        "currency->leagueexclusive" => match t { "silvercoin" => "Silver Coins", _ => return None },
        "uniques->maps" => match t { "multispecialcorrupted" => "Corrupted unique maps", _ => return None },
        "event->idols" => {
            let (kind, rar) = if let Some(k) = t.strip_suffix("rare") { (k, "rare") } else if let Some(k) = t.strip_suffix("magic") { (k, "magic") }
                else { return Some(if t == "normalidols" { "White idols".into() } else { tier_name(t) }) };
            return Some(format!("{} idol, {rar}", inf(kind)));
        }
        "artefact->misc" => match t { "anyimbued" => "Imbued items", "anysynth" => "Synthesised items", "voidstones" => "Voidstones", "anytrinket" => "Trinkets",
            "fishingrods" => "Fishing rods", "pieces" => "Unique pieces", _ => return None },
        "artefact->sanctifiedrelics" => match t { "selectedrelics" => "Good relics", "anyrelic" => "Any relic", _ => return None },
        "exoticmap->sanctum" => match t { "ilvl83" => "High item level", _ => return None },
        "miscmapitemsextra" => match t { "relickeys" => "Reliquary keys", "relickeyssafe" => "Vault keys", "itemizedleaguesincursion" => "Chronicle of Atzoatl",
            "itemizedleaguesultimatum" => "Inscribed Ultimatum", "itemizedleaguesmercenary" => "Mercenary Warrant", _ => return None },
        "questlike" => match t { "invitations" => "Invitations", "labyrinthconsumable" => "Labyrinth items", "incursionconsumable" => "Incursion items", _ => return None },
        "enshrouded" => "Enshrouded gear",
        // NeverSink ranks the bases in these: tier 1 = the most wanted ones.
        "rr" | "rr->amuring" | "rr->belts" | "rare->exotic->veiled" | "exoticbases" => {
            let n: u32 = t.strip_prefix('t')?.parse().ok()?;
            return Some(match n { 1 => "Tier 1 · best bases".into(), 2 => "Tier 2 · good bases".into(), n => format!("Tier {n} · other bases") });
        }
        _ => return None,
    }.to_string())
}

/// NeverSink's "identified with good mods" steps ("helmet_life_based", "amu_2corecaster", "idaj_mixer") in plain words.
fn mods_step_name(t: &str) -> String {
    let whole = match t {
        "minionwand" => "Minion wands", "t1veil" => "Best veiled mods", "bleedlifeflask" => "Life flasks against bleeding",
        "doublemodutilflask" => "Utility flasks, two good mods", "singlemodutilflask" => "Utility flasks, one good mod",
        "doublemodtincture" => "Tinctures, two good mods", "singlemodtincture" => "Tinctures, one good mod", "bladeblast_2core" => "Blade Blast weapons",
        "gem_bow" => "Bows with + gem levels", "gem_caster" => "Caster weapons with + gem levels", "gem_staves" => "Staves with + gem levels",
        "amu_exalter" => "Amulets worth exalting", _ => "",
    };
    if !whole.is_empty() { return whole.into(); }
    // "t1phys", "t2suppress": tier, then the kind.
    if let Some(rest) = t.strip_prefix('t') {
        let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !n.is_empty() && !rest[n.len()..].is_empty() && !rest.contains('_') {
            let kind = match &rest[n.len()..] { "phys" => "physical weapons", "caster" => "caster weapons", "proj" => "bows and wands (projectiles)",
                "amu" => "amulets", "minionhelm" => "minion helmets", "quiver" => "quivers", "suppress" => "spell suppression", o => o };
            return format!("Tier {n}: {kind}");
        }
    }
    let word = |w: &str| -> String { match w {
        "weapon" => "Weapons", "helmet" => "Helmets", "boots" => "Boots", "gloves" => "Gloves", "shield" => "Shields", "amu" => "Amulets",
        "ring" => "Rings", "quiv" => "Quivers", "body" => "Body armours", "belt" => "Belts", "idjewel" => "Jewels", "idaj" => "Abyss jewels",
        "caster" => "Caster weapons", "phys" => "physical", "physpure" => "pure physical", "ele" => "elemental", "elepure" => "pure elemental",
        "life" => "life", "es" => "energy shield", "based" | "general" => "", "esfocus" => "energy shield", "defensefocus" => "defences",
        "casterfocus" => "caster", "lifefocus" => "life", "attack" => "attack", "light" => "lightning", "minion" => "minion", "hit" => "hit damage",
        "dot" => "damage over time", "defense" => "defences", "lifedefense" => "life and defences", "critattack" => "attack crits",
        "critspell" => "spell crits", "mixer" => "mixed", "1corecaster" => "caster (1 key mod)", "2corecaster" => "caster (2 key mods)",
        "1coredot" => "damage over time (1 key mod)", "2coredot" => "damage over time (2 key mods)", "1coreattack" => "attack (1 key mod)",
        "2coreattack" => "attack (2 key mods)", "proj" => "projectile", "amulet" => "amulets", "minionhelm" => "minion helmets",
        "quiver" => "quivers", "suppress" => "spell suppression",
        other => {
            // "fireweapont2" -> "fire", "slingert2" -> "spell slinger", "t1" -> "Tier 1".
            if let Some(n) = other.strip_prefix('t').and_then(|n| n.parse::<u32>().ok()) { return format!("Tier {n}"); }
            let o = other.trim_end_matches("t2").trim_end_matches("weapon");
            return match o { "light" => "lightning".into(), "phys" => "physical".into(), "slinger" => "spell slinger".into(), _ => o.to_string() };
        }
    }.to_string() };
    let parts: Vec<String> = t.split('_').map(word).filter(|w| !w.is_empty()).collect();
    let out = match parts.as_slice() { [a, rest @ ..] if !rest.is_empty() => format!("{a}: {}", rest.join(" ")), _ => parts.join(" ") };
    let mut c = out.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// A step's name in its section: gear sections have their own words, the rest `tier_name`.
pub fn step_name(key: &str, t: &str) -> String {
    gear_tier_name(key, t).unwrap_or_else(|| if key == "gold" { gold_name(t) } else { tier_name(t) })
}

/// The Tiers tab's name for a NeverSink section, when it lists it.
pub fn section_title(type_path: &[String]) -> Option<String> {
    let key = type_path.join("->");
    KNOWN.iter().find(|k| k.0 == key).map(|k| k.2.to_string())
}

pub fn gold_name(t: &str) -> String {
    match t {
        "stack3" => "Huge piles".into(), "stack2" => "Large piles".into(), "stack1" => "Medium piles".into(),
        "stacklvl1" => "Small piles (levelling only)".into(), "anyother" => "Any other gold".into(), _ => tier_name(t),
    }
}

fn note_of(lines: &[String]) -> String {
    let mut out = vec![];
    for l in lines {
        let t = l.trim();
        if let Some(v) = t.strip_prefix("StackSize") {
            let v = v.trim();
            if let Some(n) = v.strip_prefix(">=") { out.push(format!("stacks of {}+", n.trim())); }
            else if let Some(n) = v.strip_prefix('>') { out.push(format!("stacks over {}", n.trim())); }
            else if let Some(n) = v.strip_prefix('<') { out.push(format!("stacks under {}", n.trim_start_matches('=').trim())); }
        } else if let Some(v) = t.strip_prefix("ItemLevel") {
            let v = v.trim();
            out.push(match v.strip_prefix(">=") { Some(n) => format!("item level {}+", n.trim()), None => format!("item level {v}") });
        } else if let Some(v) = t.strip_prefix("AreaLevel") {
            out.push(format!("area level {}", v.trim()));
        }
    }
    out.join(", ")
}

/// Every tier-listed section in a filter: known ones first in a fixed order,
/// or (when none are known, e.g. PoE 2) any section with 3+ base-listed tiers.
pub fn sections(segs: &[Segment]) -> Vec<Section> {
    let mut found: Vec<Section> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        let Some(tier) = b.tier.clone() else { continue };
        let bases = b.base_types();
        if bases.is_empty() || b.type_path.is_empty() { continue; }
        let key = b.type_path.join("->");
        let i = match found.iter().position(|x| x.type_path == b.type_path) {
            Some(i) => i,
            None => {
                let (group, title) = KNOWN.iter().find(|k| k.0 == key).map(|k| (k.1.to_string(), k.2.to_string()))
                    .unwrap_or_else(|| ("Other".into(), b.type_path.iter().map(|p| tier_name(p)).collect::<Vec<_>>().join(" › ")));
                found.push(Section { type_path: b.type_path.clone(), title, group, tiers: vec![], shared_bases: false });
                found.len() - 1
            }
        };
        // The same tier can appear twice (e.g. a campaign copy); keep the first.
        if found[i].tiers.iter().any(|t| t.tier == tier) { continue; }
        found[i].tiers.push(TierRow {
            name: step_name(&key, &tier),
            tier, visibility: b.visibility.clone(), style: style_of(&b.lines), bases, note: note_of(&b.lines),
            min_stack: b.lines.iter().find_map(|l| l.trim().strip_prefix("StackSize >=").and_then(|n| n.trim().parse().ok())),
            min_item_tier: b.lines.iter().find_map(|l| l.trim().strip_prefix("UnidentifiedItemTier >=").and_then(|n| n.trim().parse().ok())),
            conds: crate::conds::conds_of(&b.lines), changed: vec![],
        });
    }
    for s in found.iter_mut() {
        let mut seen = std::collections::HashSet::new();
        s.shared_bases = s.tiers.iter().flat_map(|t| t.bases.iter()).any(|b| !seen.insert(b.clone()));
    }
    // Levelling rules that aren't item lists (weapon steps, first areas…) still get their section.
    // Every step, with or without an item list (Act 1 gear's boots / quivers / jewellery steps have none).
    // Jewels too (abyss steps pick by item class, not by item, so they'd be missing), and any known section that
    // has steps but no item list.
    let whole: Vec<&(&str, &str, &str)> = KNOWN.iter()
        .collect();
    for (key, group, title) in whole {
        let path: Vec<String> = key.split("->").map(String::from).collect();
        let Some(mut sec) = section_at(segs, &path) else { continue };
        sec.group = group.to_string(); sec.title = title.to_string();
        // Item lists still allow dragging when no item sits in two rows.
        let mut seen = std::collections::HashSet::new();
        sec.shared_bases = sec.tiers.iter().flat_map(|t| t.bases.iter()).any(|b| !seen.insert(b.clone()));
        match found.iter().position(|s| s.type_path == path) { Some(i) => found[i] = sec, None => found.push(sec) }
    }
    for s in found.iter_mut().filter(|s| s.group == LEVELLING) {
        // Flask steps are one size each ("Small" in the filter): name them by it, e.g. "Small life flask".
        let kind = s.title.strip_suffix(" flasks").map(str::to_lowercase);
        for t in &mut s.tiers {
            if let (Some(k), [b]) = (&kind, t.bases.as_slice()) { if !b.contains("Flask") { t.name = format!("{b} {k} flask"); continue; } }
            if let Some(n) = levelling_tier_name(&t.tier) { t.name = n; }
        }
    }
    let rank = |s: &Section| KNOWN.iter().position(|k| k.0 == s.type_path.join("->")).unwrap_or(usize::MAX);
    // Unknown sections only when none are known (e.g. PoE 2's different names).
    let any_known = found.iter().any(|s| rank(s) != usize::MAX);
    found.retain(|s| if any_known { rank(s) != usize::MAX } else { s.tiers.len() >= 3 });
    found.sort_by_key(|s| rank(s));
    found
}

/// Any one of NeverSink's rules (all blocks sharing a `$type` path), even ones that
/// aren't tier lists, e.g. "levelling › white & blue › 4-links". Each tier is a row
/// with its conditions in plain words, so "Modify rule" can open it.
pub fn section_at(segs: &[Segment], type_path: &[String]) -> Option<Section> {
    let mut tiers: Vec<TierRow> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if b.type_path != type_path { continue; }
        let Some(tier) = b.tier.clone() else { continue };
        if tiers.iter().any(|t| t.tier == tier) { continue; }
        let mut note = conditions(&b.lines);
        let base_note = note_of(&b.lines);
        if !base_note.is_empty() { note.push(base_note); }
        tiers.push(TierRow {
            name: step_name(&type_path.join("->"), &tier), tier, visibility: b.visibility.clone(), style: style_of(&b.lines),
            bases: b.base_types(), note: note.join(", "),
            min_stack: b.lines.iter().find_map(|l| l.trim().strip_prefix("StackSize >=").and_then(|n| n.trim().parse().ok())),
            min_item_tier: b.lines.iter().find_map(|l| l.trim().strip_prefix("UnidentifiedItemTier >=").and_then(|n| n.trim().parse().ok())),
            conds: crate::conds::conds_of(&b.lines), changed: vec![],
        });
    }
    if tiers.is_empty() { return None; }
    let title = section_title(type_path).unwrap_or_else(|| type_path.iter().map(|p| plain_part(p)).collect::<Vec<_>>().join(" › "));
    Some(Section { type_path: type_path.to_vec(), title, group: "Opened rule".into(), tiers, shared_bases: true })
}

/// NeverSink's short section names in plain words.
pub fn plain_part(p: &str) -> String {
    match p {
        "leveling" => "Levelling".into(), "normalmagic" => "White & blue".into(), "rare" => "Rares".into(),
        "4l" => "4-links".into(), "3l" => "3-links".into(), "5l" => "5-links".into(), "6l" => "6-links".into(),
        "rgb" => "R-G-B sockets".into(), "exotic" => "Special".into(), "general" => "General".into(),
        "gear" => "Gear".into(), "tiered" => "By item tier".into(), "uniques" => "Uniques".into(),
        "currency" => "Currency".into(), "divination" => "Divination cards".into(), "decorators" => "Highlights".into(),
        _ => {
            let mut c = p.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    }
}

/// The other conditions of a block in plain words: rarity, links, item classes…
fn conditions(lines: &[String]) -> Vec<String> {
    let mut out = vec![];
    let names = |v: &str| -> Vec<String> { v.split('"').map(str::trim).filter(|x| !x.is_empty() && *x != "==" && *x != "=").map(String::from).collect() };
    for l in lines {
        let t = l.trim();
        let (key, rest) = t.split_once(' ').unwrap_or((t, ""));
        let rest = rest.trim();
        match key {
            "Rarity" => {
                let r = rest.trim_start_matches(['=', '<', '>', ' ']).split_whitespace().map(|x| x.to_lowercase()).collect::<Vec<_>>();
                let op = rest.split_whitespace().next().unwrap_or("");
                out.push(match op { "<=" | "<" | ">=" | ">" => format!("rarity {op} {}", r.join(" ")), _ => r.join(" & ") });
            }
            "LinkedSockets" => {
                let n: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
                let more = if rest.starts_with('>') { "+" } else if rest.starts_with('<') { " or fewer" } else { "" };
                out.push(format!("{n}{more} links"));
            }
            "Sockets" | "SocketGroup" => out.push(format!("sockets {}", rest.trim_start_matches(['=', ' ']))),
            "Class" => {
                let c = names(rest);
                out.push(if c.len() <= 3 { c.join(", ") } else { format!("{} item types", c.len()) });
            }
            "Corrupted" => out.push(if rest == "True" { "corrupted".into() } else { "not corrupted".into() }),
            "Identified" => out.push(if rest == "True" { "identified".into() } else { "unidentified".into() }),
            "Quality" => out.push(format!("quality {rest}")),
            "DropLevel" => out.push(format!("drop level {rest}")),
            _ => {}
        }
    }
    out
}

/// One stack section as offered in the in-game panel.
#[derive(Debug, Clone, Serialize)]
pub struct StackChoice {
    pub type_path: Vec<String>,
    pub title: String,
    pub tiers: Vec<TierRow>,
    /// The tier this item is in now, if any.
    pub current: Option<String>,
}

/// Stack sections for a currency: the general ones (3+, 6+) and any that already list it.
pub fn stack_choices(segs: &[Segment], base: &str) -> Vec<StackChoice> {
    sections(segs).into_iter()
        .filter(|s| s.group == "Currency" && s.title != "Gold" && s.tiers.iter().all(|t| t.min_stack.is_some()))
        .filter_map(|s| {
            let current = s.tiers.iter().find(|t| t.bases.iter().any(|b| b == base)).map(|t| t.tier.clone());
            let general = matches!(s.type_path.last().map(String::as_str), Some("stackedthree" | "stackedsix"));
            (general || current.is_some()).then(|| StackChoice { type_path: s.type_path, title: s.title, tiers: s.tiers, current })
        })
        .collect()
}

/// One map tier rule of NeverSink's ("maps_d_t3" → maps of tier 3).
#[derive(Debug, Clone, Serialize)]
pub struct MapTierRow {
    pub map_tier: u32,
    /// NeverSink's band letter: a = T14–16, b = T11–13, c = T6–10, d = T1–5.
    pub band: String,
    #[serde(flatten)]
    pub row: TierRow,
}

/// Maps for the Tiers tab's map panel: the plain tier-by-tier rules and the special kinds
/// (influenced, blighted, Valdo…), which NeverSink checks first.
#[derive(Debug, Clone, Serialize)]
pub struct MapTiers {
    pub tiers: Vec<MapTierRow>,
    pub special: Vec<Section>,
    /// Border highlights NeverSink adds on top (upgrades, implicits…): each can be switched off.
    pub extras: Vec<MapExtra>,
}

/// One of NeverSink's map highlights that only add a border and let the tier rules decide the rest.
#[derive(Debug, Clone, Serialize)]
pub struct MapExtra {
    pub key: String,
    pub title: String,
    /// What it does in game, in one line.
    pub what: String,
    pub tiers: Vec<String>,
    /// Still draws something (not switched off).
    pub on: bool,
    /// The border colour it draws (from NeverSink's rule, or its usual one when switched off), for the preview.
    pub border: [u8; 4],
    /// A map tier it applies to, to show an example label.
    pub sample_tier: u32,
}

/// NeverSink's map rule names in plain words ("maps_a_t16" → "Tier 16 maps").
fn map_rule_name(t: &str) -> Option<String> {
    if let Some((_, n)) = map_tier_of(t) { return Some(format!("Tier {n} maps")); }
    if let Some(n) = t.strip_prefix("deco_mapup_t") { return Some(format!("Upgrade border: tier {n}+ maps while you run lower")); }
    if let Some(n) = t.strip_prefix("deco_zone").and_then(|r| r.strip_suffix("general")) {
        return Some(match n { "1" => "Border: tier 14–16 maps", "2" => "Border: tier 11–13 maps", "3" => "Border: tier 6–10 maps", _ => "Border: tier 1–5 maps" }.into());
    }
    Some(match t {
        "decor_implicit" => "Gold border: maps with implicit mods",
        "deco_corruptedmod" => "Gold border: maps with corrupted mods",
        "zanamap" => "Originator maps, tier 14+", "zanamaplow" => "Originator maps, lower tiers",
        "zanacorrupthider" => "Corrupted Zana maps (hidden)",
        "infelder" => "Shaper / Elder influenced maps", "infconquerors" => "Conqueror influenced maps",
        "valdomaps" => "Valdo's maps", "nightmaremaps" => "Nightmare maps", "shaperguardianmaps" => "Shaper guardian maps",
        "exoticmod1" => "Maps with the best special mods", "exoticmod2" => "Maps with good special mods", "mod8" => "Maps with 8 mods",
        "uber" => "Blight-ravaged maps",
        "corruptedmaphider" => "Corrupted white/blue maps", "mirroredmaphider" => "Mirrored white/blue maps",
        _ => return None,
    }.into())
}

/// "maps_a_t16" → ("a", 16).
fn map_tier_of(tier: &str) -> Option<(String, u32)> {
    let rest = tier.strip_prefix("maps_")?;
    let (band, t) = rest.split_once("_t")?;
    Some((band.to_string(), t.parse().ok()?))
}

fn map_kind(p: &str) -> String {
    match p {
        "influenced" => "Influenced maps (Shaper, Elder, Conquerors)".into(),
        "nightmare" => "Valdo's, nightmare & guardian maps".into(),
        "blighted" => "Blighted maps".into(),
        "vaaltemple" => "Vaal Temple maps".into(),
        "specialmods" => "Maps with special mods (8 mods, rare boss mods)".into(),
        "enchanted" => "Enchanted maps".into(),
        "corruptedimplicit" => "Maps with corrupted implicits".into(),
        "implicitmod" => "Citadel (conqueror) maps".into(),
        _ => plain_part(p),
    }
}

pub fn map_tiers(segs: &[Segment]) -> MapTiers {
    let maps = ["maps".to_string()];
    let mut tiers: Vec<MapTierRow> = section_at(segs, &maps).map(|s| s.tiers).unwrap_or_default().into_iter()
        .filter_map(|row| map_tier_of(&row.tier).map(|(band, map_tier)| MapTierRow { map_tier, band, row }))
        .collect();
    tiers.sort_by_key(|t| t.map_tier);
    let mut paths: Vec<Vec<String>> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if b.type_path.len() >= 2 && b.type_path[0] == "maps" && !paths.contains(&b.type_path) { paths.push(b.type_path.clone()); }
    }
    let mut special: Vec<Section> = vec![];
    for p in &paths {
        let Some(mut s) = section_at(segs, p) else { continue };
        s.title = map_kind(&p[1..].join("->"));
        s.group = "Maps".into();
        // NeverSink's generic tier names say little here: name them by what they catch.
        if p[1] == "implicitmod" {
            for t in s.tiers.iter_mut() { t.name = if t.tier == "t1" { "Citadel maps, tier 14+".into() } else { "Citadel maps, lower tiers".into() }; }
        }
        // Originator (Zana memory) maps get their own row: they look very different from other influenced maps.
        let (zana, rest): (Vec<TierRow>, Vec<TierRow>) = s.tiers.iter().cloned().partition(|t| t.tier.starts_with("zana"));
        if !zana.is_empty() {
            special.push(Section { title: "Originator maps (Zana memories)".into(), tiers: zana, ..s.clone() });
            if rest.is_empty() { continue; }
            s.tiers = rest;
            s.title = "Influenced maps (Shaper, Elder, Conquerors)".into();
        }
        special.push(s);
    }
    // Highlights: blocks under "maps" that aren't tier rules, grouped by what they do.
    let groups: [(&str, &str, &str, fn(&str) -> bool); 4] = [
        ("upgrade", "Upgrade borders", "A red border on maps above the tier you're running now, so upgrades stand out.", |t| t.starts_with("deco_mapup_")),
        ("bands", "Tier group borders", "A dark border on red maps and a light one on lower maps.", |t| t.starts_with("deco_zone")),
        ("implicit", "Implicit borders", "A gold border on maps with implicit mods (e.g. from the atlas).", |t| t == "decor_implicit"),
        ("corrupted", "Corrupted mod borders", "A gold border on maps with corrupted mods.", |t| t == "deco_corruptedmod"),
    ];
    let mut extras = vec![];
    for (key, title, what, test) in groups {
        let mut names = vec![];
        let mut on = false;
        let mut border: Option<[u8; 4]> = None;
        for s in segs {
            let Segment::Block(b) = s else { continue };
            let Some(t) = b.tier.as_deref() else { continue };
            if b.type_path != maps || !test(t) || names.iter().any(|n| n == t) { continue; }
            names.push(t.to_string());
            if border.is_none() {
                border = b.lines.iter().find_map(|l| l.trim().strip_prefix("SetBorderColor").map(|v| {
                    let n: Vec<u8> = v.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    [n.first().copied().unwrap_or(0), n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0), n.get(3).copied().unwrap_or(255)]
                }));
            }
            on |= b.lines.iter().any(|l| { let l = l.trim_start(); l.starts_with("Set") || l.starts_with("Play") || l.starts_with("MinimapIcon") });
        }
        if !names.is_empty() { let (usual, sample_tier) = match key { "upgrade" => ([220, 50, 0, 255], 14), "bands" => ([0, 0, 0, 255], 14), _ => ([255, 180, 0, 255], 10) };
            extras.push(MapExtra { key: key.into(), title: title.into(), what: what.into(), tiers: names, on, border: border.unwrap_or(usual), sample_tier }); }
    }
    MapTiers { tiers, special, extras }
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_tier_rules() {
        let segs = crate::filter::parse_filter(&fixture());
        let m = super::map_tiers(&segs);
        assert_eq!(m.tiers.iter().map(|t| t.map_tier).collect::<Vec<_>>(), (1..=16).collect::<Vec<_>>());
        assert_eq!(m.tiers[15].band, "a");
        assert_eq!(m.tiers[0].band, "d");
        assert!(m.special.iter().any(|s| s.title == "Blighted maps"));
        let z = m.special.iter().find(|s| s.title.starts_with("Originator")).unwrap();
        assert_eq!(z.tiers.iter().map(|t| t.tier.as_str()).collect::<Vec<_>>(), ["zanamap", "zanamaplow"]);
        assert!(m.special.iter().any(|s| s.title.starts_with("Influenced") && s.tiers.iter().all(|t| !t.tier.starts_with("zana"))));
        assert_eq!(m.tiers[15].row.name, "Tier 16 maps");
        assert_eq!(m.extras.iter().map(|e| e.key.as_str()).collect::<Vec<_>>(), ["upgrade", "bands", "implicit", "corrupted"]);
        assert!(m.extras.iter().all(|e| e.on));
        assert_eq!(m.extras[0].border, [220, 50, 0, 255]);
        // Switching the upgrade borders off empties their look but keeps them as pass-through rules.
        let up = m.extras[0].tiers.clone();
        let mut off = crate::filter::parse_filter(&fixture());
        let rules: Vec<Rule> = up.iter().map(|t| Rule::Tier { type_path: vec!["maps".into()], tier: t.clone(), show: None, actions: Some(vec![]), min_stack: None, min_item_tier: None, conds: None }).collect();
        apply(&mut off, &rules, "
");
        assert!(!super::map_tiers(&off).extras[0].on);
        // Hiding tiers 1-3 survives as tier rules.
        let mut segs = segs;
        let rules: Vec<Rule> = m.tiers.iter().filter(|t| t.map_tier < 4).map(|t| Rule::Tier {
            type_path: vec!["maps".into()], tier: t.row.tier.clone(), show: Some(false), actions: None, min_stack: None, min_item_tier: None, conds: None }).collect();
        assert!(apply(&mut segs, &rules, "\n").is_empty());
        let after = super::map_tiers(&segs);
        assert!(after.tiers.iter().all(|t| (t.row.visibility == "Hide") == (t.map_tier < 4)));
    }

    #[test]
    fn opens_any_rule() {
        let f = "Show # $type->leveling->normalmagic->4l $tier->general\n\tLinkedSockets >= 4\n\tRarity Normal Magic\n\tAreaLevel <= 67\n\tSetTextColor 255 255 255 255\n";
        let segs = crate::filter::parse_filter(f);
        let tp: Vec<String> = ["leveling", "normalmagic", "4l"].iter().map(|s| s.to_string()).collect();
        let s = super::section_at(&segs, &tp).unwrap();
        assert_eq!(s.title, "4-linked items"); // listed under "Before maps" now
        assert_eq!(s.tiers[0].note, "4+ links, normal & magic, area level <= 67");
        // ...and the Tiers tab lists it under "Before maps" even though it has no item list.
        let all = super::sections(&segs);
        assert!(all.iter().any(|x| x.title == "4-linked items" && x.group == super::LEVELLING && x.tiers[0].name == "General"));
    }
    use super::*;
    use crate::filter::{parse_filter, render_filter};
    use crate::rules::{apply, Rule};

    fn fixture() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap()
    }

    #[test]
    fn lists_real_tiers() {
        let s = sections(&parse_filter(&fixture()));
        let cur = &s[0];
        assert_eq!(cur.title, "Currency");
        assert!(cur.tiers[0].bases.contains(&"Mirror of Kalandra".to_string()), "{:?}", cur.tiers[0]);
        assert_eq!(tier_name("t2divine"), "Tier 2 · about a Divine Orb");
        assert_eq!(tier_name("n12_i84_t1"), "Tier 1 · 12 passives");
        assert_eq!(tier_name("optimalhighlarge"), "Large, 8 passives (best item level)");
        assert_eq!(tier_name("t3lifeforce"), "Tier 3");
        assert_eq!(tier_name("twisdom"), "Wisdom scrolls");
        let w = s.iter().find(|x| x.title == "Wisdom scroll stacks").unwrap();
        assert_eq!(w.tiers.iter().map(|t| t.min_stack.unwrap()).collect::<Vec<_>>(), vec![10, 5, 3]);
        let stacks = s.iter().find(|x| x.title == "Stacks of 3+").unwrap();
        assert!(stacks.tiers[0].note.contains("stacks of 3+"), "{:?}", stacks.tiers[0].note);
        assert!(s.iter().any(|x| x.title == "Divination cards" && x.tiers.len() >= 5));
        println!("{:?}", s.iter().map(|x| (&x.group, &x.title, x.tiers.len())).collect::<Vec<_>>());
    }

    #[test]
    fn verdict_names_its_rule() {
        let segs = parse_filter(&fixture());
        let it = |n| crate::item::Item { item_class: "Stackable Currency".into(), rarity: "Currency".into(), base_type: "Jeweller's Orb".into(), stack_size: Some(n), ..Default::default() };
        let v = crate::evaluate::status(&segs, &it(3), Some(70), false).verdict;
        println!("{:?}", v.source);
        assert_eq!(v.source.kind, "neversink");
        assert!(v.source.section.as_deref().map(|s| s.starts_with("Stacks of")).unwrap_or(false), "{:?}", v.source);
        assert!(v.source.tier_name.as_deref().unwrap_or("").starts_with("Tier"));
    }

    #[test]
    fn drag_item_between_tiers() {
        let mut segs = parse_filter(&fixture());
        let before = sections(&segs);
        let from = before[0].tiers.iter().find(|t| t.bases.contains(&"Chaos Orb".to_string())).unwrap().tier.clone();
        let rules = vec![Rule::MoveBase { type_path: vec!["currency".into()], base: "Chaos Orb".into(), to_tier: "t2divine".into() }];
        assert!(apply(&mut segs, &rules, "\r\n").is_empty());
        let after = sections(&parse_filter(&render_filter(&segs)));
        let has = |t: &str| after[0].tiers.iter().find(|x| x.tier == t).unwrap().bases.contains(&"Chaos Orb".to_string());
        assert!(has("t2divine") && !has(&from));
    }

    #[test]
    fn add_a_currency_to_a_stack_tier() {
        use crate::evaluate::status;
        let text = fixture();
        let segs0 = parse_filter(&text);
        let secs = sections(&segs0);
        let stacked: Vec<&String> = secs.iter().filter(|x| x.title.starts_with("Stacks") || x.title.ends_with("stacks"))
            .flat_map(|x| x.tiers.iter().flat_map(|t| t.bases.iter())).collect();
        // A currency NeverSink only tiers as single drops.
        let base = secs[0].tiers.iter().flat_map(|t| t.bases.iter()).find(|b| !stacked.contains(b)).unwrap().clone();
        let mut segs = parse_filter(&text);
        apply(&mut segs, &[Rule::MoveBase { type_path: vec!["currency".into(), "stackedsix".into()], base: base.clone(), to_tier: "t7".into() }], "\r\n");
        let segs = parse_filter(&render_filter(&segs));
        let it = |n| crate::item::Item { item_class: "Stackable Currency".into(), rarity: "Currency".into(), base_type: base.clone(), stack_size: Some(n), ..Default::default() };
        let v6 = status(&segs, &it(6), Some(70), false).verdict;
        let v1 = status(&segs, &it(1), Some(70), false).verdict;
        assert!(v6.decided_by.contains("stackedsix") && v6.decided_by.contains("t7"), "{base}: {}", v6.decided_by);
        assert!(!v1.decided_by.contains("stackedsix"), "single drops keep their normal tier");
        let ch = stack_choices(&segs, &base);
        assert!(ch.iter().any(|c| c.title == "Stacks of 6+" && c.current.as_deref() == Some("t7")));
        // Taking it out again.
        let mut segs2 = parse_filter(&render_filter(&segs));
        apply(&mut segs2, &[Rule::MoveBase { type_path: vec!["currency".into(), "stackedsix".into()], base: base.clone(), to_tier: String::new() }], "\r\n");
        let segs2 = parse_filter(&render_filter(&segs2));
        assert!(stack_choices(&segs2, &base).iter().all(|c| c.current.is_none()));
        let w = stack_choices(&segs2, "Scroll of Wisdom");
        assert!(w.iter().any(|c| c.title == "Wisdom scroll stacks" && c.current.is_some()), "{:?}", w.iter().map(|c| &c.title).collect::<Vec<_>>());
    }

    #[test]
    fn tier_rule_hides_and_restyles() {
        let mut segs = parse_filter(&fixture());
        let rules = vec![
            Rule::Tier { type_path: vec!["currency".into()], tier: "t5alchemy".into(), show: Some(false), actions: None, min_stack: None, min_item_tier: None, conds: None },
            Rule::Tier { type_path: vec!["gold".into()], tier: "stack2".into(), show: None, actions: None, min_stack: Some(800), min_item_tier: None, conds: None },
            Rule::Tier { type_path: vec!["divination".into()], tier: "t1".into(), show: None,
                actions: Some(vec!["SetFontSize 45".into(), "SetTextColor 1 2 3 255".into()]), min_stack: None, min_item_tier: None, conds: None },
        ];
        let w = apply(&mut segs, &rules, "\r\n");
        assert!(w.is_empty(), "{w:?}");
        let out = parse_filter(&render_filter(&segs));
        let s = sections(&out);
        let alch = s[0].tiers.iter().find(|t| t.tier == "t5alchemy").unwrap();
        assert_eq!(alch.visibility, "Hide");
        let div = s.iter().find(|x| x.title == "Divination cards").unwrap();
        let t1 = div.tiers.iter().find(|t| t.tier == "t1").unwrap();
        assert_eq!(t1.style.text, Some([1, 2, 3, 255]));
        assert_eq!(t1.style.sound, None, "old sound removed");
        let gold = s.iter().find(|x| x.title == "Gold").unwrap();
        let large = gold.tiers.iter().find(|t| t.tier == "stack2").unwrap();
        assert_eq!((large.name.as_str(), large.min_stack), ("Large piles", Some(800)));
    }
}
