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

## Name and icon (21 Nov 2026)
The app is now called **PoE Lodestone** on screen: window title, gold title bar (gem symbol in `MainTitleBar`),
sidebar, tray tooltip, page title. New icon: `app-icon.svg` (gold lodestone + compass needle); regenerate every size
with `npx tauri icon app-icon.svg`. **Deliberately unchanged:** `productName` ("PoE Filter Editor") and `identifier`
in tauri.conf.json, so updates keep installing over the same program and settings stay in the same folder. Changing
productName makes the NSIS installer put a second copy beside the old one; switch it later together with a one-time
fresh install (uninstall old, install new), and tell GROBBS first. The repo/folder name also stays.

## Design notes (impeccable plugin, from 21 Nov 2026)
GROBBS uses the `impeccable` design plugin for UI polish. `PRODUCT.md` (repo root) holds the product facts it reads:
public release planned, "one app for everything", PoE 1 and PoE 2. No DESIGN.md yet (`/impeccable document` would
record the current look).

## What the app is
Tauri 2 + React/TS desktop app with a Rust core crate (`core/`, `filter-core`) for Path of Exile loot filters, plus
in-game overlays. Built on NeverSink's filters (tags `$type->… $tier->…`), user changes stored as rules that survive updates.

## Hotkeys
- **F2** quick edit: the item under the mouse (Ctrl+C in inventory, or reads the ground label + game hover box), shows
  what the filter does, **why** (rule box with "Modify this rule" → opens the main app on that rule), and buttons to change it.
- **F3** batch mode, **F4** reload filter in game, **F5** cycle filters (off by default), **F6** build window (PoB gems/gear/tree).
- **F7** atlas strategy window (`atlas`; Endgame → Atlas picks which one).
- **Ctrl+D** price check (new, own window `trade`). Changeable in Settings; skipped quietly if another app owns it.
- **Play windows (22 Nov):** **F8** levelling route, **F9** map tracker + XP/h + stopwatch, **F10** saved searches,
  **F11** cheat sheets (Syndicate, blight oils + his pictures), notes (no key by default), **Alt+W** wiki page of the hovered
  item, **Alt+R** read the altar / remnant / reward screen under the mouse.
  Like the atlas key, a clash or a key another program owns skips just that key with a warning. Clear a key with ✕.

## Done (recent sessions)
- **Map tracker: timer stops in menus, loot per map, div per hour (22 Nov, not released yet).** After his 0.2.41 test.
  - **Pause:** the game doesn't log the Escape menu or skill tree, so (like Exile UI's "screen checks") the editor
    samples ~110 dots of the game's bottom bar once a second (only inside a map, only with the game in front;
    `screen::sample_points`, one tiny BitBlt) and compares them with a picture saved by **Settings → Map tracker →
    Set up**. 55%+ of the dots changed = covered (skill tree, or the Escape menu darkening the screen); optional
    second picture of the Escape menu's middle (`centre_points`, matched within 20%). Two readings in a row before it
    flips (`journey::Steady`). `MapTracker::set_paused` banks the visit up to the pause; `MapRun.paused` = seconds
    stood still (shown as "+m:ss paused" in the history). A zone load ends a pause. On/off in Settings
    (`PlayStore.pause_off`, `menu: MenuLook`); a new game window size needs Set up again. Tests `pauses_in_menus`, `spots_menus`.
  - **Loot:** `core/src/loot.rs`. The app reads the character's items (`trade_api::character_items`,
    get-items, own rate kind "items": 30/60s, 100/30min; get-characters now kind "chars": 10/60s, 50/30min) ~4 s
    after each map entry and exit, checking `character.lastLoginTime` (= the site's save at the zone change) and
    retrying. `loot::gained` diffs the bags (stackables by count, the rest by item id; used-up things ignored),
    `kind` sorts (currency, cards, uniques, maps, scarabs, fragments, gems, gear, other), priced with the Wealth
    page's lookup (`wealth_app::with_lookup`), unique tier by worth (T1 ≥ 1 div, T2 ≥ 10c). Stored per run
    (`MapRun.loot`, merged across portal visits, max 60 lines). One worker thread runs the reads in order. Settings:
    on/off, kinds shown, list-from N chaos (`LootPrefs`). Done pressed inside a map counts the bags as of the last
    zone change. Tests `finds_what_you_picked_up`, `kinds_and_tiers`; checked against his live bags.
  - **Look C** (he picked it from three mock-ups): tracker shows the total (chaos + div), one colour bar by kind, the
    top 4 drops with pictures (the biggest glows), "+ N smaller drops", and a session line with **div per hour**
    (his ask: session loot / time from the first counted map's start to the last one's end, or now mid-map;
    `SessionSummary.loot_secs`). Play tools history: mini bar, loot, best drop; click a map for everything by kind.
  - **Fixes from his screenshot:** XP per hour always showed 0: pathofexile.com no longer sends experience
    (checked live), so it's gone; the tracker shows "name · level N" (get-characters every 10 min while open, picks
    the log's character or the latest `lastLoginTime`). The session said 1 map while Today said 2: an editor restart
    started a new session; `journey::session_start` now carries on through runs less than 30 min apart (test `session_carries_on`).
- **Crafting jewels and cluster jewels (22 Nov, not released yet).** After his 0.2.39 test ("crafting is important for
  clusters especially"). `tools/build_craft_data.mjs` now keeps normal jewels (Cobalt, Crimson, Viridian), abyss jewels
  (Murderous / Searching / Hypnotic / Ghastly Eye) and cluster jewels: one crafting base per enchant
  ("Large Cluster Jewel: Lightning Damage", 55 of them, the enchant's tag added, so each notable has the game's weight
  for that enchant). Mods carry their domain (`dom`): jewel mods only roll on jewels and the other way round (the
  base's tags get a "dom:…" tag, `weight()` checks it). Fossils carry `only` (Hollow / Glyphic). Prismatic and
  Timeless are unique-only, Assembled Eye left out.
  - Engine (`core/src/craft.rs`, `routes.rs`): jewels hold 2 prefixes + 2 suffixes, a rare gets 3 mods (2 in 3) or
    4 (1 in 3), checked against Craft of Exile on his Large lightning cluster (Chaos Orb → Thunderstruck 5.2% there,
    5.05% here; with Widespread Destruction 0.41% / 0.42%; weights 364 / 727 identical). No influences, essences,
    bench crafts, bench locks, fracturing or Eldritch on jewels; Harvest reforges, fossils (CoE lists them for
    clusters too), Alteration / Augmentation / Regal / Exalted are. New route for any item: "Alteration, then Regal
    for the last mod" (the Regal rolls the third mod; the usual 3-notable cluster craft). When a slam on a jewel misses
    there's no fixing it, so "Exalted Orb" there is priced as "start again from the top". `craft::start` reads a
    cluster's enchant line (or PoB's "Cluster Jewel Skill:") → its crafting base, and "Adds N Passive Skills".
    `plan_item_text` writes the real base + enchant for "buy instead".
  - Crafting page: influences hidden for jewels, "x/2 prefixes", the card shows the cluster's enchant and passive
    count, and a "Start from a Large Cluster Jewel with …, item level N+" box above the columns (the route's first step
    says the same). Build window: Craft button on every non-unique jewel (Timeless / Prismatic still hidden).
  - Tests: `jewel_rules`, `cluster_from_his_build`, `three_notable_cluster`, `jewel_alt_regal`.
  - Fix after his 0.2.40 test (Cobalt Jewel, lightning cast speed + energy shield, both prefixes: the Plan said Chaos
    Orb was cheapest). The Plan box only offered Alteration for at most 1 prefix + 1 suffix. Now `magic_then_regal`
    tries every split of 2–3 planned mods into "on the magic item" (Alteration, or Alteration + Augmentation) and "from
    the Regal", pricing a miss as Scouring + Transmutation and a fresh start; plus a "+ Exalted Orb" version (the Regal
    lands on the other side, so an Exalt still has a go). Harvest reforges in the Plan box and the single-mod list are
    now priced (lifeforce, same table as the routes). The "Alteration, then Regal" route got the Exalt follow-up too.
    With poe.ninja prices that case reads: Harvest reforge Defence ~150–190c, Alteration + Regal + Exalt ~600c,
    Alteration + Regal ~890c, Chaos ~950c. It's really that dear because a Cobalt jewel can roll ~45 prefixes (about 55
    Alterations, then a 1-in-73 Regal). Fossil prices checked live: Dense Fossil 14.31c on poe.ninja (the app showed
    13.9–14.9c), so the lookup is right. Test `jewel_two_prefixes_alt_regal`.
