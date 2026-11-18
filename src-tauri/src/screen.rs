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

#[cfg(not(windows))]
pub fn read_label_under_cursor() -> Result<ScreenRead, String> {
    Err("Reading items off the ground only works on Windows".into())
}

#[cfg(windows)]
pub fn read_label_under_cursor() -> Result<ScreenRead, String> {
    imp::read().map_err(|e| e.to_string())
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

#[cfg(windows)]
mod imp {
    use super::{text_colour, ScreenRead};
    use windows::{
        Foundation::Rect,
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

    // Wide enough for the longest labels even with the mouse at one end.
    const W: i32 = 1800;
    // Tall enough for big labels that wrap onto two lines.
    const H: i32 = 240;

    /// Grabs a W x H strip centred on the mouse. Returns BGRA pixels.
    fn capture(cx: i32, cy: i32) -> windows::core::Result<(Vec<u8>, i32, i32)> {
        unsafe {
            let (vx, vy) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (vw, vh) = (GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN));
            let x = (cx - W / 2).min(vx + vw - W).max(vx);
            let y = (cy - H / 2).min(vy + vh - H).max(vy);

            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let bmp = CreateCompatibleBitmap(screen, W, H);
            let old = SelectObject(mem, bmp.into());
            let copied = BitBlt(mem, 0, 0, W, H, Some(screen), x, y, SRCCOPY);

            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: W,
                    biHeight: -H, // top-down rows
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut buf = vec![0u8; (W * H * 4) as usize];
            GetDIBits(mem, bmp, 0, H as u32, Some(buf.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);

            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);
            copied?;
            Ok((buf, cx - x, cy - y))
        }
    }

    /// Cuts out columns x0..x0+w and enlarges them; the game's small label font reads better.
    fn upscale(src: &[u8], x0: i32, w: i32, scale: i32) -> Vec<u8> {
        let (w2, h2) = (w * scale, H * scale);
        let mut out = vec![0u8; (w2 * h2 * 4) as usize];
        for y in 0..h2 {
            for x in 0..w2 {
                let s = (((y / scale) * W + x0 + x / scale) * 4) as usize;
                let d = ((y * w2 + x) * 4) as usize;
                out[d..d + 3].copy_from_slice(&src[s..s + 3]);
                out[d + 3] = 255;
            }
        }
        out
    }

    fn dist_to(r: &Rect, px: f32, py: f32) -> f32 {
        let dx = (r.X - px).max(0.0).max(px - (r.X + r.Width));
        let dy = (r.Y - py).max(0.0).max(py - (r.Y + r.Height));
        dx * dx + dy * dy * 4.0 // being on the same row matters most
    }

    pub fn read() -> windows::core::Result<ScreenRead> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p)? };
        let (raw, mx, my) = capture(p.x, p.y)?;
        let max = OcrEngine::MaxImageDimension().unwrap_or(2600) as i32;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        // Everything below works in the captured strip's own pixels (not enlarged).
        let (cx, cy) = (mx as f32, my as f32);
        let union = |a: Rect, r: Rect| {
            let (x1, y1) = (a.X.min(r.X), a.Y.min(r.Y));
            let (x2, y2) = ((a.X + a.Width).max(r.X + r.Width), (a.Y + a.Height).max(r.Y + r.Height));
            Rect { X: x1, Y: y1, Width: x2 - x1, Height: y2 - y1 }
        };
        // Reads columns x0..x0+w at a given size; every line with its box, back in strip pixels.
        let recognise = |pixels: &[u8], x0: i32, w: i32, scale: i32| -> windows::core::Result<Vec<(String, Rect)>> {
            let big = upscale(pixels, x0, w, scale);
            let buffer = CryptographicBuffer::CreateFromByteArray(&big)?;
            let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
                &buffer, BitmapPixelFormat::Bgra8, w * scale, H * scale, BitmapAlphaMode::Ignore)?;
            let result = engine.RecognizeAsync(&bitmap)?.get()?;
            let mut lines = Vec::new();
            let sf = scale as f32;
            for line in result.Lines()? {
                let mut rect: Option<Rect> = None;
                for word in line.Words()? {
                    let r = word.BoundingRect()?;
                    let r = Rect { X: r.X / sf + x0 as f32, Y: r.Y / sf, Width: r.Width / sf, Height: r.Height / sf };
                    rect = Some(match rect { None => r, Some(a) => union(a, r) });
                }
                if let Some(r) = rect { lines.push((line.Text()?.to_string(), r)); }
            }
            Ok(lines)
        };
        let near_mouse = |lines: &Vec<(String, Rect)>| lines.iter().any(|(_, r)| dist_to(r, cx, cy) < 220.0 * 220.0);
        // The darkest colour channel as grey (optionally inverted): rescues low-contrast colours.
        let grey = |invert: bool| -> Vec<u8> {
            let mut out = raw.clone();
            for px in out.chunks_exact_mut(4) {
                let v = px[0].min(px[1]).min(px[2]);
                let v = if invert { 255 - v } else { v };
                px[0] = v; px[1] = v; px[2] = v;
            }
            out
        };
        // 1. The whole strip (enlarged if the reader allows it).
        let mut lines = recognise(&raw, 0, W, if W * 2 <= max { 2 } else { 1 })?;
        // 2. Still nothing near the mouse: a closer look around it at double size,
        //    then the same with the contrast boosted both ways.
        if !near_mouse(&lines) {
            let w = (max / 2).min(1200).min(W);
            let x0 = (mx - w / 2).clamp(0, W - w);
            let (g1, g2) = (grey(false), grey(true));
            for pixels in [&raw, &g1, &g2] {
                let more = recognise(pixels, x0, w, 2)?;
                if near_mouse(&more) { lines = more; break; }
            }
        }
        // The text reader sometimes splits one label into pieces ("KARUI CH" + "OPPER"):
        // join pieces on the same row that sit close together.
        lines.sort_by(|a, b| a.1.X.partial_cmp(&b.1.X).unwrap_or(std::cmp::Ordering::Equal));
        let mut merged: Vec<(String, Rect)> = Vec::new();
        for (t, r) in lines {
            let mid = r.Y + r.Height / 2.0;
            let near = merged.iter().rposition(|e| {
                let m = e.1;
                mid > m.Y && mid < m.Y + m.Height && r.X - (m.X + m.Width) < r.Height * 1.5
            });
            match near {
                Some(i) => {
                    merged[i].0.push(' ');
                    merged[i].0.push_str(&t);
                    merged[i].1 = union(merged[i].1, r);
                }
                None => merged.push((t, r)),
            }
        }
        let best = merged.iter().enumerate().map(|(i, (_, r))| (dist_to(r, cx, cy), i))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let Some((_, bi)) = best else {
            return Ok(ScreenRead { text: String::new(), colours: None, snapshot: Some((raw, W, H, mx, my, vec![])) });
        };
        // Big labels wrap ("BLOOD PRICE" over "REAVER HELMET"): join lines stacked right above/below,
        // lined up with this one, top to bottom.
        let r0 = merged[bi].1;
        let stacked = |r: &Rect| {
            let overlap = (r.X + r.Width).min(r0.X + r0.Width) - r.X.max(r0.X);
            let gap_below = r.Y - (r0.Y + r0.Height);
            let gap_above = r0.Y - (r.Y + r.Height);
            let close = |g: f32| g > -r0.Height * 0.3 && g < r0.Height * 0.8;
            overlap > r0.Width.min(r.Width) * 0.3 && (close(gap_below) || close(gap_above))
                && (r.Height - r0.Height).abs() < r0.Height * 0.35
        };
        let mut block: Vec<&(String, Rect)> = merged.iter().enumerate()
            .filter(|(i, (_, r))| *i == bi || stacked(r)).map(|(_, x)| x).collect();
        block.sort_by(|a, b| a.1.Y.partial_cmp(&b.1.Y).unwrap_or(std::cmp::Ordering::Equal));
        let text = block.iter().map(|x| x.0.as_str()).collect::<Vec<_>>().join(" ");
        let r = r0;

        // Colour of the text inside that line, from the original pixels.
        let mut px = Vec::new();
        let (x0, y0) = ((r.X as i32).max(0), (r.Y as i32).max(0));
        let (x1, y1) = (((r.X + r.Width) as i32).min(W), ((r.Y + r.Height) as i32).min(H));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * W + x) * 4) as usize;
                px.push([raw[i + 2], raw[i + 1], raw[i]]); // BGRA -> RGB
            }
        }
        let found: Vec<String> = merged.iter().map(|(t, r)| format!("{t}  @ {:.0},{:.0} {:.0}x{:.0}", r.X, r.Y, r.Width, r.Height)).collect();
        Ok(ScreenRead { text, colours: text_colour(&px), snapshot: Some((raw, W, H, mx, my, found)) })
    }
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
}
