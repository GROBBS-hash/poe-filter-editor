import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CurrencyIcon, ItemIcon } from "./icons";
import { friendly } from "./errors";
import type { Item } from "./types";
import "./bases.css";

type Worth = { influence: string; ilvl: number; value: number; listings: number; if_level: boolean };
type BaseView = { worth: Worth; divine: number | null; loud: number | null };
type Pick = { base: string; influence: string; ilvl: number; value: number; listings: number };
type Picks = { min: number | null; influenced: boolean; total: number; list: Pick[]; divine: number | null; known: boolean };

/** "43 div" or "180c" with the currency's picture. */
export function Money({ chaos, divine }: { chaos: number; divine: number | null }) {
  return divine && chaos >= divine
    ? <span className="bs-money">{+(chaos / divine).toFixed(chaos / divine >= 10 ? 0 : 1)}<CurrencyIcon id="divine" /></span>
    : <span className="bs-money">{chaos >= 10 ? Math.round(chaos) : +chaos.toFixed(1)}<CurrencyIcon id="chaos" /></span>;
}

/** F2 and Ctrl+D: what this lists at as a base to craft on. `offer` adds "Make bases like this stand out". */
export function BaseWorth({ item, offer, onDone }: { item: Item; offer?: boolean; onDone?: (msg: string) => void }) {
  const [v, setV] = useState<BaseView | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let on = true;
    invoke<BaseView | null>("base_worth", { item }).then((x) => on && setV(x ?? null)).catch(() => on && setV(null));
    return () => { on = false; };
  }, [item]);
  if (!v) return null;
  const { worth: w, divine } = v;
  const big = divine != null && w.value >= divine;
  const loud = () => {
    const min = Math.round(divine ?? 100);
    setBusy(true);
    invoke("bases_save", { id: null, min }).then(() => { setV({ ...v, loud: min }); onDone?.("Bases listed at a divine or more now stand out."); })
      .catch((e) => onDone?.(friendly(e))).finally(() => setBusy(false));
  };
  return (
    <div className={`bs-worth ${big ? "big" : ""}`} title="What sellers ask for this base with nothing useful on it (poe.ninja). Asking prices can be higher than what sells.">
      <ItemIcon name={item.base_type} base={item.base_type} size={28} />
      <div className="bs-text">
        <div>{big ? <b>Worth picking up as a base: </b> : "As a base to craft on: "}lists at about <Money chaos={w.value} divine={divine} /></div>
        <div className="muted small">
          {w.if_level ? `if it's item level ${w.ilvl} or more (not known from the ground)` : `item level ${w.ilvl}${w.ilvl === 86 ? "+" : ""}`}
          {w.influence && `, ${w.influence}`} · {w.listings.toLocaleString()} for sale
        </div>
        {offer && big && v.loud == null && <button className="link small" disabled={busy} onClick={loud}>Make bases over a divine stand out in my filter</button>}
      </div>
    </div>
  );
}

/** Prices page: make dear bases stand out. Saves by itself; works with price tiers off. */
export default function BasesCard({ profileId, onSaved }: { profileId: string; onSaved: () => void }) {
  const [p, setP] = useState<Picks | null>(null);
  const [min, setMin] = useState(100);
  const [err, setErr] = useState("");
  const [all, setAll] = useState(false);
  const timer = useRef(0);
  const load = (m?: number, inf?: boolean) => invoke<Picks>("bases_get", { id: profileId, min: m ?? null, influenced: inf ?? null }).then((x) => {
    setP(x);
    if (m == null) setMin(Math.round(x.min ?? x.divine ?? 100));
  }).catch((e) => setErr(friendly(e)));
  useEffect(() => { load(); }, [profileId]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!p || !p.known) return null;
  const save = (on: boolean, m: number, inf: boolean) => {
    setErr("");
    invoke("bases_save", { id: profileId, min: on ? m : null, influenced: inf }).then(() => { load(m, inf); onSaved(); }).catch((e) => setErr(friendly(e)));
  };
  const on = p.min != null;
  const type = (m: number) => {
    setMin(m);
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => { if (on) save(true, m, p.influenced); else load(m, p.influenced); }, 600);
  };
  const shown = all ? p.list : p.list.slice(0, 16);
  return (
    <div className="card">
      <label className="check">
        <input type="checkbox" checked={on} onChange={(e) => save(e.target.checked, min, p.influenced)} />
        <span className="group-title">Bases worth picking up</span>
      </label>
      <p className="muted small">Plain, magic or rare bases crafters pay for, by what sellers ask (poe.ninja, only bases with 10 or more for sale).
        Your own edits still win.</p>
      <div className="bs-row">
        <span>Stand out from</span>
        <input type="number" min={10} step={10} value={min} onChange={(e) => type(Math.max(10, +e.target.value || 10))} />
        <CurrencyIcon id="chaos" />
        {p.divine != null && <button className="link small" onClick={() => type(Math.round(p.divine!))}>1 divine</button>}
        <label className="check"><input type="checkbox" checked={p.influenced} onChange={(e) => (on ? save(true, min, e.target.checked) : load(min, e.target.checked))} /><span>Influenced ones too</span></label>
      </div>
      <div className="muted small">{p.total === 0 ? "No base lists at that much now." : `${p.total} ${on ? "stand out" : "would stand out"} right now`}</div>
      <div className="bs-list">
        {shown.map((x) => (
          <div key={x.base + x.influence} className="bs-item" title={`${x.listings.toLocaleString()} for sale at item level ${x.ilvl}`}>
            <ItemIcon name={x.base} base={x.base} size={34} />
            <div><div className="bs-name">{x.base}</div>
              <div className="muted small">{x.influence && `${x.influence} · `}item level {x.ilvl}+</div></div>
            <Money chaos={x.value} divine={p.divine} />
          </div>
        ))}
      </div>
      {p.list.length > 16 && <button className="link small" onClick={() => setAll(!all)}>{all ? "Show fewer" : `Show ${p.list.length - 16} more`}</button>}
      {err && <div className="qm-warn small">{err}</div>}
    </div>
  );
}
