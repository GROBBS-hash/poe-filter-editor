//! Reads the item label under the mouse using Windows' built-in text
//! recognition (no extra installs). Only looks at a small strip of the
//! screen around the cursor, only when you press F2. The strip is kept
//! only when reading fails (see `save_snapshot`), so it can be fixed.

pub struct ScreenRead {
    pub text: String,
    /// Text and background colour of that label, used to tell the item's rarity.
    pub colours: Option<([u8; 3], [u8; 3])>,
    /// What was looked at: BGRA pixels, width, height, mouse x/y in it, and every line read.
    pub snapshot: Option<(Vec<u8>, i32, i32, i32, i32, Vec<String>)>,
}

/// Saves a failed read as a picture (+ a note) in `dir`, keeping the newest 15.
/// Returns the picture's path.
pub fn save_snapshot(dir: &std::path::Path, read: &ScreenRead, why: &str) -> Option<std::path::PathBuf> {
    let (px, w, h, mx, my, lines) = read.snapshot.as_ref()?;
    std::fs::create_dir_all(dir).ok()?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let path = dir.join(format!("read-{stamp}.bmp"));
    // A plain 32-bit BMP (bottom-up rows), no extra libraries needed.
    let (w, h) = (*w as u32, *h as u32);
    let size = 54 + w * h * 4;
    let mut out = Vec::with_capacity(size as usize);
    out.extend_from_slice(b"BM");
    for v in [size, 0, 54, 40] { out.extend_from_slice(&v.to_le_bytes()); }
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    for v in [0u32, w * h * 4, 2835, 2835, 0, 0] { out.extend_from_slice(&v.to_le_bytes()); }
    for y in (0..h).rev() {
        let row = &px[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
        out.extend_from_slice(row);
    }
    std::fs::write(&path, out).ok()?;
    let note = format!("{why}\nMouse at {mx},{my} in the picture.\nRead: {}\nLines found:\n{}\n",
        read.text, lines.join("\n"));
    let _ = std::fs::write(path.with_extension("txt"), note);
    // Keep the newest 15.
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut files: Vec<_> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "bmp").unwrap_or(false)).collect();
        files.sort();
        while files.len() > 15 {
            let old = files.remove(0);
            let _ = std::fs::remove_file(old.with_extension("txt"));
            let _ = std::fs::remove_file(old);
        }
    }
    Some(path)
}

/// Lines of the game's hover box (item tooltip) seen on screen, top to bottom, with their text colour.
pub type TooltipLines = Vec<(String, Option<[u8; 3]>)>;

#[cfg(not(windows))]
pub fn read_ground(_accept: &dyn Fn(&str) -> bool) -> Result<(ScreenRead, TooltipLines), String> {
    Err("Reading items off the ground only works on Windows".into())
}

/// Reads the label under the mouse (trying harder until `accept` likes what it read)
/// and the hover box the game shows for it.
#[cfg(windows)]
pub fn read_ground(accept: &dyn Fn(&str) -> bool) -> Result<(ScreenRead, TooltipLines), String> {
    imp::read(accept).map_err(|e| e.to_string())
}

/// Every line of text in a w x h box around the mouse, top to bottom (the choice reader: altars, remnants, rewards).
/// The colour at each of these screen points (the map tracker's "is a menu open?" check: a tiny capture, once a second).
#[cfg(windows)]
pub fn sample_points(points: &[(i32, i32)]) -> Result<Vec<[u8; 3]>, String> { imp::sample(points).map_err(|e| e.to_string()) }
#[cfg(not(windows))]
pub fn sample_points(_points: &[(i32, i32)]) -> Result<Vec<[u8; 3]>, String> { Err("Only on Windows".into()) }

#[cfg(not(windows))]
pub fn read_lines_near_mouse(_w: i32, _h: i32) -> Result<Vec<String>, String> {
    Err("Reading the screen only works on Windows".into())
}
#[cfg(windows)]
pub fn read_lines_near_mouse(w: i32, h: i32) -> Result<Vec<String>, String> {
    imp::read_lines(w, h).map_err(|e| e.to_string())
}

