//! A NeverSink step's conditions ("item level 86+", "corrupted", "3+ mods") as chips the Tiers page can change.
//! Each condition is keyed by its filter keyword; a change replaces that keyword's line(s) in the step.
//! "ModCount" is ours: NeverSink counts mods with `HasExplicitMod >=N "a" "e" "i" "o" "u" "y"` (every mod name has a
//! vowel), which only works on identified items, so it's kept apart from his named-mod lists ("Shimmering", …).

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Cond {
    /// Filter keyword ("ItemLevel", "Corrupted", …) or "ModCount".
    pub key: String,
    /// Everything after the keyword, e.g. ">= 86", "True", "Normal Magic". Several lines joined with "\n".
    pub value: String,
    /// In plain words, e.g. "item level 86+".
    pub words: String,
    /// How the app edits it: "number" (a threshold), "yesno", "rarity", "fixed" (shown, not changeable).
    pub kind: String,
}

/// Numbers you can set a threshold on, with their plain name.
pub const NUMBERS: &[(&str, &str)] = &[
    ("ItemLevel", "item level"), ("AreaLevel", "area level"), ("Quality", "quality"), ("StackSize", "stack of"),
    ("LinkedSockets", "links"), ("Sockets", "sockets"), ("GemLevel", "gem level"), ("MapTier", "map tier"),
    ("EnchantmentPassiveNum", "passives"), ("CorruptedMods", "corruption mods"), ("UnidentifiedItemTier", "item tier"),
    ("MemoryStrands", "memory strands"), ("DropLevel", "drop level"), ("ModCount", "mods"), ("Width", "width"), ("Height", "height"),
];
/// Yes/no conditions and their words for True / False.
pub const YESNO: &[(&str, &str, &str)] = &[
    ("Corrupted", "corrupted", "not corrupted"), ("Identified", "identified", "unidentified"),
    ("Mirrored", "mirrored", "not mirrored"), ("FracturedItem", "fractured", "not fractured"),
    ("SynthesisedItem", "synthesised", "not synthesised"), ("Replica", "replica", "not replica"),
    ("AnyEnchantment", "enchanted", "not enchanted"), ("BlightedMap", "blighted", "not blighted"),
    ("TransfiguredGem", "transfigured", "not transfigured"), ("Foulborn", "foulborn", "not foulborn"),
];
/// Never shown as chips: the items themselves are shown as pictures, the look lines are the look.
const SKIP: &[&str] = &["BaseType", "Continue"];

fn is_mod_count(rest: &str) -> bool {
    let r = rest.trim();
    r.starts_with(">=") && r.contains("\"a\"") && r.contains("\"e\"") && r.contains("\"u\"")
}

/// "86" from ">= 86" / ">=86" / "86".
fn split_op(v: &str) -> (&str, &str) {
    let v = v.trim();
    for op in [">=", "<=", "==", "!=", ">", "<", "="] {
        if let Some(r) = v.strip_prefix(op) { return (op, r.trim()); }
    }
    ("", v)
}

/// A threshold in words: "item level 86+", "item level up to 67", "passives: 12".
pub fn number_words(name: &str, value: &str) -> String {
    // "68+" and "up to 74" together read as "68–74".
    let lines: Vec<&str> = value.split('\n').collect();
    if let [a, b] = lines.as_slice() {
        let (oa, na) = split_op(a);
        let (ob, nb) = split_op(b);
        if oa == ">=" && ob == "<=" && !matches!(name, "mods" | "links" | "passives" | "sockets" | "corruption mods") {
            return format!("{} {na}–{nb}", if name == "stack of" { "stacks of" } else { name });
        }
    }
    let parts: Vec<String> = value.split('\n').map(|line| {
        let (op, n) = split_op(line);
        let n = n.split_whitespace().next().unwrap_or("");
        let n_val: i64 = n.parse().unwrap_or(0);
        match op {
            ">=" => format!("{name} {n}+"),
            ">" => format!("{name} {}+", n_val + 1),
            "<=" => format!("{name} up to {n}"),
            "<" => format!("{name} up to {}", n_val - 1),
            _ => format!("{name} {n}"),
        }
    }).collect();
    // "stack of 3+" reads better as "stacks of 3+", "mods 3+" as "3+ mods".
    let s = parts.join(", ");
    match name {
        "stack of" => s.replacen("stack of", "stacks of", 1),
        "mods" => s.strip_prefix("mods ").map(|r| format!("{r} mods")).unwrap_or(s),
        "links" | "passives" | "sockets" | "corruption mods" => s.strip_prefix(&format!("{name} ")).map(|r| format!("{r} {name}")).unwrap_or(s),
        _ => s,
    }
}

