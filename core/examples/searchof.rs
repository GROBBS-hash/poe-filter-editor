//! Prints the crafting search for mods on a base: cargo run -p filter-core --example searchof -- "Titan Greaves" 85 "maximum Life:2" "Movement Speed:1"
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let price = |_: &str| None;
    let pool = filter_core::craft::pool(&a[0], a[1].parse().unwrap(), &[], &[], &price).expect("base");
    let mut targets = vec![];
    for t in &a[2..] {
        let (word, tier) = t.split_once(':').unwrap_or((t, "1"));
        if let Some(f) = pool.families.iter().find(|f| f.text.contains(word) && f.chaos > 0.0 && !f.text.contains(" / ")) {
            targets.push((f.key.clone(), tier.parse().unwrap_or(1)));
        }
    }
    for all in [false, true] {
        let s = filter_core::craft::search::craft_search(&pool, &targets, all);
        println!("{} {}\n  {:?}\n  {:?}", if all { "ALL" } else { "ANY" }, s.text, s.parts, s.notes);
    }
}
