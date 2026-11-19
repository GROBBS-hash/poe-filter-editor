# PoE Filter Editor: instructions for Claude Code

**Read `HANDOFF.md` first, every new session.** It covers what's built, where the code lives and the agreed to-do order.
Keep it up to date: when a feature lands or the plan changes, edit HANDOFF.md in the same commit.

## Who you're working with
GROBBS (the owner) is non-technical. Keep updates short and in plain English, with no code jargon. He often works by voice,
so lead with what he should do or test next.

## Working on his PC (Claude Code) vs the old cloud chat
Until 19 Nov 2026 this project was built from a cloud chat, with GitHub in between. Now you run on his Windows PC,
in the same folder `Start editor.bat` runs from. That changes a few things:
- **You can run and test things yourself.** `npm run tauri dev` (or the editor he already has open) hot-reloads
  front-end edits, and Rust changes trigger a rebuild. Windows-only code (`screen.rs` imp module, `focus_game`,
  `type_in_chat`) now compiles here, so check it with `cargo check -p poe-filter-editor` before you hand over.
- **The network works.** pathofexile.com, poe.ninja, poeprices.info and data.poe.re are reachable, so you can test
  network code against the live sites before asking him to (be polite: follow the rate limits in `trade_api.rs`).
  The "Known caveats" in HANDOFF.md about the cloud workspace no longer apply here.
- **Still commit, and push after each finished change,** so GitHub stays the backup and the history stays readable.
  `Start editor.bat` runs `git pull --ff-only` every 30s. That is harmless here (nothing new to pull, or it refuses to
  merge), but don't rewrite pushed history.
- **ReaderSnapshots** (failed ground reads) are local: `%USERPROFILE%\Documents\My Games\Path of Exile\ReaderSnapshots`.
  Read them directly.
- Never print, log or commit his POESESSID (it's in the app's settings file in its data folder).

## Picking up where the chat left off (19 Nov 2026)
A lot has been built since his last in-game test. **HANDOFF.md → "Still to check in game"** is the checklist:
- Keep it current. Every new feature goes on it, and items move to "Confirmed in game" only when he says they work.
- When he sits down to play or test, walk him through it a few items at a time, in plain English.
- Test what you can yourself first (network calls, Windows builds), so he only checks what really needs the game.
Then carry on with "Left to do" in HANDOFF.md.
