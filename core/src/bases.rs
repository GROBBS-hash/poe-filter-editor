//! "Worth picking up": plain (or influenced) bases crafters pay for, from poe.ninja's base type list.
//! Those are what sellers ask, so everything shown says how many are for sale, and a base with few listings isn't
//! in the price file at all (tools/fetch_prices.py keeps 10 chaos and up with 10 or more listed).

use crate::item::{is_gear, Item};
use crate::prices::{unit_short, PriceFile, PriceSettings};
use crate::rules::style_actions;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// (base, item level, "" or "Shaper"…, chaos, how many are listed)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BasePrice(pub String, pub u32, pub String, pub f64, pub u32);

/// poe.ninja lists bases by item level from 82 up to 86 ("86 and above").
const TOP: u32 = 86;

/// What an item is worth as a base to craft on.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BaseWorth {
    pub influence: String,
    /// The item level the price is for.
    pub ilvl: u32,
    pub value: f64,
    pub listings: u32,
    /// The item level isn't known (read off the ground): this is the cheapest it could be if it's `ilvl` or more.
    pub if_level: bool,
}

/// Uniques, corrupted and mirrored items can't be crafted on; plain, magic and rare ones can.
pub fn worth(file: &PriceFile, it: &Item) -> Option<BaseWorth> {
    if it.rarity == "Unique" || it.corrupted || it.mirrored || it.influences.len() > 1 || it.kinds.iter().any(|k| k == "SynthesisedItem") || !is_gear(it) { return None; }
    let inf = it.influences.first().cloned().unwrap_or_default();
    let mine = file.bases.iter().filter(|b| b.0 == it.base_type && b.2 == inf);
    let of = |b: &BasePrice, if_level| BaseWorth { influence: inf.clone(), ilvl: b.1, value: b.3, listings: b.4, if_level };
    match it.item_level {
        Some(l) => mine.filter(|b| b.1 == l.min(TOP)).map(|b| of(b, false)).next(),
        None => mine.min_by_key(|b| b.1).map(|b| of(b, true)),
    }
}

/// A base worth making loud, from this item level up.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pick {
    pub base: String,
    pub influence: String,
    pub ilvl: u32,
    /// At that level, and how many are listed.
    pub value: f64,
    pub listings: u32,
}

/// Bases asking `min` or more, dearest first. The item level is the lowest from which every listed level asks that
/// much, so one odd price at a low level doesn't light up cheap ones.
pub fn picks(file: &PriceFile, min: f64, influenced: bool) -> Vec<Pick> {
    let mut by: BTreeMap<(&str, &str), Vec<&BasePrice>> = BTreeMap::new();
    for b in &file.bases { if influenced || b.2.is_empty() { by.entry((&b.0, &b.2)).or_default().push(b); } }
    let mut out: Vec<Pick> = by.into_iter().filter_map(|((base, inf), mut v)| {
        v.sort_by_key(|b| std::cmp::Reverse(b.1));
        // Levels missing from the list in between have too few listings to go by; a missing 86 means 85 decides.
        let ok: Vec<&&BasePrice> = v.iter().take_while(|b| b.3 >= min).collect();
        let low = ok.last()?;
        Some(Pick { base: base.into(), influence: inf.into(), ilvl: low.1, value: low.3, listings: low.4 })
    }).collect();
    out.sort_by(|a, b| b.value.total_cmp(&a.value));
    out
}

