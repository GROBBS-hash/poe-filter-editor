import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, GAME_COLOURS, fromCurrent } from "./StyleEditor";

type Picked = { item: Item; from_screen: boolean; screen_text?: string | null; confidence: number };
type Style = CurrentStyle & { sound?: string | null; minimap?: string | null };
type Status = {
  verdict: { visibility: string; style: Style; decided_by: string; by_user_edit: boolean; uncertain: string[] };
  by_stack: { from: number; visibility: string }[];
  assumptions: string[];
};
type GameFilter = { selected?: string | null; expected: string; matches: boolean };
type Reply = { status: Status; area_level?: number | null; game_filter?: GameFilter | null };

const GEAR = ["Normal", "Magic", "Rare", "Unique"];
const rgba = (c?: number[] | null, fallback = "transparent") =>
  c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : fallback;
const shown = (v: string) => v !== "Hide";
const word = (v: string) => (v === "Hide" ? "hidden" : v === "Minimal" ? "shown small" : "shown");

/** Plain-English stack rule, e.g. "Hidden in stacks of 1–2, shown at 3+". */
function stackLine(by: Status["by_stack"]) {
  return by.map((n, i) => {
    const next = by[i + 1]?.from;
    const range = next ? (next - 1 === n.from ? `${n.from}` : `${n.from}–${next - 1}`) : `${n.from}+`;
    return `${i === 0 ? word(n.visibility)[0].toUpperCase() + word(n.visibility).slice(1) : word(n.visibility)} at ${range}`;
  }).join(", ");
}

