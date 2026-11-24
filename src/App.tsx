import { friendly } from "./errors";
import { PoeLogin, needsLogin } from "./LoginNeeded";
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import Help from "./Help";
import ErrorBoundary from "./ErrorBoundary";
import { MainTitleBar } from "./WinFrame";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Overview } from "./types";
import type { Source } from "./ItemPanel";
import Setup from "./Setup";
import Profiles from "./Profiles";
import Edits from "./Edits";
import Batch from "./Batch";
import Prices from "./Prices";
import Themes from "./Themes";
import SettingsPage from "./Settings";
import Find from "./Find";
import Tiers from "./Tiers";
import Gear from "./Gear";
import MyBuild from "./MyBuild";
import Maps from "./Maps";
import Wealth from "./Wealth";
import Bulk from "./Bulk";
import PlayTools from "./Play";
import Atlas from "./Atlas";
import Craft from "./Craft";
import Home from "./Home";
import Search from "./Search";
import PriceCheck from "./PriceCheck";
import { HOTKEYS, keyOf } from "./hotkeys";

/** Where the main window's look is kept (Settings writes it too). */
export const APP_LOOK_KEY = "app.look";

type Tab = "home" | "help" | "find" | "presets" | "tiers" | "gear" | "build" | "maps" | "atlas" | "wealth" | "bulk" | "pricecheck" | "craft" | "play" | "edits" | "prices" | "themes" | "batch" | "profiles" | "settings";

/** Small line icons for the side menu. */
/** Each page's colour: a tinted tile behind its menu icon (and page title) so the menu is easy to scan. */
const COLOURS: Record<Tab, string> = {
  find: "#6aa8ff", tiers: "#ff8a65", presets: "#b48cff", themes: "#ff7eb6", prices: "#5fd38a",
  edits: "#4fd1c5", batch: "#ffa95a",
  build: "#7ad0ff", gear: "#f0b45a",
  maps: "#8bd67a", atlas: "#a78bfa", wealth: "#f5c542", bulk: "#f5c542", pricecheck: "#f5c542", craft: "#ff9f6b", play: "#6fd0a6",
  profiles: "#9fb0c8", settings: "#9fb0c8", help: "#6aa8ff", home: "#e2b96f",
};

const ICONS: Record<Tab, string> = {
  home: "M4 11l8-7 8 7M6 9.5V20h4.5v-5.5h3V20H18V9.5",
  find: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm10 17-5.2-5.2",
  tiers: "M4 6h16M4 12h16M4 18h16M7 4v4M12 10v4M17 16v4",
  presets: "M5 4v16M12 4v16M19 4v16M3 8h4M10 15h4M17 10h4",
  themes: "M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.2-1.6-.4-1 .3-2 1.4-2H17a4 4 0 0 0 4-4c0-5.5-4-10.4-9-10.4zM7.5 11h.01M10 7h.01M15 7h.01",
  prices: "M12 3v18M16.5 7.5c0-1.7-2-3-4.5-3s-4.5 1.3-4.5 3 2 2.6 4.5 3 4.5 1.3 4.5 3-2 3-4.5 3-4.5-1.3-4.5-3",
  edits: "M4 6h12M4 12h16M4 18h9M18 16l3 3M17 3l4 4-9 9H8v-4z",
  batch: "M4 7h13v13H4zM8 3h12v12",
  gear: "M14 3l7 7-9 9-3-3-4 4-2-2 4-4-3-3 9-9zM14 3l-2 6 3 3 6-2",
  build: "M12 3v4M12 17v4M3 12h4M17 12h4M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM5.6 5.6l2.8 2.8M15.6 15.6l2.8 2.8M18.4 5.6l-2.8 2.8M8.4 15.6l-2.8 2.8",
  maps: "M3 6l6-2 6 2 6-2v14l-6 2-6-2-6 2zM9 4v14M15 6v14",
  atlas: "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 3v18M4 7.5l8 4.5 8-4.5",
  wealth: "M4 7h16v12H4zM4 7l2-3h12l2 3M9 12h6",
  pricecheck: "M3 12V4h8l10 10-8 8zM7.5 7.5h.01",
  bulk: "M4 9h16l-1.5 11h-13zM8 9l4-5 4 5M9 13v4M15 13v4",
  play: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
  craft: "M14 4l6 6-3 3-6-6zM11 7L4 14l2 2 7-7M6 16l-2 4 4-2M15 15l5 5",
  profiles: "M3 7h6l2 2h10v10H3zM3 7v12",
  settings: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15l1.6 1-2 3.4-1.8-.7a7 7 0 0 1-2 1.2L15 22h-4l-.3-2.1a7 7 0 0 1-2-1.2l-1.8.7-2-3.4 1.6-1a7 7 0 0 1 0-2.4l-1.6-1 2-3.4 1.8.7a7 7 0 0 1 2-1.2L11 2h4l.3 2.1a7 7 0 0 1 2 1.2l1.8-.7 2 3.4-1.6 1a7 7 0 0 1 0 2.4z",
  help: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17h.01",
};

