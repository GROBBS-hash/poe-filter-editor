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
    use filter_core::{evaluate::status, filter::{parse_filter, render_filter}, item::parse_item, profiles::Game, rules::*, search::*};
    let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
    let style = CustomStyle { font_size: 42, text: [0, 255, 255, 255], border: None, background: [0, 0, 0, 255],
        beam: None, minimap: None, sound: Sound::Default, label_hidden: false };
    let build = |o: &LevelOptions| {
        let mut segs = parse_filter(&t);
        apply(&mut segs, &level_rules_opts(Game::Poe1, o, &style), "\r\n");
        parse_filter(&render_filter(&segs))
    };
    let item = |s: &str| parse_item(s).unwrap();
    let o = LevelOptions { build: "spells".into(), keep: vec!["Wands".into(), "Sceptres".into()],
        armour_types: vec!["es".into(), "ev_es".into()], ..Default::default() };
    let segs = build(&o);
    let v = |i: &filter_core::item::Item, lvl| status(&segs, i, Some(lvl), false).verdict;
    let axe = item("Item Class: Two Hand Axes\nRarity: Normal\nStone Axe\n--------\nItem Level: 10\n");
    let wand = item("Item Class: Wands\nRarity: Magic\nDriftwood Wand\n--------\nItem Level: 10\n");
    let rare_wand = item("Item Class: Wands\nRarity: Rare\nX\nDriftwood Wand\n--------\nItem Level: 20\n");
    let rare_axe = item("Item Class: Two Hand Axes\nRarity: Rare\nX\nStone Axe\n--------\nItem Level: 10\n");
    let uniq = item("Item Class: Two Hand Axes\nRarity: Unique\nLimbsplit\nWoodsplitter\n--------\nItem Level: 10\n");
    let linked = item("Item Class: Two Hand Axes\nRarity: Normal\nStone Axe\n--------\nSockets: R-R-G \n--------\nItem Level: 10\n");
    let es = item("Item Class: Helmets\nRarity: Rare\nX\nVine Circlet\n--------\nItem Level: 20\n");
    let ar = item("Item Class: Helmets\nRarity: Rare\nX\nIron Hat\n--------\nItem Level: 20\n");
    let ar_white = item("Item Class: Helmets\nRarity: Normal\nIron Hat\n--------\nItem Level: 20\n");

    assert_eq!(v(&axe, 10).visibility, "Hide");
    assert_eq!(v(&wand, 10).style.font_size, Some(42));
    assert_eq!(v(&wand, 10).reason, "Levelling (Spells & Minions): your weapons stand out");
    assert_ne!(v(&uniq, 10).visibility, "Hide");
    // Vendor rares on for 1-24 by default: rare axe shows at 10, hidden at 30.
    assert!(!v(&rare_axe, 10).by_user_edit);
    assert_eq!(v(&rare_axe, 30).visibility, "Hide");
    // Rares only from 16 (default): white/blue wand hidden at 20, rare wand stands out.
    assert_eq!(v(&wand, 20).visibility, "Hide");
    assert_eq!(v(&rare_wand, 20).style.font_size, Some(42));
    // 3-links spared until 28.
    assert!(!v(&linked, 20).by_user_edit);
    assert!(v(&linked, 35).by_user_edit);
    // Armour types.
    assert!(!v(&es, 30).by_user_edit);
    assert!(!v(&ar, 20).by_user_edit, "vendor rares");
    assert_eq!(v(&ar, 30).visibility, "Hide");
    assert_eq!(v(&ar_white, 10).visibility, "Hide");
    // Read off the ground (no sockets visible): still hidden.
    assert_eq!(status(&segs, &item("Item Class: Warstaves\nRarity: Rare\nFoul Staff\n"), Some(38), true).verdict.visibility, "Hide");
    // Maps untouched.
    assert!(!v(&wand, 80).by_user_edit && !v(&ar, 80).by_user_edit && !v(&axe, 80).by_user_edit);

    // Item progression for wands: a white Quartz Wand stands out at area 20 even though white wands are hidden then.
    let segs2 = build(&LevelOptions { progression: vec!["Wands".into()], ..o.clone() });
    let quartz = item("Item Class: Wands\nRarity: Normal\nQuartz Wand\n--------\nItem Level: 20\n");
    let v2 = status(&segs2, &quartz, Some(20), false).verdict;
    assert_eq!((v2.visibility.as_str(), v2.reason.as_str()), ("Show", "Levelling (Spells & Minions): Wands progression"));
    assert_eq!(status(&segs2, &quartz, Some(40), false).verdict.visibility, "Hide");

    // Off the ground: a white Hussar Brigandine (AR/EV, unpicked) with NeverSink's 4-link look
    // (white text, dark background) is read as a white 4-link, not a unique.
    {
        use filter_core::ground::{ground_item, GroundMatch, LabelColours};
        let m = GroundMatch { base: "Hussar Brigandine".into(), stack: None, tier: None, superior: false, confidence: 1.0 };
        let it = ground_item(&segs, &m, Some(LabelColours { text: [200, 200, 200], background: [20, 20, 0] }), 42);
        assert_eq!((it.rarity.as_str(), it.sockets.as_deref()), ("Normal", Some("R-R-R-R")), "{it:?}");
        let st = status(&segs, &it, Some(42), true);
        assert_ne!(st.verdict.visibility, "Hide");
        assert!(st.assumptions.iter().any(|a| a.contains("4-linked")), "{:?}", st.assumptions);
        // A plain unlinked look stays a plain guess.
        let rare = ground_item(&segs, &GroundMatch { base: "Vine Circlet".into(), ..m.clone() },
            Some(LabelColours { text: [255, 255, 119], background: [0, 0, 0] }), 42);
        assert_eq!((rare.rarity.as_str(), rare.sockets.as_deref()), ("Rare", None));
    }

    // Aggressive: linked gear hidden too.
    let segs = build(&LevelOptions { aggressive: true, ..o.clone() });
    assert_eq!(status(&segs, &linked, Some(20), false).verdict.visibility, "Hide");
}
