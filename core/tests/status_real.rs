use filter_core::{evaluate::status, filter::parse_filter, item::{fill_class, parse_item}};
const SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter");

fn show(clip: &str, ground: bool) {
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let mut it = parse_item(clip).unwrap();
    fill_class(&mut it);
    let s = status(&segs, &it, Some(83), ground);
    println!("{:<28} -> {:5} by {:<40} font {:?} sound {:?} muted {} stacks {:?} unsure {}",
        parse_item(clip).unwrap().base_type, s.verdict.visibility, s.verdict.decided_by,
        s.verdict.style.font_size, s.verdict.style.sound, s.verdict.style.muted,
        s.by_stack.iter().map(|n| format!("{}+:{}", n.from, n.visibility)).collect::<Vec<_>>(), s.verdict.uncertain.len());
}

#[test]
fn real_items() {
    show("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n--------\nStack Size: 1/20\n", false);
    show("Item Class: Stackable Currency\nRarity: Currency\nScroll of Wisdom\n--------\nStack Size: 3/40\n", false);
    show("Item Class: Stackable Currency\nRarity: Currency\nOrb of Transmutation\n--------\nStack Size: 1/40\n", false);
    show("Item Class: Stackable Currency\nRarity: Currency\nChaos Orb\n", false);
    show("Item Class: Rings\nRarity: Magic\nCoral Ring\n--------\nItem Level: 75\n", false);
    show("Item Class: Rings\nRarity: Rare\nDoom Loop\nCoral Ring\n--------\nItem Level: 84\n--------\nUnidentified\n", false);
    show("Item Class: Belts\nRarity: Unique\nHeadhunter\nLeather Belt\n--------\nItem Level: 84\n--------\nUnidentified\n", false);
    show("Item Class: Body Armours\nRarity: Normal\nPlate Vest\n--------\nItem Level: 80\n", false);
    show("Item Class: \nRarity: Rare\nCoral Ring\n", true);
}

#[test]
fn why_unsure() {
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let s = status(&segs, &parse_item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n").unwrap(), Some(83), false);
    for u in &s.verdict.uncertain { println!("UNSURE {u}"); }
}

#[test]
fn ground_currency_is_certain() {
    use filter_core::ground::{ground_item, GroundMatch};
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let m = GroundMatch { base: "Orb of Regret".into(), stack: None, tier: None, superior: false, confidence: 1.0 };
    let it = ground_item(&segs, &m, None, 83);
    let s = status(&segs, &it, Some(83), true);
    assert_eq!(s.verdict.visibility, "Show");
    assert!(s.verdict.uncertain.is_empty(), "{:?}", s.verdict.uncertain);
    assert!(s.assumptions.is_empty(), "{:?}", s.assumptions);
}

#[test]
fn ground_superior_gem_is_shown() {
    use filter_core::ground::{ground_item, known_names, match_base};
    use filter_core::rules::known_bases;
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let m = match_base("JPERIOR LIGHTNING TENDRILS", &known_names(&known_bases(&segs))).unwrap();
    let it = ground_item(&segs, &m, None, 83);
    let s = status(&segs, &it, Some(83), true);
    println!("gem: {} by {} class {}", s.verdict.visibility, s.verdict.decided_by, it.item_class);
    assert_eq!(s.verdict.visibility, "Show");
}

#[test]
fn neversink_hides_plain_normal_boots_everywhere() {
    use filter_core::ground::{ground_item, GroundMatch};
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let m = GroundMatch { base: "Conjurer Boots".into(), stack: None, tier: None, superior: false, confidence: 1.0 };
    for lvl in [30, 60, 68, 83] {
        let it = filter_core::item::Item { rarity: "Normal".into(), ..ground_item(&segs, &m, None, lvl) };
        assert_eq!(status(&segs, &it, Some(lvl), true).verdict.visibility, "Hide");
    }
}

#[test]
fn levelling_preset_on_real_filter() {
    use filter_core::{evaluate::status, filter::{parse_filter, render_filter}, item::parse_item, profiles::Game, rules::*, search::level_rules};
    let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
    let mut segs = parse_filter(&t);
    let style = CustomStyle { font_size: 42, text: [0, 255, 255, 255], border: None, background: [0, 0, 0, 255],
        beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
    apply(&mut segs, &level_rules("caster", Game::Poe1, &style), "\r\n");
    let segs = parse_filter(&render_filter(&segs));
    let axe = parse_item("Item Class: Two Hand Axes\nRarity: Normal\nStone Axe\n--------\nItem Level: 10\n").unwrap();
    let wand = parse_item("Item Class: Wands\nRarity: Magic\nDriftwood Wand\n--------\nItem Level: 10\n").unwrap();
    let uniq = parse_item("Item Class: Two Hand Axes\nRarity: Unique\nLimbsplit\nWoodsplitter\n--------\nItem Level: 10\n").unwrap();
    let v = |i, lvl| status(&segs, i, Some(lvl), false).verdict;
    assert_eq!(v(&axe, 10).visibility, "Hide", "{:?}", v(&axe, 10));
    assert_ne!(v(&wand, 10).visibility, "Hide");
    assert_eq!(v(&wand, 10).style.font_size, Some(42));
    assert_ne!(v(&uniq, 10).visibility, "Hide");
    assert_ne!(v(&wand, 80).style.font_size, Some(42));
    println!("axe in maps: {:?}", v(&axe, 80).reason);
    assert_eq!(v(&wand, 10).reason, "Levelling (Caster): your weapons stand out");
    // Read off the ground (no sockets visible): a rare warstave is still hidden for a caster.
    let foul = parse_item("Item Class: Warstaves\nRarity: Rare\nFoul Staff\n").unwrap();
    assert_eq!(status(&segs, &foul, Some(38), true).verdict.visibility, "Hide");

    // Rares only from area 15, armour too.
    let mut segs = parse_filter(&t);
    let o = filter_core::search::LevelOptions { keep: vec!["Wands".into(), "Sceptres".into()], rares_from: 15, hide_armour: true };
    apply(&mut segs, &filter_core::search::level_rules_opts("caster", Game::Poe1, &o, &style), "\r\n");
    let segs = parse_filter(&render_filter(&segs));
    let v = |i, lvl| status(&segs, i, Some(lvl), false).verdict;
    let rare_wand = parse_item("Item Class: Wands\nRarity: Rare\nX\nDriftwood Wand\n--------\nItem Level: 20\n").unwrap();
    let boots = parse_item("Item Class: Boots\nRarity: Normal\nIron Greaves\n--------\nItem Level: 20\n").unwrap();
    assert_ne!(v(&wand, 10).visibility, "Hide");
    assert_eq!(v(&wand, 20).visibility, "Hide");
    assert_eq!(v(&rare_wand, 20).style.font_size, Some(42));
    assert_eq!(v(&boots, 20).visibility, "Hide");
    let linked = parse_item("Item Class: Two Hand Axes\nRarity: Normal\nStone Axe\n--------\nSockets: R-R-G \n--------\nItem Level: 10\n").unwrap();
    assert!(!v(&linked, 20).by_user_edit, "3-links spared until 28: {:?}", v(&linked, 20));
    assert!(v(&linked, 35).by_user_edit);
    assert!(!v(&wand, 80).by_user_edit && !v(&boots, 80).by_user_edit, "maps untouched");
}
