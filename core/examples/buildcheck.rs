// Dev tool: runs the build checker on a PoB code file and prints every stage.
// cargo run -p filter-core --example buildcheck -- path/to/pob.txt
fn main() {
    let code = std::fs::read_to_string(std::env::args().nth(1).expect("path to a PoB code")).unwrap();
    let b = filter_core::pob::parse(&filter_core::pob::decode(&code).unwrap()).unwrap();
    if std::env::var("DUMP").is_ok() { for g in &b.stages[1].gear { println!("{} | {} | {:?}", g.slot, g.base, g.lines); } }
    for s in filter_core::buildcheck::check(&b) {
        println!("== {} (level {}{}) S{} D{} I{} points {}/{} mana {:?} speed {:?}", s.title, s.level, if s.level_guessed { "?" } else { "" }, s.str, s.dex, s.int, s.points, s.points_have, s.mana, s.speed);
        for n in &s.notes { println!("   [{}] {}", n.kind, n.text); }
    }
}
