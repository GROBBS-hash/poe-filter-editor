// Dev tool: the "Buy" search for an item (PoB item text on stdin) at a stage level, as My build makes it.
//   STATS=<data dir>/trade_stats_poe1.json LEVEL=68 cargo run -q -p filter-core --example buyof < item.txt
use std::io::Read;
fn main() {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).unwrap();
    let stats: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(std::env::var("STATS").unwrap()).unwrap()).unwrap();
    let index = filter_core::trade::StatIndex::from_json(&stats);
    let (mut q, _) = filter_core::trade::buy_query(&text, &index).unwrap();
    let level: u32 = std::env::var("LEVEL").ok().and_then(|l| l.parse().ok()).unwrap_or(100);
    let item = filter_core::item::parse_item(&text).unwrap();
    if let Some(b) = filter_core::buildcheck::wearable_base(&item.base_type, level) {
        eprintln!("{} needs more than {level}: searching {b}", item.base_type);
        filter_core::trade::swap_base(&mut q, &b);
    }
    filter_core::trade::max_level(&mut q, level);
    println!("{q}");
}
