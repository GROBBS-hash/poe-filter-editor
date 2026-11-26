//! "Craft: what now?" (26 Nov 2026, from the crafting research in `.claude/craft-research`): copy an item halfway through a
//! craft and get the best next click for the plan, what's left to spend (average and an unlucky run), and warnings.
//! Rules from the research: random steps first and bench crafts last; to clear a junk mod, lock the good side (3.30: 1
//! Divine a mod) and annul, since the lock sits on the other side it counts as a target for the Annul too; never suggest
//! a reroll that would wipe mods the item already has (that's "start over", which the routes cover).
use super::*;

/// One thing to do next.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NowOption {
    pub title: String,
    /// One line: why, and what it can do wrong.
    pub why: String,
    /// The currency to show a picture of.
    pub icon: String,
    /// Chance per try, 0–1.
    pub chance: f64,
    /// Chaos a try.
    pub cost_each: Option<f64>,
    /// Chaos on average to get it done.
    pub average: Option<f64>,
    /// 0 safe, 1 can fail and be tried again, 2 can take a mod you want.
    pub risk: u8,
    pub check: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WhatNow {
    pub name: String,
    pub base: String,
    pub ilvl: u32,
    pub rarity: String,
    /// The item's mods: (text, "ok" | "low" | "junk" | "fractured" | "crafted").
    pub mods: Vec<(String, String)>,
    /// Planned mods it doesn't have yet.
    pub missing: Vec<String>,
    /// Best first. Empty when it's done or nothing safe fits (then start over with a route).
    pub options: Vec<NowOption>,
    /// Chaos left to finish, on average and on an unlucky run (1 in 10).
    pub left: Option<f64>,
    pub unlucky: Option<f64>,
    pub done: bool,
    pub warnings: Vec<String>,
    pub note: String,
}

/// Tries needed so that 9 runs in 10 are done by then.
fn tries90(p: f64) -> f64 { if p >= 1.0 { 1.0 } else if p <= 0.0 { f64::INFINITY } else { (0.1f64.ln() / (1.0 - p).ln()).ceil() } }

fn icon_of(method: &str) -> String {
    let m = method.split(" + ").next().unwrap_or(method);
    if m.starts_with("Harvest") { return "Wild Crystallised Lifeforce".into() }
    if m == "Crafting bench" { return "Orb of Alteration".into() }
    m.split(" on a ").next().unwrap_or(m).to_string()
}

