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

const CDN = "https://web.poecdn.com/image/Art/2DItems/";
const L = (name: string, cat: string, stack: number, value: number, icon = "", tier: number | null = null) =>
  ({ name, cat, stack, value, icon: icon ? CDN + icon : "", tier });
const parkLoot = [
  L("Divine Orb", "currency", 1, 212, "Currency/CurrencyModValues.png"),
  { ...L("Unidentified Legion Sword", "uniques", 1, 8, "", 3), high: 424 },
  L("Ashes of the Stars Onyx Amulet", "uniques", 1, 95, "", 2),
  L("The Nurse", "cards", 1, 38, "Divination/InventoryIcon.png"),
  L("Chaos Orb", "currency", 23, 23, "Currency/CurrencyRerollRare.png"),
  L("Exalted Orb", "currency", 1, 14, "Currency/CurrencyAddModToRare.png"),
  L("Map (Tier 16)", "maps", 3, 12),
  L("Divination Scarab of Curation", "scarabs", 2, 9),
  L("Orb of Alchemy", "currency", 9, 4, "Currency/CurrencyUpgradeToRare.png"),
  L("Simulacrum Splinter", "fragments", 6, 4),
  L("Rain of Chaos", "cards", 3, 3, "Divination/InventoryIcon.png"),
  L("Unidentified Leather Belt", "uniques", 1, 1, "", 3),
];
const run = (name: string, level: number, inside: number, made: number | null, deaths = 0, loot: ReturnType<typeof L>[] = [], ago = 0) =>
  ({ name, area: "MapWorlds" + name, level, seed: 1, start: now - ago - inside - 60, last_enter: now - ago - inside, in_map: inside, left: now - ago, deaths, portals: 0, mods: [], made, paused: 0, loot });
export const sampleRuns = [
  run("Tower", 83, 280, null, 0, [L("Divination Scarab of Ambush", "scarabs", 4, 38), L("Chaos Orb", "currency", 31, 31, "Currency/CurrencyRerollRare.png"), L("Rain of Chaos", "cards", 4, 4, "Divination/InventoryIcon.png")], 2400),
  run("Strand", 83, 210, null, 0, [L("Mageblood Heavy Belt", "uniques", 1, 22000, "", 1), L("Chaos Orb", "currency", 12, 12, "Currency/CurrencyRerollRare.png")], 1800),
  run("Cursed Crypt", 83, 236, null, 0, [L("Chaos Orb", "currency", 18, 18, "Currency/CurrencyRerollRare.png"), L("Map (Tier 16)", "maps", 2, 8), L("Simulacrum Splinter", "fragments", 3, 2)], 1200),
  { ...run("Colonnade", 83, 304, null, 3, [L("The Nurse", "cards", 1, 38, "Divination/InventoryIcon.png"), L("Exalted Orb", "currency", 1, 14, "Currency/CurrencyAddModToRare.png"), L("Chaos Orb", "currency", 9, 9, "Currency/CurrencyRerollRare.png")], 600), paused: 95 },
];

export const playView = {
  tracker: {
    current: { ...run("Park", 83, 252, null, 0, parkLoot), portals: 1, mods: ["Monsters have 60% increased Critical Strike Chance", "Area contains two Unique Bosses", "+30% Monster pack size"] },
    inside: true, current_inside: 252, paused: false, recent: sampleRuns.slice().reverse(),
    today: { runs: 12, avg_inside: 291, avg_cycle: 372, per_hour: 9.7, deaths: 2, avg_made: null },
    all: { runs: 140, avg_inside: 300, avg_cycle: 380, per_hour: 9.5, deaths: 21, avg_made: null },
    char_name: "NerfDis", char_level: 91,
    watch_ms: 83_400, watch_running: false, laps: [41_200, 83_400], now,
    session: { start: now - 3600, played: 3600, maps: 4, map_time: 1030, per_hour: 9, deaths: 3, levels: 0, made: null, best: null,
      loot: parkLoot, loot_value: 2140, loot_maps: 5, loot_secs: 3000 },
    pause: { on: true, ready: true, esc_ready: false, note: "" },
    loot_prefs: { on: true, hide: ["gear"], min_chaos: 2 },
    loot_note: "",
    divine: 212,
  },
};

export const searchView = {
  made: [{ name: "Your build's gems", text: "arc|flame wa|frostbo|holy fl" }, { name: "My map regex (Map mods page)", text: "\"!s def|t reg\"" }],
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
    case "play_view": case "watch_action": case "maps_done": case "pause_prefs": case "loot_prefs_save": return playView;
    case "maps_history": return sampleRuns;
    case "play_prefs_get": return false;
    case "searches_get": return searchView;
    case "notes_get": return notesText;
    case "sheets_list": return [];
    default: return undefined;
  }
}
