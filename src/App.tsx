import { friendly } from "./errors";
import { useCallback, useEffect, useRef, useState, type CSSProperties } from "react";
import Help from "./Help";
import ErrorBoundary from "./ErrorBoundary";
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
import Atlas from "./Atlas";

type Tab = "help" | "find" | "presets" | "tiers" | "gear" | "build" | "maps" | "atlas" | "wealth" | "edits" | "prices" | "themes" | "batch" | "profiles" | "settings";

/** Small line icons for the side menu. */
/** Each page's colour: a tinted tile behind its menu icon (and page title) so the menu is easy to scan. */
const COLOURS: Record<Tab, string> = {
  find: "#6aa8ff", tiers: "#ff8a65", presets: "#b48cff", themes: "#ff7eb6", prices: "#5fd38a",
  edits: "#4fd1c5", batch: "#ffa95a",
  build: "#7ad0ff", gear: "#f0b45a",
  maps: "#8bd67a", atlas: "#a78bfa", wealth: "#f5c542",
  profiles: "#9fb0c8", settings: "#9fb0c8", help: "#6aa8ff",
};

const ICONS: Record<Tab, string> = {
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
  profiles: "M3 7h6l2 2h10v10H3zM3 7v12",
  settings: "M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM19.4 15l1.6 1-2 3.4-1.8-.7a7 7 0 0 1-2 1.2L15 22h-4l-.3-2.1a7 7 0 0 1-2-1.2l-1.8.7-2-3.4 1.6-1a7 7 0 0 1 0-2.4l-1.6-1 2-3.4 1.8.7a7 7 0 0 1 2-1.2L11 2h4l.3 2.1a7 7 0 0 1 2 1.2l1.8-.7 2 3.4-1.6 1a7 7 0 0 1 0 2.4z",
  help: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.5 9a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17h.01",
};

const MENU: { group: string; items: { id: Tab; name: string; hint: string }[] }[] = [
  { group: "Filter", items: [
    { id: "find", name: "Find", hint: "Search any item, or just say what you want" },
    { id: "tiers", name: "Tiers", hint: "NeverSink's tier lists: re-rank, hide and restyle" },
    { id: "presets", name: "Presets", hint: "Levelling setup, build switches and league mechanics" },
    { id: "themes", name: "Themes", hint: "Recolour the whole filter" },
    { id: "prices", name: "Prices", hint: "Optional poe.ninja price tiers" },
  ] },
  { group: "My changes", items: [
    { id: "edits", name: "My edits", hint: "Everything you've changed, history and preview" },
    { id: "batch", name: "Batch mode", hint: "Edit lots of items in a row" },
  ] },
  { group: "Levelling", items: [
    { id: "build", name: "My build", hint: "Your Path of Building gems, gear and tree, in game with F6" },
    { id: "gear", name: "Levelling gear", hint: "Uniques for your level and skill, with trade links" },
  ] },
  { group: "Endgame", items: [
    { id: "maps", name: "Maps", hint: "Map regex for your stash, and buy maps you can run" },
    { id: "atlas", name: "Atlas", hint: "Atlas strategies from maxroll or pathofexile.com, in game with F7" },
    { id: "wealth", name: "Wealth", hint: "Stash worth, things worth selling, earnings per strategy" },
  ] },
  { group: "App", items: [
    { id: "profiles", name: "Filters", hint: "Your filters, strictness and NeverSink version" },
    { id: "settings", name: "Settings", hint: "Hotkeys, area level and AI" },
    { id: "help", name: "Help", hint: "How do I…?" },
  ] },
];

