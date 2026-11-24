//! Shop sales and stats on your phone (24 Nov 2026, Left to do 8a/8b).
//!
//! - **Alerts (8a):** a push to your phone when a buyer whispers ("wants X for Y", with the Fair/Underpriced check),
//!   when that buyer arrives in your hideout, and when a listed item is gone from a trade tab (a sale, or moved).
//!   Sent through ntfy (ntfy.sh: free app, you pick a topic name) and/or a Discord webhook to your own channel.
//!   "Only while I'm away": only when the game isn't the window in front.
//! - **Stats page (8b):** the editor serves a small read-only page on your home network (sales, stash worth, earnings
//!   per strategy, recent maps). The address has a secret key in it; nothing is open to the internet (for away from
//!   home, Tailscale). Off by default.
//!
//! Alerts and stats only: nothing here answers or finishes trades (GGG's rules). The PC and the editor must be on.

use super::*;
use filter_core::shop::{Listed, Sale};
use std::collections::HashMap;

#[derive(Serialize, serde::Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PhonePrefs {
    /// ntfy topic name (the phone app subscribes to the same name). Empty = off.
    pub ntfy_topic: String,
    /// ntfy server (ntfy.sh unless you run your own).
    pub ntfy_server: String,
    /// Discord webhook URL for your own channel. Empty = off.
    pub discord: String,
    pub on_whisper: bool,
    pub on_arrive: bool,
    pub on_sold: bool,
    /// Only send while the game isn't the window in front.
    pub only_away: bool,
    /// Read your trade tabs every N minutes for sales.
    pub shop_watch: bool,
    pub shop_minutes: u32,
    /// The phone stats page.
    pub page_on: bool,
    pub page_port: u16,
    pub page_key: String,
}

impl Default for PhonePrefs {
    fn default() -> Self {
        Self { ntfy_topic: String::new(), ntfy_server: "https://ntfy.sh".into(), discord: String::new(), on_whisper: true, on_arrive: true,
            on_sold: true, only_away: true, shop_watch: false, shop_minutes: 5, page_on: false, page_port: 7777, page_key: String::new() }
    }
}

#[derive(Serialize, serde::Deserialize, Clone, Default)]
#[serde(default)]
struct PhoneStore {
    prefs: PhonePrefs,
    /// Sales (and items moved out of trade tabs), newest last, kept to 500.
    sales: Vec<Sale>,
    /// The listed items at the last read, per tab index.
    shop: HashMap<usize, Vec<Listed>>,
}

static STORE: Mutex<Option<PhoneStore>> = Mutex::new(None);

fn path(app: &AppHandle) -> PathBuf { app.state::<AppState>().data_dir.join("phone.json") }

fn with_store<T>(app: &AppHandle, f: impl FnOnce(&mut PhoneStore) -> T) -> T {
    let mut g = STORE.lock().unwrap();
    let s = g.get_or_insert_with(|| {
        let mut s: PhoneStore = std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if s.prefs.page_key.is_empty() { s.prefs.page_key = new_key(); }
        s
    });
    f(s)
}

fn save(app: &AppHandle) {
    let text = with_store(app, |s| serde_json::to_string(s).ok());
    if let Some(t) = text {
        let p = path(app);
        let tmp = p.with_extension("json.tmp");
        if std::fs::write(&tmp, t).is_ok() { let _ = std::fs::rename(tmp, p); }
    }
}

fn unix() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }

/// A hard-to-guess key for the stats page address.
fn new_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) + i);
        h.write_u32(std::process::id());
        out.push_str(&format!("{:x}", h.finish()));
    }
    out.chars().take(20).collect()
}

// ---------- sending ----------

/// Sends one message to the phone (ntfy and/or Discord), in the background. `force`: send even while you're playing
/// (the Test button).
pub fn send(app: &AppHandle, title: &str, body: &str, force: bool) {
    let p = with_store(app, |s| s.prefs.clone());
    if p.ntfy_topic.trim().is_empty() && p.discord.trim().is_empty() { return; }
    if !force && p.only_away && game_in_front() { return; }
    let (title, body) = (title.to_string(), body.to_string());
    thread::spawn(move || {
        if !p.ntfy_topic.trim().is_empty() {
            let url = format!("{}/{}", p.ntfy_server.trim().trim_end_matches('/'), p.ntfy_topic.trim());
            let _ = ureq::post(&url).set("Title", &title).set("Tags", "moneybag").timeout(Duration::from_secs(10)).send_string(&body);
        }
        if p.discord.trim().starts_with("https://") {
            let _ = ureq::post(p.discord.trim()).timeout(Duration::from_secs(10))
                .send_json(serde_json::json!({ "content": format!("**{title}**\n{body}") }));
        }
    });
}

