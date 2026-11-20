import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { ItemIcon } from "./icons";
import TreeView from "./TreeView";

export type PobGem = { name: string; level: number; quality: number; enabled: boolean; support: boolean; colour: string };
export type PobGroup = { label: string; slot: string; enabled: boolean; gems: PobGem[] };
export type PobGear = { slot: string; rarity: string; name: string; base: string; lines: string[] };
/** A jewel in a tree socket (node) or in an item's abyssal socket (node null, slot "Belt Abyssal Socket 1"). */
export type PobJewel = { node: number | null; item: PobGear; notables: string[] };
export type PobStage = { title: string; tree: { version: string; nodes: number[]; url: string } | null; groups: PobGroup[]; gear: PobGear[]; jewels?: PobJewel[] };
export type PobBuild = { class: string; ascendancy: string; level: number; stages: PobStage[]; active: number };
export type PobState = { build: PobBuild; stage: number; link: string; follow?: boolean };

const RARITY: Record<string, string> = { normal: "#c8c8c8", magic: "#8888ff", rare: "#ffff77", unique: "#af6025", relic: "#82ad6a" };
const SLOT_ORDER = ["Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Amulet", "Ring 1", "Ring 2", "Belt",
  "Flask 1", "Flask 2", "Flask 3", "Flask 4", "Flask 5"];
const SLOT_NAME: Record<string, string> = { "Weapon 1": "Weapon", "Weapon 2": "Off hand", "Body Armour": "Body" };

/** PoB's look: socket colours and supports a little smaller. */
function GemPill({ g, main, fresh }: { g: PobGem; main: boolean; fresh?: boolean }) {
  return (
    <span className={`gem gem-${g.colour} ${g.support ? "sup" : ""} ${main ? "main" : ""} ${g.enabled ? "" : "off"} ${fresh ? "fresh" : ""}`}
      title={`${g.name} · level ${g.level}${g.quality ? ` · ${g.quality}% quality` : ""}${g.enabled ? "" : " (switched off in PoB)"}`}>
      <span className="gem-dot" />{g.name}
      {(g.level > 1 || g.quality > 0) && <span className="gem-lvl">{g.level}{g.quality ? `/${g.quality}` : ""}</span>}
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

export default function BuildView({ compact = false }: { compact?: boolean }) {
  const [st, setSt] = useState<PobState | null>(null);
  // A jewel picked in the list (shown on the tree) or on the tree (opened in the list).
  const [jewelNode, setJewelNode] = useState<number | null>(null);
  const [view, setView] = useState<"gems" | "gear" | "jewels" | "tree">(() => {
    try { return (localStorage.getItem("build.view") as "gems" | "gear" | "tree") || "gems"; } catch { return "gems"; }
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

  if (!st) return <div className="empty">No build yet. Paste your Path of Building link in the app (Levelling → My build).</div>;
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
  const itemRow = (it: PobGear, key: string, where: string) => (
    <li key={key} onClick={() => setOpenItem(openItem === key ? null : key)}>
      <div className="bv-item">
        <span className="bv-slot muted small">{where}</span>
        <ItemIcon name={it.rarity === "unique" ? it.name : null} base={it.base} size={26} />
        <span className="bv-iname">
          <span style={{ color: RARITY[it.rarity] ?? "#ccc" }}>{it.name}</span>
          {it.base !== it.name && <span className="muted small"> {it.base}</span>}
        </span>
      </div>
      {openItem === key && <ul className="bv-lines">{it.lines.map((l, k) => <li key={k}>{l}</li>)}</ul>}
    </li>
  );

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
        {(["gems", "gear", "jewels", "tree"] as const).map((v) => (
          <button key={v} className={view === v ? "on" : ""} onClick={() => setView(v)}>{{ gems: "Gems", gear: "Gear", jewels: `Jewels${jewels.length ? ` (${jewels.length})` : ""}`, tree: "Tree" }[v]}</button>
        ))}
      </div>

      <div className="bv-panel" ref={panel}>
      {view === "gems" && (
        <div className="bv-groups">
          {s.groups.length === 0 && <div className="empty">No gems in this stage.</div>}
          {newGems.length > 0 && (
            <div className="bv-newgems"><b>New this stage:</b> {newGems.join(" · ")}</div>
          )}
          {s.groups.map((g, i) => (
            <div key={i} className={`bv-group ${g.enabled ? "" : "off"}`}>
              {(g.label || g.slot) && <div className="bv-glabel">{g.label}{g.label && g.slot ? " · " : ""}<span className="muted">{g.slot}</span></div>}
              <div className="bv-gems">{g.gems.map((x, j) => <GemPill key={j} g={x} main={!x.support && j === 0} fresh={newGems.includes(x.name)} />)}</div>
            </div>
          ))}
        </div>
      )}

      {view === "gear" && (
        <ul className="bv-gear">
          {gear.length === 0 && <div className="empty">No gear set for this stage.</div>}
          {gear.map((it) => itemRow(it, it.slot, SLOT_NAME[it.slot] ?? it.slot))}
          {flasks.length > 0 && <li className="bv-subhead">Flasks</li>}
          {flasks.map((it) => itemRow(it, it.slot, it.slot))}
        </ul>
      )}

      {view === "jewels" && (
        <ul className="bv-gear bv-jewels">
          {jewels.length === 0 && <div className="empty">No jewels in this stage.</div>}
          {jewels.map((j, i) => {
            const key = "j" + (j.node ?? j.item.slot);
            return (
              <li key={i} onClick={() => setOpenItem(openItem === key ? null : key)} className={j.node != null && j.node === jewelNode ? "hl" : ""}>
                <div className="bv-item">
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
