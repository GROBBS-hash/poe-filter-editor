//! Reads league mechanic choices off the screen (22 Nov 2026): the lines the screen reader saw around the mouse,
//! turned into an answer. Eldritch altars (top or bottom: which reward is better, which downside is dangerous for
//! you), Expedition remnants (the mods you said you can't run), and reward screens (every item that has a price).
//!
//! Ratings are a rough guide from the words on screen, not an exact value: the window says so.

use crate::maps::MAP_DANGER;
use serde::Serialize;

/// One option on screen (an altar's top or bottom choice, or the whole thing for a remnant).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Choice {
    /// "Map Boss gains:" etc. (empty when there's no heading).
    pub heading: String,
    pub lines: Vec<ChoiceLine>,
    /// Rough reward score (higher is better); dangers count against it.
    pub score: f64,
    pub danger: bool,
    /// Where it is on screen (x, y, width, height), when the reader gave positions: for marking it over the game.
    pub area: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChoiceLine {
    pub text: String,
    /// "reward", "danger" (on your can't-run list or a known danger), or "".
    pub kind: String,
    pub note: Option<String>,
}

/// What the screen showed.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChoiceRead {
    /// "altar", "remnant", "rewards", or "unknown".
    pub kind: String,
    pub choices: Vec<Choice>,
    /// Index of the better choice (altars).
    pub pick: Option<usize>,
    /// Items with a poe.ninja price seen on screen: (name, chaos).
    pub priced: Vec<(String, f64)>,
    pub lines: Vec<String>,
}

/// Altar rewards worth having, best first: (words to look for, score).
const REWARDS: &[(&[&str], f64)] = &[
    (&["mirror"], 100.0),
    (&["divine orb"], 40.0),
    (&["sacred orb"], 30.0),
    (&["orb of annulment"], 12.0),
    (&["exalted orb"], 10.0),
    (&["orb of dominance"], 10.0),
    (&["divination card"], 9.0),
    (&["eldritch chaos"], 8.0),
    (&["eldritch exalted"], 8.0),
    (&["veiled"], 6.0),
    (&["basic currency", "duplicated"], 7.0),
    (&["currency"], 5.0),
    (&["fracturing"], 8.0),
    (&["thread"], 5.0),
    (&["fossil"], 4.0),
    (&["delirium"], 4.0),
    (&["essence"], 3.0),
    (&["unique"], 3.0),
    (&["gem"], 2.0),
    (&["map"], 2.0),
    (&["fragment"], 2.0),
    (&["breach"], 2.0),
    (&["quantity"], 3.0),
    (&["rarity"], 1.5),
];

const HEADINGS: &[&str] = &["map boss gains", "eldritch minions gain", "player gains", "players gain"];

fn danger_of(low: &str, avoid: &[String]) -> Option<String> {
    // A known danger, counted when it's on your can't-run list (or every known danger when you haven't made one).
    MAP_DANGER.iter().find(|(w, _)| w.iter().all(|x| low.contains(x))).map(|(_, n)| n.to_string())
        .filter(|n| avoid.is_empty() || avoid.iter().any(|a| a.eq_ignore_ascii_case(n)))
}

/// Reads the lines the screen reader saw. `avoid`: your can't-run list (the map check's danger names; empty = all of them); `price`: chaos for a
/// name poe.ninja knows.
pub fn read(lines: &[String], avoid: &[String], price: &dyn Fn(&str) -> Option<f64>) -> ChoiceRead {
    let placed: Vec<(String, Option<[f32; 4]>)> = lines.iter().map(|l| (l.clone(), None)).collect();
    read_placed(&placed, avoid, price)
}

