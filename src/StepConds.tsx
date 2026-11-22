import { useEffect, useRef, useState } from "react";

/** One condition of a NeverSink step, as the core sends it (core/src/conds.rs). */
export type Cond = { key: string; value: string; words: string; kind: "number" | "yesno" | "rarity" | "fixed" };

/** What "+ Add a condition" offers: keyword, plain name, kind, the value it starts with. */
type Addable = { key: string; label: string; kind: Cond["kind"]; start: string; also?: Record<string, string>; hint?: string };
/** Every condition "+ Add a condition" can offer, once. */
const POOL: Record<string, Addable> = {
  ItemLevel: { key: "ItemLevel", label: "Item level", kind: "number", start: ">= 84", hint: "Higher item level can roll better mods" },
  UnidentifiedItemTier: { key: "UnidentifiedItemTier", label: "Item tier", kind: "number", start: ">= 4", hint: "The tier shown on unidentified rare labels" },
  Rarity: { key: "Rarity", label: "Rarity", kind: "rarity", start: "Rare" },
  Corrupted: { key: "Corrupted", label: "Corrupted or not", kind: "yesno", start: "False" },
  CorruptedMods: { key: "CorruptedMods", label: "Has a corruption mod", kind: "number", start: ">= 1", hint: "The extra mod a Vaal Orb can add" },
  ModCount: { key: "ModCount", label: "Number of mods (identified only)", kind: "number", start: ">= 4", also: { Identified: "True" },
    hint: "Only works once an item is identified, so this also adds “identified”" },
  EnchantmentPassiveNum: { key: "EnchantmentPassiveNum", label: "Cluster passives", kind: "number", start: ">= 8", hint: "“Adds N Passive Skills” on cluster jewels" },
  Identified: { key: "Identified", label: "Identified or not", kind: "yesno", start: "True" },
  Quality: { key: "Quality", label: "Quality", kind: "number", start: ">= 20" },
  LinkedSockets: { key: "LinkedSockets", label: "Links", kind: "number", start: ">= 5" },
  Sockets: { key: "Sockets", label: "Sockets", kind: "number", start: ">= 6" },
  FracturedItem: { key: "FracturedItem", label: "Fractured or not", kind: "yesno", start: "True" },
  Mirrored: { key: "Mirrored", label: "Mirrored or not", kind: "yesno", start: "False" },
  AreaLevel: { key: "AreaLevel", label: "Area level (where it drops)", kind: "number", start: ">= 68", hint: "68+ = maps, below = the campaign" },
  StackSize: { key: "StackSize", label: "Stack size", kind: "number", start: ">= 3", hint: "Only stacks of at least this many" },
  GemLevel: { key: "GemLevel", label: "Gem level", kind: "number", start: ">= 20" },
  TransfiguredGem: { key: "TransfiguredGem", label: "Transfigured or not", kind: "yesno", start: "True" },
  DropLevel: { key: "DropLevel", label: "Base level (drop level)", kind: "number", start: ">= 30", hint: "How late in the game the base first drops" },
  Replica: { key: "Replica", label: "Replica or not", kind: "yesno", start: "True" },
  Foulborn: { key: "Foulborn", label: "Foulborn or not", kind: "yesno", start: "True" },
};
/** Which conditions each kind of section offers, most useful first. */
export type Menu = "jewels" | "cluster" | "gear" | "flasks" | "gems" | "currency" | "uniques" | "levelling" | "league" | "any";
const MENUS: Record<Menu, string[]> = {
  jewels: ["ItemLevel", "Corrupted", "CorruptedMods", "ModCount", "Rarity", "Identified", "Mirrored"],
  cluster: ["ItemLevel", "EnchantmentPassiveNum", "Corrupted", "ModCount", "Rarity", "Identified", "Mirrored"],
  gear: ["ItemLevel", "UnidentifiedItemTier", "Rarity", "Corrupted", "CorruptedMods", "ModCount", "Identified", "Quality", "LinkedSockets", "Sockets",
    "FracturedItem", "Mirrored", "AreaLevel"],
  flasks: ["ItemLevel", "Quality", "Rarity", "Corrupted", "Identified", "ModCount", "AreaLevel", "Mirrored"],
  gems: ["GemLevel", "Quality", "Corrupted", "TransfiguredGem", "AreaLevel", "Mirrored"],
  currency: ["StackSize", "AreaLevel"],
  uniques: ["LinkedSockets", "Sockets", "Corrupted", "CorruptedMods", "Quality", "ItemLevel", "Identified", "Mirrored", "Replica", "Foulborn", "AreaLevel"],
  levelling: ["AreaLevel", "ItemLevel", "DropLevel", "Rarity", "LinkedSockets", "Sockets", "Quality", "Identified", "Corrupted"],
  league: ["ItemLevel", "Rarity", "Corrupted", "Identified", "StackSize", "AreaLevel", "Quality"],
  any: ["ItemLevel", "Corrupted", "Rarity", "Identified", "Quality", "AreaLevel", "StackSize", "LinkedSockets", "Sockets", "Mirrored", "FracturedItem"],
};