- **Play tools (22 Nov, from the "what do players want" research: Exile UI, Exiled Exchange 2, PoE Overlay, Path of
  Leveling, Reddit/forum asks).** Built on branch `ccr-26676017-7ecilq` while he was out; merged and released in 0.2.39 (22 Nov).
  Core `core/src/journey.rs` (tested), app `src-tauri/src/play_app.rs`, front end `src/Play.tsx` + `src/play.css`,
  main-window page "Play tools" (Your character group), windows `level`, `tracker`, `searches`, `sheet`, `notes`.
  - The Client.txt watcher (`start_whisper_watcher`) now always follows the log and hands every line to
    `play_app::on_line`; trade pop-ups still only when the trade helper is on. On finding the log, `play_app::prime`
    reads the last 4 MB for current zone + character. Live events are stamped with real time (the log's clock is local).
  - **Levelling (F8):** route/zones/quest rewards from HeartofPhos/exile-leveling (MIT), rebuilt with
    `node tools/build_levelling.mjs <exile-leveling clone>` → `core/data/levelling.json`. Step pointer per character
    (play.json `level_pos`), moves on "Generating level N area <id>" (enter/waypoint to that zone, logout = act town;
    looks 30 steps ahead, 12 behind). Options: league start, bandits, library, follow on/off, jump to act (Play tools
    page). Shows quest gems your PoB build wants for your class ("Take X as your reward" / "Buy X from Nessa"), zone
    level vs yours with PoE 1's XP penalty, and a vendor search of your build's gems (shortest unique bits, 250 chars).
    PoE 1 only (the route is PoE 1's campaign).
  - **Map tracker (F9):** `MapTracker` counts runs from zone loads (map areas start "Map", same seed = same map, portals
    back in counted, hideout time not counted), deaths of your character, Done button. A Ctrl+D map check in the
    10 minutes before attaches its mods. With an earnings session running, `made` = session value change over the run
    (rough: sessions check every few minutes). History (500) in play.json; table on the Play tools page.
    ~~XP per hour~~ (removed 22 Nov: the site stopped sending experience; see the newer entry above). Stopwatch with laps.
  - **Searches (F10):** saved searches (play.json, editable on the page; defaults: movement speed, 3-links, RGB,
    life/res, quality gems) plus "made for you" (build gems, Maps page regex). Click = focus game, Ctrl+F, Ctrl+A,
    Ctrl+V (pasted, not typed, so stray letters never hit skill keys; clipboard restored). Wealth → Worth selling has
    "Highlight in stash" (`stash_highlight`, item names, 250-char cap, most valuable first).
  - **Cheat sheets (F11):** Syndicate table from Lailloken/Exile-UI Betrayal.json (MIT, data dated 12 Nov 2026),
    rebuilt with `node tools/build_syndicate.mjs <Betrayal.json>` → `src/data/syndicate.json`; click cells to mark
    picks (localStorage), find box dims the rest. Plus his own pictures: added on the page, copied to the data
    folder's `cheatsheets`, shown as tabs.
  - **Notes** window (notes.txt, saved as you type) and **wiki key** (Ctrl+C the hovered item → poewiki.net /
    poe2wiki.net, uniques by name, else base).
  - Checked here: cargo check of the app (Linux), all Rust tests, tsc, and screenshots of every window with sample data.
- **Play windows design pass (22 Nov, after 0.2.39):** impeccable polish of all play windows; cheat sheets in look A
  (`REWARD_ART` in Play.tsx maps Syndicate rewards to poe.ninja icon names); blight oils list recipes using any ticked
  oil, best match first; play windows scroll instead of clipping (`.bo.pw > .bo-body`). Browser preview of every play
  window with sample data: `preview-app.html?play=level|tracker|searches|sheet|notes|choice` (`src/preview-play.ts`).
  Jewel / cluster crafting is being built in a separate session (he asked for a Craft button on jewels, clusters most).
- **QoL wave 2 (22 Nov, same branch, from "be the player" review; his picks):**
  - **Crafting search** (`core/src/craft/search.rs`, command `craft_search`, `CraftSearchBox` in the Plan card): planned
    mods -> stash regex, "this tier or better" = at least that tier's lowest roll (`at_least`, up to 4 digits), words cut to
    the shortest start no other mod on the base shares; any / all; Copy / Put it in game / Add to my searches (F10).
    Limit stated in the UI: searches read numbers, not tiers.
  - **Crafting verdict** (`Verdict` in RouteSummary: Easy / Worth a try / Pricey / Usually not worth, from the cheapest
    route's cost in divines and risk) and **buy price for planned items** with no copied item (`craft_buy_plan`: a
    made-up copy from `plan_item_text`, every non-junk stat ticked at its value).
  - **Trades pop-up:** "Underpriced? / Fair / Good sale" vs poe.ninja (`whispers::check_price`, `trade_price_check`);
    **Show in stash** draws a pulsing box on the whisper's cell (`stashbox` window, click-through, 20s; `stash_cell`:
    grid starts 16/1080 x 161/1080 of the game height, cell 52.8/1080, quad = half; past 12 = quad; "(quad)" button).
    The geometry is a best guess: CHECK IN GAME.
  - **Map check verdict:** big "Safe to run / Don't run" + how to reroll (Chaos on rare, Alteration on magic, corrupted = put aside).
  - **Session summary** (`SessionSummary` in tracker view: new session after 30 quiet minutes; maps, made, XP gained,
    levels, deaths, best map) in the tracker window and on Play tools.
  - **Levelling:** trials count per lab ("Normal trials 4/6", from the route's trial steps vs your position), "Trial of
    Ascendancy in this zone", Today's lab link (poelab.com) once a lab's trials are done, res -30/-60 reminder, a note
    on PoE 2 that the route is PoE 1's.
  - **Wealth:** "Check your prices" (listed items under 70% / over 150% of poe.ninja; `Valued.note` now kept),
    "Divination cards" (sets from `Valued.max_stack`, all tabs), **price alerts** (play.json `alerts`, checked after each
    hourly price fetch by `check_alerts`, shown once per crossing in the trades pop-up).
  - **Read a choice (Alt+R):** `screen::read_lines_near_mouse` (Windows OCR of a 1500x1000 box, NOT testable here) ->
    `core/src/choices.rs` (altar headings split top/bottom, rewards scored by words, dangers = MAP_DANGER filtered by his
    can't-run list and outweigh rewards; remnants; reward screens priced by exact poe.ninja names) -> `choice` window.
  - **Play windows hide in maps** (level, searches, sheet, notes; back on leaving; play.json `keep_in_maps` = off switch on Play tools).
  - **Blight oils** (`src/data/anoints.json` from Exile UI, MIT, `node tools/build_anoints.mjs`): search by notable or
    stat, tick oils you have, recipe cost from poe.ninja. Tab in F11 and a card on Play tools.
  - **Currency exchange check** on Play tools (`price_names`): your rate vs poe.ninja.
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
- Awakened ideas, done 21 Nov (all checked against the live trade site, Covens):
  - Uniques: every mod that rolls is ticked, minimum 10% of its roll range below yours ("+90(80-100)" -> 88), exactly
    yours on a perfect roll; fixed mods and plain copies stay unticked. `ModLine.width` = roll-range width.
    Uniques also search "not corrupted" / "corrupted" like yours (`TradeSetup.corrupted`, a switch under "Also searching for").
  - Anoints: "Allocates X (enchant)" on amulets and rings is ticked (stat list has them as `enchant.stat_2954116742|<node>`).
    The line has no number, so `mods()` keeps lines starting "Allocates ".
  - Cluster jewels (non-unique): "Adds # Passive Skills" ticked and exact (`StatFilter.max` = min) and the small-passive
    "grant" enchant ticked; the jewel-socket enchant stays off (asking for it finds nothing). Live: Behemoth Wound -> 27.
  - Flasks / tinctures: always this base (a magic flask used to search any flask!); without tiers every positive mod
    counts; downsides ("66% reduced Amount Recovered") are not ticked.
  - Heist: categories for heist gear/tools/cloaks/brooches, contracts, blueprints; `trade_rules::heist_props` adds
    area level (ticked), a blueprint's total wings (ticked), revealed wings / escape routes / reward rooms and job levels
    (to tick). No item-level filter on contracts. Live: a Bunker blueprint, area 83, 4 wings -> 301.
  - Sentinels: skipped. The trade site has no Sentinel category any more (league mechanic gone).
  - Influences: the trade site dropped its "Shaper Item" switches; they're "Has Shaper Influence" pseudo stats now.
    Each influence is a row under "Influence" (unticked); "As a base" ticks it (the old `shaper_item` misc filter was a
    silent bug). Item level was already a tick box beside "<base> only".
  - "Also searching for" fold under the Search button (`trade_hidden` command, `trade::describe`): instant buyout,
    name/base, rarity, kind of item, item level, corrupted, links, map tier, in plain words. Open/closed is remembered.

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

- Crafted chaos 21 Nov (0.2.18, his Demon Cowl): a crafted "+15% to Fire and Chaos Resistances" was counted into the
  elemental total and hidden, and the chaos total was skipped (bench craft only), so chaos vanished. Now the chaos
  total shows, unticked (trade_rules: bench_only).

- Crafting routes 21 Nov (0.2.32, replaced 0.2.31's single `recipe()`): `core/src/craft/routes.rs` `routes()` builds
  several whole routes for the plan and prices each (avg chaos, clicks, risk 0/1/2, "check in game" notes, badges
  Cheapest / Fewest clicks / Safest): spam it all; roll one side → bench lock → Harvest-reforge the other side (3.30
  locks: 1 Div a mod, 2 for 2; full side lock 4 Div + 1 Sacred weapon / 4 Blessed armour+quiver / 8 Exalted jewellery);
  magic first (Alteration or Alt+Aug, Regal); roll the hardest then add the rest; essence first; fracture the hardest
  (Fracturing Orb ≈ 1 in 4.3 on a 4–6 mod item); Eldritch (one implicit makes its side dominant, Eldritch Chaos rerolls
  that side 1–3 mods: a guess, flagged); keep what the copied item has. Key idea: a reroll step only counts when it
  leaves room for later steps (`Cfg.cap`). Adding one mod: Exalt + annul (dropped when annul loses more than it gains),
  bench filler on the other side, Cannot roll Attack/Caster, Eldritch Exalt, lock + exalt + scour. Harvest priced from
  lifeforce (Maxroll 3.24 amounts). Chaos/essences/fossils are assumed not to work with a lock on (Harvest and Scouring
  do). Mod groups now have one stable number (`group_id`) so fossil pools and kept mods line up. Commands
  `craft_routes` (+ the item's lines) and `craft_buy` (Ctrl+D's trade search on the copied item). UI: `RouteSummary`
  (left column) + `RoutesPanel` (wide, over columns 2–3) in Craft.tsx, styles in craft-recipe.css. Dev tool:
  `cargo run --release -p filter-core --example routesof -- "Paladin Boots" 86 "Movement Speed:2" "to maximum Life:5"`
  (CRAFT_JSON=public writes the preview files; preview-app.html mocks `craft_routes` / `craft_buy`).
  Research notes: Craft of Exile only works out one method at a time (plus a fossil optimiser); it has no route
  planner, so the routes are ours. poewiki blocks automated reads.

- Local stats 21 Nov (0.2.30): armour/weapons tried the site's "(Local)" stat first for every mod, so ES on a staff
  (Xirgil's Crank) and attack speed on gloves searched stats those items can't have. `local_fits` (trade.rs): weapons use
  local only for non-defence mods, armour only for non-weapon (damage/speed/crit/accuracy) mods.

- Flask quality + jewels 21 Nov (0.2.28): `prop.quality` row (trade_rules `property_filters`) when quality > 20 (not gems or
  maps), ticked on flasks/tinctures, searched as misc_filters.quality. Tiers `KNOWN` lists NeverSink's jewels->generic /
  abyss / cluster / clustereco / special / leveling (only the ones the filter has show).

- Corrupted only 21 Nov (0.2.27): `MatchOpts.corrupted` / `RuleEdit.corrupted` / `RuleDetail.corrupted` (rules.rs) add "Corrupted True"
  and a "Corrupted" label prefix; a corrupted-only edit has its own slot and doesn't shadow plain ones. F2 panel offers "Only
  corrupted ones" when the item is corrupted. Double title bar: tauri-plugin-window-state restored "decorated: true" from
  .window-state.json; now it keeps only size, position and maximised (lib.rs).

- Frame + fixes 21 Nov (0.2.24): main window has no Windows frame (`"decorations": false`), `MainTitleBar` in
  WinFrame.tsx (gold-trimmed, drag / double-click maximise, min / max / close; close still hides to the tray) and a
  thin gold border (`body:has(.mt-bar)`); needs window permissions minimize / toggle-maximize / is-maximized / close.
  Compare look was empty: the trade site now sends listing mods as `{ description, … }` objects, not strings
  (trade.rs `listings` reads both). Settings look picker shows a small drawing of each look (`LookPreview`). Crafting:
  item card shows the base picture; mods marked fractured (Fracturing Orb icon + note) or crafted (⚒), carried as
  "prefix|fractured|text"; bench-only mods on the item are found by whole wording (clickable); mods outside the pool
  get a "how to get it" note (`how_to`: beastcraft for "Aspect of the …", bench, veiled/delve otherwise).

- Looks + sidebar 21 Nov (0.2.23): price window has 5 looks (`TW_STYLES` in TradeWindow.tsx, localStorage `tw.style`,
  shared with Settings → Price check "Look of the price window"; "Style: … ›" at the bottom cycles): classic,
  tooltip (game-tooltip header, price moved down above Search), split (`.tw-market` side panel: hero, trend,
  `MarketStats`, `PriceSpread`), compare (each listing's mods vs yours: `ListingMods`, trade.rs `Listing.mods` from
  implicit/fractured/explicit/crafted), price first (hero + spread above the item). Sidebar: group headings gold with a
  rule; in-game keys from Settings beside their pages (gold chips: My edits F2, Batch F3, My build F6, Atlas F7,
  Prices Ctrl+D, Filters F4), Ctrl+N shows on hover; footer links to Settings for changing hotkeys.

- Crafting page 21 Nov (0.2.20): new "Crafting" page (Endgame menu) + "Craft this" link in the Ctrl+D window (gear,
  not uniques; passes the item through localStorage `craft.item`, opens the editor on the page). Data:
  `core/data/poe1_craft.json` from RePoE via `node tools/build_craft_data.mjs` (bases with tags, prefix/suffix mods
  with spawn weights, essences, fossils, bench crafts). `core/src/craft.rs`: `pool()` = every mod family on a base at
  an item level (+ influences, which add "<tag>_shaper" etc.), tiers, how often one Chaos Orb gives it (20k simulated
  rolls: 4/5/6 mods at 8/3/1 in 12, max 3+3, one per group, by weight), essence / bench / fossil options, which ones
  the item has; `ways()` = Chaos Orb, essences, bench, best fossils (single + pairs) for "tier N or better", with
  poe.ninja cost. Bench/essence-only mods are folded away; on armour pieces, bench/essence mods for a defence the base
  lacks are hidden. "Open in Craft of Exile" copies the item and opens their site (they have no import link).
  Dev tool: `cargo run -p filter-core --example craftof -- "Eternal Burgonet" 85 "maximum Life"` (CRAFT_JSON=public
  saves files for the page preview).
  0.2.21: `ways()` takes the item's mods + rarity and groups methods ("Guaranteed" / "Reroll" / "Add a mod"): adds
  Orb of Alteration (magic: 1 or 2 mods, even split, 1+1), Harvest reforge per mod tag (first mod forced among that
  tag), Exalted Orb on a rare with a free slot (exact: weight share of what can still go on), Augmentation / Regal on
  a magic item; with no item, a rough Exalted. Item lines carry "prefix|"/"suffix|" (Item Rarity is both) and
  two-line mods (Mammoth's) count once. "More ways" panel lists what isn't worked out (veiled, conqueror exalts,
  awakener, eldritch, recombinator, fracturing, metamods, beastcraft). Not modelled: harvest costs (lifeforce).
  0.2.22: page rebuilt to fit one screen (full width: `.content.wide`; bar + three scrolling columns: your item as a
  game tooltip with P/S + tier per mod, clickable / plan / mod list / detail). Influence-only mods are listed on any
  base (`Family.needs`, coloured per influence, icon = that influence's Exalted Orb picture); their ways are the
  "<Influence>'s Exalted Orb" (weight share of that influence's own mods) and Chaos on an item that has it.
  Calculator: `plan()` = chance per method of hitting several mods at once (`simulate_all`, essences force one
  target, fossils that help any and block none, Harvest per tag, Alteration when ≤1 prefix + ≤1 suffix); "+" on a mod
  adds it, tier per target. Catalyst quality on the item ("quality|Life and Mana|20" in `have`) is taken off before
  reading tiers. Icons for orbs, essences, fossils, bench costs. Pool returns `item_mods`.

- Price check sweep 21 Nov (0.2.19): ran every saved sample through `setupof` (now prints UNKNOWN lines too).
  Mods written over 2–3 lines (Foulborn Asenath's corpse/Blinded/explode) are matched joined, as the site lists them.
  Map mods with no chance ("Monsters Hinder on Hit with Spells", "Monsters Inflict Withered…") retry as "Monsters
  have 100% chance to …"; any line retries as "100% chance to …" (replaces the Curse-only rule). Still unknown: "Players
  have -20% to amount of Suppressed Spell Damage Prevented" (not on the site's list). Map rows in the price window sit
  under "Map" instead of "Defences at 20% quality".

- Curse on Hit 21 Nov (0.2.17): "Curse Enemies with Punishment on Hit" (Foulborn Asenath's) is the site's "#% chance to
  Curse Enemies with Punishment on Hit" at 100% (listings show hash explicit.stat_2950697759, magnitude 100): setup
  retries "Curse Enemies with …" lines as "100% chance to …". All 334 Foulborn Asenath's listed have that mod, so his
  15c price was right. Left: multi-line unique mods ("…explode, dealing
3% of their Life…") still show not searchable.

- Foulborn local mods 21 Nov (0.2.16): Foulborn items looked their mods up as "mutated" first, and that lookup quietly
  fell back to the global stat, skipping the local one: Foulborn Ghostwrithe's "+183 to maximum Energy Shield" searched
  the jewellery stat and found 0. Now a mutated match counts only if its id is mutated.*, then local, then global.
  Live: 183+ ES, 90+ life, 35% → 34 listed from 250c (poe.ninja said 25c). Dev tool: examples/setupof.rs reruns a
  saved price check (samples/*.json) and lists the stats it picks.

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

**Map tracker: menus, loot, div per hour (22 Nov, not released yet):**
- [ ] Settings → Map tracker → **Set up**: click it, click into the game within 5 s with nothing open. Then in a map,
  open the passive tree: within ~2 s the tracker tag says "paused" and the clock stops; close it and it goes on.
- [ ] Same with the Escape menu. If it doesn't stop: press **Escape menu**, click into the game and press Escape
  within 5 s, then try again.
- [ ] Nothing pauses by mistake while playing normally (flasks, skills on cooldown, big fights, inventory open).
- [ ] Loot: run a map and portal out; within ~10 s the tracker shows "Last map" / "This map" with a total, the colour
  bar and the best drops. Portal back in and out again: the second visit adds to it. Totals look right?
- [ ] Session line shows maps · chaos · a map · **div / hour** after a few maps; Play tools → Map runs shows the
  bar, loot and best drop; clicking a map lists everything by kind.
- [ ] Settings: unticking a kind hides it from the list (still in the total); "List an item from N chaos" works.
- [ ] Tracker shows "NerfDis · level 91" instead of the old XP line; This session counts the same maps as Today.

**Crafting jewels and cluster jewels (22 Nov, not released yet):**
- [ ] Build window → Jewels tab: every non-unique jewel (clusters, Cobalt etc., abyss jewels) has a Craft button.
- [ ] Craft on his Large lightning cluster: the Crafting page opens on "Large Cluster Jewel: Lightning Damage" with
  Overshock, Thunderstruck, Widespread Destruction and the life regen planned; the "Start from…" box names the
  enchant, 8 passives and item level 50+; "How to make it" lists routes cheapest first (no bench / essence steps).
- [x] (0.2.40 fix) Cobalt Jewel, lightning cast speed + energy shield in the plan: the Plan box now lists Harvest
  reforge Defence first, then "Alteration + Regal Orb + Exalted Orb" and "Alteration + Regal Orb", all cheaper than
  Chaos Orbs. **Confirmed in game 22 Nov** (Harvest reforge Defence cheapest, then Alteration + Regal, Chaos near the
  bottom). Small mismatch for later: the Plan list says Harvest Defence 1 in 149 / ~198c while the "How to make it"
  panel says 1 in 174 / ~230c for the same step.
- [ ] Do the odds feel right next to Craft of Exile for a cluster he actually crafts? (Weights and mod counts were
  matched against Craft of Exile on 22 Nov.)

**Play tools (22 Nov, released in 0.2.39):**
- [ ] F8 levelling window: opens over the game without taking focus; moves to the next step when you enter a zone;
  ‹ › arrows; quest reward lines name your build's gems; "Type it" puts the gem search in a vendor's search box.
- [ ] Levelling options on the Play tools page (league start, bandits, jump to act) keep your place.
- [x] F9 map tracker: timer, maps per hour, deaths, Done button (**confirmed in game 22 Nov**, Park T16). Still unchecked:
  a Ctrl+D map check just before shows that map's mods; portals back in counted.
- [x] ~~XP per hour~~: showed 0 in his test. pathofexile.com no longer sends a character's experience (checked live
  22 Nov: get-characters and get-items only give name, class, league, level, lastLoginTime), so XP per hour was removed;
  the tracker shows "name · level N" instead.
- [ ] Stopwatch start / pause / lap / reset.
- [ ] F10 searches: clicking one with the stash or a vendor open puts it in the search box (and nothing happens in
  game when no search box is open). Your own searches can be added on the Play tools page.
- [ ] Wealth → Worth selling → "Highlight in stash" lights up those items.
- [x] F11 opens and both tabs work (22 Nov). He asked for game pictures and a better layout: done as look A (he picked it
  from three mock-ups): oil pictures, coloured Syndicate divisions, reward cards with pictures, cost in chaos/divine.
- [x] F11 look A (confirmed 22 Nov, he asked for oil names too: added): pictures show for oils and most Syndicate rewards; ticking oils lists recipes that use them
  ("you have 2 of 3", missing oils faded); marking cells sticks; added pictures show as tabs.
- [ ] Map tracker window scrolls when its content is taller than the window (the session summary used to be cut off).
- [ ] Notes window (set a key first) saves as you type; Esc gives the game back its focus.
- [ ] Alt+W opens the wiki page of the item under the mouse.
- [ ] None of F8-F11 / Alt+W / Alt+R clash with his other tools (Settings → Hotkeys to change or clear).
- [ ] Crafting: plan 2+ mods → "Crafting search" lights the item up in the stash when one rolls (any / all); "Add to my
  searches" puts it in F10. Verdict line under "How to make it"; "Or just buy one" works without a copied item.
- [ ] Trades: price line on a whisper looks sensible; **Show in stash** box lands on the right square (normal and quad tab).
- [ ] Ctrl+D on a map: "Safe to run" / "Don't run" with the reroll hint.
- [ ] Session summary in the tracker after a few maps; levelling trials count, "Trial in this zone", lab link, res reminder.
- [ ] Wealth: Check your prices, Divination cards, price alert pops up over the game when it crosses.
- [ ] Alt+R on an Eldritch altar picks top/bottom sensibly; on a remnant flags your can't-run mods; on a Ritual/reward
  screen lists prices. (Reads the screen: tell me what it got wrong.)
- [ ] Play windows hide while in a map and come back in town.
- [ ] Blight oils tab (F11) and the exchange check on Play tools.

**Next test session (planned 21 Nov night, for 22 Nov).** Walk him through it a few at a time, between maps.
Latest release is 0.2.41 (22 Nov, adds jewel Alteration + Regal routes and clearer Syndicate rows; 0.2.40 added jewel and cluster crafting, cheat sheets look A, scrolling play windows; 0.2.39 added the Play tools + QoL wave 2 above); everything below is in it (0.2.38 added fixed base mods hidden, mod source badges, docked price window).
**He plays on the installed release** (`%LOCALAPPDATA%\PoE Filter Editor\poe-filter-editor.exe`), not the dev
editor, so unreleased things (e.g. Settings → Main window look) only reach him after a release.
**22 Nov results:** 1. Grouped layout, low confidence and Dragon Turn +57 seen working; he wanted low confidence more
obvious → now a solid red badge and a red-tinted price box. Flat layout not tried yet. 2. Batch mode works; the small
box was cut off → it now sizes itself to fit. 3. Guild stash not tried yet. 4. Main window look missing because it
isn't released. Also: the "search straight away" setting reworded ("Search rares and gear straight away too" + a
plain explanation underneath). Flat layout confirmed. He picked menu mock-up A (six small groups, each with a
"what's in here" line) with C's colours (blue = filter, amber = your changes, green = playing, purple = money and
items) and Settings mock-up 1 (side menu: Hotkeys, Price check, Trading, Maps and area, Looks, AI). Both built
22 Nov, in 0.2.37. Group colours only show in the Colourful look; the calm looks keep gold/grey headings.
Price window: he wants Awakened PoE Trade as the template, made nicer: everything needed one click away (nothing
behind "More"), the item's own picture, currency icons, game icons everywhere. Round 2 mock-ups (answer first, chips,
strip, ladder…) passed over. Built 22 Nov, in 0.2.37: three new looks (Style button / Settings → Looks):
**Awakened** (item picture + filter chips under it: base only, ilvl band, Vaal-orb corrupted toggle for uniques, and
the "Also searching for" filters shown as chips; stats; Search; price; listings; bottom bar with icons: Trade site,
Craft, Save search, Saved, Style, Layout, Report), **Awakened + price** (adds a price bar under the item with
cheapest / typical / listed tiles) and **Awakened tooltip** (same inside the game's tooltip frame). Also: exalted
orb picture on exalt prices, orb pictures in the market tiles (Split look too). Next: he tries them in game and
picks one (then maybe make it the default and drop the looks he doesn't use). 22 Nov after 0.2.37: **Awakened +
price is his favourite so far** (not final). Base mods with no roll range (same on every item of that base, e.g.
Cogwork's four) are now hidden behind "4 base mods the same on every Cogwork Ring · show", like Awakened's "Show
hidden" (`hide_const_roll`); only when the copy had ranges (advanced copy), never ticked ones or maps. In 0.2.38.
**Mod source badges** (22 Nov, in 0.2.38): like Awakened's tags, a coloured badge (with the game's little icon from
`public/mod-icons/`, copied from Awakened's repo) after any mod that needs something special: Exarch / Eater (eldritch),
Essence, Delve, Veiled, Incursion, the six influences, Infamous, Rare only, Synthesised, Corrupted. Read from the
advanced copy's "{ … Modifier "name" }" heading (`mod_source` in core/src/trade.rs, Awakened's name lists);
`StatFilter.source`. Crafted / fractured / foulborn tags now coloured too. Eldritch implicits are never hidden as
"fixed base mods". **Docked price window** (22 Nov, in 0.2.38): like Awakened, the Ctrl+D window opens at the top
of the game beside its side panel, every time: just left of the inventory when you price check on the right half of
the game, just right of the stash on the left half (panel = game height × 370/600, Awakened's figure;
`place_beside_panel` + `TRADE_DOCK` in lib.rs keep that edge when the window grows). Falls back to beside the mouse
when the game window isn't found. Check in game: the window's spot, and that it doesn't cover the inventory.
1. Price window (Ctrl+D on a rare): stats in groups (Minimum rolls, Prefixes "1 of 2 used", Suffixes...); More → Layout
   → Flat gives one list with a tag on each row. An item with under 5 listed shows amber "low confidence".
   Cogwork Ring (Dragon Turn): Intelligence reads +57 (not +46).
2. Batch mode: F3, Ctrl+C a few items (small box says "Added"), F3 again, pick Hide/Highlight, Save, F4.
   After any filter change: "✓ Saved … · Undo" note at the bottom; Undo puts it back.
3. Wealth page: tick "Include my guild stash", scan: "Guild · …" tabs appear in the list and the total.
4. Settings → Main window look: try Colourful, Calm, Tooltip Gold, Mono Amber; pick a favourite. Small grey text easy
   to read on every page? Home page + Ctrl+K search feel easier?

**Batch mode (21 Nov, in 0.2.36)**
- [x] (Confirmed 22 Nov, "seems good".) Box was cut off at its fixed 250×64 size; it now fits itself to its text
  (in 0.2.37; check the box shows its close button and whole second line).
- [ ] Press F3 in game: the editor no longer pops up over the game; a small "Batch mode · N items" box shows instead
  (drag it anywhere, it remembers). Ctrl+C items: it says "Added: …" or "Already listed: …". Press F3 again: the
  editor opens on Batch mode with the list. Each row says what the filter does now ("now Hidden"); hidden items start
  on Highlight, shown ones on Hide; "Leave as is" takes it off with no edit. Save, then F4.
  Code: the list lives in the app (`BATCH_LIST`, `batch_list` / `batch_forget`), so switching pages loses nothing;
  window label "batch" (`BatchWindow` in src/Batch.tsx). Uniques are told apart by name.

**Tiers page and build window (21 Nov, after 0.2.33)**
- [ ] Tiers: every tier now has a plain name (e.g. cluster jewels "Large, 8 passives (best item level)", uniques
  "Could be worth something (several uniques share the base)", currency "Tier 2 · about a Divine Orb", "New items, not
  tiered yet" for NeverSink's restex safety net). Viscera / Essences / Delirium orbs say why there's no tier 1
  (NeverSink comments it out: nothing is worth that much).
- [ ] Jewels page: after "Show them" on corrupted rare jewels, "How they look" opens the look editor (saves as a
  "Style: Corrupted Rare every Jewels" edit; Back to NeverSink's removes it).
- [ ] Build window (F6): rare/magic gear has a "Craft" button: opens Crafting with every mod planned, from a fresh base at
  item level 86 (PoB copies say item level 1). Not on uniques, flasks or jewels.

**Price check (Ctrl+D)**
- [ ] Awakened ideas (21 Nov, after 0.2.33): price a unique with rolls (mods ticked, minimums a little below yours,
  "Not corrupted" in "Also searching for"); an anointed amulet (the anoint ticked); a rare cluster jewel (passive count
  and the small-passive line ticked); a magic utility flask (searches that flask base only); a heist blueprint or
  contract (area level, wings); an influenced item (an "Influence" row to tick; "As a base" ticks it). Open "Also
  searching for" and check it reads right.
- 21 Nov polish (impeccable): armour/ES rows no longer repeat "(at 20% quality)"; Save search sits beside the Search
  button; the small links (trade site, Craft this, Saved searches, Style, Something's wrong?) share one footer row;
  Compare look: listings no longer in a cut-off inner scroll box (the window scrolls), "Like yours / Also has" mods
  line up in a column. Tick boxes drawn dark/gold to match (were browser-white); unsearchable mods under their own
  "Not searchable" heading, lined up with the rows.
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

**Crafted chaos (21 Nov, 0.2.18)**
- [x] (Confirmed 21 Nov.) Ctrl+D on gear with a crafted "X and Chaos Resistances": "+N% total to Chaos Resistance" is there to tick.

**Crafting routes (21 Nov, 0.2.32)**
- [ ] Crafting with a copied item: "Plan this whole item" puts its mods in the plan. "Searing Exarch Item" / "Eater of
      Worlds Item" no longer count as suffixes.
- [ ] "How to make it" (left column) names the cheapest route; "Compare all N routes →" opens the wide panel: routes
      sorted Cheapest / Fewest clicks / Safest, each with cost (chaos + Divines), clicks, risk; click one for its steps,
      "N other ways" per step. "Or just buy one" → Check the price searches trade and says if buying is cheaper.
- [ ] In game: the 3.30 bench lock ("1 Prefix Modifier cannot be changed") survives a Harvest reforge and an Orb of
      Scouring (routes marked "check in game" depend on it). Also: does Chaos Orb keep locked mods?
- [ ] In game (if he tries Eldritch): one Lesser Eldritch Ember alone makes prefixes dominant; how many mods an Eldritch
      Chaos Orb adds.

**Corrupted jewels (21 Nov, 0.2.31)**
- [ ] Tiers → Jewels → Jewels: a "Corrupted rare jewels" card; Show them / Hide them / Back to NeverSink's; after F4,
      corrupted rare Cobalt / Crimson / Viridian jewels show highlighted.
- [ ] F2 with "Only corrupted ones" ticked: a gold note says to pick a colour or Hide (ticking alone saves nothing).

**Local stats fix (21 Nov, 0.2.30)**
- [ ] Ctrl+D on Xirgil's Crank (or any staff/weapon with +maximum Energy Shield): ticked rolls find listings again.
- [ ] Gloves with "increased Attack Speed" (e.g. Phoenix Clutches) find listings with that roll.

**Flask quality + jewels on Tiers (21 Nov, 0.2.28)**
- [ ] Ctrl+D on a flask with quality over 20% (e.g. 26%): a ticked "Quality: 26%" row; the search finds 26%+ flasks.
- [ ] Tiers page: a Jewels group (Jewels, Cluster jewels, Cluster jewels (cheap), Special jewels) with jewel pictures.

**Corrupted only + frame fix (21 Nov, 0.2.27)**
- [ ] F2 on a corrupted item (e.g. a corrupted rare Cobalt Jewel): an "Only corrupted ones" tick; highlight with it ticked:
      corrupted rare jewels show, ordinary ones stay hidden.
- [ ] Editor window has only the gold title bar (the Windows one is gone; the saved window state no longer brings it back).
      21 Nov: he still saw both, but was running 0.2.24 (the fix is in 0.2.27): check after "Restart to update".
- [ ] General polish (after 0.2.27): name shows once (gold title bar; the sidebar no longer repeats it); pages use up
      to 1280px and sit centred beside the sidebar on wide windows; every tick box in the app is dark with a gold tick.
      (In 0.2.28.)
- [ ] Tiers page polish (after 0.2.28): the "?" help opens under the heading in normal letters (was squeezed beside it
      in small caps); help boxes everywhere have a soft gold frame; tiers aren't a box inside a box; item chips show as pills.
      (In 0.2.29.)
- [ ] "Polish everything" pass (21 Nov, released after 0.2.29): card headings one size everywhere (ones beside a
      button were smaller: Filters, My edits, Batch, Prices, Wealth); hint text in empty boxes easier to read; Help says
      "Filters" (not "Profiles"/"Built on"), adds price check + crafting answers and every hotkey; Atlas page opens at the
      top (the step list used to scroll the whole page down to the current step); Maps wishes three per row (labels no
      longer wrap); Levelling gear main skill box fills its row; Themes: the one in use reads "✓ In use on …"; Presets
      tiles flatter; F2 "Pick a sound…" link gold instead of web blue; F7 strategy picker shows the whole name.
      Dev: preview-app.html now has sample data for Presets, Themes, Levelling gear and Prices; `await __go("Themes")`
      in the console opens a page.
- [ ] Home page, calmer sidebar and Ctrl+K search (21 Nov, in 0.2.32): the editor now always opens on
      **Home** (your filter, recent changes, prices on/off, build, atlas strategy, crafting, and the in-game hotkeys;
      "Reload in game" button). Alt+← still goes back to the page you had open last time. Sidebar: Home, then two
      groups ("Your filter", "Playing"), Settings + Help pinned at the bottom; only the gold in-game keys show (Ctrl+1…9
      still work, listed in each item's tooltip). **Ctrl+K** (or the Search button) finds items (opens them on Find,
      ready to change), pages and Help answers. Check: does it feel easier? Is anything missing on Home?
      Undo: Home.tsx / Search.tsx are self-contained; App.tsx MENU/BOTTOM hold the sidebar.
- [ ] Price window clean-up, every style (21 Nov, in 0.2.33): window 480px wide so long mods fit on one line;
      price box reads "sells for about" with "20c to 1 div · 32 listed"; unticked stats fade back; each stat keeps its
      tier and roll bar plus the roll range (no "high roll" words); headings without hints (hover for them); small
      "This item / As a base" switch; no "Instant" tag on listings; item level column only when levels differ;
      footer = Open on the trade site · Craft this · More (saved searches, Style, Something's wrong?).
      **New setting, on by default: Settings → Price check → "Instant buyout only"** (trade status "securable",
      checked on the live site: Stygian Vise 7,029 vs 7,913 listings). GROBBS dropped the "Quick look" style idea:
      roll ranges matter for most rare/unique prices. (In 0.2.33.)
- [ ] "Saved" note + less reading (21 Nov, in 0.2.34): after any change to the filter, from any page or F2, the
      editor shows a small note at the bottom: "✓ Saved <what> · Press F4 in game to see it · Undo". It fades after a
      few seconds (stays while the mouse is on it); Undo goes back one change and reloads the open page. (Rust: every
      change goes through `remember()`, which now emits `filter-changed` to the main window.) Long explanations at the
      top of Filters, My edits, Themes, Prices, Maps, Atlas, Crafting and Settings cut to one line; the detail moved
      into the "?" next to the heading.
- [ ] Price window: two layouts + low confidence + Cogwork numbers (21 Nov, in 0.2.34). 22 Nov: Grouped, low
      confidence, +57 Int and Flat confirmed in game; low confidence changed from amber to red (in 0.2.37).
      **Grouped** (default): headings Minimum rolls · Defences (at 20% quality) · Totals (all mods added up) · Base mods
      (implicit) · Prefixes "1 of 2 used" · Suffixes "3 of 4 used" (open-slot rows sit inside their group, "1 open
      prefix · room to craft") · Base and item level · For sale now. **Flat** (like Awakened): no stat headings, a tag
      on each row (total, base mod, prefix, crafted prefix, suffix, open slot, 20% quality); mods already counted in a
      total (resistances, life, mana, attributes) tucked behind "N mods are inside the totals · show". Switch under
      More → "Layout", or Settings → Price check → "How the stats are listed". Fewer than 5 listed: amber
      "low confidence · only N listed" instead of "sells for about" (his Dragon Turn showed 45 div from 1 listing;
      live check: ~55 similar rings, 50c–3 div). Cogwork Ring: mods marked "— 25% Increased" in the advanced copy now
      use the in-game numbers (+46 Int → +57, range 53–62); GROBBS rejected splitting Totals into Resistances / Life.
- [ ] Wealth: guild stash (21 Nov, approved and released in 0.2.35).
      Wealth page → "Include my guild stash" (off by default: guild items aren't always yours). On: the guild's tabs
      join the tab list as "Guild · <name>" (stored at index 1000+ so they never clash with your own), and scans,
      scan profiles, earnings tabs and Find in stash use them like any other tab. Off: they leave the list, profiles
      and earnings tabs. Checked live 21 Nov: his login reads the guild stash (3 tabs: normal, currency, quad).
      Endpoint: character-window/get-guild-stash-items (same POESESSID and rate limits as get-stash-items).
- [ ] Design review fixes + main window looks (21 Nov, in 0.2.37). Whole-app impeccable critique scored 28/40
      (saved in .impeccable/critique/). Done from it: small grey text readable everywhere (--muted lifted), faded
      price-window rows and roll ranges clearer, no labels under 11px; wording (Setup says Filters, atlas key says
      Atlas page, "More > Style/Layout", Help shows his own hotkeys, "Rebuild filter", "Save report" not "Save for
      Claude"); calmer stash sign-in panel. GROBBS kept F2 and the rare price check as they are (Awakened also waits
      on rares). "Sign in with Path of Exile" (GGG OAuth) isn't possible: GGG isn't taking new app registrations, and
      guild stash access would need a special request. **Settings → Main window look**: Colourful (old), Calm (plain
      menu icons, gold for the current page, Home leads with the filter), Tooltip Gold (game tooltip frames, bronze,
      Fontin headings) and Mono Amber (plain greys, amber accent). Main window only; the overlays keep their own looks.
      GROBBS is a visual learner: show design options as mock-up pictures.

**Frame + fixes (21 Nov, 0.2.24)**
- [ ] Editor window: no Windows title bar, a gold one instead; drag it to move, double-click to maximise, the three
      buttons work, close hides to the tray; the window still resizes from its edges and snaps to screen sides.
- [x] (Confirmed 21 Nov, "compare works".) Compare look after a search: each listing shows its mods in green / red / blue.
- [ ] 0.2.25: Compare splits each listing into "Like yours" (green/red/blue) and "Also has"; the Crafting item card
      has the fracture symbol (or influence symbols) at both ends of its title, like the game.
- [ ] Settings → Price check: a small picture of each look; clicking one picks it.
- [ ] Crafting with a copied item: base picture on the card; fractured mod has the fracture icon and a note; a crafted
      mod (⚒) opens its bench cost; "Aspect of the Spider" shows how to get it (beastcraft).

**Looks + sidebar (21 Nov, 0.2.23)**
- [ ] Ctrl+D → "Style: Classic ›" at the bottom switches the look (Tooltip, Split, Compare, Price first) and it sticks;
      Settings → Price check has the same choice. Compare: each listing's mods green (better) / red (worse) than yours.
- [ ] Sidebar: group headings easy to read; in-game keys beside My edits, Batch mode, My build, Atlas, Prices, Filters
      (they follow what you set in Settings); hovering a page shows its Ctrl+number.

**Crafting page (21 Nov, 0.2.20)**
- 21 Nov polish (impeccable): the mod list column is wider (fewer mods wrap); "per Chaos Orb" labels the odds
  column beside each Prefixes/Suffixes heading (the cramped legend line is gone); "+ Add to plan" stays on one line;
  way cards say "price unknown" instead of "≈ price unknown".
- [ ] Editor → Crafting: pick a base (e.g. Eternal Burgonet), item level, influence: prefixes and suffixes listed with
      "1 in N" per Chaos Orb; click a mod → tiers, ways to get it (Chaos, essence, bench, fossils) with costs.
- [ ] Ctrl+D on a rare helmet/ring/weapon → "Craft this": the editor opens on Crafting with that item, its mods marked "yours".
- [ ] "Open in Craft of Exile": their site opens; Import item + Ctrl+V brings your item in.
- [ ] 0.2.21: a mod's "Ways to get it" also lists Orb of Alteration, Harvest reforges, and (with your item brought
      over) Exalted Orb on a rare with a free slot / Augmentation and Regal on a magic item; cheapest is outlined.
- [ ] 0.2.22: the page fits one screen; a copied item (e.g. Entropy Grasp) shows its mods on the left with P/S and
      tier, click one to see it; search "shaper" or a Hunter mod on a plain base: it shows, coloured with the orb icon,
      "needs Hunter"; press + on 2–3 mods: the Plan box shows the odds of all of them at once.

**Price check sweep (21 Nov, 0.2.19)**
- [ ] Ctrl+D on a map with "Monsters Hinder on Hit with Spells" or "Monsters Inflict Withered…": no longer "not
      searchable"; Atlas Drops / Rarity / Pack Size sit under a "Map" heading.
- [ ] Ctrl+D on Foulborn Asenath's Gentle Touch: the corpse / Blinded / explode mod is one row you can tick.

**Foulborn (21 Nov, 0.2.16–0.2.17)**
- [x] (Confirmed 21 Nov.) Foulborn Asenath's Gentle Touch: "Curse Enemies with Punishment on Hit" searchable.
- [x] (Confirmed 21 Nov.) Ctrl+D on a Foulborn unique armour/weapon (e.g. Foulborn Ghostwrithe): ticked rolls find listings.

**Buy base (21 Nov, 0.2.14)**
- [x] (Confirmed 21 Nov: "cluster base worked well".) F6 → Jewels → "Buy base" on a cluster jewel: trade site opens with cheap bases at the right item level; the note
      says "N bases · item level 75+ (Inspired Oppression)".

**Buy (21 Nov, 0.2.13)**
- [ ] F6 → Jewels → Buy on the "8 Passive Only" large cluster: the trade site opens with results, button says
      "… for sale · any 2 of 3 mods".

**Thin cards (21 Nov, 0.2.12)**
- [ ] With the 5c card line on: The Calling and other hardly-traded cheap cards are hidden after F4; Ctrl+D on one says
      "Hardly traded: about N sold". Valuable cards still show.

**Ctrl+D again (21 Nov, 0.2.11)**
- [x] (Confirmed 21 Nov.) With the price window open, hover another item and press Ctrl+D: it prices the new item straight away.

**Worth line (21 Nov, 0.2.10)**
- [ ] Tiers → Divination cards: tick "Hide cards worth under", set e.g. 5c, F4 in game: cheap single cards hidden,
      a stack worth 5c+ (e.g. Rain of Chaos ×17) still shows; good cards look as before. Same on Uniques.

**Tray menu (21 Nov, 0.2.8)**
- [x] (Confirmed 21 Nov, "updates works too"; restart editor: comes back in front.) Tray "Check for updates" says "Up to date (0.2.8)";
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
0. **Play tools (22 Nov) released in 0.2.39.** Not built yet: live rank/race tracker (PoeRank-style), bulk-selling helper
   (Bulky-style), PoE 2's campaign route, crafting methods still unmodelled (veiled, conqueror exalts, awakener,
   recombinators, beastcraft), PoE 2 crafting.
1. **GROBBS to work through "Still to check in game" above**; fix what he reports.
2. Wealth tracker follow-ups (real account tested and rares priced via the trade site, 21 Nov):
   "search every tab" done 21 Nov; guild stash built 21 Nov on branch `guild-stash` (awaiting approval); crafting planner done (0.2.32).
3. Price check polish: done 21 Nov (Awakened ideas: unique roll ranges, anoints, cluster jewels, flasks, heist,
   influences, "Also searching for"). Waiting on his in-game check.
4. Deferred earlier: PoE 2 coverage. Batch mode tidy-up done 21 Nov (see "Batch mode"). (Installer + auto-update done 21 Nov: see "Installer and updates".)

## Known caveats
- This cloud workspace can't reach pathofexile.com / poe.ninja / poeprices.info, so network code is only tested by
  GROBBS in game. Windows-only code (`screen.rs` imp module) can't be compiled here: keep logic in the plain functions
  above it (they have tests) and write the Windows part carefully.
- Trade site User-Agent: `OAuth poe-filter-editor/0.1 (contact: github repo)`; if Cloudflare starts blocking, revisit.
- Linux type-check of the app: install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev, then `cargo check -p poe-filter-editor`.
- Test harness: Playwright scripts mocking `window.__TAURI_INTERNALS__` (render `dist/` after `npm run build`); the
  `trade` window is chosen by window label in `src/main.tsx`.
