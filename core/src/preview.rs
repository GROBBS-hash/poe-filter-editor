//! "How would these drops look?" A set of sample drops (plus anything you've
//! made edits for), run through a filter exactly like the game would.

use crate::evaluate::{status, Style};
use crate::filter::Segment;
use crate::item::{fill_class, Item};
use crate::profiles::Game;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PreviewDrop {
    /// The label text as the game shows it, e.g. "3x Chaos Orb".
    pub text: String,
    pub item: Item,
    pub visibility: String,
    pub style: Style,
    /// Plain-English reason, or your edit's name.
    pub reason: String,
    pub by_user_edit: bool,
}

fn it(class: &str, rarity: &str, name: Option<&str>, base: &str, stack: Option<u32>, ilvl: Option<u32>, quality: Option<u32>) -> Item {
    let mut i = Item {
        item_class: class.into(), rarity: rarity.into(), name: name.map(Into::into), base_type: base.into(),
        stack_size: stack, item_level: ilvl, quality,
        unidentified: matches!(rarity, "Magic" | "Rare" | "Unique"), ..Default::default()
    };
    fill_class(&mut i);
    i
}

/// A spread of typical drops, from chase items down to junk.
pub fn sample_drops(game: Game) -> Vec<Item> {
    let c = "Stackable Currency";
    match game {
        Game::Poe1 => vec![
            it(c, "Currency", None, "Mirror of Kalandra", Some(1), None, None),
            it(c, "Currency", None, "Divine Orb", Some(1), None, None),
            it("Divination Cards", "Normal", None, "The Doctor", Some(1), None, None),
            it("Belts", "Unique", Some("Headhunter"), "Leather Belt", None, Some(84), None),
            it(c, "Currency", None, "Chaos Orb", Some(3), None, None),
            it(c, "Currency", None, "Orb of Alchemy", Some(1), None, None),
            it("Body Armours", "Rare", None, "Vaal Regalia", None, Some(86), None),
            it("Rings", "Rare", None, "Coral Ring", None, Some(75), None),
            it("Wands", "Rare", None, "Opal Wand", None, Some(75), None),
            it("Skill Gems", "Gem", None, "Lightning Tendrils", None, None, Some(20)),
            it("Helmets", "Unique", Some("Goldrim"), "Leather Cap", None, Some(20), None),
            it(c, "Currency", None, "Orb of Transmutation", Some(1), None, None),
            it("Gold", "Currency", None, "Gold", Some(900), None, None),
            it("Gold", "Currency", None, "Gold", Some(40), None, None),
            it("Divination Cards", "Normal", None, "Rain of Chaos", Some(1), None, None),
            it("Skill Gems", "Gem", None, "Fireball", None, None, None),
            it("Rings", "Magic", None, "Sapphire Ring", None, Some(70), None),
            it("Boots", "Normal", None, "Conjurer Boots", None, Some(70), None),
            it(c, "Currency", None, "Portal Scroll", Some(1), None, None),
            it(c, "Currency", None, "Scroll of Wisdom", Some(2), None, None),
        ],
        Game::Poe2 => vec![
            it(c, "Currency", None, "Divine Orb", Some(1), None, None),
            it(c, "Currency", None, "Exalted Orb", Some(2), None, None),
            it(c, "Currency", None, "Chaos Orb", Some(1), None, None),
            it(c, "Currency", None, "Regal Orb", Some(1), None, None),
            it(c, "Currency", None, "Orb of Augmentation", Some(1), None, None),
            it(c, "Currency", None, "Orb of Transmutation", Some(1), None, None),
            it("Gold", "Currency", None, "Gold", Some(300), None, None),
            it("Gold", "Currency", None, "Gold", Some(20), None, None),
            it(c, "Currency", None, "Scroll of Wisdom", Some(1), None, None),
        ],
    }
}

/// The label text the game shows on the ground.
fn label(i: &Item) -> String {
    let name = match (&i.name, i.rarity.as_str()) {
        (Some(n), "Unique") if !i.unidentified => n.clone(),
        _ => i.base_type.clone(),
    };
    let name = if i.quality.unwrap_or(0) > 0 && i.rarity == "Gem" { format!("Superior {name}") } else { name };
    match i.stack_size {
        Some(n) if n > 1 && i.item_class == "Gold" => format!("{n} Gold"),
        Some(n) if n > 1 => format!("{n}x {name}"),
        _ => name,
    }
}

/// Runs each item through the filter. `extra` (your edited items) go first.
pub fn preview(segs: &[Segment], extra: &[Item], game: Game, area_level: u32) -> Vec<PreviewDrop> {
    let mut items: Vec<Item> = extra.to_vec();
    for s in sample_drops(game) {
        if !items.iter().any(|x| x.base_type == s.base_type && x.rarity == s.rarity && x.stack_size == s.stack_size) {
            items.push(s);
        }
    }
    items.into_iter().map(|item| {
        let st = status(segs, &item, Some(area_level), false);
        PreviewDrop {
            text: label(&item),
            visibility: st.verdict.visibility.clone(),
            style: st.verdict.style.clone(),
            reason: st.verdict.reason.clone(),
            by_user_edit: st.verdict.by_user_edit,
            item,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::parse_filter;

    #[test]
    fn real_filter_preview() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        let segs = parse_filter(&t);
        for lvl in [30, 80] {
            let p = preview(&segs, &[], Game::Poe1, lvl);
            let shown: Vec<&str> = p.iter().filter(|d| d.visibility != "Hide").map(|d| d.text.as_str()).collect();
            let hidden: Vec<&str> = p.iter().filter(|d| d.visibility == "Hide").map(|d| d.text.as_str()).collect();
            println!("area {lvl}: shown {shown:?}\\n         hidden {hidden:?}");
            assert!(shown.contains(&"Mirror of Kalandra") && shown.contains(&"3x Chaos Orb"));
        }
        let p = preview(&segs, &[], Game::Poe1, 80);
        assert!(p.iter().any(|d| d.text == "Superior Lightning Tendrils"));
        assert!(p.iter().any(|d| d.text == "900 Gold"));
    }
}