/// Like `read`, with each line's place on screen, so each option knows its area (the altar marks over the game).
pub fn read_placed(lines: &[(String, Option<[f32; 4]>)], avoid: &[String], price: &dyn Fn(&str) -> Option<f64>) -> ChoiceRead {
    let kept: Vec<&(String, Option<[f32; 4]>)> = lines.iter().filter(|(l, _)| l.trim().len() > 2).collect();
    let clean: Vec<String> = kept.iter().map(|(l, _)| l.trim().to_string()).collect();
    let boxes: Vec<Option<[f32; 4]>> = kept.iter().map(|(_, b)| *b).collect();
    let low: Vec<String> = clean.iter().map(|l| l.to_lowercase()).collect();
    let is_altar = low.iter().any(|l| HEADINGS.iter().any(|h| l.contains(h)));
    let is_remnant = low.iter().any(|l| l.contains("remnant") || l.contains("logbook") || l.contains("artifact"));

    // Split into options at each heading (an altar shows its top and bottom choice one under the other).
    let mut choices: Vec<Choice> = vec![];
    let mut cur: Option<Choice> = if is_altar { None } else { Some(Choice { heading: String::new(), lines: vec![], score: 0.0, danger: false, area: None }) };
    for ((l, lo), bx) in clean.iter().zip(&low).zip(&boxes) {
        if HEADINGS.iter().any(|h| lo.contains(h)) {
            if let Some(c) = cur.take() { choices.push(c); }
            cur = Some(Choice { heading: l.clone(), lines: vec![], score: 0.0, danger: false, area: *bx });
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        c.area = grow(c.area, *bx);
        let reward = REWARDS.iter().find(|(w, _)| w.iter().all(|x| lo.contains(x)));
        // Dangers count when they hit you: on an altar, the downside lines; for a remnant, every mod.
        let danger = danger_of(lo, avoid);
        let (kind, note) = match (danger, reward) {
            // A downside that can kill you outweighs any reward.
            (Some(d), _) => { c.danger = true; c.score -= 100.0; ("danger", Some(d)) }
            (None, Some((_, s))) => {
                // "1 additional" vs "4 additional": more is better.
                let n: f64 = lo.split_whitespace().filter_map(|w| w.trim_end_matches('%').parse::<f64>().ok()).next().unwrap_or(1.0);
                let scale = if lo.contains('%') { (n / 25.0).clamp(0.2, 4.0) } else { n.clamp(1.0, 10.0) };
                c.score += s * scale;
                ("reward", None)
            }
            _ => ("", None),
        };
        c.lines.push(ChoiceLine { text: l.clone(), kind: kind.into(), note });
    }
    if let Some(c) = cur.take() { choices.push(c); }
    choices.retain(|c| !c.lines.is_empty());

    let pick = if is_altar && choices.len() >= 2 {
        choices.iter().enumerate().max_by(|a, b| a.1.score.total_cmp(&b.1.score)).map(|(i, _)| i)
    } else { None };

    // Anything on screen with a price (reward screens): whole lines that are an item name.
    let mut priced: Vec<(String, f64)> = vec![];
    for l in &clean {
        let name = l.trim_start_matches(|c: char| c.is_ascii_digit() || c == 'x' || c == ' ').trim();
        if name.len() < 4 { continue; }
        if let Some(v) = price(name) { if !priced.iter().any(|(n, _)| n == name) { priced.push((name.to_string(), v)); } }
    }
    priced.sort_by(|a, b| b.1.total_cmp(&a.1));

    let kind = if is_altar { "altar" } else if is_remnant { "remnant" } else if !priced.is_empty() { "rewards" } else { "unknown" };
    ChoiceRead { kind: kind.into(), choices, pick, priced, lines: clean }
}

/// Two screen boxes (x, y, width, height) together.
fn grow(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> Option<[f32; 4]> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
            Some([x0, y0, (a[0] + a[2]).max(b[0] + b[2]) - x0, (a[1] + a[3]).max(b[1] + b[3]) - y0])
        }
        (a, b) => a.or(b),
    }
}

/// One mark drawn over the game on an altar option.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AltarMark {
    /// x, y, width, height on screen.
    pub area: [f32; 4],
    /// "take", "skip" (worse, or has a danger for you) or "either" (about the same).
    pub verdict: String,
    /// The danger's name ("Reflect"), shown beside the cross.
    pub note: Option<String>,
}

