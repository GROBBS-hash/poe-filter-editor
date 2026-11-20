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
  const [view, setView] = useState<"gems" | "gear" | "tree">(() => {
    try { return (localStorage.getItem("build.view") as "gems" | "gear" | "tree") || "gems"; } catch { return "gems"; }
  });
  const [openItem, setOpenItem] = useState<string | null>(null);
  // In the overlay the tree fills the window (it can be resized).
  const [winH, setWinH] = useState(() => window.innerHeight);
  useEffect(() => {
    const on = () => setWinH(window.innerHeight);
    window.addEventListener("resize", on);
    // The game's passive tree key opened the overlay: show the tree.
    const t = listen("build-tree", () => setView("tree"));
    return () => { window.removeEventListener("resize", on); t.then((f) => f()); };
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
          <label className="bv-follow" title="Pick the stage from the act you're in (from the game's log)">
            <input type="checkbox" checked={st.follow ?? true} onChange={(e) => setFollow(e.target.checked)} /> follow my game
          </label>
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
              <TreeView nodes={s.tree.nodes} previous={stage > 0 ? build.stages[stage - 1].tree?.nodes : undefined} height={compact ? Math.max(420, winH - 340) : 560} ascendancy={build.ascendancy || undefined}
                masteries={(s.tree as { masteries?: [number, number][] }).masteries ?? []}
                prevMasteries={stage > 0 ? (build.stages[stage - 1].tree as { masteries?: [number, number][] } | null)?.masteries : undefined} />
            </>
          ) : <div className="empty">No tree saved for this stage.</div>}
        </div>
      )}
    </div>
  );
}
