//! "How would these drops look?" A set of sample drops (plus anything you've
//! made edits for), run through a filter exactly like the game would.

use crate::evaluate::{status, Style};
use crate::filter::{parse_filter, Segment};
use crate::item::{fill_class, Item};
use crate::prices::PriceFile;
use crate::profiles::{render_profile, Game, Store};
use crate::themes::{builtin_themes, level_of, Level};
use serde::Serialize;
use std::{io, path::Path};

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

/// Extra drops a theme card can show, so every kind of label has a chance in a strict filter too.
fn card_candidates(game: Game) -> Vec<Item> {
    let mut v = sample_drops(game);
    // The Divine Orb first: everyone knows how theirs looks.
    v.sort_by_key(|i| i.base_type != "Divine Orb");
    if game == Game::Poe1 {
        let c = "Stackable Currency";
        v.extend([
            it(c, "Currency", None, "Exalted Orb", Some(1), None, None),
            it(c, "Currency", None, "Orb of Annulment", Some(1), None, None),
            it(c, "Currency", None, "Stacked Deck", Some(1), None, None),
            it(c, "Currency", None, "Vaal Orb", Some(1), None, None),
            it("Viscera", "Normal", None, "Maven's Orrery", None, Some(84), None),
            it(c, "Currency", None, "Golden Oil", Some(1), None, None),
            it(c, "Currency", None, "Deafening Essence of Zeal", Some(1), None, None),
            it(c, "Currency", None, "Fractured Fossil", Some(1), None, None),
            it("Map Fragments", "Normal", None, "Simulacrum", Some(1), None, None),
            it("Map Fragments", "Normal", None, "Timeless Maraketh Emblem", Some(1), None, None),
            it("Jewels", "Rare", None, "Cobalt Jewel", None, Some(84), None),
            it("Belts", "Rare", None, "Stygian Vise", None, Some(86), None),
            it("Amulets", "Rare", None, "Onyx Amulet", None, Some(84), None),
        ]);
    }
    v
}

/// Which sample drops a theme card shows: ones this filter shows, each with a look of its own, best first. Labels the
/// theme decides come before your own edits (those look the same in every theme).
/// `probe` is the same filter with some theme on: a drop that looks the same there isn't the theme's to colour.
pub fn card_items(segs: &[Segment], probe: &[Segment], game: Game, area_level: u32, want: usize) -> Vec<Item> {
    let look = |v: &crate::evaluate::Verdict| (v.style.text, v.style.border, v.style.background);
    let drops: Vec<(Item, crate::evaluate::Verdict, bool)> = card_candidates(game).into_iter()
        .map(|i| {
            let v = status(segs, &i, Some(area_level), false).verdict;
            let own = look(&v) == look(&status(probe, &i, Some(area_level), false).verdict);
            (i, v, own)
        })
        .filter(|(_, v, _)| v.visibility != "Hide" && !v.style.label_hidden).collect();
    let mut seen = std::collections::HashSet::new();
    let mut picked: Vec<usize> = vec![];
    for mine in [false, true] {
        for (n, (_, v, own)) in drops.iter().enumerate() {
            if picked.len() < want && *own == mine && seen.insert(format!("{:?}{:?}{:?}", v.style.text, v.style.border, v.style.background)) {
                picked.push(n);
            }
        }
    }
    for n in 0..drops.len() {
        if picked.len() < want && !picked.contains(&n) { picked.push(n); }
    }
    picked.sort();
    picked.into_iter().map(|n| drops[n].0.clone()).collect()
}

/// These items through a filter, as labels.
pub fn card_labels(segs: &[Segment], items: &[Item], area_level: u32) -> Vec<PreviewDrop> {
    items.iter().map(|i| {
        let v = status(segs, i, Some(area_level), false).verdict;
        // Uniques by name here (Headhunter), though the ground shows an unidentified one's base.
        PreviewDrop { text: i.name.clone().filter(|_| i.rarity == "Unique").unwrap_or_else(|| label(i)), visibility: v.visibility, style: v.style, reason: v.reason, by_user_edit: matches!(v.source.kind.as_str(), "edit" | "price"), item: i.clone() }
    }).collect()
}

/// One theme card's labels ("" = NeverSink's own colours).
#[derive(Debug, Clone, Serialize)]
pub struct ThemeCard {
    pub name: String,
    pub labels: Vec<PreviewDrop>,
}

/// The Themes page: every theme's card, and one of this filter's drops for each of the six value levels (Chase … Junk;
/// None when none of the sample drops is at that level).
#[derive(Debug, Clone, Serialize)]
pub struct ThemeCards {
    pub cards: Vec<ThemeCard>,
    pub examples: Vec<Option<String>>,
}