type MenuItem = { id: Tab; name: string; hint: string };
/** The groups' colours (22 Nov, GROBBS picked "more, smaller groups" with a colour per kind of group):
 *  blue = the filter, amber = your changes, green = playing, purple = money. */
const G_FILTER = "#7fb2ea", G_CHANGES = "#e9b46a", G_PLAY = "#6fd0a6", G_MONEY = "#c9a3ea";
/** Small groups, each with a line saying what's in it; Settings and Help sit at the bottom. Wording and grouping from
 *  mock-up "B · Sorted by what you're doing" (22 Nov 2026): his build sits with the gear for it, the things used while
 *  playing are together, and the price check has its own page under Money. Each hint is also the page's subtitle and
 *  what Ctrl+K search matches, so it says plainly what's on the page. */
const MENU: { group: string; sub?: string; colour?: string; items: MenuItem[] }[] = [
  { group: "", items: [
    { id: "home", name: "Home", hint: "Your filter at a glance, recent changes and your in-game keys" },
  ] },
  { group: "Your loot filter", sub: "Which filter you use, how strict, and its colours", colour: G_FILTER, items: [
    { id: "profiles", name: "Filters", hint: "Which filter you're editing, how strict it is, and its NeverSink version" },
    { id: "presets", name: "Presets", hint: "Ready-made changes: levelling, your build's weapons and armour, league mechanics" },
    { id: "themes", name: "Themes", hint: "Recolour the whole filter in one go" },
  ] },
  { group: "How items look", sub: "Change any item, NeverSink's tiers, or colour by price", colour: G_FILTER, items: [
    { id: "find", name: "Change an item", hint: "The item you picked in Search (Ctrl+F): change how it drops, or describe a change in your own words" },
    { id: "tiers", name: "Tiers", hint: "NeverSink's ranked item lists: move items up or down, hide them, change their look" },
    { id: "prices", name: "Price tiers", hint: "Make valuable items stand out using poe.ninja prices (off until you turn it on)" },
  ] },
  { group: "Your changes", sub: "Everything you've changed. F2 and F3 change items in game", colour: G_CHANGES, items: [
    { id: "edits", name: "My edits", hint: "Everything you've changed: undo it, see the history, preview drops" },
    { id: "batch", name: "Batch mode", hint: "Collect items in game (F3, then Ctrl+C on each) and change them all at once" },
  ] },
  { group: "Build and gear", sub: "Your build at every stage, Act 1 to endgame, and gear for it", colour: G_PLAY, items: [
    { id: "build", name: "My build", hint: "Your Path of Building build: gems, gear and tree for every stage, Act 1 to endgame" },
    { id: "gear", name: "Unique finder", hint: "Uniques you can wear at your level that suit your skill, cheapest first, with trade links" },
    { id: "craft", name: "Crafting", hint: "What can roll on an item, and the cheapest way to get the mods you want" },
  ] },
  { group: "While you play", sub: "Windows over the game, maps you can run, your atlas tree", colour: G_PLAY, items: [
    { id: "play", name: "Game windows", hint: "Windows over the game: levelling route (F8), map tracker (F9), searches (F10), cheat sheets (F11), notes" },
    { id: "maps", name: "Map mods", hint: "Map mods your build can't run, a stash search for good maps, and maps to buy" },
    { id: "atlas", name: "Atlas tree", hint: "Atlas tree plans from maxroll or pathofexile.com, over the game with F7" },
  ] },
  { group: "Money", sub: "What items sell for, what your stash is worth, what you earn", colour: G_MONEY, items: [
    { id: "pricecheck", name: "Price check", hint: "What an item sells for: hover it in game and press Ctrl+D. Plus your saved and live searches" },
    { id: "wealth", name: "Stash worth", hint: "What your stash is worth, what to sell, and what each farming strategy earns" },
    { id: "bulk", name: "Sell in bulk", hint: "Sell lots of one kind (currency, essences, cards, lifeforce…): the price to type in game, the total and a Discord post" },
  ] },
];
/** A page's colour: its group's colour, or its own (Home, Settings, Help). */
const tabColour = (t: Tab) => MENU.find((g) => g.items.some((i) => i.id === t))?.colour ?? COLOURS[t];
const BOTTOM: MenuItem[] = [
  { id: "settings", name: "Settings", hint: "Hotkeys, price check, trading, maps, looks and AI" },
  { id: "help", name: "Help", hint: "Quick answers, and what each hotkey does" },
];
const ALL: MenuItem[] = [...MENU.flatMap((g) => g.items), ...BOTTOM];

