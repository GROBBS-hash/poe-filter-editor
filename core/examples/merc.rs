// Dev tool: the "Mercenary like it" trade search for some skill / support gem names.
// cargo run -p filter-core --example merc -- stats.json "Wrath" "Greater Multiple Projectiles Support"
// (stats.json: https://www.pathofexile.com/api/trade/data/stats). Prints the search to send to the trade site.
use filter_core::merc;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let idx = filter_core::trade::StatIndex::from_json(&v);
    let gems: Vec<merc::MercGem> = args[1..].iter().map(|n| {
        let class = if n.ends_with(" Support") { "Support Gems" } else { "Skill Gems" };
        let mut g = merc::gem(&format!("Item Class: {class}\nRarity: Gem\n{n}\n--------\n")).unwrap();
        merc::fill_tier(&mut g, &idx);
        g
    }).collect();
    eprintln!("{gems:?}");
    let mut s = filter_core::trade::setup(&merc::warrant_text("Test", &gems), &idx).unwrap();
    for f in s.filters.iter_mut().filter(|f| f.kind == "mercenary" && f.tier.is_none()) { f.enabled = true; }
    for f in &s.filters { eprintln!("{} {} {}", f.enabled, f.id, f.label); }
    println!("{}", filter_core::trade::query(&s));
}
