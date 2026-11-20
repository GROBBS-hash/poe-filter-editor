import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { ResizeGrips, WinBar } from "./WinFrame";
import AtlasTreeView, { AtlasStore, ChoiceList, StepBar, StepList, StrategyNotes, currentStrategy, useStep } from "./AtlasTreeView";

type View = "tree" | "notes";

/** Size of an element, kept up to date as the window is resized. */
function useSize<T extends HTMLElement>(): [React.RefObject<T | null>, { w: number; h: number }] {
  const ref = useRef<T | null>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => { const r = el.getBoundingClientRect(); setSize({ w: r.width, h: r.height }); });
    ro.observe(el);
    return () => ro.disconnect();
  });
  return [ref, size];
}

/**
 * The atlas strategy window over the game (F7): steps on the left, the tree up to the chosen step on the
 * right, notes on their own tab. Nothing scrolls the whole window: the tree fits the space, lists scroll in place.
 * Opens without taking the game's focus; remembers its size and place (atlas_app.rs).
 */
export default function AtlasOverlay() {
  const [store, setStore] = useState<AtlasStore | null>(null);
  const [view, setView] = useState<View>(() => { try { return localStorage.getItem("atlas-view") === "notes" ? "notes" : "tree"; } catch { return "tree"; } });
  const [key, setKey] = useState("F7");
  const [treeBox, treeSize] = useSize<HTMLDivElement>();
  const [body, bodySize] = useSize<HTMLDivElement>();

  const load = () => {
    invoke<AtlasStore>("atlas_list").then(setStore).catch(() => setStore(null));
    invoke<{ key_atlas?: string }>("get_settings").then((s) => setKey(s?.key_atlas || "F7")).catch(() => {});
  };
  useEffect(() => {
    load();
    const un = listen("atlas-changed", load);
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("hide_atlas_window"); };
    window.addEventListener("keydown", onKey);
    return () => { un.then((f) => f()); window.removeEventListener("keydown", onKey); };
  }, []);
  const pick = (v: View) => { setView(v); try { localStorage.setItem("atlas-view", v); } catch { /* fine */ } };

  const s = currentStrategy(store);
  const vi = s && store ? Math.max(0, Math.min(store.active === s.id ? store.variant : 0, s.variants.length - 1)) : 0;
  const v = s?.variants[vi];
  const steps = v?.steps ?? [];
  const [step, setStep] = useStep(`${s?.id ?? 0}-${vi}`, steps.length);
  const [find, setFind] = useState<{ id: number; n: number } | null>(null);
  const finder = (id: number) => setFind((f) => ({ id, n: (f?.n ?? 0) + 1 }));
  // Narrow window: no room for the list beside the tree (the arrows still step through).
  const side = steps.length > 0 && bodySize.w >= 560;

  return (
    <div className="bo ao">
      <WinBar title={<span title={s?.name}>{s ? s.name : "Atlas strategy"}</span>} hint={`drag to move · Esc / ${key} to hide`}
        onClose={() => invoke("hide_atlas_window")} />
      {!s ? (
        <div className="ao-empty"><div className="empty">No atlas strategy yet. Paste a maxroll or pathofexile.com link on the Atlas page.<br />
          <button className="primary empty-go" onClick={() => invoke("open_editor", { tab: "atlas" })}>Open the Atlas page</button></div></div>
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
            <div className="ao-tabs">
              <button className={view === "tree" ? "on" : ""} onClick={() => pick("tree")}>Tree</button>
              <button className={view === "notes" ? "on" : ""} onClick={() => pick("notes")}>Notes</button>
            </div>
          </div>
          <div className="ao-body" ref={body}>
            {view === "notes" ? (
              <div className="ao-scroll ao-notes"><StrategyNotes s={s} v={v} /></div>
            ) : (
              <div className={`ao-split ${side ? "" : "solo"}`}>
                {side && (
                  <div className="ao-steps">
                    <div className="ao-steps-head"><b>Steps</b><span className="muted small">{steps[steps.length - 1].total} points</span></div>
                    <div className="ao-scroll"><StepList steps={steps} step={step} onStep={setStep} /></div>
                  </div>
                )}
                <div className="ao-treecol">
                  {steps.length > 0 && <StepBar steps={steps} step={step} onStep={setStep} />}
                  <div className="ao-treebox" ref={treeBox}>
                    {v && treeSize.h > 0 && <AtlasTreeView key={`${s.id}-${vi}`} nodes={v.nodes} choices={v.choices}
                      steps={steps} step={step} height={Math.max(160, treeSize.h - 34)} compact find={find} />}
                  </div>
                  {v && <div className="ao-choices ao-scroll"><ChoiceList nodes={v.nodes} choices={v.choices} steps={steps} step={step} onFind={finder} /></div>}
                </div>
              </div>
            )}
          </div>
        </>
      )}
      <ResizeGrips />
    </div>
  );
}
