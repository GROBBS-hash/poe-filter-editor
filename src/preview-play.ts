// Sample data for the play windows in the browser preview: preview-app.html?play=level (or tracker, searches,
// sheet, notes, choice). Shapes mirror play_app.rs / journey.rs.
const now = Math.floor(Date.now() / 1000);

const step = (act: number, parts: [string, string][], subs: string[] = [], rewards: [string, boolean, string][] = []) => ({ act, parts, subs, rewards });

export const levelView = {
  pos: 41, total: 412, current: 2, zone: "The Crossroads", zone_level: 9, char_level: 11, class: "Witch", xp_keep: 1,
  gem_search: "arc|flame wa|frostbo|holy fl|inc d", gems_left_out: [], manual: false, credit: "Route: exile-leveling (MIT)",
  options: { league_start: true, bandit: "kill_all", library: true },
  trials: [{ zone: "The Crypt", lab: "Normal", done: true, here: false }, { zone: "The Chamber of Sins", lab: "Normal", done: false, here: true },
    { zone: "The Crematorium", lab: "Normal", done: false, here: false }],
  res_penalty: 0, poe2: false,
  steps: [
    step(2, [["text", "Take the "], ["wp", "Waypoint"], ["text", " to "], ["enter", "The Crossroads"]]),
    step(2, [["text", "Talk to "], ["quest", "Yeena"], ["text", ", take the "], ["item", "Quicksilver Flask"]]),
    step(2, [["enter", "The Chamber of Sins"], ["text", " · "], ["kill", "Fidelitas"], ["text", " · take the "], ["item", "Baleful Gem"]],
      ["Trial of Ascendancy on level 1"], [["Arc", true, "Yeena"], ["Frostblink", false, "Yeena"]]),
    step(2, [["text", "Go back to "], ["enter", "The Forest Encampment"]]),
    step(2, [["enter", "The Riverways"], ["text", " → "], ["enter", "The Western Forest"]]),
  ],
};

const run = (name: string, level: number, inside: number, made: number | null, deaths = 0) =>
  ({ name, area: "MapWorlds" + name, level, seed: 1, start: now - inside - 60, last_enter: now - inside, in_map: inside, left: 0, deaths, portals: 0, mods: [], made });

export const playView = {
  tracker: {
    current: { ...run("Strand", 81, 214, null), portals: 1, mods: ["Monsters have 60% increased Critical Strike Chance", "Area contains two Unique Bosses", "+30% Monster pack size"] },
    inside: true, current_inside: 214, recent: [run("Tower", 81, 312, 42), run("Dunes", 80, 268, 18, 1)],
    today: { runs: 12, avg_inside: 291, avg_cycle: 372, per_hour: 9.7, deaths: 2, avg_made: 31 },
    all: { runs: 140, avg_inside: 300, avg_cycle: 380, per_hour: 9.5, deaths: 21, avg_made: 28 },
    xp_per_hour: 48_200_000, xp_left: 210_000_000, xp_frac: 0.42, xp_minutes: 262, xp_char: "ArcWitch", xp_level: 94, xp_note: "",
    watch_ms: 83_400, watch_running: false, laps: [41_200, 83_400], now,
    session: { start: now - 3600, played: 3600, maps: 9, map_time: 2700, per_hour: 9, deaths: 1, levels: 0, xp_gained: 41_000_000, made: 280, best: run("Tower", 81, 312, 42) },
  },
};

export const searchView = {
  made: [{ name: "Your build's gems", text: "arc|flame wa|frostbo|holy fl" }, { name: "Maps you can run (Maps page)", text: "\"!s def|t reg\"" }],
  saved: [{ name: "Movement speed", text: "\"y: +\\d\\d% m\"" }, { name: "3-links", text: "-\\w-\\w-" }, { name: "Chromatic (RGB)", text: "r-g-b|r-b-g|g-r-b|g-b-r|b-r-g|b-g-r" },
    { name: "Life and resistances", text: "\"to max.*life\" \"res\"" }, { name: "Quality gems", text: "quality: \\+" }],
};

export const choiceView = {
  divine: 260, error: null,
  read: {
    kind: "altar", pick: 0, priced: [], lines: [],
    choices: [
      { heading: "Map boss", score: 3, danger: false, lines: [{ text: "Drops additional Currency Items", kind: "reward", note: null }, { text: "Gains 25% increased Movement Speed", kind: "", note: null }] },
      { heading: "Eldritch minions", score: -5, danger: true, lines: [{ text: "Drops 2 additional Divination Cards", kind: "reward", note: null }, { text: "Monsters reflect 18% of Elemental Damage", kind: "danger", note: "can't run" }] },
    ],
  },
};

export const notesText = "Buy: 6-link for the Arc witch (under 1 div)\nNext: run Strand with Ambush scarabs\nCheck Kirac missions after act 10";

/** Answers the play windows' commands; null = not a play command. */
export function playMock(cmd: string): unknown {
  switch (cmd) {
    case "level_view": case "level_move": return levelView;
    case "play_view": case "watch_action": case "maps_done": return playView;
    case "searches_get": return searchView;
    case "notes_get": return notesText;
    case "sheets_list": return [];
    default: return undefined;
  }
}
