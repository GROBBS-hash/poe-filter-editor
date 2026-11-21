# Handoff: where the PoE Filter Editor is at

Read this first in a new session. The owner (GROBBS) is non-technical: keep updates short, plain English, no code jargon.
He tests on Windows by running `Start editor.bat`, which pulls from GitHub every 30s and rebuilds (`npm run tauri dev`),
so **commit and push after every change** (end commits with the attribution lines the session gives you).

## Installer and updates (from 21 Nov 2026)
- `tools/release.ps1 -Notes "…"` (PowerShell 7) bumps the patch version, builds the NSIS installer (per-user, no admin),
  signs it with `%USERPROFILE%\.tauri\poe-filter-editor.key` (no password; **never commit it, losing it means installed
  editors can't take updates**), commits "Release x.y.z", and publishes GitHub release `vx.y.z` with the installer and
  `latest.json`. First release: 0.2.0.
- The installed editor (`src-tauri/src/updates.rs`, release builds only) checks
  `releases/latest/download/latest.json` 20s after start and every 3 hours, downloads a newer version in the
  background, and shows "Version x is ready · Restart to update" at the top of the main window (`UpdateBar` in App.tsx).
  It never installs or restarts by itself (he hated pop-ups mid-game).
- Once he has switched to the installed editor: release after each finished change he should get (a few small
  changes can share one release). `Start editor.bat` / `npm run tauri dev` stays the dev way; don't run both at
  once (hotkeys clash, both write the same settings folder).

## What the app is
Tauri 2 + React/TS desktop app with a Rust core crate (`core/`, `filter-core`) for Path of Exile loot filters, plus
in-game overlays. Built on NeverSink's filters (tags `$type->… $tier->…`), user changes stored as rules that survive updates.

## Hotkeys
- **F2** quick edit: the item under the mouse (Ctrl+C in inventory, or reads the ground label + game hover box), shows
  what the filter does, **why** (rule box with "Modify this rule" → opens the main app on that rule), and buttons to change it.
- **F3** batch mode, **F4** reload filter in game, **F5** cycle filters (off by default), **F6** build window (PoB gems/gear/tree).
- **F7** atlas strategy window (`atlas`; Endgame → Atlas picks which one).
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
  instantly; trade site search with listings (price, ilvl, age, seller, Whisper copies the message); (the poeprices.info AI
  estimate was removed 21 Nov 2026: the site is gone, its domain is for sale); bulk exchange rates for currency/scarabs/etc. (pay in chaos or divines); follows the
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
- Awakened ideas NOT done yet (roll bars, timeless jewels, influence/ilvl base search since done): cluster jewel passive
  count rules, flask rules, anointment / heist / sentinel handling, influence + ilvl bracket
  filters, "hidden filters" toggle, roll ranges as min..max for uniques (± % of roll range, exact on perfect rolls).

- Price check "Compared with your build (PoB)" (`core/src/compare.rs`): checked item vs the PoB stage's gear in the
  same slot (both rings), totals for life(+Str/2)/ES/mana/each res/attributes/speeds/suppress/crit multi/spell dmg,
  plus mods only one side has. Collapsible (remembered). 20 Nov: PoB gear keeps only the selected variant's lines and
  rolls "(60-80)" ranges at PoB's {range} (`roll_ranges` in pob.rs); window re-laid out (section headings, "yours /
  this" table with ▲▼, one mod per line folding after four, verdict badge, highlighted minimum preset).
- Price window, 20 Nov (2): "Search trade did nothing" was the window capped at 900px with results below the edge;
  now capped at screen height, the page scrolls (`body.overlay.scroll`) and results scroll into view.
  trade_search / trade_bulk / rare_estimate are `#[tauri::command(async)]` (they used to block the main thread).
  Roll bars: `roll_of` in trade.rs reads the advanced copy's "(25-29)" → `StatFilter.roll` 0..1 (within the tier
  for rares, the whole range for uniques), shown as low / mid / high roll. Settings → Price check: `price_compare`
  and `price_rolls` (both on by default).
- Price window, 20 Nov (3): every Ctrl+D searches trade at once (not only rares). Big price box at the top
  (`PriceHero`: trade typical price, else exchange / poe.ninja; divines when ≥ 1 div). Listings as a table
  (price with chaos value for alch etc., ilvl, listed, seller). Tier badges coloured T1 green → T6+ red; roll line
  shows "T3 range 6–11" (`StatFilter.range`, `range_text` in trade.rs).
- **Preview page (dev tool):** with the editor running (`npm run tauri dev` serves vite on :1420), open
  http://localhost:1420/preview.html. It renders the price window in a browser with mocked Tauri calls
  (`src/preview.tsx`). The app saves every price check and trade result to `%APPDATA%\com.grobbs.poefiltereditor\samples`
  (newest 60, local only); vite.config.ts serves them to the preview's dropdown. Use it to check layout changes
  before asking GROBBS to test in game.
- **F2 pop-up preview (dev tool):** http://localhost:1420/preview-qm.html?state=hide|icon|show renders ItemPanel with a sample
  item (`src/preview-qm.tsx`). If the running editor seems to show an old version after a bad edit, the dev server may
  have kept a stale copy: `touch` the file (seen 20 Nov with ItemPanel.tsx).
- **App preview (dev tool):** http://localhost:1420/preview-app.html renders the main window with sample data
  (`src/preview-app.tsx`, mocks `overview` and `wealth_get`; opens on Wealth). Other pages show their loading state.
- **Clean glass look (20 Nov, he picked it from three mockups):** new colour tokens and card style, gold bar on the
  current menu item, page title icons, page/card fade-in (`.page-in`, keyed on the tab), springy buttons, focus rings,
  shimmering placeholders (`LoadingShell` in App.tsx, `PageLoading` in Loading.tsx), stat tiles (`.tiles`) on Wealth.
  All in the "Clean glass" block at the end of styles.css; motion is off when Windows asks for reduced motion.
  Main window opens at 1240×860 and remembers size/position (tauri-plugin-window-state; pop-ups are on its denylist).
- Map price check, 20 Nov: maps were searched as type "Map (Tier 2)" with every T1 map mod ticked (hidden in the
  window), so nothing matched. Now: type `{option:"Map", discriminator:"map"}` for "Map (Tier N)" bases, `map_tier`
  min=max, rarity filter, no mods ticked; mods behind "Search by map mods too" in the window. Gems: empty ilvl hidden.
  Then, like Awakened (`renderer/src/web/price-check/filters/pseudo/maps.ts`): `map_props` in trade_rules.rs adds
  Item Quantity / Atlas Drops / Pack Size (ticked), Item Rarity (ticked unless a More Currency/Scarabs/Div Cards bonus),
  More Currency/Scarabs/Div Cards (ticked), More Maps (not ticked), "8 mods" (ticked when 8 mod headings). They
  go to `map_filters` (map_iiq/map_bonus/map_iir/map_packsize) or the pseudo map stats. Rarity filter = nonunique.
  Live check: his tier 2 map (78% atlas, 46% rarity, 30% pack) → 24 listed.
- **"Something's wrong?" button** (price window): saves his note plus check/setup/result/error to
  `%APPDATA%\com.grobbs.poefiltereditor\reports\<unix time>.json` (`report_problem`). Read these when he says he
  reported something.