const RARITIES: [string, string][] =[["Normal", "white"], ["Magic", "magic"], ["Rare", "rare"], ["Unique", "unique"]];
const YES_WORDS: Record<string, [string, string]> = {
  Corrupted: ["corrupted", "not corrupted"], Identified: ["identified", "unidentified"], Mirrored: ["mirrored", "not mirrored"],
  FracturedItem: ["fractured", "not fractured"], SynthesisedItem: ["synthesised", "not synthesised"], Replica: ["replica", "not replica"],
  AnyEnchantment: ["enchanted", "not enchanted"], BlightedMap: ["blighted", "not blighted"], TransfiguredGem: ["transfigured", "not transfigured"],
  Foulborn: ["foulborn", "not foulborn"],
};
/** The plain name of a number condition, for the editor's sentence. */
const NUM_NAME: Record<string, string> = {
  ItemLevel: "item level", AreaLevel: "area level", Quality: "quality", StackSize: "stack size", LinkedSockets: "links",
  Sockets: "sockets", GemLevel: "gem level", MapTier: "map tier", EnchantmentPassiveNum: "passives", CorruptedMods: "corruption mods",
  UnidentifiedItemTier: "item tier", MemoryStrands: "memory strands", DropLevel: "drop level", ModCount: "mods", Width: "width", Height: "height",
  HasSearingExarchImplicit: "Exarch implicit level", HasEaterOfWorldsImplicit: "Eater implicit level",
};

/** ">= 86" -> [">=", 86]; two lines (68+ and up to 74) -> the first. */
function parseNum(v: string): [string, number, number | null] {
  const lines = v.split("\n").map((l) => l.trim());
  const one = (l: string): [string, number] => {
    const m = l.match(/^(>=|<=|==|>|<|=)?\s*(\d+)/);
    if (!m) return [">=", 0];
    const op = m[1] ?? "==", n = parseInt(m[2], 10);
    return op === ">" ? [">=", n + 1] : op === "<" ? ["<=", n - 1] : op === "=" ? ["==", n] : [op, n];
  };
  const [op, n] = one(lines[0] ?? "");
  if (lines.length === 2) { const [op2, n2] = one(lines[1]); if (op === ">=" && op2 === "<=") return ["range", n, n2]; }
  return [op, n, null];
}

/** The little box that opens when you click a chip. */
function Editor({ cond, changed, onSave, onRemove, onUndo, onClose }: {
  cond: Cond; changed: boolean; onSave: (v: string) => void; onRemove: () => void; onUndo: () => void; onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const off = (e: MouseEvent) => { if (ref.current && !ref.current.contains(e.target as Node)) onClose(); };
    window.setTimeout(() => document.addEventListener("mousedown", off), 0);
    return () => document.removeEventListener("mousedown", off);
  }, [onClose]);
  const [op0, n0, hi0] = parseNum(cond.value);
  const [op, setOp] = useState(op0);
  const [n, setN] = useState(String(n0));
  const [hi, setHi] = useState(String(hi0 ?? n0));
  const rar0 = cond.value.replace(/^[<>=]+\s*/, "").split(/\s+/);
  const [rar, setRar] = useState<string[]>(cond.kind === "rarity" && /^[<>]/.test(cond.value)
    ? RARITIES.map((r) => r[0]).filter((r, i) => { const at = RARITIES.findIndex((x) => x[0] === rar0[0]); return cond.value.startsWith("<") ? i <= at : i >= at; })
    : rar0);
  const name = NUM_NAME[cond.key] ?? cond.key;
  const save = () => {
    if (cond.kind === "number") {
      const a = parseInt(n, 10), b = parseInt(hi, 10);
      if (isNaN(a)) return;
      onSave(op === "range" && !isNaN(b) ? `>= ${Math.min(a, b)}\n<= ${Math.max(a, b)}` : `${op} ${a}`);
    } else if (cond.kind === "rarity") {
      if (rar.length) onSave(RARITIES.map((r) => r[0]).filter((r) => rar.includes(r)).join(" "));
    }
  };
  return (
    <div className="cond-edit" ref={ref} onKeyDown={(e) => { if (e.key === "Enter") save(); if (e.key === "Escape") onClose(); }}>
      {cond.kind === "number" && (
        <div className="cond-edit-row">
          <span className="muted small">{name[0].toUpperCase() + name.slice(1)}</span>
          <select value={op} onChange={(e) => setOp(e.target.value)}>
            <option value=">=">at least</option>
            <option value="<=">at most</option>
            <option value="==">exactly</option>
            <option value="range">from … to</option>
          </select>
          <input type="number" min={0} autoFocus value={n} onChange={(e) => setN(e.target.value)} />
          {op === "range" && <><span className="muted small">to</span><input type="number" min={0} value={hi} onChange={(e) => setHi(e.target.value)} /></>}
        </div>
      )}
      {cond.kind === "yesno" && (
        <div className="cond-edit-row">
          {(YES_WORDS[cond.key] ?? ["yes", "no"]).map((w, i) => {
            const v = i === 0 ? "True" : "False";
            return <button key={v} className={cond.value.trim() === v ? "primary" : ""} onClick={() => onSave(v)}>{w[0].toUpperCase() + w.slice(1)}</button>;
          })}
        </div>
      )}
      {cond.kind === "rarity" && (
        <div className="cond-edit-row">
          {RARITIES.map(([r, w]) => (
            <label key={r} className={`cond-rar r-${r.toLowerCase()}`}>
              <input type="checkbox" checked={rar.includes(r)} onChange={(e) => setRar(e.target.checked ? [...rar, r] : rar.filter((x) => x !== r))} />{w}
            </label>
          ))}
        </div>
      )}
      <div className="cond-edit-row">
        {cond.kind !== "yesno" && <button className="primary" onClick={save}>Save</button>}
        <button className="link small" onClick={onRemove} title="This step then ignores it">Remove this condition</button>
        {changed && <button className="link small" onClick={onUndo}>Back to NeverSink's</button>}
      </div>
    </div>
  );
}

