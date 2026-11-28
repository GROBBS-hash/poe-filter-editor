//! Price tiering: sorts items into your own value tiers using poe.ninja
//! prices (from the app's hourly price file) and writes filter blocks for them.
//! Your manual edits stay on top; price tiers come next; NeverSink after that.

use crate::rules::{style_actions, Beam, CustomStyle, Minimap, Sound};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceItem {
    pub name: String,
    pub base: String,
    pub cat: String,
    pub value: f64,
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub listings: Option<u32>,
    /// poe.ninja's last 7 days: % change each day from a week ago (oldest first), and the total change.
    #[serde(default)]
    pub trend: Option<Vec<f64>>,
    #[serde(default)]
    pub change: Option<f64>,
    /// poe.ninja exchange items: how much was traded recently, in the file's unit (value x number sold).
    #[serde(default)]
    pub volume: Option<f64>,
}

impl PriceItem {
    /// Roughly how many were sold recently (exchange items only).
    pub fn sold(&self) -> Option<f64> { self.volume.filter(|_| self.value > 0.0).map(|v| v / self.value) }
}

/// A divination card's price only counts if people really trade it: under this many sold recently AND worth
/// less than THIN_CAP, poe.ninja's number is a guess from a sale or two (21 Nov: The Calling, "5c", 2 sold, 1c
/// listings unsold). Rare but valuable cards (Father's Love, The Miracle) keep their price.
pub const THIN_SOLD: f64 = 5.0;
pub const THIN_CAP: f64 = 50.0;
pub fn thin_card(it: &PriceItem) -> bool { thin_card_at(it, THIN_SOLD) }
/// The same with his own "really sells" number (Tiers > Divination cards; 0 = every card counts).
pub fn thin_card_at(it: &PriceItem, min_sold: f64) -> bool {
    it.cat == "DivinationCard" && it.value < THIN_CAP && it.sold().is_some_and(|n| n < min_sold)
}

/// A divination card poe.ninja has no line for: nobody bought one lately (it lists every card that traded). False when
/// the file has too few cards to tell (no list that day).
pub fn unsold_card(file: &PriceFile, name: &str) -> bool {
    let mut cards = file.items.iter().filter(|i| i.cat == "DivinationCard" && i.volume.is_some());
    cards.clone().nth(99).is_some() && !cards.any(|i| i.name == name)
}

