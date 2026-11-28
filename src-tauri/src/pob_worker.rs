//! Real Path of Building numbers (28 Nov 2026, GROBBS: "real Path of Building numbers"; try-out in tools/pob-tryout).
//!
//! The user's own installed Path of Building does the maths. The editor starts itself again with `--pob-worker`: that
//! process loads PoB's lua51.dll (so PoB's own C modules work), sets the working folder to the PoB install and runs
//! `pob/worker.lua` (PoB's HeadlessWrapper plus our answers loop). Requests go in as one JSON line each, answers come
//! back as lines starting "@@POB@@". A separate process so a PoB crash or hang can't take the editor down; it's
//! started on first use (about 2 s), kept, and started again if it died.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

const FILES: [(&str, &str); 4] = [
    ("HeadlessWrapper.lua", include_str!("../pob/HeadlessWrapper.lua")),
    ("_SimpleGraphic.def.lua", include_str!("../pob/_SimpleGraphic.def.lua")),
    ("inflate.lua", include_str!("../pob/inflate.lua")),
    ("worker.lua", include_str!("../pob/worker.lua")),
];

/// PoB's numbers for one setup ("Life", "EnergyShield", "CombinedDPS"...).
pub type Numbers = HashMap<String, f64>;

/// Where Path of Building Community (PoE 1) is installed: its uninstall entry, else the usual folders.
pub fn find_pob() -> Option<PathBuf> {
    let ok = |p: &Path| p.join("lua51.dll").exists() && p.join("Launch.lua").exists();
    #[cfg(windows)]
    for root in ["HKCU", "HKLM", r"HKLM\Software\WOW6432Node"] {
        let key = if root.starts_with("HKLM\\") { format!(r"{root}\Microsoft\Windows\CurrentVersion\Uninstall\Path of Building Community") }
            else { format!(r"{root}\Software\Microsoft\Windows\CurrentVersion\Uninstall\Path of Building Community") };
        let mut c = Command::new("reg");
        c.args(["query", &key, "/v", "InstallLocation"]);
        std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // CREATE_NO_WINDOW
        if let Ok(o) = c.output() {
            let text = String::from_utf8_lossy(&o.stdout);
            if let Some(v) = text.lines().find(|l| l.contains("InstallLocation")).and_then(|l| l.split("REG_SZ").nth(1)) {
                let p = PathBuf::from(v.trim().trim_matches('"'));
                if ok(&p) { return Some(p); }
            }
        }
    }
    let home = dirs::home_dir().unwrap_or_default();
    [dirs::data_dir().map(|d| d.join("Path of Building Community")), dirs::data_local_dir().map(|d| d.join(r"Programs\Path of Building Community")),
        Some(PathBuf::from(r"C:\ProgramData\Path of Building Community")), Some(PathBuf::from(r"C:\Program Files\Path of Building Community")),
        Some(home.join("Path of Building Community"))]
        .into_iter().flatten().find(|p| ok(p))
}

// ---------- the worker process (the editor started with --pob-worker <pob folder> <scripts folder>) ----------

/// Runs instead of the editor when started with `--pob-worker`. Returns true when it did.
pub fn worker_main() -> bool {
    let args: Vec<String> = std::env::args().collect();
    let Some(i) = args.iter().position(|a| a == "--pob-worker") else { return false };
    let (Some(pob), Some(scripts)) = (args.get(i + 1), args.get(i + 2)) else { return true };
    if let Err(e) = run_lua(Path::new(pob), Path::new(scripts)) {
        println!("@@POB@@{}", json!({ "error": e }));
    }
    true
}