/** A step's conditions as chips: click one to change it, "+ Add a condition" for more. Changed ones are gold. */
export default function StepConds({ conds, changed, menu, onSet }: {
  conds: Cond[]; changed: string[]; menu: Menu;
  onSet: (set: Record<string, string>, undo: string[], msg: string) => void;
}) {
  const [open, setOpen] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  // Conditions you removed aren't in the step any more: show them crossed out so they can come back.
  const removed = changed.filter((k) => !conds.some((c) => c.key === k));
  const jewels = menu === "jewels" || menu === "cluster";
  const offer = MENUS[menu].map((k) => POOL[k]).filter((a) => !conds.some((c) => c.key === a.key));
  const shown = conds.filter((c) => c.key !== "Class" || !jewels);
  return (
    <div className="conds">
      {shown.length === 0 && removed.length === 0 && <span className="muted small">Catches every item listed, with no other conditions.</span>}
      {shown.map((c) => {
        const edit = c.kind !== "fixed";
        const isChanged = changed.includes(c.key);
        return (
          <span key={c.key} className="cond-wrap">
            <button className={`cond ${edit ? "edit" : "fixed"} ${isChanged ? "changed" : ""}`} disabled={!edit}
              title={edit ? "Click to change" : "NeverSink's condition (can't be changed here)"}
              onClick={() => setOpen(open === c.key ? null : c.key)}>
              {c.words}{edit && <span className="cond-pen" aria-hidden>✎</span>}
            </button>
            {open === c.key && (
              <Editor cond={c} changed={isChanged} onClose={() => setOpen(null)}
                onSave={(v) => { setOpen(null); onSet({ [c.key]: v }, [], "Step changed.") }}
                onRemove={() => { setOpen(null); onSet({ [c.key]: "" }, [], `“${c.words}” removed from this step.`); }}
                onUndo={() => { setOpen(null); onSet({}, [c.key], "Back to NeverSink's."); }} />
            )}
          </span>
        );
      })}
      {removed.map((k) => (
        <button key={k} className="cond removed" title="You removed this. Click to put NeverSink's back." onClick={() => onSet({}, [k], "Back to NeverSink's.")}>
          {NUM_NAME[k] ?? YES_WORDS[k]?.[0] ?? k.toLowerCase()} <span className="cond-pen">↺</span>
        </button>
      ))}
      <span className="cond-wrap">
        {offer.length > 0 && <button className="cond add" onClick={() => setAdding(!adding)}>+ Add a condition</button>}
        {adding && (
          <div className="cond-edit cond-menu" onMouseLeave={() => setAdding(false)}>
            {offer.map((a) => (
              <button key={a.key} className="cond-menu-item" onClick={() => { setAdding(false); onSet({ [a.key]: a.start, ...(a.also ?? {}) }, [], `Added “${a.label.toLowerCase()}”. Click it to set the number.`); }}>
                <b>{a.label}</b>{a.hint && <span className="muted small">{a.hint}</span>}
              </button>
            ))}
          </div>
        )}
      </span>
    </div>
  );
}
