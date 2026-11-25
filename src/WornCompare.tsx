import { useEffect, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./worn.css";

export type Comparison = {
  slot: string; current: string; rows: { stat: string; now: number; this: number }[]; gain: string[]; lose: string[];
  worn?: boolean; icon?: string; note?: string; resists?: { name: string; now: number; after: number }[];
};
type WornView = { slots: string[]; character: string; at: number | null; by_hand: boolean; login: boolean; stale: boolean; list: Comparison[] };

function ago(secs: number) {
  const m = Math.max(0, Math.round((Date.now() / 1000 - secs) / 60));
  return m < 1 ? "just now" : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} days ago`;
}

/** After the story resistances drop 60, so 135% from gear, tree and jewels reaches the 75% cap. */
const CAP = 135;

/** The whole gear's resistances that change, and which ones fall under the cap. */
export function Resists({ list }: { list: { name: string; now: number; after: number }[] }) {
  if (list.length === 0) return null;
  return (
    <div className="wc-res" title={`Added up from all the gear you wear, without your passive tree, jewels and auras. In maps ${CAP}% in total reaches the 75% cap (elemental).`}>
      <span className="wc-res-title">All your gear</span>
      {list.map((r) => (
        <span key={r.name} className={`wc-r wc-${r.name.toLowerCase()}`}>
          <b>{r.name}</b> {r.now} → <em className={r.after >= r.now ? "up" : "down"}>{r.after}%</em>
        </span>
      ))}
    </div>
  );
}

/** The price window's comparison: what you're wearing first, your build guide's gear a click away. */
export default function WornCompare({ raw, itemClass, build, children }: {
  raw: string; itemClass: string; build: Comparison[];
  /** Draws a list of comparisons (the window's own block), with this heading. */
  children: (list: Comparison[], title: string, head: (c: Comparison) => ReactNode) => ReactNode;
}) {
  const [v, setV] = useState<WornView | null>(null);
  const [tab, setTab] = useState<"worn" | "build">("worn");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const asked = useRef("");

  const refresh = async (quiet: boolean) => {
    setBusy(true);
    if (!quiet) setErr("");
    try { setV(await invoke<WornView | null>("worn_refresh", { text: raw, class: itemClass })); setErr(""); }
    catch (e) { if (!quiet) setErr(String(e)); }
    setBusy(false);
  };
  useEffect(() => {
    let on = true;
    asked.current = raw;
    setErr(""); setV(null); setTab("worn");
    invoke<WornView | null>("worn_compare", { text: raw, class: itemClass }).then((x) => {
      if (!on) return;
      setV(x);
      // Old or never read: ask the site once, quietly, after the price search has gone first.
      if (x && x.login && x.stale) setTimeout(() => { if (on && asked.current === raw) refresh(true); }, 1500);
    }).catch(() => { if (on) setV(null); });
    return () => { on = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [raw, itemClass]);

  const wear = (slot: string) => invoke<WornView | null>("worn_set", { text: raw, class: itemClass, slot }).then(setV).catch((e) => setErr(String(e)));

  if (!v) return build.length ? <>{children(build, "Against your build", () => null)}</> : null;
  const showBuild = build.length > 0 && (tab === "build" || v.at == null);
  return (
    <div className="wc">
      {build.length > 0 && v.at != null && (
        <div className="wc-tabs">
          <button className={tab === "worn" ? "on" : ""} onClick={() => setTab("worn")}>What I wear</button>
          <button className={tab === "build" ? "on" : ""} onClick={() => setTab("build")}>My build guide</button>
        </div>
      )}
      {showBuild ? children(build, "Against your build guide", () => null)
        : v.list.length > 0 ? children(v.list, "Better than mine?", (c) => c.icon ? <img className="wc-pic" src={c.icon} alt="" /> : null)
        : v.at != null && <div className="wc-none muted small">You're wearing this one, or nothing in that slot yet.</div>}
      <div className="wc-foot muted small">
        {v.at != null
          ? <span title="pathofexile.com saves your gear each time you change zone">{v.by_hand ? "Gear you told me about" : `${v.character}'s gear`} · {ago(v.at)}</span>
          : <span>{v.login ? (busy ? "Reading what you wear…" : "Your gear hasn't been read yet") : "Compare with your own gear:"}</span>}
        {v.login
          ? <button className="link small" disabled={busy} onClick={() => refresh(false)}>{busy ? "Reading…" : "Read again"}</button>
          : <span className="wc-wear">{v.slots.map((s) => <button key={s} className="link small" title="Press Ctrl+D on each item you wear and click this once, or log in with pathofexile.com in Settings → Map tracker"
              onClick={() => wear(s)}>I'm wearing this{v.slots.length > 1 ? ` (${s})` : ""}</button>)}</span>}
      </div>
      {err && <div className="qm-warn small">{err}</div>}
    </div>
  );
}