#[cfg(windows)]
fn run_lua(pob: &Path, scripts: &Path) -> Result<(), String> {
    use libloading::{Library, Symbol};
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    type State = *mut c_void;
    std::env::set_current_dir(pob).map_err(|e| format!("Path of Building folder: {e}"))?;
    unsafe {
        let lib = Library::new(pob.join("lua51.dll")).map_err(|e| format!("Couldn't load Path of Building's lua51.dll: {e}"))?;
        let newstate: Symbol<unsafe extern "C" fn() -> State> = lib.get(b"luaL_newstate").map_err(|e| e.to_string())?;
        let openlibs: Symbol<unsafe extern "C" fn(State)> = lib.get(b"luaL_openlibs").map_err(|e| e.to_string())?;
        let loadstring: Symbol<unsafe extern "C" fn(State, *const c_char) -> c_int> = lib.get(b"luaL_loadstring").map_err(|e| e.to_string())?;
        let pcall: Symbol<unsafe extern "C" fn(State, c_int, c_int, c_int) -> c_int> = lib.get(b"lua_pcall").map_err(|e| e.to_string())?;
        let tolstring: Symbol<unsafe extern "C" fn(State, c_int, *mut usize) -> *const c_char> = lib.get(b"lua_tolstring").map_err(|e| e.to_string())?;
        let l = newstate();
        if l.is_null() { return Err("Lua didn't start".into()); }
        openlibs(l);
        // Forward slashes and a long-bracket string: any folder name works.
        let dir = scripts.to_string_lossy().replace('\\', "/");
        let chunk = CString::new(format!("WORKER_DIR = [==[{dir}]==]\ndofile(WORKER_DIR .. \"/worker.lua\")")).map_err(|e| e.to_string())?;
        let mut r = loadstring(l, chunk.as_ptr());
        if r == 0 { r = pcall(l, 0, 0, 0); }
        if r != 0 {
            let p = tolstring(l, -1, std::ptr::null_mut());
            return Err(if p.is_null() { "Path of Building stopped".into() } else { CStr::from_ptr(p).to_string_lossy().into_owned() });
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_lua(_: &Path, _: &Path) -> Result<(), String> { Err("Path of Building numbers need Windows".into()) }

// ---------- the editor's side ----------

struct Worker {
    child: Child,
    stdin: ChildStdin,
    answers: Receiver<Value>,
    /// The build and stage loaded now (so the same search doesn't load it again).
    loaded: Option<(u64, String)>,
}

static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn start(data_dir: &Path) -> Result<Worker, String> {
    let pob = find_pob().ok_or("Path of Building Community isn't installed (pathofbuilding.community)")?;
    let scripts = data_dir.join("pob_worker");
    std::fs::create_dir_all(&scripts).map_err(|e| e.to_string())?;
    for (name, text) in FILES { std::fs::write(scripts.join(name), text).map_err(|e| e.to_string())?; }
    // The editor itself (tests point POB_WORKER_EXE at a built editor: their own exe is the test runner).
    let exe = match std::env::var_os("POB_WORKER_EXE") { Some(p) => PathBuf::from(p), None => std::env::current_exe().map_err(|e| e.to_string())? };
    let mut c = Command::new(exe);
    c.arg("--pob-worker").arg(&pob).arg(&scripts).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut c, 0x0800_0000); // CREATE_NO_WINDOW
    let mut child = c.spawn().map_err(|e| format!("Couldn't start Path of Building: {e}"))?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(j) = line.strip_prefix("@@POB@@") {
                if tx.send(serde_json::from_str(j).unwrap_or(Value::Null)).is_err() { break; }
            }
        }
    });
    let mut w = Worker { child, stdin, answers: rx, loaded: None };
    let first = w.answers.recv_timeout(Duration::from_secs(60)).map_err(|_| "Path of Building didn't start in time".to_string())?;
    if let Some(e) = first["error"].as_str() { let _ = w.child.kill(); return Err(e.to_string()); }
    Ok(w)
}

fn ask(w: &mut Worker, req: &Value, wait: Duration) -> Result<Value, String> {
    writeln!(w.stdin, "{req}").and_then(|_| w.stdin.flush()).map_err(|_| "Path of Building stopped".to_string())?;
    let v = w.answers.recv_timeout(wait).map_err(|_| "Path of Building didn't answer".to_string())?;
    if let Some(e) = v["error"].as_str() { return Err(e.to_string()); }
    Ok(v)
}

fn numbers(v: &Value) -> Numbers {
    v.as_object().map(|o| o.iter().filter_map(|(k, x)| Some((k.clone(), x.as_f64()?))).collect()).unwrap_or_default()
}

