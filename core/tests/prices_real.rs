//! Runs real downloaded poe.ninja data (if present locally) through the real filter.
use filter_core::{evaluate::status, filter::{parse_filter, render_filter}, item::parse_item, prices::*, rules::apply_with_extra};

#[test]
fn real_prices_through_real_filter() {
    let Ok(text) = std::fs::read_to_string("/tmp/claude-0/prices/poe1.json") else { return };
    let file: PriceFile = serde_json::from_str(&text).unwrap();
    let mut s = default_settings(&file.unit);
    s.enabled = true;
    for g in s.groups.iter_mut() { g.enabled = true; }
    s.groups.iter_mut().find(|g| g.group == Group::Uniques).unwrap().hide_below = Some(5.0);
    let blocks = price_blocks(&s, &file, "\r\n");
    println!("price blocks: {} bytes, {} blocks", blocks.len(), blocks.matches("\r\nShow #").count() + blocks.matches("\r\nHide #").count());

    let base = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
    let mut segs = parse_filter(&base);
    apply_with_extra(&mut segs, &[], &blocks, "\r\n");
    let out = render_filter(&segs);
    let segs = parse_filter(&out);
    for clip in [
        "Item Class: Belts\nRarity: Unique\nHeadhunter\nLeather Belt\n",
        "Item Class: Divination Cards\nRarity: Normal\nThe Doctor\n",
        "Item Class: Divination Cards\nRarity: Normal\nRain of Chaos\n",
        "Item Class: Stackable Currency\nRarity: Currency\nDivine Orb\n",
        "Item Class: Stackable Currency\nRarity: Currency\nOrb of Transmutation\n",
        "Item Class: Helmets\nRarity: Unique\nGoldrim\nLeather Cap\n",
    ] {
        let it = parse_item(clip).unwrap();
        let v = status(&segs, &it, Some(83), false).verdict;
        println!("{:<22} {:>9}  {:5} by {}", it.name.clone().unwrap_or(it.base_type.clone()),
            price_of(&file, it.name.as_deref(), &it.base_type, &it.rarity).map(|p| format!("{p:.1}c")).unwrap_or("-".into()),
            v.visibility, v.decided_by);
    }
    assert!(out.contains("Price tier 1"));
}
