import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Overview } from "./types";
import Setup from "./Setup";
import Profiles from "./Profiles";
import Edits from "./Edits";
import Batch from "./Batch";
import Prices from "./Prices";

type Tab = "edits" | "prices" | "batch" | "profiles";

export default function App() {
  const [ov, setOv] = useState<Overview | null>(null);
  const [tab, setTab] = useState<Tab>("edits");
  const [error, setError] = useState("");

  const refresh = useCallback(async () => {
    try { setOv(await invoke<Overview>("overview")); } catch (e) { setError(String(e)); }
  }, []);

  useEffect(() => {
    refresh();
    const a = listen("batch-mode", (e) => { if (e.payload) setTab("batch"); refresh(); });
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    return () => { a.then((f) => f()); window.removeEventListener("focus", onFocus); };
  }, [refresh]);

  if (!ov) return <div className="page">{error || "Loading…"}</div>;
  if (ov.profiles.length === 0) return <Setup ov={ov} onDone={refresh} />;

  const active = ov.profiles.find((p) => p.id === ov.active) ?? ov.profiles[0];

  return (
    <div className="page">
      <header className="top">
        <div>
          <div className="brand">PoE Filter Editor</div>
          <div className="muted">F2 on an item = quick edit · F3 = batch mode · F4 = reload filter</div>
        </div>
        <select className="profile-pick" value={active.id}
          onChange={async (e) => { await invoke("set_active", { id: e.target.value }); refresh(); }}>
          {ov.profiles.map((p) => (
            <option key={p.id} value={p.id}>{p.name} ({p.game === "Poe1" ? "PoE 1" : "PoE 2"})</option>
          ))}
        </select>
      </header>

      <nav className="tabs">
        {(["edits", "prices", "batch", "profiles"] as Tab[]).map((t) => (
          <button key={t} className={tab === t ? "tab on" : "tab"} onClick={() => setTab(t)}>
            {{ edits: "My edits", prices: "Prices", batch: "Batch mode", profiles: "Profiles" }[t]}
          </button>
        ))}
      </nav>

      {tab === "edits" && <Edits profile={active} onChange={refresh} />}
      {tab === "prices" && <Prices profile={active} />}
      {tab === "batch" && <Batch on={ov.batch_on} onChange={refresh} />}
      {tab === "profiles" && <Profiles ov={ov} active={active} onChange={refresh} />}
    </div>
  );
}
