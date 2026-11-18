//! "Show gold in a purple highlight" -> a plan the app can carry out.
//! The plan is always shown to the player before anything changes.
//! Works offline with a simple phrase reader; the app can also ask Claude
//! (with the player's own API key) to fill in the same plan from looser wording.

use crate::search::{search, Found};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Intent { Highlight, Hide, Icon, Louder, Quieter }

/// What the player asked for, before it's matched to a real item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Request {
    pub intent: Option<Intent>,
    /// The item words, e.g. "gold", "2h maces", "chaos orb".
    pub target: String,
    pub colour: Option<String>,
    pub rarity: Option<String>,
    pub min_stack: Option<u32>,
    pub below_stack: Option<u32>,
}

/// A request matched to a real item, ready to show and apply.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Plan {
    pub intent: Intent,
    pub target: Found,
    pub colour: String,
    pub rarity: Option<String>,
    pub min_stack: Option<u32>,
    pub below_stack: Option<u32>,
    /// "Highlight every Gold in purple (stacks of 500+)".
    pub summary: String,
    /// Other things the words could have meant.
    pub alternatives: Vec<Found>,
}

pub const COLOURS: [&str; 11] = ["Red", "Green", "Blue", "Brown", "White", "Yellow", "Cyan", "Grey", "Orange", "Pink", "Purple"];

fn colour_word(w: &str) -> Option<&'static str> {
    let w = w.to_lowercase();
    let alias = match w.as_str() {
        "gold" | "golden" => "yellow", "violet" | "magenta" => "purple", "teal" | "aqua" | "turquoise" => "cyan",
        "gray" | "silver" => "grey", "lime" => "green", "crimson" | "scarlet" => "red", _ => w.as_str(),
    };
    COLOURS.iter().find(|c| c.to_lowercase() == alias).copied()
}

/// Reads common phrasings without any AI.
pub fn read_offline(text: &str) -> Request {
    let lower = format!(" {} ", text.to_lowercase().replace([',', '.', '!', '?'], " "));
    let has = |p: &str| lower.contains(p);
    let mut r = Request::default();
    r.intent = Some(if has(" icon only") || has(" only icon") || has(" just the beam") || has(" just beam") || has(" only the beam") || has(" no label") {
        Intent::Icon
    } else if has(" hide ") || has("don't show") || has("dont show") || has(" remove ") || has(" stop showing") || has(" get rid of ") {
        Intent::Hide
    } else if has(" louder") || has(" loud ") || has(" bigger") {
        Intent::Louder
    } else if has(" quieter") || has(" silent") || has(" mute") || has(" smaller") {
        Intent::Quieter
    } else {
        Intent::Highlight
    });

    let words: Vec<&str> = lower.split_whitespace().collect();
    let num = |w: &str| w.trim_end_matches('+').replace(['k', 'K'], "000").parse::<u32>().ok();
    let mut skip: BTreeSet<usize> = BTreeSet::new();
    for (i, w) in words.iter().enumerate() {
        // The colour: but "gold" is also an item, so only treat it as a colour after "in"/"as"/"with".
        if let Some(c) = colour_word(w) {
            let after_in = i > 0 && ["in", "as", "with", "colour", "color", "a", "bright", "light", "dark"].contains(&words[i - 1]);
            if *w != "gold" || after_in {
                r.colour = Some(c.to_string());
                skip.insert(i);
                continue;
            }
        }
        if ["normal", "magic", "rare", "unique", "uniques", "rares"].contains(w) {
            let cap = w.trim_end_matches('s');
            r.rarity = Some(cap[..1].to_uppercase() + &cap[1..]);
            skip.insert(i);
        }
        if let Some(n) = num(w) {
            let before = if i > 0 { words[i - 1] } else { "" };
            let before2 = if i > 1 { words[i - 2] } else { "" };
            let after = words.get(i + 1).copied().unwrap_or("");
            let more = w.ends_with('+') || ["over", "above"].contains(&before) || (before == "than" && before2 == "more")
                || (after == "or" && words.get(i + 2) == Some(&"more")) || before == "least";
            let less = ["under", "below"].contains(&before) || (before == "than" && ["less", "fewer"].contains(&before2));
            if more { r.min_stack = Some(n); skip.insert(i); }
            if less { r.below_stack = Some(n); skip.insert(i); }
        }
    }
    // Item words: what's left after dropping instructions and filler.
    const FILLER: [&str; 58] = ["show", "make", "highlight", "hide", "remove", "don't", "dont", "stop", "showing", "get", "rid", "of",
        "in", "a", "an", "the", "with", "as", "on", "my", "filter", "please", "can", "you", "i", "want", "to", "stand", "out",
        "colour", "color", "highlighted", "bright", "light", "dark", "icon", "only", "just", "beam", "no", "label", "louder", "loud",
        "quieter", "silent", "mute", "bigger", "smaller", "stacks", "stack", "over", "above", "under", "below", "than", "more", "less", "or"];
    r.target = words.iter().enumerate()
        .filter(|(i, w)| !skip.contains(i) && !FILLER.contains(w) && num(w).is_none() && !MORE_FILLER.contains(w))
        .map(|(_, w)| *w).collect::<Vec<_>>().join(" ");
    r
}

const MORE_FILLER: [&str; 28] = ["fewer", "least", "at", "and", "all", "every", "any", "would", "like", "need", "could", "have",
    "see", "me", "it", "them", "they", "be", "so", "that", "for", "is", "are", "drops", "drop", "items", "do", "thing"];

