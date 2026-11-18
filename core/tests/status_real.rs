use filter_core::{evaluate::status, filter::parse_filter, item::{fill_class, parse_item}};
const SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter");

fn show(clip: &str, ground: bool) {
    let segs = parse_filter(&std::fs::read_to_string(SRC).unwrap());
    let mut it = parse_item(clip).unwrap();
    fill_class(&mut it);
    let s = status(&segs, &it, 83, ground);
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
    let s = status(&segs, &parse_item("Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n").unwrap(), 83, false);
    for u in &s.verdict.uncertain { println!("UNSURE {u}"); }
}
