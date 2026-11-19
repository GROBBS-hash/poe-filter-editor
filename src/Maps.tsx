import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import { CurrencyIcon } from "./icons";

type MapMod = { id: number; text: string; token: string; scary: number; nightmare: boolean; trade_id: string | null; danger: string | null };
type Wants = { quantity: number | null; pack_size: number | null; rarity: number | null; more_maps: number | null; more_currency: number | null;
  more_scarabs: number | null; more_cards: number | null; tier_min: number | null; kind: string; corrupted_ok: boolean };
type ToolState = { mods: MapMod[]; bad: number[]; wants: Wants; source: string };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; instant?: boolean };
type Result = { listings: Listing[]; summary: [number, number] | null; total: number; url: string; divine: number | null };

/** The stash search box takes up to 250 characters. */
const LIMIT = 250;

const NUMBERS: [keyof Wants, string][] = [
  ["quantity", "Item quantity"], ["pack_size", "Pack size"], ["rarity", "Item rarity"], ["more_maps", "More maps"],
  ["more_currency", "More currency"], ["more_scarabs", "More scarabs / threads"], ["more_cards", "More divination cards"],
];
const KINDS: [string, string][] = [["", "Any map"], ["originator", "8-mod (Originator)"], ["conqueror", "Conqueror"], ["elder", "Elder"], ["shaper", "Shaper"]];

