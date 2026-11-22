//! Crafting search (22 Nov 2026): a stash search that lights up the item the moment a wanted mod rolls, so you can
//! spam orbs on an item in your stash tab and stop when it glows (no Ctrl+C after every orb).
//!
//! Each planned mod becomes "number pattern + the start of the words after the number": `"([8-9]\d|\d{3}) to maximum l"`.
//! "This tier or better" turns into "at least the tier's lowest roll". The words are cut to the shortest start that no
//! other mod on this base shares, so the search stays short (the game allows 250 characters).
//!
//! Limit: the search reads the item's words and numbers, not its tiers, so two tiers whose numbers overlap look the same.

use super::{Family, Pool};
use serde::Serialize;

/// The game's search boxes take up to 250 characters.
pub const MAX: usize = 250;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CraftSearch {
    /// What to paste into the stash search box.
    pub text: String,
    /// One line per mod: (mod, its part of the search).
    pub parts: Vec<(String, String)>,
    pub too_long: bool,
    /// Things to know ("a mod without a number can't have a minimum"…).
    pub notes: Vec<String>,
}

/// A regex for a whole number of at least `n` (up to 4 digits).
pub fn at_least(n: u32) -> String {
    if n <= 1 { return r"\d+".into(); }
    let digits: Vec<u8> = n.to_string().bytes().map(|b| b - b'0').collect();
    let mut alts = vec![];
    // Same number of digits, at least n: for each position, a bigger digit there and anything after.
    for i in 0..digits.len() {
        let d = digits[i];
        let rest = digits.len() - i - 1;
        let head: String = digits[..i].iter().map(|x| x.to_string()).collect();
        if i == digits.len() - 1 {
            alts.push(format!("{head}{}", if d == 9 { "9".into() } else { format!("[{d}-9]") }));
        } else if d < 9 {
            let tail = if rest == 1 { r"\d".to_string() } else { format!(r"\d{{{rest}}}") };
            alts.push(format!("{head}{}{tail}", if d + 1 == 9 { "9".into() } else { format!("[{}-9]", d + 1) }));
        }
    }
    // Longer numbers too, up to 3 digits (mods don't roll past 999), or one more digit for bigger minimums.
    for len in digits.len() + 1..=3.max(digits.len() + 1).min(4) { alts.push(if len - 1 == 1 { r"[1-9]\d".into() } else { format!(r"[1-9]\d{{{}}}", len - 1) }); }
    if alts.len() == 1 { alts.remove(0) } else { format!("({})", alts.join("|")) }
}

/// The first line of a mod's text split at its first number: ("Adds ", lowest roll, " to (20-30) Physical Damage").
/// "(80-89)" gives 80; "30%" gives 30. None when there's no number.
fn split_number(text: &str) -> Option<(String, u32, String)> {
    let line = text.split(" / ").next().unwrap_or(text).lines().next().unwrap_or("");
    let b = line.as_bytes();
    let start = b.iter().position(|c| c.is_ascii_digit() || *c == b'(')?;
    if b[start] == b'(' {
        let close = line[start..].find(')')? + start;
        let lo: u32 = line[start + 1..close].split('-').next()?.trim().parse().ok()?;
        return Some((line[..start].to_string(), lo, line[close + 1..].to_string()));
    }
    let end = b[start..].iter().position(|c| !c.is_ascii_digit()).map(|e| e + start).unwrap_or(b.len());
    Some((line[..start].to_string(), line[start..end].parse().ok()?, line[end..].to_string()))
}

/// Rewrites later numbers in the words ("to (20-30) Physical") as \d+ so any roll matches, and escapes the rest.
fn words_pattern(words: &str) -> String {
    let mut out = String::new();
    let mut chars = words.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '(' {
            // "(20-30)" -> \d+
            let mut inner = String::new();
            for d in chars.by_ref() { if d == ')' { break; } inner.push(d); }
            out.push_str(r"\d+");
        } else if c.is_ascii_digit() {
            while chars.peek().is_some_and(|d| d.is_ascii_digit()) { chars.next(); }
            out.push_str(r"\d+");
        } else if "^$.|?*+[]{}\\".contains(c) {
            out.push('\\');
            out.push(c);
        } else if c != '"' {
            out.push(c.to_ascii_lowercase());
        }
    }
    out
}

/// The shortest start of `words` (at least 4 letters) that no other mod's words start with.
fn short_start(words: &str, others: &[String]) -> String {
    let w: Vec<char> = words.to_lowercase().chars().collect();
    for len in 4.min(w.len())..=w.len() {
        let s: String = w[..len].iter().collect();
        if s.ends_with(' ') { continue; }
        // The same words on another mod (the bench or essence version of this one) don't count.
        if !others.iter().any(|o| *o != words.to_lowercase() && o.starts_with(&s)) { return s; }
    }
    words.to_lowercase()
}

