import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ItemIcon } from "./icons";
import { friendly } from "./errors";
import { CustomStyle, LookButton, Preview } from "./StyleEditor";
import "./bases.css";

export type DustTier = { min: number; style: CustomStyle };
export type DustSettings = { enabled: boolean; tiers: DustTier[] };
type Pick = { base: string; low: number; high: number; uniques: number; tier: number; ilvl: number; neversink: boolean };
type Picks = { total: number; list: Pick[]; updated: string };

/** "52k" / "1.2m" dust. */
export const dustText = (n: number) => n >= 1e6 ? `${+(n / 1e6).toFixed(1)}m` : n >= 1000 ? `${Math.round(n / 1000)}k` : `${n}`;

/** Prices page: uniques by how much Thaumaturgic Dust they give. Saved with the page (its Save button). */
export default function DustCard({ profileId, dust, onChange, onLook }: {
  profileId: string; dust: DustSettings; onChange: (d: DustSettings) => void; onLook: (tier: number) => void;
}) {
  const [p, setP] = useState<Picks | null>(null);
  const [err, setErr] = useState("");
  const [all, setAll] = useState(false);
  const timer = useRef(0);
  const key = JSON.stringify(dust.tiers.map((t) => t.min));
  useEffect(() => {
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => invoke<Picks>("dust_picks", { id: profileId, tiers: dust.tiers })
      .then((x) => { setP(x); setErr(""); }).catch((e) => setErr(friendly(e))), 400);
  }, [profileId, key]); // eslint-disable-line react-hooks/exhaustive-deps
  const set = (fn: (d: DustSettings) => void) => { const c: DustSettings = JSON.parse(JSON.stringify(dust)); fn(c); onChange(c); };
  const order = dust.tiers.map((t, ti) => ({ t, ti })).sort((a, b) => b.t.min - a.t.min);
  const mine = (p?.list ?? []).filter((x) => !x.neversink);
  const theirs = (p?.list ?? []).filter((x) => x.neversink);
  const shown = all ? mine : mine.slice(0, 16);
  return (
    <div className="card">
      <label className="check">
        <input type="checkbox" checked={dust.enabled} onChange={(e) => set((d) => { d.enabled = e.target.checked; })} />
        <span className="group-title">Uniques that give lots of dust</span>
      </label>
      <p className="muted small">Faustus in Kingsmarch turns uniques into Thaumaturgic Dust. How much depends on the unique and its item
        level: 84 or higher gives 20 times what 65 gives. Quality, influence and corrupted implicits add more on top (not counted here).
        A filter can only see the base, so a base counts at its lowest-dust unique. Works with price tiers off; a price tier still wins.</p>
      <ul className="tiers">
        {order.map(({ t, ti }, rank) => (
          <li key={ti}>
            <span className="tier-name">Tier {rank + 1}</span>
            <span className="muted small">gives</span>
            <input type="number" min={1000} step={1000} value={t.min}
              onChange={(e) => set((d) => { d.tiers[ti].min = Math.max(0, Math.round(+e.target.value || 0)); })} />
            <span className="muted small">dust or more</span>
            <span className="tier-preview"><LookButton onClick={() => onLook(ti)}><Preview s={t.style} text="Example" /></LookButton></span>
            <button className="link" onClick={() => onLook(ti)}>Change look</button>
            <button className="link" title="Remove this tier" onClick={() => set((d) => { d.tiers.splice(ti, 1); })}>✕</button>
          </li>
        ))}
      </ul>
      <button disabled={dust.tiers.length === 0} onClick={() => set((d) => {
        const low = Math.min(...d.tiers.map((t) => t.min));
        d.tiers.push({ min: Math.max(1000, Math.round(low / 2000) * 1000), style: JSON.parse(JSON.stringify(order.at(-1)!.t.style)) });
      })}>Add a tier</button>
      {p && <div className="muted small" style={{ marginTop: 8 }}>
        {p.total === 0 ? "No base gives that much." : `${p.total} bases ${dust.enabled ? "stand out" : "would stand out"} at item level 84`}
        {theirs.length > 0 && ` (and ${theirs.length} NeverSink already shows loudly, left to him)`}. Dust list from {p.updated}.
      </div>}
      <div className="bs-list">
        {shown.map((x) => (
          <div key={x.base} className="bs-item" title={`${x.uniques} unique${x.uniques === 1 ? "" : "s"} on this base: ${dustText(x.low)}${x.high > x.low ? ` to ${dustText(x.high)}` : ""} dust at item level 84`}>
            <ItemIcon name={x.base} base={x.base} size={34} />
            <div><div className="bs-name">{x.base}</div>
              <div className="muted small">Tier {x.tier + 1}{x.ilvl > 64 ? ` from item level ${x.ilvl}` : ", any item level"}</div></div>
            <span className="bs-money">{dustText(x.low)}{x.high > x.low ? "+" : ""}</span>
          </div>
        ))}
      </div>
      {mine.length > 16 && <button className="link small" onClick={() => setAll(!all)}>{all ? "Show fewer" : `Show ${mine.length - 16} more`}</button>}
      {err && <div className="qm-warn small">{err}</div>}
    </div>
  );
}