#[cfg_attr(not(windows), allow(dead_code))]
/// (text colour, background colour): background is the most common colour,
/// text the most common one that clearly stands out from it.
fn text_colour(px: &[[u8; 3]]) -> Option<([u8; 3], [u8; 3])> {
    use std::collections::HashMap;
    let q = |c: [u8; 3]| [c[0] >> 4, c[1] >> 4, c[2] >> 4];
    let mut counts: HashMap<[u8; 3], (u32, [u32; 3])> = HashMap::new();
    for &c in px {
        let e = counts.entry(q(c)).or_default();
        e.0 += 1;
        for i in 0..3 { e.1[i] += c[i] as u32; }
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    let avg = |(n, sum): &(u32, [u32; 3])| [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8];
    let (bg, bg_stats) = v.first()?;
    let bg = *bg;
    let far = |c: [u8; 3]| -> i32 { (0..3).map(|i| (c[i] as i32 - bg[i] as i32).pow(2)).sum() };
    v.iter()
        .find(|(k, _)| far(*k) > 3 * 4 * 4)
        .map(|(_, stats)| (avg(stats), avg(bg_stats)))
}

// ---------- the parts that don't need Windows (so they can be tested) ----------

/// A box on the picture: x, y, width, height.
pub type Bx = [f32; 4];
/// A piece of text read off the screen, its box and (text, background) colour.
pub type Piece = (String, Bx, Option<([u8; 3], [u8; 3])>);

#[cfg_attr(not(windows), allow(dead_code))]
fn union(a: Bx, r: Bx) -> Bx {
    let (x1, y1) = (a[0].min(r[0]), a[1].min(r[1]));
    let (x2, y2) = ((a[0] + a[2]).max(r[0] + r[2]), (a[1] + a[3]).max(r[1] + r[3]));
    [x1, y1, x2 - x1, y2 - y1]
}

#[cfg_attr(not(windows), allow(dead_code))]
fn dist_to(r: &Bx, px: f32, py: f32) -> f32 {
    let dx = (r[0] - px).max(0.0).max(px - (r[0] + r[2]));
    let dy = (r[1] - py).max(0.0).max(py - (r[1] + r[3]));
    dx * dx + dy * dy * 4.0 // being on the same row matters most
}

/// Colours inside a box of a BGRA picture `w` pixels wide.
#[cfg_attr(not(windows), allow(dead_code))]
fn colour_in(raw: &[u8], w: i32, h: i32, r: Bx) -> Option<([u8; 3], [u8; 3])> {
    let (x0, y0) = ((r[0] as i32).max(0), (r[1] as i32).max(0));
    let (x1, y1) = (((r[0] + r[2]) as i32).min(w), ((r[1] + r[3]) as i32).min(h));
    let mut px = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            px.push([raw[i + 2], raw[i + 1], raw[i]]); // BGRA -> RGB
        }
    }
    text_colour(&px)
}

/// The brightest colours inside a box (the hover box's text is drawn brighter than
/// its fancy header frame): the average of the brightest tenth.
#[cfg_attr(not(windows), allow(dead_code))]
fn bright_colour(raw: &[u8], w: i32, h: i32, r: Bx) -> Option<[u8; 3]> {
    let (x0, y0) = ((r[0] as i32).max(0), (r[1] as i32).max(0));
    let (x1, y1) = (((r[0] + r[2]) as i32).min(w), ((r[1] + r[3]) as i32).min(h));
    let mut px: Vec<[u8; 3]> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            px.push([raw[i + 2], raw[i + 1], raw[i]]);
        }
    }
    if px.is_empty() { return None; }
    px.sort_by_key(|c| std::cmp::Reverse(c[0] as u32 + c[1] as u32 + c[2] as u32));
    let top = &px[..(px.len() / 10).max(1)];
    let n = top.len() as u32;
    let sum = top.iter().fold([0u32; 3], |a, c| [a[0] + c[0] as u32, a[1] + c[1] as u32, a[2] + c[2] as u32]);
    Some([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
}

#[cfg_attr(not(windows), allow(dead_code))]
fn far_apart(a: Option<([u8; 3], [u8; 3])>, b: Option<([u8; 3], [u8; 3])>) -> bool {
    match (a, b) {
        (Some((a, _)), Some((b, _))) => a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2)).sum::<i32>() > 3 * 60 * 60,
        _ => false,
    }
}

/// Two lines sit on different label boxes: their backgrounds clearly differ (a grey
/// "MAP (TIER 4)" box right under a dark "SPINY ROUND SHIELD" one), or their text does.
#[cfg_attr(not(windows), allow(dead_code))]
fn other_label(a: Option<([u8; 3], [u8; 3])>, b: Option<([u8; 3], [u8; 3])>) -> bool {
    let bg = match (a, b) {
        (Some((_, a)), Some((_, b))) => a.iter().zip(b).map(|(x, y)| (*x as i32 - y as i32).pow(2)).sum::<i32>() > 3 * 40 * 40,
        _ => false,
    };
    bg || far_apart(a, b)
}