/// The shortest piece of a numberless mod's text that no other mod has (at least 4 letters).
fn short_piece(text: &str, others: &[String]) -> String {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    for len in 4.min(t.len())..=t.len() {
        for s in 0..=t.len() - len {
            let p: String = t[s..s + len].iter().collect();
            if p.starts_with(' ') || p.ends_with(' ') { continue; }
            if !others.iter().any(|o| o.contains(&p)) { return p; }
        }
    }
    text.to_lowercase()
}

fn best_text(f: &Family, tier: u32) -> (String, Option<String>) {
    // The tier asked for ("this tier or better" = at least its lowest roll); else the best one.
    let t = f.tiers.iter().find(|t| t.tier == tier).or_else(|| f.tiers.first());
    let text = t.map(|t| t.text.clone()).unwrap_or_else(|| f.text.clone());
    (text, t.map(|t| format!("T{}", t.tier)))
}

/// The search for these planned mods (key, "this tier or better"). `all`: the item must have every mod (else any one).
pub fn craft_search(pool: &Pool, targets: &[(String, u32)], all: bool) -> CraftSearch {
    let mut notes = vec![];
    // Every mod's words after its number, and every numberless mod's text, to keep the cut-down words unique.
    let words_of = |f: &Family| -> Option<String> { split_number(&best_text(f, 1).0).map(|(_, _, w)| words_pattern(&w)) };
    let mut parts: Vec<(String, String)> = vec![];
    for (key, tier) in targets {
        let Some(f) = pool.families.iter().find(|f| f.key == *key) else { continue };
        let (text, tier_name) = best_text(f, *tier);
        let label = text.split(" / ").next().unwrap_or(&text).lines().next().unwrap_or("").to_string();
        let part = match split_number(&text) {
            Some((before, lo, words)) => {
                let others: Vec<String> = pool.families.iter().filter(|o| o.key != *key).filter_map(&words_of).collect();
                let w = short_start(&words_pattern(&words), &others);
                // Nothing after the number ("Adds 3 to 5" style): keep the words before it instead.
                let w = if w.trim().is_empty() { words_pattern(&before) } else { w };
                let num = if *tier <= 1 && f.tiers.len() <= 1 { r"\d+".to_string() } else { at_least(lo) };
                if f.tiers.windows(2).any(|p| tier_name.as_deref() == Some(&format!("T{}", p[0].tier)) && overlaps(&p[0].text, &p[1].text)) {
                    notes.push(format!("{label}: its tiers overlap, so a high roll of the tier below also lights up"));
                }
                format!("{num}{w}")
            }
            None => {
                let others: Vec<String> = pool.families.iter().filter(|o| o.key != *key).map(|o| best_text(o, 1).0.to_lowercase()).collect();
                short_piece(&label, &others)
            }
        };
        if !parts.iter().any(|(_, p)| *p == part) { parts.push((label, part)); }
    }
    let quote = |p: &str| format!("\"{p}\"");
    let text = if parts.is_empty() { String::new() }
        else if all { parts.iter().map(|(_, p)| quote(p)).collect::<Vec<_>>().join(" ") }
        else { quote(&parts.iter().map(|(_, p)| p.as_str()).collect::<Vec<_>>().join("|")) };
    let too_long = text.len() > MAX;
    if too_long { notes.push(format!("The search is {} characters; the game allows {MAX}. Take a mod out of the plan.", text.len())); }
    if !parts.is_empty() { notes.push("The search reads numbers, not tiers: \"this tier or better\" means at least that tier's lowest roll.".into()); }
    CraftSearch { text, parts, too_long, notes }
}

/// A made-up copy of the item with the planned mods (each at its chosen tier's lowest roll), for a trade search.
pub fn plan_item_text(pool: &Pool, targets: &[(String, u32)]) -> String {
    let mut lines = vec![];
    for (key, tier) in targets {
        let Some(f) = pool.families.iter().find(|f| f.key == *key) else { continue };
        let (text, _) = best_text(f, *tier);
        for line in text.split(" / ").flat_map(|l| l.lines()) { lines.push(lowest_roll(line)); }
    }
    format!("Item Class: {}\nRarity: Rare\nPlanned Item\n{}\n--------\nItem Level: {}\n--------\n{}\n", game_class(&pool.class), pool.base, pool.ilvl, lines.join("\n"))
}

/// The game's item class name ("Staff" -> "Staves", "Ring" -> "Rings"; "Thrusting One Hand Sword" copies as "Thrusting One Hand Swords").
fn game_class(c: &str) -> String {
    match c {
        "Staff" => "Staves".into(),
        "Warstaff" => "Warstaves".into(),
        _ if c.ends_with('s') => c.into(),
        _ => format!("{c}s"),
    }
}

