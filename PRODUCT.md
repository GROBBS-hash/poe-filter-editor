# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(A Tauri 2 desktop app for Windows: React/TypeScript front end in `src/`, Rust in `core/` and `src-tauri/`. The
screens are web UI in native windows: one main window plus small always-on-top overlay windows over the game.)

## Users

Path of Exile players, of PoE 1 and PoE 2. Today the only user is the owner, GROBBS, who is non-technical and plays
with the editor open beside the game. A public release is planned, so screens must make sense to a stranger who
installs it without help.

Two situations of use:
- **Mid-game, seconds at a time:** a hotkey (F2 quick edit, Ctrl+D price check, F6 build, F7 atlas, trade whispers)
  opens a small overlay on top of the game. The player glances, decides, clicks once, and goes back to playing.
- **Between maps or before a league, minutes at a time:** the main window, for editing the filter, crafting plans,
  map regex, builds and settings.

## Product Purpose

One app that does the jobs players otherwise spread over several tools: loot filter editing (built on NeverSink's
filters, edits kept as rules that survive filter updates), price checks against poe.ninja and the trade site,
crafting odds and plans, map checks and stash regex, PoB build guidance, atlas strategy, and trade whisper handling.
Success: a player keeps this one app open instead of FilterBlade, Awakened PoE Trade, Craft of Exile and the rest.

## Positioning

Everything in one place, tied together: the price window can open the crafting page for the same item, a map check
uses the player's own PoB build, F2 on an item shows exactly which filter rule caught it and lets you change it in
place. Separate single-purpose tools can't connect these steps.

## Operating Context

- Played in Windowed Fullscreen so overlays can sit on top of the game; overlays must not steal focus or cover the
  item being looked at for longer than needed.
- Never interrupt play: no pop-ups, restarts or update prompts appearing on their own mid-game.
- Reads the game: copied item text (Ctrl+C / Ctrl+Alt+C), screen reading of ground labels, Client.txt for whispers.
- Talks to pathofexile.com trade (strict rate limits), poe.ninja, data.poe.re, NeverSink's GitHub.
- Installed per-user via an NSIS installer with background updates that only apply when the player restarts.

## Capabilities and Constraints

- Terminology follows the game exactly (mod tiers T1..., prefixes/suffixes, implicit, fractured, influenced, chaos /
  divine orbs, ilvl, 6-link). Players read these at a glance; don't rename them.
- PoE 2 support: the product is meant to cover both games; how far each feature applies to PoE 2 is still open.
- poe.re data is used under their terms (credit shown; ask them before selling or a big release).
- Never show, log or store the player's POESESSID anywhere visible.

## Brand Commitments

- Name: "PoE Lodestone" (chosen 21 Nov 2026; was "PoE Filter Editor"). Icon: a faceted gold lodestone with a compass
  needle, on a dark rounded tile (`app-icon.svg`, source for every icon size via `npx tauri icon app-icon.svg`).
- The app's existing look borrows from the game (gold title bars, item-card styling, rarity colours). Visual direction
  is recorded in DESIGN.md when documented, not here.
- Not affiliated with Grinding Gear Games; must not present itself as an official tool.

## Evidence on Hand

- Real sample price checks and trade results, saved locally in `%APPDATA%\com.grobbs.poefiltereditor\samples`, shown
  in the dev preview pages (`preview.html`, `preview-qm.html`, `preview-app.html`).
- No users, testimonials or download numbers yet; don't invent any.

## Product Principles

1. **Glance, decide, back to the game.** Overlay screens put the answer first and need one click at most.
2. **Connect the jobs.** Every screen should lead naturally to the related one (price → craft, item → filter rule).
3. **Speak the game's language.** Use PoE's own terms and conventions; a player should never need a manual.
4. **Never get in the way.** No surprise windows, focus theft or blocking waits while the player is in game.
5. **Clear to a stranger.** Plain labels and sensible defaults, because the public will use it without help.
