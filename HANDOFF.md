# Handoff: where the PoE Filter Editor is at

Read this first in a new session. The owner (GROBBS) is non-technical: keep updates short, plain English, no code jargon.
He tests on Windows by running `Start editor.bat`, which pulls from GitHub every 30s and rebuilds (`npm run tauri dev`),
so **commit and push after every change** (end commits with the attribution lines the session gives you).

## What the app is
Tauri 2 + React/TS desktop app with a Rust core crate (`core/`, `filter-core`) for Path of Exile loot filters, plus
in-game overlays. Built on NeverSink's filters (tags `$type->… $tier->…`), user changes stored as rules that survive updates.

## Hotkeys
- **F2** quick edit: the item under the mouse (Ctrl+C in inventory, or reads the ground label + game hover box), shows
  what the filter does, **why** (rule box with "Modify this rule" → opens the main app on that rule), and buttons to change it.
- **F3** batch mode, **F4** reload filter in game, **F5** cycle filters (off by default), **F6** build window (PoB gems/gear/tree).
- **Ctrl+D** price check (new, own window `trade`). Changeable in Settings; skipped quietly if another app owns it.

## Done (recent sessions)
- Filter editing: Find, Tiers (FilterBlade-style drag & drop, show/hide, restyle, copy/paste, stack sizes, item tiers 3.30),
  "Opened rule" view for any NeverSink rule that isn't a tier list, Presets (levelling like FilterBlade auto-adjust:
  build buttons, weapon types, armour types, vendor rares, rares-only after area 16), Themes, Prices (off by default),
  My edits, Batch, Help tips, side-menu UI polish.
- Levelling: gear helper (uniques by level/skill, trade links), My build (PoB import, stages follow the game, tree view
  with new points, ascendancy, masteries).
- Ground reader (`src-tauri/src/screen.rs`, core `ground.rs`): Windows OCR of a strip round the mouse; splits side-by-side
  labels; only picks the label actually under the mouse; rejoins labels with lost letters; boosted passes for washed-out
  (HDR) screens incl. 3x local-contrast; reads the game's hover box (lines packed round a real "Item Level: N" line; ignores
  quest tracker / other windows) for base, rarity, item level, unidentified. Hover box rarity only used when the colour is
  clearly a rarity colour AND fits how the filter draws that rarity (stops white items reading as unique). Failed reads
  save a picture + note in `My Games/Path of Exile/ReaderSnapshots` (stage them through the device bridge to debug).
  Matching: short names must be whole words (no "Hope" inside "cHOPPEr"), partial reads ("CHOPP") match the one item
  containing them, cut-off names ("SHARKSKII") match by start.
- Price check (Ctrl+D) — `core/src/trade.rs` (tested offline), `src-tauri/src/trade_api.rs`, `src/TradeWindow.tsx`:
  advanced copy (Ctrl+Alt+C) with mod tiers; stats that matter ticked (T1–T2 mods, total life/res pseudos), junk mods
  dimmed and never ticked; mouse-wheel minimums; white/unidentified items search the base at that ilvl+; poe.ninja price
  instantly; trade site search with listings (price, ilvl, age, seller, Whisper copies the message); poeprices.info AI
  estimate for identified rares; bulk exchange rates for currency/scarabs/etc. (pay in chaos or divines); follows the
  trade site's rate-limit headers and says how long to wait instead of getting timed out; window placed on the side of
  the mouse with more room and kept on screen (both pop-ups). Confirmed working in game (search returned listings).

- Map check (Ctrl+D on a map) — `core/src/maps.rs`: reads every map mod (also ones without numbers), flags those on
  the Settings list "Map check" (`settings.map_avoid`, None = all), "Suggest from my build" uses the PoB endgame stage
  (main skill elemental/physical/minion/DoT → reflect, curses → hexproof, leech, ailments, always max res + no regen).
- Trade helper — `core/src/whispers.rs` + `start_whisper_watcher` in lib.rs: tails Client.txt, trade whispers pop up
  in the `trades` window (TradesWindow.tsx; shown without taking focus: re-focuses the game via `focus_game`).
  Invite / Trade / Thanks / Kick: each press = `focus_game` + one `type_in_chat` line. Turns green when the buyer
  joins your area. Settings: on/off, thanks message, chat hotkeys (`settings.chat_keys`, HotAction::Chat).
- Saved & live searches — `core/src/saved.rs` (saved_searches.json in the data folder), price window "Save search" +
  "Saved searches" list (Run / Live / delete; also shown when Ctrl+D has nothing to price). Live = background
  re-search every 90s (max 3 live), only when `trade_api::has_room` leaves spare requests and not within 20s of a
  manual price check; new listings pop up in the `trades` window with a Whisper button (types the whisper in game).
  Official websocket live search needs a POESESSID login, so this polls instead.

- Price window shows chaos AND divine for every price (orb icons via `CurrencyIcon`, falls back to "c"/"div" text);
  bulk shows "1 div ≈ N items" ratios.
