//! Optional check against a real, current filter: POE_FILTER=<path> cargo test --test local_filter -- --nocapture
#[test]
fn rare_gear_item_tiers() {
    let Ok(path) = std::env::var("POE_FILTER") else { return };
    let text = std::fs::read_to_string(path).unwrap();
    let segs = filter_core::filter::parse_filter(&text);
    let secs = filter_core::tiers::sections(&segs);
    let g = secs.iter().find(|s| s.title == "Rare gear by item tier").expect("section");
    assert!(g.shared_bases);
    for t in &g.tiers { println!("{:32} {:?} {} ({} bases)", t.name, t.min_item_tier, t.note, t.bases.len()); }
    let it = filter_core::item::Item { item_class: "Rings".into(), rarity: "Rare".into(), base_type: "Bone Ring".into(),
        item_level: Some(75), item_tier: Some(4), ..Default::default() };
    let v = filter_core::evaluate::status(&segs, &it, Some(75), false).verdict;
    println!("Bone Ring tier 4: {} {:?}", v.visibility, v.source);
}
