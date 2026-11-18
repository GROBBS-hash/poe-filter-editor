import { useCallback, useEffect, useState } from "react";
import Help from "./Help";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Overview } from "./types";
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

type Tab = "help" | "find" | "presets" | "tiers" | "gear" | "edits" | "prices" | "themes" | "batch" | "profiles" | "settings";

/** Small line icons for the side menu. */
const ICONS: Record<Tab, string> = {
  find: "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm10 17-5.2-5.2",
  tiers: "M4 6h16M4 12h16M4 18h16M7 4v4M12 10v4M17 16v4",
  presets: "M5 4v16M12 4v16M19 4v16M3 8h4M10 15h4M17 10h4",
  themes: "M12 3a9 9 0 1 0 0 18c1 0 1.5-.8 1.2-1.6-.4-1 .3-2 1.4-2H17a4 4 0 0 0 4-4c0-5.5-4-10.4-9-10.4zM7.5 11h.01M10 7h.01M15 7h.01",
  prices: "M12 3v18M16.5 7.5c0-1.7-2-3-4.5-3s-4.5 1.3-4.5 3 2 2.6 4.5 3 4.5 1.3 4.5 3-2 3-4.5 3-4.5-1.3-4.5-3",
  edits: "M4 6h12M4 12h16M4 18h9M18 16l3 3M17 3l4 4-9 9H8v-4z",
  batch: "M4 7h13v13H4zM8 3h12v12",
  gear: "M14 3l7 7-9 9-3-3-4 4-2-2 4-4-3-3 9-9zM14 3l-2 6 3 3 6-2",
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
    { id: "gear", name: "Levelling gear", hint: "Uniques for your level and skill, with trade links" },
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

export default function App() {
  const [ov, setOv] = useState<Overview | null>(null);
  const [tab, setTabRaw] = useState<Tab>(() => {
    try { return (localStorage.getItem("tab") as Tab) || "edits"; } catch { return "edits"; }
  });
  const setTab = (t: Tab) => { setTabRaw(t); try { localStorage.setItem("tab", t); } catch { /* not kept */ } };
  const [error, setError] = useState("");
  const [adding, setAdding] = useState(false);

  const refresh = useCallback(async () => {
    try { setOv(await invoke<Overview>("overview")); } catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => {
    refresh();
    const a = listen("batch-mode", (e) => { if (e.payload) setTab("batch"); refresh(); });
    const c = listen("active-changed", () => refresh());
    const d = listen("neversink-updated", () => refresh());
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => { a.then((f) => f()); c.then((f) => f()); d.then((f) => f()); window.removeEventListener("focus", onFocus); };
  }, [refresh]);

  if (!ov) return <div className="page">{error || "Loading…"}</div>;
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
                  <Icon d={ICONS[i.id]} /><span>{i.name}</span>
                  {i.id === "batch" && ov.batch_on && <span className="menu-dot" title="Batch mode is on" />}
                </button>
              ))}
            </div>
          ))}
        </nav>
        <div className="side-foot muted small">In game: <b>F2</b> edit item · <b>F4</b> reload</div>
      </aside>

      <main className="content">
        <div className="content-head">
          <h1>{current?.name}</h1>
          <div className="muted small">{current?.hint}</div>
        </div>
      {tab === "edits" && <Edits profile={active} onChange={refresh} />}
      {tab === "prices" && <Prices profile={active} />}
      {tab === "themes" && <Themes profile={active} onChange={refresh} />}
      {tab === "settings" && <SettingsPage />}
      {tab === "find" && <Find onChange={refresh} />}
      {tab === "help" && <Help go={(t) => setTab(t as Tab)} />}
      {tab === "gear" && <Gear />}
      {tab === "tiers" && <Tiers profile={active} onChange={refresh} />}
      {tab === "presets" && <Find key="presets" view="presets" onChange={refresh} />}
      {tab === "batch" && <Batch on={ov.batch_on} onChange={refresh} />}
      {tab === "profiles" && <Profiles ov={ov} active={active} onChange={refresh} onNew={() => setAdding(true)} />}
      </main>
    </div>
  );
}
