import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import AtlasTreeView, { AtlasStore, StrategyNotes, currentStrategy } from "./AtlasTreeView";

/** The atlas strategy window over the game (F7): tree and notes. Opens without taking the game's focus. */
export default function AtlasOverlay() {
  const [store, setStore] = useState<AtlasStore | null>(null);
  const [view, setView] = useState<"tree" | "notes">(() => { try { return localStorage.getItem("atlas-view") === "notes" ? "notes" : "tree"; } catch { return "tree"; } });
  const [key, setKey] = useState("F7");
  const [h, setH] = useState(window.innerHeight);
  useEffect(() => { const r = () => setH(window.innerHeight); window.addEventListener("resize", r); return () => window.removeEventListener("resize", r); }, []);

  const load = () => {
    invoke<AtlasStore>("atlas_list").then(setStore).catch(() => setStore(null));
    invoke<{ key_atlas?: string }>("get_settings").then((s) => setKey(s.key_atlas || "F7")).catch(() => {});
  };
  useEffect(() => {
    load();
    const un = listen("atlas-changed", load);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("hide_atlas_window"); };
    window.addEventListener("keydown", onKey);
    return () => { un.then((f) => f()); window.removeEventListener("keydown", onKey); };
  }, []);
  const pick = (v: "tree" | "notes") => { setView(v); try { localStorage.setItem("atlas-view", v); } catch { /* fine */ } };

  const s = currentStrategy(store);
  const vi = s && store ? Math.min(store.active === s.id ? store.variant : 0, s.variants.length - 1) : 0;
  const v = s?.variants[vi];

  return (
    <div className="bo ao">
      <div className="bo-bar" onMouseDown={(e) => { if (e.button === 0 && (e.target as HTMLElement).closest("button,select") == null) getCurrentWindow().startDragging(); }}>
        <span className="ao-name" title={s?.name}>{s ? s.name : "Atlas strategy"}</span>
        <span className="muted small">drag · {key} / Esc hides</span>
        <button className="link" onClick={() => invoke("hide_atlas_window")}>✕</button>
      </div>
      {!s ? (
        <div className="bo-body"><div className="empty">No atlas strategy yet. In the editor go to <b>Endgame → Atlas</b> and paste a maxroll or pathofexile.com link.</div></div>
      ) : (
        <>
          <div className="ao-head">
            {store && store.list.length > 1 && (
              <select value={s.id} onChange={(e) => invoke<AtlasStore>("atlas_use", { id: Number(e.target.value), variant: 0 }).then(setStore)}>
                {store.list.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
              </select>
            )}
            {s.variants.length > 1 && (
              <select value={vi} onChange={(e) => invoke<AtlasStore>("atlas_use", { id: s.id, variant: Number(e.target.value) }).then(setStore)}>
                {s.variants.map((x, i) => <option key={i} value={i}>{x.name}</option>)}
              </select>
            )}
            <span className="grow" />
            <div className="seg">
              <button className={view === "tree" ? "on" : ""} onClick={() => pick("tree")}>Tree</button>
              <button className={view === "notes" ? "on" : ""} onClick={() => pick("notes")}>Notes</button>
            </div>
          </div>
          <div className="bo-body">
            {view === "tree" && v && <AtlasTreeView key={`${s.id}-${vi}`} nodes={v.nodes} choices={v.choices} height={Math.max(240, h - 140)} compact />}
            {view === "notes" && <StrategyNotes s={s} v={v} />}
          </div>
        </>
      )}
    </div>
  );
}
