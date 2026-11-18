//! Reads the item label under the mouse using Windows' built-in text
//! recognition (no extra installs). Only looks at a small strip of the
//! screen around the cursor, only when you press F2, and keeps nothing.

pub struct ScreenRead {
    pub text: String,
    /// Text and background colour of that label, used to tell the item's rarity.
    pub colours: Option<([u8; 3], [u8; 3])>,
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

    /// Enlarges the image; the game's small label font reads better.
    fn upscale(src: &[u8], scale: i32) -> Vec<u8> {
        let (w2, h2) = (W * scale, H * scale);
        let mut out = vec![0u8; (w2 * h2 * 4) as usize];
        for y in 0..h2 {
            for x in 0..w2 {
                let s = (((y / scale) * W + x / scale) * 4) as usize;
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
        let scale = if W * 2 <= max { 2 } else { 1 };
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let (cx, cy) = ((mx * scale) as f32, (my * scale) as f32);
        let union = |a: Rect, r: Rect| {
            let (x1, y1) = (a.X.min(r.X), a.Y.min(r.Y));
            let (x2, y2) = ((a.X + a.Width).max(r.X + r.Width), (a.Y + a.Height).max(r.Y + r.Height));
            Rect { X: x1, Y: y1, Width: x2 - x1, Height: y2 - y1 }
        };
        // Every recognised line with its box.
        let recognise = |pixels: &[u8]| -> windows::core::Result<Vec<(String, Rect)>> {
            let big = upscale(pixels, scale);
            let buffer = CryptographicBuffer::CreateFromByteArray(&big)?;
            let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
                &buffer, BitmapPixelFormat::Bgra8, W * scale, H * scale, BitmapAlphaMode::Ignore)?;
            let result = engine.RecognizeAsync(&bitmap)?.get()?;
            let mut lines = Vec::new();
            for line in result.Lines()? {
                let mut rect: Option<Rect> = None;
                for word in line.Words()? {
                    let r = word.BoundingRect()?;
                    rect = Some(match rect { None => r, Some(a) => union(a, r) });
                }
                if let Some(r) = rect { lines.push((line.Text()?.to_string(), r)); }
            }
            Ok(lines)
        };
        // Some filter colours give the reader too little contrast (white text on bright orange):
        // if nothing is found near the mouse, try again with the darkest colour channel as grey,
        // then the same inverted.
        let near_mouse = |lines: &Vec<(String, Rect)>| lines.iter().any(|(_, r)| dist_to(r, cx, cy) < (220.0 * scale as f32).powi(2));
        let grey = |invert: bool| -> Vec<u8> {
            let mut out = raw.clone();
            for px in out.chunks_exact_mut(4) {
                let v = px[0].min(px[1]).min(px[2]);
                let v = if invert { 255 - v } else { v };
                px[0] = v; px[1] = v; px[2] = v;
            }
            out
        };
        let mut lines = recognise(&raw)?;
        if !near_mouse(&lines) {
            for invert in [false, true] {
                let more = recognise(&grey(invert))?;
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
            return Ok(ScreenRead { text: String::new(), colours: None });
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
        let (x0, y0) = ((r.X as i32 / scale).max(0), (r.Y as i32 / scale).max(0));
        let (x1, y1) = (((r.X + r.Width) as i32 / scale).min(W), ((r.Y + r.Height) as i32 / scale).min(H));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * W + x) * 4) as usize;
                px.push([raw[i + 2], raw[i + 1], raw[i]]); // BGRA -> RGB
            }
        }
        Ok(ScreenRead { text, colours: text_colour(&px) })
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
