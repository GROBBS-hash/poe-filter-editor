//! Reads a beaten Mercenary's skills off the screen (28 Nov 2026: Ctrl+D copies nothing from its skill list).
//!
//! The window the game shows after the duel has one row per skill: the skill's icon, its name as text, then its
//! supports as round gem icons with a Roman numeral (I, II, III: the tier) on each. So: the screen reader's text
//! finds the skill names, and each support is told apart by its picture. Every support has its own, so a picture
//! "print" (a tiny 8 x 8 colour grid of the gem) is compared with a book of known prints: the support gems' art from
//! the game (built in, `merc_icons_seed.json`), plus the ones learned in game (hover an unknown one and press Ctrl+D:
//! its tooltip names it). The numeral's upright strokes are counted for the tier.

use serde::{Deserialize, Serialize};

/// A picture: 4 bytes a pixel (BGRA from a screen grab, or RGBA).
pub struct Pic<'a> { pub px: &'a [u8], pub w: usize, pub h: usize, pub bgra: bool }

impl Pic<'_> {
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y.min(self.h - 1) * self.w + x.min(self.w - 1)) * 4;
        let p = &self.px[i..i + 3];
        if self.bgra { [p[2], p[1], p[0]] } else { [p[0], p[1], p[2]] }
    }
}

/// Gem colour, not the panel's dark brown or its beige text.
fn colourful(p: [u8; 3]) -> bool {
    let (mx, mn) = (*p.iter().max().unwrap() as i32, *p.iter().min().unwrap() as i32);
    mx - mn > 60 && mx > 90
}

/// The numeral's pale strokes.
fn pale(p: [u8; 3]) -> bool { *p.iter().min().unwrap() > 120 && *p.iter().max().unwrap() > 200 }

const GRID: usize = 8;

/// A gem's print: GRID x GRID cells of average colour (RGB).
pub type Print = Vec<u8>;

/// Cells left out of a comparison: the corners (outside the round gem) and the bottom right (the numeral).
fn masked(gx: usize, gy: usize) -> bool {
    let c = (GRID as f32 - 1.0) / 2.0;
    let d = ((gx as f32 - c).powi(2) + (gy as f32 - c).powi(2)).sqrt();
    d > c + 0.6 || (gx >= 4 && gy >= 5)
}

/// The print of the square [x, y, side, side] of a picture.
pub fn print_of(pic: &Pic, r: [usize; 3]) -> Print {
    let [x0, y0, s] = r;
    let mut out = Vec::with_capacity(GRID * GRID * 3);
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (ax, bx) = (x0 + s * gx / GRID, (x0 + s * (gx + 1) / GRID).max(x0 + s * gx / GRID + 1));
            let (ay, by) = (y0 + s * gy / GRID, (y0 + s * (gy + 1) / GRID).max(y0 + s * gy / GRID + 1));
            let mut sum = [0u32; 3];
            let mut n = 0;
            for y in ay..by { for x in ax..bx { let p = pic.at(x, y); for i in 0..3 { sum[i] += p[i] as u32; } n += 1; } }
            for v in sum { out.push((v / n.max(1)) as u8); }
        }
    }
    out
}

/// A support gem's art from the game (RGBA, see-through round the gem), as it looks in its slot: over the slot's dark
/// brown, cut to the gem.
pub fn print_of_art(rgba: &[u8], w: usize, h: usize) -> Option<Print> {
    let bg = [25.0, 22.0, 18.0];
    let mut flat = Vec::with_capacity(rgba.len());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            let a = rgba[i + 3] as f32 / 255.0;
            for c in 0..3 { flat.push((rgba[i + c] as f32 * a + bg[c] * (1.0 - a)) as u8); }
            flat.push(255);
            if rgba[i + 3] > 100 { x0 = x0.min(x); y0 = y0.min(y); x1 = x1.max(x + 1); y1 = y1.max(y + 1); }
        }
    }
    if x1 <= x0 || y1 <= y0 { return None; }
    let s = (x1 - x0).max(y1 - y0);
    let pic = Pic { px: &flat, w, h, bgra: false };
    Some(print_of(&pic, [((x0 + x1) / 2).saturating_sub(s / 2), y0, s]))
}

