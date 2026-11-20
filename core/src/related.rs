//! "Related items" for the Ctrl+D price window, like Awakened PoE Trade shows them: for a fragment, invitation
//! or boss key, the rest of its set (or the parts it's made from) and what the boss drops, with poe.ninja prices.
//! Data: `data/related_items.json`, built by `tools/build_related_items.mjs` from Awakened PoE Trade's
//! item-drop.json (MIT licence, (c) 2020 Alexander Drozdov).

use crate::prices::PriceFile;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Default, Deserialize)]
struct Data { groups: Vec<Group>, recipes: Vec<Recipe> }

#[derive(Debug, Deserialize)]
struct Group { group: Vec<String>, set: bool, drops: Vec<String> }

#[derive(Debug, Deserialize)]
struct Recipe { item: String, part: String, count: u32 }

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| serde_json::from_str(include_str!("../data/related_items.json")).unwrap_or_default())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Priced {
    pub name: String,
    /// poe.ninja price of one, in the price file's unit (chaos for PoE 1).
    pub value: Option<f64>,
    /// How many of it (recipes); 1 otherwise.
    pub count: u32,
    /// This is the item being price checked.
    pub me: bool,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Related {
    /// The item's own price, to set against its parts.
    pub own: Option<f64>,
    /// "N of this part make one X": parts list is the part, `makes` the whole.
    pub made_from: Option<Priced>,
    pub makes: Option<Priced>,
    /// Items that go with this one. `set` = all of them are needed together (e.g. the four Shaper fragments).
    pub group: Vec<Priced>,
    pub set: bool,
    /// What the boss drops (or what the set turns into), most valuable first.
    pub drops: Vec<Priced>,
}

/// The best price poe.ninja has for anything with this name (uniques with several variants take the dearest).
fn price(file: &PriceFile, name: &str) -> Option<f64> {
    file.items.iter().filter(|i| i.name == name).map(|i| i.value).reduce(f64::max)
}

fn priced(file: &PriceFile, name: &str, count: u32, me: bool) -> Priced {
    Priced { name: name.to_string(), value: price(file, name), count, me }
}

/// poe.ninja categories whose items are siblings of one kind (all threads, all fossils…).
const FAMILY_CATS: &[&str] = &["Thread", "DeliriumOrb", "Fossil", "Resonator", "Omen", "Oil", "Runegraft", "AllflameEmber", "Astrolabe"];

/// The other items of the same kind, dearest first (at most 12, always including this one), or None.
fn family(file: &PriceFile, name: &str) -> Option<Vec<Priced>> {
    let me = file.items.iter().find(|i| i.name == name)?;
    // Families told apart by name (they share a poe.ninja category with unrelated currency): "… Catalyst",
    // "Maven's Chisel of …".
    let by_name = |n: &str| -> Option<fn(&str) -> bool> {
        if n.ends_with(" Catalyst") { Some(|x| x.ends_with(" Catalyst")) }
        else if n.starts_with("Maven's Chisel of ") { Some(|x| x.starts_with("Maven's Chisel of ")) }
        else { None }
    };
    let named = by_name(name);
    let same = |i: &crate::prices::PriceItem| match named { Some(f) => f(&i.name), None => i.cat == me.cat };
    if named.is_none() && !FAMILY_CATS.contains(&me.cat.as_str()) { return None; }
    let mut all: Vec<Priced> = Vec::new();
    for i in file.items.iter().filter(|i| same(i)) {
        if !all.iter().any(|p| p.name == i.name) { all.push(priced(file, &i.name, 1, i.name == name)); }
    }
    all.sort_by(|a, b| b.value.unwrap_or(-1.0).total_cmp(&a.value.unwrap_or(-1.0)));
    if all.len() > 12 {
        let mine = all.iter().position(|p| p.me);
        let mut top: Vec<Priced> = all.iter().take(11).cloned().collect();
        if let Some(i) = mine.filter(|i| *i >= 11) { top.push(all[i].clone()) } else { top.push(all[11].clone()) }
        all = top;
    }
    (all.len() > 1).then_some(all)
}

/// A group whose names share a word ("… Oil", "… Crystallised Lifeforce") is one kind of item; one that mixes
/// whatever a league mechanic drops (Ancient Orb, Astragali, Voidborn Reliquary Key…) isn't shown as "goes with".
fn one_kind(names: &[String]) -> bool {
    let words = |n: &str| n.split(|c: char| !c.is_alphanumeric() && c != '\'').filter(|w| w.len() >= 3 && !["the", "The"].contains(w))
        .map(str::to_string).collect::<Vec<_>>();
    let first = words(&names[0]);
    first.iter().any(|w| names[1..].iter().all(|n| words(n).contains(w)))
}

/// Related items for one item name, or None when we know nothing about it.
pub fn related(file: &PriceFile, name: &str) -> Option<Related> {
    let d = data();
    let mut r = Related { own: price(file, name), ..Default::default() };
    // Threads, fossils, catalysts…: the others of the same kind, and nothing else.
    if let Some(f) = family(file, name) {
        r.group = f;
        return Some(r);
    }
    if let Some(rc) = d.recipes.iter().find(|rc| rc.item == name) {
        r.made_from = Some(priced(file, &rc.part, rc.count, false));
    }
    if let Some(rc) = d.recipes.iter().find(|rc| rc.part == name) {
        r.makes = Some(priced(file, &rc.item, rc.count, false));
    }
    if let Some(g) = d.groups.iter().find(|g| g.group.iter().any(|n| n == name)) {
        // A recipe pair (Crescent Splinter + Maven's Writ) is already shown as made-from / makes.
        let skip = |n: &str| r.made_from.as_ref().is_some_and(|p| p.name == n) || r.makes.as_ref().is_some_and(|p| p.name == n);
        let kind = !g.set && g.group.len() > 1 && one_kind(&g.group);
        // Sets (Shaper fragments…) and one-kind groups (oils, lifeforce) list their members; league-mechanic mixes don't.
        if g.set || kind {
            r.group = g.group.iter().filter(|n| !skip(n)).map(|n| priced(file, n, 1, n == name)).collect();
            if r.group.len() == 1 && !g.set { r.group.clear() }
        }
        r.set = g.set;
        // A one-kind group's "drops" are that mechanic's general loot (Harvest's for lifeforce), not this item's.
        if !kind { r.drops = g.drops.iter().map(|n| priced(file, n, 1, false)).collect(); }
        r.drops.sort_by(|a, b| b.value.unwrap_or(-1.0).total_cmp(&a.value.unwrap_or(-1.0)));
    }
    if r.made_from.is_none() && r.makes.is_none() && r.group.is_empty() && r.drops.is_empty() { None } else { Some(r) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::PriceItem;

    fn file() -> PriceFile {
        let it = |name: &str, cat: &str, value: f64| PriceItem {
            name: name.into(), base: name.into(), cat: cat.into(), value, variant: None, listings: Some(10) };
        PriceFile {
            game: "poe1".into(), league: "Covens".into(), league_name: None, updated: "now".into(), unit: "chaos".into(),
            items: vec![
                it("The Maven's Writ", "Fragment", 40.0), it("Crescent Splinter", "Fragment", 6.0),
                it("Fragment of the Minotaur", "Fragment", 18.0), it("Fragment of the Chimera", "Fragment", 20.0),
                it("Dying Sun", "UniqueFlask", 2600.0), it("Orb of Dominance", "Currency", 500.0),
                it("Polaric Invitation", "Invitation", 5.0), it("Sudden Dawn", "UniqueArmour", 3.0),
                it("Polaric Devastation", "UniqueAccessory", 30.0), it("Polaric Devastation", "UniqueAccessory", 12.0),
            ],
        }
    }

    #[test]
    fn data_loads() {
        assert!(data().groups.len() > 100 && !data().recipes.is_empty());
    }

    #[test]
    fn shaper_set_and_drops() {
        let r = related(&file(), "Fragment of the Chimera").unwrap();
        assert!(r.set);
        assert_eq!(r.group.len(), 4);
        assert!(r.group.iter().any(|p| p.me && p.name == "Fragment of the Chimera"));
        assert_eq!(r.drops[0].name, "Dying Sun");
        assert_eq!(r.drops[1].name, "Orb of Dominance");
    }

    #[test]
    fn invitation_drops_take_dearest_variant() {
        let r = related(&file(), "Polaric Invitation").unwrap();
        assert!(r.group.is_empty() && !r.set);
        assert_eq!(r.own, Some(5.0));
        assert_eq!((r.drops[0].name.as_str(), r.drops[0].value), ("Polaric Devastation", Some(30.0)));
    }

    #[test]
    fn writ_made_from_splinters() {
        let r = related(&file(), "The Maven's Writ").unwrap();
        assert_eq!(r.made_from, Some(Priced { name: "Crescent Splinter".into(), value: Some(6.0), count: 10, me: false }));
        assert!(r.group.iter().all(|p| p.name != "Crescent Splinter"));
        assert!(!r.drops.is_empty());
        let s = related(&file(), "Crescent Splinter").unwrap();
        assert_eq!(s.makes.unwrap().name, "The Maven's Writ");
        assert!(related(&file(), "Chaos Orb").is_none());
    }

    #[test]
    fn families_and_mechanic_mixes() {
        let mut f = file();
        let it = |name: &str, cat: &str, value: f64| PriceItem { name: name.into(), base: name.into(), cat: cat.into(), value, variant: None, listings: Some(10) };
        f.items.extend([it("Maven's Chisel of Avarice", "Currency", 183.0), it("Maven's Chisel of Threads", "Currency", 9.0),
            it("Bestiary Thread", "Thread", 1.6), it("Abyss Thread", "Thread", 4.8), it("Legion Thread", "Thread", 3.6),
            it("Wild Crystallised Lifeforce", "Currency", 0.02), it("Sacred Crystallised Lifeforce", "Currency", 16.0),
            it("Ancient Orb", "Currency", 13.0), it("Astragali", "Currency", 0.1), it("Headhunter", "UniqueAccessory", 48000.0)]);
        // Threads: only other threads, dearest first.
        let t = related(&f, "Bestiary Thread").unwrap();
        assert_eq!(t.group.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Abyss Thread", "Legion Thread", "Bestiary Thread"]);
        assert!(t.drops.is_empty());
        // Lifeforce: the four kinds, but not Harvest's general loot.
        let l = related(&f, "Wild Crystallised Lifeforce").unwrap();
        assert!(l.group.iter().any(|p| p.name == "Sacred Crystallised Lifeforce") && l.drops.is_empty());
        // Ancient Orb: what it rerolls into, not the rest of Expedition's currency.
        let a = related(&f, "Ancient Orb").unwrap();
        assert!(a.group.is_empty() && a.drops.iter().any(|p| p.name == "Headhunter"));
        // Maven's chisels: the other chisels, not every other currency.
        let c = related(&f, "Maven's Chisel of Avarice").unwrap();
        assert_eq!(c.group.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Maven's Chisel of Avarice", "Maven's Chisel of Threads"]);
    }
}
