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
  { setting: "key_quick_edit", def: "F2", group: "filter", name: "Change how an item drops",
    how: "Hover an item (on the ground, in your bags or stash) and press it. Choose how your loot filter shows that kind of item from now on: hide it, make it stand out, or change its colour or drop sound." },
  { setting: "key_batch", def: "F3", group: "filter", name: "Change many items at once",
    how: "Press it, then Ctrl+C each item you want to change. Press it again and the editor opens with all of them, so you change them together." },
  { setting: "key_reload", def: "F4", group: "filter", name: "Apply filter changes",
    how: "Makes the game load your filter again, so changes you made in the editor show straight away. It types /itemfilter in chat for you." },
  { setting: "key_cycle", def: "F5", group: "filter", name: "Switch filter", extra: true,
    how: "Swaps to your next filter (e.g. League Start, then Strict) and loads it in the game. Handy when you want to hide more as you get stronger." },
  { setting: "key_price", def: "Ctrl+D", group: "lookup", name: "Price check",
    how: "Hover an item and press it to see what it sells for on the trade site. On a map it also warns you about mods your build can't run." },
  { setting: "key_find", def: "Alt+F", group: "lookup", name: "Search a price",
    how: "Press it anywhere in the game and type an item name: its poe.ninja price shows as you type, with every version and a link to the trade site. Not Ctrl+F, which is the game's own stash search." },
  { setting: "key_wiki", def: "Alt+W", group: "lookup", name: "Open on the wiki",
    how: "Hover an item and press it: its page on the Path of Exile wiki opens in your browser." },
  { setting: "key_read", def: "Alt+R", group: "lookup", name: "Which one should I pick?",
    how: "When the game makes you choose (an Eldritch altar, an Expedition remnant, a reward screen), hover it and press: it tells you the best pick for your build and what's dangerous." },
  { setting: "key_build", def: "F6", group: "windows", name: "Your build",
    how: "Shows your Path of Building build over the game: which gems, gear and passive points to aim for." },
  { setting: "key_atlas", def: "F7", group: "windows", name: "Atlas plan",
    how: "Shows your atlas passive tree plan over the game, one step at a time, so you know which point to take next." },
  { setting: "key_level", def: "F8", group: "windows", name: "Levelling route",
    how: "For a new character: where to go next and which quest rewards to take. Moves on by itself as you enter each zone." },
  { setting: "key_tracker", def: "F9", group: "windows", name: "Map tracker",
    how: "Times each map and counts your loot and deaths, so you see how much you make per hour. Stops while you alt-tab or a menu is open." },
  { setting: "key_searches", def: "F10", group: "windows", name: "Stash searches",
    how: "Your saved search words. Click one and it's typed into the stash or vendor search box for you, so matching items light up." },
  { setting: "key_sheet", def: "F11", group: "windows", name: "Cheat sheets",
    how: "Quick-look pictures over the game: Syndicate rewards, Blight oil recipes and any pictures you add." },
  { setting: "key_notes", def: "", group: "windows", name: "Notes", extra: true,
    how: "A notepad over the game for anything you want to remember. Saved as you type." },
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
