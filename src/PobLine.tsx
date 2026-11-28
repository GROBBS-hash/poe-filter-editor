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

const ALWAYS = "pob.ctrl-d.always";
/** Item classes that go in a gear slot (core compare.rs `slots_for`): only those get the button. */
const GEAR = new Set(["Helmets", "Body Armours", "Gloves", "Boots", "Belts", "Amulets", "Rings", "Shields", "Quivers", "Wands", "Sceptres", "Staves", "Warstaves",
  "Daggers", "Rune Daggers", "Claws", "Bows", "One Hand Swords", "Thrusting One Hand Swords", "Two Hand Swords", "One Hand Axes", "Two Hand Axes", "One Hand Maces", "Two Hand Maces"]);
const always = () => { try { return localStorage.getItem(ALWAYS) === "1"; } catch { return false; } };

/** Ctrl+D: what Path of Building says this item would do to the build shown on My build (28 Nov 2026). An option
 *  (he: "This should be a OPTION not mandatory on the price checker"): a button, or every time with "Always" ticked.
 *  Nothing shows without a build, for items that don't go in a slot, or when PoB isn't installed and nothing was asked. */
export function PobCheck({ raw, itemClass }: { raw: string; itemClass: string }) {
  const [c, setC] = useState<Check | null>(null);
  const [busy, setBusy] = useState(false);
  const [auto, setAuto] = useState(always);
  const [asked, setAsked] = useState(false);
  const ask = () => {
    setC(null); setBusy(true); setAsked(true);
    invoke<Check>("pob_check", { text: raw, itemClass }).then(setC).catch(() => setC(null)).finally(() => setBusy(false));
  };
  const gear = GEAR.has(itemClass);
  useEffect(() => { setC(null); setAsked(false); if (gear && always()) ask(); }, [raw, itemClass]); // eslint-disable-line react-hooks/exhaustive-deps
  const setAlways = (on: boolean) => { setAuto(on); try { localStorage.setItem(ALWAYS, on ? "1" : "0"); } catch { /* not kept */ } if (on && !asked) ask(); };
  if (!gear) return null;
  if (!asked) return (
    <div className="pob-ask row small">
      <button className="link small" onClick={ask} title="Path of Building works out what this item does to the build shown on My build (life, ES, damage, resistances)">Ask Path of Building</button>
      <label className="check" style={{ margin: 0 }}><input type="checkbox" checked={auto} onChange={(e) => setAlways(e.target.checked)} /><span className="muted">Always</span></label>
    </div>
  );
  if (busy) return <div className="pob-check muted small">Asking Path of Building what it does to your build…</div>;
  if (!c || (!c.rows.length && !c.note)) return <div className="muted small pob-ask">Path of Building has nothing to say here (no build on My build, or not a gear piece).</div>;
  return (
    <div className="pob-check">
      <div className="pob-check-head small row" style={{ margin: 0 }}><b>Path of Building</b> <span className="muted">· {c.build}, “{c.stage}”</span>
        <label className="check" style={{ margin: "0 0 0 auto" }}><input type="checkbox" checked={auto} onChange={(e) => setAlways(e.target.checked)} /><span className="muted">Always</span></label></div>
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
