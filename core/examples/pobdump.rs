// Dev tool: prints what the app reads from a PoB code/XML file (stages, gems, gear, jewels).
// cargo run -p filter-core --example pobdump -- path/to/pob.txt
fn main() {
    let code = std::fs::read_to_string(std::env::args().nth(1).expect("path to a PoB code or XML")).unwrap();
    let xml = if code.trim_start().starts_with('<') { code } else { filter_core::pob::decode(&code).unwrap() };
    let b = filter_core::pob::parse(&xml).unwrap();
    for s in &b.stages {
        println!("{}: {} groups, {} gems, {} gear: {}", s.title, s.groups.len(), s.groups.iter().map(|g| g.gems.len()).sum::<usize>(), s.gear.len(),
            s.gear.iter().map(|g| g.slot.as_str()).collect::<Vec<_>>().join(", "));
        for j in &s.jewels { println!("   jewel {:?} {} / {} {:?}", j.node, j.item.name, j.item.base, j.notables); }
        // POB_COPY=1: the price-window copy of every item, and whether it reads back.
        // POB_BUY=<trade_stats_poe1.json> [POB_STAGE=<part of a stage name>]: the "Buy" search for each item, one JSON per line.
        if let (Ok(stats), true) = (std::env::var("POB_BUY"), s.title.contains(&std::env::var("POB_STAGE").unwrap_or_default())) {
            let index = filter_core::trade::StatIndex::from_json(&serde_json::from_str(&std::fs::read_to_string(stats).unwrap()).unwrap());
            for g in s.gear.iter().chain(s.jewels.iter().map(|j| &j.item)) {
                let Some(setup) = filter_core::trade::setup(&g.copy, &index) else { continue };
                let b = filter_core::trade::buy_setup(setup);
                if std::env::var("POB_BUY_DEBUG").is_ok() {
                    eprintln!("== {} ({})", g.name, g.base);
                    for f in &b.filters { eprintln!("   {} {:9} {} min {:?}", if f.enabled { "[x]" } else { "[ ]" }, f.kind, f.label, f.min); }
                    for u in &b.unknown { eprintln!("   ??? not on the trade site's list: {u}"); }
                }
                let q = |spare| filter_core::trade::buy_query(&b, spare);
                println!("BUY\t{}\t{}\t{}\t{}\t{}", g.name, filter_core::trade::buy_mod_count(&b), q(0), q(1), q(2));
            }
            return;
        }
        if std::env::var("POB_COPY").is_ok() {
            for g in s.gear.iter().chain(s.jewels.iter().map(|j| &j.item)) {
                let ok = filter_core::trade::setup(&g.copy, &Default::default()).map(|t| format!("{} / {} / {} mods", t.item.item_class, t.item.base_type, filter_core::trade::mods(&g.copy).len()));
                println!("   price {} -> {:?}", g.name, ok);
                if std::env::var("POB_COPY").as_deref() == Ok("full") { println!("{}", g.copy); }
            }
        }
    }
}
