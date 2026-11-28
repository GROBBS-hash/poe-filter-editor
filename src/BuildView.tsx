import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type CSSProperties } from "react";
import UpgradeFinder from "./UpgradeFinder";
import { ItemIcon, useIcons } from "./icons";
import TreeView from "./TreeView";
import { CRAFT_ITEM_KEY, CRAFT_PLAN_MARK } from "./Craft";

export type PobGem = { name: string; level: number; quality: number; enabled: boolean; support: boolean; colour: string };
export type PobGroup = { label: string; slot: string; enabled: boolean; gems: PobGem[] };
export type PobGear = { slot: string; rarity: string; name: string; base: string; lines: string[]; copy?: string };
/** A jewel in a tree socket (node) or in an item's abyssal socket (node null, slot "Belt Abyssal Socket 1"). */
export type PobJewel = { node: number | null; item: PobGear; notables: string[] };
/** Spectres and the Animate Guardian's gear (28 Nov). `main`: picked on a Raise Spectre gem; the rest are the build's spectre list. */
export type PobMinions = { spectres: { id: string; name: string; main: boolean }[]; spectre_gem: PobGem | null; spectre_limit: number | null;
  guardian_gem: PobGem | null; guardian_set: string; guardian: PobGear[] };
export type PobStage = { title: string; tree: { version: string; nodes: number[]; url: string } | null; groups: PobGroup[]; gear: PobGear[]; jewels?: PobJewel[]; minions?: PobMinions };
/** alternatives: items in the PoB that no stage wears (the guide's options and upgrades). */
export type PobBuild = { class: string; ascendancy: string; level: number; stages: PobStage[]; active: number; alternatives?: PobGear[] };
export type CheckNote = { kind: "bad" | "warn" | "ok"; text: string };
export type StageCheck = { title: string; level: number; level_guessed: boolean; str: number; dex: number; int: number; points: number; points_have: number;
  mana: { total: number; reserved: number; free: number; skill: string; cost: number | null } | null;
  speed: { skill: string; fires_after: number; attack_every: number; spare_pct: number } | null; notes: CheckNote[];
  /** Gear needing a higher level than the stage starts at (and a base to wear until then). */
  later?: { slot: string; level: number; instead: string | null }[] };
export type PobState = { build: PobBuild; stage: number; link: string; follow?: boolean; checks?: StageCheck[] };

/** Red count for things that won't work, yellow for later or weak, a tick when clean. */
function CheckBadge({ c }: { c: StageCheck }) {
  const bad = c.notes.filter((n) => n.kind === "bad").length, warn = c.notes.filter((n) => n.kind === "warn").length;
  if (!bad && !warn) return <span className="bv-cbadge ok" title="Nothing wrong">✓</span>;
  return <>{bad > 0 && <span className="bv-cbadge bad" title={`${bad} won't work`}>{bad}</span>}{warn > 0 && <span className="bv-cbadge warn" title={`${warn} to watch`}>{warn}</span>}</>;
}

const RARITY: Record<string, string> = { normal: "#c8c8c8", magic: "#8888ff", rare: "#ffff77", unique: "#af6025", relic: "#82ad6a" };
const SLOT_ORDER = ["Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Amulet", "Ring 1", "Ring 2", "Belt",
  "Flask 1", "Flask 2", "Flask 3", "Flask 4", "Flask 5"];
const SLOT_NAME: Record<string, string> = { "Weapon 1": "Weapon", "Weapon 2": "Off hand", "Body Armour": "Body" };

/** Opens the price window for one of the build's items or gems, like Ctrl+D on it in game. */
const priceItem = (it: PobGear) => invoke("price_build_item", { text: it.copy ?? "" }).catch(() => null);
const priceGem = (g: PobGem) => invoke("price_build_item", { gem: g }).catch(() => null);

function PriceButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="bv-price" title="Price check this item (the price window, like Ctrl+D)"
      onClick={(e) => { e.stopPropagation(); onClick(); }}>Price</button>
  );
}

/** "Craft": opens the editor's Crafting page with this item, to see how to make it (uniques can't be crafted; jewels,
 *  abyss jewels and cluster jewels can). */
function CraftButton({ it }: { it: PobGear }) {
  if (it.rarity === "unique" || !it.copy || it.slot.startsWith("Flask") || /Timeless|Prismatic Jewel/.test(it.base)) return null;
  return (
    <button className="bv-price" title="How to craft this item (opens the Crafting page with it)"
      onClick={(e) => { e.stopPropagation(); try { localStorage.setItem(CRAFT_ITEM_KEY, CRAFT_PLAN_MARK + (it.copy ?? "")); } catch { /* not passed */ } invoke("open_editor", { tab: "craft" }); }}>Craft</button>
  );
}

type BuyResult = { total: number; of: number; need: number; level?: number | null; any_level?: boolean; base_level?: number | null; swapped?: string | null };

/** "Buy": opens the trade site in your browser, searching for an item with the mods PoB lists. */
type BaseResult = { total: number; ilvl: number; notables: [string, number][] };

/** "Buy base" on a cluster jewel: the base to craft it on, with the item level its notables need. Often 1c against
 *  a finished jewel's price (guides say: reforge at the Harvest bench rather than buy exact jewels). */
