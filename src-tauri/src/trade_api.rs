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

fn learn(kind: &'static str, resp: &ureq::Response) {
    let mut l = limiter().lock().unwrap();
    if let Some(r) = resp.header("X-Rate-Limit-Ip") {
        let rules = parse_rules(r);
        if !rules.is_empty() { l.rules.insert(kind, rules); }
    }
    // The site's own count: if it says we're at a limit, stop until that window passes.
    if let (Some(r), Some(s)) = (resp.header("X-Rate-Limit-Ip"), resp.header("X-Rate-Limit-Ip-State")) {
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

/// poeprices.info's rough price for a rare, from its copied text.
pub fn poeprices(league: &str, item_text: &str) -> Result<Value, String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(item_text.replace('\r', ""));
    let url = format!("https://www.poeprices.info/api?l={}&i={}&s=poe-filter-editor", enc(league), enc(&b64));
    wait_turn("poeprices")?;
    ureq::get(&url).set("User-Agent", UA).timeout(Duration::from_secs(15)).call()
        .map_err(|e| format!("poeprices.info didn't answer: {e}"))?
        .into_json::<Value>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn rules_from_headers() {
        assert_eq!(super::parse_rules("8:10:60,15:60:300"), vec![(8, 10), (15, 60)]);
        assert!(super::site_url(false, "Standard", "abc").ends_with("/trade/search/Standard/abc"));
    }
}
