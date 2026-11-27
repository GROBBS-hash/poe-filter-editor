import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { ItemIcon, useIcons } from "./icons";
import TreeView from "./TreeView";
import { CRAFT_ITEM_KEY, CRAFT_PLAN_MARK } from "./Craft";

export type PobGem = { name: string; level: number; quality: number; enabled: boolean; support: boolean; colour: string };
export type PobGroup = { label: string; slot: string; enabled: boolean; gems: PobGem[] };
export type PobGear = { slot: string; rarity: string; name: string; base: string; lines: string[]; copy?: string };
/** A jewel in a tree socket (node) or in an item's abyssal socket (node null, slot "Belt Abyssal Socket 1"). */
export type PobJewel = { node: number | null; item: PobGear; notables: string[] };
export type PobStage = { title: string; tree: { version: string; nodes: number[]; url: string } | null; groups: PobGroup[]; gear: PobGear[]; jewels?: PobJewel[] };
/** alternatives: items in the PoB that no stage wears (the guide's options and upgrades). */
export type PobBuild = { class: string; ascendancy: string; level: number; stages: PobStage[]; active: number; alternatives?: PobGear[] };
export type PobState = { build: PobBuild; stage: number; link: string; follow?: boolean };

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

type BuyResult = { total: number; of: number; need: number };

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

function BuyButton({ it }: { it: PobGear }) {
  const [state, setState] = useState<"idle" | "busy" | BuyResult | string>("idle");
  const go = (e: { stopPropagation(): void }) => {
    e.stopPropagation();
    setState("busy");
    invoke<BuyResult | null>("buy_build_item", { text: it.copy }).then((r) => setState(r ?? "No answer from the trade site")).catch((err) => setState(String(err)));
  };
  const note = typeof state === "object" && state
    ? `${state.total} for sale${state.need < state.of ? ` · any ${state.need} of ${state.of} mods` : ""}`
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

export default function BuildView({ compact = false }: { compact?: boolean }) {
  const [have, toggleHave] = useHave();
  const [hideHave, setHideHave] = useState(() => { try { return localStorage.getItem("build.hideHave") === "1"; } catch { return false; } });
  useEffect(() => { try { localStorage.setItem("build.hideHave", hideHave ? "1" : "0"); } catch { /* not kept */ } }, [hideHave]);
  const [st, setSt] = useState<PobState | null>(null);
  // A jewel picked in the list (shown on the tree) or on the tree (opened in the list).
  const [jewelNode, setJewelNode] = useState<number | null>(null);
  // Gems live under their item on the Gear tab now (21 Nov): an old "gems" choice opens Gear.
  const [view, setView] = useState<"gear" | "jewels" | "tree">(() => {
    try {
      const v = localStorage.getItem("build.view");
      return v === "jewels" || v === "tree" ? v : "gear";
    } catch { return "gear"; }
  });
  const [openItem, setOpenItem] = useState<string | null>(null);
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
          </span>
          <SocketStrip it={it} />
          <CraftButton it={it} />
          {it.copy && <BuyButton it={it} />}
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
        {(["gear", "jewels", "tree"] as const).map((v) => (
          <button key={v} className={view === v ? "on" : ""} onClick={() => setView(v)}>{{ gear: "Gear and gems", jewels: `Jewels${jewels.length ? ` (${jewels.length})` : ""}`, tree: "Tree" }[v]}</button>
        ))}
      </div>

      <div className="bv-panel" ref={panel}>
      {view === "gear" && (
        <ul className="bv-gear">
          {newGems.length > 0 && <li className="bv-newgems"><b>New gems this stage:</b> {newGems.join(" · ")}</li>}
          {gear.length === 0 && <div className="empty">No gear set for this stage.</div>}
          <HaveBar n={sorted.filter((it) => have.has(haveKey(it))).length} of={sorted.length} hide={hideHave} setHide={setHideHave} />
          {gear.filter(shown).map((it) => itemRow(it, it.slot, SLOT_NAME[it.slot] ?? it.slot))}
          {flasks.filter(shown).length > 0 && <li className="bv-subhead">Flasks</li>}
          {flasks.filter(shown).map((it) => itemRow(it, it.slot, it.slot))}
          {otherGroups.length > 0 && <li className="bv-subhead">Other gems</li>}
          {otherGroups.length > 0 && <li className="bv-othergems">{otherGroups.map((g, i) => groupView(g, i, true))}</li>}
          {(build.alternatives ?? []).filter(shown).length > 0 && <li className="bv-subhead" title="Items in the Path of Building that no stage wears: the guide's options and upgrades">Alternatives in this build</li>}
          {(build.alternatives ?? []).filter(shown).map((it, i) => itemRow(it, "alt" + i, "Alternative"))}
        </ul>
      )}

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
                  {j.item.copy && <BuyButton it={j.item} />}
                  {j.item.copy && <PriceButton onClick={() => priceItem(j.item)} />}
                </div>
                {openItem === key && <ul className="bv-lines">{j.item.lines.map((l, k) => <li key={k}>{l}</li>)}</ul>}
              </li>
            );
          })}
        </ul>
      )}

      {view === "tree" && (
        <div className="bv-tree">
          {s.tree ? (
            <>
              <div className="bv-tree-sum small"><b>{change.total}</b> points
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