/// Searches the whole phrase, then shorter and shorter parts of it.
fn find_target(target: &str, extra: &BTreeSet<String>) -> Vec<Found> {
    let words: Vec<&str> = target.split_whitespace().collect();
    for len in (1..=words.len()).rev() {
        for w in words.windows(len) {
            let r = search(&w.join(" "), extra, 6);
            // Parts of the phrase must match whole words of a name, not just letters inside one.
            let whole_words = |f: &Found| {
                let name = format!("{} {}", f.base_type, f.item_class).to_lowercase();
                let names: Vec<&str> = name.split(|c: char| !c.is_alphanumeric()).collect();
                w.iter().all(|q| q.len() >= 3 && names.iter().any(|n| n.trim_end_matches('s') == q.trim_end_matches('s')
                    || crate::search::expand(q).split(' ').all(|e| names.iter().any(|n| n.starts_with(e.trim_end_matches('s'))))))
            };
            let r: Vec<Found> = if len == words.len() { r } else { r.into_iter().filter(|f| whole_words(f)).collect() };
            if !r.is_empty() { return r; }
        }
    }
    vec![]
}

/// Matches a request to a real item and writes a plain summary.
pub fn plan(req: &Request, extra_bases: &BTreeSet<String>) -> Option<Plan> {
    let found = find_target(&req.target, extra_bases);
    let target = found.first()?.clone();
    let intent = req.intent.unwrap_or(Intent::Highlight);
    let colour = req.colour.clone().unwrap_or_else(|| "Purple".into());
    let what = if target.base_type.is_empty() { format!("every {}", target.item_class) } else { target.base_type.clone() };
    let what = match &req.rarity { Some(r) => format!("{r} {what}"), None => what };
    let mut summary = match intent {
        Intent::Highlight => format!("Highlight {what} in {}", colour.to_lowercase()),
        Intent::Hide => format!("Hide {what}"),
        Intent::Icon => format!("Show only a beam and map icon for {what} (no label)"),
        Intent::Louder => format!("Make {what} bigger and louder"),
        Intent::Quieter => format!("Make {what} smaller and silent"),
    };
    match (req.min_stack, req.below_stack) {
        (Some(a), Some(b)) => summary.push_str(&format!(", stacks {a} to {}", b.saturating_sub(1))),
        (Some(a), None) => summary.push_str(&format!(", stacks of {a} or more")),
        (None, Some(b)) => summary.push_str(&format!(", stacks under {b}")),
        _ => {}
    }
    Some(Plan {
        intent, target, colour, rarity: req.rarity.clone(), min_stack: req.min_stack, below_stack: req.below_stack,
        summary, alternatives: found.into_iter().skip(1).collect(),
    })
}

/// The tool definition handed to Claude so it fills in a `Request`.
pub fn claude_tool() -> serde_json::Value {
    serde_json::json!({
        "name": "make_rule",
        "description": "Turn a Path of Exile player's loot filter request into one structured rule.",
        "input_schema": {
            "type": "object",
            "properties": {
                "intent": {"type": "string", "enum": ["highlight", "hide", "icon", "louder", "quieter"],
                    "description": "highlight = make it stand out in a colour; icon = no label, just beam and minimap icon"},
                "target": {"type": "string", "description": "The item or item type in game terms, e.g. 'Gold', 'Two Hand Maces', 'Chaos Orb', 'Divination Cards'"},
                "colour": {"type": "string", "enum": COLOURS},
                "rarity": {"type": "string", "enum": ["Normal", "Magic", "Rare", "Unique"]},
                "min_stack": {"type": "integer", "description": "Only stacks of at least this many"},
                "below_stack": {"type": "integer", "description": "Only stacks smaller than this"}
            },
            "required": ["intent", "target"]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(t: &str) -> Plan { plan(&read_offline(t), &BTreeSet::new()).unwrap_or_else(|| panic!("no plan for {t}")) }

    #[test]
    fn everyday_requests() {
        let a = p("I would like to show gold in a purple highlight");
        assert_eq!((a.intent, a.target.base_type.as_str(), a.colour.as_str()), (Intent::Highlight, "Gold", "Purple"));

        let b = p("hide gold under 500");
        assert_eq!((b.intent, b.below_stack), (Intent::Hide, Some(500)));
        assert_eq!(b.summary, "Hide Gold, stacks under 500");

        let c = p("make 2h maces stand out in cyan");
        assert_eq!((c.target.item_class.as_str(), c.target.base_type.as_str(), c.colour.as_str()), ("Two Hand Maces", "", "Cyan"));

        let d = p("chaos orbs 3+ in red");
        assert_eq!((d.target.base_type.as_str(), d.min_stack, d.colour.as_str()), ("Chaos Orb", Some(3), "Red"));

        let e = p("icon only for scrolls of wisdom");
        assert_eq!((e.intent, e.target.base_type.as_str()), (Intent::Icon, "Scroll of Wisdom"));

        let f = p("highlight rare amulets in gold");
        assert_eq!((f.target.item_class.as_str(), f.rarity.as_deref(), f.colour.as_str()), ("Amulets", Some("Rare"), "Yellow"));
    }

    #[test]
    fn nonsense_gives_no_plan() {
        assert!(plan(&read_offline("please do the thing"), &BTreeSet::new()).is_none());
    }
}
