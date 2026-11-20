// Dev tool: reads a price window setup (JSON, as the window sends it) on stdin and prints the trade search it makes.
// Pair with basecheck's BASE_SETUP output to reproduce a search he ran.
use std::io::Read;
fn main() {
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).unwrap();
    let setup: filter_core::trade::TradeSetup = serde_json::from_str(&s).unwrap();
    println!("{}", filter_core::trade::query(&setup));
}
