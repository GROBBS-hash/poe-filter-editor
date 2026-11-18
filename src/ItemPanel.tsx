import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, GAME_COLOURS, fromCurrent } from "./StyleEditor";

export type Picked = { item: Item; from_screen: boolean; screen_text?: string | null; confidence: number };

/** One of your saved edits, as opened for changing. */
export type RuleDetail = {
  label: string; show: boolean; item_class: string; base_type: string; rarity?: string | null;
  min_stack?: number | null; below_stack?: number | null; style: CurrentStyle;
};
export type Replacing = { id: string; index: number; detail: RuleDetail };

/** The item a saved edit is about, so it can be shown in the same panel. */
export function itemFromRule(d: RuleDetail): Item {
  const stacky = ["Stackable Currency", "Gold", "Divination Cards", "Map Fragments"].includes(d.item_class);
  return {
    item_class: d.item_class, base_type: d.base_type, name: null,
    rarity: d.rarity ?? (stacky ? (d.item_class === "Divination Cards" ? "Divination Card" : "Currency") : "Normal"),
    item_level: null, stack_size: d.min_stack ?? null, quality: null, sockets: null, map_tier: null,
    corrupted: false, mirrored: false, unidentified: false,
  };
}
type Style = CurrentStyle & { sound?: string | null; minimap?: string | null };
type Status = {
  verdict: { visibility: string; style: Style; decided_by: string; by_user_edit: boolean; uncertain: string[] };
  by_stack: { from: number; visibility: string }[];
  assumptions: string[];
};
type GameFilter = { selected?: string | null; expected: string; matches: boolean };
type Reply = { status: Status; area_level?: number | null; game_filter?: GameFilter | null; stackable: boolean };

/** Stack amounts the slider snaps to, plus any cut-offs the filter already uses. */
const STEPS = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 50, 100, 200, 300, 500, 750, 1000, 1500, 2000, 3000, 5000, 10000];

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

/**
 * Everything about one item: what your filter does to it, and buttons to change it.
 * Used by the in-game pop-up and by "Edit" in the editor window. With `replacing`,
 * every button changes that saved edit instead of adding a new one.
 */
