//! Talks to the official trade site for price checks, keeping well inside its
//! rate limits (it tells us the limits in every answer; we follow them and stop
//! early rather than get timed out).

use filter_core::trade::StatIndex;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

const UA: &str = "OAuth poe-filter-editor/0.1 (contact: https://github.com/GROBBS-hash/poe-filter-editor)";

pub fn api_base(poe2: bool) -> &'static str {
    if poe2 { "https://www.pathofexile.com/api/trade2" } else { "https://www.pathofexile.com/api/trade" }
}

/// The trade site page for a search the app made.
pub fn site_url(poe2: bool, league: &str, id: &str) -> String {
    let league = enc(league);
    if poe2 { format!("https://www.pathofexile.com/trade2/search/poe2/{league}/{id}") }
    else { format!("https://www.pathofexile.com/trade/search/{league}/{id}") }
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

/// Requests made per kind ("search", "fetch") and the limits the site last told us.
#[derive(Default)]
struct Limiter {
    sent: HashMap<&'static str, Vec<Instant>>,
    /// (max requests, window seconds)
    rules: HashMap<&'static str, Vec<(u32, u64)>>,
    blocked_until: HashMap<&'static str, Instant>,
}

fn limiter() -> &'static Mutex<Limiter> {
    static L: OnceLock<Mutex<Limiter>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(Limiter::default()))
}

fn default_rules(kind: &str) -> Vec<(u32, u64)> {
    // Careful guesses until the site tells us its real numbers.
    match kind {
        "search" | "exchange" => vec![(4, 10), (12, 60), (40, 300)],
        "poeprices" => vec![(3, 10), (20, 300)],
        // Your stash on pathofexile.com: slow and steady.
        "stash" => vec![(8, 10), (30, 60)],
        // Character lists and a character's items (checked live 22 Nov: 10/60s + 50/30min, and 30/60s + 100/30min).
        "chars" => vec![(10, 60), (50, 1800)],
        "items" => vec![(30, 60), (100, 1800)],
        _ => vec![(8, 4), (12, 10)],
    }
}

/// "8:10:60,15:60:300" -> [(8, 10), (15, 60)]
fn parse_rules(h: &str) -> Vec<(u32, u64)> {
    h.split(',').filter_map(|r| {
        let mut p = r.split(':');
        Some((p.next()?.trim().parse().ok()?, p.next()?.trim().parse().ok()?))
    }).collect()
}

/// Waits (briefly) until a request of this kind is safe, or says how long to wait.
fn wait_turn(kind: &'static str) -> Result<(), String> {
    let wait = {
        let mut l = limiter().lock().unwrap();
        let now = Instant::now();
        if let Some(until) = l.blocked_until.get(kind) {
            if *until > now { return Err(format!("The trade site asked us to slow down: try again in {}s", (*until - now).as_secs() + 1)); }
        }
        let rules = l.rules.get(kind).cloned().unwrap_or_else(|| default_rules(kind));
        let sent = l.sent.entry(kind).or_default();
        sent.retain(|t| now.duration_since(*t) < Duration::from_secs(1800));
        let mut wait = Duration::ZERO;
        for (max, window) in rules {
            let win = Duration::from_secs(window);
            let recent: Vec<&Instant> = sent.iter().filter(|t| now.duration_since(**t) < win).collect();
            // Keep one request spare under every limit.
            let allowed = max.saturating_sub(1).max(1) as usize;
            if recent.len() >= allowed {
                let oldest = recent[recent.len() - allowed];
                wait = wait.max(win.saturating_sub(now.duration_since(*oldest)));
            }
        }
        wait
    };
    if wait > Duration::from_secs(6) {
        return Err(format!("Waiting on the trade site's limit: try again in {}s", wait.as_secs() + 1));
    }
    std::thread::sleep(wait);
    limiter().lock().unwrap().sent.entry(kind).or_default().push(Instant::now());
    Ok(())
}

