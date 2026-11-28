//! Builds core/data/merc_icons_seed.json: the support gems' art, as prints for the Mercenary skill reader
//! (`filter_core::merc_screen`). Give it a folder of the art, one PNG per support named after its family
//! ("Brutality.png", "Area of Effect.png"), e.g. downloaded from the icon URLs in the app's icons_poe1.json:
//! `cargo run -p filter-core --example merc_icons -- <folder> core/data/merc_icons_seed.json`
use filter_core::merc_screen::{print_of_art, to_hex, Known};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut out = vec![];
    let mut files: Vec<_> = std::fs::read_dir(&a[1]).unwrap().filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "png").unwrap_or(false)).collect();
    files.sort();
    for p in files {
        let img = image::open(&p).unwrap().to_rgba8();
        let Some(print) = print_of_art(img.as_raw(), img.width() as usize, img.height() as usize) else { continue };
        out.push(Known { name: p.file_stem().unwrap().to_string_lossy().to_string(), print: to_hex(&print) });
    }
    std::fs::write(&a[2], serde_json::to_string_pretty(&out).unwrap()).unwrap();
    println!("{} supports", out.len());
}