/// What each item would do in `slot` of the build's stage: (the stage as it is, each item's numbers or why not).
/// `xml` is the build (PoB code decoded); `stage` the item set's name.
pub fn compare(data_dir: &Path, xml: &str, stage: &str, slot: &str, items: &[String]) -> Result<(Numbers, Vec<Result<Numbers, String>>), String> {
    let mut guard = WORKER.lock().unwrap_or_else(|e| e.into_inner());
    // Started again if it died since last time.
    if guard.as_mut().is_some_and(|w| w.child.try_wait().ok().flatten().is_some()) { *guard = None; }
    if guard.is_none() { *guard = Some(start(data_dir)?); }
    let key = (hash(xml), stage.to_string());
    let result = (|| {
        let w = guard.as_mut().unwrap();
        if w.loaded.as_ref() != Some(&key) {
            ask(w, &json!({ "cmd": "load", "xml": xml, "stage": stage }), Duration::from_secs(60))?;
            w.loaded = Some(key.clone());
        }
        let v = ask(w, &json!({ "cmd": "compare", "slot": slot, "items": items }), Duration::from_secs(60))?;
        let list = v["items"].as_array().cloned().unwrap_or_default();
        let each = (0..items.len()).map(|i| {
            let x = list.get(i).cloned().unwrap_or(Value::Null);
            match x["error"].as_str() { Some(e) => Err(e.to_string()), None => Ok(numbers(&x)) }
        }).collect();
        Ok((numbers(&v["base"]), each))
    })();
    // A worker that stopped answering is thrown away; the next search starts a fresh one.
    if result.is_err() { if let Some(mut w) = guard.take() { let _ = w.child.kill(); } }
    result
}

/// A trade listing written the way Path of Building reads items (its own "Implicits: N" layout).
pub fn listing_text(item: &Value) -> Option<String> {
    let rarity = match item["frameType"].as_i64()? { 0 => "NORMAL", 1 => "MAGIC", 2 => "RARE", 3 => "UNIQUE", _ => return None };
    let mut out = vec![format!("Rarity: {rarity}")];
    if let Some(n) = item["name"].as_str().filter(|n| !n.is_empty()) { out.push(n.to_string()); }
    out.push(item["typeLine"].as_str()?.to_string());
    if let Some(l) = item["ilvl"].as_i64() { out.push(format!("Item Level: {l}")); }
    for p in item["properties"].as_array().into_iter().flatten() {
        if p["name"].as_str() == Some("Quality") {
            if let Some(q) = p["values"][0][0].as_str() { out.push(format!("Quality: {}", q.trim_matches(|c: char| !c.is_ascii_digit()))); }
        }
    }
    // Each mod is text, or (newer listings) a record with its text in "description"; one mod can be two lines.
    let list = |k: &str| item[k].as_array().cloned().unwrap_or_default().into_iter()
        .filter_map(|m| m.as_str().map(String::from).or_else(|| m["description"].as_str().map(String::from)))
        .flat_map(|t| t.lines().map(String::from).collect::<Vec<_>>()).collect::<Vec<_>>();
    let (enchants, implicits) = (list("enchantMods"), list("implicitMods"));
    out.push(format!("Implicits: {}", enchants.len() + implicits.len()));
    out.extend(enchants.iter().map(|m| format!("{{enchant}}{m}")));
    out.extend(implicits);
    out.extend(list("fracturedMods").iter().map(|m| format!("{{fractured}}{m}")));
    out.extend(list("explicitMods"));
    out.extend(list("craftedMods").iter().map(|m| format!("{{crafted}}{m}")));
    if item["corrupted"].as_bool() == Some(true) { out.push("Corrupted".into()); }
    // Mod text can hold "[Tag|Shown text]" links; PoB wants the shown text.
    Some(out.into_iter().map(|l| plain(&l)).collect::<Vec<_>>().join("\n"))
}