function Icon({ d }: { d: string }) {
  return <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d={d} /></svg>;
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

export default function App() {
  const [ov, setOv] = useState<Overview | null>(null);
  const [tab, setTabRaw] = useState<Tab>(() => {
    try { return (localStorage.getItem("tab") as Tab) || "edits"; } catch { return "edits"; }
  });
  const prevTab = useRef<Tab | null>(null);
  const setTab = (t: Tab) => {
    setTabRaw((cur) => { if (cur !== t) prevTab.current = cur; return t; });
    try { localStorage.setItem("tab", t); } catch { /* not kept */ }
  };
  const [error, setError] = useState("");
  /** "Modify rule" from the quick menu: which rule to show. */
  const [focus, setFocus] = useState<Source | null>(null);
  const [adding, setAdding] = useState(false);

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

  // Keyboard: Ctrl+F jumps to Find and into its search box; Ctrl+1…9 opens the menu's pages in order;
  // Alt+← goes back to the page you were on before.
  useEffect(() => {
    const pages = MENU.flatMap((g) => g.items.map((i) => i.id));
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setTab("find");
        setTimeout(() => (document.querySelector("main.content input[type=search], main.content input.text, main.content input:not([type])") as HTMLInputElement | null)?.focus(), 80);
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

  const current = MENU.flatMap((g) => g.items).find((i) => i.id === tab);
  return (
    <div className="shell">
      <aside className="side">
        <div className="brand">PoE Filter Editor</div>
        <label className="side-label">Editing</label>
        <select className="profile-pick" value={active.id}
          onChange={async (e) => { await invoke("set_active", { id: e.target.value }); refresh(); }}>
          {ov.profiles.map((p) => (
            <option key={p.id} value={p.id}>{p.name} ({p.game === "Poe1" ? "PoE 1" : "PoE 2"})</option>
          ))}
        </select>
        <nav className="menu">
          {MENU.map((g) => (
            <div key={g.group} className="menu-group">
              <div className="menu-gname">{g.group}</div>
              {g.items.map((i) => (
                <button key={i.id} className={`menu-item ${tab === i.id ? "on" : ""}`} title={i.hint} onClick={() => setTab(i.id)}>
                  <span className="menu-ico" style={{ "--c": COLOURS[i.id] } as CSSProperties}><Icon d={ICONS[i.id]} /></span><span>{i.name}</span>
                  {i.id === "batch" && ov.batch_on && <span className="menu-dot" title="Batch mode is on" />}
                  {i.id === "find" && <span className="menu-key" title="Press Ctrl+F anywhere in the editor">Ctrl+F</span>}
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="side-foot muted small">In game: <b>F2</b> edit item · <b>F4</b> reload · <b>F6</b> build<br />Here: <b>Ctrl+F</b> find · <b>Ctrl+1–9</b> pages · <b>Alt+←</b> back</div>
      </aside>

      <main className="content">
        <UpdateBar />
        <ErrorBoundary key={tab}>
        <div key={tab} className="page-in">
        <div className="content-head">
          <h1><span className="head-icon" style={{ "--c": COLOURS[tab] } as CSSProperties}><Icon d={ICONS[tab]} /></span>{current?.name}</h1>
          <div className="muted small">{current?.hint}</div>
        </div>
      {tab === "edits" && <Edits profile={active} onChange={refresh} focus={focus?.kind === "edit" ? focus : null} />}
      {tab === "prices" && <Prices profile={active} />}
      {tab === "themes" && <Themes profile={active} onChange={refresh} />}
      {tab === "settings" && <SettingsPage />}
      {tab === "find" && <Find onChange={refresh} />}
      {tab === "help" && <Help go={(t) => setTab(t as Tab)} />}
      {tab === "gear" && <Gear />}
      {tab === "build" && <MyBuild />}
      {tab === "maps" && <Maps />}
      {tab === "atlas" && <Atlas />}
      {tab === "wealth" && <Wealth />}
      {tab === "tiers" && <Tiers profile={active} onChange={refresh} focus={focus?.kind === "neversink" ? focus : null} />}
      {tab === "presets" && <Find key="presets" view="presets" onChange={refresh} />}
      {tab === "batch" && <Batch on={ov.batch_on} onChange={refresh} />}
      {tab === "profiles" && <Profiles ov={ov} active={active} onChange={refresh} onNew={() => setAdding(true)} />}
        </div>
        </ErrorBoundary>
      </main>
    </div>
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
