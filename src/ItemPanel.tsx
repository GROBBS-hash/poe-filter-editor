import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, GAME_COLOURS, fromCurrent } from "./StyleEditor";
import { ItemIcon } from "./icons";

export type Picked = { item: Item; from_screen: boolean; screen_text?: string | null; confidence: number };

/** One of your saved edits, as opened for changing. */
export type RuleDetail = {
  label: string; show: boolean; item_class: string; base_type: string; rarity?: string | null;
  min_stack?: number | null; below_stack?: number | null; when?: When; style: CurrentStyle;
};
export type When = "always" | "levelling" | "maps";
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
export type Source = { kind: string; label: string; type_path: string[]; tier: string | null; section: string | null; tier_name: string | null };
const WHO: Record<string, string> = { neversink: "NeverSink", edit: "Your edit", preset: "Preset", price: "Price tier", none: "No rule" };

/** A clear "why": who decided it, in plain words, which rule, and a button to open that rule in the app. */
function Why({ v, onEditHere }: { v: Status["verdict"]; onEditHere?: () => void }) {
  const src = v.source ?? { kind: v.by_user_edit ? "edit" : "neversink", label: v.decided_by, type_path: [], tier: null, section: null, tier_name: null };
  const name = src.label.replace(/^(Preset|Hide|Highlight|Louder|Quieter|Style): /, "");
  const headline = src.kind === "neversink" ? v.reason.split(", ")[0] : src.kind === "none" ? "No rule covers it, so the game shows it plainly" : name || v.decided_by;
  const where = src.kind === "neversink"
    ? (src.section ? `Tiers › ${src.section} › ${src.tier_name ?? src.tier}` : `NeverSink › ${src.type_path.join(" › ")}${src.tier ? ` › ${src.tier}` : ""}`)
    : src.kind === "edit" ? "My edits" : src.kind === "preset" ? (src.label.startsWith("Preset: Levelling") ? "Presets › Levelling setup" : "Presets") : src.kind === "price" ? "Prices" : "";
  const canOpen = src.kind !== "none" && (src.kind !== "neversink" || !!src.section);
  return (
    <div className={`qm-why why-${src.kind}`} title={v.reason}>
      <div className="why-top">
        <span className="why-badge">{WHO[src.kind] ?? src.kind}</span>
        <span className="why-text">{headline}</span>
      </div>
      {where && <div className="why-where">{where}</div>}
      {src.kind === "neversink" && v.reason.includes(", ") && <div className="why-more">{v.reason.split(", ").slice(1).join(", ")}</div>}
      <div className="why-actions">
        {onEditHere && <button className="qm-inline" onClick={onEditHere}>Edit here</button>}
        {canOpen && <button className="qm-inline" onClick={() => invoke("open_rule", { source: src })}>Modify rule ↗</button>}
        {!canOpen && src.kind === "neversink" && <span className="muted small">Change it from here with the buttons below.</span>}
      </div>
    </div>
  );
}

