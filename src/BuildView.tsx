import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { ItemIcon } from "./icons";
import TreeView from "./TreeView";

export type PobGem = { name: string; level: number; quality: number; enabled: boolean; support: boolean; colour: string };
export type PobGroup = { label: string; slot: string; enabled: boolean; gems: PobGem[] };
export type PobGear = { slot: string; rarity: string; name: string; base: string; lines: string[] };
export type PobStage = { title: string; tree: { version: string; nodes: number[]; url: string } | null; groups: PobGroup[]; gear: PobGear[] };
export type PobBuild = { class: string; ascendancy: string; level: number; stages: PobStage[]; active: number };
export type PobState = { build: PobBuild; stage: number; link: string };

const RARITY: Record<string, string> = { normal: "#c8c8c8", magic: "#8888ff", rare: "#ffff77", unique: "#af6025", relic: "#82ad6a" };
const SLOT_ORDER = ["Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Amulet", "Ring 1", "Ring 2", "Belt",
  "Flask 1", "Flask 2", "Flask 3", "Flask 4", "Flask 5"];
const SLOT_NAME: Record<string, string> = { "Weapon 1": "Weapon", "Weapon 2": "Off hand", "Body Armour": "Body" };

/** PoB's look: socket colours and supports a little smaller. */
function GemPill({ g, main }: { g: PobGem; main: boolean }) {
  return (
    <span className={`gem gem-${g.colour} ${g.support ? "sup" : ""} ${main ? "main" : ""} ${g.enabled ? "" : "off"}`}
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
  const [view, setView] = useState<"gems" | "gear" | "tree">(() => {
    try { return (localStorage.getItem("build.view") as "gems" | "gear" | "tree") || "gems"; } catch { return "gems"; }
  });
  const [openItem, setOpenItem] = useState<string | null>(null);

  const load = () => invoke<PobState | null>("pob_get").then(setSt).catch(() => setSt(null));
  useEffect(() => {
    load();
    const a = listen("pob-stage", () => load());
    const b = listen("build-shown", () => load());
    const c = listen("pob-imported", () => load());
    return () => { a.then((f) => f()); b.then((f) => f()); c.then((f) => f()); };
  }, []);
  useEffect(() => { try { localStorage.setItem("build.view", view); } catch { /* not kept */ } }, [view]);

  if (!st) return <div className="empty">No build yet. Paste your Path of Building link in the app (Levelling → My build).</div>;
  const { build, stage } = st;
  const s = build.stages[stage];
  const setStage = (i: number) => { setSt({ ...st, stage: i }); invoke("pob_set_stage", { stage: i }); };
  const change = treeChange(build.stages, stage);
  const gear = [...s.gear].sort((a, b) => (SLOT_ORDER.indexOf(a.slot) + 99) % 99 - (SLOT_ORDER.indexOf(b.slot) + 99) % 99);

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
        </span>
      </div>
      <div className="bv-tabs">
        {(["gems", "gear", "tree"] as const).map((v) => (
          <button key={v} className={view === v ? "on" : ""} onClick={() => setView(v)}>{{ gems: "Gems", gear: "Gear", tree: "Tree" }[v]}</button>
        ))}
      </div>

      {view === "gems" && (
        <div className="bv-groups">
          {s.groups.length === 0 && <div className="empty">No gems in this stage.</div>}
          {s.groups.map((g, i) => (
            <div key={i} className={`bv-group ${g.enabled ? "" : "off"}`}>
              {(g.label || g.slot) && <div className="bv-glabel">{g.label}{g.label && g.slot ? " · " : ""}<span className="muted">{g.slot}</span></div>}
              <div className="bv-gems">{g.gems.map((x, j) => <GemPill key={j} g={x} main={!x.support && j === 0} />)}</div>
            </div>
          ))}
        </div>
      )}

      {view === "gear" && (
        <ul className="bv-gear">
          {gear.length === 0 && <div className="empty">No gear set for this stage.</div>}
          {gear.map((it) => (
            <li key={it.slot} onClick={() => setOpenItem(openItem === it.slot ? null : it.slot)}>
              <div className="bv-item">
                <span className="bv-slot muted small">{SLOT_NAME[it.slot] ?? it.slot}</span>
                <ItemIcon name={it.rarity === "unique" ? it.name : null} base={it.base} size={26} />
                <span className="bv-iname">
                  <span style={{ color: RARITY[it.rarity] ?? "#ccc" }}>{it.name}</span>
                  {it.base !== it.name && <span className="muted small"> {it.base}</span>}
                </span>
              </div>
              {openItem === it.slot && <ul className="bv-lines">{it.lines.map((l, k) => <li key={k}>{l}</li>)}</ul>}
            </li>
          ))}
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
              <TreeView nodes={s.tree.nodes} previous={stage > 0 ? build.stages[stage - 1].tree?.nodes : undefined} height={compact ? 470 : 560} ascendancy={build.ascendancy || undefined}
                masteries={(s.tree as { masteries?: [number, number][] }).masteries ?? []}
                prevMasteries={stage > 0 ? (build.stages[stage - 1].tree as { masteries?: [number, number][] } | null)?.masteries : undefined} />
            </>
          ) : <div className="empty">No tree saved for this stage.</div>}
        </div>
      )}
    </div>
  );
}