/// For background checks (live searches): is there room for a request of this kind
/// right now, leaving `spare` requests free under every limit for your own price checks?
pub fn has_room(kind: &'static str, spare: usize) -> bool {
    let mut l = limiter().lock().unwrap();
    let now = Instant::now();
    if l.blocked_until.get(kind).map(|u| *u > now).unwrap_or(false) { return false; }
    let rules = l.rules.get(kind).cloned().unwrap_or_else(|| default_rules(kind));
    let sent = l.sent.entry(kind).or_default();
    rules.iter().all(|(max, window)| {
        let recent = sent.iter().filter(|t| now.duration_since(**t) < Duration::from_secs(*window)).count();
        let allowed = (*max as usize).saturating_sub(1);
        recent + spare.min(allowed / 2) < allowed
    })
}

fn learn(kind: &'static str, resp: &ureq::Response) {
    let mut l = limiter().lock().unwrap();
    // Limits per IP and (for your own stash) per account: follow both.
    let mut rules: Vec<(u32, u64)> = Vec::new();
    for h in ["X-Rate-Limit-Ip", "X-Rate-Limit-Account"] {
        if let Some(r) = resp.header(h) { rules.extend(parse_rules(r)); }
    }
    if !rules.is_empty() { l.rules.insert(kind, rules); }
    // The site's own count: if it says we're at a limit, stop until that window passes.
    for (rh, sh) in [("X-Rate-Limit-Ip", "X-Rate-Limit-Ip-State"), ("X-Rate-Limit-Account", "X-Rate-Limit-Account-State")] {
    if let (Some(r), Some(s)) = (resp.header(rh), resp.header(sh)) {
        for ((max, window), state) in parse_rules(r).into_iter().zip(s.split(',')) {
            let mut p = state.split(':');
            let used: u32 = p.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            let _ = p.next();
            let penalty: u64 = p.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            if penalty > 0 { l.blocked_until.insert(kind, Instant::now() + Duration::from_secs(penalty)); }
            else if used >= max { l.blocked_until.insert(kind, Instant::now() + Duration::from_secs(window)); }
        }
    }
    }
}

fn call(kind: &'static str, req: ureq::Request, body: Option<&Value>) -> Result<Value, String> {
    wait_turn(kind)?;
    let req = req.set("User-Agent", UA).set("Accept", "application/json").timeout(Duration::from_secs(15));
    let res = match body { Some(b) => req.send_json(b.clone()), None => req.call() };
    match res {
        Ok(resp) => {
            learn(kind, &resp);
            resp.into_json::<Value>().map_err(|e| format!("The trade site sent something unexpected: {e}"))
        }
        Err(ureq::Error::Status(429, resp)) => {
            let secs: u64 = resp.header("Retry-After").and_then(|v| v.parse().ok()).unwrap_or(60);
            limiter().lock().unwrap().blocked_until.insert(kind, Instant::now() + Duration::from_secs(secs));
            Err(format!("The trade site asked us to slow down: try again in {secs}s"))
        }
        Err(ureq::Error::Status(code, resp)) => {
            learn(kind, &resp);
            let msg = resp.into_json::<Value>().ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or_default();
            Err(format!("Trade site error {code}{}", if msg.is_empty() { String::new() } else { format!(": {msg}") }))
        }
        Err(e) => Err(format!("Couldn't reach the trade site: {e}")),
    }
}

/// Searches; returns (search id, listing ids cheapest first, how many in total).
pub fn search(poe2: bool, league: &str, query: &Value) -> Result<(String, Vec<String>, u64), String> {
    let url = format!("{}/search/{}", api_base(poe2), enc(league));
    let v = call("search", ureq::post(&url), Some(query))?;
    let id = v["id"].as_str().ok_or("The trade site didn't return a search")?.to_string();
    let ids = v["result"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect();
    Ok((id, ids, v["total"].as_u64().unwrap_or(0)))
}

/// Details (price, seller…) for up to 10 listings.
pub fn fetch(poe2: bool, ids: &[String], search_id: &str) -> Result<Value, String> {
    if ids.is_empty() { return Ok(serde_json::json!({ "result": [] })); }
    let url = format!("{}/fetch/{}?query={}", api_base(poe2), ids[..ids.len().min(10)].join(","), search_id);
    call("fetch", ureq::get(&url), None)
}

/// One of the trade site's data lists ("stats", "static"), kept on disk for a day.
fn data(data_dir: &Path, poe2: bool, kind: &str) -> Result<std::sync::Arc<Value>, String> {
    static CACHE: OnceLock<Mutex<HashMap<(bool, String), std::sync::Arc<Value>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(v) = cache.lock().unwrap().get(&(poe2, kind.to_string())) { return Ok(v.clone()); }
    let path = data_dir.join(format!("trade_{kind}_{}.json", if poe2 { "poe2" } else { "poe1" }));
    let fresh = std::fs::metadata(&path).and_then(|m| m.modified()).ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok()).map(|age| age < Duration::from_secs(86_400)).unwrap_or(false);
    let v: Value = if fresh {
        serde_json::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?
    } else {
        match call("data", ureq::get(&format!("{}/data/{kind}", api_base(poe2))), None) {
            Ok(v) => { let _ = std::fs::write(&path, v.to_string()); v }
            // Offline: an old copy is better than none.
            Err(e) => serde_json::from_str(&std::fs::read_to_string(&path).map_err(|_| e)?).map_err(|e| e.to_string())?,
        }
    };
    let v = std::sync::Arc::new(v);
    cache.lock().unwrap().insert((poe2, kind.to_string()), v.clone());
    Ok(v)
}

