//! "How to make it": whole crafting routes for the mods in the plan, side by side, so you can pick the cheapest, the
//! one with the fewest clicks or the safest. Each route is a list of steps; each step lists its ways (cheapest first)
//! with the odds and cost, worked out by rolling the item here many times (the same maths as the rest of the page).
//!
//! The routes (3.30 Covens of Umbra rules):
//! - Spam it all: one method (Chaos, fossils, an essence, a Harvest reforge) until every mod is there at once.
//! - One side, lock, then the other: get the prefixes (or suffixes), lock them at the bench (3.30: 1 Divine a mod, or
//!   the full side lock), Harvest-reforge the other side.
//! - Magic first: Alteration (+ Augmentation) for the hardest prefix and suffix, Regal, then add the rest.
//! - Spam the hardest, then slam: reroll for the hard part (leaving room), add the rest one at a time with the best
//!   kind of Exalted Orb (blocked with a bench filler or a metamod, Eldritch, or lock + scour).
//! - Essence first: the same, starting from an essence that guarantees one of the mods.
//! - Fracture the hardest mod: make it, fracture it (Fracturing Orb picks one mod at random), then spam the rest.
//! - Eldritch: spam one side, then an Eldritch Chaos Orb rerolls only the other side (helmets, gloves, boots, body).
//! - Keep what your item has: lock the good side of the copied item and reroll the rest.
//! A step that rerolls the whole item only counts when it leaves room for what comes after (no stray mod in a slot a
//! later step needs). Rules that aren't confirmed yet are marked "check in game" on the route.
use super::*;

type Slot = (usize, f64, bool, usize);
const T: u32 = if cfg!(test) { 4_000 } else { 12_000 };

/// One whole way to make the item.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Route {
    pub id: String,
    pub name: String,
    /// One line: what the route does.
    pub blurb: String,
    pub steps: Vec<Step>,
    /// Average cost in chaos (None = a price is missing).
    pub cost: Option<f64>,
    /// About how many currency uses / crafts it takes.
    pub clicks: f64,
    /// 0 safe, 1 a step can fail and start again, 2 can lose mods you already have.
    pub risk: u8,
    /// Rules this route leans on that aren't confirmed yet.
    pub checks: Vec<String>,
    /// "Cheapest", "Fewest clicks", "Safest".
    pub badges: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Routes { pub routes: Vec<Route>, pub ilvl_needed: u32, pub note: String, pub divine: Option<f64> }

const CHECK_LOCK: &str = "The 1–2 mod bench locks are new in 3.30: check that a Harvest reforge or Orb of Scouring keeps the locked mods.";
const CHECK_ELDRITCH: &str = "Eldritch Chaos Orb: how many new mods it adds (1–3) is a guess here. One Eldritch implicit is assumed to make its side dominant.";
const CHECK_HARVEST: &str = "Harvest reforge with a lock on the item: check it keeps the locked mods.";

/// What's on the item, for the next roll: prefixes, suffixes and the groups they take.
#[derive(Clone, Default, Debug)]
struct Seed { pre: i32, suf: i32, groups: Vec<usize> }
impl Seed {
    fn add(&mut self, prefix: bool, g: Option<usize>) { if prefix { self.pre += 1 } else { self.suf += 1 } if let Some(g) = g { self.groups.push(g) } }
    fn side(&self, prefix: bool) -> i32 { if prefix { self.pre } else { self.suf } }
}

/// How a roll goes: a rare reroll (4–6 mods), magic (1–2), Harvest's "with a <tag> mod" (the first new mod has it),
/// an essence's mod, or only one side (Eldritch Chaos: 1–3 new mods there). `cap` = the most prefixes / suffixes the
/// item may end with (room left for later steps).
#[derive(Clone, Copy)]
struct Cfg<'a> { magic: bool, tag: Option<&'a str>, forced: Option<(bool, Option<usize>)>, side: Option<bool>, cap: (i32, i32) }
impl Default for Cfg<'_> { fn default() -> Self { Cfg { magic: false, tag: None, forced: None, side: None, cap: (3, 3) } } }

/// Chance one roll gives every target (each target = the pool slots that count for it) and fits in `cap`.
fn sim(pool: &[Slot], cfg: Cfg, seed: &Seed, targets: &[Vec<bool>]) -> f64 {
    let d = data();
    let mut rng = Rng(0x51A7_E5ED_C0FF_EE11 ^ ((pool.len() as u64) << 7) ^ (targets.len() as u64));
    let side_max = if cfg.magic { 1 } else { 3 };
    let tagged: Vec<bool> = pool.iter().map(|(i, ..)| cfg.tag.is_some_and(|t| d.mods[*i].tags.iter().any(|x| x == t))).collect();
    let mut used: Vec<usize> = Vec::with_capacity(8);
    let mut got = vec![false; targets.len()];
    let mut wins = 0u32;
    for _ in 0..T {
        let r = rng.next();
        let (mut pre, mut suf) = (seed.pre, seed.suf);
        used.clear();
        used.extend_from_slice(&seed.groups);
        got.iter_mut().for_each(|g| *g = false);
        if let Some((p, g)) = cfg.forced { if p { pre += 1 } else { suf += 1 } if let Some(g) = g { used.push(g) } }
        let new = match cfg.side {
            Some(_) => if r < 0.5 { 1 } else if r < 0.85 { 2 } else { 3 },
            None if cfg.magic => if r < 0.5 { 1 } else { 2 },
            None => { let n = if r < 8.0 / 12.0 { 4 } else if r < 11.0 / 12.0 { 5 } else { 6 }; (n - pre - suf).max(1) }
        };
        for step in 0..new {
            let only_tag = step == 0 && cfg.tag.is_some();
            let open = |k: usize, p: bool, g: usize, pre: i32, suf: i32, used: &[usize]| !used.contains(&g)
                && (if p { pre < side_max } else { suf < side_max }) && (!only_tag || tagged[k]) && cfg.side.is_none_or(|s| s == p);
            let total: f64 = pool.iter().enumerate().filter(|(k, (_, _, p, g))| open(*k, *p, *g, pre, suf, &used)).map(|(_, (_, w, ..))| w).sum();
            if total <= 0.0 { break }
            let mut x = rng.next() * total;
            for (k, (_, w, p, g)) in pool.iter().enumerate() {
                if !open(k, *p, *g, pre, suf, &used) { continue }
                x -= w;
                if x <= 0.0 {
                    used.push(*g);
                    if *p { pre += 1 } else { suf += 1 }
                    for (t, set) in targets.iter().enumerate() { if set[k] { got[t] = true } }
                    break;
                }
            }
        }
        if got.iter().all(|g| *g) && pre <= cfg.cap.0 && suf <= cfg.cap.1 { wins += 1 }
    }
    wins as f64 / T as f64
}

/// A mod in the plan.
struct Tgt { key: String, text: String, tier: u32, prefix: bool, group: Option<usize>, wanted: HashSet<String>, chaos: f64, tags: Vec<String>,
    natural: bool, fam: Family }