function BuyBaseButton({ it }: { it: PobGear }) {
  const [state, setState] = useState<"idle" | "busy" | BaseResult | string>("idle");
  const go = (e: { stopPropagation(): void }) => {
    e.stopPropagation();
    setState("busy");
    invoke<BaseResult>("buy_base_item", { text: it.copy }).then(setState).catch((err) => setState(friendly(err)));
  };
  const r = typeof state === "object" ? state : null;
  const need = r?.notables.filter(([, l]) => l === r.ilvl).map(([n]) => n) ?? [];
  const note = r ? `${r.total} bases · item level ${r.ilvl}+${need.length && r.ilvl > 1 ? ` (${need.join(", ")})` : ""}` : typeof state === "string" && state !== "idle" && state !== "busy" ? state : "";
  return (
    <>
      {note && <span className={`bv-buynote small ${typeof state === "string" ? "bad" : ""}`} title={r ? r.notables.map(([n, l]) => `${n}: item level ${l}+`).join("\n") : note}>{note}</span>}
      <button className="bv-price bv-buybase" disabled={state === "busy"}
        title={"Search the trade site for the empty base to craft this on: same passive count and enchant, item level high\n" +
          "enough for its notables to roll (from the game's data, checked against poedb), cheapest first"}
        onClick={go}>{state === "busy" ? "Searching…" : "Buy base"}</button>
    </>
  );
}

function BuyButton({ it, stage }: { it: PobGear; stage?: string }) {
  const [state, setState] = useState<"idle" | "busy" | BuyResult | string>("idle");
  const go = (e: { stopPropagation(): void }) => {
    e.stopPropagation();
    setState("busy");
    invoke<BuyResult | null>("buy_build_item", { text: it.copy, stage: stage ?? null }).then((r) => setState(r ?? "No answer from the trade site")).catch((err) => setState(String(err)));
  };
  const note = typeof state === "object" && state
    ? `${state.total} for sale${state.need < state.of ? ` · any ${state.need} of ${state.of} mods` : ""}${state.swapped ? ` · as ${state.swapped} (${it.base} needs level ${state.base_level})` : ""}${state.base_level && !state.swapped ? ` · this base needs level ${state.base_level}` : state.any_level ? ` · none you can wear at level ${state.level}, showing all` : state.level ? ` · wearable at level ${state.level}` : ""}`
    : state !== "idle" && state !== "busy" ? state : "";
  return (
    <>
      {note && <span className={`bv-buynote small ${typeof state === "string" ? "bad" : ""}`} title={note}>{note}</span>}
      <button className="bv-price bv-buy" disabled={state === "busy"}
        title={"Search the trade site for this item with the mods your build lists (totals for life and resistances,\n" +
          "10% under PoB's numbers, crafted mods left out) and open it in your browser"}
        onClick={go}>{state === "busy" ? "Searching…" : "Buy"}</button>
    </>
  );
}

type MercBuy = { total: number; need: number; greater: boolean; skill: string; supports: string[] };
/** A Mercenary stage's first row: Mercenary Warrants with the same main attack and supports (28 Nov). */
function MercRow({ groups }: { groups: PobGroup[] }) {
  const [state, setState] = useState<"idle" | "busy" | MercBuy | string>("idle");
  const buy = () => {
    setState("busy");
    invoke<MercBuy>("buy_build_merc", { groups }).then(setState).catch((err) => setState(String(err)));
  };
  const note = typeof state === "object"
    ? `${state.total} for sale · ${state.need < state.supports.length ? `${state.need} of its ${state.supports.length} supports` : "all its supports"}${state.greater ? " as Greater" : ", any tier"}`
    : state !== "idle" && state !== "busy" ? state : "";
  const main = groups.find((g) => g.enabled && g.gems.some((x) => !x.support))?.gems.find((x) => !x.support);
  return (
    <li>
      <div className="bv-item">
        <span className="bv-slot muted small">Mercenary</span>
        <ItemIcon name={null} base="Mercenary Warrant" size={26} />
        <span className="bv-iname">
          <span>A Mercenary like this</span>
          <span className="muted small"> {typeof state === "object" ? `${state.skill} with ${state.supports.join(", ")}` : `its main attack${main ? ` (${main.name})` : ""} and supports`}</span>
        </span>
        {note && <span className={`bv-buynote small ${typeof state === "string" ? "bad" : ""}`} title={note}>{note}</span>}
        <button className="bv-price bv-buy" disabled={state === "busy"}
          title={"Search the trade site for instant-buyout Mercenary Warrants with this main attack and its supports as Greater\n(Tier 3), no Wild Strike (and no Pierce on Frost Blades), cheapest first. The site can't tell which skill a support\nis on, so check the warrant's skill list before you buy."}
          onClick={buy}>{state === "busy" ? "Searching…" : "Buy"}</button>
        <PriceButton onClick={() => invoke("price_build_merc", { groups }).catch(() => null)} />
      </div>
    </li>
  );
}

/** The gem's picture from the game (its colour dot when there's none). */
function GemPic({ g }: { g: PobGem }) {
  const icons = useIcons();
  const url = icons[g.name] || icons[`${g.name} Support`];
  return url ? <img className="gem-pic" src={url} alt="" width={18} height={18} loading="lazy" /> : <span className="gem-dot" />;
}

/** PoB's look: socket colours and supports a little smaller. Click = price check. */
function GemPill({ g, main, fresh }: { g: PobGem; main: boolean; fresh?: boolean }) {
  return (
    <span onClick={() => priceGem(g)} className={`gem gem-${g.colour} ${g.support ? "sup" : ""} ${main ? "main" : ""} ${g.enabled ? "" : "off"} ${fresh ? "fresh" : ""}`}
      title={`${g.name} · level ${g.level}${g.quality ? ` · ${g.quality}% quality` : ""}${g.enabled ? "" : " (switched off in PoB)"}\nClick to price check`}>
      <GemPic g={g} />{g.name}
      {(g.level > 1 || g.quality > 0) && <span className="gem-lvl">{g.level}{g.quality ? `/${g.quality}` : ""}</span>}
    </span>
  );
}

