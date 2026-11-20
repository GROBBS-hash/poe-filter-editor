// Dev tool: reads a stash tab as pathofexile.com's get-stash-items returns it, and prints, for each rare the
// Wealth page would price, its name, the ticked stats and the trade search (one JSON per line, after a tab).
//   cargo run -p filter-core --example rarequeries -- tab.json %APPDATA%\com.grobbs.poefiltereditor\trade_stats_poe1.json
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tab: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let stats: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let index = filter_core::trade::StatIndex::from_json(&stats);
    for r in filter_core::wealth::rares_in_tab(&tab["items"], "tab") {
        let Some(s) = filter_core::trade::setup(&r.text, &index) else { println!("{}\t(couldn't read)", r.name); continue };
        let ticked: Vec<&str> = s.filters.iter().filter(|f| f.enabled).map(|f| f.label.as_str()).collect();
        eprintln!("{} | {}", r.name, ticked.join(" ; "));
        println!("{}\t{}", r.name, filter_core::trade::query(&s));
    }
}
