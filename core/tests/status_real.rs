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
    let m = GroundMatch { base: "Orb of Regret".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
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
    let m = GroundMatch { base: "Conjurer Boots".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
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
    // A white 3-linked wand: kept until area 28, hidden after (wands can't be 4-linked).
    let wand3 = item("Item Class: Wands\nRarity: Normal\nDriftwood Wand\n--------\nSockets: B-B-B \n--------\nItem Level: 20\n");
    assert!(!v(&wand3, 20).by_user_edit || v(&wand3, 20).visibility != "Hide");
    assert_eq!(v(&wand3, 48).visibility, "Hide", "{:?}", v(&wand3, 48));
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
        let m = GroundMatch { base: "Hussar Brigandine".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
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

/// Tiers page chips (23 Nov): "Rare jewels" set to item level 84+ and "3+ mods" really changes what shows.
#[test]
fn jewel_step_conditions() {
    use filter_core::rules::{apply, Rule};
    let mut segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let jewel = |ilvl: u32| { let mut it = parse_item(&format!("Item Class: Jewels\nRarity: Rare\nGloom Spark\nCobalt Jewel\n--------\nItem Level: {ilvl}\n--------\nUnidentified\n")).unwrap(); fill_class(&mut it); it };
    let before = status(&segs, &jewel(70), Some(83), false).verdict;
    assert_eq!((before.visibility.as_str(), before.decided_by.contains("anyrare")), ("Show", true), "{}", before.decided_by);
    let mut c = std::collections::BTreeMap::new();
    c.insert("ItemLevel".to_string(), ">= 84".to_string());
    let rule = Rule::Tier { type_path: vec!["jewels".into(), "generic".into()], tier: "anyrare".into(), show: None, actions: None, min_stack: None, min_item_tier: None, conds: Some(c), off: false, look: None };
    assert!(apply(&mut segs, &[rule], "\n").is_empty());
    let low = status(&segs, &jewel(70), Some(83), false).verdict;
    let high = status(&segs, &jewel(85), Some(83), false).verdict;
    println!("ilvl 70 -> {} by {}; ilvl 85 -> {} by {}", low.visibility, low.decided_by, high.visibility, high.decided_by);
    assert!(!low.decided_by.contains("anyrare"));
    assert!(high.decided_by.contains("anyrare"));
}

/// Gear chips (23 Nov): "Tier 1 bases, item level 86+" lowered to 84 catches an item level 84 Crystal Belt.
#[test]
fn gear_step_conditions() {
    use filter_core::rules::{apply, Rule};
    let mut segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let belt = || { let mut it = parse_item("Item Class: Belts\nRarity: Normal\nCrystal Belt\n--------\nItem Level: 84\n").unwrap(); fill_class(&mut it); it };
    let before = status(&segs, &belt(), Some(83), false).verdict;
    assert!(!before.decided_by.contains("t1_86"), "{}", before.decided_by);
    let mut c = std::collections::BTreeMap::new();
    c.insert("ItemLevel".to_string(), ">= 84".to_string());
    let rule = Rule::Tier { type_path: vec!["crafting".into(), "generalgear".into()], tier: "t1_86".into(), show: None, actions: None, min_stack: None, min_item_tier: None, conds: Some(c), off: false, look: None };
    assert!(apply(&mut segs, &[rule], "\n").is_empty());
    let after = status(&segs, &belt(), Some(83), false).verdict;
    println!("before {} by {}; after {} by {}", before.visibility, before.decided_by, after.visibility, after.decided_by);
    assert!(after.decided_by.contains("t1_86"), "{}", after.decided_by);
}

/// F2 on a rare Twilight Regalia label (23 Nov): the label has no item level, and without one NeverSink hides rares in
/// maps, so "rare" used to look unlikely and the reader picked the base's unique (Mantle of Dreams). Now it's rare,
/// and the hover box's "rare" isn't overruled either.
#[test]
fn ground_rare_isnt_read_as_its_unique() {
    use filter_core::ground::{apply_tooltip_checked, ground_item, GroundMatch, LabelColours, TooltipRead};
    use filter_core::item::Item;
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    // How the filter draws an item level 86 rare Twilight Regalia (what he saw on the ground).
    let mut drawn = Item { item_class: "Body Armours".into(), base_type: "Twilight Regalia".into(), rarity: "Rare".into(), item_level: Some(86), unidentified: true, ..Default::default() };
    fill_class(&mut drawn);
    let v = status(&segs, &drawn, Some(83), true).verdict;
    let rgb = |c: Option<[u8; 4]>, d: [u8; 3]| c.map(|c| [c[0], c[1], c[2]]).unwrap_or(d);
    let bg = v.style.background.map(|c| { let a = c[3] as u32; [(c[0] as u32 * a / 255) as u8, (c[1] as u32 * a / 255) as u8, (c[2] as u32 * a / 255) as u8] }).unwrap_or([0, 0, 0]);
    let seen = LabelColours { text: rgb(v.style.text, [255, 255, 119]), background: bg };
    let m = GroundMatch { base: "Twilight Regalia".into(), stack: None, tier: None, superior: false, confidence: 1.0, unique: None };
    let mut it = ground_item(&segs, &m, Some(seen), 83);
    assert_ne!(it.rarity, "Unique", "label {seen:?} drawn by {}", v.decided_by);
    let t = TooltipRead { found: m.clone(), item_level: Some(86), unidentified: true, rarity: Some("Rare".into()), header: "Twilight Regalia".into() };
    apply_tooltip_checked(&segs, &mut it, &t, Some(seen), 83);
    assert_eq!((it.rarity.as_str(), it.item_level), ("Rare", Some(86)));
}

/// Dev check on his own built filter (23 Nov): who decides a rare Originator T16 map? His F2 edit "Rare Map (Tier 16)"
/// covers every T16 map, so the Map tiers page's Originator look never shows (the "Your edit wins" note).
/// `LODESTONE_FILTER="<path to Red-Maps.filter>" cargo test -p filter-core --test status_real originator -- --ignored --nocapture`
#[test]
#[ignore]
fn originator_t16_decided_by_edit() {
    let path = std::env::var("LODESTONE_FILTER").expect("LODESTONE_FILTER");
    let segs = parse_filter(&std::fs::read_to_string(path).unwrap());
    let mut it = parse_item("Item Class: Maps\nRarity: Rare\nGrim Trail\nMap (Tier 16)\n--------\nMap Tier: 16\n--------\nItem Level: 83\n--------\nArea is influenced by the Originator's Memories (implicit)\n").unwrap();
    fill_class(&mut it);
    assert!(it.zana_memory);
    let s = status(&segs, &it, Some(83), false);
    println!("decided by {:?} (user edit: {})", s.verdict.decided_by, s.verdict.by_user_edit);
    assert!(s.verdict.by_user_edit);
}

/// Dev check (24 Nov): his Blasting filter (built on Red-Maps) shows Stacks of 6+ Tier 5 again.
/// `cargo test -p filter-core --test status_real blasting_tier5 -- --ignored --nocapture`
#[test]
#[ignore]
fn blasting_tier5() {
    let appdata = std::env::var("APPDATA").unwrap();
    let store = filter_core::profiles::Store::load(std::path::Path::new(&format!("{appdata}/com.grobbs.poefiltereditor/profiles.json")));
    let dir = std::path::PathBuf::from(format!("{}/Documents/My Games/Path of Exile", std::env::var("USERPROFILE").unwrap()));
    let (text, warnings) = filter_core::profiles::render_profile(&store, "blasting", &dir, None, None).unwrap();
    let line = text.lines().find(|l| l.contains("$type->currency->stackedsix $tier->t5")).unwrap_or("not found");
    println!("{line}\nwarnings: {warnings:?}");
    assert!(line.starts_with("Show"), "{line}");
}

/// Dev check (24 Nov): what F2's "Reset to NeverSink's" finds for 15 Orb of Alchemy on Blasting.
#[test]
#[ignore]
fn blasting_reset_alchemy() {
    use filter_core::{filter::parse_filter, profiles::{render_profile, Store}, rules::{restore, touching}};
    let appdata = std::env::var("APPDATA").unwrap();
    let mut store = Store::load(std::path::Path::new(&format!("{appdata}/com.grobbs.poefiltereditor/profiles.json")));
    let dir = std::path::PathBuf::from(format!("{}/Documents/My Games/Path of Exile", std::env::var("USERPROFILE").unwrap()));
    let item = filter_core::item::parse_item("Item Class: Stackable Currency\nRarity: Currency\nOrb of Alchemy\n--------\nStack Size: 15/40\n").unwrap();
    let verdict = |s: &Store| filter_core::evaluate::status(&parse_filter(&render_profile(s, "blasting", &dir, None, None).unwrap().0), &item, Some(83), false).verdict;
    let v = verdict(&store);
    println!("now: {} by {:?} {:?}", v.visibility, v.source.type_path, v.source.tier);
    let inherited = store.resolved_rules("red-maps");
    let orig = {
        let p = store.profiles.get_mut("blasting").unwrap();
        let (r, par) = (std::mem::take(&mut p.rules), p.parent.take());
        let o = parse_filter(&render_profile(&store, "blasting", &dir, None, None).unwrap().0);
        let p = store.profiles.get_mut("blasting").unwrap(); p.rules = r; p.parent = par; o
    };
    let tier = v.source.tier.clone().unwrap();
    let own = &mut store.profiles.get_mut("blasting").unwrap().rules;
    let mine = touching(own, &v.source.type_path, &tier, "Orb of Alchemy");
    for i in mine.iter().rev() { own.remove(*i); }
    let back = restore(&orig, &inherited, &v.source.type_path, &tier, "Orb of Alchemy");
    println!("removes {} of Blasting's, adds {:?}", mine.len(), back.iter().map(|r| format!("{r:?}").chars().take(90).collect::<String>()).collect::<Vec<_>>());
    own.extend(back);
    let after = verdict(&store);
    println!("after reset: {} by {:?} {:?}", after.visibility, after.source.type_path, after.source.tier);
}

/// Dev check (24 Nov): from what stack each Thread shows in his filters, and what hides the rest.
/// `cargo test -p filter-core --test status_real threads_show -- --ignored --nocapture`
#[test]
#[ignore]
fn threads_show() {
    use filter_core::{filter::parse_filter, prices::{thread_prices, PriceFile}, profiles::{render_profile, Store}};
    let appdata = format!("{}/com.grobbs.poefiltereditor", std::env::var("APPDATA").unwrap());
    let store = Store::load(std::path::Path::new(&format!("{appdata}/profiles.json")));
    let file: PriceFile = serde_json::from_str(&std::fs::read_to_string(format!("{appdata}/prices_poe1.json")).unwrap()).unwrap();
    let dir = std::path::PathBuf::from(format!("{}/Documents/My Games/Path of Exile", std::env::var("USERPROFILE").unwrap()));
    for id in ["blasting", "red-maps", "mid-game"] {
        let segs = parse_filter(&render_profile(&store, id, &dir, Some(&file), None).unwrap().0);
        println!("== {id}");
        for (n, v) in thread_prices(&file) {
            let (from, why) = filter_core::preview::shows_from(&segs, "Map Fragments", &n, 83);
            println!("{n:20} {v:5.2}c from {from:?} {:?}", why.map(|w| (w.kind, w.label, w.section, w.tier_name)));
        }
    }
}

/// Dev check: what a filter file on disk does with an item. `FILTER=<path> ITEM="Map Fragments|Expedition Thread|1" cargo test -p filter-core --test status_real file_item -- --ignored --nocapture`
#[test]
#[ignore]
fn file_item() {
    let segs = filter_core::filter::parse_filter(&std::fs::read_to_string(std::env::var("FILTER").unwrap()).unwrap());
    for one in std::env::var("ITEM").unwrap().split(';') {
        let p: Vec<&str> = one.split('|').collect();
        let i = filter_core::item::parse_item(&format!("Item Class: {}\nRarity: Normal\n{}\n--------\nStack Size: {}/20\n", p[0], p[1], p[2])).unwrap();
        let v = filter_core::evaluate::status(&segs, &i, Some(83), false).verdict;
        println!("{one}: {} by {} {:?} {:?}", v.visibility, v.decided_by, v.source.label, v.uncertain);
    }
}

/// Dev check: are his filter files what this version would build? (changed lines listed)
/// `cargo test -p filter-core --test status_real out_of_date -- --ignored --nocapture`
#[test]
#[ignore]
fn out_of_date() {
    use filter_core::{prices::PriceFile, profiles::{out_of_date, render_profile, Store}};
    let appdata = format!("{}/com.grobbs.poefiltereditor", std::env::var("APPDATA").unwrap());
    let store = Store::load(std::path::Path::new(&format!("{appdata}/profiles.json")));
    let file: PriceFile = serde_json::from_str(&std::fs::read_to_string(format!("{appdata}/prices_poe1.json")).unwrap()).unwrap();
    let dir = std::path::PathBuf::from(format!("{}/Documents/My Games/Path of Exile", std::env::var("USERPROFILE").unwrap()));
    for (id, p) in &store.profiles {
        println!("== {id}: out of date {}", out_of_date(&store, id, &dir, Some(&file)));
        let (new, old) = (render_profile(&store, id, &dir, Some(&file), None).unwrap().0, std::fs::read_to_string(dir.join(&p.output_file)).unwrap_or_default());
        for (a, b) in old.lines().zip(new.lines()).filter(|(a, b)| a != b).take(4) { println!("  old {}\n  new {}", &a[..a.len().min(120)], &b[..b.len().min(120)]); }
        println!("  {} -> {} lines", old.lines().count(), new.lines().count());
    }
}

/// Dev check (24 Nov): what the cards' worth line does in his filters. CARDS="The Master Artisan|5;…"
/// `cargo test -p filter-core --test status_real cards_line -- --ignored --nocapture`
#[test]
#[ignore]
fn cards_line() {
    use filter_core::{filter::parse_filter, prices::PriceFile, profiles::{render_profile, Store}};
    let appdata = format!("{}/com.grobbs.poefiltereditor", std::env::var("APPDATA").unwrap());
    let store = Store::load(std::path::Path::new(&format!("{appdata}/profiles.json")));
    let file: PriceFile = serde_json::from_str(&std::fs::read_to_string(format!("{appdata}/prices_poe1.json")).unwrap()).unwrap();
    let dir = std::path::PathBuf::from(format!("{}/Documents/My Games/Path of Exile", std::env::var("USERPROFILE").unwrap()));
    let cards = std::env::var("CARDS").unwrap_or("The Master Artisan|1;The Master Artisan|3;The Master Artisan|4;The Master Artisan|5".into());
    let (text, warn) = render_profile(&store, "blasting", &dir, Some(&file), None).unwrap();
    println!("{} 'or more' blocks, warnings {warn:?}", text.matches("or more").count());
    for b in text.split("Show # Price: Divination cards worth").skip(1) { println!("  {}", b.lines().take(4).map(|l| l.trim().chars().take(150).collect::<String>()).collect::<Vec<_>>().join(" | ")); }
    let (new, old) = (parse_filter(&text), parse_filter(&std::fs::read_to_string(dir.join("Blasting.filter")).unwrap()));
    for one in cards.split(';') {
        let (n, k) = one.split_once('|').unwrap();
        let i = filter_core::item::parse_item(&format!("Item Class: Divination Cards\nRarity: Normal\n{n}\n--------\nStack Size: {k}/5\n")).unwrap();
        let v = |s| filter_core::evaluate::status(s, &i, Some(83), false).verdict;
        println!("{one}: {} -> {} ({})", v(&old).visibility, v(&new).visibility, v(&new).decided_by);
    }
    // Single cards that change from hidden to shown.
    let mut up = vec![];
    for c in file.items.iter().filter(|i| i.cat == "DivinationCard") {
        let i = filter_core::item::parse_item(&format!("Item Class: Divination Cards\nRarity: Normal\n{}\n--------\nStack Size: 1/5\n", c.name)).unwrap();
        let v = |s| filter_core::evaluate::status(s, &i, Some(83), false).verdict.visibility;
        if v(&old) != v(&new) { up.push(format!("{} {}c {}->{}", c.name, c.value, v(&old), v(&new))); }
    }
    println!("{} single cards change: {up:?}", up.len());
}