export default function QuickMenu() {
  const [picked, setPicked] = useState<Picked | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [extra, setExtra] = useState<Omit<Reply, "status"> | null>(null);
  const [failed, setFailed] = useState("");
  const [toast, setToast] = useState("");
  const [anyRarity, setAnyRarity] = useState(false);
  const [editing, setEditing] = useState(false);
  const [autoReload, setAutoReload] = useState(() => {
    try { return localStorage.getItem("autoReload") !== "off"; } catch { return true; }
  });
  const toggleAuto = (on: boolean) => {
    setAutoReload(on);
    try { localStorage.setItem("autoReload", on ? "on" : "off"); } catch { /* not saved */ }
  };
  const editingRef = useRef(false);
  editingRef.current = editing;
  const box = useRef<HTMLDivElement | null>(null);

  const close = useCallback(async () => {
    setPicked(null);
    setFailed("");
    setEditing(false);
    await getCurrentWindow().hide();
  }, []);

  // Fit the window to the menu as it grows (e.g. when the editor opens).
  useEffect(() => {
    if (!box.current) return;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(box.current?.getBoundingClientRect().height ?? 300) + 4;
      getCurrentWindow().setSize(new LogicalSize(300, Math.max(120, h))).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  const refresh = useCallback(async (p: Picked) => {
    try {
      const r = await invoke<Reply>("item_status", { item: p.item, fromScreen: p.from_screen });
      setStatus(r.status);
      setExtra({ area_level: r.area_level, game_filter: r.game_filter });
    } catch {
      setStatus(null);
    }
  }, []);

  useEffect(() => {
    const a = listen<Picked>("item-picked", (e) => {
      setPicked(e.payload);
      setFailed("");
      setToast("");
      setStatus(null);
      setAnyRarity(false);
      setEditing(false);
      refresh(e.payload);
    });
    const b = listen<string>("pick-failed", (e) => {
      setPicked(null);
      setFailed(e.payload);
      setTimeout(close, 2500);
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") { if (editingRef.current) setEditing(false); else close(); }
      if (e.key.toLowerCase() === "h" && !editingRef.current) document.getElementById("qm-hide")?.click();
    };
    // Clicking back into the game closes the menu, but not while editing
    // (the colour picker opens its own window).
    const onBlur = () => { if (!editingRef.current) close(); };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => {
      a.then((f) => f()); b.then((f) => f());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
    };
  }, [close, refresh]);

  const setRarity = (rarity: string) => {
    if (!picked) return;
    const p = { ...picked, item: { ...picked.item, rarity } };
    setPicked(p);
    refresh(p);
  };

  /** After a change: either reload it in game straight away, or tell them how. */
  const afterChange = async (message: string, warning?: string) => {
    if (autoReload && !warning) {
      await close();
      invoke("reload_filter").catch(() => { /* game not in front: F4 reloads later */ });
      return;
    }
    setToast(warning ?? `${message} Press F4 in game to reload.`);
    if (picked) refresh(picked);
  };
  const saved = async (call: Promise<{ label: string; warnings: string[] }>) => {
    try {
      const r = await call;
      await afterChange("Saved.", r.warnings[0]);
    } catch (e) {
      setToast(String(e));
    }
  };
  const opts = () => ({ same_rarity: !anyRarity, max_item_level: null });
  const hide = () => picked && saved(invoke("apply_quick_action", { item: picked.item, action: "hide", colour: null, opts: opts() }));
  const saveStyle = (style: CustomStyle) => {
    if (!picked) return;
    setEditing(false);
    saved(invoke("apply_custom_style", { item: picked.item, style, opts: opts() }));
  };
  /** Current look, so quick buttons only change what they're about. */
  const current = () => fromCurrent(status?.verdict.visibility === "Hide" ? undefined : status?.verdict.style, picked?.item.rarity ?? "Normal");
  const louder = () => {
    const c = current();
    const sound = c.sound.kind === "Builtin" || c.sound.kind === "Custom" ? { ...c.sound, volume: 300 } : { kind: "Builtin" as const, id: "2", volume: 300 };
    saveStyle({ ...c, font_size: 45, sound });
  };
  const quieter = () => {
    const c = current();
    saveStyle({ ...c, font_size: Math.max(18, c.font_size - 10), sound: { kind: "Off" }, beam: null });
  };
  const highlight = (name: string) => {
    const c = current();
    const rgb = GAME_COLOURS[name].slice(1).match(/../g)!.map((h) => parseInt(h, 16)).concat(255);
    saveStyle({ ...c, text: rgb, border: rgb, background: [20, 20, 20, 240], font_size: Math.max(c.font_size, 40),
      beam: { colour: name, temp: false }, minimap: { size: 1, colour: name, shape: c.minimap?.shape ?? "Star" } });
  };

  const undo = async () => {
    const r = await invoke<string | null>("undo_last").catch(() => null);
    if (r) await afterChange(`Undone: ${r}.`);
    else setToast("Nothing to undo");
  };

  if (failed) return <div className="qm" ref={box}><div className="qm-fail">{failed}</div></div>;
  if (!picked) return null;
  const { item } = picked;
  const v = status?.verdict;
  const st = v?.style;
  const isGear = GEAR.includes(item.rarity);

  const gf = extra?.game_filter;
  const nameOf = (f: string) => f.replace(/\.filter$/i, "");

  if (editing) {
    return (
      <div className="qm" ref={box}>
        <div className="qm-head">
          <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
          <div className="qm-sub">Customise how it looks in game</div>
        </div>
        <StyleEditor start={current()} label={item.base_type} onSave={saveStyle} onCancel={() => setEditing(false)} />
      </div>
    );
  }

  return (
    <div className="qm" ref={box}>
      {gf && !gf.matches && (
        <div className="qm-warn">
          Your game is using <b>{nameOf(gf.selected ?? "")}</b>, not <b>{nameOf(gf.expected)}</b>.
          Pick {nameOf(gf.expected)} in Options › Game › Item Filter, or this won't match what you see.
        </div>
      )}
      <div className="qm-head">
        <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
        <div className="qm-sub">{itemSub(item)}</div>
        {picked.from_screen && (
          <div className="qm-where">Read from the ground: “{picked.screen_text}”{picked.confidence < 1 ? " (best guess)" : ""}</div>
        )}
      </div>

      {picked.from_screen && isGear && (
        <div className="qm-rarity">
          {GEAR.map((r) => (
            <button key={r} className={item.rarity === r ? "on" : ""} style={{ color: RARITY_COLOUR[r] }}
              onClick={() => setRarity(r)}>{r}</button>
          ))}
        </div>
      )}

      {v ? (
        <div className={`qm-status ${shown(v.visibility) ? "is-shown" : "is-hidden"}`}>
          <div className="qm-status-top">
            <span className="qm-pill">{shown(v.visibility) ? (v.visibility === "Minimal" ? "SHOWN SMALL" : "SHOWN") : "HIDDEN"}</span>
            {shown(v.visibility) && (
              <span className="qm-preview" style={{
                color: rgba(st?.text, "#c8c8c8"),
                borderColor: rgba(st?.border, "transparent"),
                background: rgba(st?.background, "rgba(0,0,0,.7)"),
                fontSize: Math.max(11, Math.min(22, (st?.font_size ?? 32) / 2.2)),
              }}>{item.base_type}</span>
            )}
          </div>
          {shown(v.visibility) && (
            <div className="qm-facts">
              {st?.muted ? "No drop sound" : st?.sound ? `🔊 ${st.sound}` : "Default drop sound"}
              {st?.beam ? ` · ${st.beam} beam` : ""}
              {st?.minimap ? ` · map icon` : ""}
            </div>
          )}
          {status!.by_stack.length > 0 && <div className="qm-facts">Stacks: {stackLine(status!.by_stack)}</div>}
          <div className="qm-by">
            {v.by_user_edit ? <>Your edit: <b>{v.decided_by}</b></> : <>NeverSink: {v.decided_by}</>}
          </div>
          {v.uncertain.length > 0 && (
            <div className="qm-unsure" title={v.uncertain.join("\n")}>
              Could differ depending on its mods ({v.uncertain.length} rule{v.uncertain.length > 1 ? "s" : ""})
            </div>
          )}
          {status!.assumptions.length > 0 && <div className="qm-unsure">Assuming {status!.assumptions.join(", ")}</div>}
          {extra?.area_level != null && <div className="qm-unsure">Area level {extra.area_level} (from your game)</div>}
        </div>
      ) : (
        <div className="qm-status"><div className="qm-facts">Checking your filter…</div></div>
      )}

      <div className="qm-row">
        <button id="qm-hide" className="qm-btn hide" onClick={hide}>Hide</button>
        <button className="qm-btn" onClick={louder} disabled={!status}>Louder</button>
        <button className="qm-btn" onClick={quieter} disabled={!status}>Quieter</button>
      </div>

      <div className="qm-label">Quick highlight</div>
      <div className="qm-row">
        {["Purple", "Yellow", "Cyan", "Red", "Green"].map((c) => (
          <button key={c} className="qm-swatch" title={c} style={{ background: GAME_COLOURS[c] }}
            onClick={() => highlight(c)} disabled={!status} />
        ))}
      </div>
      <button className="qm-custom" onClick={() => setEditing(true)} disabled={!status}>
        Customise colours, beam, icon & sound…
      </button>

      {isGear && (
        <label className="qm-check">
          <input type="checkbox" checked={anyRarity} onChange={(e) => setAnyRarity(e.target.checked)} />
          Apply to every rarity, not just {item.rarity}
        </label>
      )}

      <label className="qm-check">
        <input type="checkbox" checked={autoReload} onChange={(e) => toggleAuto(e.target.checked)} />
        Reload the filter in game after each change (F4 does it by hand)
      </label>

      <div className="qm-foot">
        <button className="qm-link" onClick={undo}>Undo last</button>
        <button className="qm-link" onClick={close}>Close (Esc)</button>
      </div>
      {toast && <div className="qm-toast">{toast}</div>}
    </div>
  );
}