/** The item's sockets as PoB has them ("Sockets: R-G-B B"): coloured dots, linked ones joined by a bar. */
function SocketStrip({ it }: { it: PobGear }) {
  const line = (it.copy ?? "").split("\n").concat(it.lines).find((l) => l.startsWith("Sockets: "));
  if (!line) return null;
  const groups = line.slice(9).trim().split(/\s+/).map((g) => g.split("-").filter(Boolean));
  return (
    <span className="bv-sockets" title={`Sockets: ${line.slice(9).trim()}`}>
      {groups.map((g, i) => (
        <span key={i} className="bv-link">
          {g.map((c, j) => <span key={j} className={`bv-sock sock-${c.toLowerCase()}`} />)}
        </span>
      ))}
    </span>
  );
}

/** What's new on the tree at this stage compared with the one before. */
function treeChange(stages: PobStage[], i: number) {
  const cur = stages[i]?.tree?.nodes ?? [];
  const prev = new Set(stages[i - 1]?.tree?.nodes ?? []);
  const added = cur.filter((n) => !prev.has(n)).length;
  const removed = (stages[i - 1]?.tree?.nodes ?? []).filter((n) => !cur.includes(n)).length;
  return { total: cur.length, added, removed };
}

/** Gear checklist: the build's items you already have, ticked by hand. Kept in this PC's app storage, shared by the
 *  F6 window and My build (a tick in one shows in the other). One key per item: slot, name and base, so the same item
 *  stays ticked in every stage that uses it. */
const HAVE_KEY = "build.have";
const haveKey = (it: PobGear) => `${it.slot}|${it.name}|${it.base}`;
function readHave(): Set<string> {
  try { return new Set(JSON.parse(localStorage.getItem(HAVE_KEY) || "[]")); } catch { return new Set(); }
}
function useHave() {
  const [have, setHave] = useState<Set<string>>(readHave);
  useEffect(() => {
    const on = (e: StorageEvent) => { if (e.key === HAVE_KEY) setHave(readHave()); };
    window.addEventListener("storage", on);
    return () => window.removeEventListener("storage", on);
  }, []);
  const toggle = (k: string) => {
    const next = new Set(readHave());
    if (next.has(k)) next.delete(k); else next.add(k);
    try { localStorage.setItem(HAVE_KEY, JSON.stringify([...next])); } catch { /* not kept */ }
    setHave(next);
  };
  return [have, toggle] as const;
}

/** "Have 4 of 11 · hide what I have" over the gear and jewel lists. */
function HaveBar({ n, of, hide, setHide }: { n: number; of: number; hide: boolean; setHide: (b: boolean) => void }) {
  if (of === 0) return null;
  return (
    <li className="bv-havebar small">
      <span><b>Have {n} of {of}</b>{n === of ? " · all done" : <span className="muted"> · tick items as you get them</span>}</span>
      <label><input type="checkbox" checked={hide} onChange={(e) => setHide(e.target.checked)} /> hide what I have</label>
    </li>
  );
}

/** Where each slot sits on the game's character screen (grid column / row, spans). 28 Nov: he picked this look ("F")
 *  so the gear shows as item pictures and it's clear which gems go in which item. */
const DOLL: Record<string, [number, number, number, number]> = {
  "Weapon 1": [1, 1, 1, 3], "Helmet": [3, 1, 1, 1], "Amulet": [4, 1, 1, 1], "Weapon 2": [5, 1, 1, 3],
  "Ring 1": [2, 2, 1, 1], "Body Armour": [3, 2, 1, 2], "Ring 2": [4, 2, 1, 1],
  "Gloves": [2, 3, 1, 1], "Boots": [4, 3, 1, 1], "Belt": [3, 4, 1, 1],
};
const FLASKS = ["Flask 1", "Flask 2", "Flask 3", "Flask 4", "Flask 5"];
const slotName = (slot: string) => SLOT_NAME[slot] ?? slot;
const itemClass = (it: PobGear) => (it.copy ?? "").match(/^Item Class: (.+)$/m)?.[1]?.trim() ?? "";

/** The gear as the game's character screen: item pictures in their places, flasks under them. Click one to see it on the
 *  right: its sockets, the gems that go in it (each linked group on its own line), and Buy / Price / Craft. */