/// A drop of this filter's at each value level, for the theme editor.
fn level_examples(segs: &[Segment], game: Game, area_level: u32) -> Vec<Option<String>> {
    let mut out: Vec<Option<String>> = vec![None; Level::ALL.len()];
    for i in card_candidates(game) {
        let v = status(segs, &i, Some(area_level), false).verdict;
        if v.visibility == "Hide" || v.style.label_hidden || v.source.kind != "neversink" { continue; }
        let n = Level::ALL.iter().position(|l| *l == level_of(&v.style)).unwrap_or(5);
        if out[n].is_none() { out[n] = Some(i.name.clone().filter(|_| i.rarity == "Unique").unwrap_or_else(|| label(&i))); }
    }
    out
}

/// What every theme does to this filter: the same handful of drops, run through the whole filter (its edits, tiers and
/// prices) with each theme in turn, like the game would.
pub fn theme_cards(store: &Store, id: &str, dir: &Path, prices: Option<&PriceFile>, area_level: u32) -> io::Result<ThemeCards> {
    let game = store.profiles.get(id).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "profile not found"))?.game;
    let code = if game == Game::Poe1 { "poe1" } else { "poe2" };
    let plain = parse_filter(&render_profile(store, id, dir, prices, Some(None))?.0);
    let probe = parse_filter(&render_profile(store, id, dir, prices, Some(Some("Classic PoE")))?.0);
    let items = card_items(&plain, &probe, game, area_level, 8);
    let mut out = vec![ThemeCard { name: String::new(), labels: card_labels(&plain, &items, area_level) }];
    let names: Vec<String> = builtin_themes().into_iter().chain(store.themes.iter().cloned())
        .filter(|t| t.game.as_deref().is_none_or(|g| g == code)).map(|t| t.name).collect();
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| for _ in 0..workers {
        s.spawn(|| loop {
            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some(name) = names.get(i) else { break };
            let Ok((text, _)) = render_profile(store, id, dir, prices, Some(Some(name.as_str()))) else { continue };
            let labels = card_labels(&parse_filter(&text), &items, area_level);
            done.lock().unwrap().push((i, ThemeCard { name: name.clone(), labels }));
        });
    });
    let mut done = done.into_inner().unwrap();
    done.sort_by_key(|(i, _)| *i);
    out.extend(done.into_iter().map(|(_, c)| c));
    // A label that looks the same in every theme is one of your own looks (an edit, a tier change, a price tier).
    let colours = |l: &PreviewDrop| (l.style.text, l.style.border, l.style.background);
    for n in 0..items.len() {
        let same = out.iter().all(|c| colours(&c.labels[n]) == colours(&out[0].labels[n]));
        if same { for c in &mut out { c.labels[n].by_user_edit = true; } }
    }
    Ok(ThemeCards { cards: out, examples: level_examples(&plain, game, area_level) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dev look at his real filters' theme cards: `cargo test -p filter-core --lib theme_cards_real -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn theme_cards_real() {
        let data = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("com.grobbs.poefiltereditor");
        let store: Store = serde_json::from_str(&std::fs::read_to_string(data.join("profiles.json")).unwrap()).unwrap();
        let prices: PriceFile = serde_json::from_str(&std::fs::read_to_string(data.join("prices_poe1.json")).unwrap()).unwrap();
        let dir = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap()).join("Documents/My Games/Path of Exile");
        for (id, p) in store.profiles.iter().filter(|(_, p)| p.game == Game::Poe1) {
            let t = std::time::Instant::now();
            let ThemeCards { cards, examples } = theme_cards(&store, id, &dir, Some(&prices), 83).unwrap();
            println!("{examples:?}");
            if let Ok(f) = std::env::var("THEME_CARDS_OUT") { std::fs::write(f, serde_json::to_string(&ThemeCards { cards: cards.clone(), examples: examples.clone() }).unwrap()).unwrap(); }
            println!("== {} ({:?}), {} themes in {:?}", p.name, p.theme, cards.len(), t.elapsed());
            for c in cards.iter().take(2) {
                for l in &c.labels { println!("  {:22} {:24} {} {:?} {:?} {:?} mine {}", c.name, l.text, l.visibility, l.style.text, l.style.border, l.style.background, l.by_user_edit); }
            }
        }
    }

    #[test]
    fn card_items_are_shown_and_different() {
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        let segs = parse_filter(&t);
        let mut probe = parse_filter(&t);
        crate::themes::apply_theme(&mut probe, builtin_themes().iter().find(|t| t.name == "Classic PoE").expect("the probe theme"));
        let items = card_items(&segs, &probe, Game::Poe1, 83, 8);
        let labels = card_labels(&segs, &items, 83);
        assert_eq!(labels.len(), 8);
        assert!(labels.iter().all(|l| l.visibility != "Hide"));
        let looks: std::collections::HashSet<_> = labels.iter().map(|l| format!("{:?}{:?}{:?}", l.style.text, l.style.border, l.style.background)).collect();
        assert!(looks.len() >= 6, "{looks:?}");
    }

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
