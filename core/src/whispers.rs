//! Trade helper: reads incoming trade whispers (and buyers arriving in your
//! hideout) from the game's log, Client.txt.

use serde::Serialize;

/// An incoming whisper from another player.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Whisper {
    /// "2026/11/18 19:48:01" as the log has it.
    pub time: String,
    /// Their character name (guild tag removed).
    pub from: String,
    pub message: String,
    /// A trade request: what they want, the price, and where it is in your stash.
    pub item: Option<String>,
    pub price: Option<String>,
    pub league: Option<String>,
    pub tab: Option<String>,
    pub left: Option<u32>,
    pub top: Option<u32>,
}

/// Something worth showing from one log line.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LogEvent {
    Whisper(Whisper),
    /// A player came into your area (a buyer arriving).
    Joined { name: String },
    /// A player left your area.
    Left { name: String },
}

/// Reads one Client.txt line:
/// `2026/11/18 19:48:01 123 cff94598 [INFO Client 1234] @From <GLD> Name: Hi, I would like to buy your ...`
pub fn parse_line(line: &str) -> Option<LogEvent> {
    let line = line.trim_end();
    let time = line.get(..19).filter(|t| t.as_bytes().get(4) == Some(&b'/')).unwrap_or("").to_string();
    // The message part comes after the "[INFO Client 1234] " tag.
    let body = match line.find("] ") { Some(i) => &line[i + 2..], None => line };
    if let Some(rest) = body.strip_prefix("@From ") {
        let (who, message) = rest.split_once(": ")?;
        let from = match who.strip_prefix('<') { Some(r) => r.split_once("> ").map(|x| x.1).unwrap_or(r), None => who }.trim().to_string();
        if from.is_empty() { return None; }
        let mut w = Whisper { time, from, message: message.to_string(), item: None, price: None, league: None, tab: None, left: None, top: None };
        read_trade(&mut w);
        return Some(LogEvent::Whisper(w));
    }
    if let Some(rest) = body.strip_prefix(": ") {
        if let Some(name) = rest.strip_suffix(" has joined the area.") { return Some(LogEvent::Joined { name: name.trim().into() }); }
        if let Some(name) = rest.strip_suffix(" has left the area.") { return Some(LogEvent::Left { name: name.trim().into() }); }
    }
    None
}

/// Fills in the trade details from the trade site's standard messages:
/// "Hi, I would like to buy your X listed for 5 chaos in League (stash tab "T"; position: left 3, top 5)"
/// "Hi, I'd like to buy your 2 Divine Orb for my 360 Chaos Orb in League."
fn read_trade(w: &mut Whisper) {
    let m = w.message.clone();
    let Some(start) = m.find("like to buy your ") else { return };
    let rest = &m[start + "like to buy your ".len()..];
    // Stash position at the end, if any.
    let (main, place) = match rest.find(" (stash tab \"") {
        Some(i) => (&rest[..i], Some(&rest[i + " (stash tab \"".len()..])),
        None => (rest, None),
    };
    if let Some(p) = place {
        if let Some((tab, pos)) = p.split_once('"') {
            w.tab = Some(tab.to_string());
            let num = |key: &str| pos.find(key).and_then(|i| pos[i + key.len()..].chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok());
            w.left = num("left ");
            w.top = num("top ");
        }
    }
    let main = main.trim_end_matches('.');
    // "<item> listed for <price> in <league>" / "<item> for my <price> in <league>" / "<item> in <league>"
    let (before_league, league) = match main.rfind(" in ") {
        Some(i) => (&main[..i], Some(main[i + 4..].trim().to_string())),
        None => (main, None),
    };
    w.league = league.filter(|l| !l.is_empty());
    if let Some((item, price)) = before_league.split_once(" listed for ") {
        w.item = Some(item.trim().into());
        w.price = Some(price.trim().into());
    } else if let Some((item, price)) = before_league.split_once(" for my ") {
        w.item = Some(item.trim().into());
        w.price = Some(price.trim().into());
    } else {
        w.item = Some(before_league.trim().into());
    }
}