/// Turns the reader's lines (each a list of words with boxes) into pieces of text:
/// a line is cut where there's a wide gap or the colour changes (two labels side by
/// side, "OPAL SCEPTRE" next to "TYRANT'S SEKHEM"), and pieces the reader split
/// ("KARUI CH" + "OPPER") are joined back when they sit tight together.
#[cfg_attr(not(windows), allow(dead_code))]
fn pieces(lines: Vec<Vec<(String, Bx)>>, colour: &dyn Fn(Bx) -> Option<([u8; 3], [u8; 3])>) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    for words in lines {
        let mut cur: Option<Piece> = None;
        let mut last_word: Option<(Bx, Option<([u8; 3], [u8; 3])>)> = None;
        for (t, r) in words {
            let c = colour(r);
            let split = match last_word {
                None => false,
                Some((p, pc)) => {
                    let gap = r[0] - (p[0] + p[2]);
                    let h = p[3].max(r[3]);
                    gap > h * 0.75 || (gap > h * 0.2 && far_apart(pc, c))
                }
            };
            if split { out.extend(cur.take()); }
            cur = Some(match cur {
                None => (t, r, c),
                Some((mut s, b, pc)) => { s.push(' '); s.push_str(&t); (s, union(b, r), pc.or(c)) }
            });
            last_word = Some((r, c));
        }
        out.extend(cur);
    }
    // Join pieces on the same row that sit tight together and look alike.
    out.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged: Vec<Piece> = Vec::new();
    for (t, r, c) in out {
        let mid = r[1] + r[3] / 2.0;
        let near = merged.iter().rposition(|(_, m, mc)| {
            let gap = r[0] - (m[0] + m[2]);
            let h = r[3].max(m[3]);
            // Words this tight are one label even if their colours look different (a
            // minimap icon at the end of "THROAT STABBER" tints the last word's colour):
            // two labels always have their borders between them.
            mid > m[1] && mid < m[1] + m[3] && gap < h * 0.6 && (!far_apart(*mc, c) || gap < h * 0.45)
        });
        match near {
            Some(i) => {
                let e = &mut merged[i];
                // The reader sometimes splits inside a word: no space if there's no gap.
                if r[0] - (e.1[0] + e.1[2]) > r[3] * 0.15 { e.0.push(' '); }
                e.0.push_str(&t);
                e.1 = union(e.1, r);
                e.2 = e.2.or(c);
            }
            None => merged.push((t, r, c)),
        }
    }
    merged
}

/// The piece of text nearest the mouse, but only if the mouse is actually on its
/// label (labels have some padding round the text). A neighbouring label is never
/// picked just because the one under the mouse couldn't be read.
#[cfg_attr(not(windows), allow(dead_code))]
fn nearest_under(merged: &[Piece], cx: f32, cy: f32) -> Option<usize> {
    let (_, bi) = merged.iter().enumerate().map(|(i, p)| (dist_to(&p.1, cx, cy), i))
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    let r = merged[bi].1;
    let dx = (r[0] - cx).max(0.0).max(cx - (r[0] + r[2]));
    let dy = (r[1] - cy).max(0.0).max(cy - (r[1] + r[3]));
    (dx <= r[3] * 2.5 && dy <= r[3] * 0.9).then_some(bi)
}

/// The label nearest the mouse, with lines stacked right above/below it joined
/// ("BLOOD PRICE" over "REAVER HELMET"). Returns its text, box and colours.
#[cfg_attr(not(windows), allow(dead_code))]
fn pick_label(merged: &[Piece], cx: f32, cy: f32) -> Option<Piece> {
    let bi = nearest_under(merged, cx, cy)?;
    let r0 = merged[bi].1;
    let stacked = |r: &Bx| {
        let overlap = (r[0] + r[2]).min(r0[0] + r0[2]) - r[0].max(r0[0]);
        let gap_below = r[1] - (r0[1] + r0[3]);
        let gap_above = r0[1] - (r[1] + r[3]);
        let close = |g: f32| g > -r0[3] * 0.3 && g < r0[3] * 0.8;
        overlap > r0[2].min(r[2]) * 0.3 && (close(gap_below) || close(gap_above))
            && (r[3] - r0[3]).abs() < r0[3] * 0.35
    };
    let mut block: Vec<&Piece> = merged.iter().enumerate()
        .filter(|(i, p)| *i == bi || (stacked(&p.1) && !other_label(p.2, merged[bi].2))).map(|(_, x)| x).collect();
    block.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap_or(std::cmp::Ordering::Equal));
    let text = block.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ");
    Some((text, r0, merged[bi].2))
}

