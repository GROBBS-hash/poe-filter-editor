use filter_core::filter::*;
const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter");

fn blocks(s: &[Segment]) -> Vec<&Block> { s.iter().filter_map(|x| if let Segment::Block(b)=x {Some(b)} else {None}).collect() }

#[test]
fn round_trip_is_byte_identical() {
    let text = std::fs::read_to_string(PATH).unwrap();
    assert!(render_filter(&parse_filter(&text)) == text, "round trip changed the file");
}

#[test]
fn finds_tagged_blocks() {
    let text = std::fs::read_to_string(PATH).unwrap();
    let segs = parse_filter(&text);
    let b = blocks(&segs);
    let tagged = b.iter().filter(|b| !b.type_path.is_empty()).count();
    println!("blocks: {}, tagged: {}", b.len(), tagged);
    assert_eq!(tagged, 786);
    let rep = b.iter().find(|b| b.type_path == ["uniques","replicas"] && b.tier.as_deref()==Some("t2")).unwrap();
    assert!(rep.header.contains("%H8") && rep.header.contains("!uniques_a"));
}

#[test]
fn move_unique_to_hidden_tier_and_back_out() {
    let text = std::fs::read_to_string(PATH).unwrap();
    let mut segs = parse_filter(&text);
    let tiers: Vec<String> = blocks(&segs).iter().filter(|b| b.type_path==["uniques"]).filter_map(|b| b.tier.clone()).collect();
    println!("unique tiers: {:?}", tiers);
    let t1 = blocks(&segs).into_iter().find(|b| b.type_path==["uniques"] && b.tier.as_deref()==Some("t1") && !b.base_types().is_empty()).unwrap();
    let base = t1.base_types()[0].clone();
    let hide_tier = tiers.iter().find(|t| t.contains("hide") || t.contains("restex")).cloned().unwrap_or("t3".into());
    println!("moving {base} -> {hide_tier}");
    assert!(move_base(&mut segs, &["uniques"], &base, &hide_tier));
    let out = render_filter(&segs);
    assert!(out.contains("\r\n"));
    let again = parse_filter(&out);
    let b = blocks(&again);
    assert!(!b.iter().find(|b| b.type_path==["uniques"] && b.tier.as_deref()==Some("t1")).unwrap().base_types().contains(&base));
    assert!(b.iter().any(|b| b.type_path==["uniques"] && b.tier.as_deref()==Some(hide_tier.as_str()) && b.base_types().contains(&base)));
}