/// The trade site's stat list.
pub fn stats(data_dir: &Path, poe2: bool) -> Result<StatIndex, String> {
    Ok(StatIndex::from_json(&*data(data_dir, poe2, "stats")?))
}

/// The trade site's bulk-exchange item ids.
pub fn statics(data_dir: &Path, poe2: bool) -> Result<filter_core::trade::StaticIndex, String> {
    Ok(filter_core::trade::StaticIndex::from_json(&*data(data_dir, poe2, "static")?))
}

/// The trade site's item list (for transfigured gems' search names).
pub fn item_types(data_dir: &Path, poe2: bool) -> Result<filter_core::trade::ItemTypes, String> {
    Ok(filter_core::trade::ItemTypes::from_json(&*data(data_dir, poe2, "items")?))
}

/// Bulk exchange search; returns (search id, answer).
pub fn exchange(poe2: bool, league: &str, query: &Value) -> Result<(String, Value), String> {
    let url = format!("{}/exchange/{}", api_base(poe2), enc(league));
    let v = call("exchange", ureq::post(&url), Some(query))?;
    Ok((v["id"].as_str().unwrap_or("").to_string(), v))
}

pub fn exchange_url(poe2: bool, league: &str, id: &str) -> String {
    let league = enc(league);
    if poe2 { format!("https://www.pathofexile.com/trade2/exchange/poe2/{league}/{id}") }
    else { format!("https://www.pathofexile.com/trade/exchange/{league}/{id}") }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rules_from_headers() {
        assert_eq!(super::parse_rules("8:10:60,15:60:300"), vec![(8, 10), (15, 60)]);
        assert!(super::site_url(false, "Standard", "abc").ends_with("/trade/search/Standard/abc"));
    }

    #[test]
    fn account_name_from_profile_page() {
        let page = "<!DOCTYPE html><html lang=\"en\">\n<head>\n<title>View Profile - Path of Exile - Some_Name#1234</title><meta x>";
        assert_eq!(super::account_from_page(page).as_deref(), Some("Some_Name#1234"));
        assert_eq!(super::account_from_page("<title>Login - Path of Exile</title>"), None);
        assert_eq!(super::account_from_page("no title"), None);
    }
}

// ---------- your stash (wealth tracker) ----------

/// One page of your stash from pathofexile.com, signed in with your POESESSID
/// (it only ever goes to pathofexile.com). Waits out the rate limit instead of failing,
/// since a full stash scan is many requests.
pub fn stash_tab(account: &str, sessid: &str, league: &str, poe2: bool, index: usize, with_tabs: bool) -> Result<Value, String> {
    stash_page("get-stash-items", account, sessid, league, poe2, index, with_tabs)
}

/// One page of your guild's stash (same sign-in, same limits). Errors when you're not in a guild or can't view it.
pub fn guild_stash_tab(account: &str, sessid: &str, league: &str, poe2: bool, index: usize, with_tabs: bool) -> Result<Value, String> {
    stash_page("get-guild-stash-items", account, sessid, league, poe2, index, with_tabs)
}

