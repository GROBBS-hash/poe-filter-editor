//! Reads the item label under the mouse using Windows' built-in text
//! recognition (no extra installs). Only looks at a small strip of the
//! screen around the cursor, only when you press F2, and keeps nothing.

pub struct ScreenRead {
    pub text: String,
    /// Main text colour of that label, used to tell the item's rarity.
    pub rgb: Option<[u8; 3]>,
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
/// Picks the most common colour that stands out from the background.
fn text_colour(px: &[[u8; 3]]) -> Option<[u8; 3]> {
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
    let bg = v.first()?.0;
    let far = |c: [u8; 3]| -> i32 { (0..3).map(|i| (c[i] as i32 - bg[i] as i32).pow(2)).sum() };
    v.iter()
        .find(|(k, _)| far(*k) > 3 * 4 * 4)
        .map(|(_, (n, sum))| [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
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

    const W: i32 = 760;
    const H: i32 = 140;
    const SCALE: i32 = 2;

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

    /// Doubles the image size; the game's small label font reads better.
    fn upscale(src: &[u8]) -> Vec<u8> {
        let (w2, h2) = (W * SCALE, H * SCALE);
        let mut out = vec![0u8; (w2 * h2 * 4) as usize];
        for y in 0..h2 {
            for x in 0..w2 {
                let s = (((y / SCALE) * W + x / SCALE) * 4) as usize;
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
        let big = upscale(&raw);

        let buffer = CryptographicBuffer::CreateFromByteArray(&big)?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer, BitmapPixelFormat::Bgra8, W * SCALE, H * SCALE, BitmapAlphaMode::Ignore)?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
        let result = engine.RecognizeAsync(&bitmap)?.get()?;

        let (cx, cy) = ((mx * SCALE) as f32, (my * SCALE) as f32);
        let mut best: Option<(f32, String, Rect)> = None;
        for line in result.Lines()? {
            let mut rect: Option<Rect> = None;
            for word in line.Words()? {
                let r = word.BoundingRect()?;
                rect = Some(match rect {
                    None => r,
                    Some(a) => {
                        let (x1, y1) = (a.X.min(r.X), a.Y.min(r.Y));
                        let (x2, y2) = ((a.X + a.Width).max(r.X + r.Width), (a.Y + a.Height).max(r.Y + r.Height));
                        Rect { X: x1, Y: y1, Width: x2 - x1, Height: y2 - y1 }
                    }
                });
            }
            let Some(r) = rect else { continue };
            let d = dist_to(&r, cx, cy);
            if best.as_ref().map(|b| d < b.0).unwrap_or(true) {
                best = Some((d, line.Text()?.to_string(), r));
            }
        }
        let Some((_, text, r)) = best else {
            return Ok(ScreenRead { text: String::new(), rgb: None });
        };

        // Colour of the text inside that line, from the original pixels.
        let mut px = Vec::new();
        let (x0, y0) = ((r.X as i32 / SCALE).max(0), (r.Y as i32 / SCALE).max(0));
        let (x1, y1) = (((r.X + r.Width) as i32 / SCALE).min(W), ((r.Y + r.Height) as i32 / SCALE).min(H));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * W + x) * 4) as usize;
                px.push([raw[i + 2], raw[i + 1], raw[i]]); // BGRA -> RGB
            }
        }
        Ok(ScreenRead { text, rgb: text_colour(&px) })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn picks_text_over_background() {
        let mut px = vec![[20u8, 20, 20]; 300];
        px.extend(vec![[255u8, 200, 40]; 80]);
        let c = super::text_colour(&px).unwrap();
        assert!(c[0] > 240 && c[1] > 190 && c[2] < 60);
    }
}