- Maps page (main app, Endgame → Maps; `src/Maps.tsx`, `core/src/mapregex.rs`): tick mods you can't run + minimums
  (quantity, pack size, rarity, more maps/currency/scarabs/cards) → stash regex (`"!tok|tok" "as d.*([8-9]\d|\d{3})%"`,
  250-char limit shown) and "Find maps like this on trade" (stats `not` with the mods' trade ids, map_filters
  map_tier/map_bonus/map_packsize/map_iir, pseudo more-drops, conqueror/elder/shaper/originator implicits).
  Ctrl+D map window has "Copy map regex". Mod list + search codes + trade ids are poe.re's generated data
  (data.poe.re, downloaded daily with UA `poe-filter-editor/0.1 (local app)`, fallback copy in core/data/poere).
  Their data terms: fine for own/small free tools with credit (shown on the page); ask them before selling or a big release.
  Not verified in game yet: the stat-line fragments (from poe.re's English settings) and the trade map filter names.

- Reader fixes (19 Nov): our own pop-ups are hidden before the screen picture (their "item level 68+" text was read
  as a hover box → wrong base/rarity); a hover box name missing a letter ("HROATSTABBER") is accepted; `rarity_unsure`
  on Picked when the filter draws Normal/Magic/Rare alike (NeverSink tiered gear) → note in the F2 panel.
- Transfigured gems price check: `trade_api::item_types` (api/trade/data/items `disc`) rewrites the search type to
  {option, discriminator} (was "Unknown item base type").

- Price check, Awakened PoE Trade ideas (MIT; studied SnosMe/awakened-poe-trade, written fresh in
  `core/src/trade_rules.rs`): totals table (life incl. Str/2, mana incl. Int/2, ele res with x2/x3 multipliers,
  chaos, Str/Dex/Int, attack/cast/move speed, crit, spell/ele damage) looked up by text in the site's pseudo list;
  mods a total counts are removed from the list (hybrids that also give something else stay); uniques only get a
  total when 2+ of their mods add to it. Weapon pDPS/eDPS/total DPS and armour/evasion/ES/ward at 20% quality
  (ticked: single-defence armour; total DPS when phys+ele; pDPS on axes/swords/bows/warstaves) → armour_filters /
  weapon_filters, replacing the local mods. "Open prefix/suffix" from the advanced copy's headings (unticked).
  Price window: "Minimums: exact / −10% / −20%" buttons. Foulborn = name without "Foulborn " + misc mutated,
  mods looked up as "mutated" first; two-line mods joined; numberless mods kept under headings; minimums start
  at the item's roll; status "available" (instant buyout + in person).
- Awakened ideas NOT done yet: roll-quality bar per mod (where in its tier range it rolled), cluster jewel passive
  count rules, flask rules, timeless jewel / anointment / heist / sentinel handling, influence + ilvl bracket
  filters, "hidden filters" toggle, roll ranges as min..max for uniques (± % of roll range, exact on perfect rolls).

- Price check "Compared with your build (PoB)" (`core/src/compare.rs`): checked item vs the PoB stage's gear in the
  same slot (both rings), totals for life(+Str/2)/ES/mana/each res/attributes/speeds/suppress/crit multi/spell dmg,
  plus mods only one side has. Collapsible (remembered).
- Wealth page (Endgame → Wealth; `core/src/wealth.rs`, `src-tauri/src/wealth_app.rs`, `src/Wealth.tsx`,
  `src/EarningsWindow.tsx`, window "earnings"): reads the stash via the session API
  (character-window/get-stash-items, Cookie POESESSID, realm pc/poe2, rate-limited as kind "stash", learns
  X-Rate-Limit-Account too). Values with poe.ninja (uniques by name, gems at their cheapest variant, everything else
  by base; rares/gear skipped). Whole-stash scan → snapshot history (wealth.json, newest 400) + chart + by-tab, and
  "Worth selling": unlisted items ≥ N chaos. Earnings sessions: tick dump tabs, name a strategy, start → tabs
  valued now and every N minutes (`start_session_timer`), overlay shows time / made / per hour; pause, stop & save;
  "Best strategies" averages per name. OAuth would need GGG to register the app, hence POESESSID (kept local).
  Not tested against the live site: realm for PoE 2 and the exact error shapes.

## Left to do (agreed order)
1. **GROBBS to test in game**: trade helper buttons (focus switch + typing), whisper pop-up not stealing focus,
   chat hotkeys, live search pop-ups. Windows-only code (`focus_game`) couldn't be compiled in the cloud.
2. Wealth tracker follow-ups: test with GROBBS's real account; price rares via the trade site (slow) or poeprices;
   PoeStash-style "search every tab" and guild stash; crafting planner (parked).
3. Price check polish ideas: group resistances "any resistance" (count group), collapse duplicate sellers, show price
   in divines toggle, currency trend chart; map tier filter when price checking maps.
4. Deferred earlier: installer + auto-update, batch mode tidy-up, PoE 2 coverage.

## Known caveats
- This cloud workspace can't reach pathofexile.com / poe.ninja / poeprices.info, so network code is only tested by
  GROBBS in game. Windows-only code (`screen.rs` imp module) can't be compiled here: keep logic in the plain functions
  above it (they have tests) and write the Windows part carefully.
- Trade site User-Agent: `OAuth poe-filter-editor/0.1 (contact: github repo)`; if Cloudflare starts blocking, revisit.
- Linux type-check of the app: install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev, then `cargo check -p poe-filter-editor`.
- Test harness: Playwright scripts mocking `window.__TAURI_INTERNALS__` (render `dist/` after `npm run build`); the
  `trade` window is chosen by window label in `src/main.tsx`.