/// Your characters (name, league, class, level, experience) from pathofexile.com, signed in with your POESESSID.
/// The map tracker reads your level from it (the site no longer sends experience, Nov 2026).
pub fn characters(account: &str, sessid: &str, poe2: bool) -> Result<Value, String> {
    let url = format!("https://www.pathofexile.com/character-window/get-characters?accountName={}&realm={}",
        enc(account), if poe2 { "poe2" } else { "pc" });
    let req = ureq::get(&url).set("Cookie", &format!("POESESSID={}", sessid.trim()));
    // The site counts character lists apart from stash tabs (its own limit: 10 a minute, 50 in 30 minutes).
    match call("chars", req, None) {
        Ok(v) if v.get("error").is_some() => Err(stash_error(&v["error"]["message"].as_str().unwrap_or("").to_string())),
        Ok(v) => Ok(v),
        Err(e) => Err(e),
    }
}

/// Whose website login this POESESSID is: "Name#1234", or None when it isn't logged in. (pathofexile.com/my-account
/// answers 401 to a logged-out session; logged in, its page title is "View Profile - Path of Exile - Name#1234".
/// The old get-account-name call is gone, checked 22 Nov 2026.)
pub fn account_for_session(sessid: &str) -> Result<Option<String>, String> {
    let req = ureq::get("https://www.pathofexile.com/my-account")
        .set("Cookie", &format!("POESESSID={}", sessid.trim())).set("User-Agent", UA).timeout(Duration::from_secs(15));
    match req.call() {
        Ok(r) => Ok(account_from_page(&r.into_string().unwrap_or_default())),
        Err(ureq::Error::Status(401 | 403, _)) => Ok(None),
        Err(e) => Err(format!("Couldn't reach pathofexile.com: {e}")),
    }
}

/// The account name in the "View Profile" page's title.
pub fn account_from_page(html: &str) -> Option<String> {
    let start = html.find("<title>")? + 7;
    let title = &html[start..start + html[start..].find("</title>")?];
    let name = title.trim().strip_prefix("View Profile - Path of Exile - ")?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// One character's items (what it wears and its bags) plus `character` (name, level, lastLoginTime = its last zone
/// change, when the site saved it). The map tracker reads it before and after each map visit.
pub fn character_items(account: &str, sessid: &str, character: &str, poe2: bool) -> Result<Value, String> {
    let url = format!("https://www.pathofexile.com/character-window/get-items?accountName={}&realm={}&character={}",
        enc(account), if poe2 { "poe2" } else { "pc" }, enc(character));
    let req = ureq::get(&url).set("Cookie", &format!("POESESSID={}", sessid.trim()));
    match call("items", req, None) {
        Ok(v) if v.get("error").is_some() => Err(stash_error(&v["error"]["message"].as_str().unwrap_or("").to_string())),
        Ok(v) => Ok(v),
        Err(e) => Err(e),
    }
}

fn stash_page(kind: &str, account: &str, sessid: &str, league: &str, poe2: bool, index: usize, with_tabs: bool) -> Result<Value, String> {
    let url = format!("https://www.pathofexile.com/character-window/{kind}?accountName={}&realm={}&league={}&tabs={}&tabIndex={}",
        enc(account), if poe2 { "poe2" } else { "pc" }, enc(league), if with_tabs { 1 } else { 0 }, index);
    let mut tries = 0;
    loop {
        tries += 1;
        let req = ureq::get(&url).set("Cookie", &format!("POESESSID={}", sessid.trim()));
        match call("stash", req, None) {
            Ok(v) if v.get("error").is_some() => return Err(stash_error(&v["error"]["message"].as_str().unwrap_or("").to_string())),
            Ok(v) => return Ok(v),
            Err(e) if tries < 6 && (e.contains("slow down") || e.contains("limit")) => {
                let secs: u64 = e.split("in ").last().and_then(|x| x.trim_end_matches('s').parse().ok()).unwrap_or(10);
                std::thread::sleep(Duration::from_secs(secs.min(90) + 1));
            }
            Err(e) => return Err(stash_error(&e)),
        }
    }
}

fn stash_error(e: &str) -> String {
    if e.contains("403") || e.to_lowercase().contains("forbidden") {
        "Your pathofexile.com login has expired: press \"Log in again\" (or \"Log in with pathofexile.com\" in Settings → Map tracker).".into()
    } else if e.contains("404") || e.to_lowercase().contains("not found") {
        "pathofexile.com couldn't find that account or league: check your account name (with its #1234) and league.".into()
    } else { e.replace("trade site", "pathofexile.com") }
}
