import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Path of Building's numbers in plain words, most important first; resistances in %. */
const POB_SHOW: [string, string, boolean][] = [["CombinedDPS", "DPS", false], ["Life", "Life", false], ["EnergyShield", "ES", false], ["TotalEHP", "Effective HP", false],
  ["FireResist", "Fire res", true], ["ColdResist", "Cold res", true], ["LightningResist", "Lightning res", true], ["ChaosResist", "Chaos res", true], ["Mana", "Mana", false],
  ["Armour", "Armour", false], ["Evasion", "Evasion", false], ["Str", "Str", false], ["Dex", "Dex", false], ["Int", "Int", false]];

/** One "PoB" line: what an item changes on the build (green better, red worse). Upgrades results and Ctrl+D. */
export function PobLine({ d, label = "PoB" }: { d: Record<string, number>; label?: string }) {
  const parts = POB_SHOW.map(([k, name, pct]) => ({ k, name, pct, v: Math.round(d[k] ?? 0) })).filter((x) => x.v !== 0).slice(0, 6);
  return (
    <div className="uf-pob small" title="Path of Building's own numbers for this item on the build's stage, against what the stage wears now">
      <span className="uf-pob-tag">{label}</span>
      {parts.length === 0 ? <span className="muted">no change to life, damage or resistances</span>
        : parts.map((x) => <span key={x.k} className={x.v > 0 ? "up" : "down"}>{x.name} {x.v > 0 ? "+" : "−"}{Math.abs(x.v).toLocaleString()}{x.pct ? "%" : ""}</span>)}
    </div>
  );
}

type Row = { slot: string; now: string; change: Record<string, number> | null; error: string };
type Check = { build: string; stage: string; rows: Row[]; note: string };

/** Ctrl+D: what Path of Building says this item would do to the build shown on My build (28 Nov 2026). Nothing
 *  shows without a build, for items that don't go in a slot, or when PoB isn't installed and nothing was asked. */
export function PobCheck({ raw, itemClass }: { raw: string; itemClass: string }) {
  const [c, setC] = useState<Check | null>(null);
  const [busy, setBusy] = useState(true);
  useEffect(() => {
    let on = true;
    setC(null); setBusy(true);
    invoke<Check>("pob_check", { text: raw, itemClass }).then((x) => on && setC(x)).catch(() => on && setC(null)).finally(() => on && setBusy(false));
    return () => { on = false; };
  }, [raw, itemClass]);
  if (busy) return <div className="pob-check muted small">Asking Path of Building what it does to your build…</div>;
  if (!c || (!c.rows.length && !c.note)) return null;
  return (
    <div className="pob-check">
      <div className="pob-check-head small"><b>Path of Building</b> <span className="muted">· {c.build}, “{c.stage}”</span></div>
      {c.note && <div className="muted small">No numbers: {c.note}</div>}
      {c.rows.map((r) => (
        <div key={r.slot} className="pob-check-row">
          <span className="muted small">{r.slot}{r.now ? ` instead of ${r.now}` : " (empty now)"}</span>
          {r.change ? <PobLine d={r.change} /> : <div className="muted small">{r.error || "Path of Building couldn't read it"}</div>}
        </div>
      ))}
    </div>
  );
}
