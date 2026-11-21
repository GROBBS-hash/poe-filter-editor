//! Prints every crafting route for some mods, to check the maths by eye.
//! cargo run --release -p filter-core --example routesof -- "Paladin Boots" 86 "Movement Speed:2" "to maximum Life:5" "Chaos Resistance:2" "to Fire Resistance:3"
//! Prices: the app's saved poe.ninja file (%APPDATA%\com.grobbs.poefiltereditor\prices_poe1.json) when it's there.
use filter_core::craft::{pool, routes::routes};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (base, ilvl) = (a[0].as_str(), a[1].parse::<u32>().unwrap());
    let file = std::env::var("APPDATA").ok().and_then(|d| std::fs::read_to_string(format!("{d}/com.grobbs.poefiltereditor/prices_poe1.json")).ok())
        .and_then(|s| serde_json::from_str::<filter_core::prices::PriceFile>(&s).ok());
    let price = |n: &str| file.as_ref().and_then(|f| filter_core::prices::price_of(f, None, n, ""));
    let p = pool(base, ilvl, &[], &[], &price).unwrap();
    let targets: Vec<(String, u32)> = a[2..].iter().map(|s| {
        let (text, tier) = s.rsplit_once(':').unwrap();
        let f = p.families.iter().find(|f| f.text.contains(text) && !f.tiers.is_empty() && f.needs.is_none()).unwrap_or_else(|| panic!("no mod {text}"));
        (f.key.clone(), tier.parse().unwrap())
    }).collect();
    let t = std::time::Instant::now();
    let r = routes(base, ilvl, &[], &targets, &[], &price).unwrap();
    println!("{} routes in {:?}; note: {}", r.routes.len(), t.elapsed(), r.note);
    // CRAFT_JSON=public: files for the page preview (preview-app.html), plus the plan to paste into its localStorage.
    if let Ok(dir) = std::env::var("CRAFT_JSON") {
        std::fs::write(format!("{dir}/_craft_routes.json"), serde_json::to_string(&r).unwrap()).unwrap();
        std::fs::write(format!("{dir}/_craft_pool.json"), serde_json::to_string(&p).unwrap()).unwrap();
        let plan: Vec<serde_json::Value> = targets.iter().map(|(k, t)| serde_json::json!({ "key": k, "tier": t })).collect();
        println!("plan: {}", serde_json::to_string(&plan).unwrap());
    }
    for x in &r.routes {
        println!("\n== {} [{}] cost {:?} clicks {} risk {} {:?}", x.name, x.id, x.cost.map(|c| c.round()), x.clicks, x.risk, x.badges);
        for s in &x.steps {
            println!("  - {}", s.title);
            for w in s.ways.iter().take(3) { println!("      {} 1 in {:.1}, avg {:?}, tries {:.0}", w.method, 1.0 / w.chance, w.average.map(|a| a.round()), w.tries) }
        }
        for c in &x.checks { println!("  check: {c}") }
    }
}
