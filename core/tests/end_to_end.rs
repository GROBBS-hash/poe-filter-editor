use filter_core::{filter::*, item::*, profiles::*, rules::*};
use std::fs;

const SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter");

fn setup(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pfe-{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::copy(SRC, dir.join("NeverSink.filter")).unwrap();
    dir
}

#[test]
fn f2_hide_then_rebuild_and_undo() {
    let dir = setup("hide");
    let base_before = fs::read(dir.join("NeverSink.filter")).unwrap();
    let mut store = Store::default();
    let id = store.create("League Start", Game::Poe1, "NeverSink.filter");

    // What the game puts on the clipboard for a magic ring:
    let clip = "Item Class: Rings\r\nRarity: Magic\r\nCoral Ring of the Whelpling\r\n--------\r\nItem Level: 75\r\n";
    let mut item = parse_item(clip).unwrap();
    let segs = parse_filter(&String::from_utf8(base_before.clone()).unwrap());
    resolve_magic_base(&mut item, &known_bases(&segs));
    assert_eq!(item.base_type, "Coral Ring");
    println!("Coral Ring lives in: {:?}", locate(&segs, &item).iter().map(|l| format!("{}/{:?}", l.type_path.join(">"), l.tier)).collect::<Vec<_>>());

    store.profiles.get_mut(&id).unwrap().rules.push(quick_rule(&item, Action::Hide, None, &MatchOpts::default()));
    let warn = build(&store, &id, &dir).unwrap();
    assert!(warn.is_empty());

    let out = fs::read_to_string(dir.join("League Start.filter")).unwrap();
    let head: String = out.lines().take(9).collect::<Vec<_>>().join("\n");
    println!("--- top of new filter ---\n{head}");
    assert!(out.starts_with(START));
    assert!(out.contains("Hide # Hide: Magic Coral Ring\r\n    Class == \"Rings\"\r\n    BaseType == \"Coral Ring\"\r\n    Rarity = Magic"));
    assert_eq!(fs::read(dir.join("NeverSink.filter")).unwrap(), base_before, "base file must never change");
    // Everything below our section is NeverSink's file untouched, except his citadel map rules switched on:
    let after_ours = &out[out.find(END).unwrap() + END.len() + 4..];
    let mut expected = parse_filter(&String::from_utf8_lossy(&base_before));
    assert_eq!(enable_commented(&mut expected, &["maps", "implicitmod"]), 2);
    assert_eq!(after_ours, render_filter(&expected));

    // Undo = drop the last rule and rebuild; a backup of the previous version is kept.
    store.profiles.get_mut(&id).unwrap().rules.pop();
    build(&store, &id, &dir).unwrap();
    assert!(!fs::read_to_string(dir.join("League Start.filter")).unwrap().contains("Coral Ring\"\r\n    Rarity"));
    assert_eq!(fs::read_dir(dir.join("LiveEditorBackups")).unwrap().count(), 1);
}

#[test]
fn profiles_fork_and_inherit() {
    let dir = setup("fork");
    let mut store = Store::default();
    let ls = store.create("League Start", Game::Poe1, "NeverSink.filter");
    let gold = parse_item("Item Class: Stackable Currency\nRarity: Currency\nGold\n--------\nStack Size: 50\n").unwrap();
    store.profiles.get_mut(&ls).unwrap().rules.push(quick_rule(&gold, Action::Highlight, Some("purple"), &MatchOpts::default()));

    let s1 = store.fork(&ls, "Strict 1", true).unwrap();
    let wis = parse_item("Item Class: Stackable Currency\nRarity: Currency\nScroll of Wisdom\n").unwrap();
    store.profiles.get_mut(&s1).unwrap().rules.push(quick_rule(&wis, Action::Hide, None, &MatchOpts::default()));

    build(&store, &ls, &dir).unwrap();
    build(&store, &s1, &dir).unwrap();
    // Freshly built: nothing to do. One an older version built differently is out of date; somebody's own file isn't ours.
    let old = |id: &str| filter_core::profiles::out_of_date(&store, id, &dir, None);
    assert!(!old(&ls) && !old(&s1));
    let file = dir.join("League Start.filter");
    let good = fs::read_to_string(&file).unwrap();
    fs::write(&file, good.replace("Highlight: Gold", "Hide # Price: Fragments worth under 4c")).unwrap();
    assert!(old(&ls));
    fs::write(&file, "Show
").unwrap();
    assert!(!old(&ls));
    fs::write(&file, good).unwrap();
    let a = fs::read_to_string(dir.join("League Start.filter")).unwrap();
    let b = fs::read_to_string(dir.join("Strict 1.filter")).unwrap();
    assert!(a.contains("Highlight: Gold") && !a.contains("Scroll of Wisdom\"\r\n    DisableDropSound"));
    assert!(b.contains("Highlight: Gold") && b.contains("Hide: Scroll of Wisdom"));
    // Newest edit sits on top so it wins:
    assert!(b.find("Hide: Scroll of Wisdom").unwrap() < b.find("Highlight: Gold").unwrap());

    let path = dir.join("profiles.json");
    store.save(&path).unwrap();
    assert_eq!(Store::load(&path).profiles.len(), 2);
}

#[test]
fn locate_unique() {
    let text = fs::read_to_string(SRC).unwrap();
    let segs = parse_filter(&text);
    let item = parse_item("Item Class: Belts\nRarity: Unique\nHeadhunter\nLeather Belt\n").unwrap();
    let locs = locate(&segs, &item);
    println!("Leather Belt unique: {:?}", locs.iter().map(|l| format!("{}/{:?}", l.type_path.join(">"), l.tier)).collect::<Vec<_>>());
    assert!(locs.iter().any(|l| l.type_path == ["uniques"]));
}