/// Marks for an altar read with positions: the better option ticked, the other crossed (or both left plain when
/// they're about the same and neither is dangerous). None when this isn't an altar with two placed options.
pub fn altar_marks(r: &ChoiceRead) -> Option<Vec<AltarMark>> {
    if r.kind != "altar" || r.choices.len() != 2 { return None; }
    let pick = r.pick?;
    let close = (r.choices[0].score - r.choices[1].score).abs() < 1.0 && !r.choices.iter().any(|c| c.danger);
    r.choices.iter().enumerate().map(|(i, c)| {
        let verdict = if close { "either" } else if i == pick && !c.danger { "take" } else if i == pick { "either" } else { "skip" };
        let note = c.lines.iter().find_map(|l| l.note.clone());
        Some(AltarMark { area: c.area?, verdict: verdict.into(), note })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn altar_picks_the_better_reward() {
        let lines = s(&["Map Boss gains:", "Monsters reflect 15% of Elemental Damage", "Final Map Boss drops 1 additional Divine Orb",
            "Eldritch Minions gain:", "+20% to all Elemental Resistances", "Basic Currency Items dropped by slain Enemies have 12% chance to be Duplicated"]);
        let r = read(&lines, &[], &|_| None);
        assert_eq!(r.kind, "altar");
        assert_eq!(r.choices.len(), 2);
        assert!(r.choices[0].danger);
        // The Divine Orb choice reflects elemental damage: the safe currency one wins.
        assert_eq!(r.pick, Some(1));
        let safe = read(&s(&["Map Boss gains:", "+30% Monster Life", "Final Map Boss drops 1 additional Divine Orb", "Player gains:", "10% increased Item Rarity"]), &[], &|_| None);
        assert_eq!(safe.pick, Some(0));
    }

    #[test]
    fn altar_marks_over_the_game() {
        let at = |y: f32| Some([100.0, y, 300.0, 20.0]);
        let lines: Vec<(String, Option<[f32; 4]>)> = [("Map Boss gains:", 10.0), ("Monsters reflect 15% of Elemental Damage", 32.0),
            ("Final Map Boss drops 1 additional Divine Orb", 54.0), ("Eldritch Minions gain:", 90.0), ("+20% to all Elemental Resistances", 112.0),
            ("Basic Currency Items dropped by slain Enemies have 12% chance to be Duplicated", 134.0)]
            .iter().map(|(t, y)| (t.to_string(), at(*y))).collect();
        let r = read_placed(&lines, &[], &|_| None);
        let m = altar_marks(&r).unwrap();
        assert_eq!((m[0].verdict.as_str(), m[1].verdict.as_str()), ("skip", "take"));
        assert_eq!(m[0].area, [100.0, 10.0, 300.0, 64.0]);
        assert_eq!(m[1].area, [100.0, 90.0, 300.0, 64.0]);
        assert!(m[0].note.is_some());
        // Without positions there's nothing to draw.
        let plain: Vec<String> = lines.iter().map(|(t, _)| t.clone()).collect();
        assert!(altar_marks(&read(&plain, &[], &|_| None)).is_none());
    }

    #[test]
    fn remnant_and_rewards() {
        let r = read(&s(&["Remnant", "Monsters cannot be Stunned", "Players cannot Regenerate Life, Mana or Energy Shield", "25% increased quantity of Artifacts"]), &[], &|_| None);
        assert_eq!(r.kind, "remnant");
        assert!(r.choices[0].lines.iter().filter(|l| l.kind == "danger").count() >= 2);
        let price = |n: &str| if n == "Divine Orb" { Some(180.0) } else if n == "Orb of Alchemy" { Some(0.3) } else { None };
        let r = read(&s(&["Ritual", "Divine Orb", "3x Orb of Alchemy", "Defer"]), &[], &price);
        assert_eq!(r.kind, "rewards");
        assert_eq!(r.priced[0], ("Divine Orb".into(), 180.0));
    }
}
