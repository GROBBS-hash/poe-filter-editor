//! "Sell in bulk" (Money → Sell in bulk, 23 Nov): lots of one kind of item (currency, essences, cards,
//! fragments, lifeforce, threads…) from the last stash scan, grouped, priced at poe.ninja minus a discount,
//! with the price to type in the game's "Exact price" box and a ready post for Discord / TFT.
//! Nothing here lists or trades: GGG gives apps no way to set prices, so the player types them in game.

use crate::wealth::Valued;
use serde::Serialize;

/// The groups shown as cards, in this order.
pub const GROUPS: [&str; 15] = [
    "Currency", "Essences", "Div cards", "Fragments", "Lifeforce", "Threads", "Catalysts", "Fossils",
    "Oils", "Delirium orbs", "Omens", "Eldritch", "Maps", "Tattoos & runegrafts", "Other",
];

/// Which card an item goes on, from its poe.ninja category (None: not sold in bulk, e.g. uniques and gems).
pub fn group_of(name: &str, cat: &str) -> Option<&'static str> {
    if cat.starts_with("Unique") || cat.contains("Gem") || name == "Chaos Orb" { return None; }
    Some(match cat {
        "Currency" if name.ends_with("Crystallised Lifeforce") => "Lifeforce",
        "Currency" if name.ends_with("Catalyst") => "Catalysts",
        "Currency" if name.contains("Eldritch") => "Eldritch",
        "Currency" => "Currency",
        "Essence" => "Essences",
        "DivinationCard" => "Div cards",
        "Fragment" | "Invitation" | "Memory" => "Fragments",
        "Thread" => "Threads",
        "Fossil" | "Resonator" => "Fossils",
        "Oil" => "Oils",
        "DeliriumOrb" => "Delirium orbs",
        "Omen" => "Omens",
        "Map" | "BlightedMap" | "BlightRavagedMap" => "Maps",
        "Tattoo" | "Runegraft" => "Tattoos & runegrafts",
        _ => "Other",
    })
}

/// One kind of item you have, added up over every stack and tab.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BulkItem {
    pub name: String,
    pub group: String,
    pub have: u32,
    /// poe.ninja's chaos price for one.
    pub each: f64,
    pub icon: String,
    /// The tabs it's in ("Dump 1, Currency").
    pub tabs: String,
    /// Divination cards: how many make a full set.
    pub set: Option<u32>,
}

/// What you have to sell in bulk: unlisted, priced items from the last scan (not rares), added up by name,
/// most valuable first. `cat_of` gives an item's poe.ninja category.
pub fn bulk_items(items: &[Valued], cat_of: &dyn Fn(&str) -> Option<String>) -> Vec<BulkItem> {
    let mut out: Vec<BulkItem> = Vec::new();
    for v in items.iter().filter(|v| !v.listed && !v.rare && v.stack > 0) {
        if let Some(b) = out.iter_mut().find(|b| b.name == v.name) {
            b.have += v.stack;
            if !b.tabs.split(", ").any(|t| t == v.tab) { b.tabs = format!("{}, {}", b.tabs, v.tab); }
            continue;
        }
        let Some(group) = cat_of(&v.name).and_then(|c| group_of(&v.name, &c)) else { continue };
        out.push(BulkItem {
            name: v.name.clone(), group: group.to_string(), have: v.stack, each: v.value / v.stack as f64,
            icon: v.icon.clone(), tabs: v.tab.clone(), set: v.max_stack,
        });
    }
    out.sort_by(|a, b| (b.each * b.have as f64).total_cmp(&(a.each * a.have as f64)));
    out
}

/// Your asking price for one: poe.ninja's (or the live rate) less `discount` percent.
pub fn price_each(market: f64, discount: f64) -> f64 {
    (market * (1.0 - discount.clamp(0.0, 90.0) / 100.0)).max(0.0)
}

/// What to type in the game's price box (right-click → Exact price): an amount and a currency.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    /// "86/24" = 24 items for 86 (the box takes a fraction: the price per item is 86 ÷ 24), or "1.5".
    pub amount: String,
    /// "chaos" or "divine".
    pub currency: String,
    /// What it really asks per item, in chaos, after rounding (so the page can show the true total).
    pub each: f64,
}

