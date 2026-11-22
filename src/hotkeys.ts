/** Every in-game hotkey in one place, in plain words: the Home page's "In game" card, Settings → Hotkeys, Help and the
 *  sidebar's gold key badges all read this, so a key is explained the same way everywhere (22 Nov 2026, GROBBS picked
 *  hotkey card "C": a short name, then how it works, grouped by what the keys are for). */
export type Hotkey = {
  /** The setting that holds the key (Settings → Hotkeys). */
  setting: string;
  /** The key when he hasn't changed it ("" = no key until he sets one). */
  def: string;
  /** A short name, e.g. "Change an item's look". */
  name: string;
  /** How it works, in one or two short sentences. */
  how: string;
  group: "filter" | "lookup" | "windows";
  /** Left off the Home card (no key by default, or off by default). */
  extra?: boolean;
};

export const HOTKEYS: Hotkey[] = [
  { setting: "key_quick_edit", def: "F2", group: "filter", name: "Change an item's look",
    how: "Hover any item, on the ground or in your stash, and press it: hide it, highlight it, change its colour or sound." },
  { setting: "key_batch", def: "F3", group: "filter", name: "Batch mode",
    how: "Press it, Ctrl+C each item you want to change, press it again, then change them all in the editor." },
  { setting: "key_reload", def: "F4", group: "filter", name: "Reload filter",
    how: "Press it after you change something in the editor, so the game shows the change." },
  { setting: "key_cycle", def: "F5", group: "filter", name: "Next filter", extra: true,
    how: "Switches to your next filter (e.g. League Start → Strict 1 → Strict 2) and loads it." },
  { setting: "key_price", def: "Ctrl+D", group: "lookup", name: "Price check",
    how: "Hover an item to see what it sells for. On a map it warns you about mods your build can't run." },
  { setting: "key_wiki", def: "Alt+W", group: "lookup", name: "Wiki",
    how: "Opens the wiki page of the item under your mouse in your browser." },
  { setting: "key_read", def: "Alt+R", group: "lookup", name: "Read a choice",
    how: "Hover an Eldritch altar, a remnant or a reward screen: it tells you which pick is best for you." },
  { setting: "key_build", def: "F6", group: "windows", name: "Your build",
    how: "Your Path of Building gems, gear and passive tree over the game." },
  { setting: "key_atlas", def: "F7", group: "windows", name: "Atlas plan",
    how: "Your atlas tree plan over the game, step by step." },
  { setting: "key_level", def: "F8", group: "windows", name: "Levelling route",
    how: "Where to go next and which quest rewards to take. Moves on by itself as you enter zones." },
  { setting: "key_tracker", def: "F9", group: "windows", name: "Map tracker",
    how: "Time, loot and deaths for each map, and how much you make per hour." },
  { setting: "key_searches", def: "F10", group: "windows", name: "Searches",
    how: "Click a saved search and it's typed into your stash or a vendor's search box." },
  { setting: "key_sheet", def: "F11", group: "windows", name: "Cheat sheets",
    how: "Syndicate rewards, blight oil recipes and any pictures you add." },
  { setting: "key_notes", def: "", group: "windows", name: "Notes", extra: true,
    how: "A notepad over the game, saved as you type." },
];

export const HOTKEY_GROUPS: { id: Hotkey["group"]; name: string; note?: string }[] = [
  { id: "filter", name: "Change your filter" },
  { id: "lookup", name: "Look up an item" },
  { id: "windows", name: "Windows over the game", note: "press the key again to hide it" },
];

/** One hotkey by its setting. */
export const hotkey = (setting: string) => HOTKEYS.find((h) => h.setting === setting)!;

/** The key he uses now (from Settings), or the default. */
export const keyOf = (keys: Record<string, string | undefined>, setting: string) => keys[setting] ?? hotkey(setting).def;