type Style = CurrentStyle & { sound?: string | null; minimap?: string | null };
type Status = {
  verdict: { visibility: string; style: Style; decided_by: string; by_user_edit: boolean; reason: string; uncertain: string[]; source: Source };
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

type StackTier = { tier: string; name: string; min_stack: number | null; style: { text?: number[] | null; background?: number[] | null; border?: number[] | null } };
type StackChoice = { type_path: string[]; title: string; tiers: StackTier[]; current: string | null };
const rgb = (c?: number[] | null) => (c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : "transparent");

/** NeverSink's stack tiers for a currency (3+, 6+, its own supply stacks): see and change them in game. */
function StackTiers({ base, onDone }: { base: string; onDone: (msg: string) => void }) {
  const [list, setList] = useState<StackChoice[]>([]);
  const load = useCallback(() => {
    invoke<StackChoice[]>("stack_choices", { base }).then(setList).catch(() => setList([]));
  }, [base]);
  useEffect(() => { load(); }, [load]);
  if (list.length === 0) return null;
  const pick = async (c: StackChoice, tier: string) => {
    try {
      await invoke("set_stack_tier", { typePath: c.type_path, base, tier });
      const t = c.tiers.find((x) => x.tier === tier);
      onDone(tier ? `${base} stacks now use ${c.title} › ${t?.name} (${t?.min_stack}+).` : `${base} taken out of ${c.title}.`);
      load();
    } catch (e) { onDone(String(e)); }
  };
  return (
    <div className="qm-stacktiers">
      <div className="qm-label">Stack tiers <span className="muted small">(NeverSink's: how stacks of it look)</span></div>
      {list.map((c) => {
        const cur = c.tiers.find((t) => t.tier === c.current);
        return (
          <div key={c.type_path.join(">")} className="qm-st-row">
            <span className="qm-st-title">{c.title}</span>
            {cur && <span className="qm-st-swatch" style={{ color: rgb(cur.style.text), background: rgb(cur.style.background), borderColor: rgb(cur.style.border) }}>Aa</span>}
            <select value={c.current ?? ""} onChange={(e) => pick(c, e.target.value)}>
              <option value="">Not in it</option>
              {c.tiers.map((t) => <option key={t.tier} value={t.tier}>{t.name} · {t.min_stack}+</option>)}
            </select>
          </div>
        );
      })}
    </div>
  );
}

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
  /** Always, only while levelling (areas below 68), or only in maps. */
  const [when, setWhen] = useState<When>(replacingProp?.detail.when ?? "always");
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
    ({ same_rarity: !anyRarity, max_item_level: null, when, ...extraOpts });

  /** Every change goes through here: a new edit, or (when editing a rule) that rule replaced. */
  const commit = (show: boolean, style: CustomStyle | null, stack: { min_stack?: number; below_stack?: number }, addNew = false) => {
    if (replacing && !addNew) {
      const d = replacing.detail;
      const keepStack = stack.min_stack === undefined && stack.below_stack === undefined;
      return saved(invoke("update_rule", { id: replacing.id, index: replacing.index, edit: {
        show, style, every_rarity: anyRarity, when,
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
      setWhen(detail.when ?? "always");
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
  const iconOnly = () => {
    const c = current();
    saveStyle({ ...c, label_hidden: true, sound: { kind: "Off" },
      beam: c.beam ?? { colour: "White", temp: false },
      minimap: c.minimap ?? { size: 1, colour: "White", shape: "Circle" } });
  };
  /** Sound on/off and volume, keeping the rest of the look as it is. */
  const setSound = (sound: CustomStyle["sound"]) => saveStyle({ ...current(), sound });
  const soundOff = () => setSound({ kind: "Off" });
  const soundOn = () => {
    const c = current();
    // Coming back from silent: the game's normal drop sound.
    setSound(c.sound.kind === "Off" ? { kind: "Default" } : c.sound);
  };
  const setVolume = (v: number) => {
    const c = current();
    if (c.sound.kind === "Builtin" || c.sound.kind === "Custom") setSound({ ...c.sound, volume: v });
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
  /** A rule for every item of a type (e.g. every Two Hand Maces), not one base. */
  const wholeType = !item.base_type;
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
      <div className="qm-head with-icon">
        <ItemIcon name={item.rarity === "Unique" ? item.name : null} base={item.base_type || null} size={40} />
        <div className="qm-head-text">
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
      </div>

      {(picked.from_screen || replacing || !inGame) && isGear && (
        <div className="qm-rarity">
          {GEAR.map((r) => (
            <button key={r} className={item.rarity === r ? "on" : ""} style={{ color: RARITY_COLOUR[r] }}
              onClick={() => setRarity(r)}>{r}</button>
          ))}
        </div>
      )}

      {wholeType ? (
        <div className="qm-status"><div className="qm-facts">Changes here apply to <b>every {item.item_class}</b>
          {anyRarity ? " of every rarity" : ` that is ${item.rarity}`}. Tick "every rarity" below to cover all of them.</div></div>
      ) : v ? (
        <div className={`qm-status ${shown(v.visibility) ? "is-shown" : "is-hidden"}`}>
          <div className="qm-status-top">
            <span className="qm-pill">{!shown(v.visibility) ? "HIDDEN" : st?.label_hidden ? "ICON ONLY" : v.visibility === "Minimal" ? "SHOWN SMALL" : "SHOWN"}</span>
            {shown(v.visibility) && !st?.label_hidden && (
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
          <Why v={v} onEditHere={v.by_user_edit && !replacing && v.source?.kind === "edit" ? () => openRule(v.decided_by) : undefined} />
          {(v.uncertain.length > 0 || status!.assumptions.length > 0 || extra?.area_level != null) && (
            <div className="qm-unsure" title={[
              extra?.area_level != null ? `Area level ${extra.area_level} (from your game)` : "",
              status!.assumptions.length ? `Assuming ${status!.assumptions.join(", ")}` : "",
              ...v.uncertain.map((u) => `Could differ: ${u}`),
            ].filter(Boolean).join("\n")}>
              {extra?.area_level != null ? `Area ${extra.area_level}` : ""}{v.uncertain.length > 0 ? `${extra?.area_level != null ? " · " : ""}could differ with its mods` : ""} ⓘ
            </div>
          )}
        </div>
      ) : (
        <div className="qm-status"><div className="qm-facts">Checking your filter…</div></div>
      )}

      {!wholeType && extra?.stackable && status && (
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
          {item.item_class === "Stackable Currency" && item.base_type && (
            <StackTiers base={item.base_type} onDone={(m) => afterChange(m)} />
          )}
        </div>
      )}

      <div className="qm-row">
        <button id="qm-hide" className="qm-btn hide" onClick={hide}>Hide</button>
        <button className="qm-btn" onClick={iconOnly} disabled={!status && !wholeType} title="No label: just the minimap icon (and beam, if set)">Minimap icon only</button>

      </div>

      {(status || wholeType) && (() => {
        const snd = current().sound;
        const off = snd.kind === "Off";
        const alert = snd.kind === "Builtin" || snd.kind === "Custom";
        return (
          <div className="qm-sound">
            <span className="qm-label">Sound</span>
            <button className={!off ? "on" : ""} onClick={soundOn}>On</button>
            <button className={off ? "on off" : ""} onClick={soundOff}>Off</button>
            {!off && alert && (
              <>
                <input type="range" min={0} max={300} step={10} key={`${snd.kind}-${(snd as { volume: number }).volume}`}
                  defaultValue={(snd as { volume: number }).volume}
                  title="Volume (saved when you let go)"
                  onMouseUp={(e) => setVolume(+(e.target as HTMLInputElement).value)}
                  onKeyUp={(e) => setVolume(+(e.target as HTMLInputElement).value)} />
                <span className="qm-vol">{Math.round(((snd as { volume: number }).volume / 300) * 100)}%</span>
              </>
            )}
            {!off && !alert && (
              <button className="qm-inline" onClick={() => setEditing(true)} title="Opens Customise">Pick a sound…</button>
            )}
          </div>
        );
      })()}

      <div className="qm-label">Quick highlight</div>
      <div className="qm-row">
        {["Purple", "Yellow", "Cyan", "Red", "Green"].map((c) => (
          <button key={c} className="qm-swatch" title={c} style={{ background: GAME_COLOURS[c] }}
            onClick={() => highlight(c)} disabled={!status && !wholeType} />
        ))}
      </div>
      <button className="qm-custom" onClick={() => setEditing(true)} disabled={!status && !wholeType}>
        Customise colours, beam, icon & sound…
      </button>

      <div className="qm-when">
        <span>When:</span>
        {([["always", "Always"], ["levelling", "While levelling"], ["maps", "In maps"]] as [When, string][]).map(([k, label]) => (
          <button key={k} className={when === k ? "on" : ""} onClick={() => setWhen(k)}
            title={k === "levelling" ? "Areas below level 68 (the campaign)" : k === "maps" ? "Areas level 68 and up" : "Everywhere"}>{label}</button>
        ))}
      </div>

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