/// How far apart two prints are: the average difference of a colour channel (0 = the same picture). The same
/// support read twice off the screen is under ~12; from the game's art, ~15-25; another support, ~30 or more.
pub fn distance(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() { return f32::MAX; }
    let (mut s, mut n) = (0.0, 0.0);
    for gy in 0..GRID {
        for gx in 0..GRID {
            if masked(gx, gy) { continue; }
            let i = (gy * GRID + gx) * 3;
            for c in 0..3 { s += (a[i + c] as f32 - b[i + c] as f32).abs(); n += 1.0; }
        }
    }
    s / n
}

/// The tier from the numeral on the gem's bottom right: its upright strokes, counted (I, II or III).
pub fn tier(pic: &Pic, r: [usize; 3]) -> Option<u32> {
    let [x0, y0, s] = r;
    let (ya, yb) = (y0 + s * 6 / 10, (y0 + s * 108 / 100).min(pic.h));
    let (xa, xb) = (x0 + s * 4 / 10, (x0 + s * 110 / 100).min(pic.w));
    let long = (s as f32 * 0.22).ceil() as usize;
    let stroke: Vec<bool> = (xa..xb).map(|x| {
        let (mut best, mut run) = (0, 0);
        for y in ya..yb { if pale(pic.at(x, y)) { run += 1; best = best.max(run); } else { run = 0; } }
        best >= long
    }).collect();
    // Thin groups of stroke columns: a numeral's stroke is a few pixels wide, a pale patch of the gem is wider.
    let mut count = 0;
    let mut i = 0;
    while i < stroke.len() {
        if !stroke[i] { i += 1; continue; }
        let start = i;
        while i < stroke.len() && stroke[i] { i += 1; }
        if (i - start) as f32 <= s as f32 * 0.15 { count += 1; }
    }
    (1..=3).contains(&count).then_some(count as u32)
}

/// One support icon in a row.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenIcon { pub rect: [usize; 3], pub print: Print, pub tier: Option<u32> }

/// One skill row: the skill's name (as the trade site writes it) and its supports, left to right.
#[derive(Debug, Clone, PartialEq)]
pub struct SeenSkill { pub name: String, pub y: f32, pub icons: Vec<SeenIcon> }

fn norm(t: &str) -> String {
    t.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' }).collect::<String>()
        .split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The name read off the screen, as one of `names` (the screen reader misreads the odd letter).
pub fn match_name<'a>(text: &str, names: &'a [String]) -> Option<&'a String> {
    let t = norm(text);
    if t.len() < 3 { return None; }
    let (score, best) = names.iter().map(|n| (strsim::normalized_levenshtein(&t, &norm(n)), n))
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())?;
    (score >= 0.84).then_some(best)
}

/// Is the window after the duel open? (Its buttons: Take Item, Rematch, Hire.)
pub fn panel_open(lines: &[(String, [f32; 4])]) -> bool {
    let has = |w: &str| lines.iter().any(|(t, _)| norm(t).split(' ').any(|x| x == w));
    has("hire") && (has("rematch") || has("take"))
}

/// The skill rows on screen. `lines` is the screen reader's text with where each line is ([x, y, w, h], in the
/// picture's pixels); `skills` the Mercenary skills the trade site knows.
pub fn read_rows(pic: &Pic, lines: &[(String, [f32; 4])], skills: &[String]) -> Vec<SeenSkill> {
    let mut found: Vec<(String, [f32; 4])> = lines.iter()
        .filter_map(|(t, r)| match_name(t, skills).map(|n| (n.clone(), *r))).collect();
    found.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap());
    found.dedup_by(|a, b| a.0 == b.0);
    if found.is_empty() { return vec![]; }
    let text_h = { let mut h: Vec<f32> = found.iter().map(|f| f.1[3]).collect(); h.sort_by(|a, b| a.partial_cmp(b).unwrap()); h[h.len() / 2] };
    // Row height: the smallest gap between rows (a hover box can hide the rows between), else from the text size.
    let mut gaps: Vec<f32> = found.windows(2).map(|w| (w[1].1[1] + w[1].1[3] / 2.0) - (w[0].1[1] + w[0].1[3] / 2.0))
        .filter(|g| *g > text_h * 2.0).collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let row = gaps.first().copied().unwrap_or(text_h * 3.6);
    found.into_iter().map(|(name, r)| {
        let yc = r[1] + r[3] / 2.0;
        SeenSkill { name, y: yc, icons: icons_in_row(pic, r[0] + r[2] + row * 0.25, r[0] + row * 11.0, yc, row) }
    }).collect()
}

