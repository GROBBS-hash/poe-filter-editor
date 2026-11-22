// Dev tool: what the Tiers page would show for NeverSink rules of a type (tiers, notes, bases).
// cargo run -p filter-core --example tierdump -- <file.filter> <type prefix, e.g. leveling->flasks>
// With "--json" as the prefix: the Tiers tab's whole section list as JSON (for the app preview's public/_tiers.json).
fn main() {
    let mut a = std::env::args().skip(1);
    let text = std::fs::read_to_string(a.next().expect("filter file")).unwrap();
    let prefix = a.next().unwrap_or_default();
    let segs = filter_core::filter::parse_filter(&text);
    if prefix == "--json" {
        let mut secs = filter_core::tiers::sections(&segs);
        // PRICE_FILE=<the app's prices_poe1.json>: worth beside each item, like the app.
        if let Some(f) = std::env::var_os("PRICE_FILE").and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str::<filter_core::prices::PriceFile>(&t).ok()) {
            filter_core::tiers::attach_prices(&mut secs, &f, &Default::default());
        }
        println!("{}", serde_json::to_string(&secs).unwrap());
        return;
    }
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
