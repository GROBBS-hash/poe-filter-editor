// Dev tool: the upgrade finder's plan and trade query for each slot in a worn.json.
// cargo run -p filter-core --example upgrade -- path/to/worn.json [slot]
fn main() {
    let w: filter_core::worn::Worn = serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).expect("worn.json")).unwrap()).unwrap();
    let gear = w.gear();
    let only = std::env::args().nth(2);
    for s in ["Helmet", "Body Armour", "Gloves", "Boots", "Belt", "Amulet", "Ring 1", "Ring 2"] {
        if only.as_deref().is_some_and(|o| o != s) || !gear.iter().any(|g| g.slot == s) { continue; }
        let p = filter_core::upgrade::plan(s, &gear, [0, 0, 0]);
        eprintln!("{} ({}): now {} | {} | not counted: {:?}", p.slot, p.current, p.now, p.wants.iter().map(|w| format!("{} x{}", w.stat, w.weight)).collect::<Vec<_>>().join("; "), p.not_counted);
        if only.is_some() { println!("{}", filter_core::upgrade::query(&p, None).unwrap()); }
    }
}