/// The support icons in one row, from x `from` to `to`, centred on `yc`.
fn icons_in_row(pic: &Pic, from: f32, to: f32, yc: f32, row: f32) -> Vec<SeenIcon> {
    let (xa, xb) = (from.max(0.0) as usize, (to as usize).min(pic.w));
    let band = (row * 0.15).max(3.0);
    let (ya, yb) = ((yc - band).max(0.0) as usize, ((yc + band) as usize).min(pic.h));
    if xa >= xb || ya >= yb { return vec![]; }
    let need = ((yb - ya) as f32 * 0.3).ceil() as usize;
    let on: Vec<bool> = (xa..xb).map(|x| (ya..yb).filter(|&y| colourful(pic.at(x, y))).count() >= need).collect();
    // Runs of gem-coloured columns, joining small breaks.
    let mut runs: Vec<(usize, usize)> = vec![];
    let mut x = 0;
    while x < on.len() {
        if !on[x] { x += 1; continue; }
        let s = x;
        while x < on.len() && on[x] { x += 1; }
        match runs.last_mut() { Some(l) if (s - l.1) as f32 <= row * 0.06 => l.1 = x, _ => runs.push((s, x)) }
    }
    runs.into_iter().filter(|(a, b)| { let w = (b - a) as f32; w >= row * 0.35 && w <= row * 0.85 }).filter_map(|(a, b)| {
        let (x0, x1) = (xa + a, xa + b);
        // How tall the gem is (its glow can make the run wider than the gem).
        let need = ((x1 - x0) / 12).max(2);
        let top = ((yc - row * 0.45).max(0.0) as usize, ((yc + row * 0.45) as usize).min(pic.h));
        let rows: Vec<usize> = (top.0..top.1).filter(|&y| (x0..x1).filter(|&x| colourful(pic.at(x, y))).count() >= need).collect();
        let (y0, y1) = (*rows.first()?, *rows.last()? + 1);
        let s = y1 - y0;
        if (s as f32) < row * 0.35 { return None; }
        let rect = [((x0 + x1) / 2).saturating_sub(s / 2), y0, s];
        Some(SeenIcon { print: print_of(pic, rect), tier: tier(pic, rect), rect })
    }).collect()
}

/// A support's family: "Greater Area of Effect (Tier 3)", "Increased Area of Effect" and "Lesser Increased Area of
/// Effect" are all "Area of Effect" (its tiers share one picture). "Gilded" supports are their own.
pub fn family(name: &str) -> String {
    let n = name.rsplit_once(" (Tier ").map(|(n, _)| n).unwrap_or(name).trim_end_matches(" Support");
    let n = n.strip_prefix("Lesser ").or_else(|| n.strip_prefix("Greater ")).unwrap_or(n);
    n.strip_prefix("Increased ").unwrap_or(n).to_string()
}

/// The trade site's name for a support of this family at this tier ("Greater Brutality", 3). `names` are its
/// Mercenary stats. With no tier read, the middle one (or whichever there is).
pub fn tier_name(fam: &str, tier: Option<u32>, names: &[String]) -> Option<(String, u32)> {
    let of: Vec<(String, u32)> = names.iter().filter_map(|n| {
        let (base, t) = n.rsplit_once(" (Tier ")?;
        let t: u32 = t.trim_end_matches(')').parse().ok()?;
        (family(base) == fam).then(|| (base.to_string(), t))
    }).collect();
    let want = tier.unwrap_or(2);
    of.iter().find(|(_, t)| *t == want).or_else(|| tier.is_none().then(|| of.iter().max_by_key(|(_, t)| *t)).flatten()).cloned()
}

