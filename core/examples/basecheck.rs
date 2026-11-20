// Dev tool: runs the price check's "search as a base" rule (Awakened's) over saved price checks.
// cargo run -p filter-core --example basecheck -- <trade_stats_poe1.json> <samples folder>
// Prints each item that qualifies, why, the mods it keeps, and the search (one JSON line, prefixed QUERY).
fn main() {
    let mut args = std::env::args().skip(1);
    let stats = std::fs::read_to_string(args.next().expect("trade_stats_poe1.json")).unwrap();
    let index = filter_core::trade::StatIndex::from_json(&serde_json::from_str(&stats).unwrap());
    let dir = args.next().expect("samples folder");
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.ends_with(".result.json") { continue; }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(e.path()).unwrap_or_default()) else { continue };
        let Some(raw) = v["setup"]["raw"].as_str().filter(|r| !r.is_empty()) else { continue };
        let Some(mut s) = filter_core::trade::setup(raw, &index) else { continue };
        if std::env::var("BASE_SETUP").is_ok_and(|w| name.starts_with(&w)) { println!("SETUP	{}", serde_json::to_string(&s).unwrap()); }
        let Some(b) = s.base_item.clone() else { continue };
        let kept: Vec<&str> = s.filters.iter().filter(|f| b.ids.contains(&f.id)).map(|f| f.label.as_str()).collect();
        println!("{name}: {} | ilvl {:?} | {:?} | keeps {:?}", b.why.join(", "), b.ilvl_min, b.misc, kept);
        s.base_only = true; s.ilvl_min = b.ilvl_min; s.ilvl_max = None; s.misc = b.misc.clone();
        for f in &mut s.filters { f.enabled = b.ids.contains(&f.id); }
        println!("QUERY\t{name}\t{}", filter_core::trade::query(&s));
    }
}