- Wealth page (Endgame → Wealth; `core/src/wealth.rs`, `src-tauri/src/wealth_app.rs`, `src/Wealth.tsx`,
  `src/EarningsWindow.tsx`, window "earnings"): reads the stash via the session API
  (character-window/get-stash-items, Cookie POESESSID, realm pc/poe2, rate-limited as kind "stash", learns
  X-Rate-Limit-Account too). Values with poe.ninja (uniques by name, gems at their cheapest variant, everything else
  by base; rares/gear skipped). Whole-stash scan → snapshot history (wealth.json, newest 400) + chart + by-tab, and
  "Worth selling": unlisted items ≥ N chaos. Earnings sessions: tick dump tabs, name a strategy, start → tabs
  valued now and every N minutes (`start_session_timer`), overlay shows time / made / per hour; pause, stop & save;
  "Best strategies" averages per name. OAuth would need GGG to register the app, hence POESESSID (kept local).
  Not tested against the live site: realm for PoE 2 and the exact error shapes.
  20 Nov: scan profiles like WealthyExile (`ScanProfile` in WealthStore: name + tabs; `active` = None for whole stash;
  `wealth_profile_save/delete/use`, `wealth_tabs_refresh`). Snapshots carry `profile`, so each profile has its own
  total and history. "Worth selling" (`missed`) now adds up the same unlisted item across stacks and tabs before
  comparing with the threshold (`Valued.places`), and the page shows "×10 in 3 places · 1.2c each".
- Price check 20 Nov (4): jewels have max 2 prefixes / 2 suffixes in "Open prefix/suffix"; reversed ranges like
  "29(30-25)% reduced …" count the bigger number as the better roll and show "25–30".
- Wealth scan 20 Nov (2): price file lacked threads (poe.ninja exchange type "Thread"), maps, blighted maps,
  beasts and gems, so they counted as 0. Added to tools/fetch_prices.py (gems cut to each gem's cheapest version).
  Scarabs are gone this league (Covens; threads replaced them). New groups are scan-only (`Group::of` → None) and
  gems are skipped by `price_of` (cheapest version would undersell). Items in a tab named "~price …" count as listed.
- Unique price tiers 20 Nov: Replica and Foulborn uniques don't drop as plain ground uniques, so they no longer set a
  base's value (`drops_on_ground` in prices.rs). Mixed bases (cheapest unique below tier 1, e.g. Headhunter + Wurm's
  Molt on Leather Belt) go no higher than tier 2: no divine sound for every Leather Belt. "Could be one of N uniques"
  list (`BaseUniques.tsx`, command `base_uniques`) in the F2 pop-up and Ctrl+D for uniques without a name.
- Fractured items 20 Nov: fractured mods keep their own row even when a total or the 20%-quality defence counts
  them (unticked then). "Fractured item" box searches base + ilvl+ + only fractured mods (Awakened-style). `StatIndex::find`
  now falls back to "… (Local)" stats (e.g. local % ES on armour). Live: his Fossilised Spirit Shield → 6 listed.
  Item-level bands (`ilvl_band` / `ilvlBand`): 1–67, 68–74, 75–81, 82–83, 84–85, 86+; base searches use the item's
  level up to its band top (`TradeSetup.ilvl_max`). Fractured box is one row: "Fractured · Price as base · ilvl 73–74".
- Related items 20 Nov (Awakened-style): Ctrl+D on a fragment, invitation, boss key, Maven's Writ etc. shows the
  rest of its set with prices and the whole-set total, "made from 10 × Crescent Splinter" (parts vs whole), and the
  boss's drops with poe.ninja prices, dearest first. Data `core/data/related_items.json` from Awakened PoE Trade's
  item-drop.json (MIT), rebuilt with `node tools/build_related_items.mjs`; core `related.rs`, command
  `related_items`, `src/RelatedItems.tsx`. Preview page has "Fragment: …" examples using the real price file.
  20 Nov: merged into main; the panel now sits beside the price window (`.tw-wrap` / `.tw-side`, window widens to fit),
  e.g. lifeforce shows its four types.
  Families (`family` in related.rs): threads, delirium orbs, fossils, resonators, omens, oils, runegrafts, embers,
  astrolabes (poe.ninja category) and catalysts show the rest of their kind, dearest first (max 12). Awakened's
  non-set groups only show when their names share a word (`one_kind`), and then without the mechanic's loot; mixed
  league-mechanic groups (Expedition: Ancient Orb + Astragali + keys) show only drops ("Can reroll a unique into").
  The preview page's mock of `related_items` is a rough JS copy; trust the Rust tests.

- Tiers → **Map tiers** (20 Nov, FilterBlade-style; `src/MapTiers.tsx`, core `tiers::map_tiers`, commands
  `get_map_tiers` / `set_tiers`): "Show maps from tier N and up" (hides NeverSink's `$type->maps $tier->maps_<band>_t<N>`
  blocks below N), one chip per tier to toggle, one look per band (d = T1–5 whites, c = T6–10, b = T11–13, a = T14–16),
  and the special kinds (`maps->influenced/blighted/nightmare/vaaltemple/specialmods/enchanted`) with a tick and "Open".
  All saved as normal Tier rules, so they survive NeverSink updates. Searching "map" on the Tiers page offers it.
  Not done: rarity-specific map hiding (e.g. only hide white maps), which would need an Override block.
  20 Nov (2), after GROBBS found it confusing: panel is now four numbered steps, each with a one-line "what this does
  in game" (1 which tiers show, 2 how each group looks, 3 extra borders, 4 special maps). "Extra borders" =
  NeverSink's Continue decorators (`deco_mapup_*` upgrade border, `deco_zone*` group borders, `decor_implicit`,
  `deco_corruptedmod`) as on/off switches (`MapTiers.extras`; off = Tier rule with an empty look via
  `set_tiers(clear_style)`, on = reset). NeverSink's map rule names are plain words everywhere (`map_rule_name` in
  tiers.rs: "Tier 16 maps", "Upgrade border: tier 13+ maps while you run lower", "Valdo's maps"…), and
  "Modify this rule" on a map opens the Map tiers panel instead of the raw "Maps" rule list.
  FilterBlade's map section couldn't be read (its page only renders in a browser), so wording follows NeverSink's own
  section names in the filter.
- Ground reader 20 Nov: a label stacked right under another (grey "MAP (TIER 4)" under a dark "SPINY ROUND SHIELD")
  was joined into one text and picked the shield. Stacked lines now also need the same background colour
  (`other_label` in screen.rs). The game's hover highlight wasn't used: the little squares at the end of labels in his
  screenshot look like socket colours, not a hover marker.
- Build window (F6) 20 Nov: opens 780×900, Clean glass look (gold heading bar, card header, pill tabs, tree fills the
  window height). "Open with the passive tree (P)" tick box in the window and in Settings (`build_with_tree`,
  `key_tree`): `start_tree_watcher` in lib.rs polls the key with GetAsyncKeyState (does NOT take the key from the
  game), only while the game is in front; opens on the Tree tab without taking focus; P or Esc closes it if it opened
  that way. Enter toggles a "chat may be open" guard so typing P in chat doesn't open it.
  Preview: http://localhost:1420/preview-app.html?build renders the window with a sample build.
