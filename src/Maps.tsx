import { Tip } from "./Help";
import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";
import { MechArt } from "./mechArt";

type MapMod = { id: number; text: string; token: string; scary: number; nightmare: boolean; trade_id: string | null; danger: string | null };
type Wants = { quantity: number | null; pack_size: number | null; rarity: number | null; more_maps: number | null; more_currency: number | null;
  more_scarabs: number | null; more_cards: number | null; tier_min: number | null; kind: string; corrupted_ok: boolean;
  extras?: Extra[]; extras_any?: boolean; price_max?: number | null; instant?: boolean };
/** A line the map must have: the trade site's stat, its words, and the least it must be ("#" lines). */
type Extra = { id: string; text: string; min: number | null };
type ToolState = { mods: MapMod[]; bad: number[]; wants: Wants; source: string };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; instant?: boolean };
type Result = { listings: Listing[]; summary: [number, number] | null; total: number; url: string; divine: number | null };

/** The stash search box takes up to 250 characters. */
const LIMIT = 250;

const NUMBERS: [keyof Wants, string][] = [
  ["quantity", "Item quantity"], ["pack_size", "Pack size"], ["rarity", "Item rarity"], ["more_maps", "More maps"],
  ["more_currency", "More currency"], ["more_scarabs", "More threads"], ["more_cards", "More divination cards"],
];
const KINDS: [string, string][] = [["", "Any map"], ["originator", "8-mod (Originator)"], ["conqueror", "Conqueror"], ["elder", "Elder"], ["shaper", "Shaper"],
  ["blighted", "Blighted"], ["ravaged", "Blight-ravaged"]];
/** One click for the ones people buy maps for (checked on the trade site, 25 Nov). */
const QUICK: { m: string; label: string; id: string; text: string }[] = [
  { m: "shrines", label: "Terror Shrines", id: "enchant.stat_667284445", text: "Area is dominated by Terror Shrines" },
  { m: "shrines", label: "Extra shrines", id: "enchant.stat_1468737867", text: "Area contains an additional Shrine" },
  { m: "strongboxes", label: "Extra strongboxes", id: "enchant.stat_3240183538", text: "Area contains an additional Strongbox" },
  { m: "strongboxes", label: "Ambush", id: "enchant.stat_112276874", text: "Area's inhabitants are lying in ambush" },
  { m: "blight", label: "Blight", id: "enchant.stat_2459443694", text: "Area contains a Blight Encounter" },
  { m: "item:Vaal Orb", label: "Vaal Vessel", id: "implicit.stat_44159086", text: "Area contains an additional guarded Vaal Vessel" },
  { m: "item:Orb of Unmaking", label: "Extra atlas notable", id: "implicit.stat_2834034653", text: "Area is affected by an additional random Unallocated Notable Atlas Passives" },
];
const KIND_NAMES: Record<string, string> = { enchant: "Extra content", implicit: "Implicit", explicit: "Map mod", pseudo: "Total" };