/// The filter's rules for them: one look, per item level and influence. Corrupted ones, fractured ones and six-links
/// are left to the rest of the filter.
pub fn blocks(settings: &PriceSettings, file: &PriceFile, nl: &str) -> String {
    let Some(min) = settings.bases_min.filter(|m| *m > 0.0) else { return String::new() };
    let mut by: BTreeMap<(String, std::cmp::Reverse<u32>), Vec<String>> = BTreeMap::new();
    for p in picks(file, min, settings.bases_influenced) { by.entry((p.influence, std::cmp::Reverse(p.ilvl))).or_default().push(p.base); }
    let style = settings.bases_style.clone().unwrap_or_else(|| default_style(&file.unit));
    let mut out = String::new();
    for ((inf, level), names) in by {
        out.push_str(&format!("Show # Price: bases listed at {min}{}+{}{nl}", unit_short(&file.unit), if inf.is_empty() { String::new() } else { format!(" ({inf})") }));
        out.push_str(&format!("    Rarity <= Rare{nl}    Corrupted False{nl}    Mirrored False{nl}    FracturedItem False{nl}    LinkedSockets <= 4{nl}"));
        if !inf.is_empty() { out.push_str(&format!("    HasInfluence \"{inf}\"{nl}")); }
        out.push_str(&format!("    ItemLevel >= {}{nl}", level.0));
        out.push_str(&format!("    BaseType == {}{nl}", names.iter().map(|n| format!("\"{}\"", n.replace('"', ""))).collect::<Vec<_>>().join(" ")));
        for a in style_actions(&style) { out.push_str(&format!("    {a}{nl}")); }
        out.push_str(nl);
    }
    out
}

/// The second price tier's look (yellow with a diamond) to start with.
pub fn default_style(unit: &str) -> crate::rules::CustomStyle {
    crate::prices::default_settings(unit).groups[0].tiers[1].style.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate::status, filter::parse_filter, item::parse_item, prices::default_settings};

    fn file() -> PriceFile {
        // Shaped like poe.ninja's list on 25 Nov 2026.
        let b = |n: &str, l: u32, i: &str, v: f64, c: u32| BasePrice(n.into(), l, i.into(), v, c);
        PriceFile { game: "poe1".into(), league: "Covens".into(), league_name: None, updated: String::new(), unit: "chaos".into(), items: vec![], bases: vec![
            b("Focused Amulet", 86, "", 7665.0, 86), b("Focused Amulet", 85, "", 7539.0, 265), b("Focused Amulet", 84, "", 7629.0, 48),
            b("Vaal Regalia", 86, "", 30.0, 300), b("Vaal Regalia", 84, "", 90.0, 11), b("Vaal Regalia", 86, "Shaper", 120.0, 50),
        ] }
    }

    #[test]
    fn what_a_base_is_worth() {
        let f = file();
        let it = |text: &str| parse_item(text).unwrap();
        let amulet = it("Item Class: Amulets\nRarity: Normal\nFocused Amulet\n--------\nItem Level: 88\n");
        let w = worth(&f, &amulet).unwrap();
        assert_eq!((w.ilvl, w.value, w.listings), (86, 7665.0, 86));
        let mut low = amulet.clone();
        low.item_level = Some(70);
        assert_eq!(worth(&f, &low), None, "poe.ninja has no price for a low level one");
        low.item_level = None;
        assert!(worth(&f, &low).unwrap().if_level);
        low.corrupted = true;
        assert_eq!(worth(&f, &low), None);
    }

    #[test]
    fn rules_for_dear_bases() {
        let f = file();
        assert_eq!(picks(&f, 100.0, false).iter().map(|p| (p.base.as_str(), p.ilvl)).collect::<Vec<_>>(), [("Focused Amulet", 84)]);
        assert_eq!(picks(&f, 50.0, true).len(), 2, "one odd price at 84 doesn't count when 86 is cheap");
        let mut s = default_settings("chaos");
        assert_eq!(blocks(&s, &f, "\n"), "");
        s.bases_min = Some(100.0);
        s.bases_influenced = true;
        let text = blocks(&s, &f, "\n") + "Hide\n";
        let segs = parse_filter(&text);
        let see = |t: &str| status(&segs, &parse_item(t).unwrap(), None, false).verdict.visibility == "Show";
        let want = |t: &str, show: bool| assert_eq!(see(t), show, "{t}\n{text}");
        want("Item Class: Amulets\nRarity: Normal\nFocused Amulet\n--------\nItem Level: 84\n", true);
        want("Item Class: Amulets\nRarity: Normal\nFocused Amulet\n--------\nItem Level: 83\n", false);
        want("Item Class: Body Armours\nRarity: Normal\nVaal Regalia\n--------\nItem Level: 86\n", false);
        want("Item Class: Body Armours\nRarity: Rare\nDoom Shell\nVaal Regalia\n--------\nItem Level: 86\n--------\nShaper Item\n", true);
    }
}
