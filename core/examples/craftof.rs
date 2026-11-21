// Dev tool: what can roll on a base, and the ways to get one mod.
//   cargo run -p filter-core --example craftof -- "Eternal Burgonet" 85 "maximum Life" [Shaper]
fn main() {
    let mut a = std::env::args().skip(1);
    let base = a.next().expect("base");
    let ilvl: u32 = a.next().and_then(|s| s.parse().ok()).unwrap_or(84);
    let want = a.next().unwrap_or_default();
    let infl: Vec<String> = a.collect();
    let price = |n: &str| match n { "Chaos Orb" => Some(1.0), _ => None };
    let p = filter_core::craft::pool(&base, ilvl, &infl, &[], &price).expect("base known");
    // CRAFT_JSON=dir: also save the pool and one mod's ways for the page preview (preview-app.tsx).
    if let Ok(dir) = std::env::var("CRAFT_JSON") {
        std::fs::write(format!("{dir}/_craft_pool.json"), serde_json::to_string(&p).unwrap()).unwrap();
        if let Some(f) = p.families.iter().find(|f| f.text.contains(&want)) {
            let w = filter_core::craft::ways(&base, ilvl, &infl, &f.key, 2, &price);
            std::fs::write(format!("{dir}/_craft_ways.json"), serde_json::to_string(&w).unwrap()).unwrap();
        }
    }
    println!("{} ({}) ilvl {}: {} families", p.base, p.class, p.ilvl, p.families.len());
    for f in &p.families {
        println!("{:6} {:5.1}%  T{:<2} {}{}{}", f.affix, f.chaos * 100.0, f.tiers.len(), f.text.replace('\n', " / "),
            if f.essences.is_empty() { "" } else { "  [essence]" }, if f.bench.is_empty() { "" } else { "  [bench]" });
    }
    if want.is_empty() { return }
    let f = p.families.iter().find(|f| f.text.contains(&want)).expect("no such mod");
    println!("\n== {} ({})", f.text.replace('\n', " / "), f.key);
    for t in &f.tiers { println!("  T{} {:12} lvl {:2} w {:5} {}", t.tier, t.name, t.lvl, t.weight, t.text.replace('\n', " / ")); }
    for e in &f.essences { println!("  essence {} -> {} (like T{:?})", e.name, e.text, e.like_tier); }
    println!("  fossils more: {:?}\n  fossils none: {:?}", f.fossils_more, f.fossils_none);
    for w in filter_core::craft::ways(&base, ilvl, &infl, &f.key, 2, &price) {
        println!("  WAY {:45} {:6.2}%  1 in {:6.0}  {}", w.method, w.chance * 100.0, 1.0 / w.chance.max(1e-9), w.note);
    }
}
