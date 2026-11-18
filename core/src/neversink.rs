//! NeverSink's filters straight from his GitHub: pick a strictness inside the
//! app and get his updates automatically (no FilterBlade round trip).

use crate::profiles::Game;

pub const STRICTNESS: [&str; 7] = ["SOFT", "REGULAR", "SEMI-STRICT", "STRICT", "VERY-STRICT", "UBER-STRICT", "UBER-PLUS-STRICT"];
pub const STRICTNESS_NAMES: [&str; 7] = ["Soft", "Regular", "Semi-Strict", "Strict", "Very Strict", "Uber Strict", "Uber Plus Strict"];

/// The file name NeverSink uses, e.g. "NeverSink's filter - 2-SEMI-STRICT.filter".
pub fn file_name(game: Game, level: u8) -> String {
    let n = level.min(6) as usize;
    match game {
        Game::Poe1 => format!("NeverSink's filter - {n}-{}.filter", STRICTNESS[n]),
        Game::Poe2 => format!("NeverSink's filter 2 - {n}-{}.filter", STRICTNESS[n]),
    }
}

/// Where to download it from (his GitHub, raw file).
pub fn url(game: Game, level: u8) -> String {
    let (repo, branch) = match game {
        Game::Poe1 => ("NeverSink-Filter", "master"),
        Game::Poe2 => ("NeverSink-PoE2litefilter", "main"),
    };
    let encoded = file_name(game, level).replace('\'', "%27").replace(' ', "%20");
    format!("https://raw.githubusercontent.com/NeverSinkDev/{repo}/{branch}/{encoded}")
}

/// "8.21.0e" from the "# VERSION:" line in the header.
pub fn version_of(text: &str) -> Option<String> {
    text.lines().take(30).find_map(|l| l.trim_start_matches(['#', ' ']).strip_prefix("VERSION:").map(|v| v.trim().to_string()))
}

/// A real NeverSink filter (not an error page or something else)?
pub fn looks_like_neversink(text: &str) -> bool {
    text.lines().take(10).any(|l| l.contains("NeverSink")) && text.contains("$type->")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_urls() {
        assert_eq!(file_name(Game::Poe1, 2), "NeverSink's filter - 2-SEMI-STRICT.filter");
        assert_eq!(url(Game::Poe2, 2), "https://raw.githubusercontent.com/NeverSinkDev/NeverSink-PoE2litefilter/main/NeverSink%27s%20filter%202%20-%202-SEMI-STRICT.filter");
        let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/poe1-semistrict.filter")).unwrap();
        assert_eq!(version_of(&t).as_deref(), Some("8.21.0e"));
        assert!(looks_like_neversink(&t));
        assert!(!looks_like_neversink("404: Not Found"));
    }
}