fn plain_list(v: &str) -> Vec<String> {
    v.split('"').map(str::trim).filter(|x| !x.is_empty() && !["==", "=", "!="].contains(x)).map(String::from).collect()
}

/// The conditions of one step, in filter order (same keyword twice is merged, e.g. two AreaLevel lines).
pub fn conds_of(lines: &[String]) -> Vec<Cond> {
    let mut out: Vec<Cond> = vec![];
    for l in lines {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') { continue; }
        let (kw, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
        let rest = rest.trim();
        if SKIP.contains(&kw) || kw.starts_with("Set") || kw.starts_with("Play") || kw.starts_with("Minimap")
            || kw.starts_with("Custom") || kw.starts_with("Disable") || kw.starts_with("Enable") { continue; }
        let key = if kw == "HasExplicitMod" && is_mod_count(rest) { "ModCount" } else { kw };
        let value = if key == "ModCount" {
            let n: String = rest.trim_start_matches(">=").chars().take_while(|c| c.is_ascii_digit() || *c == ' ').collect();
            format!(">= {}", n.trim())
        } else { rest.to_string() };
        if let Some(c) = out.iter_mut().find(|c| c.key == key) {
            c.value = format!("{}\n{value}", c.value);
            c.words = words(key, &c.value).0;
            continue;
        }
        let (w, kind) = words(key, &value);
        out.push(Cond { key: key.into(), value, words: w, kind: kind.into() });
    }
    out
}

/// A condition in plain words, and how it's edited.
pub fn words(key: &str, value: &str) -> (String, &'static str) {
    if let Some((_, name)) = NUMBERS.iter().find(|n| n.0 == key) { return (number_words(name, value), "number"); }
    if let Some((_, yes, no)) = YESNO.iter().find(|n| n.0 == key) {
        return ((if value.trim().eq_ignore_ascii_case("true") { *yes } else { *no }).to_string(), "yesno");
    }
    match key {
        "Rarity" => {
            let (op, r) = split_op(value);
            let names: Vec<String> = r.split_whitespace().map(|x| match x { "Normal" => "white".into(), o => o.to_lowercase() }).collect();
            let s = match op { "<=" | "<" => format!("{} or lower", names.join(" ")), ">=" | ">" => format!("{} or better", names.join(" ")),
                _ => match names.as_slice() { [a] => a.clone(), [a, b] => format!("{a} or {b}"), _ => names.join(", ") } };
            (s, "rarity")
        }
        "Class" => {
            let c = plain_list(value);
            (if c.len() <= 3 { c.join(", ") } else { format!("{} item types", c.len()) }, "fixed")
        }
        "HasExplicitMod" | "HasImplicitMod" => {
            let (op, rest) = split_op(value);
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let mods = plain_list(value);
            let few = if mods.len() <= 3 { mods.join(", ") } else { format!("{}, {} or another of {} mods", mods[0], mods[1], mods.len()) };
            (if n.is_empty() || op.is_empty() { format!("has a mod: {few}") } else { format!("{n}+ of these mods: {few}") }, "fixed")
        }
        "HasInfluence" => (format!("influence: {}", plain_list(value).join(", ").to_lowercase()), "fixed"),
        "SocketGroup" => (format!("socket colours {}", value.trim_start_matches(['=', '>', ' '])), "fixed"),
        "EnchantmentPassiveNode" => {
            let n = plain_list(value);
            (if n.len() <= 3 { format!("cluster: {}", n.join(", ")) } else { format!("cluster: {}, {} or {} more", n[0], n[1], n.len() - 2) }, "fixed")
        }
        other => (format!("{} {}", other, value), "fixed"),
    }
}

/// A condition's plain name with no value ("item level", "corrupted"), e.g. for "item level condition removed".
pub fn name_of(key: &str) -> String {
    NUMBERS.iter().find(|n| n.0 == key).map(|n| n.1.to_string())
        .or_else(|| YESNO.iter().find(|n| n.0 == key).map(|n| n.1.to_string()))
        .unwrap_or_else(|| key.to_lowercase())
}

/// The filter line(s) for one condition. "ModCount" -> NeverSink's vowel trick.
pub fn lines_for(key: &str, value: &str) -> Vec<String> {
    value.split('\n').filter(|v| !v.trim().is_empty()).map(|v| {
        if key == "ModCount" {
            let (_, n) = split_op(v);
            format!("HasExplicitMod >={} \"a\" \"e\" \"i\" \"o\" \"u\" \"y\"", n.trim())
        } else { format!("{key} {}", v.trim()) }
    }).collect()
}

/// Does this filter line belong to `key`?
pub fn line_is(line: &str, key: &str) -> bool {
    let t = line.trim();
    let (kw, rest) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    if key == "ModCount" { return kw == "HasExplicitMod" && is_mod_count(rest); }
    if kw == "HasExplicitMod" && key == "HasExplicitMod" { return !is_mod_count(rest); }
    kw == key
}

/// Your changes to a step's conditions: keyword -> new value ("" = condition removed).
pub type CondEdits = BTreeMap<String, String>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{parse_filter, Segment};

    fn block(text: &str) -> crate::filter::Block {
        match parse_filter(text).into_iter().find(|s| matches!(s, Segment::Block(_))) { Some(Segment::Block(b)) => b, _ => panic!() }
    }

    #[test]
    fn reads_neversinks_jewel_steps() {
        let b = block("Show # $type->jewels->abyss $tier->extramodcorruptedabyssjewel\n\tCorrupted True\n\tRarity Rare\n\tClass == \"Abyss Jewels\"\n\tHasExplicitMod >=5 \"a\" \"e\" \"i\" \"o\" \"u\" \"y\"\n\tItemLevel <= 67\n\tSetFontSize 45\n");
        let c = conds_of(&b.lines);
        let w: Vec<&str> = c.iter().map(|c| c.words.as_str()).collect();
        assert_eq!(w, ["corrupted", "rare", "Abyss Jewels", "5+ mods", "item level up to 67"]);
        assert_eq!(c[3].key, "ModCount");
        assert_eq!(number_words("passives", "<= 8"), "up to 8 passives");
        assert_eq!(number_words("item level", ">= 75\n<= 85"), "item level 75–85");
    }

    #[test]
    fn changes_and_adds_conditions() {
        let mut b = block("Show # $type->jewels->generic $tier->anyrare\n\tRarity Rare\n\tHasExplicitMod \"Shimmering\"\n\tBaseType == \"Cobalt Jewel\"\n\tSetFontSize 45\n");
        let mut e = CondEdits::new();
        e.insert("ItemLevel".into(), ">= 84".into());
        e.insert("ModCount".into(), ">= 3".into());
        e.insert("Corrupted".into(), "False".into());
        b.set_conds(&e);
        let w: Vec<String> = conds_of(&b.lines).into_iter().map(|c| c.words).collect();
        assert!(w.contains(&"item level 84+".to_string()) && w.contains(&"3+ mods".to_string()) && w.contains(&"not corrupted".to_string()));
        // His named-mod list stays when the mod count changes.
        assert!(w.contains(&"has a mod: Shimmering".to_string()));
        // Removing one.
        let mut e = CondEdits::new();
        e.insert("Rarity".into(), "".into());
        b.set_conds(&e);
        assert!(!conds_of(&b.lines).iter().any(|c| c.key == "Rarity"));
        assert!(b.render().contains("SetFontSize 45"));
    }
}