function Icon({ d }: { d: string }) {
  return <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>;
}

/** After any change to the filter (from any page): a short note at the bottom that it's saved, what to press in game,
 *  and Undo. Fades away by itself after a few seconds (stays while the mouse is on it). */
function SavedNote({ reloadKey, onUndo }: { reloadKey: string; onUndo: () => void }) {
  const [note, setNote] = useState<{ what: string; n: number } | null>(null);
  const [undone, setUndone] = useState("");
  const [hover, setHover] = useState(false);
  useEffect(() => {
    const un = listen<string>("filter-changed", (e) => { setUndone(""); setNote((p) => ({ what: e.payload, n: (p?.n ?? 0) + 1 })); });
    return () => { un.then((f) => f()); };
  }, []);
  useEffect(() => {
    if (!note || hover) return;
    const t = setTimeout(() => setNote(null), undone ? 2500 : 6000);
    return () => clearTimeout(t);
  }, [note, hover, undone]);
  if (!note) return null;
  return (
    <div key={note.n} className="saved-note" role="status" onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}>
      <span className="saved-tick" aria-hidden="true">✓</span>
      {undone ? <span>Undone: {undone}</span> : <>
        <span><b>Saved</b> <span className="saved-what">{note.what}</span></span>
        <span className="muted">Press <kbd>{reloadKey}</kbd> in game to see it</span>
        <button className="link" onClick={() => invoke<string | null>("undo_last").then((w) => { setUndone(w ?? note.what); onUndo(); }).catch(() => {})}>Undo</button>
      </>}
      <button className="link saved-x" aria-label="Close" onClick={() => setNote(null)}>×</button>
    </div>
  );
}

type UpdateInfo = { version: string; current: string; notes: string };

/** A new version was downloaded in the background: it only installs when he clicks (never mid-game by itself). */
function UpdateBar() {
  const [u, setU] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  useEffect(() => {
    invoke<UpdateInfo | null>("update_status").then(setU).catch(() => {});
    const a = listen<UpdateInfo>("update-ready", (e) => setU(e.payload));
    return () => { a.then((f) => f()); };
  }, []);
  if (!u) return null;
  return (
    <div className="update-bar">
      <span><b>Version {u.version} is ready</b> <span className="muted small">(you have {u.current})</span>
        {u.notes && <span className="muted small"> · {u.notes}</span>}</span>
      <button className="primary" disabled={busy} onClick={() => { setBusy(true); setErr("");
        invoke("update_install").catch((e) => { setErr(friendly(e)); setBusy(false); }); }}>
        {busy ? "Updating…" : "Restart to update"}</button>
      {err && <span className="danger-text small">{err}</span>}
    </div>
  );
}

