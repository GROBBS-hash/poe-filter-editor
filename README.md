# PoE Filter Editor

Edit your NeverSink loot filter while you play. Hover an item, press **F2**, click Hide or a colour.

## Run it

You already have Rust and Node from building Sortie, so:

```
npm install
npm run tauri dev
```

To make an installer: `npm run tauri build` (the installer lands in `src-tauri/target/release/bundle/nsis`).

## Use it

1. First launch asks: which game, which NeverSink filter, and what to call your filter (default "League Start").
2. In game, open Options > Game > Item Filter and pick **League Start**. Play in **Windowed Fullscreen** so the pop-up can appear over the game.
3. Hover any item and press **F2**. Pick Hide, Louder, Quieter or a highlight colour.
4. Type `/itemfilter League Start` in chat to reload. Done.

- **F3** turns on batch mode: Ctrl+C as many items as you like, then set them all at once in the app.
- **Undo last** is in the pop-up. Every edit is also listed under *My edits*, where you can remove any of them.
- **Profiles > Save as new** makes a stricter copy without touching your league-start filter.
- Your NeverSink file is never changed. Edits are stored by the app and written into a separate filter. After downloading a new NeverSink version over the old file, press **Rebuild**.
- Backups of the last 30 versions go to `LiveEditorBackups` in your game's filter folder.
- The app lives in the system tray (Open / Quit). Closing the window keeps the hotkeys running.

## Project layout

- `core/` — the filter brain: reads NeverSink filters losslessly, parses copied items, stores edits and profiles. `cargo test` runs 11 tests against a real NeverSink filter.
- `src-tauri/` — the desktop app: hotkeys, clipboard, tray, commands.
- `src/` — the screens (React).

Plan and roadmap: see the "PoE Live Filter Editor" doc.