fn plain(l: &str) -> String {
    let mut out = String::new();
    let mut rest = l;
    while let Some(i) = rest.find('[') {
        let Some(j) = rest[i..].find(']').map(|j| i + j) else { break };
        out.push_str(&rest[..i]);
        let inner = &rest[i + 1..j];
        out.push_str(inner.rsplit('|').next().unwrap_or(inner));
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listings_in_pobs_layout() {
        let item = json!({ "frameType": 2, "name": "Doom Visage", "typeLine": "Deicide Mask", "ilvl": 84,
            "properties": [{ "name": "Quality", "values": [["+20%", 1]] }],
            "implicitMods": [], "explicitMods": ["+100 to maximum Life", { "description": "+40% to [Resistances|Fire Resistance]", "domain": "explicit" }],
            "craftedMods": ["+20% to Chaos Resistance"], "corrupted": true });
        assert_eq!(listing_text(&item).unwrap(), "Rarity: RARE\nDoom Visage\nDeicide Mask\nItem Level: 84\nQuality: 20\nImplicits: 0\n+100 to maximum Life\n+40% to Fire Resistance\n{crafted}+20% to Chaos Resistance\nCorrupted");
    }

    /// Needs Path of Building installed: `cargo test -p poe-filter-editor pob_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn pob_live() {
        let code = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../Ruetoo Luminary aurabot (PoB code).txt")).unwrap();
        let xml = filter_core::pob::decode(code.trim()).unwrap();
        let dir = std::env::temp_dir().join("pob-live-test");
        let item = "Rarity: RARE\nTest Visage\nDeicide Mask\nQuality: 20\nItem Level: 84\nImplicits: 0\n+100 to maximum Life\n+20% to Chaos Resistance\n+40% to Fire Resistance".to_string();
        let t = std::time::Instant::now();
        let (base, each) = compare(&dir, &xml, "Swap at Merciless Lab (level 68-90)", "Helmet", &[item.clone(), "nonsense".into()]).unwrap();
        println!("first: {:?}\n base {base:?}\n each {each:?}", t.elapsed());
        let t = std::time::Instant::now();
        let again = compare(&dir, &xml, "Swap at Merciless Lab (level 68-90)", "Helmet", &[item]).unwrap();
        println!("again: {:?} {:?}", t.elapsed(), again.1);
        assert!(base.get("Life").copied().unwrap_or(0.0) > 1000.0);
    }

    /// A copied item (Ctrl+C or Ctrl+Alt+C text in POB_ITEM) in POB_SLOT of his Luminary's swap stage.
    #[test]
    #[ignore]
    fn pob_item_live() {
        let text = std::fs::read_to_string(std::env::var("POB_ITEM").unwrap()).unwrap();
        let slot = std::env::var("POB_SLOT").unwrap_or("Ring 1".into());
        let code = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../Ruetoo Luminary aurabot (PoB code).txt")).unwrap();
        let xml = filter_core::pob::decode(code.trim()).unwrap();
        let (base, each) = compare(&std::env::temp_dir().join("pob-live-test"), &xml, "Swap at Merciless Lab (level 68-90)", &slot, &[text]).unwrap();
        let r = each[0].as_ref().unwrap();
        for k in ["Life", "EnergyShield", "Mana", "FireResist", "ColdResist", "LightningResist", "ChaosResist", "TotalEHP"] {
            println!("{k}: {:+.0}", r.get(k).unwrap_or(&0.0) - base.get(k).unwrap_or(&0.0));
        }
    }

    /// Live trade listings through PoB: `POB_WORKER_EXE=<built editor> cargo test -p poe-filter-editor pob_trade_live -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn pob_trade_live() {
        let q = json!({ "query": { "status": { "option": "securable" }, "type": "Deicide Mask", "stats": [{ "type": "and", "filters": [{ "id": "pseudo.pseudo_total_life", "value": { "min": 80 } }] }],
            "filters": { "type_filters": { "filters": { "rarity": { "option": "rare" } } } } }, "sort": { "price": "asc" } });
        let ua = "poe-filter-editor-test";
        let s: Value = ureq::post("https://www.pathofexile.com/api/trade/search/Covens").set("User-Agent", ua).send_json(q).unwrap().into_json().unwrap();
        let ids: Vec<String> = s["result"].as_array().unwrap().iter().take(5).map(|v| v.as_str().unwrap().to_string()).collect();
        std::thread::sleep(Duration::from_secs(2));
        let f: Value = ureq::get(&format!("https://www.pathofexile.com/api/trade/fetch/{}?query={}", ids.join(","), s["id"].as_str().unwrap()))
            .set("User-Agent", ua).call().unwrap().into_json().unwrap();
        let texts: Vec<String> = f["result"].as_array().unwrap().iter().map(|r| listing_text(&r["item"]).unwrap()).collect();
        let code = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../Ruetoo Luminary aurabot (PoB code).txt")).unwrap();
        let xml = filter_core::pob::decode(code.trim()).unwrap();
        let (base, each) = compare(&std::env::temp_dir().join("pob-live-test"), &xml, "Swap at Merciless Lab (level 68-90)", "Helmet", &texts).unwrap();
        for (t, r) in texts.iter().zip(&each) {
            let d = r.as_ref().map(|n| ["Life", "EnergyShield", "FireResist", "ColdResist", "LightningResist", "ChaosResist", "TotalEHP"].iter()
                .map(|k| format!("{k} {:+.0}", n.get(*k).unwrap_or(&0.0) - base.get(*k).unwrap_or(&0.0))).collect::<Vec<_>>().join(", "));
            println!("---
{t}
=> {d:?}");
        }
        assert!(each.iter().filter(|r| r.is_ok()).count() >= 4);
    }
}