export default function ItemPanel({ start, inGame, replacing: replacingProp, onClose, onChanged, onEditingChange }: {
  start: Picked;
  inGame: boolean;
  replacing?: Replacing | null;
  onClose: () => void;
  onChanged?: () => void;
  /** Tells the pop-up window not to close itself while a sub-editor is open. */
  onEditingChange?: (editing: boolean) => void;
}) {
  const [picked, setPicked] = useState<Picked>(start);
  const [replacing, setReplacing] = useState<Replacing | null>(replacingProp ?? null);
  const [status, setStatus] = useState<Status | null>(null);
  const [extra, setExtra] = useState<Omit<Reply, "status"> | null>(null);
  const [toast, setToast] = useState("");
  const [anyRarity, setAnyRarity] = useState(replacingProp ? !replacingProp.detail.rarity : false);
  /** false = not editing; otherwise the stack size the look applies from (1 = all). */
  const [editing, setEditingRaw] = useState<false | { minStack: number }>(false);
  const setEditing = (v: boolean | { minStack: number }) => setEditingRaw(v === true ? { minStack: 1 } : v);
  const [stackPick, setStackPick] = useState(-1); // index into the slider steps
  const [autoReload, setAutoReload] = useState(() => {
    try { return localStorage.getItem("autoReload") !== "off"; } catch { return true; }
  });
  const toggleAuto = (on: boolean) => {
    setAutoReload(on);
    try { localStorage.setItem("autoReload", on ? "on" : "off"); } catch { /* not saved */ }
  };
  useEffect(() => { onEditingChange?.(editing !== false); }, [editing, onEditingChange]);

  const refresh = useCallback(async (p: Picked) => {
    try {
      const r = await invoke<Reply>("item_status", { item: p.item, fromScreen: p.from_screen });
      setStatus(r.status);
      setExtra({ area_level: r.area_level, game_filter: r.game_filter, stackable: r.stackable });
    } catch {
      setStatus(null);
    }
  }, []);
  useEffect(() => { refresh(start); }, [refresh, start]);
  const [price, setPrice] = useState<[number, string] | null>(null);
  useEffect(() => {
    invoke<[number, string] | null>("item_price", { item: picked.item }).then(setPrice).catch(() => setPrice(null));
  }, [picked.item]);

  const setRarity = (rarity: string) => {
    const p = { ...picked, item: { ...picked.item, rarity } };
    setPicked(p);
    refresh(p);
  };

  /** After a change: either reload it in game straight away, or tell them how. */
  const afterChange = async (message: string, warning?: string) => {
    onChanged?.();
    if (inGame && autoReload && !warning) {
      onClose();
      invoke("reload_filter").catch(() => { /* game not in front: F4 reloads later */ });
      return;
    }
    setToast(warning ?? `${message} Press F4 in game to reload.`);
    refresh(picked);
  };
  const saved = async (call: Promise<{ label: string; warnings: string[] }>, addedNew = false) => {
    try {
      const r = await call;
      // Keep editing the same rule after a change. A new rule can shift the
      // list, so find it by name then; a changed rule keeps its place.
      if (replacing) {
        try {
          const [id, index, detail] = await invoke<[string, number, RuleDetail]>("get_rule", addedNew
            ? { id: replacing.id, index: null, label: replacing.detail.label }
            : { id: replacing.id, index: replacing.index, label: null });
          setReplacing({ id, index, detail });
        } catch { /* keep the old details */ }
      }
      await afterChange(replacing ? `Updated “${r.label}”.` : "Saved.", r.warnings[0]);
    } catch (e) {
      setToast(String(e));
    }
  };
  const opts = (extraOpts: { min_stack?: number; below_stack?: number } = {}) =>
    ({ same_rarity: !anyRarity, max_item_level: null, ...extraOpts });

  /** Every change goes through here: a new edit, or (when editing a rule) that rule replaced. */
  const commit = (show: boolean, style: CustomStyle | null, stack: { min_stack?: number; below_stack?: number }, addNew = false) => {
    if (replacing && !addNew) {
      const d = replacing.detail;
      const keepStack = stack.min_stack === undefined && stack.below_stack === undefined;
      return saved(invoke("update_rule", { id: replacing.id, index: replacing.index, edit: {
        show, style, every_rarity: anyRarity,
        min_stack: keepStack ? d.min_stack ?? null : stack.min_stack ?? null,
        below_stack: keepStack ? d.below_stack ?? null : stack.below_stack ?? null,
      } }));
    }
    return show && style
      ? saved(invoke("apply_custom_style", { item: picked.item, style, opts: opts(stack) }), addNew)
      : saved(invoke("apply_quick_action", { item: picked.item, action: "hide", colour: null, opts: opts(stack) }), addNew);
  };
  const hide = () => commit(false, null, {});
  // Stack rules always sit alongside your other rules for this item ("hide the
  // small piles" + "style the big ones"), so they're added, never swapped in.
  const hideBelow = (n: number) => commit(false, null, { below_stack: n }, true);
  const saveStyle = (style: CustomStyle) => {
    const minStack = editing ? editing.minStack : 1;
    setEditingRaw(false);
    commit(true, style, minStack > 1 ? { min_stack: minStack } : {}, minStack > 1);
  };
  const openRule = async (label: string) => {
    try {
      const [id, index, detail] = await invoke<[string, number, RuleDetail]>("get_rule", { id: null, index: null, label });
      setReplacing({ id, index, detail });
      setAnyRarity(!detail.rarity);
    } catch (e) { setToast(String(e)); }
  };
  const removeRule = async () => {
    if (!replacing) return;
    try {
      await invoke("delete_rule", { id: replacing.id, index: replacing.index });
      setReplacing(null);
      if (!inGame) { onChanged?.(); onClose(); return; }
      await afterChange("Edit removed.");
    } catch (e) { setToast(String(e)); }
  };

  /** Current look, so quick buttons only change what they're about. */
  const current = () => {
    // Editing a rule: start from that rule's own look.
    if (replacing && replacing.detail.show) return fromCurrent(replacing.detail.style, picked.item.rarity);
    return fromCurrent(status?.verdict.visibility === "Hide" ? undefined : status?.verdict.style, picked.item.rarity);
  };
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

  const { item } = picked;
  const v = status?.verdict;
  const st = v?.style;
  const isGear = GEAR.includes(item.rarity);

  const gf = extra?.game_filter;
  const nameOf = (f: string) => f.replace(/\.filter$/i, "");

  // Slider steps: the usual amounts plus any cut-offs the filter already uses.
  const steps = Array.from(new Set([...STEPS, ...(status?.by_stack.map((b) => b.from) ?? [])])).sort((a, b) => a - b);
  const defaultPick = (() => {
    const cut = status?.by_stack.find((b) => b.from > 1)?.from;
    const want = cut ?? (item.stack_size && item.stack_size > 1 ? item.stack_size : 3);
    const i = steps.findIndex((n) => n >= want);
    return i < 0 ? steps.length - 1 : i;
  })();
  const pickIndex = stackPick < 0 || stackPick >= steps.length ? defaultPick : stackPick;
  const stackN = steps[pickIndex];

  if (editing) {
    return (
      <div className="qm-panel">
        <div className="qm-head">
          <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
          <div className="qm-sub">
            {editing.minStack > 1 ? `Customise stacks of ${editing.minStack} or more` : "Customise how it looks in game"}
          </div>
        </div>
        <StyleEditor start={current()} label={item.base_type} onSave={saveStyle} onCancel={() => setEditing(false)} />
      </div>
    );
  }

  return (
    <div className="qm-panel">
      {replacing && (
        <div className="qm-editing">
          Editing your rule: <b>{replacing.detail.label}</b>. Buttons below change this rule.
          <button className="qm-inline" onClick={removeRule}>Remove it</button>
          {inGame && <button className="qm-inline" onClick={() => setReplacing(null)}>Stop editing</button>}
        </div>
      )}
      {inGame && gf && !gf.matches && (
        <div className="qm-warn">
          Your game is using <b>{nameOf(gf.selected ?? "")}</b>, not <b>{nameOf(gf.expected)}</b>.
          Press <b>F4</b> in game to switch to {nameOf(gf.expected)}.
        </div>
      )}
      <div className="qm-head">
        <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
        <div className="qm-sub">{itemSub(item)}</div>
        {price && (
          <div className="qm-price">
            ≈ {price[0] >= 100 ? Math.round(price[0]).toLocaleString() : +price[0].toFixed(price[0] < 1 ? 2 : 1)}
            {({ chaos: "c", exalted: "ex", divine: "div" } as Record<string, string>)[price[1]] ?? price[1]}
            <span> on poe.ninja{item.rarity === "Unique" && !item.name ? " (best unique on this base)" : ""}</span>
          </div>
        )}
        {picked.from_screen && (
          <div className="qm-where">Read from the ground: “{picked.screen_text}”{picked.confidence < 1 ? " (best guess)" : ""}</div>
        )}
      </div>

      {(picked.from_screen || replacing) && isGear && (
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
            {v.by_user_edit ? <>Your edit: <b>{v.decided_by}</b>{" "}
              {!replacing && <button className="qm-inline" onClick={() => openRule(v.decided_by)}>Edit this rule</button>}</>
              : v.decided_by.startsWith("Price") ? <>Price tiers: {v.decided_by}</> : <>NeverSink: {v.decided_by}</>}
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

      {extra?.stackable && status && (
        <div className="qm-stack">
          <div className="qm-label">Stack size: <b>{stackN.toLocaleString()}</b>{item.stack_size ? ` (this one: ${item.stack_size.toLocaleString()})` : ""}</div>
          <input type="range" min={0} max={steps.length - 1} value={pickIndex}
            onChange={(e) => setStackPick(+e.target.value)} />
          <div className="qm-row">
            <button className="qm-btn small hide" onClick={() => hideBelow(stackN)} disabled={stackN <= 1}
              title={`Hide stacks smaller than ${stackN.toLocaleString()}`}>
              Only show {stackN.toLocaleString()}+
            </button>
            <button className="qm-btn small" onClick={() => setEditing({ minStack: stackN })}>
              Style {stackN.toLocaleString()}+…
            </button>
          </div>
        </div>
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
          <span>Apply to every rarity, not just {item.rarity}</span>
        </label>
      )}

      {inGame && (
        <label className="qm-check">
          <input type="checkbox" checked={autoReload} onChange={(e) => toggleAuto(e.target.checked)} />
          <span>Reload {gf ? <b>{nameOf(gf.expected)}</b> : "the filter"} in game after each change (F4 does it by hand)</span>
        </label>
      )}

      <div className="qm-foot">
        {!replacing && <button className="qm-link" onClick={undo}>Undo last</button>}
        <button className="qm-link" onClick={onClose}>{inGame ? "Close (Esc)" : "Done"}</button>
      </div>
      {toast && <div className="qm-toast">{toast}</div>}
    </div>
  );
}
