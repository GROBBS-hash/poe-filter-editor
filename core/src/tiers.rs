//! NeverSink's tier lists (currency, div cards, uniques...) for the Tiers tab:
//! like FilterBlade, each section is a stack of tiers you can show/hide,
//! restyle, and drag items between.

use crate::evaluate::{style_of, Style};
use crate::filter::Segment;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TierRow {
    pub tier: String,
    /// "Tier 1", "Tier 2 (Divine)", "Exotic: hidden"...
    pub name: String,
    pub visibility: String,
    pub style: Style,
    pub bases: Vec<String>,
    /// Extra conditions in plain words, e.g. "stacks of 3+".
    pub note: String,
    /// "StackSize >= N" tiers (stacks, gold): N, so the app can offer a slider.
    pub min_stack: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Section {
    /// e.g. ["currency", "stackedthree"].
    pub type_path: Vec<String>,
    pub title: String,
    pub group: String,
    pub tiers: Vec<TierRow>,
}

/// Known sections: (type path, group, title). Order here is the order shown.
const KNOWN: &[(&str, &str, &str)] = &[
    ("currency", "Currency", "Currency"),
    ("currency->stackedthree", "Currency", "Stacks of 3+"),
    ("currency->stackedsix", "Currency", "Stacks of 6+"),
    ("currency->stackedsupplieswisdom", "Currency", "Wisdom scroll stacks"),
    ("currency->stackedsuppliesportal", "Currency", "Portal scroll stacks"),
    ("currency->stackedsupplieshigh", "Currency", "Transmutation stacks"),
    ("currency->stackedsupplieslow", "Currency", "Augmentation stacks"),
    ("currency->leveling", "Currency", "Campaign currency"),
    ("currency->levelingstacked", "Currency", "Campaign currency stacks"),
    ("divination", "Divination cards", "Divination cards"),
    ("uniques", "Uniques", "Uniques"),
    ("uniques->replicas", "Uniques", "Replica uniques"),
    ("uniques->foulborn", "Uniques", "Foulborn uniques"),
    ("fragments", "Fragments", "Fragments"),
    ("currency->splinter->simulacrum", "Fragments", "Simulacrum splinters"),
    ("currency->stackedsplintershigh", "Fragments", "Splinter stacks (high)"),
    ("currency->stackedsplinterslow", "Fragments", "Splinter stacks (low)"),
    ("currency->essence", "League currency", "Essences"),
    ("currency->fossil", "League currency", "Fossils"),
    ("currency->oil", "League currency", "Oils"),
    ("currency->deliriumorbs", "League currency", "Delirium orbs"),
    ("currency->trial->omen", "League currency", "Omens"),
    ("currency->trial->tattoo", "League currency", "Tattoos"),
    ("currency->harvest", "League currency", "Lifeforce"),
    ("currency->deadmansulphur", "League currency", "Sulphur"),
    ("runesgrafts", "League currency", "Runegrafts"),
    ("corpses", "League currency", "Corpses"),
    ("viscera", "League currency", "Viscera"),
    ("gold", "Currency", "Gold"),
    ("exotic->fractured", "Gear", "Fractured bases"),
    ("rare->exotic->veiled", "Gear", "Veiled items"),
    ("exoticbases", "Gear", "Exotic bases"),
];

/// "t2divine" -> "Tier 2 (divine)", "exhide" -> "Hidden extras".
pub fn tier_name(t: &str) -> String {
    match t {
        "exhide" => return "Hidden extras".into(),
        "tportal" => return "Portal scrolls".into(),
        "twisdom" => return "Wisdom scrolls".into(),
        "scrollfragments" => return "Scroll fragments".into(),
        "anyother" | "any" => return "Any other".into(),
        _ => {}
    }
    if let Some(rest) = t.strip_prefix('t') {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() {
            let tail = rest[digits.len()..].trim_start_matches('_');
            return if tail.is_empty() { format!("Tier {digits}") } else { format!("Tier {digits} ({tail})") };
        }
    }
    let mut c = t.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn gold_name(t: &str) -> String {
    match t {
        "stack3" => "Huge piles".into(), "stack2" => "Large piles".into(), "stack1" => "Medium piles".into(),
        "stacklvl1" => "Small piles (levelling only)".into(), "anyother" => "Any other gold".into(), _ => tier_name(t),
    }
}

fn note_of(lines: &[String]) -> String {
    let mut out = vec![];
    for l in lines {
        let t = l.trim();
        if let Some(v) = t.strip_prefix("StackSize") {
            let v = v.trim();
            if let Some(n) = v.strip_prefix(">=") { out.push(format!("stacks of {}+", n.trim())); }
            else if let Some(n) = v.strip_prefix('>') { out.push(format!("stacks over {}", n.trim())); }
            else if let Some(n) = v.strip_prefix('<') { out.push(format!("stacks under {}", n.trim_start_matches('=').trim())); }
        } else if let Some(v) = t.strip_prefix("AreaLevel") {
            out.push(format!("area level {}", v.trim()));
        }
    }
    out.join(", ")
}

/// Every tier-listed section in a filter: known ones first in a fixed order,
/// or (when none are known, e.g. PoE 2) any section with 3+ base-listed tiers.
pub fn sections(segs: &[Segment]) -> Vec<Section> {
    let mut found: Vec<Section> = vec![];
    for s in segs {
        let Segment::Block(b) = s else { continue };
        let Some(tier) = b.tier.clone() else { continue };
        let bases = b.base_types();
        if bases.is_empty() || b.type_path.is_empty() { continue; }
        let key = b.type_path.join("->");
        let i = match found.iter().position(|x| x.type_path == b.type_path) {
            Some(i) => i,
            None => {
                let (group, title) = KNOWN.iter().find(|k| k.0 == key).map(|k| (k.1.to_string(), k.2.to_string()))
                    .unwrap_or_else(|| ("Other".into(), b.type_path.iter().map(|p| tier_name(p)).collect::<Vec<_>>().join(" › ")));
                found.push(Section { type_path: b.type_path.clone(), title, group, tiers: vec![] });
                found.len() - 1
            }
        };
        // The same tier can appear twice (e.g. a campaign copy); keep the first.
        if found[i].tiers.iter().any(|t| t.tier == tier) { continue; }
        found[i].tiers.push(TierRow {
            name: if key == "gold" { gold_name(&tier) } else { tier_name(&tier) },
            tier, visibility: b.visibility.clone(), style: style_of(&b.lines), bases, note: note_of(&b.lines),
            min_stack: b.lines.iter().find_map(|l| l.trim().strip_prefix("StackSize >=").and_then(|n| n.trim().parse().ok())),
        });
    }
    let rank = |s: &Section| KNOWN.iter().position(|k| k.0 == s.type_path.join("->")).unwrap_or(usize::MAX);
    // Unknown sections only when none are known (e.g. PoE 2's different names).
    let any_known = found.iter().any(|s| rank(s) != usize::MAX);
    found.retain(|s| if any_known { rank(s) != usize::MAX } else { s.tiers.len() >= 3 });
    found.sort_by_key(|s| rank(s));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{parse_filter, render_filter};
    use crate::rules::{apply, Rule};

    fn fixture() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap()
    }

    #[test]
    fn lists_real_tiers() {
        let s = sections(&parse_filter(&fixture()));
        let cur = &s[0];
        assert_eq!(cur.title, "Currency");
        assert!(cur.tiers[0].bases.contains(&"Mirror of Kalandra".to_string()), "{:?}", cur.tiers[0]);
        assert_eq!(tier_name("t2divine"), "Tier 2 (divine)");
        assert_eq!(tier_name("twisdom"), "Wisdom scrolls");
        let w = s.iter().find(|x| x.title == "Wisdom scroll stacks").unwrap();
        assert_eq!(w.tiers.iter().map(|t| t.min_stack.unwrap()).collect::<Vec<_>>(), vec![10, 5, 3]);
        let stacks = s.iter().find(|x| x.title == "Stacks of 3+").unwrap();
        assert!(stacks.tiers[0].note.contains("stacks of 3+"), "{:?}", stacks.tiers[0].note);
        assert!(s.iter().any(|x| x.title == "Divination cards" && x.tiers.len() >= 5));
        println!("{:?}", s.iter().map(|x| (&x.group, &x.title, x.tiers.len())).collect::<Vec<_>>());
    }

    #[test]
    fn drag_item_between_tiers() {
        let mut segs = parse_filter(&fixture());
        let before = sections(&segs);
        let from = before[0].tiers.iter().find(|t| t.bases.contains(&"Chaos Orb".to_string())).unwrap().tier.clone();
        let rules = vec![Rule::MoveBase { type_path: vec!["currency".into()], base: "Chaos Orb".into(), to_tier: "t2divine".into() }];
        assert!(apply(&mut segs, &rules, "\r\n").is_empty());
        let after = sections(&parse_filter(&render_filter(&segs)));
        let has = |t: &str| after[0].tiers.iter().find(|x| x.tier == t).unwrap().bases.contains(&"Chaos Orb".to_string());
        assert!(has("t2divine") && !has(&from));
    }

    #[test]
    fn add_a_currency_to_a_stack_tier() {
        use crate::evaluate::status;
        let text = fixture();
        let segs0 = parse_filter(&text);
        let secs = sections(&segs0);
        let stacked: Vec<&String> = secs.iter().filter(|x| x.title.starts_with("Stacks") || x.title.ends_with("stacks"))
            .flat_map(|x| x.tiers.iter().flat_map(|t| t.bases.iter())).collect();
        // A currency NeverSink only tiers as single drops.
        let base = secs[0].tiers.iter().flat_map(|t| t.bases.iter()).find(|b| !stacked.contains(b)).unwrap().clone();
        let mut segs = parse_filter(&text);
        apply(&mut segs, &[Rule::MoveBase { type_path: vec!["currency".into(), "stackedsix".into()], base: base.clone(), to_tier: "t7".into() }], "\r\n");
        let segs = parse_filter(&render_filter(&segs));
        let it = |n| crate::item::Item { item_class: "Stackable Currency".into(), rarity: "Currency".into(), base_type: base.clone(), stack_size: Some(n), ..Default::default() };
        let v6 = status(&segs, &it(6), Some(70), false).verdict;
        let v1 = status(&segs, &it(1), Some(70), false).verdict;
        assert!(v6.decided_by.contains("stackedsix") && v6.decided_by.contains("t7"), "{base}: {}", v6.decided_by);
        assert!(!v1.decided_by.contains("stackedsix"), "single drops keep their normal tier");
    }

    #[test]
    fn tier_rule_hides_and_restyles() {
        let mut segs = parse_filter(&fixture());
        let rules = vec![
            Rule::Tier { type_path: vec!["currency".into()], tier: "t5alchemy".into(), show: Some(false), actions: None, min_stack: None },
            Rule::Tier { type_path: vec!["gold".into()], tier: "stack2".into(), show: None, actions: None, min_stack: Some(800) },
            Rule::Tier { type_path: vec!["divination".into()], tier: "t1".into(), show: None,
                actions: Some(vec!["SetFontSize 45".into(), "SetTextColor 1 2 3 255".into()]), min_stack: None },
        ];
        let w = apply(&mut segs, &rules, "\r\n");
        assert!(w.is_empty(), "{w:?}");
        let out = parse_filter(&render_filter(&segs));
        let s = sections(&out);
        let alch = s[0].tiers.iter().find(|t| t.tier == "t5alchemy").unwrap();
        assert_eq!(alch.visibility, "Hide");
        let div = s.iter().find(|x| x.title == "Divination cards").unwrap();
        let t1 = div.tiers.iter().find(|t| t.tier == "t1").unwrap();
        assert_eq!(t1.style.text, Some([1, 2, 3, 255]));
        assert_eq!(t1.style.sound, None, "old sound removed");
        let gold = s.iter().find(|x| x.title == "Gold").unwrap();
        let large = gold.tiers.iter().find(|t| t.tier == "stack2").unwrap();
        assert_eq!((large.name.as_str(), large.min_stack), ("Large piles", Some(800)));
    }
}
