import { type ReactNode, useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";
import { LookChips, looksChanged, useLooks } from "./Looks";
import StyleEditor, { CurrentStyle, CustomStyle, GAME_COLOURS, LookButton, fromCurrent } from "./StyleEditor";
import { JewelMod, JewelRule, jewelModList, readJewelRule, saveJewelRule } from "./jewelMods";
import { ItemIcon } from "./icons";
import { mapItemIcon } from "./mapIcons";
import BaseUniques from "./BaseUniques";

export type Picked = { item: Item; from_screen: boolean; screen_text?: string | null; confidence: number; rarity_unsure?: boolean };

/** One of your saved edits, as opened for changing. */
export type RuleDetail = {
  label: string; show: boolean; item_class: string; base_type: string; rarity?: string | null;
  min_stack?: number | null; below_stack?: number | null; when?: When; corrupted?: boolean; style: CurrentStyle;
  /** The saved look it uses, if any. */
  look?: string | null; off?: boolean;
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
export type Source = { kind: string; label: string; type_path: string[]; tier: string | null; section: string | null; tier_name: string | null; conds?: string[];
  /** Your edit names only this one item. */
  only_base?: string | null };
const WHO: Record<string, string> = { neversink: "NeverSink", edit: "Your edit", preset: "Preset", price: "Price tier", none: "No rule" };

/** NeverSink's internal section names in plain words ("normalmagic" -> "white & blue"). */
const PLAIN: Record<string, string> = { leveling: "levelling", normalmagic: "white & blue", "4l": "4-links", "3l": "3-links",
  "6l": "6-links", "5l": "5-links", rare: "rares", rgb: "R-G-B sockets", exotic: "special", exoticbases: "special bases",
  general: "general", gear: "gear", tiered: "by item tier", uniques: "uniques", currency: "currency", divination: "divination cards" };
const plain = (p: string) => PLAIN[p] ?? p.replace(/([a-z])([A-Z])/g, "$1 $2");

/** Which rule decides it, in the Tiers page's words (23 Nov: "a way to see what rule is showing this item"):
 *  "Shown by Crafting bases › Tier 1 bases, item level 86+", that rule's conditions as small chips; click to open it. */
function RuleLine({ src, hidden, onOpen, action, scope }: { src?: Source; hidden: boolean; onOpen?: () => void; action: string; scope?: ReactNode }) {
  if (!src) return null;
  const verb = hidden ? "Hidden by" : "Shown by";
  const name = src.label.replace(/^(Preset|Hide|Highlight|Louder|Quieter|Style): /, "");
  const what = src.kind === "neversink"
    ? <>{src.section ? <b>{src.section}</b> : <b>NeverSink › {src.type_path.map(plain).join(" › ")}</b>}{(src.tier_name ?? src.tier) && <> › {src.tier_name ?? src.tier}</>}</>
    : src.kind === "edit" ? <>your edit <b>{name}</b></> : src.kind === "preset" ? <>preset <b>{name}</b></>
    : src.kind === "price" ? <><b>{src.label}</b></> : <>no rule: the game shows it plainly</>;
  const body = (
    <>
      <span className="qm-rule-h">{src.kind === "none" ? "Rule" : verb}</span>
      <span className="qm-rule-name">{what}</span>
      {scope && <span className="qm-rule-scope"><b>Just this item</b> · {scope}</span>}
      {(src.conds?.length ?? 0) > 0 && <span className="qm-rule-conds">{src.conds!.map((c) => <span key={c}>{c}</span>)}</span>}
    </>
  );
  return (
    <div className={`qm-rule ${onOpen ? "can-open" : ""}`} onClick={onOpen} title={onOpen ? "Opens this rule in the editor" : undefined}>
      {body}
      {onOpen && <button className="qm-change qm-rule-btn" onClick={(e) => { e.stopPropagation(); onOpen(); }}>{action}</button>}
    </div>
  );
}

/** A clear "why": who decided it, in plain words, which rule, and a button to open that rule in the app. */
function Why({ v, onEditHere }: { v: Status["verdict"]; onEditHere?: () => void }) {
  const src = v.source ?? { kind: v.by_user_edit ? "edit" : "neversink", label: v.decided_by, type_path: [], tier: null, section: null, tier_name: null };
  const name = src.label.replace(/^(Preset|Hide|Highlight|Louder|Quieter|Style): /, "");
  const headline = src.kind === "neversink" ? v.reason.split(", ")[0] : src.kind === "none" ? "No rule covers it, so the game shows it plainly" : name || v.decided_by;
  const where = src.kind === "neversink"
    ? (src.section ? `Tiers › ${src.section} › ${src.tier_name ?? src.tier}` : `NeverSink › ${[...src.type_path, ...(src.tier ? [src.tier] : [])].map(plain).join(" › ")}`)
    : src.kind === "edit" ? "My edits" : src.kind === "preset" ? (src.label.startsWith("Preset: Levelling") ? "Presets › Levelling setup" : "Presets") : src.kind === "price" ? "Prices" : "";
  const canOpen = src.kind !== "none" && (src.kind !== "neversink" || src.type_path.length > 0);
  const hidden = v.visibility === "Hide";
  return (
    <div className={`qm-why why-${src.kind} ${hidden ? "why-hidden" : ""}`} title={v.reason}>
      <div className="why-label">{src.kind === "none" ? "Why it shows" : hidden ? "Hidden by" : "Shown by"}
        <span className="why-badge">{WHO[src.kind] ?? src.kind}</span></div>
      <div className="why-text">{headline}</div>
      {where && <div className="why-where">{where}</div>}
      {src.kind === "neversink" && v.reason.includes(", ") && <div className="why-more">{v.reason.split(", ").slice(1).join(", ")}</div>}
      <div className="why-actions">
        {canOpen && <button className="why-open" onClick={() => invoke("open_rule", { source: src })}>Modify this rule</button>}
        {onEditHere && <button className="qm-inline" onClick={onEditHere}>Edit here</button>}
        {src.kind === "none" && <span className="muted small">Use the buttons below to add your own rule.</span>}
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

export default function ItemPanel({ start, inGame, replacing: replacingProp, onClose, onChanged, onEditingChange, openLook, onMoved, startToast, justItem }: {
  start: Picked;
  inGame: boolean;
  replacing?: Replacing | null;
  onClose: () => void;
  onChanged?: () => void;
  /** Tells the pop-up window not to close itself while a sub-editor is open. */
  onEditingChange?: (editing: boolean) => void;
  /** Open straight on the look editor (a label preview was clicked). */
  openLook?: boolean;
  /** The saved edit being changed is another one now, or at another place in the list (with the message to show). */
  onMoved?: (index: number, detail: RuleDetail, message: string) => void;
  startToast?: string;
  /** Opened for this one item (clicked on the Tiers page): start on "Just this item", whatever F2 last used. */
  justItem?: boolean;
}) {
  const [picked, setPicked] = useState<Picked>(start);
  const [replacing, setReplacing] = useState<Replacing | null>(replacingProp ?? null);
  const [status, setStatus] = useState<Status | null>(null);
  // "More options" folded or open, remembered between pop-ups.
  const [more, setMore] = useState(() => { try { return localStorage.getItem("qm-more") === "1"; } catch { return false; } });
  const toggleMore = (on: boolean) => { setMore(on); try { localStorage.setItem("qm-more", on ? "1" : "0"); } catch { /* not kept */ } };
  const [extra, setExtra] = useState<Omit<Reply, "status"> | null>(null);
  const [toast, setToast] = useState(startToast ?? "");
  const [anyRarity, setAnyRarity] = useState(replacingProp ? !replacingProp.detail.rarity : false);
  /** Always, only while levelling (areas below 68), or only in maps. */
  const [when, setWhen] = useState<When>(replacingProp?.detail.when ?? "always");
  /** Only corrupted items (offered when the item is corrupted). */
  const [onlyCorrupted, setOnlyCorrupted] = useState<boolean>(replacingProp?.detail.corrupted ?? false);
  /** false = not editing; otherwise the stack size the look applies from (1 = all). */
  const [editing, setEditingRaw] = useState<false | { minStack: number }>(openLook ? { minStack: 1 } : false);
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
    const p = { ...picked, rarity_unsure: false, item: { ...picked.item, rarity } };
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
  const saved = async (call: Promise<{ label: string; warnings: string[] }>, addedNew = false, follow = false) => {
    try {
      const r = await call;
      // Keep editing the same rule after a change. A new rule can shift the
      // list, so find it by name then; a changed rule keeps its place.
      if (replacing) {
        try {
          const [id, index, detail] = await invoke<[string, number, RuleDetail]>("get_rule", addedNew
            ? { id: replacing.id, index: null, label: follow ? r.label : replacing.detail.label }
            : { id: replacing.id, index: replacing.index, label: null });
          setReplacing({ id, index, detail });
          if (index !== replacing.index) onMoved?.(index, detail, `Updated “${r.label}”. Press F4 in game to reload.`);
        } catch { if (follow) { onChanged?.(); onClose(); return; } /* else keep the old details */ }
      }
      // Notes about older edits whose NeverSink tier no longer exists aren't about this change (My edits lists
      // them): don't let them block the reload or replace the "Saved" message.
      const relevant = r.warnings.filter((w) => !w.includes("is gone in this version"));
      await afterChange(replacing ? `Updated “${r.label}”.` : "Saved.", relevant[0]);
    } catch (e) {
      setToast(String(e));
    }
  };
  const opts = (extraOpts: { min_stack?: number; below_stack?: number } = {}) =>
    ({ same_rarity: !anyRarity, max_item_level: null, when, corrupted: onlyCorrupted, ...extraOpts });

  /** "All <group>" (change the NeverSink rule in charge, like the Tiers / Map tiers page) or "Just this" (your own
   *  edit on top). He chose this switch (2A) on 23 Nov so only one rule decides a look; remembered. */
  const [scope, setScopeRaw] = useState<"group" | "item">(() => { if (justItem) return "item"; try { return localStorage.getItem("f2.scope") === "item" ? "item" : "group"; } catch { return "group"; } });
  const setScope = (s: "group" | "item") => { setScopeRaw(s); if (justItem) return; try { localStorage.setItem("f2.scope", s); } catch { /* not kept */ } };
  const groupSrc = status?.verdict.source;
  /** The NeverSink rule in charge, when there is one F2 can change for the whole group. */
  const group = !replacing && groupSrc?.kind === "neversink" && groupSrc.type_path.length > 0 && groupSrc.tier ? groupSrc : null;
  const groupName = group ? (group.tier_name ?? group.tier ?? "") : "";

  /** Every change goes through here: a new edit, or (when editing a rule) that rule replaced. */
  const commit = (show: boolean, style: CustomStyle | null, stack: { min_stack?: number; below_stack?: number }, addNew = false, look: string | null = null) => {
    // "All <group>": change the rule already in charge (stack cut-offs stay your own edits).
    if (group && scope === "group" && stack.min_stack === undefined && stack.below_stack === undefined) {
      return saved(invoke("apply_to_tier", { typePath: group.type_path, tier: group.tier, show, style, look }));
    }
    if (replacing && !addNew) {
      const d = replacing.detail;
      const keepStack = stack.min_stack === undefined && stack.below_stack === undefined;
      return saved(invoke("update_rule", { id: replacing.id, index: replacing.index, edit: {
        show, style, every_rarity: anyRarity, when, corrupted: onlyCorrupted,
        min_stack: keepStack ? d.min_stack ?? null : stack.min_stack ?? null,
        below_stack: keepStack ? d.below_stack ?? null : stack.below_stack ?? null, look,
      } }));
    }
    return show && style
      ? saved(invoke("apply_custom_style", { item: picked.item, style, opts: opts(stack), look }), addNew)
      : saved(invoke("apply_quick_action", { item: picked.item, action: "hide", colour: null, opts: opts(stack) }), addNew);
  };
  const hide = () => commit(false, null, {});
  /** On a hidden item, the same button shows it again: your own hide is simply removed (NeverSink's look
   *  comes back); anything else hidden gets a plain visible look you can restyle. */
  const unhide = async () => {
    const v = status?.verdict;
    if (v?.by_user_edit && v.source?.kind === "edit" && !replacing) {
      try {
        const [id, index] = await invoke<[string, number, RuleDetail]>("get_rule", { id: null, index: null, label: v.decided_by });
        await invoke("delete_rule", { id, index });
        await afterChange(`Removed your hide (“${v.decided_by}”): it shows again.`);
        return;
      } catch (e) {
        // The hide is in the filter this one builds on: show it here with an edit of its own (below).
        if (!String(e).includes("another profile")) { setToast(String(e)); return; }
      }
    }
    // Hidden by NeverSink (e.g. single orbs in a low tier): its hiding rule still has its own look, so show it with
    // that look rather than a plain one.
    const look = v?.style && (v.style.text || v.style.background || v.style.border) ? v.style : undefined;
    return commit(true, fromCurrent(look, picked.item.rarity), {});
  };
  // Hides the small piles and makes sure the rest shows: your own hide of every stack is replaced (25 Nov).
  const hideBelow = (n: number) => saved(invoke("only_show_stack", { item: picked.item, min: n, opts: opts() }), true, true);
  /** `look`: a saved look's name; the edit then follows that look when it's changed later. */
  const saveStyle = (style: CustomStyle, look: string | null = null) => {
    const minStack = editing ? editing.minStack : 1;
    setEditingRaw(false);
    commit(true, style, minStack > 1 ? { min_stack: minStack } : {}, minStack > 1, look);
  };
  const [looks] = useLooks();
  const saveLook = async (name: string) => {
    await invoke("save_look", { look: { name, style: current() }, oldName: null });
    looksChanged();
    setToast(`Saved “${name}”. Click it on any item to give it this look.`);
  };
  const openRule = async (label: string) => {
    try {
      const [id, index, detail] = await invoke<[string, number, RuleDetail]>("get_rule", { id: null, index: null, label });
      setReplacing({ id, index, detail });
      setAnyRarity(!detail.rarity);
      setWhen(detail.when ?? "always");
      setOnlyCorrupted(!!detail.corrupted);
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
  // Rarity only matters for gear and maps; currency, fragments, cards etc. are "Normal" but have no rarity to pick.
  const isGear = GEAR.includes(item.rarity) && !extra?.stackable && !/Currency|Fragment|Divination|Gem|Corpse/.test(item.item_class);

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
    <div className={`qm-panel ${inGame ? "" : "qm-sheet"}`}>
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
        <ItemIcon name={item.rarity === "Unique" ? item.name : mapItemIcon(item)} base={item.base_type || null} size={40} />
        <div className="qm-head-text">
        <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
        <div className="qm-sub">{itemSub(item)}</div>
        {price && (
          <div className="qm-price">
            ≈ {price[0] >= 100 ? Math.round(price[0]).toLocaleString() : +price[0].toFixed(price[0] < 1 ? 2 : 1)}
            {({ chaos: "c", exalted: "ex", divine: "div" } as Record<string, string>)[price[1]] ?? price[1]}
            <span> on poe.ninja{item.rarity === "Unique" && !item.name ? " (best ground unique on this base)" : ""}</span>
          </div>
        )}
        {picked.from_screen && (
          <div className="qm-where">Read from the ground: “{picked.screen_text}”{picked.confidence < 1 ? " (best guess)" : ""}</div>
        )}
        </div>
      </div>
      {item.rarity === "Unique" && !item.name && item.base_type && <BaseUniques base={item.base_type} />}

      {picked.from_screen && picked.rarity_unsure && isGear && (
        <div className="qm-rarity-note">Your filter shows this as the same colours for every rarity, so the label can't say which it is.
          Pick the rarity below, or hover the item until its info box shows, then press again.</div>
      )}
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
              <LookButton onClick={() => setEditing(true)}><span className="qm-preview" style={{
                color: rgba(st?.text, "#c8c8c8"),
                borderColor: rgba(st?.border, "transparent"),
                background: rgba(st?.background, "rgba(0,0,0,.7)"),
                fontSize: Math.max(11, Math.min(22, (st?.font_size ?? 32) / 2.2)),
              }}>{item.base_type}</span></LookButton>
            )}
          </div>
          {(() => {
            // Plain words, no filter jargon: what it does as chips, and one button to change it.
            const src = v.source;
            const canOpen = !!src && src.kind !== "none" && (src.kind !== "neversink" || src.type_path.length > 0);
            const change = () => (v.by_user_edit && !replacing && src?.kind === "edit" ? openRule(v.decided_by) : invoke("open_rule", { source: src }));
            // An edit for this one item (24 Nov: he couldn't tell it hid only Omen of Return, not every omen).
            const only = (src?.kind === "edit" || src?.kind === "preset") && !!src.only_base;
            const rest = groupName ? <>the rest of <b>{groupName}</b></> : "the rest of its group";
            const scopeText = !only ? undefined : !shown(v.visibility)
              ? <>only {src!.only_base} is hidden; {rest} still shows as before.</>
              : <>only {src!.only_base} looks like this; {rest} keeps NeverSink's look.</>;
            const chips = shown(v.visibility)
              ? [st?.muted ? "No sound" : st?.sound ? `🔊 ${st.sound}` : "Default sound", st?.beam ? `${st.beam} beam` : "", st?.minimap ? "Map icon" : ""]
              : [`${only ? `Only ${src!.only_base} hidden` : "Hidden on the ground"}${extra?.stackable && status!.by_stack.length === 0 ? ", every stack size" : ""}`];
            return (
              <>
                <div className="qm-chips">{chips.filter(Boolean).map((c) => <span key={c} className="qm-chip">{c}</span>)}</div>
                <RuleLine src={src} hidden={!shown(v.visibility)} onOpen={canOpen ? change : undefined}
                  action="Modify this rule" scope={scopeText} />
                {!replacing && <ResetLine item={picked.item} fromScreen={picked.from_screen} stamp={JSON.stringify(v.source) + v.visibility}
                  onReset={async (call) => { try { await call; await afterChange("Back to the base filter's rule."); } catch (e) { setToast(String(e)); } }} />}
              </>
            );
          })()}
          {item.item_class === "Jewels" && (item.mod_names?.length ?? 0) > 0 && !replacing && (
            <JewelModsBox names={item.mod_names!} onSave={async (call, msg) => {
              try { const r = await call; await afterChange(msg, r?.warnings.filter((w) => !w.includes("is gone in this version"))[0]); }
              catch (e) { setToast(String(e)); }
            }} />
          )}
          {group && (
            <div className="qm-scope-box">
              <div className="qm-scope" role="radiogroup" aria-label="What the buttons below change">
                <button role="radio" aria-checked={scope === "group"} className={scope === "group" ? "on" : ""} onClick={() => setScope("group")} title={`All of ${groupName}`}>Whole group</button>
                <button role="radio" aria-checked={scope === "item"} className={scope === "item" ? "on" : ""} onClick={() => setScope("item")} title={`Just ${itemTitle(item)}`}>Just this item</button>
              </div>
              <div className="qm-applies">{scope === "group"
                ? <>Changes <b>{groupName}</b> (every one of them), the same rule as on the Tiers page.</>
                : <>Makes your own edit for just <b>{itemTitle(item)}</b>, shown in My edits. It wins over <b>{groupName}</b>.</>}</div>
            </div>
          )}
          {status!.by_stack.length > 0 && <div className="qm-facts">Stacks: {stackLine(status!.by_stack)}</div>}
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

      <div className="qm-row">
        {(() => {
          // Show · Hide · Minimap icon only, with the item's current state lit up.
          const isHidden = status?.verdict.visibility === "Hide";
          const isIcon = !isHidden && !!status?.verdict.style?.label_hidden;
          const isShown = !!status && !isHidden && !isIcon;
          const showIt = () => (isHidden ? unhide() : isIcon ? saveStyle({ ...current(), label_hidden: false }) : undefined);
          return (
            <div className="qm-seg">
              <button className={`qm-btn show ${isShown ? "on" : ""}`} onClick={showIt} title="Show the item with its label">Show</button>
              <button id="qm-hide" className={`qm-btn hide ${isHidden ? "on" : ""}`} onClick={hide} title="Hide it on the ground">Hide</button>
              <button className={`qm-btn ${isIcon ? "on" : ""}`} onClick={iconOnly} disabled={!status && !wholeType}
                title="No label: just the minimap icon (and beam, if set)">Minimap icon only</button>
            </div>
          );
        })()}

      </div>

      {/* Hide on a stack hides 38 of them too (25 Nov): point at the cut-off instead. */}
      {extra?.stackable && !wholeType && status && status.verdict.visibility !== "Hide" && !(group && scope === "group") && (
        <div className="qm-hint">Hide covers every stack size. To keep the big stacks, use <b>Only show {stackN.toLocaleString()}+</b> {inGame ? <button className="qm-inline" onClick={() => toggleMore(true)}>(More options)</button> : "on the right"}.</div>
      )}

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
      {(status || wholeType) && (looks.length > 0 || shown(status?.verdict.visibility ?? "Show")) && (
        <>
          <div className="qm-label">My saved looks</div>
          <LookChips text={item.base_type || item.item_class} looks={looks} picked={replacing?.detail.look}
            onPick={(l) => saveStyle(l.style, l.name)}
            onSave={shown(status?.verdict.visibility ?? "Show") ? saveLook : undefined} />
        </>
      )}
      <button className="qm-custom" onClick={() => setEditing(true)} disabled={!status && !wholeType}>
        Customise colours, beam, icon & sound…
      </button>

      {/* Less-used settings, folded away (remembered): stack sizes, when it applies, every rarity, auto reload. */}
      {/* In the editor window there's room: everything shows, these sit in the right-hand column (.qm-sheet). */}
      {inGame && <button className="qm-more-toggle" onClick={() => toggleMore(!more)}>
        {more ? "▾" : "▸"} More options
        {!more && <span className="muted small"> · {[extra?.stackable && "stack sizes", "when it applies", isGear && "every rarity", inGame && "auto reload"].filter(Boolean).join(" · ")}</span>}
      </button>}
      {(more || !inGame) && (
        <div className="qm-more">
          {!inGame && <div className="qm-side-title">Stack sizes and when it applies</div>}
      {!wholeType && extra?.stackable && status && (
        <div className="qm-stack">
          <div className="qm-label">Stack size: <b>{stackN.toLocaleString()}</b>{item.stack_size ? ` (this one: ${item.stack_size.toLocaleString()})` : ""}</div>
          <input type="range" min={0} max={steps.length - 1} value={pickIndex}
            onChange={(e) => setStackPick(+e.target.value)} />
          <div className="qm-row">
            <button className="qm-btn small hide" onClick={() => hideBelow(stackN)} disabled={stackN <= 1}
              title={`Hide stacks smaller than ${stackN.toLocaleString()} and show the rest`}>
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
      {(item.corrupted || onlyCorrupted) && (
        <label className="qm-check" title="e.g. show corrupted rare jewels while ordinary ones stay hidden">
          <input type="checkbox" checked={onlyCorrupted} onChange={(e) => setOnlyCorrupted(e.target.checked)} />
          <span>Only corrupted ones</span>
        </label>
      )}
      {onlyCorrupted && !replacing && (
        <div className="qm-hint">Ticking alone saves nothing: now pick a highlight colour (or Hide) above, and it applies to corrupted ones only.</div>
      )}

      {inGame && (
        <label className="qm-check">
          <input type="checkbox" checked={autoReload} onChange={(e) => toggleAuto(e.target.checked)} />
          <span>Reload {gf ? <b>{nameOf(gf.expected)}</b> : "the filter"} in game after each change (F4 does it by hand)</span>
        </label>
      )}
        </div>
      )}

      <div className="qm-foot">
        {!replacing && <button className="qm-link" onClick={undo}>Undo last</button>}
        <button className="qm-link" onClick={onClose}>{inGame ? "Close (Esc)" : "Done"}</button>
      </div>
      {toast && <div className="qm-toast">{toast}</div>}
    </div>
  );
}

/** F2 on an identified jewel: tick its mods to add them to "Jewels with my mods" (the list on Tiers → Jewels).
 *  One Save, since saving reloads the filter and closes F2. */
function JewelModsBox({ names, onSave }: { names: string[]; onSave: (call: Promise<{ label: string; warnings: string[] } | null>, msg: string) => void }) {
  const [rule, setRule] = useState<JewelRule | null | undefined>(undefined);
  const [all, setAll] = useState<JewelMod[]>([]);
  const [ticked, setTicked] = useState<string[]>([]);
  useEffect(() => {
    readJewelRule().then((r) => { setRule(r); setTicked(names.filter((n) => r?.mods.includes(n))); });
    jewelModList().then(setAll);
  }, [names.join("|")]); // eslint-disable-line react-hooks/exhaustive-deps
  if (rule === undefined) return null;
  const have = rule?.mods ?? [];
  const textOf = (n: string) => all.find((m) => m.name === n)?.text.replace(/\n/g, " · ") ?? n;
  const next = [...have.filter((n) => !names.includes(n)), ...ticked];
  const changed = next.length !== have.length || next.some((n) => !have.includes(n));
  const others = have.filter((n) => !names.includes(n)).length;
  const keep = rule?.styled && rule.look ? fromCurrent(rule.look, "Rare") : null;
  return (
    <div className="qm-jm">
      <div className="qm-jm-h"><b>Jewels with my mods</b>
        <span className="muted small">{have.length === 0 ? "Tick mods to highlight jewels that have them" : `${have.length} in your list${others ? ` (${others} from other jewels)` : ""}`}</span></div>
      {names.map((n) => (
        <label key={n} className="qm-check">
          <input type="checkbox" checked={ticked.includes(n)} onChange={(e) => setTicked(e.target.checked ? [...ticked, n] : ticked.filter((x) => x !== n))} />
          <span>{textOf(n)} <span className="muted small">{n}</span></span>
        </label>
      ))}
      {changed && (
        <button className="qm-change" onClick={() => onSave(saveJewelRule(next, rule?.min ?? 0, null, keep),
          next.length ? `Jewels with my mods: ${next.length} mod${next.length > 1 ? "s" : ""}.` : "Jewels with my mods removed.")}>
          Save to my jewel list
        </button>
      )}
    </div>
  );
}

type ResetReply = { what: string[]; note: string | null; after: string; base?: string };
/** "Reset to base filter" (24 Nov, he asked for a reset on F2; named for the filter yours starts from, which isn't
 *  always NeverSink's): says what it undoes and whether that base filter shows or hides the item then. */
function ResetLine({ item, fromScreen, stamp, onReset }: { item: Item; fromScreen: boolean; stamp: string; onReset: (call: Promise<unknown>) => void }) {
  const [r, setR] = useState<ResetReply | null>(null);
  useEffect(() => {
    let live = true;
    invoke<ResetReply>("reset_item", { item, fromScreen, apply: false }).then((x) => { if (live) setR(x); }).catch(() => { if (live) setR(null); });
    return () => { live = false; };
  }, [stamp]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!r || (r.what.length === 0 && !r.note)) return null;
  return (
    <div className="qm-reset">
      {r.what.length > 0 && (
        <>
          <button className="qm-reset-btn" onClick={() => onReset(invoke("reset_item", { item, fromScreen, apply: true }))}>Reset to base filter</button>
          <span className="muted small">Undoes {r.what.join(" and ")}. Then it's <b>{r.after}</b>, as your base filter{r.base ? <> (<b>{r.base}</b>)</> : ""} has it.</span>
        </>
      )}
      {r.note && <span className="muted small">{r.note}</span>}
    </div>
  );
}
