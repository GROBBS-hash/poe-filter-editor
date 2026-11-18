import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Action, COLOURS, Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";

type Picked = { item: Item; from_screen: boolean; screen_text?: string | null; confidence: number };
type Style = {
  font_size?: number | null; text?: number[] | null; border?: number[] | null; background?: number[] | null;
  sound?: string | null; muted: boolean; minimap?: string | null; beam?: string | null;
};
type Status = {
  verdict: { visibility: string; style: Style; decided_by: string; by_user_edit: boolean; uncertain: string[] };
  by_stack: { from: number; visibility: string }[];
  assumptions: string[];
};

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
  const [failed, setFailed] = useState("");
  const [toast, setToast] = useState("");
  const [anyRarity, setAnyRarity] = useState(false);

  const close = useCallback(async () => {
    setPicked(null);
    setFailed("");
    await getCurrentWindow().hide();
  }, []);

  const refresh = useCallback(async (p: Picked) => {
    try {
      setStatus(await invoke<Status>("item_status", { item: p.item, fromScreen: p.from_screen }));
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
      refresh(e.payload);
    });
    const b = listen<string>("pick-failed", (e) => {
      setPicked(null);
      setFailed(e.payload);
      setTimeout(close, 2500);
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      if (e.key.toLowerCase() === "h") document.getElementById("qm-hide")?.click();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", close);
    return () => {
      a.then((f) => f()); b.then((f) => f());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", close);
    };
  }, [close, refresh]);

  const setRarity = (rarity: string) => {
    if (!picked) return;
    const p = { ...picked, item: { ...picked.item, rarity } };
    setPicked(p);
    refresh(p);
  };

  const act = async (action: Action, colour?: string) => {
    if (!picked) return;
    try {
      const r = await invoke<{ label: string; warnings: string[] }>("apply_quick_action", {
        item: picked.item, action, colour, opts: { same_rarity: !anyRarity, max_item_level: null },
      });
      setToast(r.warnings[0] ?? "Saved. Type /itemfilter in chat to reload.");
      refresh(picked);
    } catch (e) {
      setToast(String(e));
    }
  };

  const undo = async () => {
    const r = await invoke<string | null>("undo_last").catch((e) => String(e));
    setToast(r ? `Undone: ${r}` : "Nothing to undo");
    if (picked) refresh(picked);
  };

  if (failed) return <div className="qm"><div className="qm-fail">{failed}</div></div>;
  if (!picked) return null;
  const { item } = picked;
  const v = status?.verdict;
  const st = v?.style;
  const isGear = GEAR.includes(item.rarity);

  return (
    <div className="qm">
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
        </div>
      ) : (
        <div className="qm-status"><div className="qm-facts">Checking your filter…</div></div>
      )}

      <div className="qm-row">
        <button id="qm-hide" className="qm-btn hide" onClick={() => act("hide")}>Hide</button>
        <button className="qm-btn" onClick={() => act("louder")}>Louder</button>
        <button className="qm-btn" onClick={() => act("quieter")}>Quieter</button>
      </div>

      <div className="qm-label">Highlight</div>
      <div className="qm-row">
        {COLOURS.map((c) => (
          <button key={c.name} className="qm-swatch" title={c.name} style={{ background: c.hex }}
            onClick={() => act("highlight", c.name)} />
        ))}
      </div>

      {isGear && (
        <label className="qm-check">
          <input type="checkbox" checked={anyRarity} onChange={(e) => setAnyRarity(e.target.checked)} />
          Apply to every rarity, not just {item.rarity}
        </label>
      )}

      <div className="qm-foot">
        <button className="qm-link" onClick={undo}>Undo last</button>
        <button className="qm-link" onClick={close}>Close (Esc)</button>
      </div>
      {toast && <div className="qm-toast">{toast}</div>}
    </div>
  );
}