/** Map tools: a stash regex that hides maps you can't run, and a trade search for maps you can. */
export default function Maps() {
  const [st, setSt] = useState<ToolState | null>(null);
  const [err, setErr] = useState("");
  const [regex, setRegex] = useState("");
  const [copied, setCopied] = useState(false);
  const [q, setQ] = useState("");
  const [nightmare, setNightmare] = useState(false);
  const [res, setRes] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [tradeErr, setTradeErr] = useState("");
  const loaded = useRef(false);

  useEffect(() => { invoke<ToolState>("map_tool_get").then(setSt).catch((e) => setErr(String(e))); }, []);

  // Every change: rebuild the regex and keep the choices.
  useEffect(() => {
    if (!st) return;
    invoke<string>("map_tool_regex", { bad: st.bad, wants: st.wants }).then(setRegex).catch(() => {});
    if (!loaded.current) { loaded.current = true; return; }
    const t = window.setTimeout(() => invoke("map_tool_save", { bad: st.bad, wants: st.wants }).catch(() => {}), 400);
    return () => window.clearTimeout(t);
  }, [st]);

  const shown = useMemo(() => {
    if (!st) return [];
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    return st.mods.filter((m) => (nightmare || !m.nightmare || st.bad.includes(m.id)) && words.every((w) => m.text.toLowerCase().includes(w)));
  }, [st, q, nightmare]);

  if (!st) return <div className="card">{err || "Loading the map mod list…"}</div>;
  const setWants = (p: Partial<Wants>) => { setSt({ ...st, wants: { ...st.wants, ...p } }); setCopied(false); };
  const toggle = (id: number) => { setSt({ ...st, bad: st.bad.includes(id) ? st.bad.filter((x) => x !== id) : [...st.bad, id] }); setCopied(false); };
  const fromCheck = async () => {
    const s = await invoke<{ map_avoid?: string[] | null }>("get_settings").catch(() => ({ map_avoid: null }));
    const avoid = s.map_avoid;
    setSt({ ...st, bad: st.mods.filter((m) => m.danger && (avoid == null || avoid.includes(m.danger))).map((m) => m.id) });
  };
  const copy = () => { invoke("copy_text", { text: regex }); setCopied(true); };
  const trade = async () => {
    setBusy(true); setTradeErr(""); setRes(null);
    try { setRes(await invoke<Result>("map_tool_trade", { bad: st.bad, wants: st.wants })); } catch (e) { setTradeErr(String(e)); }
    setBusy(false);
  };
  const num = (v: string) => (v === "" ? null : Math.max(0, Math.min(999, Math.round(+v) || 0)));

  return (
    <>
      <div className="card">
        <h2>Stash search for your maps</h2>
        <p className="muted small">Paste this into the search box of your map stash or map device. It hides maps with any mod you ticked below,
          and keeps only maps with the minimums you set.</p>
        <textarea className="map-regex" readOnly value={regex || "Tick a mod or set a minimum below."} rows={3} />
        <div className="row">
          <button className="primary" disabled={!regex} onClick={copy}>{copied ? "Copied" : "Copy"}</button>
          <span className={`small ${regex.length > LIMIT ? "warn-text" : "muted"}`}>{regex.length}/{LIMIT} characters
            {regex.length > LIMIT ? ": too long for the game's search box. Untick some of the milder mods." : ""}</span>
        </div>
        <p className="muted small">In game: Ctrl+D on a map has a “Copy map regex” button too.</p>
      </div>

      <div className="card">
        <h2>What you want in a map</h2>
        <p className="muted small">Leave a box empty to not mind. Minimums are in %.</p>
        <div className="map-wants">
          {NUMBERS.map(([k, label]) => (
            <label key={k}><span>{label}</span>
              <input className="num" type="number" min={0} value={(st.wants[k] as number | null) ?? ""} placeholder="any"
                onChange={(e) => setWants({ [k]: num(e.target.value) } as Partial<Wants>)} /></label>
          ))}
        </div>
        <h3>For buying on trade</h3>
        <div className="map-wants">
          <label><span>Map tier at least</span>
            <input className="num" type="number" min={1} max={17} value={st.wants.tier_min ?? ""} placeholder="16"
              onChange={(e) => setWants({ tier_min: num(e.target.value) })} /></label>
          <label><span>Kind of map</span>
            <select value={st.wants.kind} onChange={(e) => setWants({ kind: e.target.value })}>
              {KINDS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}</select></label>
          <label className="check"><input type="checkbox" checked={st.wants.corrupted_ok} onChange={(e) => setWants({ corrupted_ok: e.target.checked })} />
            <span>Corrupted maps are fine</span></label>
        </div>
        <div className="row">
          <button className="primary" disabled={busy} onClick={trade}>{busy ? "Searching…" : "Find maps like this on trade"}</button>
          {res && <button onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>}
        </div>
        {tradeErr && <div className="note warn-text">{tradeErr}</div>}
        {res && (
          <div className="map-trade">
            <div className="small">{res.total === 0 ? "None for sale like that: lower a minimum or untick a mod."
              : <>{res.total} for sale{res.summary ? <>, from <b>{Math.round(res.summary[0])}</b><CurrencyIcon id="chaos" /> (usually about <b>{Math.round(res.summary[1])}</b><CurrencyIcon id="chaos" />{res.divine ? <> = {+(res.summary[1] / res.divine).toFixed(2)}<CurrencyIcon id="divine" /></> : null})</> : null}</>}</div>
            <ul className="map-trade-list">
              {res.listings.map((l, i) => (
                <li key={i}><b>{+l.amount.toFixed(2)}</b>{["chaos", "divine"].includes(l.currency) ? <CurrencyIcon id={l.currency} /> : ` ${l.currency}`}
                  <span className="muted small">{l.account}</span>
                  {l.instant ? <span className="tw-instant">Instant</span>
                    : l.whisper && <button className="link small" onClick={() => invoke("copy_text", { text: l.whisper })}>Copy whisper</button>}</li>
              ))}
            </ul>
          </div>
        )}
      </div>

      <div className="card">
        <h2>Mods you can't run ({st.bad.length})</h2>
        <div className="row">
          <input className="text" placeholder="Search mods, e.g. reflect" value={q} onChange={(e) => setQ(e.target.value)} />
          <button onClick={fromCheck} title="Tick the mods on your Settings → Map check list">Use my map check list</button>
          <button onClick={() => setSt({ ...st, bad: [] })}>Untick all</button>
          <label className="check"><input type="checkbox" checked={nightmare} onChange={(e) => setNightmare(e.target.checked)} /><span>Show nightmare mods</span></label>
        </div>
        <div className="map-mods">
          {shown.map((m) => (
            <label key={m.id} className={`map-mod ${st.bad.includes(m.id) ? "on" : ""}`}>
              <input type="checkbox" checked={st.bad.includes(m.id)} onChange={() => toggle(m.id)} />
              <span>{m.text}
                {m.danger && <span className="tw-map-tag">{m.danger}</span>}
                {m.nightmare && <span className="map-nm">nightmare</span>}</span>
            </label>
          ))}
        </div>
        <p className="muted small">Sorted most dangerous first. Map mod list and search codes from{" "}
          <button className="link small" onClick={() => invoke("open_url", { url: "https://poe.re" })}>poe.re</button> ({st.source}).</p>
      </div>
    </>
  );
}