/// Cards that don't count at that number, dearest first: (name, value, sold).
pub fn thin_cards(file: &PriceFile, min_sold: f64) -> Vec<(String, f64, f64)> {
    let mut v: Vec<_> = file.items.iter().filter(|i| i.value > 0.0 && thin_card_at(i, min_sold))
        .map(|i| (i.name.clone(), i.value, i.sold().unwrap_or(0.0))).collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

/// An item's price trend for the price window: the week's line and its total % change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trend {
    pub points: Vec<f64>,
    pub change: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceFile {
    pub game: String,
    pub league: String,
    #[serde(default)]
    pub league_name: Option<String>,
    pub updated: String,
    /// "chaos" (PoE 1) or "exalted" (PoE 2).
    pub unit: String,
    pub items: Vec<PriceItem>,
    /// Bases to craft on (PoE 1).
    #[serde(default)]
    pub bases: Vec<crate::bases::BasePrice>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group { Uniques, DivinationCards, Currency, Other }

impl Group {
    pub const ALL: [Group; 4] = [Group::Uniques, Group::DivinationCards, Group::Currency, Group::Other];
    pub fn title(self) -> &'static str {
        match self {
            Group::Uniques => "Uniques",
            Group::DivinationCards => "Divination cards",
            Group::Currency => "Currency & league items",
            Group::Other => "Other",
        }
    }
    pub fn of(cat: &str) -> Option<Group> {
        if cat.starts_with("Unique") {
            return Some(Group::Uniques);
        }
        match cat {
            "DivinationCard" => Some(Group::DivinationCards),
            "Incubator" | "Invitation" | "Memory" | "Vial" | "Corpse" | "Wombgift" => Some(Group::Other),
            // Priced for the stash scan only, not for filter price tiers.
            "SkillGem" | "Map" | "BlightedMap" | "BlightRavagedMap" | "Beast" => None,
            // Everything else poe.ninja prices on its exchange.
            _ => Some(Group::Currency),
        }
    }
    /// Extra conditions so a tier only catches the right kind of item.
    fn conditions(self) -> Vec<String> {
        match self {
            Group::Uniques => vec!["Rarity Unique".into()],
            Group::DivinationCards => vec!["Class == \"Divination Cards\"".into()],
            _ => vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tier {
    /// Worth at least this much (in the price file's unit).
    pub min: f64,
    pub style: CustomStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupSettings {
    pub group: Group,
    pub enabled: bool,
    /// Highest first is not required; they're sorted when used.
    pub tiers: Vec<Tier>,
    /// Hide priced items worth less than this (unpriced items are left to NeverSink).
    #[serde(default)]
    pub hide_below: Option<f64>,
    /// Cards: only ones with at least this many sold recently count (None = THIN_SOLD, 0 = all of them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_sold: Option<f64>,
}
impl GroupSettings {
    pub fn really_sells(&self) -> f64 { self.min_sold.unwrap_or(THIN_SOLD) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceSettings {
    pub enabled: bool,
    pub groups: Vec<GroupSettings>,
    /// "Hide if worth under N" per Tiers page section (key "currency->corpses" style type path), 23 Nov:
    /// "only show corpses worth x chaos". Works with price tiers off, like the groups' hide_below.
    #[serde(default)]
    pub section_hide: BTreeMap<String, f64>,
    /// With a "hide if worth under" line: always show stacks of at least N, whatever they're worth (24 Nov, Threads:
    /// "sometimes they drop in stacks").
    #[serde(default)]
    pub section_stack: BTreeMap<String, u32>,
    /// Make bases listed at this much or more stand out (None = off; works with price tiers off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bases_min: Option<f64>,
    /// Influenced ones too.
    #[serde(default)]
    pub bases_influenced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bases_style: Option<CustomStyle>,
    /// Uniques by how much Thaumaturgic Dust they give (PoE 1, works with price tiers off).
    #[serde(default = "crate::dust::default_settings")]
    pub dust: crate::dust::DustSettings,
}

/// The Threads page's key in section_hide / section_stack. NeverSink has no Threads section (they sit in Fragments
/// tier 3 with the Goddess items), so the page lists every item poe.ninja prices as a Thread.
pub const THREADS_KEY: &str = "threads";

/// Every Thread poe.ninja prices, cheapest first.
pub fn thread_prices(file: &PriceFile) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = file.items.iter().filter(|i| i.cat == "Thread" && i.value > 0.0).map(|i| (i.name.clone(), i.value)).collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn look(size: u32, text: [u8; 4], border: [u8; 4], bg: [u8; 4], beam: &str, icon: (u8, &str, &str), sound: Sound) -> CustomStyle {
    CustomStyle {
        font_size: size, text, border: Some(border), background: bg,
        beam: Some(Beam { colour: beam.into(), temp: false }),
        minimap: Some(Minimap { size: icon.0, colour: icon.1.into(), shape: icon.2.into() }),
        sound,
        label_hidden: false,
    }
}

/// Sensible starting tiers: big and loud for the best, calmer further down.
pub fn default_settings(unit: &str) -> PriceSettings {
    let (t1, t2, t3) = if unit == "chaos" { (100.0, 20.0, 5.0) } else { (500.0, 100.0, 20.0) };
    let tiers = |sound1: &str| vec![
        Tier { min: t1, style: look(45, [255, 0, 0, 255], [255, 0, 0, 255], [255, 255, 255, 255], "Red", (0, "Red", "Star"),
            Sound::Builtin { id: sound1.into(), volume: 300 }) },
        Tier { min: t2, style: look(42, [0, 0, 0, 255], [0, 0, 0, 255], [255, 210, 60, 255], "Yellow", (1, "Yellow", "Diamond"),
            Sound::Builtin { id: "2".into(), volume: 250 }) },
        Tier { min: t3, style: look(38, [255, 255, 255, 255], [255, 255, 255, 255], [40, 40, 40, 240], "White", (2, "White", "Circle"),
            Sound::Default) },
    ];
    PriceSettings {
        enabled: false,
        groups: Group::ALL.iter().map(|g| GroupSettings {
            group: *g,
            enabled: matches!(g, Group::Uniques | Group::DivinationCards),
            // Tier 1 always gets NeverSink's tier 1 sound: the classic Divine Orb drop.
            tiers: tiers("6"),
            hide_below: None,
            min_sold: None,
        }).collect(),
        section_hide: BTreeMap::new(),
        section_stack: BTreeMap::new(),
        bases_min: None,
        bases_influenced: false,
        bases_style: None,
        dust: crate::dust::default_settings(),
    }
}

/// Hide rules for the Tiers page's "hide if worth under" lines: every item listed in that NeverSink section that
/// poe.ninja prices under the line. A stack counts at its total worth (5 x 1c corpses stay when the line is 4c).
/// Items poe.ninja doesn't price are left to NeverSink.
pub fn section_hide_blocks(segs: &[crate::filter::Segment], hides: &BTreeMap<String, f64>, stacks: &BTreeMap<String, u32>, file: &PriceFile, nl: &str) -> String {
    let u = unit_short(&file.unit);
    let mut out = String::new();
    // Threads sit in NeverSink's Fragments, but have a page of their own: only its line decides them (24 Nov: he
    // unticked it and Fragments' "under 4c" line still hid them).
    let threads: std::collections::BTreeSet<String> = thread_prices(file).into_iter().map(|(n, _)| n).collect();
    for (key, limit) in hides {
        // "Always show stacks of N+": a cheap item's stack shows from N even when it's worth less than the line.
        let cap = stacks.get(key).copied().filter(|n| *n > 1).unwrap_or(5000);
        let (title, bases): (String, Vec<String>) = if key == THREADS_KEY {
            ("Threads".into(), thread_prices(file).into_iter().map(|(n, _)| n).collect())
        } else {
            let path: Vec<String> = key.split("->").map(String::from).collect();
            let Some(sec) = crate::tiers::section_at(segs, &path) else { continue };
            (crate::tiers::section_title(&path).unwrap_or_else(|| key.clone()), sec.tiers.iter().flat_map(|t| t.bases.iter().cloned()).collect())
        };
        let mut by_stack: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        for b in &bases {
            if !seen.insert(b.clone()) || (key != THREADS_KEY && threads.contains(b)) { continue; }
            let Some(v) = price_of(file, None, b, "Normal").filter(|v| *v > 0.0) else { continue };
            if v >= *limit { continue; }
            by_stack.entry(((limit / v).ceil().min(5000.0) as u32).min(cap)).or_default().push(b.clone());
        }
        for (need, names) in by_stack {
            let refs: Vec<&String> = names.iter().collect();
            out.push_str(&format!("Hide # Price: {title} worth under {limit}{u}{nl}"));
            if need > 1 { out.push_str(&format!("    StackSize < {need}{nl}")); }
            out.push_str(&format!("    BaseType == {}{nl}", base_list(&refs)));
            out.push_str(&format!("    DisableDropSound True{nl}{nl}"));
        }
    }
    out
}

/// Replica uniques only come from Heist, and Foulborn ones from their league mechanic: neither drops as a
/// plain unique on the ground, so their prices mustn't decide how loud a base's ground drops are.
pub fn drops_on_ground(name: &str) -> bool {
    !name.starts_with("Replica ") && !name.starts_with("Foulborn ")
}

/// Every unique that drops on this base, with its price, most valuable first (for "what could this be?").
pub fn uniques_on_base(file: &PriceFile, base: &str) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    for it in file.items.iter().filter(|i| i.cat.starts_with("Unique") && i.base == base && drops_on_ground(&i.name)) {
        match out.iter_mut().find(|(n, _)| *n == it.name) {
            Some(e) => if it.value > e.1 { e.1 = it.value },
            None => out.push((it.name.clone(), it.value)),
        }
    }
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    out
}

/// The cheapest ground-drop unique on each base that has more than one unique.
fn cheapest_unique_by_base(file: &PriceFile) -> BTreeMap<String, f64> {
    let mut by: BTreeMap<String, Vec<(&str, f64)>> = BTreeMap::new();
    for it in file.items.iter().filter(|i| i.cat.starts_with("Unique") && drops_on_ground(&i.name)) {
        by.entry(it.base.clone()).or_default().push((&it.name, it.value));
    }
    // Versions of one unique count too (Voices).
    by.into_iter().filter(|(_, l)| l.iter().any(|(n, v)| *n != l[0].0 || *v != l[0].1))
        .map(|(b, l)| (b, l.iter().map(|x| x.1).fold(f64::INFINITY, f64::min))).collect()
}

/// Value of each base type in a group. Uniques sharing a base take the most
/// valuable one, so hiding or downgrading a base never loses a good unique.
pub fn values_by_base(file: &PriceFile, group: Group) -> BTreeMap<String, f64> { values_by_base_at(file, group, THIN_SOLD) }
pub fn values_by_base_at(file: &PriceFile, group: Group, min_sold: f64) -> BTreeMap<String, f64> {
    let mut m: BTreeMap<String, f64> = BTreeMap::new();
    for it in file.items.iter().filter(|i| Group::of(&i.cat) == Some(group) && (group != Group::Uniques || drops_on_ground(&i.name))) {
        let key = if group == Group::Uniques { it.base.clone() } else { it.name.clone() };
        // A card hardly anyone trades counts as worth nothing (see thin_card).
        let value = if thin_card_at(it, min_sold) { 0.0 } else { it.value };
        let e = m.entry(key).or_insert(0.0);
        if value > *e { *e = value; }
    }
    m
}

/// How big a stack of each card it takes to reach the worth line (0 = none does: no price, or it doesn't really sell).
/// (24 Nov: for a few hours stacks of 2+ counted whatever they sold, for 2 x The Pack Leader at "8c"; then 3 x The Lich
/// showed, 85 sellers and 4 sold, so how much must sell became his setting.)
fn card_stacks(file: &PriceFile, limit: f64, min_sold: f64) -> BTreeMap<String, u32> {
    let mut m: BTreeMap<String, u32> = BTreeMap::new();
    for it in file.items.iter().filter(|i| Group::of(&i.cat) == Some(Group::DivinationCards)) {
        let need = if it.value > 0.0 && !thin_card_at(it, min_sold) { (limit / it.value).ceil().clamp(1.0, 1000.0) as u32 } else { 0 };
        let e = m.entry(it.name.clone()).or_insert(need);
        if need > 0 && (*e == 0 || need < *e) { *e = need; }
    }
    m
}

fn base_list(names: &[&String]) -> String {
    names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ")
}

pub fn unit_short(unit: &str) -> &'static str {
    match unit { "chaos" => "c", "exalted" => "ex", "divine" => "div", _ => "" }
}

/// Filter blocks for your price tiers (empty if switched off).
/// The price rules for the filter: tiers (when price tiers are on) and "hide if worth under" (a group's own
/// setting, which also works with price tiers off: the Tiers page sets it for divination cards and uniques).
pub fn price_blocks(settings: &PriceSettings, file: &PriceFile, nl: &str) -> String {
    let hides = settings.groups.iter().any(|g| g.hide_below.is_some() && g.group != Group::Currency);
    if !settings.enabled && !hides {
        return String::new();
    }
    let u = unit_short(&file.unit);
    let mut out = format!("# Price tiers from poe.ninja ({}, {}). Rebuilt when prices update.{nl}{nl}", file.league, file.updated);
    for g in &settings.groups {
        let tiers_on = settings.enabled && g.enabled;
        if !tiers_on && g.hide_below.is_none() { continue; }
        let values = values_by_base_at(file, g.group, g.really_sells());
        let mut tiers = g.tiers.clone();
        tiers.sort_by(|a, b| b.min.partial_cmp(&a.min).unwrap_or(std::cmp::Ordering::Equal));
        // Uniques: a base where one unique is worth a lot and another next to nothing (often a boss-only drop
        // next to a common one) goes no higher than tier 2, so it doesn't play the top tier's sound every time.
        let cheapest = if g.group == Group::Uniques { cheapest_unique_by_base(file) } else { BTreeMap::new() };
        let tier_of = |base: &str, v: f64| -> Option<usize> {
            let i = tiers.iter().position(|t| v >= t.min)?;
            let mixed = cheapest.get(base).is_some_and(|c| *c < tiers[0].min);
            Some(if i == 0 && mixed && tiers.len() > 1 { 1 } else { i })
        };
        for (i, t) in tiers.iter().enumerate().filter(|_| tiers_on) {
            let names: Vec<&String> = values.iter().filter(|(k, v)| tier_of(k, **v) == Some(i)).map(|(k, _)| k).collect();
            if names.is_empty() { continue; }
            out.push_str(&format!("Show # Price tier {}: {} worth {}{u}+{nl}", i + 1, g.group.title(), t.min));
            for c in g.group.conditions() { out.push_str(&format!("    {c}{nl}")); }
            out.push_str(&format!("    BaseType == {}{nl}", base_list(&names)));
            for a in style_actions(&t.style) { out.push_str(&format!("    {a}{nl}")); }
            out.push_str(nl);
        }
        if let Some(limit) = g.hide_below.filter(|_| g.group != Group::Currency) {
            // Divination cards drop in stacks (Ritual, Harvest...): a stack counts at its total worth, so 3 x 2c
            // cards stay when the line is 5c. Cards are grouped by how big a stack it takes to reach the line.
            let mut by_stack: BTreeMap<u32, Vec<&String>> = BTreeMap::new();
            let stacks = if g.group == Group::DivinationCards { card_stacks(file, limit, g.really_sells()) } else { BTreeMap::new() };
            for (k, _) in values.iter().filter(|(_, v)| **v < limit) {
                let need = stacks.get(k).copied().unwrap_or(0);
                by_stack.entry(need).or_default().push(k);
            }
            for (need, names) in by_stack {
                out.push_str(&format!("Hide # Price: {} worth under {limit}{u}{nl}", g.group.title()));
                for c in g.group.conditions() { out.push_str(&format!("    {c}{nl}")); }
                if need > 1 { out.push_str(&format!("    StackSize < {need}{nl}")); }
                out.push_str(&format!("    BaseType == {}{nl}", base_list(&names)));
                out.push_str(&format!("    DisableDropSound True{nl}{nl}"));
            }
        }
    }
    out
}

/// The other half of the cards' worth line: a card, or a stack of it, worth the line or more shows even where NeverSink's
/// strictness hides that card (24 Nov: 5 x The Master Artisan at 1.18c with the line at 4c got past the line, then uber
/// strict hid it). `segs` = the filter with your changes in it; a step you unticked yourself stays hidden (`rules`).
/// Each takes the look of its own NeverSink step (`themed` recolours it like the rest of the filter).
pub fn card_show_blocks(settings: &PriceSettings, file: &PriceFile, segs: &[crate::filter::Segment], rules: &[crate::rules::Rule],
    themed: &dyn Fn(&mut Vec<crate::filter::Segment>), nl: &str) -> String {
    use crate::{filter::Segment, rules::Rule};
    let Some(g) = settings.groups.iter().find(|g| g.group == Group::DivinationCards) else { return String::new() };
    let Some(limit) = g.hide_below.filter(|l| *l > 0.0) else { return String::new() };
    let values = values_by_base_at(file, Group::DivinationCards, g.really_sells());
    let stacks = card_stacks(file, limit, g.really_sells());
    // Worth this much the price tiers already show it.
    let tiered = (settings.enabled && g.enabled).then(|| g.tiers.iter().map(|t| t.min).fold(f64::INFINITY, f64::min)).unwrap_or(f64::INFINITY);
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for s in segs {
        let Segment::Block(b) = s else { continue };
        if b.type_path.first().map(String::as_str) != Some("divination") { continue; }
        // Only the plain card lists: rules for some stack sizes only (NeverSink's special stacks) decide nothing here.
        let word = |l: &String| l.trim().split_whitespace().next().unwrap_or("").to_string();
        if b.lines.iter().any(|l| matches!(word(l).as_str(), "StackSize" | "ItemLevel" | "AreaLevel")) { continue; }
        let names: Vec<String> = b.base_types().into_iter().filter(|n| seen.insert(n.clone())).collect();
        if b.visibility != "Hide" { continue; }
        let mine = rules.iter().rev().filter(|r| !r.is_off()).find_map(|r| match r {
            Rule::Tier { type_path, tier, show: Some(sh), .. } if *type_path == b.type_path && b.tier.as_deref() == Some(tier.as_str()) => Some(*sh),
            _ => None,
        });
        if mine == Some(false) { continue; }
        let mut by_stack: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for n in names {
            if values.get(&n).is_some_and(|v| *v >= tiered) { continue; }
            let Some(need) = stacks.get(&n).copied() else { continue };
            // Past a full set there's no such stack.
            if (1..=100).contains(&need) { by_stack.entry(need).or_default().push(n); }
        }
        if by_stack.is_empty() { continue; }
        let mut look = b.clone();
        look.set_visibility(true);
        let mut one = vec![Segment::Block(look)];
        themed(&mut one);
        let style = match one.first() { Some(Segment::Block(x)) => x.style_lines(), _ => b.style_lines() };
        for (need, names) in by_stack {
            let refs: Vec<&String> = names.iter().collect();
            out.push_str(&format!("Show # Price: {} worth {limit}{} or more{nl}", Group::DivinationCards.title(), unit_short(&file.unit)));
            for c in Group::DivinationCards.conditions() { out.push_str(&format!("    {c}{nl}")); }
            if need > 1 { out.push_str(&format!("    StackSize >= {need}{nl}")); }
            out.push_str(&format!("    BaseType == {}{nl}", base_list(&refs)));
            for l in &style { out.push_str(&format!("    {l}{nl}")); }
            out.push_str(nl);
        }
    }
    out
}

/// What to multiply this file's prices by to get chaos: 1 for PoE 1; PoE 2's are in exalted (25 Nov: they were shown as
/// chaos, with a Chaos Orb counted as 1 exalted while it was worth 27).
pub fn chaos_per_unit(file: &PriceFile) -> f64 {
    if file.unit == "chaos" { return 1.0; }
    price_of(file, None, "Chaos Orb", "Currency").filter(|v| *v > 0.0).map_or(1.0, |v| 1.0 / v)
}

/// The price of one item, if poe.ninja has it (for the in-game pop-up).
pub fn price_of(file: &PriceFile, name: Option<&str>, base: &str, rarity: &str) -> Option<f64> {
    if rarity == "Unique" {
        if let Some(n) = name {
            if let Some(u) = unique_price(file, n, base, None) { return Some(u.value); }
        }
        return values_by_base(file, Group::Uniques).get(base).copied();
    }
    // Gems are kept at their cheapest version (for stash totals), which would undersell a 20/20 gem here.
    file.items.iter().filter(|i| i.name == base && !i.cat.starts_with("Unique") && i.cat != "SkillGem").map(|i| i.value).reduce(f64::max)
}

/// A named unique's price. Some have versions worth wildly different amounts (25 Nov: Voices 0.86c to 1,030 div, and
/// every one counted as the dearest). `text`: the item's lines, to tell which version it is; unknown, it counts as the
/// cheapest and `high` says what it could be.
#[derive(Debug, Clone, PartialEq)]
pub struct UniquePrice { pub value: f64, pub high: Option<f64> }

pub fn unique_price(file: &PriceFile, name: &str, base: &str, text: Option<&str>) -> Option<UniquePrice> {
    let mut all: Vec<&PriceItem> = file.items.iter().filter(|i| i.name == name && i.cat.starts_with("Unique")).collect();
    // The same name on several bases (Grand Spectrum, Precursor's Emblem): its own base.
    if all.iter().any(|i| i.base == base) { all.retain(|i| i.base == base); }
    if let (Some(t), true) = (text, all.len() > 1) {
        let fits: Vec<&PriceItem> = all.iter().copied().filter(|i| version_fits(i.variant.as_deref(), &all, t)).collect();
        if !fits.is_empty() { all = fits; }
    }
    let low = all.iter().map(|i| i.value).reduce(f64::min)?;
    let high = all.iter().map(|i| i.value).reduce(f64::max).filter(|h| *h > low * 1.5);
    Some(UniquePrice { value: low, high })
}

/// The whole number right before `after` in the text, or right after `before`.
fn number_between(text: &str, before: &str, after: &[&str]) -> Option<u32> {
    let l = text.to_lowercase();
    let mut from = 0;
    while let Some(i) = l[from..].find(before) {
        let rest = &l[from + i + before.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && after.iter().any(|a| rest[digits.len()..].starts_with(a)) { return digits.parse().ok(); }
        from += i + before.len();
    }
    None
}

/// Whether an item with these lines is poe.ninja's version ("3 passives", "2 Jewels", "4 Flasks", "Frenzy"). Versions we
/// can't read off the item (Foulborn mods) never fit, so the item keeps its whole range.
fn version_fits(variant: Option<&str>, all: &[&PriceItem], text: &str) -> bool {
    let num = |v: &str| v.split_whitespace().next().and_then(|n| n.parse::<u32>().ok());
    let word = |v: &str| v.split_whitespace().nth(1).unwrap_or("").trim_end_matches('s').to_lowercase();
    let on_item = |kind: &str| match kind {
        "passive" => number_between(text, "adds ", &[" passive skill", " small passive skill"]),
        "jewel" => number_between(text, "has ", &[" abyssal socket"]),
        "flask" => number_between(text, "leftmost ", &[" magic utility flask", " utility flask"]),
        _ => None,
    };
    match variant {
        Some(v) if num(v).is_some() => on_item(&word(v)).is_some() && on_item(&word(v)) == num(v),
        Some(v) if matches!(v, "Endurance" | "Frenzy" | "Power") => text.contains(&format!("{v} Charges")),
        Some(_) => false,
        // The unnamed version next to numbered ones (Voices with 1 passive): the number none of the others have.
        None => {
            let numbered: Vec<&str> = all.iter().filter_map(|i| i.variant.as_deref()).filter(|v| num(v).is_some()).collect();
            let Some(kind) = numbered.first().map(|v| word(v)) else { return false };
            on_item(&kind).is_some_and(|n| numbered.iter().all(|v| num(v) != Some(n)))
        }
    }
}

/// The lines of an item that tell a unique's version, if it has any (see `unique_price`).
pub fn version_lines(text: &str) -> Option<String> {
    let l: Vec<&str> = text.lines().filter(|l| {
        let l = l.to_lowercase();
        l.contains("passive skill") || l.contains("abyssal socket") || l.contains("leftmost") || l.contains(" charges")
    }).collect();
    (!l.is_empty()).then(|| l.join("\n"))
}

/// The week's trend for the same item `price_of` prices (uniques by name, others by base; not gems, which are kept at
/// their cheapest version). The dearest listing wins, like the price.
pub fn trend_of(file: &PriceFile, name: Option<&str>, base: &str, rarity: &str) -> Option<Trend> {
    let pick = |i: &&PriceItem| if rarity == "Unique" { name.is_some_and(|n| i.name == n) && i.cat.starts_with("Unique") }
        else { i.name == base && !i.cat.starts_with("Unique") && i.cat != "SkillGem" };
    // A unique's version isn't known here: no line when its versions differ (it would be some other one's).
    let mut all = file.items.iter().filter(pick);
    let first = all.next()?;
    let rest: Vec<&PriceItem> = all.collect();
    if rarity == "Unique" && rest.iter().any(|i| i.base == first.base && i.variant != first.variant) { return None; }
    let it = std::iter::once(first).chain(rest).max_by(|a, b| a.value.total_cmp(&b.value))?;
    Some(Trend { points: it.trend.clone()?, change: it.change? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate::status, filter::parse_filter, item::parse_item, rules::apply};

    fn file() -> PriceFile {
        let it = |name: &str, base: &str, cat: &str, value: f64| PriceItem {
            name: name.into(), base: base.into(), cat: cat.into(), value, variant: None, listings: Some(10), trend: None, change: None, volume: None };
        PriceFile {
            game: "poe1".into(), league: "Covens".into(), league_name: None, updated: "now".into(), unit: "chaos".into(), bases: vec![],
            items: vec![
                it("Headhunter", "Leather Belt", "UniqueAccessory", 29584.0),
                it("Wurm's Molt", "Leather Belt", "UniqueAccessory", 1.0),
                it("Goldrim", "Leather Cap", "UniqueArmour", 1.0),
                it("Mageblood", "Heavy Belt", "UniqueAccessory", 20000.0),
                it("Replica Maata's Teaching", "Karui Sceptre", "UniqueWeapon", 125.0),
                it("Maata's Teaching", "Karui Sceptre", "UniqueWeapon", 9.0),
                it("Kaom's Heart", "Glorious Plate", "UniqueArmour", 30.0),
                it("The Doctor", "The Doctor", "DivinationCard", 900.0),
                it("Rain of Chaos", "Rain of Chaos", "DivinationCard", 0.3),
                it("Divine Orb", "Divine Orb", "Currency", 176.0),
            ],
        }
    }

    #[test]
    fn shared_base_uses_best_unique() {
        let v = values_by_base(&file(), Group::Uniques);
        assert_eq!(v["Leather Belt"], 29584.0);
        assert_eq!(price_of(&file(), Some("Wurm's Molt"), "Leather Belt", "Unique"), Some(1.0));
        assert_eq!(price_of(&file(), None, "Leather Belt", "Unique"), Some(29584.0));
        assert_eq!(price_of(&file(), None, "Divine Orb", "Currency"), Some(176.0));
    }

    /// Tiers page "hide items here worth under 4c" on corpses: 1c ones hide unless dropped as a stack of 4+.
    #[test]
    fn threads_worth_line() {
        // 24 Nov: a Threads page with a price line, stacks counting at their total worth, and "always show stacks of N+".
        let mut f = file();
        let it = |name: &str, value: f64| PriceItem { name: name.into(), base: name.into(), cat: "Thread".into(), value, variant: None,
            listings: Some(10), trend: None, change: None, volume: None };
        f.items.push(it("Abyss Thread", 5.5));
        f.items.push(it("Betrayal Thread", 0.3));
        f.items.push(it("Ultimatum Thread", 1.6));
        assert_eq!(thread_prices(&f).first().map(|x| x.0.as_str()), Some("Betrayal Thread"));
        let mut hides = BTreeMap::new();
        hides.insert(THREADS_KEY.to_string(), 2.0);
        let blocks = section_hide_blocks(&[], &hides, &BTreeMap::new(), &f, "\n");
        // 2c line: Betrayal (0.3c) shows from 7, Ultimatum (1.6c) from 2, Abyss always.
        assert!(blocks.contains("StackSize < 7\n    BaseType == \"Betrayal Thread\""), "{blocks}");
        assert!(blocks.contains("StackSize < 2\n    BaseType == \"Ultimatum Thread\""), "{blocks}");
        assert!(!blocks.contains("Abyss"), "{blocks}");
        // "Always show stacks of 3+": Betrayal shows from 3 instead of 7.
        let mut stacks = BTreeMap::new();
        stacks.insert(THREADS_KEY.to_string(), 3);
        let blocks = section_hide_blocks(&[], &hides, &stacks, &f, "\n");
        assert!(blocks.contains("StackSize < 3\n    BaseType == \"Betrayal Thread\""), "{blocks}");
        assert!(blocks.contains("Threads worth under"));
        // The Fragments line leaves Threads alone: they have their own page.
        let segs = parse_filter("Show # $type->fragments $tier->t3\n\tClass == \"Map Fragments\"\n\tBaseType == \"Betrayal Thread\" \"Offering to the Goddess\"\n\tSetFontSize 45\n");
        f.items.push(PriceItem { cat: "Fragment".into(), ..it("Offering to the Goddess", 1.0) });
        let blocks = section_hide_blocks(&segs, &[("fragments".to_string(), 4.0)].into(), &BTreeMap::new(), &f, "\n");
        assert!(blocks.contains("Offering to the Goddess") && !blocks.contains("Thread"), "{blocks}");
    }

    #[test]
    fn card_stacks_worth_the_line_show() {
        let f = PriceFile { bases: vec![], items: vec![
            // Over 5 sold each, so the prices count.
            PriceItem { volume: Some(6.7), ..it_card("The Master Artisan", 1.18) }, PriceItem { volume: Some(100.0), ..it_card("Rain of Chaos", 0.02) },
            PriceItem { volume: Some(900.0), ..it_card("Worth It", 4.5) }, PriceItem { volume: Some(9000.0), ..it_card("Shown Already", 4.5) },
            PriceItem { volume: Some(1.2), ..it_card("Hardly Traded", 1.2) },
            // His, 24 Nov: 4c, under 1 sold.
            PriceItem { volume: Some(0.67), ..it_card("The Pack Leader", 4.0) }, PriceItem { volume: Some(0.67), ..it_card("Low Guess", 0.1) },
        ], ..file() };
        let segs = parse_filter("Show # $type->divination $tier->t3\n\tClass == \"Divination Cards\"\n\tBaseType == \"Shown Already\"\n\tSetFontSize 45\n\nHide # %HS3 $type->divination $tier->t4c\n\tClass == \"Divination Cards\"\n\tBaseType == \"The Master Artisan\" \"Rain of Chaos\" \"Worth It\" \"Hardly Traded\" \"The Pack Leader\" \"Low Guess\"\n\tSetFontSize 40\n\tSetTextColor 0 0 0 255\n");
        let mut s = default_settings("chaos");
        for g in &mut s.groups { if g.group == Group::DivinationCards { g.hide_below = Some(4.0); } }
        let blocks = card_show_blocks(&s, &f, &segs, &[], &|_| {}, "\n");
        let lists: String = price_blocks(&s, &f, "\n") + &blocks;
        let mut segs2 = segs.clone();
        apply_with_extra_test(&mut segs2, &lists);
        let shows = |n: &str, stack: u32| {
            let i = parse_item(&format!("Item Class: Divination Cards\nRarity: Normal\n{n}\n--------\nStack Size: {stack}/5\n")).unwrap();
            status(&segs2, &i, Some(83), false).verdict.visibility == "Show"
        };
        // 5 x 1.18c and 4 x 1.18c are over 4c, 3 x isn't; a card worth 4.5c shows by itself; 5 x 0.02c doesn't.
        assert!(shows("The Master Artisan", 5) && shows("The Master Artisan", 4) && !shows("The Master Artisan", 3), "{blocks}");
        assert!(shows("Worth It", 1) && shows("Shown Already", 1) && !shows("Rain of Chaos", 5), "{blocks}");
        // Hardly traded: doesn't count, not in a stack either, unless his number says so.
        assert!(!shows("Hardly Traded", 4) && !shows("The Pack Leader", 1) && !shows("The Pack Leader", 2), "{lists}");
        let with = |min: f64| {
            let mut s = s.clone();
            for g in &mut s.groups { g.min_sold = Some(min); }
            let lists = price_blocks(&s, &f, "\n") + &card_show_blocks(&s, &f, &segs, &[], &|_| {}, "\n");
            let mut segs2 = segs.clone();
            apply_with_extra_test(&mut segs2, &lists);
            move |n: &str, stack: u32| status(&segs2, &parse_item(&format!("Item Class: Divination Cards\nRarity: Normal\n{n}\n--------\nStack Size: {stack}/5\n")).unwrap(), Some(83), false).verdict.visibility == "Show"
        };
        let all = with(0.0);
        assert!(all("The Pack Leader", 1) && all("Hardly Traded", 4) && !all("Hardly Traded", 3) && !all("Low Guess", 5));
        // The Master Artisan sold 5.7: not enough at 10.
        assert!(!with(10.0)("The Master Artisan", 5) && with(10.0)("Worth It", 1));
        assert_eq!(thin_cards(&f, 5.0).iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["The Pack Leader", "Hardly Traded"].into_iter().filter(|n| f.items.iter().any(|i| i.name == *n)).collect::<Vec<_>>());
        assert!(blocks.contains("SetFontSize 40") && !blocks.contains("Shown Already"), "its own step's look: {blocks}");
        // A step he unticked himself stays hidden.
        let off = crate::rules::Rule::Tier { type_path: vec!["divination".into()], tier: "t4c".into(), show: Some(false), actions: None, min_stack: None, min_item_tier: None, conds: None, off: false, look: None };
        assert_eq!(card_show_blocks(&s, &f, &segs, &[off], &|_| {}, "\n"), "");
    }
    fn it_card(name: &str, value: f64) -> PriceItem {
        PriceItem { name: name.into(), base: name.into(), cat: "DivinationCard".into(), value, variant: None, listings: None, trend: None, change: None, volume: None }
    }

    #[test]
    fn section_worth_line() {
        let mut f = file();
        let it = |name: &str, value: f64| PriceItem { name: name.into(), base: name.into(), cat: "Corpse".into(), value, variant: None,
            listings: Some(10), trend: None, change: None, volume: None };
        f.items.push(it("Perfect Forest Tiger", 40.0));
        f.items.push(it("Blood Demon", 1.0));
        let mut segs = parse_filter("Show # $type->corpses $tier->t1\n\tClass == \"Corpses\"\n\tBaseType == \"Perfect Forest Tiger\" \"Unpriced Corpse\"\n\tSetFontSize 45\n\nShow # $type->corpses $tier->t3\n\tClass == \"Corpses\"\n\tBaseType == \"Blood Demon\"\n\tSetFontSize 40\n");
        let mut hides = BTreeMap::new();
        hides.insert("corpses".to_string(), 4.0);
        let blocks = section_hide_blocks(&segs, &hides, &BTreeMap::new(), &f, "\n");
        assert!(blocks.contains("StackSize < 4") && blocks.contains("\"Blood Demon\"") && !blocks.contains("Tiger") && !blocks.contains("Unpriced"), "{blocks}");
        apply_with_extra_test(&mut segs, &blocks);
        let corpse = |name: &str, stack: u32| { let mut i = parse_item(&format!("Item Class: Corpses\nRarity: Normal\n{name}\n--------\nStack Size: {stack}/10\n")).unwrap(); i.item_class = "Corpses".into(); i };
        let v = status(&segs, &corpse("Blood Demon", 1), Some(83), false).verdict; println!("{} {} {:?}", v.visibility, v.decided_by, v.uncertain);
        assert_eq!(v.visibility, "Hide");
        assert_eq!(status(&segs, &corpse("Blood Demon", 5), Some(83), false).verdict.visibility, "Show");
        assert_eq!(status(&segs, &corpse("Perfect Forest Tiger", 1), Some(83), false).verdict.visibility, "Show");
    }
    fn apply_with_extra_test(segs: &mut Vec<crate::filter::Segment>, extra: &str) {
        crate::rules::apply_with_extra(segs, &[], extra, "\n");
        *segs = parse_filter(&crate::filter::render_filter(segs));
    }

    #[test]
    fn tiers_and_hide_below_in_the_filter() {
        let mut s = default_settings("chaos");
        s.enabled = true;
        s.groups.iter_mut().find(|g| g.group == Group::Uniques).unwrap().hide_below = Some(5.0);
        let blocks = price_blocks(&s, &file(), "\n");
        assert!(blocks.contains("Price tier 1: Uniques worth 100c+"));
        // Heavy Belt (only Mageblood) is tier 1. Leather Belt (Headhunter, but also 1c Wurm's Molt) goes no higher
        // than tier 2. Karui Sceptre: the 125c Replica never drops on the ground, so it's priced by Maata's Teaching (9c).
        let block = |n: &str| blocks.split(&format!("Show # Price tier {n}: Uniques")).nth(1).unwrap_or("").split("\n\n").next().unwrap().to_string();
        assert!(block("1").contains("\"Heavy Belt\"") && !block("1").contains("Leather Belt") && !block("1").contains("Karui"));
        assert!(block("2").contains("\"Leather Belt\""));
        assert!(block("3").contains("\"Karui Sceptre\""));
        assert_eq!(uniques_on_base(&file(), "Karui Sceptre"), vec![("Maata's Teaching".to_string(), 9.0)]);
        let hide = blocks.split("Hide # Price: Uniques").nth(1).unwrap().split("\n\n").next().unwrap();
        assert!(hide.contains("\"Leather Cap\"") && !hide.contains("Leather Belt"));

        // Through the whole pipeline: user edits on top, then prices, then the base filter.
        let mut segs = parse_filter("Show\n    Rarity Unique\n    SetFontSize 30\n\nShow\n    Class == \"Divination Cards\"\n    SetFontSize 30\n");
        crate::rules::apply_with_extra(&mut segs, &[], &blocks, "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let st = |clip: &str| status(&segs, &parse_item(clip).unwrap(), Some(83), false).verdict;
        let doctor = st("Item Class: Divination Cards\nRarity: Normal\nThe Doctor\n");
        assert_eq!((doctor.visibility.as_str(), doctor.style.font_size), ("Show", Some(45)));
        assert!(doctor.decided_by.starts_with("Price tier 1"));
        assert_eq!(st("Item Class: Helmets\nRarity: Unique\nGoldrim\nLeather Cap\n").visibility, "Hide");
        assert_eq!(st("Item Class: Body Armours\nRarity: Unique\nKaom's Heart\nGlorious Plate\n").style.font_size, Some(42));
        let _ = apply; // plain apply still exists for profiles without prices
    }

    #[test]
    fn card_threshold_without_price_tiers() {
        // Price tiers off, only "hide divination cards under 5c" (the Tiers page's box).
        let mut s = default_settings("chaos");
        s.groups.iter_mut().find(|g| g.group == Group::DivinationCards).unwrap().hide_below = Some(5.0);
        let blocks = price_blocks(&s, &file(), "\n");
        assert!(!blocks.contains("Show # Price tier"), "no tier looks: {blocks}");
        let mut segs = parse_filter("Show\n    Class == \"Divination Cards\"\n    SetFontSize 30\n");
        crate::rules::apply_with_extra(&mut segs, &[], &blocks, "\n");
        let segs = parse_filter(&crate::filter::render_filter(&segs));
        let card = |name: &str, n: u32| status(&segs, &parse_item(&format!(
            "Item Class: Divination Cards\nRarity: Normal\n{name}\n--------\nStack Size: {n}/8\n")).unwrap(), Some(83), false).verdict.visibility;
        let cheap: Vec<(String, f64)> = values_by_base(&file(), Group::DivinationCards).into_iter().filter(|(_, v)| *v < 5.0 && *v > 0.0).collect();
        let (name, v) = cheap.first().expect("a cheap card in the test prices").clone();
        let need = (5.0 / v).ceil() as u32;
        assert_eq!(card(&name, 1), "Hide", "{name} at {v}c alone");
        assert_eq!(card(&name, need), "Show", "{need} x {name} is worth the line");
        assert_eq!(card("The Doctor", 1), "Show");
    }

    #[test]
    fn cards_hardly_anyone_trades_count_as_worthless() {
        // From poe.ninja 21 Nov: The Calling "3c" with 6c traded (2 sold); Rain of Chaos busy; Father's Love rare but 8497c.
        let card = |name: &str, value: f64, volume: f64| PriceItem { name: name.into(), base: name.into(), cat: "DivinationCard".into(),
            value, variant: None, listings: None, trend: None, change: None, volume: Some(volume) };
        let f = PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: None, updated: String::new(), unit: "chaos".into(), bases: vec![],
            items: vec![card("The Calling", 3.08, 6.17), card("Rain of Chaos", 0.3, 900.0), card("Father's Love", 8497.0, 21000.0),
                card("A Dab of Ink", 14.9, 300.0)] };
        let v = values_by_base(&f, Group::DivinationCards);
        assert_eq!(v["The Calling"], 0.0, "2 sold: not a real price");
        assert_eq!(v["Rain of Chaos"], 0.3);
        assert_eq!(v["Father's Love"], 8497.0, "rare but valuable keeps its price");
        assert_eq!(v["A Dab of Ink"], 14.9, "20 sold is a real price");
    }

    #[test]
    fn exalted_file_in_chaos() {
        let mut f = file();
        assert_eq!(chaos_per_unit(&f), 1.0);
        f.unit = "exalted".into();
        let it = |name: &str, value: f64| PriceItem { name: name.into(), base: name.into(), cat: "Currency".into(), value, variant: None, listings: None, trend: None, change: None, volume: None };
        f.items = vec![it("Chaos Orb", 26.7), it("Divine Orb", 432.54), it("Exalted Orb", 1.0)];
        let k = chaos_per_unit(&f);
        let r = crate::trade::rates(|n| price_of(&f, None, n, "Currency").map(|v| v * k));
        assert_eq!((r["chaos"], (r["divine"] * 10.0).round() / 10.0, (r["exalted"] * 1000.0).round() / 1000.0), (1.0, 16.2, 0.037));
    }

    #[test]
    fn versions_of_a_unique() {
        let it = |name: &str, base: &str, variant: Option<&str>, value: f64| PriceItem {
            name: name.into(), base: base.into(), cat: "UniqueJewel".into(), value, variant: variant.map(String::from), listings: Some(10), trend: None, change: None, volume: None };
        let mut f = file();
        f.items.extend([it("Voices", "Large Cluster Jewel", None, 185396.0), it("Voices", "Large Cluster Jewel", Some("3 passives"), 40118.0),
            it("Voices", "Large Cluster Jewel", Some("5 passives"), 58.0), it("Voices", "Large Cluster Jewel", Some("7 passives"), 0.86),
            it("Bubonic Trail", "Murder Boots", Some("2 Jewels"), 718.8), it("Bubonic Trail", "Murder Boots", Some("1 Jewel"), 2.0),
            it("Mageblood", "Stygian Belt", Some("4 Flasks"), 44218.0), it("Mageblood", "Stygian Belt", Some("2 Flasks"), 13907.0),
            it("Ralakesh's Impatience", "Riveted Boots", Some("Endurance"), 179.7), it("Ralakesh's Impatience", "Riveted Boots", Some("Power"), 30.0),
            it("Grand Spectrum", "Cobalt Jewel", None, 178.0), it("Grand Spectrum", "Viridian Jewel", None, 40.0),
            it("Foulborn Rathpith Globe", "Titanium Spirit Shield", Some("DOT per Life"), 26793.0), it("Foulborn Rathpith Globe", "Titanium Spirit Shield", Some("Ailment per Life"), 20.0)]);
        let p = |n: &str, b: &str, t: Option<&str>| unique_price(&f, n, b, t).map(|u| (u.value, u.high));
        assert_eq!(price_of(&f, Some("Voices"), "Large Cluster Jewel", "Unique"), Some(0.86));
        assert_eq!(p("Voices", "Large Cluster Jewel", None), Some((0.86, Some(185396.0))));
        assert_eq!(p("Voices", "Large Cluster Jewel", Some("Adds 5 Small Passive Skills which grant nothing")), Some((58.0, None)));
        assert_eq!(p("Voices", "Large Cluster Jewel", Some("Adds 1 Small Passive Skill which grants nothing")), Some((185396.0, None)));
        assert_eq!(p("Bubonic Trail", "Murder Boots", Some("Has 2 Abyssal Sockets")), Some((718.8, None)));
        assert_eq!(p("Mageblood", "Stygian Belt", Some("Leftmost 4 Magic Utility Flasks constantly apply their Flask Effects to you")), Some((44218.0, None)));
        assert_eq!(p("Ralakesh's Impatience", "Riveted Boots", Some("Count as having maximum number of Power Charges")), Some((30.0, None)));
        assert_eq!(p("Grand Spectrum", "Viridian Jewel", None), Some((40.0, None)));
        // Foulborn mods can't be told apart: the cheapest, and what it could be.
        assert_eq!(p("Foulborn Rathpith Globe", "Titanium Spirit Shield", Some("Adds 9 to 12 Cold Damage")), Some((20.0, Some(26793.0))));
        assert_eq!(version_lines("Rarity: Unique\nVoices\nAdds 7 Small Passive Skills which grant nothing").as_deref(), Some("Adds 7 Small Passive Skills which grant nothing"));
        assert!(trend_of(&f, Some("Voices"), "Large Cluster Jewel", "Unique").is_none());
    }

    #[test]
    fn off_means_nothing_written() {
        assert!(price_blocks(&default_settings("chaos"), &file(), "\n").is_empty());
    }
}

#[cfg(test)]
mod trend_tests {
    use super::*;

    #[test]
    fn week_trend_for_the_priced_item() {
        let it = |name: &str, cat: &str, value: f64, trend: Option<Vec<f64>>| PriceItem {
            name: name.into(), base: name.into(), cat: cat.into(), value, variant: None, listings: Some(10),
            change: trend.as_ref().and_then(|t| t.last().copied()), trend, volume: None };
        let f = PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: None, updated: String::new(), unit: "chaos".into(), bases: vec![],
            items: vec![it("Accelerating Catalyst", "Currency", 0.49, Some(vec![0.0, -43.1, -61.9, -76.0, -72.9, -84.4, -60.9])),
                it("Arcane Surge Support", "SkillGem", 1.0, Some(vec![0.0, 5.0])), it("Chaos Orb", "Currency", 1.0, None)] };
        let t = trend_of(&f, None, "Accelerating Catalyst", "Currency").unwrap();
        assert_eq!((t.points.len(), t.change), (7, -60.9));
        assert!(trend_of(&f, None, "Chaos Orb", "Currency").is_none(), "no line in the file: nothing shown");
        assert!(trend_of(&f, None, "Arcane Surge Support", "Gem").is_none(), "gems are kept at their cheapest version");
    }
}