- Atlas strategies 20 Nov (Endgame → Atlas; `core/src/atlas.rs`, `src-tauri/src/atlas_app.rs`, `src/Atlas.tsx`,
  `src/AtlasOverlay.tsx`, `src/AtlasTreeView.tsx`; saved in atlas.json in the data folder). Paste a link → one request:
  maxroll farming strategy planner (JSON at `planners.maxroll.gg/profiles/poe/<id>`; each profile × step = one tree,
  picked nodes + `pathing.include` joined to the start with shortest paths avoiding `pathing.exclude`; notes are
  Lexical rich text → lines; map device / threads / map rolls from `metadata.strategy`) or a pathofexile.com atlas
  tree link (`/atlas-skill-tree/<code>`, passive-tree v6 code; checked on the live site: a link we make shows the
  same tree there). Name, rename, delete, "In game" pick, own notes, "Open on pathofexile.com". poe.ninja/poeplanner
  links: not supported (says to use their pathofexile.com export). Window `atlas` (F7, `settings.key_atlas`; a
  clash only skips this key) shows without taking focus (re-focuses the game), Tree / Notes tabs, strategy and tree
  pickers. Tree data: `public/atlas_poe1.json` (3.30.0, rebuild with tools/build_atlas_data.mjs for a new patch).
  Preview: http://localhost:1420/preview-app.html (Atlas page) and `?atlas-window` for the F7 window.
  Dev check of a saved maxroll reply: `ATLAS_SAMPLE=<file> cargo test -p filter-core --lib atlas::sample -- --ignored --nocapture`.
  21 Nov, after his first look ("a good start"): steps like maxroll's "Leveling Path" (`Graph::steps`: points in the
  order taken, a step ends at a notable/keystone or where the next point isn't joined; matches maxroll's list exactly
  for vg1ga30g). `fill_paths` now keeps the picks' order. Strategies saved earlier get steps on first load
  (`add_missing_steps`: maxroll ones re-read once). Arrows step through ("13/105"); the tree shows only points up to
  that step, this step green and named; Steps list on the page (left of the tree) and a Steps tab in F7; the step is
  remembered per tree (localStorage, shared by page and window). Tree decluttered: area names off (button "Area
  names"), no notable names except the current step's, rest of the atlas faint, bigger taken notables, text sized
  to the window.
  21 Nov (2), after testing in game: F7 window opens 780×820 like the build window (same title bar), remembers its
  size and place (`atlas_window.json`, saved half a second after a move/resize via `on_window_event`; restored on
  open and pulled onto a screen). Steps list on the left and the tree on the right in one view (list hidden when the
  window is under 560px wide; arrows stay); Tree / Notes tabs. No scrollbar on the window: the tree is sized to the
  space, lists scroll in their own panel with a thin scrollbar that shows on hover. Preview `?atlas-window` fills the page.
- Windows over the game, 21 Nov ("consistency is key"): one title bar for all (`WinBar` in `src/WinFrame.tsx`: drag
  to move, ✕ hides) in F7 atlas, F6 build, trades list and earnings box; resize grips (`ResizeGrips`: left/right/bottom
  edges and a diagonal-lines grip in the bottom-right corner, `startResizeDragging`, capability
  `core:window:allow-start-resize-dragging`) on F6 and F7. Place memory in one module (`src-tauri/src/win_place.rs`,
  windows.json in the data folder): build + atlas keep place and size, trades + earnings keep place (they size
  themselves). Shown windows go through `win_place::show_over_game` / `restore`. The Ctrl+D price window still places
  itself by the mouse.
- 21 Nov (3): atlas "Options picked" list under the tree (`ChoiceList`: multiple-choice nodes taken up to the step with
  the strategy's pick, this step's highlighted, click → `find` zooms the tree to it), in F7 and on the Atlas page.
  F6 fits its window like F7 (`.bv-panel` under the tabs fills the rest; tree sized to it; lists and masteries scroll
  inside it). Scroll bars everywhere are thin and only show while the mouse is over what scrolls (global
  `::-webkit-scrollbar` rules at the end of styles.css).
- 21 Nov (4): changing atlas step glides the view (0.4 s, `glide` in AtlasTreeView; straight there with reduced motion) to
  fit everything taken so far incl. the new step; the first view fits the steps taken too. Zoom/drag stop a glide.
- Price fixers 20 Nov: item listings count each seller once (`summary` in trade.rs, middle of the cheapest ten
  sellers). Bulk exchange shows a stock-weighted going rate (`bulkTypical` in TradeWindow.tsx), poe.ninja beside it,
  and marks offers well under it with little stock as "low stock" (likely fixers). His Stacked Deck: 3c, not 1c.

- Search like Awakened 20 Nov (checked in its source, CheckedItem.vue): Ctrl+D searches at once only for uniques,
  unidentified items, ground reads and items with no gear category (gems, corpses…); bulk uses the exchange. Rares,
  gear, jewels and maps wait: moving the mouse onto Search from inside the window searches (or click). Changing a
  filter afterwards clears the prices until you search again (`searchKey`). Settings → Price check →
  "Search every item straight away" (`price_instant_all`, off by default) brings back the old behaviour.

- Bulk pricing matched to Awakened 20 Nov (read in its TradeBulk.vue / pathofexile-bulk.ts / TradeListing.vue): the
  headline is poe.ninja's price (in-game Currency Exchange) when it has the item, else our going rate; a gold "market
  price (poe.ninja)" row sits among the offers at that price; "Only sellers with at least <stack> in stock" sends the
  exchange `minimum` (off by default, like Awakened); "fills N" / ⚠ per offer. Item listings: same seller + same price
  in a row shows once with "× N". The preview mocks `trade_bulk` with his deck offers.

- Stat rows in game order 20 Nov: defences/DPS, totals, enchants, implicits, prefixes, suffixes, open-affix rows last
  (`ModLine.affix` / `StatFilter.affix` from the advanced copy's heading; stable sort at the end of `setup` in trade.rs).
  Rows sit under small headings in full words (`rowGroup` in TradeWindow.tsx): Defences/Damage, Totals, Enchant,
  Implicit, Prefixes / Suffixes ("up to 3 on an item", jewels 2), Open slots. (P/S badges tried first; he found them unclear.)
  Bulk fixers: judged against poe.ninja's price when it has one (`bulkTypical(…, market)`), hidden by default with a
  "show" link; our going rate is only shown when poe.ninja has no price.

- Wealth rares 21 Nov: tested live on his account first (stash API fine; limit 30/min, 100/30 min). A scan now keeps
  his identified rare gear and jewels (`wealth::rares_in_tab`, `WealthStore.rares`) with Ctrl+C-style text built from
  the stash JSON (`wealth::item_text`: mods come as `{description, flags}`). "Price N rares" (`wealth_price_rares`)
  runs each through the price check's usual search (`trade::setup` + `query`) and keeps the middle of the cheapest
  sellers (`trade::summary`) per item id in `WealthStore.rare_prices`: 40 per click, waits for room under the trade
  limit and while he price checks by hand (`LAST_MANUAL`). Prices count for 7 days, re-priced after 2. They're added to
  the last scan's total, tabs, "By tab" and "Worth selling" (`add_rares`), marked "rare · trade guess"; later scans and
  earnings sessions use the cached prices. Live check, his TENDY tab: Woe Buckle ≈ 1 div, Maelström chest ≈ 100c,
  most 1–10c, one ring with nothing like it listed = 0. Rares with nothing worth ticking price as "any rare of that
  base". Dev tool: `cargo run -p filter-core --example rarequeries -- tab.json trade_stats_poe1.json`.
  Fix same day: his first try stopped on "Unknown item base type": 3.30 rare blighted maps ("Blighted Map (Tier 2)",
  a "Map Area" line, no "Map Tier") weren't seen as maps, so they went to the trade site as gear and poe.ninja's map
  price was skipped. `is_gear` knows that shape now; and a 400 for one rare is saved as 0 and the run carries on.

- Quiet restarts 21 Nov: `npm run tauri dev` restarts the app whenever the 30s `git pull` brings Rust changes, and
  each restart popped the editor up mid-screen. The main window now starts hidden (`"visible": false`) and
  `win_place::start_main` decides: a heartbeat file (`alive.txt`, "time shown|hidden", every 10s) under 45s old means a
  restart → minimised to the taskbar without focus (`SW_SHOWMINNOACTIVE`), or stays in the tray if it was hidden;
  otherwise it opens as before. Checked on his PC: after a restart the window was minimised and not in front.
  Tray "Open editor" un-minimises too.
  0.2.2: the heartbeat also says "front" (main window focused; written on every focus change and just before
  "Restart to update"): a restart then comes back in front, as he asked. Open behind other windows: quiet as before.

- 21 Nov (0.2.1): Wealth "Count rares in my worth" (`WealthStore.skip_rares`, `wealth_count_rares`; `add_rares` puts
  rares in or out of the last snapshot, last_items and sessions, but always in "Worth selling"). Gear checklist in
  `BuildView.tsx` (F6 + My build): a tick per item, key `slot|name|base`, in localStorage `build.have` (shared by both
  windows via the storage event), "Have N of M" bar with "hide what I have" (`build.hideHave`).

- Wealth thin cards 21 Nov (0.2.15): the stash worth skips cards prices::thin_card() calls hardly traded (his
  Assassin's Favour "2.25c", ~1 sold). The Fletcher stays (~11 sold at 3.3c per poe.ninja).

- Buy base 21 Nov (0.2.14, his idea + guide: reforge clusters at Harvest instead of buying exact ones): cluster
  jewels in F6/My build Jewels get "Buy base" (buy_base_item → trade::cluster_base_query): same base, passive count and
  small-passive enchant, rarity non-unique, item level ≥ the highest notable needs. Notable item levels in
  core/data/cluster_notables.json (301 notables; levels 1/50/68/75) from RePoE mods.min.json via
  tools/build_cluster_levels.mjs; checked against poedb (Inspired Oppression 75, Doryani's Lesson 68, Overshock 50;
  poewiki blocks automated reads). Live: his jewel → 3480 bases from 1c (ilvl 76–85).

- Buy loosens 21 Nov (0.2.13, his large lightning cluster found nothing): (1) the cluster's "N Added Passive Skills are
  Jewel Sockets" line made every search 0 (as enchant or explicit), so buy_query leaves it out for cluster jewels;
  (2) nobody sold all 3 notables: buy_build_item now retries with trade::buy_loosen (enchants / seed stay required,
  the other mods go in a "count" group needing one, two, three fewer) and opens the first with results; the button
  says "any 2 of 3 mods". Live: 8 passives + lightning + any 2 of his 3 notables = 234.

- Thin cards 21 Nov (0.2.12, his The Calling: poe.ninja "5c", 2 sold, 1c listings unsold): fetch_prices.py keeps the
  exchange's volumePrimaryValue as `volume`; PriceItem::sold() = volume / value; thin_card() = card under 50c with
  under 5 sold → values_by_base counts it 0 (worth line, price tiers). Price window shows "Hardly traded: about N sold"
  (PriceCheck.sold). On 21 Nov: 48 of 198 cards priced 5c+ were thin; rare valuable ones (Father's Love) keep their price.

- Ctrl+D one after another 21 Nov (0.2.11): price_check stopped when the game wasn't in front, and the open price
  window had focus. game_back_in_front(): if one of our overlay windows (not "main") is focused, focus_game() and wait
  for it, then copy as usual (the old price window closes on blur, the new one opens).

- Worth line 21 Nov (0.2.10, his Ritual idea): Tiers → Divination cards / Uniques have "Hide … worth under N c"
  (WorthLine.tsx). It's the price settings' hide_below for that group (same box as on the Prices page), and
  price_blocks now writes hide rules even with price tiers off. Cards count a stack at its total worth: hide blocks
  grouped by the stack size that reaches the line ("StackSize < k"). price_count_below gives "hides N of M".

- Polish 5, 21 Nov (0.2.9, his screenshots): F6/F7 were see-through (.bo at 97%: another window showed through);
  .qm and .bo now solid #12151a; gold top line on .bo too; F6 tabs styled like F7's pill row; "level N in PoB" back in
  the normal font; "Ctrl+F" tag next to Find in the menu.

- Tray menu 21 Nov (0.2.8): Open editor · Check for updates · Restart editor · Quit. "Check for updates" checks now
  (updates::from_tray): installs and restarts if there's one, else shows "Up to date (x)" for 6s; the updater renames
  it "Downloading x…" / "Update to x and restart" when one is found in the background (TrayItem in app state).
  Restarts he asks for (tray, update) come back in front (win_place::mark_front).

- Polish 4, 21 Nov (0.2.7, tidy-up): F6 with no build and F7 with no strategy get an "Open My build" / "Open the
  Atlas page" button (open_editor command: shows the main window and emits open-tab to it); card headings 19px in
  the game lettering (an older rule had them at 15.5px). Pages without preview data (Presets, Themes, Prices, My
  edits, Batch, Filters, Levelling gear) weren't looked at in the preview: check them in the real app.

- Polish 3, 21 Nov (0.2.6, feel): main window keys (App.tsx): Ctrl+F = Find with the cursor in its box, Ctrl+1–9 =
  menu pages in order, Alt+← = previous page; each page's scroll position kept (localStorage scroll.<tab>, the window
  scrolls, restored a few times while data loads); Find and Find-in-stash keep their text (usePersist.ts); Wealth
  "also price new rares" (on by default) runs Price rares after a scan; spinners on Wealth buttons; hints on the
  last symbol-only buttons.

- Polish 2, 21 Nov (0.2.5, game look): Fontin isn't installable from GGG (Cloudflare blocks), so --game-font falls
  back to Alegreya SC (npm @fontsource/alegreya-sc, @import at the top of styles.css). Page titles, card headings,
  item names, F6 headings use it; pop-up headers look like the game tooltip (dark band, gold rule); gem pills show the
  gem's picture (GemPic: icons[name] or "name Support").

- Polish 1, 21 Nov (0.2.4, windows over the game): one frame for every pop-up (.qm and .bo share glass, edge, shadow;
  thin gold line on .qm), fade-and-settle each time a window appears (main.tsx: visibilitychange + price-check /
  item-picked / build-shown re-trigger #root.pop-in), Spinner + counting big price (useCountUp) in the price window,
  plain-English errors everywhere (src/errors.ts friendly(): trade 400/5xx, no internet, odd replies).

- Gear and gems 21 Nov (0.2.3, his pick of 3 mock-ups): the Gems tab is gone; Gear shows each item's sockets
  (`SocketStrip`, from its "Sockets: R-G-B B" line) and the gem groups whose PoB slot is that item under it; groups
  with no slot or a slot this stage has no item for (e.g. "Weapon 1 Swap") go under "Other gems". "New gems this
  stage" sits on top. A saved "gems" tab choice opens Gear.

- Find in stash 21 Nov (Wealth page, PoeStash-style "search every tab"): each scan keeps every item of the tabs it
  read, socketed gems too (`wealth::found_in_tab` → `Found`: tab index + name, spot, kind, ilvl, mods text from
  `item_text`, price note, scan value), replacing those tabs' old entries, in its own file `stash_index.json`
  (`StashIndex` in wealth_app.rs; kept out of wealth.json, which is saved on every view). `wealth_find` searches it
  (`wealth::find`: every word in name/kind/tab/mods, "-word" leaves out, name matches first, then worth; first 200).
  Results: tab and spot, worth, mods on click, "Copy" puts `"name"` on the clipboard for the game's stash search.
  Quiet restarts: the 45s window was too short (a rebuild took longer and the editor popped up mid-game); now 30 min.

- Wealth "By tab" card 20 Nov: a scan keeps every priced item (`WealthStore.last_items`, top 3000 by value); the card shows
  a chip per tab (worth + bar), and the picked tab's items grouped by name, most valuable first, listed ones marked.
  Scans from before this only have totals (the card says to scan again).

- One map list 20 Nov: the Maps page's "Mods you can't run" (`map_tool.bad`, poe.re ids) is the only list. Ctrl+D flags map
  mods by matching their wording to the ticked mods (`maps::map_info_bad`, numbers and poe.re's "(20-40)" ranges ignored); `map_avoid` only seeds the
  first default. Settings → Map check is now a note + "Open the Maps page" (window event `goto-tab`). Maps page buttons:
  "Suggest from my build" (`map_suggest`) and "Tick the usual dangers".

- 20 Nov: the main window went blank because a React hook was added after an early return on the Maps page (hooks must
  come before any `return`). Fixed, and every page is now wrapped in `ErrorBoundary` (src/ErrorBoundary.tsx): a crash shows
  the error in that page with "Try again" instead of blanking the window. preview-app mocks `map_tool_get` now.

- Unidentified rares 20 Nov: "Unidentified (Tier N)" counts as unidentified; a name with extra words in front of the base
  ("Fated Composite Ring") is cut down to a base the trade site knows (`ItemTypes.bases` + `fix_query`). Live: 91 listed.

- Bulk 20 Nov: items worth ≥ half a divine on poe.ninja search the exchange paying in divines first, and a chaos search with
  no offers retries in divines (Maven's Chisel of Avarice: 0 chaos sellers, 3 divine). Side panel families now also
  match by name: "Maven's Chisel of …" (and catalysts), in `family` in related.rs.

- F2 pop-up 20 Nov: Show · Hide · Minimap icon only as three buttons, the item's current state lit up (`qm-seg`).
  Show on an item hidden by your own edit deletes that edit (NeverSink's look returns); otherwise it adds a visible look.
  All three only add/change a rule for that item (Class + BaseType, + rarity unless "every rarity"), never a whole tier.
  Notes about older edits whose NeverSink tier is gone ("… is gone in this version") no longer block the reload or
  replace "Saved" (My edits lists them).

- F2 pop-up tidied 20 Nov: order is item, status card, Show · Hide · Minimap, sound, quick highlight, Customise;
  stack sizes/tiers, When, every rarity and auto reload sit under "More options" (remembered, localStorage qm-more).
  Rarity choices only for gear and maps (not currency/fragments/cards/gems/corpses).

- F2 Show on an item NeverSink hides (e.g. single Orb of Unmaking, $tier->t5alchemy is a Hide block in his Red-Maps)
  now uses that hiding block's own look (it keeps its colours), not a plain default look.

- F2 status card "option B" 20 Nov (he picked it from three mockups): pill + label preview, chips (sound, beam, icon /
  "Hidden on the ground"), one "Change this look" / "Change this rule" button (opens your edit or the NeverSink rule).
  The "Shown by NeverSink · Maps (low value) · NeverSink › maps › restex" box is gone (`Why` is no longer used).

- 20 Nov: the item panel in the main window (Find, Tiers… `inGame=false`) is one sheet in two columns (`.qm-sheet`): main
  controls left, stack sizes / When / rarity right, nothing folded; the "More options" fold is only in game.
  preview-qm.html?editor=1 shows that version. Price box: "Your 732 ≈ …" = the hovered stack's worth (`stack` prop on
  PriceHero, from the item's stack size).

- Build items on trade 21 Nov: every gear item, flask, jewel and gem in F6 / My build has a "Price" button (gems: click the
  pill) that opens the normal price window for it (command `price_build_item`; no "compared with your build" box).
  `pob::Gear.copy` is the item written like the game's Ctrl+C (`game_copy`: class from the base, PoB's "Maelstrom" -> the
  game's "Maelström", magic bases found inside the name, implicit/crafted/fractured tags), `gem_copy` for gems. Also hides
  PoB bookkeeping ("Has Alt Variant Two", "Intangibility"); Watcher's Eye keeps every alt variant's mod. Dev check:
  `POB_COPY=1 cargo run -p filter-core --example pobdump -- <pob.txt>` (all 30-odd items of his build read back).

- **Search as a base** 21 Nov (he asked for Awakened's "base item" search; read in its create-presets.ts /
  create-stat-filters.ts / create-item-filters.ts, written fresh as `base_item` in trade.rs): offered for magic/rare
  identified gear and jewels that aren't finished (no crafted mod, quality not exactly 20, not corrupted/mirrored) and
  are worth something to crafters (fractured, synthesised, influenced, cluster jewel, magic jewel, gear ilvl 82+).
  Keeps fractured mods + enchants, implicits when not influenced/fractured, magic items' tier 1-2 mods; same base,
  item level min(ilvl, 86)+ (cluster jewels by bracket 1/50/68/75/84, other jewels none), influence / fractured /
  synthesised switches (`TradeSetup.misc` -> misc_filters). Replaces the old fractured-only box ("Fractured · Price as
  base"). Magic items search their real base (`plain_base`, also `TradeSetup.base` for the "<base> only" label).
  21 Nov (2), after his feedback: Awakened-style tabs "This item" / "As a base <why>" at the top of the stats; the base
  tab swaps in the base search's ticks (your own search is kept for switching back) and shows "Searching for: Stygian
  Vise · item level 85+ · Fractured · +47 to maximum Energy Shield"; no auto search. Fractured / synthesised items get it
  even with a crafted mod or 20% quality (Awakened wouldn't; his fractured Woe Buckle / Entropy Grasp had crafts).
  Fractured mods only search as "fractured" in the base search (`query`: fractured_item switch on); on "This item"
  they search as the normal mod, so any item with that mod counts (he asked, 21 Nov).
  The base tab shows only the rows the base search uses (unknown mods hidden too); "This item" shows them all again.
  " — Unscalable Value" and the zero-width space before granted skills ("Grants Level 20 ​Aspect of the Spider")
  are cut off copied lines (both were "not searchable"). Preview hook:
  `window.previewCheck(check)` on preview.html; `BASE_SETUP=<sample name>` on basecheck prints that sample's setup.
  Live 21 Nov on his saved checks: velour boots 2015, magic dagger 121, magic burgonet 1947. Dev check:
  `cargo run -p filter-core --example basecheck -- <data dir>/trade_stats_poe1.json <data dir>/samples`.

- "Which stat to untick?" 21 Nov (he asked): after a search finds nothing, a button under the price box runs
  `trade_untick_one`: the search again without each ticked stat (and without "<base> only" / item level), one search
  request each, at most 8, stopping early if the trade site's limit is near; lists "Untick <stat> · N for sale", most first;
  click = untick + search. Preview: "Trade: nothing found" mode mocks it.
- Build gems 21 Nov: PoB lists 21/20 gems; level > 20 or quality > 20 is now searched as corrupted (`gem_copy`), no fake
  "Item Level: 1". Live: Lightning Tendrils of Eccentricity 21/20 uncorrupted 0, corrupted 13. (Confirmed in game.)
  Support gems get " Support" on the end (PoB's "Arcane Surge" = trade "Arcane Surge Support"; was "Unknown item base type").
  "Open on the trade site" now shows after every search, also when nothing is listed.
- Timeless jewels 21 Nov (next price-check extra, from Awakened's pseudo/timeless-jewel.ts): the seed line ("Denoted service of
  7000 dekhara in the akhara of Balbala", trade id explicit.pseudo_timeless_jewel_<conqueror>) is ticked and searched exactly
  (min = max); a row "Seed N under any conqueror (…)" (id `timeless_any:<ids>`, `StatIndex::timeless_family`) searches a
  count group needing one of them. −10%/−20% leave seeds alone. Buy from the build also searches the seed. Live with his
  Brutal Restraint 7000 Balbala: 2 listed; any conqueror: 5.
- Price trend 21 Nov (next price-check extra): tools/fetch_prices.py keeps poe.ninja's 7-day sparkline per item (`trend`: %
  change each day from a week ago, `change`: the week's total; exchange "sparkline", stash "sparkLine"). `prices::trend_of`
  (same item as price_of, not gems) -> `PriceCheck.trend` -> "This week ▲/▼ N%" line under the price box (`TrendLine`).
  Shows once the price job has run with the new script (GitHub Actions on push / hourly).
- Tiers menu 21 Nov (he asked): item pictures before every section (`SECTION_ICON` in Tiers.tsx: game icons by name, GGG's
  CDN art for flasks / gold / cards, "×3" / "×6" badges on stacks; else the section's first item). Transmutation and
  augmentation stacks are no longer listed. New group "Before maps" (`tiers::LEVELLING`): campaign currency (+ stacks),
  life / mana / hybrid / utility flasks (named "Small life flask"…), wand and weapon upgrades ("Step N"), first areas,
  Act 1 gear, R-G-B and 4-linked items, other magic items. Steps without item lists come from `section_at`, so every step
  shows (tick = show/hide, look editable); plain names in `levelling_tier_name`. R-G-B is commented out in his Very
  Strict filter, so it doesn't show there. Dev: `tierdump` example (`--json` > public/_tiers.json, git-ignored) feeds the
  app preview's Tiers page; preview-app now loads real icons from the running editor.
- Prefix / suffix limits 21 Nov: `trade_rules::affix_limits` reads implicits like the Cogwork Ring's "-1 Prefix Modifier
  allowed" / "+1 Suffix Modifier allowed" (jewels 2/2, other rares 3/3); open-slot rows and the "Prefixes · up to N on this
  item" headings use it (`TradeSetup.affix_max`). His Dragon Turn: 2/4, so 1 free prefix + 1 free suffix (was "2 free prefixes").
  His Entropy Grasp "none listed" was real: the 5 ticked stats together aren't for sale; without Aspect of the Spider 16.
  Dev tools: `BASE_SETUP=<sample>` on basecheck prints a sample's setup; `queryof` example turns a setup into its search.

- **Buy from your build** 21 Nov (he wants PoB's mods -> trade site in one click): gold "Buy" on each item / jewel in F6
  and My build. After two tries he showed PoEStash's build links and we copied that shape (`trade::buy_query`): exact
  base, rarity rare/magic, every mod the item has as "has this mod" with no numbers (crafted ones on the page but
  unticked), uniques by name (Watcher's Eye "while affected by" mods added), cluster passive counts exact. One search,
  opened in the browser, "N for sale" under the button. No totals, no count groups, no minimums (he found those jank).
  Matching fixes that help Ctrl+D too: armour/weapon mods use the site's "(Local)" stat first, numbers that are part
  of a stat's text ("for 4 seconds"), "-#"/"+#", "a Charge"/"# Charges", plurals, trailing " Skill", word-for-word
  stats (cluster notables), pick-one stats ("id|14" -> `stat_json` option), cluster category `jewel.cluster`, renamed
  uniques cut to the real name (`ItemTypes.uniques`). Eldritch implicits stay as PoB writes them (plain, like PoEStash).
  Live 21 Nov: gloves 1, helmet 1, belt 5, cluster 15; others 0 (PoB guide's ideal mod sets; untick on the page).
  Dev check: `POB_STAGE="Endgame" POB_BUY=<data dir>/trade_stats_poe1.json cargo run -p filter-core --example pobdump -- <pob.txt>`
  (add POB_BUY_DEBUG=1 to see each item's copy).

- Resistances like Awakened 21 Nov (he asked; read in its pseudo/index.ts): elemental total always ticked, chaos total ticked
  (left out when its only source is a crafted chaos mod), fire / cold / lightning totals unticked; on rares every
  resistance mod is folded into them (fire+chaos mods too). The "any resistance, chaos too" row is gone. Buy uses the
  elemental + chaos totals only.

- Price box design 2 (he picked it from five mockups, 20 Nov): with a stack, two tiles "YOUR N" (gold, big) and "EACH",
  both with orb icons; bulk items get "N <item> = 1 [divine]" under them (replaces the old "1 div ≈ …" line lower down).


## Still to check in game (keep this list current)
Everything below was built since GROBBS last tested and has only been checked with sample data. Tick items off
(move them to "Confirmed in game") as he reports back, and add new features here as they land.
Confirmed in game so far: basic Ctrl+D price check (trade search returned listings); 20 Nov: weapon + armour
windows work (totals, DPS/defences, build box in the right slot). He found them cluttered, so they were re-laid out.

**Price check (Ctrl+D)**
- [ ] "min: exact / −10% / −20%" buttons change the results, and the picked one stays highlighted.
- [ ] Your gear's mods in the build box show real numbers, not "(60-80)". (New layout: he said it looks tidier.)
- [ ] "Search trade" on a tall window (e.g. Duskdawn): prices appear and the window scrolls to them.
- [ ] Roll bars and "T3 range 6–11" numbers look right; tier badges coloured. (Roll bars confirmed 20 Nov.)
- [ ] Every Ctrl+D shows a trade price straight away (uniques, corpses, gems too) in the big box at the top.
      (Confirmed 20 Nov for a unique, a unique belt and a gem.)
- [x] (Confirmed 21 Nov.) A search with nothing listed: "Which stat to untick?" lists what each stat left out would find; clicking one searches.
- [x] (Confirmed 21 Nov.) Cogwork Ring: "Prefixes · up to 2", "Suffixes · up to 4", open prefix / suffix counts right.
- [x] (Confirmed 21 Nov: Woe Buckle base search, 3 listed.) "This item / As a base" tabs (Awakened-style): "As a base" shows on fractured / influenced / synthesised items, magic jewels, cluster
      jewels and unfinished gear of item level 82+, says why, and the prices look like Awakened's base search. Not on
      finished items (crafted mod, 20% quality, corrupted). Magic items keep only their tier 1-2 mods.
- [x] Price window rows read in game order (confirmed 20 Nov).
- [ ] Price window: small headings "Prefixes · up to 3 on an item", "Suffixes", "Totals", "Implicit", "Open slots".
- [ ] Bulk: likely price fixers hidden ("N likely price fixers hidden · show"), judged against poe.ninja's price.
- [ ] Rares wait for Search: move the mouse onto Search to search; changing a stat clears the prices. Uniques/currency still instant.
- [ ] Bulk items (e.g. Stacked Deck): big price = poe.ninja (about 3c), market-price row in the list, fixers marked "low stock",
      "Only sellers with at least N in stock" works; repeat item listings show "× N".
- [ ] Related items panel beside the price window: lifeforce, fragments, invitations, boss keys.
- [x] Wealth "By tab" works (confirmed 20 Nov).
- [x] Maps: a mod ticked on the Maps page shows in red on Ctrl+D and the stash search hides those maps (confirmed 20 Nov).
- [ ] Unidentified rares like "Fated Composite Ring" price as "Composite Ring" (no "Unknown item base type").
- [ ] Maven's chisels: side panel lists the other chisels; offers show (paid in divines).
- [x] F2: Show / Hide / Minimap work; Show brings items back with NeverSink's look (confirmed 20 Nov), "More options" fold works (design may change).
- [ ] F2 card: chips and "Change this look" button; no NeverSink jargon box.
- [ ] Main window item panel (Find): one sheet, two columns, nothing folded.
- [x] Price box tiles "YOUR N" / "EACH" with orb icons and "N items = 1 divine" (confirmed 20 Nov).
- [ ] Menu icons: each page has its own colour tile (Find blue, Tiers coral, Wealth gold…), page title matches.
- [ ] New Clean glass look: opens bigger and remembers its size; pages fade in; Wealth tiles look right; nothing broken on other pages.
- [ ] Unidentified unique (e.g. Karui Sceptre): no false tier 1 / divine sound; the "Could be one of N uniques" list shows in F2 and Ctrl+D.
- [ ] Jewel "Open prefix/suffix" never says more than 2 free; reduced-mod rolls (curse effect) show the right high/low.
- [ ] Rare/magic map: priced by tier + drop bonuses (Atlas Drops, Rarity, Pack Size, More Currency…), shown as
      tick boxes; the price looks sensible next to Awakened's. (Tier-only price confirmed 20 Nov.)
- [ ] "Something's wrong?" saves a report (he'll mention it in chat; check the reports folder).
- [ ] Settings → Price check: turning off the build comparison / roll bars hides them.
- [ ] Prices shown in chaos and divines with orb icons; bulk "1 div ≈ N items".
- [ ] Transfigured gem: searches without "Unknown item base type".
- [ ] Foulborn / mutated unique searches correctly.
- [ ] Ctrl+D on an invitation / Shaper or Elder fragment / Maven's Writ: the set (with total), parts and boss drops
      show with prices at the bottom of the window.
- [ ] Save search, Saved searches list (Run / Live / delete); a live search pops up new listings with a Whisper button.

- [x] (Confirmed 21 Nov, matches PoEStash.) Build window (F6) / My build: gold "Buy" opens the trade site in your browser with the item's mods filled in, and
      says how many are for sale ("any 3 of 4 mods" when it had to loosen). Try a rare, a cluster jewel, Watcher's Eye.
- [ ] Ctrl+D on a cluster jewel in game finds listings (it searched the wrong category before 21 Nov).
- [x] (Confirmed 21 Nov: gems incl. 21/20 and supports.) Build window (F6) / My build: "Price" on a gear item, flask or jewel, or a click on a gem, opens the price window for it
      (Duskdawn as Maelström Staff, rares with their mods ready to search). Jewel mods no longer show "Has Alt Variant Two".

- [x] (Confirmed 21 Nov.) Price check on gear with resistances: "total Elemental Resistance" and "total to Chaos Resistance" ticked, fire /
      cold / lightning totals there to tick, no single resistance mods left in the list.

- [ ] Ctrl+D on a timeless jewel (Brutal Restraint): seed line ticked, prices for that exact seed; "any conqueror" row works.
- [ ] Price window: "This week" line with ▲/▼ % under the price for currency, cards, uniques… (after the price file updates).

**Buy base (21 Nov, 0.2.14)**
- [ ] F6 → Jewels → "Buy base" on a cluster jewel: trade site opens with cheap bases at the right item level; the note
      says "N bases · item level 75+ (Inspired Oppression)".

**Buy (21 Nov, 0.2.13)**
- [ ] F6 → Jewels → Buy on the "8 Passive Only" large cluster: the trade site opens with results, button says
      "… for sale · any 2 of 3 mods".

**Thin cards (21 Nov, 0.2.12)**
- [ ] With the 5c card line on: The Calling and other hardly-traded cheap cards are hidden after F4; Ctrl+D on one says
      "Hardly traded: about N sold". Valuable cards still show.

**Ctrl+D again (21 Nov, 0.2.11)**
- [ ] With the price window open, hover another item and press Ctrl+D: it prices the new item straight away.

**Worth line (21 Nov, 0.2.10)**
- [ ] Tiers → Divination cards: tick "Hide cards worth under", set e.g. 5c, F4 in game: cheap single cards hidden,
      a stack worth 5c+ (e.g. Rain of Chaos ×17) still shows; good cards look as before. Same on Uniques.

**Tray menu (21 Nov, 0.2.8)**
- [ ] Tray icon menu: "Restart editor" restarts it and it comes back in front; "Check for updates" says "Up to date (0.2.8)";
      when the next version is out it says "Update to … and restart" and clicking it updates.

**Polish (21 Nov)**
- [ ] 0.2.4: pop-ups (Ctrl+D, F2, F6, F7, trades) fade in quickly and share one look; price window shows a spinner
      while it checks and the big price counts up; errors read in plain English.
- [ ] 0.2.5: headings and item names in the game-style lettering; Ctrl+D / F2 header looks like the game tooltip;
      gem pictures in F6 / My build.
- [ ] 0.2.6: Ctrl+F, Ctrl+1–9, Alt+← in the editor; pages come back scrolled where you left them; Find text stays;
      Wealth scan goes on to price new rares (tick box).
- [ ] 0.2.7: F6 / F7 with nothing loaded show an "Open …" button that brings up the editor on that page; card
      headings readable; every page looks even (say which one looks off).

**Installer (21 Nov, release 0.2.0)**
- [x] (21 Nov: installed, "everything's there".) Still check: hotkeys work in game with the installed editor.
- [ ] When 0.2.1 is out: "Version 0.2.1 is ready · Restart to update" shows at the top; clicking it updates and reopens.
- [x] (Confirmed 21 Nov, "works for now, will refine more".) 0.2.3: F6 / My build "Gear and gems" tab: each item shows its socket colours and links with its gems under it;
      gems not in an item under "Other gems"; clicking a gem still price checks it.
- [ ] 0.2.2: restart (update) while using the editor: it comes back in front; while it was behind other windows: stays on the taskbar.
- [ ] 0.2.1: Wealth "Count rares in my worth" tick box: unticked, the total drops by the rares at once; ticked, back.
- [ ] 0.2.1: Gear checklist (F6 Gear / Jewels tabs and My build): tick items you have, "Have N of M", "hide what I
      have"; a tick in F6 shows on My build too, and stays ticked in other stages using the same item.

**Editor restarts (21 Nov)**
- [ ] When the editor restarts by itself (new version pulled) while you play, it stays on the taskbar instead of
      popping up in the middle of the screen; starting it with "Start editor.bat" still opens it normally.

**Tiers menu (21 Nov)**
- [ ] Pictures next to every section; Transmutation / Augmentation stacks gone.
- [ ] "Before maps" group: hiding a step (e.g. Act 1 boots, or small life flasks) works in game after F4; looks change.

**Maps**
- [ ] Ctrl+D on a map: flags the mods on the "Map check" list; "Suggest from my build" picks sensible mods.
- [ ] Maps page: stash regex highlights the right maps in game; "Find maps like this on trade" opens a working search
      (the stat-line fragments and trade map filter names were never verified).
- [ ] "Copy map regex" from the Ctrl+D map window.

**Trade helper**
- [ ] A trade whisper pops up without stealing focus from the game.
- [ ] Invite / Trade / Thanks / Kick each type exactly one chat line in game; turns green when the buyer arrives.
- [ ] Chat hotkeys (e.g. /hideout) work.

**Map tiers (Tiers → Map tiers, new 20 Nov)**
- [ ] "Show maps from tier N" hides lower white maps on the floor after F4; special maps (blighted etc.) still show.
- [ ] Band look (e.g. make tier 14–16 bigger) shows in game; Reset puts it back.
- [ ] The panel makes sense now (numbered steps with plain explanations); F2 → "Modify this rule" on a map opens it.
- [ ] Unticking "Upgrade borders" drops the red border on higher-tier maps; ticking puts it back.
- [ ] Step 3 "with / without" example labels match what the maps look like in game (colours from NeverSink's
      rules: `MapExtra.border` + `sample_tier`, drawn with the tier's own look).
- [ ] F2 on a map on the ground: shows "Map (Tier 14)", class Maps, the red tier-14 look and NeverSink's tier 14 map
      rule (not the pink "anything left over"); "Modify this rule" opens Tiers → Map tiers. (Confirmed 20 Nov.)
      (`match_base` turns "… MAP (TIER N)" into base "Map (Tier N)"; `ground_item` sets map_tier.)
- [ ] Special maps, F2 (ground) and Ctrl+C: blighted, blight-ravaged, Valdo, Nightmare, Shaper guardian, Vaal Temple,
      influenced (Elder / Conqueror, from Ctrl+C) each show NeverSink's own rule for them and its look; "Modify this
      rule" opens Map tiers with that kind glowing in step 4. Hide/highlight on a blighted map only covers blighted maps.
      How: Item has `blighted`, `uber_blighted`, `zana_memory`, `influences`, `map_kind_known` (`item::map_kind` strips
      "Blighted " / "Blight-ravaged " from the name); evaluate checks BlightedMap / UberBlightedMap / ZanaMemory /
      HasInfluence from them; quick rules on maps add "BlightedMap True/False". Special bases ("Valdo Map"…) come from
      the filter's own BaseType names, keeping the label's tier. Not verified: the exact label text of these maps in
      3.30, the Ctrl+C line for Originator's memories (`Originator's Memories` / "Contains memories of" guessed), and
      unique maps on the ground (read as their base's tier map; the hover box then gives rarity Unique).
      Editing such an edit again in My edits drops its BlightedMap condition (RuleDetail doesn't carry it).
- [x] (Confirmed 20 Nov: F2 shows the Originator rule.) Originator (Zana memory) maps: F2 on one on the ground shows NeverSink's Originator rule (purple on white for
      tier 14+, purple on grey below) instead of the pink "Maps (low value)", and "Modify this rule" opens Map tiers
      with the new "Originator maps" row glowing. That row shows both looks next to the plain map look; click one to
      change it. How: the label only says "Map (Tier N)", so `ground_item` compares the label's colours with the
      filter's look for a Zana map vs a plain map (`label_distance`) and sets `zana_memory` when the Zana look fits
      far better. `map_tiers` splits NeverSink's `maps->influenced` rules into "Originator maps" (zanamap, zanamaplow)
      and the other influenced maps. Ctrl+C: "Originator's Memories" / "Originator Item" lines (guessed, unverified).
- [x] (Confirmed 20 Nov: citadel maps show purple in game.) Citadel (conqueror) maps ("Map contains Drox's Citadel" implicit): purple look in game (NeverSink's own rule, which
      he ships commented out), F2 / Ctrl+C show "maps › implicitmod", "Modify this rule" opens the new "Citadel
      (conqueror) maps" row in Map tiers step 4 (looks clickable, with "plain map" examples; untick hides them).
      How: `filter::enable_commented` switches on NeverSink's `#Show … $type->maps->implicitmod` blocks in place on every
      build (`rules::apply_with_extra`), so tier edits on them work and survive updates. `Item.has_implicit` from the
      copy ("{ Implicit Modifier }" or "(implicit)"); evaluate checks `HasImplicitMod` on maps from it; ground maps use
      the label colours like Originator maps. Lower-tier citadel and lower-tier Originator maps share NeverSink's
      colours exactly, so on the ground those read as Originator (same look in game; Ctrl+C tells them apart).

**Clickable looks (everywhere, 20 Nov)**
- [ ] Every label preview has a gold outline and "✎ edit look" on hover; clicking opens the look editor for that rule:
      Tiers rows, Map tiers groups / border examples / special maps, Prices tiers, My edits, the F2 / Find item panel,
      Themes (opens the theme's colour editor; NeverSink's and built-in themes make a copy first).
      Shared `LookButton` in StyleEditor.tsx. Saved as normal edits (survive NeverSink updates).

**Build window (F6, new look 20 Nov)**
- [ ] Opens bigger and easier to read; the tree fills the window.
- [ ] With "Open with the passive tree (P)" ticked: pressing P in game opens it on the Tree tab and the game's tree
      opens too; P or Esc closes it; typing a P in chat doesn't open it.
- [ ] Jewels tab (new): his 10 tree jewels (clusters with their notables, Watcher's Eye, Brutal Restraint, Cobalts) and
      the belt's abyss jewel, click one for its mods; "on tree" centres and lights up its socket; jewel sockets have a
      ring on the tree (hover = jewel name, click = open it in the list). Gear tab lists flasks under "Flasks".
      How: `pob::Jewel` from each Spec's `<Sockets><Socket nodeId itemId>` (items looked up by id) and the item set's
      "… Abyssal Socket N" slots; `Stage.jewels`. Checked against his pob.txt with
      `cargo run -p filter-core --example pobdump -- %APPDATA%\com.grobbs.poefiltereditor\pob.txt`: matches his PoB.
      Cluster jewels' own small passives aren't drawn on the tree (PoB's generated nodes aren't in the tree data).
      Gems: every skill group parses (30 gems in "Endgame Whispers", incl. item-granted Herald of Purity/RF);
      if some still look missing in F6, ask which stage and which gems.

**Atlas strategies (Endgame → Atlas, F7, new 20 Nov)**
- [ ] Import a maxroll farming strategy link and a pathofexile.com atlas tree link; both show the tree on the page.
- [ ] Rename / delete / "In game" pick work and are still there after restarting the editor.
- [ ] F7 in game shows the window without taking the game's focus (you can keep moving); F7 again hides it.
- [ ] The window's Tree / Notes tabs, tree picker, dragging it, and resizing it from its edges.
- [ ] The tree matches the site's (points count and the big notables), and "Open on pathofexile.com" shows the same tree.
- [ ] 21 Nov: steps: ‹ › arrows walk through the tree like maxroll's Leveling Path (same order and counts), the Steps
      tab jumps to a step, and F7 reopens on the step you were on. Tree is readable at the window's size (no area names
      unless "Area names" is on). His already-saved strategy gets its steps by itself after the editor restarts.
      (Steps confirmed working 21 Nov.)
- [ ] 21 Nov (2): F7 opens big enough (like F6), steps list beside the tree, no scrollbar on the window's edge;
      move and resize it, hide and reopen (and restart the editor): it comes back in the same place and size.
      (Size memory confirmed 21 Nov.)
- [ ] 21 Nov: corner grip (bottom right) resizes F6 and F7 both ways at once; left/right/bottom edges resize too.
      F6 now remembers its place and size like F7. Trades list and earnings box: same title bar, and they come back
      where you dragged them.
- [ ] 21 Nov (3): F7 lists "Options picked" under the tree (e.g. Blended Realities → its option), click one zooms to it.
      F6 has no scroll bar down its side (tree fills the window; masteries scroll inside). Scroll bars in the price
      window, F2 pop-up, trades list and main window are thin and only show on mouse-over.
- [ ] 21 Nov (4): stepping (arrows or clicking a step) smoothly moves the tree so the new step's green points are on screen.

**Ground reader (F2)**
- [ ] 20 Nov: two labels on top of each other (e.g. a map right under a shield): F2 on the lower one picks it, not the upper one.
- [ ] 19 Nov fixes: our own pop-ups no longer get read as the item; names missing a letter are still found;
      the "rarity unsure" note shows on NeverSink tiered gear.

**Wealth page (Endgame → Wealth)**
- [x] Connected with account name + POESESSID (20 Nov). Still check: clear message if the POESESSID is wrong.
- [ ] Scan whole stash: total, by-tab breakdown and history chart look right; "Worth selling" lists unlisted items.
- [x] (Confirmed 21 Nov.) Find in stash (after a fresh scan): typing finds items in any tab (names, "rare", mods like
      "maximum life", "-unique"); tab and spot right; click shows mods; Copy + Ctrl+F in the stash lights it up.
- [x] (Confirmed 21 Nov.) After a scan, "Price N rares" prices his rares (about 10 a minute, progress shown); the total goes up,
      rares show "rare · trade guess" in By tab / Worth selling, and the prices look sensible next to Ctrl+D.
- [x] Scan profiles: made one, scanned it, own total and history (confirmed 20 Nov). Still check: switching back to Whole stash.
- [x] "Worth selling" adds up stacks; threads and gems now priced (confirmed 20 Nov: 50 Blight threads = 133c).
- [ ] Earnings session: tick dump tabs, start, overlay shows time / made / per hour, pause, stop & save,
      "Best strategies" ranks runs.

## Left to do (agreed order)
1. **GROBBS to work through "Still to check in game" above**; fix what he reports.
2. Wealth tracker follow-ups (real account tested and rares priced via the trade site, 21 Nov):
   "search every tab" done 21 Nov; guild stash next; crafting planner (parked).
3. Price check polish ideas (any resistance, trend line done 21 Nov; same seller + price already shows once with "× N";
   prices already in chaos and divines; maps already search by tier).
4. Deferred earlier: batch mode tidy-up, PoE 2 coverage. (Installer + auto-update done 21 Nov: see "Installer and updates".)

## Known caveats
- This cloud workspace can't reach pathofexile.com / poe.ninja / poeprices.info, so network code is only tested by
  GROBBS in game. Windows-only code (`screen.rs` imp module) can't be compiled here: keep logic in the plain functions
  above it (they have tests) and write the Windows part carefully.
- Trade site User-Agent: `OAuth poe-filter-editor/0.1 (contact: github repo)`; if Cloudflare starts blocking, revisit.
- Linux type-check of the app: install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev, then `cargo check -p poe-filter-editor`.
- Test harness: Playwright scripts mocking `window.__TAURI_INTERNALS__` (render `dist/` after `npm run build`); the
  `trade` window is chosen by window label in `src/main.tsx`.