pub fn what_now(text: &str, targets: &[(String, u32)], price: &dyn Fn(&str) -> Option<f64>) -> Option<WhatNow> {
    let s = start(text)?;
    let p = pool(&s.base, s.ilvl, &s.influences, &s.have, price)?;
    let (class, tags) = tagset(&s.base, &s.influences)?;
    let jewel = jewel_tags(&tags);
    let cap = side_cap(jewel) as usize;
    let cost = |n: &str| price(n).or_else(|| (n == "Chaos Orb").then_some(1.0));
    let fam_of = |k: &str| p.families.iter().find(|f| f.key == k);
    let want_tier = |k: &str| targets.iter().find(|(t, _)| t == k).map(|(_, n)| *n);
    let mut warnings = Vec::new();
    let corrupted = text.lines().any(|l| l.trim() == "Corrupted" || l.trim() == "Twice Corrupted");
    let mirrored = text.lines().any(|l| l.trim() == "Mirrored");
    if corrupted || mirrored {
        warnings.push(format!("This item is {}: crafting can't change it any more.", if corrupted { "corrupted" } else { "mirrored" }));
    }

    // Sort the item's mods.
    let mut mods = Vec::new();
    let (mut pre, mut suf) = (0usize, 0usize);
    let mut good_pre = 0usize;
    let mut good_suf = 0usize;
    let mut junk_pre = 0usize;
    let mut junk_suf = 0usize;
    let mut crafted = false;
    for m in &p.item_mods {
        let prefix = m.affix.as_deref() == Some("prefix");
        if m.affix.is_some() { if prefix { pre += 1 } else { suf += 1 } }
        let wanted = m.key.as_deref().and_then(want_tier);
        let state = if m.kind == "fractured" { "fractured" }
            else if m.kind == "crafted" { crafted = true; if wanted.is_some() { "ok" } else { "crafted" } }
            else { match wanted { Some(t) if m.tier.is_none_or(|x| x <= t) => "ok", Some(_) => "low", None => "junk" } };
        match (state, prefix) {
            ("ok" | "fractured", true) => good_pre += 1,
            ("ok" | "fractured", false) => good_suf += 1,
            ("low" | "junk", true) => junk_pre += 1,
            ("low" | "junk", false) => junk_suf += 1,
            _ => {}
        }
        mods.push((m.text.clone(), state.to_string()));
    }
    let has = |k: &str, t: u32| p.item_mods.iter().any(|m| m.key.as_deref() == Some(k) && (m.kind == "crafted" || m.tier.is_none_or(|x| x <= t)));
    let missing_keys: Vec<(String, u32)> = targets.iter().filter(|(k, t)| !has(k, *t)).cloned().collect();
    let missing: Vec<String> = missing_keys.iter().filter_map(|(k, _)| fam_of(k).map(|f| f.text.replace('\n', " / "))).collect();
    let name = s.name.clone();
    let mut out = WhatNow { name, base: s.base.clone(), ilvl: s.ilvl, rarity: s.rarity.clone(), mods, missing, options: vec![], left: None, unlucky: None,
        done: missing_keys.is_empty(), warnings, note: String::new() };

    // Warnings from the research's beginner mistakes.
    for (k, t) in &missing_keys {
        let Some(f) = fam_of(k) else { continue };
        let lvl = f.tiers.iter().filter(|x| x.tier <= *t).map(|x| x.lvl).min();
        if let Some(l) = lvl.filter(|l| *l > s.ilvl) {
            out.warnings.push(format!("{}: tier {t} needs item level {l}, and this item is {}. Aim lower or start on a higher base.", f.text.replace('\n', " / "), s.ilvl));
        }
    }
    if crafted && missing_keys.iter().any(|(k, _)| fam_of(k).is_some_and(|f| !f.bench.is_empty())) {
        out.warnings.push("It already has a bench mod: only one is allowed unless you add \"Can have up to 3 Crafted Modifiers\" (2 Divine). Remove it at the bench before crafting another.".into());
    }
    if out.done || corrupted || mirrored {
        if out.done { out.note = "Everything in your plan is on it. Price it, or add more mods to the plan.".into() }
        return Some(out);
    }
    let rare = s.rarity == "Rare";
    let magic = s.rarity == "Magic";
    if !rare && !magic {
        out.note = "It has no mods to keep yet: follow the cheapest route below from the start.".into();
        return Some(out);
    }

    let lock_cost = |n: usize| -> Option<f64> {
        let div = cost("Divine Orb")?;
        if n >= 3 {
            let weapon = !matches!(class.as_str(), "Helmet" | "Gloves" | "Boots" | "Body Armour" | "Ring" | "Amulet" | "Belt" | "Quiver" | "Shield");
            let (extra, k) = if weapon { ("Sacred Orb", 1.0) } else if matches!(class.as_str(), "Ring" | "Amulet" | "Belt") { ("Exalted Orb", 8.0) } else { ("Blessed Orb", 4.0) };
            Some(div * 4.0 + cost(extra)? * k)
        } else { Some(div * n as f64) }
    };

    let mut opts: Vec<(NowOption, f64)> = Vec::new(); // option, unlucky cost
    // Clear a junk mod that takes a slot a missing mod needs (or any junk on a full item).
    let need_side = |prefix: bool| missing_keys.iter().filter(|(k, _)| fam_of(k).is_some_and(|f| (f.affix == "prefix") == prefix)).count();
    for prefix in [true, false] {
        let (n, junk, good, other_good, other_n) = if prefix { (pre, junk_pre, good_pre, good_suf, suf) } else { (suf, junk_suf, good_suf, good_pre, pre) };
        if junk == 0 || need_side(prefix) == 0 || n < cap.min(if magic { 1 } else { cap }) && n + need_side(prefix) <= cap { continue }
        let side = if prefix { "prefix" } else { "suffix" };
        let annul = cost("Orb of Annulment");
        // Lock the other side's good mods, when the lock has room on this side (a prefix lock takes a suffix slot).
        let lock = other_good > 0 && n < cap && magic.eq(&false);
        let pool_n = if lock { n + 1 } else { n + other_n };
        let chance = junk as f64 / pool_n as f64;
        let lock_c = if lock { lock_cost(other_good) } else { Some(0.0) };
        let each = annul.zip(lock_c).map(|(a, l)| a + if lock { l / pool_n as f64 } else { 0.0 });
        let average = each.zip(lock_c).map(|(e, l)| l + e / chance);
        let risk = if good > 0 || (!lock && other_good > 0) { 2 } else { 1 };
        let other = if prefix { "suffixes" } else { "prefixes" };
        let why = if lock {
            format!("The bench lock keeps your {other} safe. It sits in a {side} slot, so the Annul picks from {pool_n}: {junk} junk{}.",
                if good > 0 { format!(", {good} you want (it can take those too)") } else { String::new() })
        } else {
            format!("An Annul takes one of the item's {pool_n} mods at random{}.", if other_good + good > 0 { ": it can take one you want" } else { "" })
        };
        let title = if lock { format!("Lock your {other}, then Orb of Annulment") } else { "Orb of Annulment".into() };
        let unlucky = each.zip(lock_c).map(|(e, l)| l + e * tries90(chance)).unwrap_or(f64::INFINITY);
        opts.push((NowOption { title, why, icon: "Orb of Annulment".into(), chance, cost_each: each, average, risk,
            check: lock.then(|| "The 1–2 mod bench locks are new in 3.30: check the lock shows on the item before you annul.".into()) }, unlucky));
    }

    // Add each missing mod: random adds first, the bench last.
    let room = |prefix: bool| if prefix { pre < cap } else { suf < cap };
    let mut adds: Vec<(NowOption, f64, bool)> = Vec::new();
    for (k, t) in &missing_keys {
        let Some(f) = fam_of(k) else { continue };
        let prefix = f.affix == "prefix";
        if !room(prefix) { continue }
        let ways = ways(&s.base, s.ilvl, &s.influences, k, *t, &s.have, &s.rarity, price);
        let label = f.text.replace('\n', " / ");
        for w in ways.iter().filter(|w| w.group == "Add a mod" || w.method == "Crafting bench").take(2) {
            let bench = w.method == "Crafting bench";
            if bench && crafted { continue }
            let title = if bench { format!("Bench craft: {label}") } else { format!("{} for {label}", w.method) };
            let why = if bench { format!("{}. Crafted mods go last: anything you roll after this can wipe it.", w.note) }
                else { format!("{}. A miss adds a different mod: annul it and try again.", w.note) };
            let unlucky = w.cost_each.map(|c| c * tries90(w.chance)).unwrap_or(f64::INFINITY);
            adds.push((NowOption { title, why, icon: icon_of(&w.method), chance: w.chance, cost_each: w.cost_each, average: w.average,
                risk: if bench { 0 } else { 1 }, check: None }, unlucky, bench));
        }
    }
    // Random adds before bench crafts; cheaper first.
    adds.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.average.unwrap_or(f64::MAX).partial_cmp(&b.0.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)));
    opts.extend(adds.into_iter().map(|(o, u, _)| (o, u)));

    // What's left: the cheapest add for each missing mod, plus clearing junk where a side has no room.
    let mut left = Some(0.0);
    let mut unlucky = 0.0;
    for (k, _) in &missing_keys {
        let label = fam_of(k).map(|f| f.text.replace('\n', " / ")).unwrap_or_default();
        let best = opts.iter().filter(|(o, _)| o.title.ends_with(&label)).min_by(|a, b| a.0.average.unwrap_or(f64::MAX).partial_cmp(&b.0.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal));
        match best { Some((o, u)) => { left = left.zip(o.average).map(|(a, b)| a + b); unlucky += u } None => {} }
    }
    if let Some((o, u)) = opts.iter().find(|(o, _)| o.icon == "Orb of Annulment") { left = left.zip(o.average).map(|(a, b)| a + b); unlucky += u }
    let unknown = missing_keys.iter().any(|(k, _)| { let l = fam_of(k).map(|f| f.text.replace('\n', " / ")).unwrap_or_default(); !opts.iter().any(|(o, _)| o.title.ends_with(&l)) });
    if !unknown && !opts.is_empty() { out.left = left; out.unlucky = unlucky.is_finite().then_some(unlucky) }
    // A junk clear comes first when a missing mod has no room without it.
    out.options = opts.into_iter().map(|(o, _)| o).take(4).collect();
    if out.options.is_empty() {
        out.note = "Nothing safe adds the rest to this item: the routes below start it again.".into();
    } else if unknown {
        out.note = "Some planned mods can't be added to this item as it is: clear room first, or follow a route below.".into();
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn price(n: &str) -> Option<f64> {
        Some(match n { "Chaos Orb" => 1.0, "Divine Orb" => 200.0, "Exalted Orb" => 1.0, "Orb of Annulment" => 8.0, "Regal Orb" => 1.0, "Orb of Augmentation" => 0.1,
            "Blessed Orb" => 0.2, "Sacred Orb" => 2.0, _ => 1.0 })
    }
    const PLATE: &str = "Item Class: Body Armours\nRarity: Rare\nKraken Shell\nAstral Plate\n--------\nItem Level: 86\n--------\n{ Prefix Modifier \"Prime\" (Tier: 1) — Life }\n+112(100-119) to maximum Life\n{ Suffix Modifier \"of Tzteosh\" (Tier: 2) — Elemental, Fire, Resistance }\n+44(42-45)% to Fire Resistance\n{ Suffix Modifier \"of the Lion\" (Tier: 3) — Attribute }\n+28(28-32) to Strength\n";

    fn key(text: &str, affix: &str) -> String {
        let s = start(PLATE).unwrap();
        let p = pool(&s.base, s.ilvl, &[], &[], &price).unwrap();
        p.families.iter().find(|f| f.affix == affix && f.text.to_lowercase().contains(text)).unwrap_or_else(|| panic!("{text}")).key.clone()
    }

    #[test]
    fn half_made_plate() {
        let life = key("maximum life", "prefix");
        let fire = key("fire resistance", "suffix");
        let cold = key("cold resistance", "suffix");
        let n = what_now(PLATE, &[(life, 6), (fire, 2), (cold, 2)], &price).unwrap();
        assert!(!n.done);
        assert_eq!(n.missing.len(), 1, "{:?}", n.missing);
        assert!(n.mods.iter().any(|(t, s)| t.contains("Strength") && s == "junk"), "{:?}", n.mods);
        assert!(n.options.iter().any(|o| o.title.contains("Exalted Orb") || o.title.contains("Bench")), "{:?}", n.options);
        assert!(n.left.is_some());
        // Rarer than one in one, and the unlucky run costs more than the average.
        assert!(n.unlucky.unwrap() >= n.left.unwrap());
    }

    #[test]
    fn junk_on_a_full_side() {
        let life = key("maximum life", "prefix");
        let fire = key("fire resistance", "suffix");
        let cold = key("cold resistance", "suffix");
        let light = key("lightning resistance", "suffix");
        let chaos = key("chaos resistance", "suffix");
        let n = what_now(PLATE, &[(life, 6), (fire, 2), (cold, 2), (light, 2), (chaos, 4)], &price).unwrap();
        // Two missing suffixes, one free slot, a junk suffix: clearing it is on the list, with the prefix lock.
        let a = n.options.iter().find(|o| o.icon == "Orb of Annulment").expect("annul");
        assert!(a.title.contains("Lock your prefixes"), "{}", a.title);
        assert!((a.chance - 1.0 / 3.0).abs() < 1e-9, "{}", a.chance);
        assert_eq!(a.risk, 2);
    }

    #[test]
    fn done_and_low_level() {
        let life = key("maximum life", "prefix");
        let n = what_now(PLATE, &[(life.clone(), 6)], &price).unwrap();
        assert!(n.done && n.options.is_empty());
        let low = PLATE.replace("Item Level: 86", "Item Level: 40");
        let n = what_now(&low, &[(life, 6), (key("% increased armour", "prefix"), 1)], &price).unwrap();
        assert!(n.warnings.iter().any(|w| w.contains("needs item level")), "{:?}", n.warnings);
    }
}