/** "Must have" on the Maps page: search any line a map can carry, or click a common one. */
function MustHave({ wants, set }: { wants: Wants; set: (p: Partial<Wants>) => void }) {
  const [all, setAll] = useState<Extra[]>([]);
  const [q, setQ] = useState("");
  const [err, setErr] = useState("");
  useEffect(() => { invoke<Extra[]>("map_tool_stats").then(setAll).catch((e) => setErr(friendly(e))); }, []);
  const list = wants.extras ?? [];
  const has = (id: string) => list.some((x) => x.id === id);
  const add = (x: Extra) => { if (!has(x.id)) set({ extras: [...list, { id: x.id, text: x.text, min: null }] }); setQ(""); };
  const drop = (id: string) => set({ extras: list.filter((x) => x.id !== id) });
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  // Every word anywhere, then just the start of each word ("terror shrines" finds "Terror Shrine…" too).
  const found = words.join("").length < 2 ? [] : all.filter((s) => !has(s.id) && words.every((w) => s.text.toLowerCase().includes(w.replace(/s$/, "")))).slice(0, 14);
  return (
    <div className="map-must">
      <h3>Must have</h3>
      <div className="mm-quick">
        {QUICK.map((k) => (
          <button key={k.id} className={`mm-tile ${has(k.id) ? "on" : ""}`} aria-pressed={has(k.id)} title={k.text} onClick={() => (has(k.id) ? drop(k.id) : add({ id: k.id, text: k.text, min: null }))}>
            {k.m.startsWith("item:") ? <ItemIcon base={k.m.slice(5)} size={26} /> : <MechArt m={k.m} size={26} />}<span>{k.label}</span>
          </button>
        ))}
      </div>
      <input className="text mm-find" placeholder="Find any other line: shrine, beyond, totems, rare monsters…" value={q} onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && found[0]) add(found[0]); if (e.key === "Escape") setQ(""); }} />
      {found.length > 0 && (
        <ul className="results mm-results">
          {found.map((s) => <li key={s.id} onClick={() => add(s)}><span>{s.text}</span><span className="muted small">{KIND_NAMES[s.id.split(".")[0]] ?? ""}</span></li>)}
        </ul>
      )}
      {words.join("").length >= 2 && found.length === 0 && <div className="muted small">{err || "No map line has those words."}</div>}
      {list.length > 0 && (
        <div className="mm-list">
          {list.map((x) => (
            <div key={x.id} className="mm-row">
              <span className="mm-text">{x.text}</span>
              {/#|an additional/.test(x.text) && !x.id.includes("|") && <label className="small">at least <input className="num" type="number" min={0} placeholder="any" value={x.min ?? ""}
                onChange={(e) => set({ extras: list.map((y) => (y.id === x.id ? { ...y, min: e.target.value === "" ? null : Math.max(0, +e.target.value || 0) } : y)) })} /></label>}
              <button className="link small" aria-label={`Remove ${x.text}`} onClick={() => drop(x.id)}>Remove</button>
            </div>
          ))}
          {list.length > 1 && (
            <div className="mm-any">
              <button className={!wants.extras_any ? "primary" : ""} onClick={() => set({ extras_any: false })}>All of these</button>
              <button className={wants.extras_any ? "primary" : ""} onClick={() => set({ extras_any: true })}>Any one of them</button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

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

  useEffect(() => { invoke<ToolState>("map_tool_get").then(setSt).catch((e) => setErr(friendly(e))); }, []);

  // Every change: rebuild the regex and keep the choices.
  useEffect(() => {
    if (!st) return;
    invoke<string>("map_tool_regex", { bad: st.bad, wants: st.wants }).then(setRegex).catch(() => {});
    if (!loaded.current) { loaded.current = true; return; }
    const t = window.setTimeout(() => invoke("map_tool_save", { bad: st.bad, wants: st.wants }).catch(() => {}), 400);
    return () => window.clearTimeout(t);
  }, [st]);

  const [suggestNote, setSuggestNote] = useState("");
  const shown = useMemo(() => {
    if (!st) return [];
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    return st.mods.filter((m) => (nightmare || !m.nightmare || st.bad.includes(m.id)) && words.every((w) => m.text.toLowerCase().includes(w)));
  }, [st, q, nightmare]);

  if (!st) return <div className="card">{err || "Loading the map mod list…"}</div>;
  const setWants = (p: Partial<Wants>) => { setSt({ ...st, wants: { ...st.wants, ...p } }); setCopied(false); };
  const toggle = (id: number) => { setSt({ ...st, bad: st.bad.includes(id) ? st.bad.filter((x) => x !== id) : [...st.bad, id] }); setCopied(false); };
  // The one list of mods you can't run (Ctrl+D warns about the same ones). Quick ways to fill it:
  const usualDangers = () => setSt({ ...st, bad: st.mods.filter((m) => m.danger).map((m) => m.id) });
  const fromBuild = async () => {
    try {
      const s = await invoke<{ name: string; why: string }[]>("map_suggest");
      const names = s.map((x) => x.name);
      setSt({ ...st, bad: st.mods.filter((m) => m.danger && names.includes(m.danger)).map((m) => m.id) });
      setSuggestNote(`Ticked what your build can't run: ${s.map((x) => `${x.name} (${x.why})`).join(", ") || "nothing"}. Check them over.`);
    } catch (e) { setSuggestNote(String(e)); }
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
        <h2>Stash search for your maps <Tip>Paste this into the search box of your map stash or map device. It hides maps with any
          mod you ticked below, and keeps only maps with the minimums you set.</Tip></h2>
        <p className="muted small">Paste into your map stash's search box.</p>
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
          <label><span className="with-icon">At most <CurrencyIcon id="chaos" /></span>
            <input className="num" type="number" min={1} value={st.wants.price_max ?? ""} placeholder="any"
              onChange={(e) => setWants({ price_max: e.target.value === "" ? null : Math.max(1, Math.round(+e.target.value) || 1) })} /></label>
          <label className="check"><input type="checkbox" checked={!!st.wants.instant} onChange={(e) => setWants({ instant: e.target.checked })} />
            <span>Instant buyout only</span></label>
        </div>
        <MustHave wants={st.wants} set={setWants} />
        <div className="row">
          <button className="primary" disabled={busy} onClick={trade}>{busy ? "Searching…" : "Find maps like this on trade"}</button>
          {res && <button onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>}
        </div>
        {tradeErr && <div className="note warn-text">{tradeErr}</div>}
        {res && (
          <div className="map-trade">
            <div className="small">{res.total === 0 ? "None for sale like that: lower a minimum, raise the price, or take out a line."
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
          <button onClick={fromBuild} title="Tick the mods your Path of Building build can't handle (needs a PoB link on My build)">Suggest from my build</button>
          <button onClick={usualDangers} title="Tick the well-known dangerous mods (reflect, no regen, …)">Tick the usual dangers</button>
          <button onClick={() => setSt({ ...st, bad: [] })}>Untick all</button>
          <label className="check"><input type="checkbox" checked={nightmare} onChange={(e) => setNightmare(e.target.checked)} /><span>Show nightmare mods</span></label>
        </div>
        {suggestNote && <p className="note small">{suggestNote}</p>}
        <p className="muted small">Mods you can't run: Ctrl+D warns about them and the stash search hides them.</p>
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