/// Buyers who whispered in the last 15 minutes (so "arrived" only fires for a buyer).
static BUYERS: Mutex<Vec<(String, i64)>> = Mutex::new(Vec::new());

/// Every trade-log event (from the log watcher in lib.rs).
pub fn on_log_event(app: &AppHandle, ev: &filter_core::whispers::LogEvent) {
    use filter_core::whispers::LogEvent;
    let p = with_store(app, |s| s.prefs.clone());
    match ev {
        LogEvent::Whisper(w) if w.item.is_some() => {
            BUYERS.lock().unwrap().retain(|b| unix() - b.1 < 900);
            BUYERS.lock().unwrap().push((w.from.clone(), unix()));
            if !p.on_whisper { return; }
            let item = w.item.clone().unwrap_or_default();
            let check = w.price.as_ref().and_then(|pr| play_app::trade_price_check(app.clone(), item.clone(), pr.clone()));
            let (t, b) = filter_core::shop::whisper_text(&w.from, &item, w.price.as_deref(), check.as_ref().map(|c| c.verdict.as_str()));
            send(app, &t, &b, false);
        }
        LogEvent::Joined { name } if p.on_arrive => {
            let buyer = BUYERS.lock().unwrap().iter().any(|b| b.0 == *name && unix() - b.1 < 900);
            if buyer { send(app, &format!("{name} is in your hideout"), "The buyer arrived: trade when you're back at the PC.", false); }
        }
        _ => {}
    }
}

// ---------- the shop watcher ----------

/// Reads your trade tabs every few minutes and logs what's gone (a sale, or moved out), with a phone alert.
pub fn start_shop_watch(app: AppHandle) {
    thread::spawn(move || {
        let mut last = 0i64;
        loop {
            thread::sleep(Duration::from_secs(20));
            let p = with_store(&app, |s| s.prefs.clone());
            if !p.shop_watch || unix() - last < p.shop_minutes.max(2) as i64 * 60 { continue; }
            last = unix();
            let _ = check_shop(&app);
        }
    });
}

fn check_shop(app: &AppHandle) -> Result<usize, String> {
    let tabs = wealth_app::shop_tabs(app);
    if tabs.is_empty() { return Err("No trade tabs found: scan your stash on the Wealth page first".into()); }
    let rates = currency_rates(app);
    let rate = |id: &str| rates.get(id).copied();
    let mut new_sales = vec![];
    for (index, name) in tabs {
        let items = wealth_app::read_tab_items(app, index)?;
        let now = filter_core::shop::listed_in_tab(&items, &name);
        let before = with_store(app, |s| s.shop.insert(index, now.clone()));
        if let Some(before) = before {
            new_sales.extend(filter_core::shop::sold(&before, &now, unix(), &|n, c| filter_core::shop::note_chaos(n, c, &rate)));
        }
    }
    let n = new_sales.len();
    let on_sold = with_store(app, |s| {
        s.sales.extend(new_sales.iter().cloned());
        if s.sales.len() > 500 { let k = s.sales.len() - 500; s.sales.drain(..k); }
        s.prefs.on_sold
    });
    save(app);
    if on_sold {
        for sale in &new_sales {
            let price = sale.chaos.map(|c| format!(" for about {}c", c.round())).unwrap_or_default();
            let count = if sale.count > 1 { format!("{} x ", sale.count) } else { String::new() };
            send(app, "Sold (or moved)", &format!("{count}{}{price} ({})", sale.name, sale.tab), false);
        }
    }
    if n > 0 { let _ = app.emit("phone-sales", n); }
    Ok(n)
}

// ---------- commands ----------

#[derive(Serialize, Clone)]
pub struct PhoneView { prefs: PhonePrefs, sales: Vec<Sale>, page_url: Option<String>, page_error: Option<String> }

fn view(app: &AppHandle) -> PhoneView {
    let (prefs, mut sales) = with_store(app, |s| (s.prefs.clone(), s.sales.clone()));
    sales.reverse();
    sales.truncate(100);
    let page_url = prefs.page_on.then(|| format!("http://{}:{}/?k={}", lan_ip().unwrap_or_else(|| "this-pc".into()), prefs.page_port, prefs.page_key));
    PhoneView { prefs, sales, page_url, page_error: PAGE_ERROR.lock().unwrap().clone() }
}

#[tauri::command]
pub fn phone_get(app: AppHandle) -> PhoneView { view(&app) }

#[tauri::command]
pub fn phone_save(app: AppHandle, prefs: PhonePrefs) -> PhoneView {
    with_store(&app, |s| {
        let key = s.prefs.page_key.clone();
        s.prefs = PhonePrefs { page_key: if prefs.page_key.is_empty() { key } else { prefs.page_key.clone() }, shop_minutes: prefs.shop_minutes.max(2), ..prefs };
    });
    save(&app);
    start_page(&app);
    view(&app)
}

