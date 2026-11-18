import { useCallback, useEffect, useState } from "react";
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

type Tab = "find" | "presets" | "edits" | "prices" | "themes" | "batch" | "profiles" | "settings";

export default function App() {
  const [ov, setOv] = useState<Overview | null>(null);
  const [tab, setTab] = useState<Tab>("edits");
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

  return (
    <div className="page">
      <header className="top">
        <div>
          <div className="brand">PoE Filter Editor</div>
          <div className="muted">Hover an item in game and press your quick-edit key (F2 unless changed in Settings)</div>
        </div>
        <select className="profile-pick" value={active.id}
          onChange={async (e) => { await invoke("set_active", { id: e.target.value }); refresh(); }}>
          {ov.profiles.map((p) => (
            <option key={p.id} value={p.id}>{p.name} ({p.game === "Poe1" ? "PoE 1" : "PoE 2"})</option>
          ))}
        </select>
      </header>

      <nav className="tabs">
        {(["find", "presets", "edits", "prices", "themes", "batch", "profiles", "settings"] as Tab[]).map((t) => (
          <button key={t} className={tab === t ? "tab on" : "tab"} onClick={() => setTab(t)}>
            {{ find: "Find", presets: "Presets", edits: "My edits", prices: "Prices", themes: "Themes", batch: "Batch mode", profiles: "Profiles", settings: "Settings" }[t]}
          </button>
        ))}
      </nav>

      {tab === "edits" && <Edits profile={active} onChange={refresh} />}
      {tab === "prices" && <Prices profile={active} />}
      {tab === "themes" && <Themes profile={active} onChange={refresh} />}
      {tab === "settings" && <SettingsPage />}
      {tab === "find" && <Find onChange={refresh} />}
      {tab === "presets" && <Find key="presets" view="presets" onChange={refresh} />}
      {tab === "batch" && <Batch on={ov.batch_on} onChange={refresh} />}
      {tab === "profiles" && <Profiles ov={ov} active={active} onChange={refresh} onNew={() => setAdding(true)} />}
    </div>
  );
}
