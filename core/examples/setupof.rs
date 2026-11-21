// Dev tool: runs a saved price check's item text (its "raw" copy) through the price check again with the real
// trade stat list, and prints each stat it picks: id, label, ticked or not. Then the search it would send.
//   cargo run -p filter-core --example setupof -- <trade_stats_poe1.json> <samples/some-item.json>
fn main() {
    let mut args = std::env::args().skip(1);
    let stats = std::fs::read_to_string(args.next().expect("trade_stats_poe1.json")).unwrap();
    let index = filter_core::trade::StatIndex::from_json(&serde_json::from_str(&stats).unwrap());
    let sample: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(args.next().expect("sample json")).unwrap()).unwrap();
    let raw = sample["setup"]["raw"].as_str().expect("the sample has the item's copied text");
    let s = filter_core::trade::setup(raw, &index).expect("reads as an item");
    for f in &s.filters { println!("{}\t{}\t{}", if f.enabled { "[x]" } else { "[ ]" }, f.id, f.label); }
    println!("QUERY {}", filter_core::trade::query(&s));
}