struct Ctx<'a> {
    class: String,
    tags: HashSet<String>,
    ilvl: u32,
    roll: Vec<Slot>,
    price: &'a dyn Fn(&str) -> Option<f64>,
    t: Vec<Tgt>,
    /// Fractured mods on the item: never change, can't be annulled.
    fixed: Seed,
    armour: bool,
    influenced: bool,
}

fn way(group: &str, method: impl Into<String>, chance: f64, each: Option<f64>, average: Option<f64>, note: impl Into<String>, tries: f64, risk: u8, check: Option<&str>) -> Way {
    Way { group: group.into(), method: method.into(), chance, cost_each: each, average, note: note.into(), tries, risk, check: check.map(String::from) }
}
/// A reroll: `each` per try, 1/chance tries.
fn reroll(method: impl Into<String>, chance: f64, each: Option<f64>, note: impl Into<String>, check: Option<&str>) -> Way {
    way("Reroll", method, chance, each, each.filter(|_| chance > 0.0).map(|e| e / chance), note, if chance > 0.0 { 1.0 / chance } else { 0.0 }, 0, check)
}
fn sort_ways(v: &mut [Way]) {
    v.sort_by(|a, b| a.average.unwrap_or(f64::MAX).partial_cmp(&b.average.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal)
        .then(b.chance.partial_cmp(&a.chance).unwrap_or(std::cmp::Ordering::Equal)));
}
fn plus(a: Option<f64>, b: Option<f64>) -> Option<f64> { Some(a? + b?) }
fn side_word(prefix: bool, n: usize) -> String { format!("{n} {}{}", if prefix { "prefix" } else { "suffix" }, if n == 1 { "" } else { "es" }) }

/// Harvest reforge cost in lifeforce, by tag (Maxroll's 3.24 list; unchanged in 3.30's notes).
const HARVEST_COST: [(&str, &str, f64); 11] = [("life", "Wild", 75.0), ("defences", "Primal", 50.0), ("physical", "Vivid", 50.0), ("fire", "Wild", 50.0),
    ("cold", "Vivid", 50.0), ("lightning", "Primal", 50.0), ("chaos", "Vivid", 100.0), ("attack", "Wild", 75.0), ("caster", "Primal", 75.0),
    ("speed", "Vivid", 150.0), ("critical", "Primal", 150.0)];

