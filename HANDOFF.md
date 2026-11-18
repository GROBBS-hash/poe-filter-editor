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

## Left to do (agreed order)
1. **Map check**: Ctrl+D on a map → list mods with warnings. Core is ready: `trade::map_check(mods, avoid)` +
   `MAP_DANGER` list. Needs: detect maps in `price_check` (item_class "Maps"), show mods with red flags in TradeWindow,
   Settings list "map mods I can't run" (default all), ideally suggested from the PoB build (e.g. no regen / reflect).
2. **Trade helper**: watch the game's Client.txt (we already find the log in `core/src/area.rs`) for incoming whispers
   ("@From Name: Hi, I would like to buy your …"), show a small list with buttons Invite / Trade / Kick / Thanks.
   **One chat command per button press** (GGG rule: one action per key press); reuse `reload_in_game`'s typing code.
   Plus optional chat hotkeys (e.g. /hideout), one command each.
3. **Saved & live searches**: save a search from the price window, re-run it; live search pop-up when something lists
   (respect rate limits; price checks take priority).
4. **Stash value / earnings per hour** (Exilence-style, needs the PoE account OAuth or stash API — research first).
5. Price check polish ideas: group resistances "any resistance" (count group), collapse duplicate sellers, show price
   in divines toggle, currency trend chart.
6. Deferred earlier: installer + auto-update, batch mode tidy-up, PoE 2 coverage.

## Known caveats
- This cloud workspace can't reach pathofexile.com / poe.ninja / poeprices.info, so network code is only tested by
  GROBBS in game. Windows-only code (`screen.rs` imp module) can't be compiled here: keep logic in the plain functions
  above it (they have tests) and write the Windows part carefully.
- Trade site User-Agent: `OAuth poe-filter-editor/0.1 (contact: github repo)`; if Cloudflare starts blocking, revisit.
- Test harness: Playwright scripts mocking `window.__TAURI_INTERNALS__` (render `dist/` after `npm run build`); the
  `trade` window is chosen by window label in `src/main.tsx`.