function GearDoll({ gear, groups, stage, have, onHave, later, newGems, empty }: {
  gear: PobGear[]; groups: PobGroup[]; stage: string; have: Set<string>; onHave: (it: PobGear) => void;
  later?: StageCheck["later"]; newGems: string[]; empty: string;
}) {
  const bySlot = new Map(gear.map((g) => [g.slot, g]));
  const others = gear.filter((g) => !DOLL[g.slot] && !FLASKS.includes(g.slot));
  const first = gear.find((g) => groups.some((x) => x.slot === g.slot)) ?? gear[0];
  const [pick, setPick] = useState<string | null>(null);
  const [mods, setMods] = useState(false);
  const sel = bySlot.get(pick ?? "") ?? first;
  if (gear.length === 0) return <div className="empty">{empty}</div>;
  const gemsIn = (slot: string) => groups.filter((g) => g.slot === slot);
  const cell = (slot: string, style?: CSSProperties) => {
    const it = bySlot.get(slot);
    const n = gemsIn(slot).reduce((a, g) => a + g.gems.length, 0);
    return (
      <button key={slot} style={style} className={`doll-cell ${it ? "" : "none"} ${it && sel?.slot === slot ? "on" : ""} ${it && have.has(haveKey(it)) ? "got" : ""}`}
        title={it ? `${slotName(slot)}: ${it.name}${it.base !== it.name ? ` (${it.base})` : ""}${n ? `\n${n} gems in it` : ""}` : `${slotName(slot)}: nothing in this stage`}
        onClick={() => it && (setPick(slot), setMods(false))}>
        {it ? <><ItemIcon name={it.rarity === "unique" ? it.name : null} base={it.base} size={slot.startsWith("Flask") ? 34 : 44} />
          {n > 0 && <span className="doll-n">{n}</span>}
          {have.has(haveKey(it)) && <span className="doll-got">✓</span>}</>
          : <span className="doll-slot">{slotName(slot)}</span>}
      </button>
    );
  };
  const l = sel && later?.find((x) => x.slot === sel.slot);
  const selGroups = sel ? gemsIn(sel.slot) : [];
  return (
    <div className="doll-wrap">
      <div className="doll-left">
        <div className="doll">
          {Object.entries(DOLL).map(([slot, [c, r, w, h]]) => cell(slot, { gridColumn: `${c} / span ${w}`, gridRow: `${r} / span ${h}` }))}
        </div>
        {FLASKS.some((f) => bySlot.has(f)) && <div className="doll-flasks">{FLASKS.map((f) => cell(f))}</div>}
        {others.length > 0 && <div className="doll-flasks">{others.map((g) => cell(g.slot))}</div>}
      </div>
      {sel && (
        <div className="doll-detail">
          <div className="doll-head">
            <ItemIcon name={sel.rarity === "unique" ? sel.name : null} base={sel.base} size={52} />
            <div className="doll-title">
              <span className="muted small">{slotName(sel.slot)}</span>
              <b style={{ color: RARITY[sel.rarity] ?? "#ccc" }}>{sel.name}</b>
              {sel.base !== sel.name && <span className="muted small">{sel.base}</span>}
              {l && <span className="bv-later">needs level {l.level}{l.instead ? ` · wear ${l.instead} until then` : ""}</span>}
            </div>
            <label className="doll-have" title="Tick it when you have it"><input type="checkbox" className="bv-have" checked={have.has(haveKey(sel))} onChange={() => onHave(sel)} /> have it</label>
          </div>
          <div className="bv-item doll-buttons">
            <SocketStrip it={sel} />
            <CraftButton it={sel} />
            {sel.copy && <BuyButton it={sel} stage={stage} />}
            {sel.copy && <PriceButton onClick={() => priceItem(sel)} />}
          </div>
          {selGroups.length > 0 ? (
            <div className="doll-gems">
              <div className="muted small">Gems in this {slotName(sel.slot).toLowerCase()}{selGroups.length > 1 ? " (each line is linked together)" : ""}</div>
              {selGroups.map((g, i) => (
                <div key={i} className={`bv-group ${g.enabled ? "" : "off"}`}>
                  {g.label && <div className="bv-glabel">{g.label}</div>}
                  <div className="bv-gems">{g.gems.map((x, j) => <GemPill key={j} g={x} main={!x.support && j === 0} fresh={newGems.includes(x.name)} />)}</div>
                </div>
              ))}
            </div>
          ) : !sel.slot.startsWith("Flask") && <div className="muted small doll-gems">No gems in this one.</div>}
          <button className="link small" onClick={() => setMods(!mods)}>{mods ? "Hide its mods" : "Show its mods"}</button>
          {mods && <ul className="bv-lines">{sel.lines.map((x, k) => <li key={k}>{x}</li>)}</ul>}
        </div>
      )}
    </div>
  );
}

/** The guide's spare items (in its PoB, worn in no stage), grouped by kind, doubles counted once, and ones this stage
 *  already wears left out. 28 Nov: "the alternatives make no sense sometimes, or have no explanation". */
function spareGroups(alts: PobGear[], worn: PobGear[]) {
  const wornNames = new Set(worn.map((g) => g.name));
  const out = new Map<string, { it: PobGear; n: number }[]>();
  for (const it of alts) {
    if (wornNames.has(it.name) && it.rarity === "unique") continue;
    const cls = itemClass(it) || "Other";
    const list = out.get(cls) ?? [];
    const same = list.find((x) => x.it.name === it.name && x.it.base === it.base);
    if (same) same.n++; else list.push({ it, n: 1 });
    out.set(cls, list);
  }
  return [...out.entries()].map(([cls, items]) => {
    // What it would replace: the worn item of the same kind.
    const names = [...new Set(worn.filter((g) => itemClass(g) === cls).map((g) => g.name))];
    const instead = names.length === 1 ? `instead of your ${names[0]}` : names.length > 1 ? `instead of one of your ${cls.toLowerCase()}` : "";
    return { cls, items, instead };
  });
}