/// The price note for `count` items at `each` chaos apiece. Items worth a divine or more are priced in divines
/// (one decimal); cheaper ones in chaos as a whole-number fraction "total/count", reduced where it can be
/// (380 items at 0.27c → "103/380"; 24 at 4c → "4"). Under 1 chaos for the lot: priced at 1 chaos for the lot.
pub fn price_note(each: f64, count: u32, divine: Option<f64>) -> Note {
    let count = count.max(1);
    if let Some(d) = divine.filter(|d| *d > 0.0 && each >= *d) {
        let amount = ((each / d) * 10.0).round() / 10.0;
        return Note { amount: trim(amount), currency: "divine".into(), each: amount * d };
    }
    let total = (each * count as f64).round().max(1.0) as u64;
    let g = gcd(total, count as u64);
    let (t, c) = (total / g, count as u64 / g);
    let amount = if c == 1 { t.to_string() } else { format!("{t}/{c}") };
    Note { amount, currency: "chaos".into(), each: total as f64 / count as f64 }
}

fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a.max(1) } else { gcd(b, a % b) } }

/// 1.0 → "1", 1.5 → "1.5".
fn trim(x: f64) -> String {
    let s = format!("{x:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// One line of the basket for the post: name, how many, the asking total in chaos.
pub struct Line<'a> { pub name: &'a str, pub count: u32, pub total: f64 }

/// A "want to sell" post for Discord (TFT style): one line per item, then the price for the lot.
pub fn post(title: &str, league: &str, lines: &[Line], divine: Option<f64>, discount: f64, character: &str) -> String {
    let total: f64 = lines.iter().map(|l| l.total).sum();
    let mut s = format!("WTS {league} · {title}\n");
    for l in lines { s += &format!("{}x {} · {}c\n", l.count, l.name, l.total.round() as i64); }
    let off = if discount > 0.0 { format!(" · {}% under market", trim(discount)) } else { String::new() };
    s += &format!("All for {}{off}\n", chaos_or_div(total, divine));
    if !character.trim().is_empty() { s += &format!("IGN: @{}", character.trim()); }
    s.trim_end().to_string()
}

/// "86c", or "410c (2.3 div)" once it's worth a divine.
pub fn chaos_or_div(chaos: f64, divine: Option<f64>) -> String {
    let c = format!("{}c", chaos.round() as i64);
    match divine.filter(|d| *d > 0.0 && chaos >= *d) {
        Some(d) => format!("{c} ({} div)", trim((chaos / d * 10.0).round() / 10.0)),
        None => c,
    }
}

/// The going rate from bulk exchange offers (chaos each, stock): weigh each offer by its stock and take the
/// middle, like the Ctrl+D window, so a few price fixers with 1 in stock don't drag it down.
pub fn going_rate(offers: &[(f64, u32)]) -> Option<f64> {
    let mut rows: Vec<(f64, u32)> = offers.iter().filter(|o| o.0 > 0.0).map(|o| (o.0, o.1.max(1))).collect();
    if rows.is_empty() { return None; }
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: u64 = rows.iter().map(|r| r.1 as u64).sum();
    let mut acc = 0u64;
    for r in &rows {
        acc += r.1 as u64;
        if acc * 2 >= total { return Some(r.0); }
    }
    rows.last().map(|r| r.0)
}

/// Whether a live exchange rate is believable next to poe.ninja's: within 3x either way. Cheap things like
/// lifeforce barely trade for chaos on the exchange (23 Nov: the cheapest Wild Lifeforce offer was 1c each,
/// poe.ninja 0.02c), so a live number far off is a few odd listings, not the market.
pub fn fair_live(live: f64, market: f64) -> bool {
    market <= 0.0 || (live > 0.0 && live <= market * 3.0 && live >= market / 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(name: &str, tab: &str, stack: u32, value: f64, listed: bool) -> Valued {
        Valued { tab: tab.into(), name: name.into(), stack, value, x: 0, y: 0, listed, icon: format!("{name}.png"), places: 1,
            rare: false, max_stack: None, note: String::new() }
    }

    fn cat(name: &str) -> Option<String> {
        Some(match name {
            "Deafening Essence of Greed" => "Essence",
            "Orb of Alchemy" | "Chaos Orb" | "Divine Orb" => "Currency",
            "Wild Crystallised Lifeforce" => "Currency",
            "Abyss Thread" => "Thread",
            "Headhunter Leather Belt" => "UniqueAccessory",
            _ => return None,
        }.to_string())
    }

    #[test]
    fn groups_and_adds_up() {
        let items = vec![
            v("Orb of Alchemy", "Dump 1", 200, 60.0, false), v("Orb of Alchemy", "Currency", 180, 54.0, false),
            v("Orb of Alchemy", "Shop", 40, 12.0, true),
            v("Deafening Essence of Greed", "Dump 1", 24, 96.0, false), v("Chaos Orb", "Currency", 500, 500.0, false),
            v("Headhunter Leather Belt", "Dump 1", 1, 9000.0, false), v("Wild Crystallised Lifeforce", "Dump 2", 4200, 84.0, false),
            v("Abyss Thread", "Dump 2", 10, 30.0, false), v("Mystery", "Dump 2", 3, 9.0, false),
        ];
        let b = bulk_items(&items, &cat);
        let names: Vec<&str> = b.iter().map(|x| x.name.as_str()).collect();
        // Most valuable first; chaos, uniques, listed and unknown items left out.
        assert_eq!(names, ["Orb of Alchemy", "Deafening Essence of Greed", "Wild Crystallised Lifeforce", "Abyss Thread"]);
        assert_eq!((b[0].have, b[0].tabs.as_str(), b[0].group.as_str()), (380, "Dump 1, Currency", "Currency"));
        assert!((b[0].each - 0.3).abs() < 1e-9);
        assert_eq!(b[2].group, "Lifeforce");
        assert_eq!(b[3].group, "Threads");
        for g in b.iter().map(|x| x.group.as_str()) { assert!(GROUPS.contains(&g)); }
    }

    #[test]
    fn price_notes() {
        assert!((price_each(4.0, 10.0) - 3.6).abs() < 1e-9);
        assert!((price_each(4.0, 150.0) - 0.4).abs() < 1e-9, "discount capped at 90%");
        // 380 Alchemy at 0.27c = 102.6 → 103 for 380.
        let n = price_note(0.27, 380, Some(180.0));
        assert_eq!((n.amount.as_str(), n.currency.as_str()), ("103/380", "chaos"));
        // A whole price per item reduces to one number.
        assert_eq!(price_note(4.0, 24, Some(180.0)).amount, "4");
        assert_eq!(price_note(3.6, 24, Some(180.0)).amount, "43/12");
        // A divine or more each: in divines.
        let d = price_note(270.0, 2, Some(180.0));
        assert_eq!((d.amount.as_str(), d.currency.as_str()), ("1.5", "divine"));
        assert!((d.each - 270.0).abs() < 1e-9);
        // Tiny lots never ask 0.
        assert_eq!(price_note(0.01, 5, None).amount, "1/5");
    }

    #[test]
    fn the_post() {
        let lines = [Line { name: "Deafening Essence of Greed", count: 24, total: 86.0 }, Line { name: "Orb of Alchemy", count: 380, total: 324.0 }];
        let p = post("Essences & currency", "Keepers", &lines, Some(180.0), 10.0, "MyChar");
        assert_eq!(p, "WTS Keepers · Essences & currency\n24x Deafening Essence of Greed · 86c\n380x Orb of Alchemy · 324c\nAll for 410c (2.3 div) · 10% under market\nIGN: @MyChar");
        assert!(!post("x", "L", &lines, None, 0.0, "").contains("IGN"));
    }

    #[test]
    fn going_rate_ignores_fixers() {
        // Two fixers with 1 in stock at 1c, real sellers at 4c with lots.
        assert_eq!(going_rate(&[(1.0, 1), (1.0, 1), (4.0, 50), (4.5, 40)]), Some(4.0));
        assert_eq!(going_rate(&[]), None);
        assert!(fair_live(10.0, 6.15));
        assert!(!fair_live(1.0, 0.02), "lifeforce: a few odd listings");
        assert!(!fair_live(1.0, 6.0));
    }
}