/// Every piece on the mouse's row that belongs to the same label (close together,
/// same colour), joined left to right: for when the reader loses letters mid-label.
#[cfg_attr(not(windows), allow(dead_code))]
fn row_label(merged: &[Piece], cx: f32, cy: f32) -> Option<Piece> {
    let bi = nearest_under(merged, cx, cy)?;
    let r0 = merged[bi].1;
    let mid = r0[1] + r0[3] / 2.0;
    // Right next to the piece under the mouse counts even in another colour (see pieces()).
    let touching = |p: &Piece| {
        let gap = (p.1[0] - (r0[0] + r0[2])).max(r0[0] - (p.1[0] + p.1[2]));
        gap < r0[3].max(p.1[3]) * 0.45
    };
    let mut row: Vec<&Piece> = merged.iter()
        .filter(|p| mid > p.1[1] - p.1[3] * 0.3 && mid < p.1[1] + p.1[3] * 1.3 && (!far_apart(p.2, merged[bi].2) || touching(p)))
        .collect();
    row.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap_or(std::cmp::Ordering::Equal));
    // Grow outwards from the nearest piece while the gaps stay small.
    let pos = row.iter().position(|p| std::ptr::eq(*p, &merged[bi]))?;
    let (mut lo, mut hi) = (pos, pos);
    let gap_ok = |a: &Piece, b: &Piece| b.1[0] - (a.1[0] + a.1[2]) < r0[3] * 4.0;
    while lo > 0 && gap_ok(row[lo - 1], row[lo]) { lo -= 1; }
    while hi + 1 < row.len() && gap_ok(row[hi], row[hi + 1]) { hi += 1; }
    if lo == hi { return None; }
    let text = row[lo..=hi].iter().map(|p| p.0.as_str()).collect::<Vec<_>>().join(" ");
    let bx = row[lo..=hi].iter().fold(row[lo].1, |a, p| union(a, p.1));
    Some((text, bx, merged[bi].2))
}

/// The hover box's lines: everything in the column of the "Item Level" line nearest
/// the mouse, from a little below it up to the box's top, top to bottom.
#[cfg_attr(not(windows), allow(dead_code))]
fn tooltip_lines(all: &[Piece], cx: f32, cy: f32) -> TooltipLines {
    let is_il = |t: &str| filter_core::ground::is_item_level(t);
    let Some(il) = all.iter().filter(|p| is_il(&p.0))
        .min_by(|a, b| dist_to(&a.1, cx, cy).partial_cmp(&dist_to(&b.1, cx, cy)).unwrap_or(std::cmp::Ordering::Equal))
    else { return vec![] };
    let mid = il.1[0] + il.1[2] / 2.0;
    let h = il.1[3].max(8.0);
    let mut col: Vec<&Piece> = all.iter().filter(|p| {
        let pm = p.1[0] + p.1[2] / 2.0;
        (pm - mid).abs() < h * 14.0 && p.1[1] > il.1[1] - h * 30.0 && p.1[1] < il.1[1] + h * 12.0
    }).collect();
    col.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap_or(std::cmp::Ordering::Equal));
    // Only the box itself: lines packed together around "Item Level". Text further
    // up with a gap (the quest tracker's "Find the Moon Orb") isn't part of it.
    let Some(at) = col.iter().position(|p| std::ptr::eq(*p, il)) else { return vec![] };
    let gap = |a: &Piece, b: &Piece| b.1[1] - (a.1[1] + a.1[3]);
    let (mut lo, mut hi) = (at, at);
    while lo > 0 && gap(col[lo - 1], col[lo]) < h * 2.5 { lo -= 1; }
    while hi + 1 < col.len() && gap(col[hi], col[hi + 1]) < h * 2.5 { hi += 1; }
    col[lo..=hi].iter().map(|p| (p.0.clone(), p.2.map(|c| c.0))).collect()
}

/// For washed-out pictures (HDR screens): darkest channel, strong contrast curve,
/// stretched to full range, then flipped to dark text on light.
#[cfg_attr(not(windows), allow(dead_code))]
fn boost(raw: &[u8]) -> Vec<u8> { boost_with(raw, raw, 2.6) }

/// Like `boost`, but the contrast is set from `sample` (e.g. just the part near the
/// mouse, so bright labels on a dark scene get the whole range) with curve `power`.
#[cfg_attr(not(windows), allow(dead_code))]
fn boost_with(raw: &[u8], sample: &[u8], power: f32) -> Vec<u8> {
    let mut hist = [0u32; 256];
    for px in sample.chunks_exact(4) { hist[px[0].min(px[1]).min(px[2]) as usize] += 1; }
    let n: u32 = hist.iter().sum();
    let at = |frac: f32| {
        let mut acc = 0;
        for (v, c) in hist.iter().enumerate() { acc += c; if acc as f32 >= n as f32 * frac { return v as f32; } }
        255.0
    };
    let (lo, hi) = (at(0.05), at(0.99).max(at(0.05) + 1.0));
    let mut out = raw.to_vec();
    for px in out.chunks_exact_mut(4) {
        let v = px[0].min(px[1]).min(px[2]) as f32;
        let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0).powf(power);
        let g = 255 - (t * 255.0) as u8;
        px[0] = g; px[1] = g; px[2] = g;
    }
    out
}