/// A hover box over a support names it (its title, e.g. "Increased Area of Effect"): the family, if one is on screen.
pub fn tooltip_support(lines: &[(String, [f32; 4])], names: &[String]) -> Option<String> {
    let bases: Vec<String> = names.iter().filter_map(|n| n.rsplit_once(" (Tier ").map(|(b, _)| b.to_string())).collect();
    lines.iter().find_map(|(t, _)| match_name(t, &bases)).map(|b| family(b))
}

/// A known support picture: its family and print (hex).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Known { pub name: String, pub print: String }

pub fn to_hex(p: &[u8]) -> String { p.iter().map(|b| format!("{b:02x}")).collect() }
pub fn from_hex(s: &str) -> Print { (0..s.len() / 2).filter_map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect() }

/// The known pictures built in: the support gems' own art (made by `examples/merc_icons.rs`), then supports read off
/// screenshots of the game and named by their hover box (merc_icons_seen.json: its first, Increased Area of Effect, from
/// his 28 Nov screenshot).
pub fn seed() -> Vec<Known> {
    let mut out: Vec<Known> = serde_json::from_str(include_str!("../data/merc_icons_seed.json")).unwrap_or_default();
    out.extend(serde_json::from_str::<Vec<Known>>(include_str!("../data/merc_icons_seen.json")).unwrap_or_default());
    out
}

