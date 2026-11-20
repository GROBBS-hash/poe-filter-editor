import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";

type Priced = { name: string; value: number | null; count: number; me: boolean };
type Related = { own: number | null; made_from: Priced | null; makes: Priced | null; group: Priced[]; set: boolean; drops: Priced[] };

function Money({ v, divine }: { v: number | null; divine?: number | null }) {
  if (v == null) return <span className="muted">—</span>;
  if (divine && v >= divine) return <>{+(v / divine).toFixed(1)}<CurrencyIcon id="divine" /></>;
  return <>{v >= 10 ? Math.round(v) : +v.toFixed(v < 1 ? 2 : 1)}<CurrencyIcon id="chaos" /></>;
}

function Row({ p, divine, label }: { p: Priced; divine?: number | null; label?: string }) {
  const v = p.value;
  return (
    <li className={(p.me ? "me " : "") + (v != null && v >= 100 ? "hi" : v != null && v >= 20 ? "mid" : "")}>
      <ItemIcon name={p.name} size={22} />
      <span className="bu-name">{label ?? p.name}</span>
      <span className="bu-price"><Money v={v} divine={divine} /></span>
    </li>
  );
}

/** Awakened-style "related items": for a fragment, invitation or boss key, the rest of its set (or the parts it's
 *  made from) with the total, and what the boss drops, most valuable first. Prices from poe.ninja. */
export default function RelatedItems({ name, divine, onData }: { name: string; divine?: number | null; onData?: (has: boolean) => void }) {
  const [r, setR] = useState<Related | null>(null);
  const [all, setAll] = useState(false);
  useEffect(() => {
    let live = true;
    setAll(false);
    invoke<Related | null>("related_items", { name }).then((x) => { if (live) { setR(x ?? null); onData?.(!!x); } })
      .catch(() => { if (live) { setR(null); onData?.(false); } });
    return () => { live = false; };
  }, [name]);
  if (!r) return null;

  const own = r.own;
  const setTotal = r.set && r.group.length > 1 && r.group.every((p) => p.value != null) ? r.group.reduce((s, p) => s + p.value!, 0) : null;
  const drops = all || r.drops.length <= 10 ? r.drops : r.drops.slice(0, 8);
  const cmp = (parts: number, whole: number | null, partsLabel: string, wholeLabel: string) => whole == null ? null : (
    <div className="muted small rel-cmp">
      {partsLabel} <b><Money v={parts} divine={divine} /></b> vs {wholeLabel} <b><Money v={whole} divine={divine} /></b>
      {Math.abs(parts - whole) / Math.max(parts, whole) > 0.1 && <> → {parts < whole ? "cheaper to buy the parts" : "cheaper to buy it whole"}</>}
    </div>
  );

  return (
    <div className="bu rel">
      {r.made_from && (
        <>
          <div className="bu-title">Made from</div>
          <ul><Row p={r.made_from} divine={divine} label={`${r.made_from.count} × ${r.made_from.name}`} /></ul>
          {r.made_from.value != null && cmp(r.made_from.value * r.made_from.count, own, `${r.made_from.count} parts`, "this")}
        </>
      )}
      {r.makes && (
        <>
          <div className="bu-title">{r.makes.count} of these make</div>
          <ul><Row p={r.makes} divine={divine} /></ul>
          {r.makes.value != null && own != null && (
            <div className="muted small rel-cmp">Worth <b><Money v={r.makes.value / r.makes.count} divine={divine} /></b> each as part of one
              {own > 0 && <> ({r.makes.value / r.makes.count > own * 1.1 ? "more" : r.makes.value / r.makes.count < own * 0.9 ? "less" : "about the same"} than selling them alone)</>}</div>
          )}
        </>
      )}
      {r.group.length > 0 && (
        <>
          <div className="bu-title">{r.set ? `The set (${r.group.length} needed)` : "Goes with"}</div>
          <ul>{r.group.map((p) => <Row key={p.name} p={p} divine={divine} />)}</ul>
          {setTotal != null && <div className="rel-total"><span>Whole set</span><span className="bu-price"><Money v={setTotal} divine={divine} /></span></div>}
        </>
      )}
      {r.drops.length > 0 && (
        <>
          <div className="bu-title">{r.group.some((p) => p.name.includes(" Piece of ")) ? "Combines into"
            : name === "Ancient Orb" ? "Can reroll a unique into" : "Drops"}</div>
          <ul>{drops.map((p) => <Row key={p.name} p={p} divine={divine} />)}</ul>
          {r.drops.length > 10 && <button className="link small" onClick={() => setAll(!all)}>{all ? "Show fewer" : `Show all ${r.drops.length}`}</button>}
        </>
      )}
    </div>
  );
}