#[cfg(windows)]
mod imp {
    use super::{boost, boost_with, bright_colour, colour_in, dist_to, grey, pick_label, pieces, row_label, tooltip_lines, Bx, ScreenRead, TooltipLines};
    use windows::{
        Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Security::Cryptography::CryptographicBuffer,
        Win32::{
            Foundation::POINT,
            Graphics::Gdi::{
                BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
                ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
            },
            UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
                SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN},
        },
    };

    // The label strip: wide enough for the longest labels even with the mouse at
    // one end, tall enough for big labels that wrap onto two lines.
    const W: i32 = 1800;
    const H: i32 = 240;

    /// A picture: BGRA pixels, width, height.
    struct Pic { px: Vec<u8>, w: i32, h: i32 }

    /// Grabs up to w x h around the mouse (kept on screen). Returns the picture and the mouse in it.
    fn capture(cx: i32, cy: i32, w: i32, h: i32) -> windows::core::Result<(Pic, i32, i32)> {
        unsafe {
            let (vx, vy) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (vw, vh) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
            let (w, h) = (w.min(vw).max(1), h.min(vh).max(1));
            let x = (cx - w / 2).min(vx + vw - w).max(vx);
            let y = (cy - h / 2).min(vy + vh - h).max(vy);

            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, w, h);
            let old = SelectObject(mem, bmp.into());
            let copied = BitBlt(mem, 0, 0, w, h, Some(screen), x, y, SRCCOPY);

            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buf = vec![0u8; (w * h * 4) as usize];
            GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);

            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            copied?;
            Ok((Pic { px: buf, w, h }, cx - x, cy - y))
        }
    }

    /// The colour at each screen point (averaged over 3x3 pixels), from one small capture around them all.
    pub fn sample(points: &[(i32, i32)]) -> windows::core::Result<Vec<[u8; 3]>> {
        if points.is_empty() { return Ok(vec![]); }
        let (x0, x1) = (points.iter().map(|p| p.0).min().unwrap() - 1, points.iter().map(|p| p.0).max().unwrap() + 1);
        let (y0, y1) = (points.iter().map(|p| p.1).min().unwrap() - 1, points.iter().map(|p| p.1).max().unwrap() + 1);
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let (cx, cy) = (x0 + w / 2, y0 + h / 2);
        let (pic, mx, my) = capture(cx, cy, w, h)?;
        let (ox, oy) = (cx - mx, cy - my);
        Ok(points.iter().map(|&(x, y)| {
            let mut sum = [0u32; 3];
            let mut n = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (px, py) = (x + dx - ox, y + dy - oy);
                    if px < 0 || py < 0 || px >= pic.w || py >= pic.h { continue; }
                    let i = ((py * pic.w + px) * 4) as usize;
                    sum[0] += pic.px[i + 2] as u32; sum[1] += pic.px[i + 1] as u32; sum[2] += pic.px[i] as u32;
                    n += 1;
                }
            }
            let n = n.max(1);
            [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
        }).collect())
    }

    /// Cuts out a rectangle of a picture.
    fn crop(p: &Pic, x0: i32, y0: i32, w: i32, h: i32) -> Pic {
        let (x0, y0) = (x0.clamp(0, (p.w - 1).max(0)), y0.clamp(0, (p.h - 1).max(0)));
        let (w, h) = (w.min(p.w - x0), h.min(p.h - y0));
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y0 + h {
            let s = ((y * p.w + x0) * 4) as usize;
            px.extend_from_slice(&p.px[s..s + (w * 4) as usize]);
        }
        Pic { px, w, h }
    }

    /// Cuts out columns x0..x0+w and enlarges them; the game's small label font reads better.
    fn upscale(p: &Pic, x0: i32, w: i32, scale: i32) -> Vec<u8> {
        let (w2, h2) = (w * scale, p.h * scale);
        let mut out = vec![0u8; (w2 * h2 * 4) as usize];
        for y in 0..h2 {
            for x in 0..w2 {
                let s = (((y / scale) * p.w + x0 + x / scale) * 4) as usize;
                let d = ((y * w2 + x) * 4) as usize;
                out[d..d + 3].copy_from_slice(&p.px[s..s + 3]);
                out[d + 3] = 255;
            }
        }
        out
    }

    /// Every line of text in a w x h box around the mouse (kept within the engine's size limit).
    pub fn read_lines(w: i32, h: i32) -> windows::core::Result<Vec<String>> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p)? };
        let max = OcrEngine::MaxImageDimension().unwrap_or(2600) as i32;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let (pic, _, _) = capture(p.x, p.y, w.min(max), h.min(max))?;
        // Small game text reads better enlarged, when it still fits the engine's limit.
        let scale = if pic.w * 2 <= max && pic.h * 2 <= max { 2 } else { 1 };
        let enlarged = upscale(&pic, 0, pic.w, scale);
        let buffer = CryptographicBuffer::CreateFromByteArray(&enlarged)?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer, BitmapPixelFormat::Bgra8, pic.w * scale, pic.h * scale, BitmapAlphaMode::Ignore)?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;
        let mut out = Vec::new();
        for line in result.Lines()? {
            let t = line.Text()?.to_string();
            if !t.trim().is_empty() { out.push(t); }
        }
        Ok(out)
    }

    pub fn read(accept: &dyn Fn(&str) -> bool) -> windows::core::Result<(ScreenRead, TooltipLines)> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p)? };
        let max = OcrEngine::MaxImageDimension().unwrap_or(2600) as i32;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        // One picture of the screen around the mouse: the hover box can be anywhere near it.
        let (big, bx, by) = capture(p.x, p.y, max.min(3840), max.min(2160))?;
        let sx = (bx - W / 2).clamp(0, (big.w - W).max(0));
        let sy = (by - H / 2).clamp(0, (big.h - H).max(0));
        let raw = crop(&big, sx, sy, W, H);
        let (mx, my) = (bx - sx, by - sy);
        let (cx, cy) = (mx as f32, my as f32);

        // Reads columns x0..x0+w of a picture at a given size: each line's words with
        // their boxes, back in the picture's own pixels.
        let recognise = |pic: &Pic, x0: i32, w: i32, scale: i32| -> windows::core::Result<Vec<Vec<(String, Bx)>>> {
            let enlarged = upscale(pic, x0, w, scale);
            let buffer = CryptographicBuffer::CreateFromByteArray(&enlarged)?;
            let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
                &buffer, BitmapPixelFormat::Bgra8, w * scale, pic.h * scale, BitmapAlphaMode::Ignore)?;
            let result = engine.RecognizeAsync(&bitmap)?.get()?;
            let sf = scale as f32;
            let mut lines = Vec::new();
            for line in result.Lines()? {
                let mut words = Vec::new();
                for word in line.Words()? {
                    let r = word.BoundingRect()?;
                    words.push((word.Text()?.to_string(), [r.X / sf + x0 as f32, r.Y / sf, r.Width / sf, r.Height / sf]));
                }
                if !words.is_empty() { lines.push(words); }
            }
            Ok(lines)
        };
        let colour_raw = |r: Bx| colour_in(&raw.px, raw.w, raw.h, r);
        let near_mouse = |ps: &Vec<super::Piece>| ps.iter().any(|q| dist_to(&q.1, cx, cy) < 220.0 * 220.0);

        // Tries one way of looking; returns the label it found near the mouse.
        let mut seen: Vec<super::Piece> = Vec::new();
        let mut first: Option<super::Piece> = None;
        let mut attempt = |pic: &Pic, x0: i32, w: i32, scale: i32| -> windows::core::Result<bool> {
            let ps = pieces(recognise(pic, x0, w, scale)?, &colour_raw);
            if !near_mouse(&ps) { return Ok(false); }
            let label = pick_label(&ps, cx, cy);
            let row = row_label(&ps, cx, cy);
            seen = ps;
            // The nearest piece, or the whole row it sits in (the reader can drop letters
            // mid-label: "KARUI" … "OPPER").
            for l in [label, row].into_iter().flatten() {
                let ok = accept(&l.0);
                if first.is_none() || ok { first = Some(l); }
                if ok { return Ok(true); }
            }
            Ok(false)
        };
        // 1. The whole strip (enlarged if the reader allows it).
        let mut done = attempt(&raw, 0, raw.w, if raw.w * 2 <= max { 2 } else { 1 })?;
        // 2. A closer look around the mouse at double size: as is, contrast boosted
        //    both ways, then the strong boost for washed-out (HDR) screens.
        if !done {
            let w = (max / 2).min(1200).min(raw.w);
            let x0 = (mx - w / 2).clamp(0, raw.w - w);
            for pic in [None, Some(grey(&raw.px, false)), Some(grey(&raw.px, true)), Some(boost(&raw.px))] {
                let alt;
                let use_pic = match pic { None => &raw, Some(px) => { alt = Pic { px, w: raw.w, h: raw.h }; &alt } };
                if attempt(use_pic, x0, w, 2)? { done = true; break; }
            }
        }
        // 3. Bigger still, strongly boosted: the small-caps label font at 3x.
        if !done {
            let w = (max / 3).min(900).min(raw.w);
            let x0 = (mx - w / 2).clamp(0, raw.w - w);
            // Contrast from the area right round the mouse (the label and a bit either side).
            let near = crop(&raw, mx - 250, 0, 500, raw.h);
            for px in [boost_with(&raw.px, &near.px, 4.0), boost(&raw.px), grey(&raw.px, false)] {
                let pic = Pic { px, w: raw.w, h: raw.h };
                if attempt(&pic, x0, w, 3)? { break; }
            }
        }

        // The hover box: read the big picture at its own size.
        let tip = {
            // Small screens have small text: enlarge when the reader allows it.
            let scale = if big.h < 1300 && big.w * 2 <= max && big.h * 2 <= max { 2 } else { 1 };
            let lines = recognise(&big, 0, big.w, scale).unwrap_or_default();
            let ps = pieces(lines, &|r: Bx| bright_colour(&big.px, big.w, big.h, r).map(|c| (c, [0, 0, 0])));
            tooltip_lines(&ps, bx as f32, by as f32)
        };

        let mut found: Vec<String> = seen.iter().map(|(t, r, _)| format!("{t}  @ {:.0},{:.0} {:.0}x{:.0}", r[0], r[1], r[2], r[3])).collect();
        if !tip.is_empty() {
            found.push("-- hover box --".into());
            found.extend(tip.iter().map(|(t, c)| format!("{t}  {c:?}")));
        }
        let (text, colours) = first.map(|(t, _, c)| (t, c)).unwrap_or_default();
        Ok((ScreenRead { text, colours, snapshot: Some((raw.px, raw.w, raw.h, mx, my, found)) }, tip))
    }
}

