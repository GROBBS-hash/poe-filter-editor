//! The league you play (26 Nov, he picked the Settings mock-up): prices, trade searches and your stash follow it.
//! Automatic is the newest one; the price job lists the others it has prices for in leagues.json.

use super::*;

#[derive(Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
struct League { id: String, name: String, file: String }

type Lists = std::collections::HashMap<String, Vec<League>>;

static LISTS: Mutex<Option<Lists>> = Mutex::new(None);

fn path(state: &AppState) -> PathBuf { state.data_dir.join("leagues.json") }

fn lists(state: &AppState) -> Lists {
    LISTS.lock().unwrap().get_or_insert_with(||
        std::fs::read_to_string(path(state)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()).clone()
}

/// Reads the list of leagues with prices again (the last one kept when it can't be read).
pub(crate) fn refresh(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Some(text) = ureq::get(&format!("{PRICE_URL}/leagues.json")).set("User-Agent", "poe-filter-editor").timeout(Duration::from_secs(20))
        .call().ok().and_then(|r| r.into_string().ok()) else { return };
    let Ok(l) = serde_json::from_str::<Lists>(&text) else { return };
    let _ = std::fs::write(path(&state), &text);
    *LISTS.lock().unwrap() = Some(l);
}

fn picked(state: &AppState, game: Game) -> Option<String> {
    state.store.lock().unwrap().settings.leagues.get(game_key(game)).cloned()
}

/// The file to read: the picked league's while the job still has it, else the newest league's.
fn file_for(list: &[League], picked: Option<&str>, game: &str) -> String {
    picked.and_then(|p| list.iter().find(|l| l.id == p)).map_or_else(|| format!("{game}.json"), |l| l.file.clone())
}

pub(crate) fn price_file(app: &AppHandle, game: Game) -> String {
    let state = app.state::<AppState>();
    let list = lists(&state).remove(game_key(game)).unwrap_or_default();
    file_for(&list, picked(&state, game).as_deref(), game_key(game))
}

#[derive(Serialize)]
pub struct LeagueView {
    game: Game,
    /// Automatic's league, and what's in use now (from the prices loaded).
    automatic: Option<String>,
    now: Option<String>,
    updated: Option<String>,
    picked: Option<String>,
    /// The one you picked has no prices (any more).
    gone: bool,
    choices: Vec<League>,
}

/// One per game you have a filter for.
#[tauri::command]
pub fn league_get(app: AppHandle) -> Vec<LeagueView> {
    let state = app.state::<AppState>();
    let mut games: Vec<Game> = [Game::Poe1, Game::Poe2].into_iter()
        .filter(|g| state.store.lock().unwrap().profiles.values().any(|p| p.game == *g)).collect();
    if games.is_empty() { games.push(active_game(&state)); }
    let lists = lists(&state);
    games.into_iter().map(|game| {
        let choices = lists.get(game_key(game)).cloned().unwrap_or_default();
        let picked = picked(&state, game);
        let prices = state.prices.lock().unwrap();
        let f = prices.get(&game);
        LeagueView {
            game, automatic: choices.first().map(|l| l.name.clone()),
            now: f.map(|f| f.league_name.clone().unwrap_or_else(|| f.league.clone())), updated: f.map(|f| f.updated.clone()),
            gone: picked.as_ref().is_some_and(|p| !choices.is_empty() && !choices.iter().any(|l| l.id == *p)),
            picked, choices,
        }
    }).collect()
}

/// Picks a league (None = automatic): its prices are downloaded now, and filters with price tiers rebuilt.
#[tauri::command]
pub async fn league_set(app: AppHandle, game: Game, league: Option<String>) -> Result<Vec<LeagueView>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let put = |l: Option<String>| {
            let mut s = state.store.lock().unwrap();
            match l {
                Some(l) => { s.settings.leagues.insert(game_key(game).into(), l); }
                None => { s.settings.leagues.remove(game_key(game)); }
            }
            s.save(&state.store_path).map_err(err)
        };
        let before = picked(&state, game);
        let was = state.prices.lock().unwrap().get(&game).map(|f| f.league.clone());
        put(league.clone())?;
        let changed = fetch_prices(&app);
        let is = state.prices.lock().unwrap().get(&game).map(|f| f.league.clone());
        // Automatic's league is the first one listed.
        let want = league.or_else(|| lists(&state).get(game_key(game)).and_then(|l| l.first()).map(|l| l.id.clone()));
        if want.is_some() && is != want {
            put(before)?;
            return Err(format!("Couldn't get prices for {} just now. Nothing changed; try again in a few minutes.", want.unwrap_or_default()));
        }
        if game == Game::Poe1 && was != is { farm_app::forget_temples(&app); }
        rebuild_priced(&app, &changed);
        if was != is {
            play_app::check_alerts(&app);
            wealth_app::notify(&app);
        }
        let _ = app.emit("prices-updated", ());
        Ok(league_get(app.clone()))
    }).await.map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_for_a_league() {
        let l = |id: &str, file: &str| League { id: id.into(), name: id.into(), file: file.into() };
        let list = [l("Covens", "poe1.json"), l("Hardcore Covens", "poe1_hardcore-covens.json")];
        assert_eq!(file_for(&list, None, "poe1"), "poe1.json");
        assert_eq!(file_for(&list, Some("Hardcore Covens"), "poe1"), "poe1_hardcore-covens.json");
        // The league ended: the newest one's.
        assert_eq!(file_for(&list, Some("Keepers"), "poe1"), "poe1.json");
    }
}