impl Ctx<'_> {
    fn c(&self, name: &str) -> Option<f64> { (self.price)(name).or_else(|| (name == "Chaos Orb").then_some(1.0)) }
    fn div(&self) -> Option<f64> { self.c("Divine Orb") }
    fn is_t(&self, t: usize, i: usize) -> bool { let m = &data().mods[i]; let x = &self.t[t]; format!("{}|{}", m.gen, m.r#type) == x.key && x.wanted.contains(&m.name) }
    fn masks(&self, pool: &[Slot], want: &[usize]) -> Vec<Vec<bool>> { want.iter().map(|t| pool.iter().map(|(i, ..)| self.is_t(*t, *i)).collect()).collect() }
    fn seed_with(&self, have: &[usize]) -> Seed { let mut s = self.fixed.clone(); for t in have { s.add(self.t[*t].prefix, self.t[*t].group) } s }
    fn harvest(&self, tag: &str) -> Option<f64> {
        let (_, kind, n) = HARVEST_COST.iter().find(|(t, ..)| *t == tag)?;
        Some(n * self.c(&format!("{kind} Crystallised Lifeforce"))?)
    }
    fn jewellery(&self) -> bool { matches!(self.class.as_str(), "Ring" | "Amulet" | "Belt") }
    fn weapon(&self) -> bool { !self.armour && !self.jewellery() && !matches!(self.class.as_str(), "Quiver" | "Shield") }
    /// The bench lock for `n` mods on one side (3.30): 1 Divine a mod, or the full side lock for 3.
    fn lock(&self, prefix: bool, n: usize) -> Way {
        if n >= 3 {
            let (extra, k) = if self.weapon() { ("Sacred Orb", 1.0) } else if self.jewellery() { ("Exalted Orb", 8.0) } else { ("Blessed Orb", 4.0) };
            let each = plus(self.div().map(|d| d * 4.0), self.c(extra).map(|e| e * k));
            return way("Guaranteed", format!("Bench: {}es Cannot Be Changed", if prefix { "Prefix" } else { "Suffix" }), 1.0, each, each,
                format!("Meta-craft (4 Divine + {k} {extra}). It sits in a {} slot and survives an Orb of Scouring", if prefix { "suffix" } else { "prefix" }), 1.0, 0, None);
        }
        let each = self.div().map(|d| d * n as f64);
        way("Guaranteed", format!("Bench: lock {}", side_word(prefix, n)), 1.0, each, each,
            format!("\"{n} {} Modifier{} cannot be changed\" ({n} Divine). It sits in a {} slot", if prefix { "Prefix" } else { "Suffix" }, if n == 1 { "" } else { "s" },
                if prefix { "suffix" } else { "prefix" }), 1.0, 0, Some(CHECK_LOCK))
    }
    fn bench_cost(&self, f: &Family) -> Option<f64> { f.bench.first()?.cost.iter().map(|(c, n)| self.c(c).map(|v| v * *n as f64)).sum() }
    /// The bench can make this target well enough.
    fn bench_ok(&self, t: usize) -> bool {
        let x = &self.t[t];
        x.fam.bench.first().is_some_and(|b| !x.natural
            || first_bottom(&b.text).and_then(|v| x.fam.tiers.iter().find(|y| first_bottom(&y.text).is_some_and(|z| z <= v))).is_some_and(|y| y.tier <= x.tier))
    }
    /// The essence that gives this target (good enough, cheapest).
    fn essence(&self, t: usize) -> Option<(&EssenceWay, bool, Option<usize>)> {
        let d = data();
        let x = &self.t[t];
        let ok: Vec<&EssenceWay> = x.fam.essences.iter().filter(|e| !e.corrupt && (e.like_tier.is_some_and(|l| l <= x.tier) || !x.natural)).collect();
        let e = ok.iter().filter(|e| e.price.is_some()).min_by(|a, b| a.price.partial_cmp(&b.price).unwrap_or(std::cmp::Ordering::Equal)).or(ok.first())?;
        let raw = d.essences.iter().find(|r| r.name == e.name)?.mods.get(&self.class)?;
        let m = d.mods.iter().find(|m| &m.id == raw)?;
        Some((e, m.gen == "p", Some(group_id(&m.group))))
    }
    /// The cheapest bench craft on one side (a filler to block that side from an Exalted Orb).
    fn filler(&self, prefix: bool, used: &[usize]) -> Option<(String, f64)> {
        let d = data();
        d.bench.iter().filter(|b| b.classes.contains(&self.class)).filter_map(|b| {
            let m = d.mods.iter().find(|m| m.id == b.r#mod)?;
            if (m.gen == "p") != prefix || used.contains(&group_id(&m.group)) { return None }
            let c: Option<f64> = b.cost.iter().map(|(c, n)| self.c(currency_name(c)).map(|v| v * *n as f64)).sum();
            Some((m.text.replace('\n', " / "), c?))
        }).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Ways to reroll the item until every target in `want` is on it, keeping `seed` (fractured or locked mods) and
    /// leaving it at no more than `cap` prefixes / suffixes. `locked`: a bench lock is on (no essences, fossils or
    /// Chaos: only Harvest reforges keep locked mods).
    fn spam(&self, want: &[usize], seed: &Seed, cap: (i32, i32), locked: bool, alt: bool) -> Vec<Way> {
        let d = data();
        if want.is_empty() { return vec![] }
        let mut out = Vec::new();
        let cfg = Cfg { cap, ..Default::default() };
        // A mod only an essence makes: that essence it is.
        let only_ess: Vec<usize> = want.iter().copied().filter(|t| !self.t[*t].natural).collect();
        if only_ess.len() > 1 { return vec![] }
        let all = self.masks(&self.roll, want);
        if !locked && only_ess.is_empty() {
            out.push(reroll("Chaos Orb", sim(&self.roll, cfg, seed, &all), self.c("Chaos Orb"), "Rerolls the whole item: 4–6 new mods", None));
            if alt && seed.pre + seed.suf == 0 && want.iter().filter(|t| self.t[**t].prefix).count() <= 1 && want.iter().filter(|t| !self.t[**t].prefix).count() <= 1 {
                let a = sim(&self.roll, Cfg { magic: true, ..cfg }, seed, &all);
                let regal = self.c("Regal Orb");
                let mut w = reroll("Orb of Alteration", a, self.c("Orb of Alteration"), "On a magic item (1–2 mods), then a Regal Orb to make it rare", None);
                w.average = plus(w.average, regal);
                out.push(w);
            }
        }
        // Harvest reforges for each kind of mod we want ("Reforge … including a Life modifier").
        let mut seen: Vec<&str> = Vec::new();
        for t in want { for tag in &self.t[*t].tags { if HARVEST.iter().any(|(h, _)| h == tag) && !seen.contains(&tag.as_str()) { seen.push(tag) } } }
        if only_ess.is_empty() {
            for tag in seen {
                let name = HARVEST.iter().find(|(h, _)| *h == tag).map(|(_, n)| *n).unwrap_or(tag);
                let h = sim(&self.roll, Cfg { tag: Some(tag), ..cfg }, seed, &all);
                out.push(reroll(format!("Harvest reforge: {name}"), h, self.harvest(tag),
                    format!("Horticrafting: rerolls the item with at least one {} mod{}", name.to_lowercase(), if locked { "; locked mods stay" } else { "" }),
                    locked.then_some(CHECK_HARVEST)));
            }
        }
        if locked { sort_ways(&mut out); return out }
        // Essences: one target guaranteed, the rest have to roll.
        for (n, t) in want.iter().enumerate() {
            if !only_ess.is_empty() && only_ess[0] != *t { continue }
            let Some((e, p, g)) = self.essence(*t) else { continue };
            if self.t[*t].natural && e.like_tier.is_none() { continue }
            let rest: Vec<usize> = want.iter().enumerate().filter(|(k, _)| *k != n).map(|(_, x)| *x).collect();
            let x = sim(&self.roll, Cfg { forced: Some((p, g)), ..cfg }, seed, &self.masks(&self.roll, &rest));
            out.push(reroll(e.name.clone(), x, e.price, format!("Always gives {}; the rest has to roll", e.text.replace('\n', " / ")), None));
        }
        if !only_ess.is_empty() { sort_ways(&mut out); return out }
        // Fossils that help a target and block none (singles and pairs).
        let blocked = |fo: &RawFossil| want.iter().any(|t| self.t[*t].fam.fossils_none.contains(&fo.name));
        let helpful: Vec<&RawFossil> = d.fossils.iter().filter(|fo| !blocked(fo) && want.iter().any(|t| self.t[*t].fam.fossils_more.contains(&fo.name))).collect();
        let mut combos: Vec<Vec<&RawFossil>> = helpful.iter().map(|f| vec![*f]).collect();
        for (a, f) in helpful.iter().enumerate() { for g in helpful.iter().skip(a + 1) { combos.push(vec![*f, *g]); } }
        let mut tried: Vec<Way> = combos.into_iter().take(8).map(|fs| {
            let pool = rollable(&self.tags, self.ilvl, &fs);
            let x = sim(&pool, cfg, seed, &self.masks(&pool, want));
            let res = ["Primitive Chaotic Resonator", "Potent Chaotic Resonator", "Powerful Chaotic Resonator", "Prime Chaotic Resonator"][fs.len() - 1];
            let each: Option<f64> = fs.iter().map(|f| self.c(&f.name)).chain(std::iter::once(self.c(res))).sum();
            reroll(fs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(" + "), x, each, format!("Rerolls the item with these fossils in a {res}"), None)
        }).filter(|w| w.chance > 0.0).collect();
        sort_ways(&mut tried);
        out.extend(tried.into_iter().take(3));
        sort_ways(&mut out);
        out.retain(|w| w.chance > 0.0);
        out
    }

    /// Ways to add one target to an item that has `seed` on it (`good` of them are mods you want and an annul could
    /// hit; `good_side` = how many of those are on the target's side).
    fn slam(&self, t: usize, seed: &Seed, good: usize, good_side: usize) -> Vec<Way> {
        let x = &self.t[t];
        if seed.side(x.prefix) >= 3 { return vec![] }
        let used: HashSet<usize> = seed.groups.iter().copied().collect();
        let is_t = |i: &usize| self.is_t(t, *i);
        let mut out = Vec::new();
        // Exalt; on a miss annul and hope it takes the new mod (k other mods it could take instead).
        let annul = |p: f64, s: Option<f64>, a: Option<f64>, k: usize, others: usize| -> (Option<f64>, f64) {
            let n = (k + others) as f64 + 1.0;
            let q = k as f64 / n;
            let den = p - (1.0 - p) * q;
            // A losing bet (annul takes a mod you want too often): not offered (NaN tries).
            if p <= 0.0 || den <= 0.0 { return (None, f64::NAN) }
            ((|| Some((s? + (1.0 - p) * a? * (1.0 + q)) / den))(), (1.0 + (1.0 - p) * (1.0 + q)) / den)
        };
        let (ex, an) = (self.c("Exalted Orb"), self.c("Orb of Annulment"));
        let (pre, suf) = (seed.pre, seed.suf);
        let junk = (pre + suf) as usize - good.min((pre + suf) as usize);
        let p = add_one(&self.roll, &used, pre, suf, 3, &is_t);
        let (avg, tries) = annul(p, ex, an, good, junk);
        out.push(way("Add a mod", "Exalted Orb + Orb of Annulment", p, ex, avg,
            "Exalt; if it adds something else, annul (it may take a mod you want instead)", tries, 2, None));
        // Fill the other side at the bench first, so the Exalt can only land on this side.
        if seed.side(!x.prefix) < 3 {
            if let Some((what, cost)) = self.filler(!x.prefix, &seed.groups) {
                let (fp, fs) = if x.prefix { (pre, 3) } else { (3, suf) };
                let p = add_one(&self.roll, &used, fp, fs, 3, &is_t);
                let (avg, tries) = annul(p, ex, an, good, junk + 1);
                out.push(way("Add a mod", "Bench filler + Exalted Orb", p, ex, plus(avg, Some(cost)),
                    format!("Bench-craft {what} to fill the {}es, then Exalt (annul on a miss); remove the craft after", if x.prefix { "suffix" } else { "prefix" }), tries + 1.0, 2, None));
            }
        }
        // "Cannot roll Attack / Caster Modifiers" (1 Divine, a suffix) takes those out of the Exalt's pool.
        for (tag, name) in [("attack", "Attack"), ("caster", "Caster")] {
            if self.t.iter().any(|y| y.tags.iter().any(|g| g == tag)) { continue }
            if !(if x.prefix { suf < 3 } else { suf + 1 < 3 }) { continue }
            let pool: Vec<Slot> = self.roll.iter().copied().filter(|(i, ..)| !data().mods[*i].tags.iter().any(|g| g == tag)).collect();
            let p2 = add_one(&pool, &used, pre, suf + 1, 3, &is_t);
            if p2 <= p * 1.15 { continue }
            let (avg, tries) = annul(p2, ex, an, good, junk + 1);
            out.push(way("Add a mod", format!("Bench: Cannot roll {name} Modifiers + Exalted Orb"), p2, ex, plus(avg, self.div()),
                format!("The meta-craft (1 Divine, takes a suffix) stops {} mods; Exalt, annul on a miss", name.to_lowercase()), tries + 1.0, 2, None));
        }
        // Eldritch Exalted Orb: only adds to the dominant side (armour pieces without influence).
        if self.armour && !self.influenced {
            let (fp, fs) = if x.prefix { (pre, 3) } else { (3, suf) };
            let p = add_one(&self.roll, &used, fp, fs, 3, &is_t);
            let setup = self.c(if x.prefix { "Lesser Eldritch Ember" } else { "Lesser Eldritch Ichor" });
            let side_junk = (seed.side(x.prefix) as usize).saturating_sub(good_side);
            let (avg, tries) = annul(p, self.c("Eldritch Exalted Orb"), self.c("Eldritch Orb of Annulment"), good_side, side_junk);
            out.push(way("Add a mod", "Eldritch Exalted Orb", p, self.c("Eldritch Exalted Orb"), plus(avg, setup),
                format!("{} makes {}es dominant; the Eldritch Exalt only adds there, the Eldritch Annul only removes there",
                    if x.prefix { "A Searing Exarch implicit (Lesser Eldritch Ember)" } else { "An Eater of Worlds implicit (Lesser Eldritch Ichor)" }, if x.prefix { "prefix" } else { "suffix" }),
                tries + 1.0, 2, Some(CHECK_ELDRITCH)));
        }
        // Lock the other side, Exalt, scour on a miss (only when this side has nothing you want yet).
        let other_good = good - good_side;
        if good_side == 0 && other_good > 0 && seed.side(x.prefix) + 1 < 3 {
            let lock = self.lock(!x.prefix, other_good);
            let (fp, fs) = if x.prefix { (pre + 1, suf) } else { (pre, suf + 1) };
            let p = add_one(&self.roll, &used, fp, fs, 3, &is_t);
            let sc = self.c("Orb of Scouring");
            let avg = (|| Some(lock.average? + (ex? + sc?) / p - sc?))().filter(|_| p > 0.0);
            out.push(way("Add a mod", format!("Lock the {}es + Exalted Orb + Scouring", if x.prefix { "suffix" } else { "prefix" }), p, ex, avg,
                format!("{}, then Exalt; on a miss an Orb of Scouring clears it and you try again. Nothing you want is lost", lock.method),
                if p > 0.0 { 2.0 / p } else { 0.0 }, 0, lock.check.as_deref()));
        }
        out.retain(|w| w.chance > 0.0 && !w.tries.is_nan());
        sort_ways(&mut out);
        out
    }
}

/// A route being built.
struct Build { steps: Vec<Step>, cost: Option<f64>, clicks: f64, risk: u8, checks: Vec<String>, ok: bool, crafted: bool }
impl Build {
    fn new(base: &str, ilvl: u32) -> Self {
        Build { steps: vec![Step { title: format!("Get a {base} of item level {ilvl}+"), detail: format!("The tiers you want need item level {ilvl}."), ways: vec![] }],
            cost: Some(0.0), clicks: 0.0, risk: 0, checks: vec![], ok: true, crafted: false }
    }
    fn step(&mut self, title: impl Into<String>, detail: impl Into<String>, ways: Vec<Way>) {
        let Some(b) = ways.first() else { self.ok = false; return };
        if b.chance <= 0.0 { self.ok = false }
        self.cost = plus(self.cost, b.average);
        self.clicks += if b.tries > 0.0 { b.tries } else { 1.0 };
        self.risk = self.risk.max(b.risk);
        if let Some(c) = &b.check { if !self.checks.contains(c) { self.checks.push(c.clone()) } }
        self.steps.push(Step { title: title.into(), detail: detail.into(), ways });
    }
    fn done(self, id: &str, name: &str, blurb: String) -> Option<Route> {
        self.ok.then(|| Route { id: id.into(), name: name.into(), blurb, steps: self.steps, cost: self.cost, clicks: self.clicks.round(), risk: self.risk,
            checks: self.checks, badges: vec![] })
    }
}

/// Every route worth showing for these targets, cheapest first. `have` = the copied item's lines (its fractured
/// mods are kept; "Keep what your item has" starts from its good mods).
pub fn routes(base: &str, ilvl: u32, influences: &[String], targets: &[(String, u32)], have: &[String], price: &dyn Fn(&str) -> Option<f64>) -> Option<Routes> {
    let d = data();
    let p = pool(base, ilvl, influences, have, price)?;
    let (class, tags) = tagset(base, influences)?;
    let roll = rollable(&tags, ilvl, &[]);
    let fam_group = |key: &str| d.mods.iter().find(|m| format!("{}|{}", m.gen, m.r#type) == key && m.src.is_empty()).map(|m| group_id(&m.group));
    let mut t: Vec<Tgt> = Vec::new();
    let mut needs: Vec<String> = Vec::new();
    for (k, tier) in targets {
        let Some(f) = p.families.iter().find(|f| &f.key == k) else { continue };
        if let Some(n) = f.needs.as_ref().filter(|n| !influences.contains(n)) { needs.push(format!("{} (needs {n}: a {n}'s Exalted Orb adds the influence and one of its mods)", f.text.replace('\n', " / "))); continue }
        let wanted: HashSet<String> = f.tiers.iter().filter(|x| x.tier <= *tier).map(|x| x.name.clone()).collect();
        t.push(Tgt { key: f.key.clone(), text: f.text.replace('\n', " / "), tier: *tier, prefix: f.affix == "prefix", group: fam_group(&f.key), wanted, chaos: f.chaos,
            tags: f.tags.clone(), natural: !f.tiers.is_empty() && f.tiers.iter().any(|x| x.ok && x.tier <= *tier), fam: f.clone() });
    }
    if t.len() < 2 { return (!needs.is_empty()).then(|| Routes { routes: vec![], ilvl_needed: 1,
        note: format!("Not in these routes: {}.", needs.join("; ")), divine: price("Divine Orb") }) }
    let mut fixed = Seed::default();
    for m in p.item_mods.iter().filter(|m| m.kind == "fractured") {
        let prefix = m.affix.as_deref() == Some("prefix");
        fixed.add(prefix, m.key.as_deref().and_then(|k| fam_group(k)));
    }
    let armour = matches!(class.as_str(), "Helmet" | "Gloves" | "Boots" | "Body Armour");
    let cx = Ctx { class, tags, ilvl, roll, price, t, fixed, armour, influenced: !influences.is_empty() };
    let n = cx.t.len();
    let ilvl_needed = cx.t.iter().filter_map(|x| x.fam.tiers.iter().filter(|y| y.tier <= x.tier).map(|y| y.lvl).min()).max().unwrap_or(1);
    let mut note = if needs.is_empty() { String::new() } else { format!("Not in these routes: {}. ", needs.join("; ")) };

    // Too many for one item?
    let (np, ns) = (cx.t.iter().filter(|x| x.prefix).count() as i32 + cx.fixed.pre, cx.t.iter().filter(|x| !x.prefix).count() as i32 + cx.fixed.suf);
    if np > 3 || ns > 3 {
        return Some(Routes { routes: vec![], ilvl_needed, note: "A rare holds at most 3 prefixes and 3 suffixes: take some mods out of the plan.".into(), divine: cx.div() })
    }
    // Mods only the bench makes go last (more than one needs "Can have up to 3 Crafted Modifiers"); otherwise the
    // bench finishes the hardest mod it can make well enough.
    let bench_only: Vec<usize> = (0..n).filter(|i| !cx.t[*i].natural && cx.essence(*i).is_none() && !cx.t[*i].fam.bench.is_empty()).collect();
    let impossible: Vec<usize> = (0..n).filter(|i| !cx.t[*i].natural && cx.essence(*i).is_none() && cx.t[*i].fam.bench.is_empty()).collect();
    if !impossible.is_empty() {
        note += &format!("Left out (no way found to make it here): {}.", impossible.iter().map(|i| cx.t[*i].text.clone()).collect::<Vec<_>>().join("; "));
    }
    let reserved: Vec<usize> = if !bench_only.is_empty() { bench_only.clone() }
        else { (0..n).filter(|i| cx.bench_ok(*i)).min_by(|a, b| cx.t[*a].chaos.partial_cmp(&cx.t[*b].chaos).unwrap_or(std::cmp::Ordering::Equal)).into_iter().collect() };
    let multimod = reserved.len() >= 2;
    if multimod && ns + 1 > 3 { note.push_str(" More than one bench-only mod needs \"Can have up to 3 Crafted Modifiers\", which takes a suffix: no room."); }
    // Slots kept free for the bench at the end.
    let res_room = |prefix: bool| reserved.iter().filter(|i| cx.t[**i].prefix == prefix).count() as i32 + if multimod && !prefix { 1 } else { 0 };
    let work: Vec<usize> = (0..n).filter(|i| !reserved.contains(i) && !impossible.contains(i)).collect();
    let by_hard = |v: &mut Vec<usize>| v.sort_by(|a, b| cx.t[*a].chaos.partial_cmp(&cx.t[*b].chaos).unwrap_or(std::cmp::Ordering::Equal));

    let bench_finish = |b: &mut Build| {
        if reserved.is_empty() { return }
        let pre = if b.crafted { "Remove the crafted lock / filler first (bench: Remove Crafted Mods). " } else { "" };
        if multimod {
            let w = way("Guaranteed", "Bench: Can have up to 3 Crafted Modifiers", 1.0, cx.div().map(|d| d * 2.0), cx.div().map(|d| d * 2.0),
                "Meta-craft (2 Divine, takes a suffix): lets the item hold the bench mods below", 1.0, 0, None);
            b.step("Allow more bench mods", format!("{pre}Needed for more than one bench mod."), vec![w]);
        }
        for (k, i) in reserved.iter().enumerate() {
            let f = &cx.t[*i].fam;
            let Some(bw) = f.bench.first() else { b.ok = false; return };
            let each = cx.bench_cost(f);
            let w = way("Guaranteed", "Crafting bench", 1.0, each, each, bw.cost.iter().map(|(c, n)| format!("{n} × {c}")).collect::<Vec<_>>().join(", "), 1.0, 0, None);
            b.step(format!("Finish at the bench: {}", bw.text.replace('\n', " / ")),
                format!("{}Crafted last, in the free {} slot.", if k == 0 && !multimod { pre } else { "" }, if cx.t[*i].prefix { "prefix" } else { "suffix" }), vec![w]);
        }
    };
    // Adds the rest one at a time on an item with `seed` (the good mods `good` + stray ones up to the cap).
    let finish = |b: &mut Build, seed: &mut Seed, good: &mut Vec<usize>, rest: &[usize]| {
        let mut rest = rest.to_vec();
        by_hard(&mut rest);
        for t in rest {
            let gs = good.iter().filter(|g| cx.t[**g].prefix == cx.t[t].prefix).count();
            let ways = cx.slam(t, seed, good.len(), gs);
            if ways.first().is_some_and(|w| w.method.starts_with("Bench") || w.method.starts_with("Lock")) { b.crafted = true }
            b.step(format!("Add {}", cx.t[t].text), format!("One {} at a time, onto the item as it is now.", if cx.t[t].prefix { "prefix" } else { "suffix" }), ways);
            seed.add(cx.t[t].prefix, cx.t[t].group);
            good.push(t);
        }
    };
    let cap_for = |later: &[usize]| -> (i32, i32) {
        (3 - res_room(true) - later.iter().filter(|i| cx.t[**i].prefix).count() as i32, 3 - res_room(false) - later.iter().filter(|i| !cx.t[**i].prefix).count() as i32)
    };
    let names = |v: &[usize]| v.iter().map(|i| cx.t[*i].text.clone()).collect::<Vec<_>>().join(" + ");
    let mut out: Vec<Route> = Vec::new();

    // 1. Spam it all.
    {
        let mut b = Build::new(base, ilvl_needed);
        let ways = cx.spam(&work, &cx.fixed, cap_for(&[]), false, true);
        b.step(format!("Roll {}", names(&work)), "Reroll the whole item until all of them are on it at once (with room left for the bench).", ways);
        bench_finish(&mut b);
        if let Some(r) = b.done("spam", "Spam it all", "One method, over and over, until every mod is there at once.".into()) { out.push(r) }
    }

    // 2. One side, lock it, Harvest-reforge the other side.
    for a_pre in [true, false] {
        let a: Vec<usize> = work.iter().copied().filter(|i| cx.t[*i].prefix == a_pre).collect();
        let bside: Vec<usize> = work.iter().copied().filter(|i| cx.t[*i].prefix != a_pre).collect();
        if a.is_empty() || bside.is_empty() { continue }
        if bside.iter().any(|i| !cx.t[*i].natural) { continue }
        let mut b = Build::new(base, ilvl_needed);
        let side = if a_pre { "prefixes" } else { "suffixes" };
        let other = if a_pre { "suffixes" } else { "prefixes" };
        // The lock takes one of the other side's slots, so the other side needs room for it too.
        let room_b = 3 - res_room(!a_pre) - 1;
        if room_b <= 0 { continue }
        let now: Vec<usize> = bside.iter().copied().take(room_b as usize).collect();
        let later: Vec<usize> = bside.iter().copied().skip(now.len()).collect();
        let cap_a = 3 - res_room(a_pre);
        let ways = cx.spam(&a, &cx.fixed, if a_pre { (cap_a, 3) } else { (3, cap_a) }, false, false);
        b.step(format!("Roll the {side}: {}", names(&a)), format!("Only the {side} matter here; the {other} get rerolled later."), ways);
        b.step(format!("Lock the {side}"), "Keeps them through the next rerolls.", vec![cx.lock(a_pre, a.len() + cx.fixed.side(a_pre) as usize)]);
        b.crafted = true;
        let mut seed = cx.seed_with(&a);
        if a_pre { seed.suf += 1 } else { seed.pre += 1 }
        // The lock's slot frees up when it comes off, for the next mod.
        let cap_b = (3 - res_room(!a_pre) - later.len() as i32 + 1).min(3);
        let cap = if a_pre { (cap_a, cap_b) } else { (cap_b, cap_a) };
        let ways = cx.spam(&now, &seed, cap, true, false);
        b.step(format!("Reroll the {other}: {}", names(&now)), format!("Harvest reforges keep the locked {side}. Stop when these are on with room for the rest."), ways);
        // The lock comes off before adding anything else; stray mods may sit in the free slots up to the caps.
        let mut good: Vec<usize> = a.iter().chain(now.iter()).copied().collect();
        let mut seed = cx.seed_with(&good);
        if a_pre { seed.pre = seed.pre.max(cap_a); seed.suf = seed.suf.max(cap_b - 1) } else { seed.suf = seed.suf.max(cap_a); seed.pre = seed.pre.max(cap_b - 1) }
        if !later.is_empty() { finish(&mut b, &mut seed, &mut good, &later); }
        bench_finish(&mut b);
        if let Some(r) = b.done(if a_pre { "lock-p" } else { "lock-s" }, &format!("Roll the {side}, lock them, reroll the {other}"),
            format!("Get the {side} right first, lock them at the bench, then rework only the {other}.")) { out.push(r) }
    }

    // 3. Magic first: Alteration for the hardest prefix and suffix, Regal, then the rest.
    {
        let mut w = work.clone();
        by_hard(&mut w);
        let hp = w.iter().copied().find(|i| cx.t[*i].prefix && cx.t[*i].natural);
        let hs = w.iter().copied().find(|i| !cx.t[*i].prefix && cx.t[*i].natural);
        let h: Vec<usize> = hp.into_iter().chain(hs).collect();
        if !h.is_empty() && cx.fixed.pre + cx.fixed.suf == 0 {
            let rest: Vec<usize> = work.iter().copied().filter(|i| !h.contains(i)).collect();
            let mut b = Build::new(base, ilvl_needed);
            let all = cx.masks(&cx.roll, &h);
            let pa = sim(&cx.roll, Cfg { magic: true, ..Default::default() }, &Seed::default(), &all);
            let alt = cx.c("Orb of Alteration");
            let mut ways = vec![reroll(if h.len() == 2 { "Orb of Alteration (both at once)" } else { "Orb of Alteration" }, pa, alt, "Rerolls a magic item: 1–2 mods", None)];
            if h.len() == 2 {
                // Alteration until the harder one shows alone, then Augmentation for the other.
                let one = sim(&cx.roll, Cfg { magic: true, cap: if cx.t[h[0]].prefix { (1, 0) } else { (0, 1) }, ..Default::default() }, &Seed::default(), &cx.masks(&cx.roll, &h[..1]));
                let s1 = cx.seed_with(&h[..1]);
                let pg = add_one(&cx.roll, &s1.groups.iter().copied().collect::<HashSet<usize>>(), s1.pre, s1.suf, 1, &|i: &usize| cx.is_t(h[1], *i));
                let aug = cx.c("Orb of Augmentation");
                if one > 0.0 && pg > 0.0 {
                    let avg = (|| Some((alt? / one + aug?) / pg))();
                    ways.push(way("Reroll", "Orb of Alteration + Orb of Augmentation", one * pg, alt, avg,
                        "Alteration until the harder mod shows alone, then Augmentation for the other (miss: back to Alteration)", (1.0 / one + 1.0) / pg, 1, None));
                }
            }
            sort_ways(&mut ways);
            ways.retain(|w| w.chance > 0.0);
            b.step(format!("Make it magic with {}", names(&h)), "Orb of Transmutation on a white base, then Alteration.", ways);
            // Regal: its new mod has to land where there's room.
            let s = cx.seed_with(&h);
            let room = cap_for(&rest);
            let used: HashSet<usize> = s.groups.iter().copied().collect();
            let open: Vec<&Slot> = cx.roll.iter().filter(|(_, _, p, g)| !used.contains(g) && if *p { s.pre < 3 } else { s.suf < 3 }).collect();
            let total: f64 = open.iter().map(|x| x.1).sum();
            let ok: f64 = open.iter().filter(|(_, _, p, _)| if *p { s.pre < room.0 } else { s.suf < room.1 }).map(|x| x.1).sum();
            let pr = if total > 0.0 { ok / total } else { 0.0 };
            let first = b.steps.last().and_then(|s| s.ways.first()).and_then(|w| w.average);
            let regal = cx.c("Regal Orb");
            let avg = (|| Some(regal? / pr + first? * (1.0 / pr - 1.0)))().filter(|_| pr > 0.0);
            b.step("Regal Orb", "Makes it rare with one more mod. If that mod takes a slot you still need, scour and start the magic step again.",
                vec![way("Reroll", "Regal Orb", pr, regal, avg, "Adds one random mod", 1.0 / pr.max(1e-9), 1, None)]);
            let mut seed = s.clone();
            if room.0 > seed.pre { seed.pre += 1 } else { seed.suf += 1 }
            let mut good = h.clone();
            finish(&mut b, &mut seed, &mut good, &rest);
            bench_finish(&mut b);
            if let Some(r) = b.done("magic", "Magic first: Alteration, then Regal", "Alteration (cheap) for the two hardest mods, Regal, then add the rest.".into()) { out.push(r) }
        }
    }

    // 4 + 5. Reroll for the hardest part (leaving room), then add the rest one at a time; and the same from an essence.
    {
        let mut w = work.clone();
        by_hard(&mut w);
        let mut best: Option<Route> = None;
        let mut ess: Option<Route> = None;
        for k in 1..=w.len().min(3) {
            let h: Vec<usize> = w[..k].to_vec();
            if k == w.len() { break }
            let rest: Vec<usize> = w[k..].to_vec();
            if rest.iter().any(|i| !cx.t[*i].natural) { continue }
            let cap = cap_for(&rest);
            let ways = cx.spam(&h, &cx.fixed, cap, false, false);
            for only_essence in [false, true] {
                let ways: Vec<Way> = ways.iter().filter(|x| !only_essence || x.method.contains("Essence of")).cloned().collect();
                if ways.is_empty() { continue }
                let mut b = Build::new(base, ilvl_needed);
                b.step(format!("Roll {}", names(&h)), "Reroll until they're on, with room left for the rest.", ways);
                let mut seed = cx.seed_with(&h);
                seed.pre = seed.pre.max(cap.0.min(3));
                seed.suf = seed.suf.max(cap.1.min(3));
                let mut good = h.clone();
                finish(&mut b, &mut seed, &mut good, &rest);
                bench_finish(&mut b);
                let r = if only_essence { b.done("essence", "Essence first, then add the rest", "An essence guarantees one mod; roll for the hardest, then add the rest.".into()) }
                    else { b.done("slam", "Roll the hardest, then add the rest", format!("Reroll for the {k} hardest, then add the others one at a time.")) };
                let Some(r) = r else { continue };
                let slot = if only_essence { &mut ess } else { &mut best };
                if slot.as_ref().is_none_or(|x| r.cost.unwrap_or(f64::MAX) < x.cost.unwrap_or(f64::MAX)) { *slot = Some(r) }
            }
        }
        out.extend(best);
        if let Some(e) = ess { if !out.iter().any(|r| r.steps.get(1).map(|s| &s.ways) == e.steps.get(1).map(|s| &s.ways) && r.cost == e.cost) { out.push(e) } }
    }

    // 6. Fracture the hardest mod, then spam the rest with it kept.
    {
        let mut w: Vec<usize> = work.iter().copied().filter(|i| cx.t[*i].natural).collect();
        by_hard(&mut w);
        if let Some(&f) = w.first().filter(|_| cx.fixed.pre + cx.fixed.suf == 0 && work.len() >= 2) {
            let mut b = Build::new(base, ilvl_needed);
            let ways = cx.spam(&[f], &Seed::default(), (3, 3), false, false);
            let make = ways.first().and_then(|w| w.average);
            b.step(format!("Roll {} (any 4+ mods)", cx.t[f].text), "Any rare with it and at least 4 mods will do.", ways);
            // The orb fractures one of the item's 4–6 mods at random: about 1 in 4.3.
            let pf = (8.0 / 12.0) / 4.0 + (3.0 / 12.0) / 5.0 + (1.0 / 12.0) / 6.0;
            let orb = cx.c("Fracturing Orb");
            let avg = (|| Some(orb? / pf + make? * (1.0 / pf - 1.0)))();
            b.step(format!("Fracture {}", cx.t[f].text), "If another mod gets fractured, start the step before again. (Or buy a fractured base with it: often cheaper.)",
                vec![way("Reroll", "Fracturing Orb", pf, orb, avg, "Locks one random mod for good (needs 4+ mods)", 1.0 / pf, 1, None)]);
            let rest: Vec<usize> = work.iter().copied().filter(|i| *i != f).collect();
            let seed = cx.seed_with(&[f]);
            let ways = cx.spam(&rest, &seed, cap_for(&[]), false, false);
            b.step(format!("Roll {}", names(&rest)), "The fractured mod never changes: reroll the rest with anything (Chaos, essences, fossils).", ways);
            bench_finish(&mut b);
            if let Some(r) = b.done("fracture", "Fracture the hardest mod", format!("Make {} once, fracture it, then reroll the rest freely.", cx.t[f].text)) { out.push(r) }
        }
    }

    // 7. Eldritch: spam one side, then Eldritch Chaos rerolls only the other side.
    if cx.armour && !cx.influenced {
        for b_pre in [true, false] {
            let a: Vec<usize> = work.iter().copied().filter(|i| cx.t[*i].prefix != b_pre).collect();
            let bside: Vec<usize> = work.iter().copied().filter(|i| cx.t[*i].prefix == b_pre).collect();
            if bside.is_empty() || bside.iter().any(|i| !cx.t[*i].natural) { continue }
            let mut b = Build::new(base, ilvl_needed);
            let (side, other) = if b_pre { ("prefixes", "suffixes") } else { ("suffixes", "prefixes") };
            let cap_a = if b_pre { (3, 3 - res_room(false)) } else { (3 - res_room(true), 3) };
            if !a.is_empty() {
                let ways = cx.spam(&a, &cx.fixed, cap_a, false, false);
                b.step(format!("Roll the {other}: {}", names(&a)), format!("Only the {other} matter here."), ways);
            }
            let imp = if b_pre { "Lesser Eldritch Ember" } else { "Lesser Eldritch Ichor" };
            let c = cx.c(imp);
            b.step(format!("Make the {side} dominant"), format!("Use a {imp} ({} implicit): Eldritch currency then works on the {side} only.", if b_pre { "Searing Exarch" } else { "Eater of Worlds" }),
                vec![way("Guaranteed", imp, 1.0, c, c, "Adds an Eldritch implicit", 1.0, 0, Some(CHECK_ELDRITCH))]);
            // The kept side: its mods plus strays up to its cap; the rerolled side starts empty.
            let kept = a.len() as i32;
            let mut seed = Seed { groups: a.iter().filter_map(|i| cx.t[*i].group).collect(), ..Default::default() };
            if b_pre { seed.suf = kept.max(cap_a.1) } else { seed.pre = kept.max(cap_a.0) }
            let cap = if b_pre { (3 - res_room(true), 3) } else { (3, 3 - res_room(false)) };
            let p = sim(&cx.roll, Cfg { side: Some(b_pre), cap, ..Default::default() }, &seed, &cx.masks(&cx.roll, &bside));
            b.step(format!("Reroll the {side}: {}", names(&bside)), format!("Eldritch Chaos Orb rerolls only the {side}; the {other} stay."),
                vec![reroll("Eldritch Chaos Orb", p, cx.c("Eldritch Chaos Orb"), format!("Rerolls the {side}: 1–3 new ones"), Some(CHECK_ELDRITCH))]);
            bench_finish(&mut b);
            if let Some(r) = b.done(if b_pre { "eldritch-p" } else { "eldritch-s" }, &format!("Eldritch: {other} first, then reroll the {side}"),
                format!("No lock needed: an Eldritch implicit lets you reroll the {side} alone.")) { out.push(r) }
        }
    }

    // 8. Keep what the copied item has: lock its good side, reroll the rest.
    {
        let good: Vec<usize> = (0..n).filter(|i| cx.t[*i].fam.have.is_some_and(|h| h > 0 && h <= cx.t[*i].tier)).collect();
        let missing: Vec<usize> = work.iter().copied().filter(|i| !good.contains(i)).collect();
        if !good.is_empty() && !missing.is_empty() {
            let gp = good.iter().filter(|i| cx.t[**i].prefix).count();
            let a_pre = gp * 2 >= good.len();
            let keep: Vec<usize> = good.iter().copied().filter(|i| cx.t[*i].prefix == a_pre).collect();
            if !keep.is_empty() {
                let lost: Vec<usize> = good.iter().copied().filter(|i| cx.t[*i].prefix != a_pre && !reserved.contains(i)).collect();
                let want: Vec<usize> = missing.iter().copied().chain(lost.iter().copied()).filter(|i| !keep.contains(i)).collect();
                let mut b = Build::new(base, ilvl_needed);
                b.steps[0] = Step { title: "Start from your item".into(), detail: format!("It already has {}.", names(&keep)), ways: vec![] };
                let side = if a_pre { "prefixes" } else { "suffixes" };
                b.step(format!("Lock your {side}"), "Keeps them through the rerolls.", vec![cx.lock(a_pre, keep.len() + cx.fixed.side(a_pre) as usize)]);
                b.crafted = true;
                let mut seed = cx.seed_with(&keep);
                if a_pre { seed.suf += 1 } else { seed.pre += 1 }
                let ways = cx.spam(&want, &seed, cap_for(&[]), true, false);
                b.step(format!("Reroll for {}", names(&want)), format!("Harvest reforges keep the locked {side}."), ways);
                bench_finish(&mut b);
                if let Some(r) = b.done("keep", "Keep what your item has", format!("Your item's {side} stay; the rest gets rerolled.")) { out.push(r) }
            }
        }
    }

    // Badges, cheapest first.
    out.retain(|r| r.cost.is_some() || r.steps.iter().all(|s| s.ways.first().is_none_or(|w| w.chance > 0.0)));
    out.sort_by(|a, b| a.cost.unwrap_or(f64::MAX).partial_cmp(&b.cost.unwrap_or(f64::MAX)).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen: HashSet<String> = HashSet::new();
    out.retain(|r| seen.insert(r.name.clone()));
    if let Some(r) = out.first_mut().filter(|r| r.cost.is_some()) { r.badges.push("Cheapest".into()) }
    if let Some(i) = (0..out.len()).min_by(|a, b| out[*a].clicks.partial_cmp(&out[*b].clicks).unwrap_or(std::cmp::Ordering::Equal)) { out[i].badges.push("Fewest clicks".into()) }
    if let Some(i) = (0..out.len()).filter(|i| out[*i].cost.is_some()).min_by(|a, b| (out[*a].risk, out[*a].cost.unwrap_or(f64::MAX)).partial_cmp(&(out[*b].risk, out[*b].cost.unwrap_or(f64::MAX))).unwrap_or(std::cmp::Ordering::Equal)) {
        out[i].badges.push("Safest".into())
    }
    Some(Routes { routes: out, ilvl_needed, note: note.trim().to_string(), divine: cx.div() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(n: &str) -> Option<f64> {
        Some(match n { "Chaos Orb" => 1.0, "Divine Orb" => 220.0, "Exalted Orb" => 0.8, "Orb of Annulment" => 7.0, "Orb of Scouring" => 0.8, "Fracturing Orb" => 1300.0,
            "Orb of Alteration" => 0.2, "Orb of Augmentation" => 0.2, "Regal Orb" => 0.3, "Eldritch Chaos Orb" => 36.0, "Eldritch Exalted Orb" => 2.0,
            "Eldritch Orb of Annulment" => 26.0, "Lesser Eldritch Ember" => 0.7, "Lesser Eldritch Ichor" => 0.6, "Blessed Orb" => 0.2, "Sacred Orb" => 2.0,
            x if x.contains("Lifeforce") => 0.02, x if x.contains("Resonator") => 1.0, x if x.contains("Fossil") => 2.0, x if x.contains("Essence") => 5.0, _ => 2.0 })
    }

    #[test]
    fn routes_for_boots() {
        let p = pool("Paladin Boots", 86, &[], &[], &price).unwrap();
        let find = |s: &str| p.families.iter().find(|f| f.text.contains(s) && !f.tiers.is_empty() && f.needs.is_none()).unwrap().key.clone();
        let t = vec![(find("Movement Speed"), 2), (find("to maximum Life"), 5), (find("Chaos Resistance"), 2), (find("to Fire Resistance"), 3)];
        let r = routes("Paladin Boots", 86, &[], &t, &[], &price).unwrap();
        let names: Vec<&str> = r.routes.iter().map(|r| r.name.as_str()).collect();
        assert!(r.routes.len() >= 5, "{names:?}");
        for id in ["spam", "fracture"] { assert!(r.routes.iter().any(|x| x.id == id), "{id} missing: {names:?}") }
        assert!(r.routes.iter().any(|x| x.id.starts_with("eldritch")), "{names:?}");
        assert!(r.routes.iter().any(|x| x.id.starts_with("lock")), "{names:?}");
        assert!(r.routes[0].badges.contains(&"Cheapest".to_string()));
        // Cheapest first, every route priced, and the bench finishes one mod.
        for w in r.routes.windows(2) { assert!(w[0].cost.unwrap() <= w[1].cost.unwrap()) }
        assert!(r.routes.iter().all(|x| x.steps.iter().any(|s| s.title.starts_with("Finish at the bench"))), "{:#?}", r.routes[0]);
        // Doing it in steps beats spamming all four at once.
        let spam = r.routes.iter().find(|x| x.id == "spam").unwrap().cost.unwrap();
        assert!(r.routes[0].cost.unwrap() < spam, "{names:?}");
    }

    #[test]
    fn two_mods_magic() {
        // One prefix + one suffix: Alteration on a magic item is a real option.
        let p = pool("Paladin Boots", 86, &[], &[], &price).unwrap();
        let find = |s: &str| p.families.iter().find(|f| f.text.contains(s) && !f.tiers.is_empty() && f.needs.is_none()).unwrap().key.clone();
        let t = vec![(find("Movement Speed"), 3), (find("to Cold Resistance"), 3)];
        let r = routes("Paladin Boots", 86, &[], &t, &[], &price).unwrap();
        let names: Vec<&str> = r.routes.iter().map(|r| r.name.as_str()).collect();
        assert!(r.routes.iter().any(|x| x.id == "magic"), "{names:?}");
        assert!(r.routes.iter().any(|x| x.steps.iter().any(|s| s.ways.iter().any(|w| w.method.starts_with("Orb of Alteration")))), "{names:?}");
    }

    #[test]
    fn room_is_kept() {
        // Rolling a prefix while keeping two suffix slots free is harder than without.
        let (_, tags) = tagset("Paladin Boots", &[]).unwrap();
        let roll = rollable(&tags, 86, &[]);
        let d = data();
        let life: Vec<bool> = roll.iter().map(|(i, ..)| d.mods[*i].r#type.contains("IncreasedLife")).collect();
        let free = sim(&roll, Cfg::default(), &Seed::default(), std::slice::from_ref(&life));
        let room = sim(&roll, Cfg { cap: (3, 1), ..Default::default() }, &Seed::default(), std::slice::from_ref(&life));
        assert!(room > 0.0 && room < free, "{room} vs {free}");
    }

    #[test]
    fn keeps_your_item() {
        let have = vec!["prefix|+90 to maximum Life".to_string(), "prefix|30% increased Movement Speed".to_string(), "suffix|+10% to Cold Resistance".to_string()];
        let p = pool("Paladin Boots", 86, &[], &have, &price).unwrap();
        let find = |s: &str| p.families.iter().find(|f| f.text.contains(s) && !f.tiers.is_empty() && f.needs.is_none()).unwrap().key.clone();
        let t = vec![(find("Movement Speed"), 2), (find("to maximum Life"), 5), (find("Chaos Resistance"), 2), (find("to Fire Resistance"), 3)];
        let r = routes("Paladin Boots", 86, &[], &t, &have, &price).unwrap();
        let k = r.routes.iter().find(|x| x.id == "keep").expect("keep route");
        assert!(k.steps[1].title.contains("prefixes"), "{:#?}", k.steps);
        assert!(!k.checks.is_empty());
    }
}
