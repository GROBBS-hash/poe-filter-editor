// Dev tool: what the Tiers page would show for NeverSink rules of a type (tiers, notes, bases).
// cargo run -p filter-core --example tierdump -- <file.filter> <type prefix, e.g. leveling->flasks>
// With "--json" as the prefix: the Tiers tab's whole section list as JSON (for the app preview's public/_tiers.json).
fn main() {
    let mut a = std::env::args().skip(1);
    let text = std::fs::read_to_string(a.next().expect("filter file")).unwrap();
    let prefix = a.next().unwrap_or_default();
    let segs = filter_core::filter::parse_filter(&text);
    if prefix == "--json" { println!("{}", serde_json::to_string(&filter_core::tiers::sections(&segs)).unwrap()); return; }
    let mut seen = std::collections::BTreeSet::new();
    for s in &segs {
        let filter_core::filter::Segment::Block(b) = s else { continue };
        let key = b.type_path.join("->");
        if key.starts_with(&prefix) && seen.insert(key.clone()) {
            match filter_core::tiers::section_at(&segs, &b.type_path) {
                Some(sec) => {
                    println!("== {key}: {} tiers", sec.tiers.len());
                    for t in &sec.tiers { println!("   {} [{}] {} | {} bases: {}", t.tier, t.name, t.note, t.bases.len(), t.bases.iter().take(4).cloned().collect::<Vec<_>>().join(", ")); }
                }
                None => println!("== {key}: no base-listed tiers"),
            }
        }
    }
}