/// "+(80-89) to maximum Life" -> "+80 to maximum Life".
fn lowest_roll(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(s) = rest.find('(') {
        out.push_str(&rest[..s]);
        let Some(e) = rest[s..].find(')') else { break };
        let inner = &rest[s + 1..s + e];
        out.push_str(inner.split('-').next().unwrap_or(inner).trim());
        rest = &rest[s + e + 1..];
    }
    out.push_str(rest);
    out
}

/// Do two tier texts' first number ranges overlap?
fn overlaps(a: &str, b: &str) -> bool {
    let range = |t: &str| -> Option<(u32, u32)> {
        let s = t.find('(')?;
        let e = t[s..].find(')')? + s;
        let (lo, hi) = t[s + 1..e].split_once('-')?;
        Some((lo.trim().parse().ok()?, hi.trim().parse().ok()?))
    };
    match (range(a), range(b)) { (Some((alo, _)), Some((_, bhi))) => bhi >= alo, _ => false }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(re: &str, n: u32) -> bool {
        // A tiny check of our own patterns: alternatives of digit classes.
        let re = re.trim_start_matches('(').trim_end_matches(')');
        let s = n.to_string();
        re.split('|').any(|alt| {
            let mut toks: Vec<String> = vec![];
            let mut it = alt.chars().peekable();
            while let Some(c) = it.next() {
                match c {
                    '[' => { let mut t = String::from("["); for d in it.by_ref() { t.push(d); if d == ']' { break; } } toks.push(t); }
                    '\\' => { it.next(); if it.peek() == Some(&'{') { it.next(); let mut k = String::new(); for d in it.by_ref() { if d == '}' { break; } k.push(d); } for _ in 0..k.parse::<usize>().unwrap() { toks.push("d".into()); } } else { toks.push("d".into()); } }
                    '+' => { toks.push("+".into()); }
                    d => toks.push(d.to_string()),
                }
            }
            if toks.last().map(|t| t == "+").unwrap_or(false) { return true; }
            toks.len() == s.len() && toks.iter().zip(s.chars()).all(|(t, c)| {
                if t == "d" { c.is_ascii_digit() } else if t.starts_with('[') {
                    let b = t.as_bytes(); (b[1]..=b[3]).contains(&(c as u8))
                } else { t.starts_with(c) }
            })
        })
    }

    #[test]
    fn numbers_at_least() {
        for n in [2, 5, 9, 10, 15, 19, 30, 80, 99, 100, 115, 129, 250, 999, 1000, 1234] {
            let re = at_least(n);
            let top = if n < 100 { 1000 } else { 10000 };
            for k in 1..top { assert_eq!(matches(&re, k), k >= n, "n={n} k={k} re={re}"); }
        }
    }

    #[test]
    fn splits_mod_text() {
        assert_eq!(split_number("+(115-129) to maximum Life"), Some(("+".into(), 115, " to maximum Life".into())));
        assert_eq!(split_number("30% increased Movement Speed"), Some(("".into(), 30, "% increased Movement Speed".into())));
        assert_eq!(split_number("Adds (10-15) to (20-30) Physical Damage"), Some(("Adds ".into(), 10, " to (20-30) Physical Damage".into())));
        assert_eq!(words_pattern(" to (20-30) Physical Damage"), r" to \d+ physical damage");
        assert_eq!(split_number("Unaffected by Chilled Ground"), None);
    }

    #[test]
    fn lowest_rolls() {
        assert_eq!(lowest_roll("+(80-89) to maximum Life"), "+80 to maximum Life");
        assert_eq!(lowest_roll("Adds (10-15) to (20-30) Physical Damage"), "Adds 10 to 20 Physical Damage");
        assert_eq!(game_class("Staff"), "Staves");
        assert_eq!(game_class("Body Armour"), "Body Armours");
        assert_eq!(game_class("Gloves"), "Gloves");
    }

    #[test]
    fn real_boots() {
        let price = |_: &str| None;
        let pool = crate::craft::pool("Titan Greaves", 85, &[], &[], &price).unwrap();
        let life = pool.families.iter().find(|f| f.text.contains("to maximum Life") && f.affix == "prefix" && !f.text.contains('/') && f.chaos > 0.0).unwrap();
        let ms = pool.families.iter().find(|f| f.text.contains("increased Movement Speed") && f.chaos > 0.0).unwrap();
        let s = craft_search(&pool, &[(life.key.clone(), 2), (ms.key.clone(), 1)], false);
        assert!(s.text.starts_with('"') && s.text.contains('|') && !s.too_long, "{s:?}");
        assert!(s.parts[0].1.contains(" to max") || s.parts[0].1.contains(" to m"), "{:?}", s.parts);
        let both = craft_search(&pool, &[(life.key.clone(), 2), (ms.key.clone(), 1)], true);
        assert_eq!(both.text.matches('"').count(), 4);
    }
}