/// The darkest colour channel as grey (optionally inverted): rescues low-contrast colours.
#[cfg_attr(not(windows), allow(dead_code))]
fn grey(raw: &[u8], invert: bool) -> Vec<u8> {
    let mut out = raw.to_vec();
    for px in out.chunks_exact_mut(4) {
        let v = px[0].min(px[1]).min(px[2]);
        let v = if invert { 255 - v } else { v };
        px[0] = v; px[1] = v; px[2] = v;
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn picks_text_over_background() {
        let mut px = vec![[20u8, 20, 20]; 300];
        px.extend(vec![[255u8, 200, 40]; 80]);
        let (c, bg) = super::text_colour(&px).unwrap();
        assert!(bg[0] < 30);
        assert!(c[0] > 240 && c[1] > 190 && c[2] < 60);
    }

    fn w(t: &str, x: f32, wd: f32) -> (String, super::Bx) { (t.to_string(), [x, 100.0, wd, 14.0]) }

    #[test]
    fn side_by_side_labels_are_split() {
        // One line from the reader running across two labels.
        let line = vec![w("OPAL", 100.0, 40.0), w("SCEPTRE", 146.0, 70.0), w("TYRANT'S", 240.0, 80.0), w("SEKHEM", 326.0, 60.0)];
        let white = Some(([220, 220, 220], [0, 0, 0]));
        let yellow = Some(([255, 255, 119], [0, 0, 0]));
        let colour = |r: super::Bx| if r[0] < 230.0 { white } else { yellow };
        let ps = super::pieces(vec![line], &colour);
        let texts: Vec<_> = ps.iter().map(|p| p.0.as_str()).collect();
        assert_eq!(texts, ["OPAL SCEPTRE", "TYRANT'S SEKHEM"]);
        // Mouse over the sceptre: that's the one picked.
        assert_eq!(super::pick_label(&ps, 150.0, 105.0).unwrap().0, "OPAL SCEPTRE");
        // Mouse on a label the reader couldn't read (a row below): don't grab a neighbour.
        assert!(super::pick_label(&ps, 150.0, 140.0).is_none());
        // Same colour but a wide gap: still two labels.
        let ps = super::pieces(vec![vec![w("OPAL", 100.0, 40.0), w("SCEPTRE", 146.0, 70.0), w("WAND", 236.0, 50.0)]], &|_| white);
        assert_eq!(ps.len(), 2);
        // A word the reader split in two is joined back.
        let ps = super::pieces(vec![vec![w("KARUI", 100.0, 50.0), w("CH", 155.0, 20.0)], vec![w("OPPER", 176.0, 50.0)]], &|_| white);
        assert_eq!(ps[0].0, "KARUI CHOPPER");
    }

    #[test]
    fn label_right_under_another_is_not_joined() {
        // "SPINY ROUND SHIELD (TIER 4)" (yellow on dark) with "MAP (TIER 4)" (light grey on grey)
        // right under it; the mouse is on the map.
        let shield = Some(([240, 235, 150], [20, 20, 22]));
        let map = Some(([225, 225, 225], [110, 110, 110]));
        let ps = vec![("SPINY ROUNDSHIELD (TIEE4)-t".to_string(), [100.0, 100.0, 700.0, 36.0], shield),
            ("MAP (TIER 4)".to_string(), [320.0, 145.0, 280.0, 36.0], map)];
        assert_eq!(super::pick_label(&ps, 450.0, 165.0).unwrap().0, "MAP (TIER 4)");
        assert_eq!(super::pick_label(&ps, 450.0, 118.0).unwrap().0, "SPINY ROUNDSHIELD (TIEE4)-t");
        // A real two-line label (same box) is still joined.
        let ps = vec![("BLOOD PRICE".to_string(), [300.0, 100.0, 200.0, 30.0], shield),
            ("REAVER HELMET".to_string(), [290.0, 135.0, 220.0, 30.0], shield)];
        assert_eq!(super::pick_label(&ps, 400.0, 150.0).unwrap().0, "BLOOD PRICE REAVER HELMET");
    }

    #[test]
    fn icon_tinted_word_still_joins() {
        // "THROAT STABBER" with a yellow minimap icon after it: the reader saw two lines,
        // and the second word's colour came out yellow-ish.
        let white = Some(([230, 230, 230], [90, 90, 50]));
        let yellow = Some(([250, 230, 40], [90, 90, 50]));
        let lines = vec![vec![("THRdAT".to_string(), [729.0, 90.0, 129.0, 39.0])], vec![("STABBE}Ü".to_string(), [871.0, 98.0, 146.0, 30.0])]];
        let ps = super::pieces(lines, &|r: super::Bx| if r[0] < 860.0 { white } else { yellow });
        assert_eq!(ps.len(), 1);
        assert_eq!(super::pick_label(&ps, 900.0, 120.0).unwrap().0, "THRdAT STABBE}Ü");
        // Already split into pieces: the row still joins them.
        let ps = vec![("THRdAT".to_string(), [729.0, 90.0, 129.0, 39.0], white), ("STABBE}Ü".to_string(), [871.0, 98.0, 146.0, 30.0], yellow)];
        assert_eq!(super::row_label(&ps, 900.0, 120.0).unwrap().0, "THRdAT STABBE}Ü");
    }

    #[test]
    fn whole_row_when_letters_go_missing() {
        let ps = vec![("KARUI".to_string(), [732.0, 112.0, 112.0, 33.0], None), ("OPPER-".to_string(), [925.0, 119.0, 149.0, 25.0], None),
            ("PORTAL".to_string(), [1500.0, 110.0, 100.0, 30.0], None)];
        assert_eq!(super::row_label(&ps, 900.0, 120.0).unwrap().0, "KARUI OPPER-");
    }

    #[test]
    fn hover_box_column() {
        let p = |t: &str, x: f32, y: f32| (t.to_string(), [x, y, 120.0, 16.0], None);
        let all = vec![p("TYRANT'S SEKHEM", 900.0, 300.0), p("Sceptre", 920.0, 320.0), p("Item Level: 70", 910.0, 345.0),
            p("Unidentified", 915.0, 380.0), p("PORTAL", 100.0, 350.0)];
        let mut all = all;
        // The quest tracker above, with a gap: not part of the box.
        all.push(p("Find the Moon Orb", 900.0, 120.0));
        let lines = super::tooltip_lines(&all, 500.0, 500.0);
        let t: Vec<_> = lines.iter().map(|l| l.0.as_str()).collect();
        assert_eq!(t, ["TYRANT'S SEKHEM", "Sceptre", "Item Level: 70", "Unidentified"]);
    }

    #[test]
    fn bright_text_over_frame() {
        // 90% brown frame, 10% pale-yellow text.
        let mut raw = Vec::new();
        for i in 0..100 { let c = if i % 10 == 0 { [119u8, 255, 255] } else { [58, 88, 119] }; raw.extend_from_slice(&[c[0], c[1], c[2], 255]); }
        assert_eq!(super::bright_colour(&raw, 100, 1, [0.0, 0.0, 100.0, 1.0]), Some([255, 255, 119]));
    }

    #[test]
    fn boost_makes_washed_text_dark() {
        // Washed-out background (110) with pale text (235): text should end up much darker.
        let mut raw = Vec::new();
        for i in 0..100 { let v = if i % 10 == 0 { 235 } else { 110 }; raw.extend_from_slice(&[v, v, v, 255]); }
        let b = super::boost(&raw);
        assert!(b[0] < 40 && b[4] > 200);
    }
}