/// The closest known picture, if it's close enough and clearly closer than any other support.
pub fn identify<'a>(print: &[u8], book: &'a [Known]) -> Option<(&'a str, f32)> {
    let mut d: Vec<(f32, &str)> = book.iter().map(|k| (distance(print, &from_hex(&k.print)), k.name.as_str())).collect();
    d.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let (best, name) = *d.first()?;
    let next = d.iter().find(|x| x.1 != name).map(|x| x.0).unwrap_or(f32::MAX);
    (best < 26.0 && next - best >= 5.0).then_some((name, best))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain picture with a coloured disc at (cx, cy) and `strokes` numeral strokes on its bottom right.
    fn draw(w: usize, h: usize, discs: &[(usize, usize, usize, [u8; 3], usize)]) -> Vec<u8> {
        let mut px = vec![0u8; w * h * 4];
        for i in 0..w * h { px[i * 4..i * 4 + 4].copy_from_slice(&[30, 25, 20, 255]); }
        for &(cx, cy, r, c, strokes) in discs {
            for y in cy - r..cy + r { for x in cx - r..cx + r {
                if (x as i32 - cx as i32).pow(2) + (y as i32 - cy as i32).pow(2) <= (r * r) as i32 {
                    // A little pattern, so prints differ by more than one flat colour.
                    let k = if (x + y) % 7 == 0 { 40 } else { 0 };
                    px[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[c[0].saturating_sub(k), c[1], c[2]]);
                }
            } }
            for s in 0..strokes {
                let x = cx + r / 5 + s * (r / 3);
                for y in cy + r / 2..cy + r - 1 { for dx in 0..2 { px[(y * w + x + dx) * 4..(y * w + x + dx) * 4 + 3].copy_from_slice(&[230, 225, 210]); } }
            }
        }
        px
    }

    #[test]
    fn reads_a_row_of_supports_and_their_tiers() {
        let (w, h) = (900, 200);
        let px = draw(w, h, &[(400, 100, 22, [60, 90, 230], 2), (480, 100, 22, [210, 50, 40], 3), (560, 100, 22, [50, 200, 60], 1)]);
        let pic = Pic { px: &px, w, h, bgra: false };
        let lines = vec![("SUNDER OF TRARTHUS".to_string(), [60.0, 90.0, 260.0, 20.0])];
        let skills = vec!["Ground Slam".to_string(), "Sunder of Trarthus".to_string()];
        let rows = read_rows(&pic, &lines, &skills);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Sunder of Trarthus");
        let tiers: Vec<Option<u32>> = rows[0].icons.iter().map(|i| i.tier).collect();
        assert_eq!(tiers, vec![Some(2), Some(3), Some(1)]);
        // Each is told apart from the others, and found again.
        let book: Vec<Known> = ["Blue", "Red", "Green"].iter().zip(&rows[0].icons).map(|(n, i)| Known { name: n.to_string(), print: to_hex(&i.print) }).collect();
        for (n, i) in ["Blue", "Red", "Green"].iter().zip(&rows[0].icons) { assert_eq!(identify(&i.print, &book).map(|x| x.0), Some(*n)); }
    }

    #[test]
    fn families_and_trade_names() {
        assert_eq!(family("Greater Area of Effect (Tier 3)"), "Area of Effect");
        assert_eq!(family("Lesser Increased Area of Effect"), "Area of Effect");
        assert_eq!(family("Increased Area of Effect Support"), "Area of Effect");
        assert_eq!(family("Gilded Pierce"), "Gilded Pierce");
        let names: Vec<String> = ["Lesser Increased Area of Effect (Tier 1)", "Increased Area of Effect (Tier 2)", "Greater Area of Effect (Tier 3)", "Gilded Pierce (Tier 3)", "Pierce (Tier 2)"]
            .iter().map(|s| s.to_string()).collect();
        assert_eq!(tier_name("Area of Effect", Some(3), &names), Some(("Greater Area of Effect".into(), 3)));
        assert_eq!(tier_name("Area of Effect", None, &names), Some(("Increased Area of Effect".into(), 2)));
        assert_eq!(tier_name("Gilded Pierce", None, &names), Some(("Gilded Pierce".into(), 3)));
        assert_eq!(tier_name("Pierce", Some(1), &names), None);
        let tip = vec![("INCREASED AREA OF EFFECT".to_string(), [0.0; 4]), ("Supported skills have 40% increased Area of".to_string(), [0.0; 4])];
        assert_eq!(tooltip_support(&tip, &names), Some("Area of Effect".into()));
        assert!(panel_open(&[("TAKE ITEM".into(), [0.0; 4]), ("REMATCH".into(), [0.0; 4]), ("HIRE".into(), [0.0; 4])]));
        assert!(!panel_open(&[("Hired help".into(), [0.0; 4])]));
    }

    #[test]
    fn seed_book_loads() {
        let s = seed();
        assert!(s.len() > 30, "{}", s.len());
        assert!(s.iter().any(|k| k.name == "Brutality"));
        assert!(s.iter().all(|k| from_hex(&k.print).len() == GRID * GRID * 3));
    }

    /// His screenshot of a beaten Mercenary (28 Nov): `MERC_SHOT=<png/webp> cargo test -p filter-core merc_screen -- --ignored`.
    #[test]
    #[ignore]
    fn his_screenshot() {
        let Ok(path) = std::env::var("MERC_SHOT") else { return };
        let img = image::open(path).unwrap().to_rgba8();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let pic = Pic { px: img.as_raw(), w, h, bgra: false };
        // Where the screen reader puts the names in that picture (2000 x 1238).
        let lines: Vec<(String, [f32; 4])> = [("SUNDER OF TRARTHUS", 768.0, 266.0), ("GROUND SLAM", 849.0, 177.0), ("BURROW", 930.0, 104.0), ("DETERMINATION", 1011.0, 197.0)]
            .iter().map(|(t, y, wd)| (t.to_string(), [558.0, *y, *wd, 22.0])).collect();
        let skills: Vec<String> = ["Sunder of Trarthus", "Ground Slam", "Burrow", "Determination", "Leap Slam"].iter().map(|s| s.to_string()).collect();
        let rows = read_rows(&pic, &lines, &skills);
        let book = seed();
        for r in &rows {
            let got: Vec<String> = r.icons.iter().map(|i| format!("{:?} {:?}", identify(&i.print, &book), i.tier)).collect();
            println!("{}: {:?}", r.name, got);
        }
        let tiers: Vec<Vec<Option<u32>>> = rows.iter().map(|r| r.icons.iter().map(|i| i.tier).collect()).collect();
        assert_eq!(tiers, vec![vec![Some(2), Some(3), Some(3)], vec![Some(3), Some(2)], vec![Some(2)], vec![]]);
        assert_eq!(identify(&rows[0].icons[1].print, &book).map(|x| x.0), Some("Brutality"));
        assert_eq!(identify(&rows[2].icons[0].print, &book).map(|x| x.0), Some("Pulverise"));
        assert_eq!(identify(&rows[0].icons[0].print, &book).map(|x| x.0), Some("Area of Effect"));
    }
}