/// "Send a test message" (always sends, even with the game in front).
#[tauri::command]
pub fn phone_test(app: AppHandle) -> Result<(), String> {
    let p = with_store(&app, |s| s.prefs.clone());
    if p.ntfy_topic.trim().is_empty() && p.discord.trim().is_empty() { return Err("Fill in an ntfy topic or a Discord webhook first".into()); }
    send(&app, "PoE Lodestone", "Phone alerts work. You'll get buyers, arrivals and sales here.", true);
    Ok(())
}

/// "Check my shop now".
#[tauri::command(async)]
pub fn phone_check_shop(app: AppHandle) -> Result<PhoneView, String> {
    check_shop(&app)?;
    Ok(view(&app))
}

/// A new secret for the stats page address (the old link stops working).
#[tauri::command]
pub fn phone_new_key(app: AppHandle) -> PhoneView {
    with_store(&app, |s| s.prefs.page_key = new_key());
    save(&app);
    view(&app)
}

// ---------- the stats page ----------

static PAGE_ERROR: Mutex<Option<String>> = Mutex::new(None);
/// The port the page is being served on (None = not running).
static PAGE_PORT: Mutex<Option<u16>> = Mutex::new(None);

/// This PC's address on the home network (no traffic is sent to find it).
fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.168.255.255:9").ok().or_else(|| s.connect("10.255.255.255:9").ok())?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// Starts the page if it's switched on (once; a port change needs an editor restart).
pub fn start_page(app: &AppHandle) {
    let p = with_store(app, |s| s.prefs.clone());
    if !p.page_on { return; }
    let mut running = PAGE_PORT.lock().unwrap();
    if running.is_some() { return; }
    match std::net::TcpListener::bind(("0.0.0.0", p.page_port)) {
        Ok(l) => {
            *running = Some(p.page_port);
            *PAGE_ERROR.lock().unwrap() = None;
            let app = app.clone();
            thread::spawn(move || {
                for stream in l.incoming().flatten() {
                    let app = app.clone();
                    thread::spawn(move || { let _ = serve(&app, stream); });
                }
            });
        }
        Err(e) => *PAGE_ERROR.lock().unwrap() = Some(format!("Couldn't open port {}: {e}", p.page_port)),
    }
}

fn serve(app: &AppHandle, mut s: std::net::TcpStream) -> std::io::Result<()> {
    use std::io::{BufRead, BufReader, Write};
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut first = String::new();
    BufReader::new(&s).read_line(&mut first)?;
    let target = first.split_whitespace().nth(1).unwrap_or("/");
    let (route, query) = target.split_once('?').unwrap_or((target, ""));
    let key = with_store(app, |st| st.prefs.clone());
    let ok = key.page_on && query.split('&').any(|kv| kv == format!("k={}", key.page_key));
    let (status, kind, body) = if !ok {
        ("403 Forbidden", "text/plain", "Open the link shown in the editor (Settings → Phone).".to_string())
    } else if route == "/data.json" {
        ("200 OK", "application/json", stats_json(app).to_string())
    } else {
        ("200 OK", "text/html; charset=utf-8", PAGE.to_string())
    };
    write!(s, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}", body.len())?;
    s.flush()
}

/// Everything the page shows, read-only.
fn stats_json(app: &AppHandle) -> serde_json::Value {
    let wealth = serde_json::to_value(wealth_app::wealth_get(app.clone())).unwrap_or_default();
    let play = serde_json::to_value(play_app::play_view(app.clone())).unwrap_or_default();
    let maps: Vec<serde_json::Value> = play_app::maps_history(app.clone()).into_iter().rev().take(20)
        .map(|r| {
            let loot: f64 = r.loot.iter().map(|l| l.value).sum();
            let best = r.loot.iter().max_by(|a, b| a.value.total_cmp(&b.value)).map(|l| l.name.clone());
            serde_json::json!({ "name": r.name, "level": r.level, "start": r.start, "in_map": r.in_map, "deaths": r.deaths, "made": r.made, "loot": loot, "best": best })
        })
        .collect();
    let sales: Vec<Sale> = with_store(app, |s| s.sales.iter().rev().take(50).cloned().collect());
    let snaps: Vec<serde_json::Value> = wealth["snapshots"].as_array().cloned().unwrap_or_default().into_iter().rev().take(60)
        .map(|s| serde_json::json!({ "time": s["time"], "total": s["total"], "divine": s["divine"] })).collect();
    serde_json::json!({
        "now": unix(), "divine": wealth["divine"], "league": wealth["league"],
        "sales": sales, "snapshots": snaps, "strategies": wealth["strategies"], "current": wealth["current"],
        "session": play["tracker"]["session"], "maps": maps,
    })
}

const PAGE: &str = include_str!("phone_page.html");