/// The chat line each trade button types (one per button press).
pub fn command(action: &str, name: &str, thanks: &str) -> Option<String> {
    Some(match action {
        "invite" => format!("/invite {name}"),
        "trade" => format!("/tradewith {name}"),
        "kick" => format!("/kick {name}"),
        "thanks" => format!("@{name} {}", if thanks.trim().is_empty() { "thanks, good luck!" } else { thanks.trim() }),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_trade_whispers() {
        let l = r#"2026/11/18 19:48:01 1234567 cff94598 [INFO Client 9876] @From <GLD> Seller_Bob: Hi, I would like to buy your Tabula Rasa Simple Robe listed for 5 chaos in Settlers (stash tab "~price 5 chaos"; position: left 3, top 5)"#;
        let Some(LogEvent::Whisper(w)) = parse_line(l) else { panic!("not read") };
        assert_eq!(w.time, "2026/11/18 19:48:01");
        assert_eq!(w.from, "Seller_Bob");
        assert_eq!(w.item.as_deref(), Some("Tabula Rasa Simple Robe"));
        assert_eq!(w.price.as_deref(), Some("5 chaos"));
        assert_eq!(w.league.as_deref(), Some("Settlers"));
        assert_eq!(w.tab.as_deref(), Some("~price 5 chaos"));
        assert_eq!((w.left, w.top), (Some(3), Some(5)));

        let b = "2026/11/18 19:50:00 1 a [INFO Client 1] @From Buyer: Hi, I'd like to buy your 2 Divine Orb for my 360 Chaos Orb in Keepers.";
        let Some(LogEvent::Whisper(w)) = parse_line(b) else { panic!() };
        assert_eq!((w.item.as_deref(), w.price.as_deref(), w.league.as_deref()), (Some("2 Divine Orb"), Some("360 Chaos Orb"), Some("Keepers")));

        let chat = "2026/11/18 19:50:00 1 a [INFO Client 1] @From Friend: hey you around?";
        let Some(LogEvent::Whisper(w)) = parse_line(chat) else { panic!() };
        assert!(w.item.is_none());

        assert_eq!(parse_line("2026/11/18 19:51:00 1 a [INFO Client 1] : Buyer has joined the area."), Some(LogEvent::Joined { name: "Buyer".into() }));
        assert_eq!(parse_line("2026/11/18 19:51:00 1 a [INFO Client 1] @To Bob: hi"), None);
        assert_eq!(parse_line("2026/11/18 19:51:00 1 a [DEBUG Client 1] Generating level 83 area"), None);
        assert_eq!(command("invite", "Bob", "").as_deref(), Some("/invite Bob"));
        assert_eq!(command("thanks", "Bob", "").as_deref(), Some("@Bob thanks, good luck!"));
    }
}

// ---------- is your price right? (22 Nov) ----------

/// What a trade whisper's price looks like next to poe.ninja: your asking price vs what the item is worth.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PriceCheck {
    /// Your asking price in chaos.
    pub asking: f64,
    /// poe.ninja's value for what they're buying, in chaos.
    pub worth: f64,
    /// "low" (you may be underpriced), "fair", "high" (a good sale for you).
    pub verdict: String,
}

/// "5 chaos" / "2.5 divine" / "360 Chaos Orb" -> (amount, currency id like "divine").
pub fn read_price(price: &str) -> Option<(f64, String)> {
    let p = price.trim();
    let (num, rest) = p.split_once(' ')?;
    let amount: f64 = num.replace(',', "").parse().ok()?;
    let word = rest.trim().trim_end_matches('.').to_lowercase();
    let short = |id: &str| match id { "div" | "divines" => "divine", "ex" | "exalt" | "exalts" => "exalted", "c" | "chaos orb" => "chaos", x => x }.to_string();
    let id = crate::trade::CURRENCY_NAMES.iter().find(|(id, name)| word == *id || word == name.to_lowercase() || word == format!("{}s", name.to_lowercase()))
        .map(|(id, _)| id.to_string()).unwrap_or_else(|| short(&word));
    Some((amount, id))
}

/// What the item a buyer asks for is worth: "Tabula Rasa Simple Robe" (a unique by name), or "2 Divine Orb" (bulk).
pub fn item_worth(item: &str, items: &[crate::prices::PriceItem]) -> Option<f64> {
    let item = item.trim();
    // Bulk: "<count> <name>".
    if let Some((n, rest)) = item.split_once(' ') {
        if let Ok(count) = n.parse::<f64>() {
            if let Some(v) = items.iter().filter(|i| i.name.eq_ignore_ascii_case(rest.trim()) && i.cat != "SkillGem").map(|i| i.value).reduce(f64::max) {
                return Some(v * count);
            }
        }
    }
    // A unique: its name starts the text and its base is in it; the longest name wins.
    items.iter().filter(|i| i.cat.starts_with("Unique") && !i.name.is_empty() && item.starts_with(&i.name) && item.contains(&i.base))
        .max_by_key(|i| i.name.len()).and_then(|best| {
            // Its version isn't in the whisper: no verdict when the versions' prices are far apart.
            let u = items.iter().filter(|i| i.name == best.name && i.base == best.base && i.cat.starts_with("Unique")).map(|i| i.value);
            let (lo, hi) = u.fold((f64::INFINITY, 0.0f64), |(l, h), v| (l.min(v), h.max(v)));
            (hi <= lo * 1.5).then_some(lo)
        })
        .or_else(|| items.iter().filter(|i| i.name.eq_ignore_ascii_case(item) && i.cat != "SkillGem").map(|i| i.value).reduce(f64::max))
}

/// Your asking price against poe.ninja. `rates`: chaos per currency id; `unit`: chaos per unit of the prices.
pub fn check_price(item: &str, price: &str, items: &[crate::prices::PriceItem], rates: &std::collections::HashMap<String, f64>, unit: f64) -> Option<PriceCheck> {
    let (amount, cur) = read_price(price)?;
    let asking = amount * rates.get(&cur)?;
    let worth = item_worth(item, items)? * unit;
    if worth <= 0.0 { return None; }
    let r = asking / worth;
    let verdict = if r < 0.7 { "low" } else if r > 1.3 { "high" } else { "fair" };
    Some(PriceCheck { asking, worth, verdict: verdict.into() })
}

#[cfg(test)]
mod price_tests {
    use super::*;
    use crate::prices::PriceItem;

    fn it(name: &str, base: &str, cat: &str, v: f64) -> PriceItem {
        serde_json::from_value(serde_json::json!({ "name": name, "base": base, "cat": cat, "value": v })).unwrap()
    }

    #[test]
    fn checks_asking_prices() {
        let items = vec![it("Tabula Rasa", "Simple Robe", "UniqueArmour", 20.0), it("Divine Orb", "Divine Orb", "Currency", 180.0)];
        let rates: std::collections::HashMap<String, f64> = [("chaos".to_string(), 1.0), ("divine".to_string(), 180.0)].into_iter().collect();
        assert_eq!(read_price("2.5 divine"), Some((2.5, "divine".into())));
        assert_eq!(read_price("360 Chaos Orb"), Some((360.0, "chaos".into())));
        let c = check_price("Tabula Rasa Simple Robe", "5 chaos", &items, &rates, 1.0).unwrap();
        assert_eq!((c.asking, c.worth, c.verdict.as_str()), (5.0, 20.0, "low"));
        let c = check_price("2 Divine Orb", "360 Chaos Orb", &items, &rates, 1.0).unwrap();
        assert_eq!(c.verdict, "fair");
        assert!(check_price("Doom Grip Titan Gauntlets", "1 divine", &items, &rates, 1.0).is_none());
        // Versions far apart (Voices): no telling whether the offer is fair.
        let voices = vec![it("Voices", "Large Cluster Jewel", "UniqueJewel", 185396.0), it("Voices", "Large Cluster Jewel", "UniqueJewel", 0.86)];
        assert!(check_price("Voices Large Cluster Jewel", "1 divine", &voices, &rates, 1.0).is_none());
    }
}
