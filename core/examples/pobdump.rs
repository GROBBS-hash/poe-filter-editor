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
    }
}