export default function BuildView({ compact = false }: { compact?: boolean }) {
  const [have, toggleHave] = useHave();
  const [hideHave, setHideHave] = useState(() => { try { return localStorage.getItem("build.hideHave") === "1"; } catch { return false; } });
  useEffect(() => { try { localStorage.setItem("build.hideHave", hideHave ? "1" : "0"); } catch { /* not kept */ } }, [hideHave]);
  const [st, setSt] = useState<PobState | null>(null);
  // A jewel picked in the list (shown on the tree) or on the tree (opened in the list).
  const [jewelNode, setJewelNode] = useState<number | null>(null);
  // Gems live under their item on the Gear tab now (21 Nov): an old "gems" choice opens Gear.
  const [view, setView] = useState<"gear" | "jewels" | "tree" | "check" | "upgrades">(() => {
    try {
      const v = localStorage.getItem("build.view");
      return v === "jewels" || v === "tree" || v === "check" || v === "upgrades" ? v : "gear";
    } catch { return "gear"; }
  });
  const [openItem, setOpenItem] = useState<string | null>(null);
  // Who wears the gear shown: you, your Mercenary, the Animate Guardian, or the spectres (28 Nov, he asked for a section each).
  const [owner, setOwner] = useState<"you" | "merc" | "guardian" | "spectres">("you");
  const [mercPick, setMercPick] = useState<number | null>(null);
  const [spareOpen, setSpareOpen] = useState(false);
  // In the overlay (F6) the page never scrolls: the tree fills the space under the tabs and
  // lists scroll inside their own panel. This is that panel's height.
  const panel = useRef<HTMLDivElement | null>(null);
  const [panelH, setPanelH] = useState(0);
  useEffect(() => {
    const el = panel.current;
    if (!compact || !el) return;
    const ro = new ResizeObserver(() => setPanelH(el.getBoundingClientRect().height));
    ro.observe(el);
    return () => ro.disconnect();
  });
  useEffect(() => {
    // The game's passive tree key opened the overlay: show the tree.
    const t = listen("build-tree", () => setView("tree"));
    return () => { t.then((f) => f()); };
  }, []);

  // Follow the game first (moves to your act's stage if that's on), then show.
  const load = () => invoke("pob_follow_now").catch(() => null)
    .then(() => invoke<PobState | null>("pob_get")).then(setSt).catch(() => setSt(null));
  useEffect(() => {
    load();
    const every = window.setInterval(load, 30000);
    const a = listen("pob-stage", () => load());
    const b = listen("build-shown", () => load());
    const c = listen("pob-imported", () => load());
    return () => { window.clearInterval(every); a.then((f) => f()); b.then((f) => f()); c.then((f) => f()); };
  }, []);
  useEffect(() => { try { localStorage.setItem("build.view", view); } catch { /* not kept */ } }, [view]);

  if (!st) return compact
    ? <div className="empty">No build yet. Paste your Path of Building link on the My build page.<br />
        <button className="primary empty-go" onClick={() => invoke("open_editor", { tab: "build" })}>Open My build</button></div>
    : <div className="empty">No build yet. Paste your Path of Building link in the box above.</div>;
  const { build, stage } = st;
  const s = build.stages[stage];
  const setStage = (i: number) => { setSt({ ...st, stage: i, follow: false }); invoke("pob_set_stage", { stage: i }); };
  const setFollow = (on: boolean) => { setSt({ ...st, follow: on }); invoke("pob_set_follow", { on }).then(load); };
  // Gems you didn't have in the stage before: what to buy or swap in now.
  const before = new Set((build.stages[stage - 1]?.groups ?? []).flatMap((g) => g.gems.map((x) => x.name)));
  const newGems = stage > 0 ? [...new Set(s.groups.flatMap((g) => g.gems.filter((x) => x.enabled).map((x) => x.name)))].filter((n) => !before.has(n)) : [];
  const change = treeChange(build.stages, stage);
  const checks = st.checks ?? [];
  const chk = checks[stage];
  const sorted = [...s.gear].sort((a, b) => (SLOT_ORDER.indexOf(a.slot) + 99) % 99 - (SLOT_ORDER.indexOf(b.slot) + 99) % 99);
  const gear = sorted.filter((g) => !g.slot.startsWith("Flask"));
  const flasks = sorted.filter((g) => g.slot.startsWith("Flask"));
  const jewels = s.jewels ?? [];
  const jewelWhere = (j: PobJewel) => j.node != null ? "Tree socket" : j.item.slot.replace(" Abyssal Socket ", " · abyssal socket ");
  const haveBox = (it: PobGear) => (
    <input type="checkbox" className="bv-have" checked={have.has(haveKey(it))} title="I have this"
      onClick={(e) => e.stopPropagation()} onChange={() => toggleHave(haveKey(it))} />
  );
  const shown = (it: PobGear) => !(hideHave && have.has(haveKey(it)));
  // Gem groups by the item they're socketed in; the rest (no item set in PoB, or an item this stage doesn't have)
  // go under "Other gems".
  const slots = new Set(s.gear.map((g) => g.slot));
  const groupsIn = (slot: string) => s.groups.filter((g) => g.slot === slot);
  const otherGroups = s.groups.filter((g) => !slots.has(g.slot));
  const groupView = (g: PobGroup, i: number, withSlot: boolean) => (
    <div key={i} className={`bv-group ${g.enabled ? "" : "off"}`}>
      {(g.label || (withSlot && g.slot)) && <div className="bv-glabel">{g.label}{g.label && withSlot && g.slot ? " · " : ""}
        {withSlot && <span className="muted">{g.slot}</span>}</div>}
      <div className="bv-gems">{g.gems.map((x, j) => <GemPill key={j} g={x} main={!x.support && j === 0} fresh={newGems.includes(x.name)} />)}</div>
    </div>
  );
  const itemRow = (it: PobGear, key: string, where: string) => {
    const groups = groupsIn(it.slot);
    return (
      <li key={key} className={have.has(haveKey(it)) ? "got" : ""} onClick={() => setOpenItem(openItem === key ? null : key)}>
        <div className="bv-item">
          {haveBox(it)}
          <span className="bv-slot muted small">{where}</span>
          <ItemIcon name={it.rarity === "unique" ? it.name : null} base={it.base} size={26} />
          <span className="bv-iname">
            <span style={{ color: RARITY[it.rarity] ?? "#ccc" }}>{it.name}</span>
            {it.base !== it.name && <span className="muted small"> {it.base}</span>}
            {(() => {
              // Needs a higher level than this stage starts at (his level 68 swap's Torturer's Mask needs 84).
              const l = chk?.later?.find((x) => x.slot === it.slot);
              return l && <span className="bv-later" title={l.instead ? `Buy searches ${l.instead} with the same mods, wearable at level ${chk!.level}` : `Wait until level ${l.level}, or use what you have`}>
                needs level {l.level}{l.instead ? ` · wear ${l.instead} until then` : ""}</span>;
            })()}
          </span>
          <SocketStrip it={it} />
          <CraftButton it={it} />
          {it.copy && <BuyButton it={it} stage={s.title} />}
          {it.copy && <PriceButton onClick={() => priceItem(it)} />}
        </div>
        {groups.length > 0 && <div className="bv-ingems" onClick={(e) => e.stopPropagation()}>{groups.map((g, i) => groupView(g, i, false))}</div>}
        {openItem === key && <ul className="bv-lines">{it.lines.map((l, k) => <li key={k}>{l}</li>)}</ul>}
      </li>
    );
  };

  return (
    <div className={`buildview ${compact ? "compact" : ""}`}>
      <div className="bv-head">
        <span className="bv-who">{build.ascendancy || build.class} <span className="muted small">level {build.level} in PoB</span></span>
        <span className="bv-stage">
          <button className="link" disabled={stage === 0} onClick={() => setStage(stage - 1)} title="Previous stage">‹</button>
          <select value={stage} onChange={(e) => setStage(+e.target.value)}>
            {build.stages.map((x, i) => <option key={i} value={i}>{x.title}</option>)}
          </select>
          <button className="link" disabled={stage >= build.stages.length - 1} onClick={() => setStage(stage + 1)} title="Next stage">›</button>
          <label className="bv-follow" title="Pick the stage from the act you're in (from the game's log)">
            <input type="checkbox" checked={st.follow ?? true} onChange={(e) => setFollow(e.target.checked)} /> follow my game
          </label>
        </span>
      </div>
      <div className="bv-tabs">
        {(["gear", "jewels", "tree", "check", "upgrades"] as const).filter((v) => v !== "upgrades" || !compact).map((v) => (
          <button key={v} className={view === v ? "on" : ""} onClick={() => setView(v)}>{{ gear: "Gear and gems", jewels: `Jewels${jewels.length ? ` (${jewels.length})` : ""}`, tree: "Tree",
            check: <>Check{chk && <CheckBadge c={chk} />}</>, upgrades: "Upgrades" }[v]}</button>
        ))}
      </div>

      <div className="bv-panel" ref={panel}>
      {view === "gear" && (() => {
        // Mercenary stages ("Mercenary: at the swap"): the one named most like this stage, unless one was picked.
        const mercs = build.stages.map((x, i) => ({ x, i })).filter(({ x }) => /mercenar/i.test(x.title));
        const isMerc = /mercenar/i.test(s.title);
        const words = (x: string) => new Set(x.toLowerCase().split(/[^a-z0-9]+/).filter((w) => w.length > 2 && w !== "mercenary" && w !== "level"));
        const mine = words(s.title);
        const guess = isMerc ? stage : [...mercs].sort((a, b) => [...words(b.x.title)].filter((w) => mine.has(w)).length - [...words(a.x.title)].filter((w) => mine.has(w)).length)[0]?.i;
        const mi = mercPick != null && mercs.some((m) => m.i === mercPick) ? mercPick : guess;
        const merc = mi != null ? build.stages[mi] : null;
        const m = s.minions;
        const tabs = [
          { k: "you" as const, label: isMerc ? "This stage" : "You", show: true },
          { k: "merc" as const, label: "Mercenary", show: !isMerc && mercs.length > 0 },
          { k: "guardian" as const, label: "Animate Guardian", show: !!m?.guardian_gem },
          { k: "spectres" as const, label: `Spectres${m?.spectre_limit ? ` ×${m.spectre_limit}` : ""}`, show: !!m?.spectre_gem },
        ].filter((x) => x.show);
        const who = tabs.some((x) => x.k === owner) ? owner : "you";
        const spares = spareGroups((build.alternatives ?? []).filter(shown), s.gear);
        const spareN = spares.reduce((a, g) => a + g.items.length, 0);
        return (
          <div className="bv-gearv">
            {newGems.length > 0 && who === "you" && <div className="bv-newgems"><b>New gems this stage:</b> {newGems.join(" · ")}</div>}
            {tabs.length > 1 && (
              <div className="bv-owners">
                {tabs.map((x) => <button key={x.k} className={who === x.k ? "on" : ""} onClick={() => setOwner(x.k)}>{x.label}</button>)}
                <span className="muted small bv-havecount">Have {sorted.filter((it) => have.has(haveKey(it))).length} of {sorted.length}</span>
              </div>
            )}
            {who === "you" && <>
              {isMerc && s.groups.length > 0 && <ul className="bv-gear"><MercRow groups={s.groups} /></ul>}
              <GearDoll key={"you" + stage} gear={s.gear} groups={s.groups} stage={s.title} have={have} onHave={(it) => toggleHave(haveKey(it))}
                later={chk?.later} newGems={newGems} empty="No gear set for this stage." />
              {otherGroups.length > 0 && <><div className="bv-subhead">Gems not in an item</div><div className="bv-othergems">{otherGroups.map((g, i) => groupView(g, i, true))}</div></>}
            </>}
            {who === "merc" && merc && <>
              <div className="bv-ownerhead">
                {mercs.length > 1 ? <select value={mi ?? undefined} onChange={(e) => setMercPick(+e.target.value)}>
                  {mercs.map(({ x, i }) => <option key={i} value={i}>{x.title}</option>)}</select> : <b>{merc.title}</b>}
                <span className="muted small">Your Mercenary's gear and skills from the guide</span>
              </div>
              {merc.groups.length > 0 && <ul className="bv-gear"><MercRow groups={merc.groups} /></ul>}
              <GearDoll key={"merc" + mi} gear={merc.gear} groups={merc.groups} stage={merc.title} have={have} onHave={(it) => toggleHave(haveKey(it))}
                newGems={[]} empty="The guide gives no gear for the Mercenary here." />
              {merc.groups.some((g) => !merc.gear.some((it) => it.slot === g.slot)) && <>
                <div className="bv-subhead">Its skills</div>
                <div className="bv-othergems">{merc.groups.filter((g) => !merc.gear.some((it) => it.slot === g.slot)).map((g, i) => groupView(g, i, false))}</div>
              </>}
            </>}
            {who === "guardian" && m && <>
              <div className="bv-ownerhead">
                {m.guardian_gem && <GemPill g={m.guardian_gem} main={true} />}
                <span className="muted small">{m.guardian.length ? <>The Animate Guardian wears the guide's “{m.guardian_set}” set. Drop these on the ground, then use the gem on them.</> : "The guide doesn't give the Animate Guardian any gear."}</span>
              </div>
              <GearDoll key={"ag" + stage} gear={m.guardian} groups={[]} stage={s.title} have={have} onHave={(it) => toggleHave(haveKey(it))}
                newGems={[]} empty="No gear for it in this guide." />
            </>}
            {who === "spectres" && m?.spectre_gem && (
              <div className="bv-spectres">
                <div className="bv-ownerhead">
                  <GemPill g={m.spectre_gem} main={true} />
                  <span>raises <b>{m.spectre_limit ?? "?"}</b> spectre{m.spectre_limit === 1 ? "" : "s"}</span>
                  <span className="muted small" title="From the gem's level, gem levels on your gear, and +spectres from your tree and gear. Support gems aren't counted.">(about: from the gem level, gear and tree)</span>
                </div>
                {m.spectres.length === 0 && <div className="empty">The guide doesn't say which spectres to raise.</div>}
                {m.spectres.some((x) => x.main) && <div className="bv-subhead">Raise these</div>}
                {m.spectres.filter((x) => x.main).map((x) => (
                  <div key={x.id} className="bv-spectre main"><GemPic g={m.spectre_gem!} /><b>{x.name}</b>
                    <span className="muted small">the guide's pick on this stage's Raise Spectre gem</span></div>
                ))}
                {m.spectres.some((x) => !x.main) && <div className="bv-subhead" title="Path of Building's spectre list for this build: every spectre the guide set up, for all its stages">Also in the guide's spectre list</div>}
                {m.spectres.filter((x) => !x.main).map((x) => <div key={x.id} className="bv-spectre"><span>{x.name}</span></div>)}
              </div>
            )}
            {who === "you" && spareN > 0 && (
              <div className="bv-spares">
                <button className="bv-sparehead" onClick={() => setSpareOpen(!spareOpen)}>
                  {spareOpen ? "▾" : "▸"} Spare items in the guide <span className="cnt">{spareN}</span>
                  <span className="muted small">items the guide put in its Path of Building but no stage wears: options, upgrades, levelling pieces</span>
                </button>
                {spareOpen && <ul className="bv-gear">
                  {spares.map((g) => <li key={g.cls} className="bv-sparegroup">
                    <div className="bv-subhead">{g.cls}{g.instead && <span className="muted"> · {g.instead}</span>}</div>
                    <ul className="bv-gear">{g.items.map(({ it, n }, i) => itemRow(it, g.cls + i, n > 1 ? `×${n}` : ""))}</ul>
                  </li>)}
                </ul>}
              </div>
            )}
          </div>
        );
      })()}

      {view === "jewels" && (
        <ul className="bv-gear bv-jewels">
          {jewels.length === 0 && <div className="empty">No jewels in this stage.</div>}
          <HaveBar n={jewels.filter((j) => have.has(haveKey(j.item))).length} of={jewels.length} hide={hideHave} setHide={setHideHave} />
          {jewels.map((j, i) => {
            const key = "j" + (j.node ?? j.item.slot);
            if (!shown(j.item)) return null;
            return (
              <li key={i} onClick={() => setOpenItem(openItem === key ? null : key)}
                className={`${j.node != null && j.node === jewelNode ? "hl" : ""} ${have.has(haveKey(j.item)) ? "got" : ""}`}>
                <div className="bv-item">
                  {haveBox(j.item)}
                  <span className="bv-slot muted small">{jewelWhere(j)}</span>
                  <ItemIcon name={j.item.rarity === "unique" ? j.item.name : null} base={j.item.base} size={26} />
                  <span className="bv-iname">
                    <span style={{ color: RARITY[j.item.rarity] ?? "#ccc" }}>{j.item.name}</span>
                    {j.item.base !== j.item.name && <span className="muted small"> {j.item.base}</span>}
                    {j.notables.length > 0 && <span className="bv-notables">{j.notables.map((n) => <span key={n} className="bv-notable">{n}</span>)}</span>}
                  </span>
                  {j.node != null && s.tree && (
                    <button className="link small bv-onTree" title="Show this jewel's socket on the tree"
                      onClick={(e) => { e.stopPropagation(); setJewelNode(j.node); setView("tree"); }}>on tree</button>
                  )}
                  {j.item.copy && j.item.base.endsWith("Cluster Jewel") && <BuyBaseButton it={j.item} />}
                  <CraftButton it={j.item} />
                  {j.item.copy && <BuyButton it={j.item} stage={s.title} />}
                  {j.item.copy && <PriceButton onClick={() => priceItem(j.item)} />}
                </div>
                {openItem === key && <ul className="bv-lines">{j.item.lines.map((l, k) => <li key={k}>{l}</li>)}</ul>}
              </li>
            );
          })}
        </ul>
      )}

      {view === "upgrades" && !compact && <UpgradeFinder stage={stage} />}

      {view === "check" && (
        <div className="bv-check">
          {!chk ? <div className="empty">Nothing to check in this stage.</div> : <>
            {(chk.str || chk.dex || chk.int || chk.points > 0) ? <div className="bv-check-sum small">
              Level <b>{chk.level}</b>{chk.level_guessed && <span className="muted"> (guessed from the tree)</span>}
              {" · "}{chk.str} Str · {chk.dex} Dex · {chk.int} Int
              {chk.points > 0 && <> · {chk.points} passive points</>}
            </div> : null}
            {chk.speed && (
              <div className={`bv-check-line ${chk.speed.spare_pct < 0 ? "bad" : "ok"}`}>
                <b>{chk.speed.skill}</b> fires after <b>{chk.speed.fires_after.toFixed(2)} s</b>, you attack every <b>{chk.speed.attack_every.toFixed(2)} s</b>.{" "}
                {chk.speed.spare_pct >= 0 ? `Room for about ${chk.speed.spare_pct}% more attack speed.` : "You attack too fast for it."}
              </div>
            )}
            {chk.mana && (
              <div className={`bv-check-line ${chk.mana.free < 0 ? "bad" : chk.mana.cost && chk.mana.free < chk.mana.cost * 2 ? "warn" : "ok"}`}>
                Mana <b>{chk.mana.total}</b>, auras reserve <b>{chk.mana.reserved}</b>, <b>{chk.mana.free} free</b>
                {chk.mana.cost ? <>. {chk.mana.skill} costs <b>{chk.mana.cost}</b> a use.</> : "."}
              </div>
            )}
            <ul className="bv-check-notes">
              {chk.notes.map((n, i) => <li key={i} className={n.kind}><span className="bv-check-icon">{n.kind === "bad" ? "✕" : n.kind === "warn" ? "!" : "✓"}</span>{n.text}</li>)}
            </ul>
            {checks.length > 1 && (
              <div className="bv-check-all">
                <span className="muted small">All stages:</span>
                {checks.map((c, i) => (
                  <button key={i} className={`bv-check-stage ${i === stage ? "on" : ""}`} onClick={() => setStage(i)}>{c.title} <CheckBadge c={c} /></button>
                ))}
              </div>
            )}
            <div className="muted small">Checked against Path of Building's game data: item and gem levels, Str / Dex / Int, sockets, passive points, mana after auras{chk.speed ? " and the duration breakpoint" : ""}. It doesn't work out damage.</div>
          </>}
        </div>
      )}

      {view === "tree" && (
        <div className="bv-tree">
          {s.tree ? (
            <>
              <div className="bv-tree-sum small"><b>{chk?.points || change.total}</b> points
                {chk?.points && change.total > chk.points ? <span className="muted"> (+{change.total - chk.points} ascendancy)</span> : null}
                {stage > 0 && <> · <b className="c-new">{change.added}</b> new{change.removed ? <>, <b className="c-gone">{change.removed}</b> to take back</> : ""} since “{build.stages[stage - 1].title}”</>}
                <button className="link small" onClick={() => invoke("open_url", { url: s.tree!.url })}>Open on pathofexile.com</button>
              </div>
              <TreeView nodes={s.tree.nodes} previous={stage > 0 ? build.stages[stage - 1].tree?.nodes : undefined} height={compact ? Math.max(240, (panelH || 600) - 100) : 560} ascendancy={build.ascendancy || undefined}
                masteries={(s.tree as { masteries?: [number, number][] }).masteries ?? []}
                prevMasteries={stage > 0 ? (build.stages[stage - 1].tree as { masteries?: [number, number][] } | null)?.masteries : undefined}
                jewels={jewels.filter((j) => j.node != null).map((j) => ({ node: j.node!, name: j.item.name, base: j.item.base, colour: RARITY[j.item.rarity] ?? "#ccc" }))}
                highlight={jewelNode} onJewelClick={(n) => { setJewelNode(n); setOpenItem("j" + n); setView("jewels"); }} />
              {jewels.some((j) => j.node != null) && (
                <div className="bv-jewelchips">
                  <span className="muted small">Jewels:</span>
                  {jewels.filter((j) => j.node != null).map((j) => (
                    <button key={j.node} className={`bv-jchip ${jewelNode === j.node ? "on" : ""}`} title={`${j.item.name} · ${j.item.base}`}
                      onMouseEnter={() => setJewelNode(j.node)} onClick={() => setJewelNode(jewelNode === j.node ? null : j.node)}>
                      <span style={{ color: RARITY[j.item.rarity] ?? "#ccc" }}>{j.item.name}</span>
                    </button>
                  ))}
                </div>
              )}
            </>
          ) : <div className="empty">No tree saved for this stage.</div>}
        </div>
      )}
      </div>
    </div>
  );
}