/** Loot isn't being counted because the pathofexile.com login has run out (24 Nov: 17 maps went by without it being noticed). */
function LoginBar() {
  const [note, setNote] = useState("");
  useEffect(() => {
    const take = (v: { tracker: { loot_note: string } } | null) => setNote(v?.tracker?.loot_note ?? "");
    invoke<{ tracker: { loot_note: string } }>("play_view").then(take).catch(() => {});
    const a = listen<{ tracker: { loot_note: string } }>("play-update", (e) => take(e.payload));
    const b = listen<{ ok: boolean }>("poe-login", (e) => { if (e.payload.ok) setNote(""); });
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, []);
  if (!needsLogin(note)) return null;
  return (
    <div className="update-bar login-bar">
      <span><b>Your pathofexile.com login has run out</b> <span className="muted small">Map loot isn't being counted until you log in again.</span></span>
      <PoeLogin big label="Log in again" onDone={() => setNote("")} />
    </div>
  );
}

export default function App() {
  const [ov, setOv] = useState<Overview | null>(null);
  // The editor always opens on Home; Alt+← takes you back to the page you were on last time.
  const [tab, setTabRaw] = useState<Tab>("home");
  const [searching, setSearching] = useState(false);
  /** An item picked in the Ctrl+K search, for the Find page to show. */
  const [findPick, setFindPick] = useState<{ base_type: string; item_class: string } | null>(null);
  /** A change typed in the search box ("hide normal 2h swords"), for the Change an item page to work out. */
  const [askStart, setAskStart] = useState<{ text: string; n: number } | null>(null);
  /** A help question picked in the search, for Help to open. */
  const [helpStart, setHelpStart] = useState(0);
  /** The main window's look (Settings → Main window look): "colourful" (a colour per page) or "calm" (plain icons, gold for
   *  the page you're on). Kept in localStorage; Settings tells this window with an "app-look" event. */
  const [look, setLook] = useState<string>(() => { try { return localStorage.getItem(APP_LOOK_KEY) || "colourful"; } catch { return "colourful"; } });
  useEffect(() => {
    // Tooltip Gold and Mono Amber sit on the Calm layout and add their own colours.
    document.body.classList.toggle("look-calm", ["calm", "tooltip", "mono"].includes(look));
    document.body.classList.toggle("look-tooltip", look === "tooltip");
    document.body.classList.toggle("look-mono", look === "mono");
    const on = (e: Event) => setLook((e as CustomEvent<string>).detail);
    window.addEventListener("app-look", on);
    return () => window.removeEventListener("app-look", on);
  }, [look]);
  /** Bumped by the Saved note's Undo: the open page reloads so it shows the filter as it is now. */
  const [undoN, setUndoN] = useState(0);
  const prevTab = useRef<Tab | null>((() => {
    try { const t = localStorage.getItem("tab") as Tab | null; return t && t !== "home" && ALL.some((i) => i.id === t) ? t : null; } catch { return null; }
  })());
  const setTab = (t: Tab) => {
    setTabRaw((cur) => { if (cur !== t) prevTab.current = cur; return t; });
    try { localStorage.setItem("tab", t); } catch { /* not kept */ }
  };
  const [error, setError] = useState("");
  /** "Modify rule" from the quick menu: which rule to show. */
  const [focus, setFocus] = useState<Source | null>(null);
  const [adding, setAdding] = useState(false);
  /** In-game hotkeys from Settings, shown next to their pages. */
  const [keys, setKeys] = useState<Record<string, string>>({});
  useEffect(() => {
    const load = () => invoke<Record<string, unknown>>("get_settings").then((s) => setKeys(Object.fromEntries(Object.entries(s).filter(([k, v]) => k.startsWith("key_") && typeof v === "string") as [string, string][]))).catch(() => {});
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [tab]);

  const refresh = useCallback(async () => {
    try { setOv(await invoke<Overview>("overview")); } catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => {
    refresh();
    const a = listen("batch-mode", (e) => { if (e.payload) setTab("batch"); refresh(); });
    const c = listen("active-changed", () => refresh());
    const d = listen("neversink-updated", () => refresh());
    // "Open the editor" in a window over the game: show that page.
    const o = listen<string>("open-tab", (e) => setTab(e.payload as Tab));
    const g = listen<Source>("goto-rule", (e) => {
      const src = e.payload;
      setTab(src.kind === "neversink" ? "tiers" : src.kind === "edit" ? "edits" : src.kind === "preset" ? "presets" : src.kind === "price" ? "prices" : "find");
      setFocus({ ...src });
      refresh();
    });
    // Other pages can send you somewhere (e.g. Settings → "Open the Maps page").
    const onGoto = (e: Event) => { const t = (e as CustomEvent<string>).detail as Tab; if (t) setTab(t); };
    window.addEventListener("goto-tab", onGoto);
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => { a.then((f) => f()); c.then((f) => f()); d.then((f) => f()); o.then((f) => f()); g.then((f) => f()); window.removeEventListener("focus", onFocus); window.removeEventListener("goto-tab", onGoto); };
  }, [refresh]);

  // Keyboard: Ctrl+F opens the one search box (Ctrl+K too; 23 Nov he asked for Ctrl+F); Ctrl+1…9 opens the menu's pages in order;
  // Alt+← goes back to the page you were on before.
  useEffect(() => {
    const pages = MENU.flatMap((g) => g.items.map((i) => i.id));
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && (e.key.toLowerCase() === "f" || e.key.toLowerCase() === "k")) {
        e.preventDefault();
        setSearching((s) => !s);
      } else if (e.ctrlKey && !e.altKey && /^[1-9]$/.test(e.key) && pages[+e.key - 1]) {
        e.preventDefault();
        setTab(pages[+e.key - 1]);
      } else if (e.altKey && e.key === "ArrowLeft" && prevTab.current) {
        e.preventDefault();
        setTab(prevTab.current);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Each page comes back scrolled where you left it (also after a restart).
  useEffect(() => {
    const el = document.scrollingElement as HTMLElement | null;
    if (!el || !ov) return;
    const key = `scroll.${tab}`;
    let saved = 0;
    try { saved = +(localStorage.getItem(key) ?? 0); } catch { /* none */ }
    // Pages fill in after their data arrives: put the scroll back a few times while they settle.
    const timers = [0, 150, 500, 1200].map((ms) => setTimeout(() => { if (saved && el.scrollTop < 5) el.scrollTop = saved; }, ms));
    let t = 0;
    const onScroll = () => { clearTimeout(t); t = window.setTimeout(() => { try { localStorage.setItem(key, String(Math.round(el.scrollTop))); } catch { /* not kept */ } }, 200); };
    window.addEventListener("scroll", onScroll);
    return () => { timers.forEach(clearTimeout); clearTimeout(t); window.removeEventListener("scroll", onScroll); };
  }, [tab, !!ov]);

  if (!ov) return error ? <div className="page">{error}</div> : <LoadingShell />;
  if (ov.profiles.length === 0 || adding) {
    return <Setup ov={ov} onDone={() => { setAdding(false); refresh(); }} onCancel={ov.profiles.length ? () => setAdding(false) : undefined} />;
  }

  const active = ov.profiles.find((p) => p.id === ov.active) ?? ov.profiles[0];

  const current = ALL.find((i) => i.id === tab);
  const pages = MENU.flatMap((g) => g.items.map((i) => i.id));
  // The in-game key for a page (from Settings): the only key shown in the menu, to keep it calm. Hover = how it works.
  const KEY_OF_PAGE: Partial<Record<Tab, string>> = { edits: "key_quick_edit", batch: "key_batch", build: "key_build",
    atlas: "key_atlas", pricecheck: "key_price", profiles: "key_reload", play: "key_level" };
  const gameKey = (t: Tab): [string, string] | null => {
    const h = HOTKEYS.find((x) => x.setting === KEY_OF_PAGE[t]);
    return h ? [keyOf(keys, h.setting), `In game: ${h.name}. ${h.how}`] : null;
  };
  const item = (i: MenuItem) => {
    const n = pages.indexOf(i.id) + 1;
    const here = n >= 1 && n <= 9 ? `Ctrl+${n}` : "";
    const g = gameKey(i.id);
    return (
      <button key={i.id} className={`menu-item ${tab === i.id ? "on" : ""}`} title={here ? `${i.hint} (${here} in the editor)` : i.hint} onClick={() => setTab(i.id)}>
        <span className="menu-ico" style={{ "--c": tabColour(i.id) } as CSSProperties}><Icon d={ICONS[i.id]} /></span><span>{i.name}</span>
        {i.id === "batch" && ov.batch_on && <span className="menu-dot" title="Batch mode is on" />}
        {g && <span className="menu-keys"><span className="menu-key game" title={g[1]}>{g[0]}</span></span>}
      </button>
    );
  };
  return (
    <>
    <MainTitleBar />
    <div className="shell">
      <aside className="side">
        <div className="brand">PoE Lodestone</div>
        <label className="side-label">Editing</label>
        <select className="profile-pick" value={active.id}
          onChange={async (e) => { await invoke("set_active", { id: e.target.value }); refresh(); }}>
          {ov.profiles.map((p) => (
            <option key={p.id} value={p.id}>{p.name} ({p.game === "Poe1" ? "PoE 1" : "PoE 2"})</option>
          ))}
        </select>
        <button className="side-search" onClick={() => setSearching(true)} title="Find any item, Tiers rule, page or help answer, or type a change in your own words">
          <Icon d={ICONS.find} /><span>Search</span><kbd>Ctrl+F</kbd>
        </button>
        <nav className="menu">
          {MENU.map((g) => (
            <div key={g.group || "top"} className={`menu-group ${g.colour ? "has-g" : ""}`} style={g.colour ? { "--g": g.colour } as CSSProperties : undefined}>
              {g.group && <div className="menu-gname">{g.group}</div>}
              {g.sub && <div className="menu-gsub">{g.sub}</div>}
              {g.items.map(item)}
            </div>
          ))}
        </nav>
        <div className="menu-bottom">
          {BOTTOM.map(item)}
          <div className="side-foot muted small" title="Alt+← goes back to the last page"><span className="menu-key game">{keyOf(keys, "key_quick_edit")}</span> Gold keys work in game</div>
        </div>
      </aside>

      <main className={`content ${tab === "craft" ? "wide" : ""}`}>
        <UpdateBar />
        <LoginBar />
        <ErrorBoundary key={tab + undoN}>
        <div key={tab} className="page-in">
        <div className="content-head">
          <h1><span className="head-icon" style={{ "--c": tabColour(tab) } as CSSProperties}><Icon d={ICONS[tab]} /></span>{current?.name}</h1>
          <div className="muted small">{current?.hint}</div>
        </div>
      {tab === "edits" && <Edits profile={active} onChange={refresh} focus={focus?.kind === "edit" ? focus : null} />}
      {tab === "prices" && <Prices profile={active} />}
      {tab === "themes" && <Themes profile={active} onChange={refresh} />}
      {tab === "settings" && <SettingsPage />}
      {tab === "home" && <Home profile={active} keys={keys} go={(t) => setTab(t as Tab)} search={() => setSearching(true)} />}
      {tab === "find" && <Find onChange={refresh} pick={findPick} ask={askStart} openSearch={() => setSearching(true)} />}
      {tab === "help" && <Help key={helpStart} start={helpStart} keys={keys} go={(t) => setTab(t as Tab)} />}
      {tab === "gear" && <Gear />}
      {tab === "build" && <MyBuild />}
      {tab === "maps" && <Maps />}
      {tab === "atlas" && <Atlas />}
      {tab === "wealth" && <Wealth />}
      {tab === "bulk" && <Bulk />}
      {tab === "pricecheck" && <PriceCheck keys={keys} />}
      {tab === "play" && <PlayTools />}
      {tab === "craft" && <Craft />}
      {tab === "tiers" && <Tiers profile={active} onChange={refresh} focus={focus?.kind === "neversink" ? focus : null} />}
      {tab === "presets" && <Find key="presets" view="presets" onChange={refresh} />}
      {tab === "batch" && <Batch on={ov.batch_on} onChange={refresh} />}
      {tab === "profiles" && <Profiles ov={ov} active={active} onChange={refresh} onNew={() => setAdding(true)} />}
        </div>
        </ErrorBoundary>
      </main>
    </div>
    <SavedNote reloadKey={keys.key_reload ?? "F4"} onUndo={() => { refresh(); setUndoN((n) => n + 1); }} />
    {searching && <Search pages={ALL} onClose={() => setSearching(false)}
      goPage={(id) => setTab(id as Tab)}
      goHelp={(i) => { setHelpStart(i); setTab("help"); }}
      goItem={(f) => { setFindPick({ ...f }); setTab("find"); }}
      goAsk={(text) => { setAskStart({ text, n: Date.now() }); setTab("find"); }}
      profileId={active?.id}
      goTier={(h) => { setTab("tiers"); setFocus({ kind: "neversink", label: h.title, type_path: h.type_path, tier: h.tier, section: null, tier_name: null }); }} />}
    </>
  );
}

/** While the app starts: the shape of the page, gently shimmering, instead of a blank "Loading…". */
function LoadingShell() {
  return (
    <div className="skel-shell">
      <aside className="side">
        <span className="skel" style={{ height: 18, width: "70%", margin: "0 8px 10px" }} />
        <span className="skel" style={{ height: 34 }} />
        {Array.from({ length: 9 }, (_, i) => <span key={i} className="skel" style={{ height: 26, opacity: 1 - i * 0.07 }} />)}
      </aside>
      <main className="content">
        <span className="skel" style={{ height: 30, width: 220 }} />
        <span className="skel" style={{ height: 14, width: 340 }} />
        <span className="skel" style={{ height: 150, borderRadius: 14 }} />
        <span className="skel" style={{ height: 220, borderRadius: 14 }} />
      </main>
    </div>
  );
}
