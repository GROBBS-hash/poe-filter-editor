import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { friendly } from "./errors";
import { ItemIcon } from "./icons";
import { MechArt } from "./mechArt";
import { Tip } from "./Help";
import "./farm.css";
import FarmNow, { useFarmNow, type FarmView } from "./FarmNow";
import FarmBoard, { type Strat } from "./FarmBoard";

type Drop = { name: string; pic?: string; chaos: number; sold_div: number; change: number | null; jackpot: boolean };
type Why = { text: string; src: string };
type Thread = { key: string; thread: string; count: number; each: number; change: number | null };
type MapTip = { map: string; why: Why[]; yours: [number, number] | null };
type Farm = { keys: string[]; title: string; score: number; value: number; steady: number; jackpots: number; raw: number; verdict: string;
  estimate: boolean; players: number | null; change: number | null; drops: Drop[]; threads: Thread[]; cost: number; why: Why[];
  also: [string, string][]; altar: Why | null; maps: MapTip[]; avoid: Why[]; creator: [string, string, string] | null; rolls: string | null; unseen: string[]; own: Own | null; warn: Why[]; picked: boolean };
/** What your own maps made with a tree for it. */
type Own = { div_hour: number; chaos_map: number; maps: number; few: boolean; trees: string[] };
type OwnRow = { name: string; keys: string[]; title: string; own: Own };
type Worth = { key: string; label: string; pic: string; share: number; steady: number; jackpots: number; raw: number; change: number | null;
  drops: Drop[]; unseen: string | null; estimate: boolean; blind: boolean };
type Finder = { farms: Farm[]; mechanics: Worth[]; trees_from: string; trees: number; prices_from: string; temples_from: string; own: OwnRow[]; divine: number; strategies?: Strat[] };
type Ask = { count: "all" | "steady"; max_cost: number | null; size: number; want: string[] };

const SRC: Record<string, string> = { maxroll: "maxroll", "poe.ninja": "players' trees", guides: "guide sites", community: "community tree", "patch notes": "patch notes", "trade site": "trade site" };
const VERDICT: Record<string, string> = { "Under-farmed": "good", Strong: "good", Solid: "ok", Crowded: "warn" };
const HINT: Record<string, string> = {
  "Under-farmed": "Its drops sell well and few players go for it", Strong: "Its drops sell well for how many players go for it",
  Solid: "About what a typical mechanic sells for each farmer", Crowded: "Many players go for it for what its drops sell",
};
const COSTS: [string, number | null][] = [["Any cost", null], ["Cheap Threads (≤ 30c)", 30], ["No Threads", 0]];

const money = (c: number, div: number) => c >= div ? `${(c / div).toFixed(c / div >= 10 ? 0 : 1)} div` : c >= 10 ? `${Math.round(c)}c` : `${c.toFixed(c >= 1 ? 1 : 2)}c`;
const day = (iso: string) => { const d = new Date(iso); return isNaN(d.getTime()) ? iso : d.toLocaleDateString(undefined, { day: "numeric", month: "short" }); };
const short = (map: string) => map.replace(/ Map$/, "");

function Change({ v }: { v: number | null }) {
  if (v == null) return null;
  return <span className={v >= 0 ? "ff-up" : "ff-dn"}>{v >= 0 ? "▲" : "▼"} {Math.abs(Math.round(v))}%</span>;
}

const perHour = (o: Own) => `${o.div_hour >= 10 ? Math.round(o.div_hour) : o.div_hour.toFixed(o.div_hour >= 1 ? 1 : 2)} div an hour`;

function Yours({ o }: { o: Own | null }) {
  if (!o) return <span className="muted">no maps yet</span>;
  return <span title={`${Math.round(o.chaos_map)}c a map on ${o.trees.join(", ")}${o.few ? "\nOnly a few maps so far: luck still decides it" : ""}`}>
    <b className={o.few ? "" : "ff-up"}>{perHour(o)}</b> <span className="muted small">({o.maps} map{o.maps > 1 ? "s" : ""}{o.few ? ", early" : ""})</span></span>;
}

function Reasons({ list }: { list: Why[] }) {
  return <ul className="ff-why">{list.map((w, i) => <li key={i}>{w.text} <span className="ff-src">{SRC[w.src] ?? w.src}</span></li>)}</ul>;
}

/** Where a farm's value comes from: regular drops, big drops, raw currency (estimate). */
function Split({ f }: { f: Pick<Farm, "steady" | "jackpots" | "raw"> }) {
  const total = f.steady + f.jackpots + f.raw || 1;
  const part = (v: number, c: string, t: string) => v / total > 0.01 && <i className={c} style={{ width: `${v / total * 100}%` }} title={`${t}: ${Math.round(v / total * 100)}%`} />;
  return (
    <div className="ff-split">
      <div className="ff-bar">{part(f.steady, "steady", "Regular drops")}{part(f.raw, "raw", "Raw currency (estimate)")}{part(f.jackpots, "jack", "Rare big drops (worth a div or more)")}</div>
      <div className="ff-legend">
        {f.steady > 0 && <span><i className="steady" />regular drops {Math.round(f.steady / total * 100)}%</span>}
        {f.raw > 0 && <span><i className="raw" />raw currency ~{Math.round(f.raw / total * 100)}%</span>}
        {f.jackpots > 0 && <span><i className="jack" />big drops {Math.round(f.jackpots / total * 100)}%</span>}
      </div>
    </div>
  );
}

function Maps({ f }: { f: Farm }) {
  return (
    <div className="ff-maps">
      {f.maps.length === 0 && <div className="small muted">No map stands out for this one.</div>}
      {f.maps.map((m) => (
        <div key={m.map} className="ff-map">
          <ItemIcon name={m.map} size={30} />
          <div>
            <b>{short(m.map)}</b>
            {m.yours && <span className="muted small"> · yours: {m.yours[0].toFixed(0)} min a map ({m.yours[1]})</span>}
            {m.why[0] && <div className="small">{m.why[0].text} <span className="ff-src">{SRC[m.why[0].src] ?? m.why[0].src}</span></div>}
          </div>
        </div>
      ))}
      {f.avoid.map((a, i) => <div key={i} className="small ff-avoid">Skip {a.text} <span className="ff-src">{SRC[a.src] ?? a.src}</span></div>)}
    </div>
  );
}

function Card({ f, best, lead, div, onBuild, onFarm, now }: { f: Farm; best: boolean; /** Its Farm this is the gold one (one per screen). */ lead: boolean; div: number; onBuild: () => void; onFarm: () => Promise<void>; now: boolean }) {
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(false);
  return (
    <article className={`ff-card ${best ? "best" : ""}`}>
      <header>
        <span className="ff-pics">{f.keys.map((k) => <MechArt key={k} m={k} size={40} />)}</span>
        <div>{f.picked && <div className="ff-picked">Your pick</div>}<h3>{f.title}</h3><span className={`ff-tag ${VERDICT[f.verdict] ?? "ok"}`} title={HINT[f.verdict]}>{f.verdict}</span></div>
        <div className="ff-score" title="Against the best mix there is today"><b>{Math.round(f.score)}</b><span>/100</span></div>
      </header>
      <Split f={f} />
      <div className="ff-drops">
        {f.drops.slice(0, 6).map((d) => (
          <span key={d.name} title={`${d.name}: ${money(d.chaos, div)} each`}>
            <ItemIcon name={d.pic ?? d.name} base={d.pic ?? d.name} size={30} /><small>{money(d.chaos, div)}</small>
          </span>
        ))}
      </div>
      <dl>
        <div><dt>Your own maps</dt><dd><Yours o={f.own} /></dd></div>
        <div><dt>Drop prices, 7 days</dt><dd><Change v={f.change} /></dd></div>
        <div><dt>Threads a map</dt><dd>{f.threads.length === 0 ? "none" : (
          <span className="ff-threads" title={f.threads.map((t) => `${t.count} × ${t.thread} at ${money(t.each, div)}`).join("\n") + "\nBefore the free ones from Shaped Regions"}>
            {f.threads.map((t) => <span key={t.key}>{t.count}<ItemIcon name={t.thread} size={20} /></span>)} {money(f.cost, div)}
          </span>)}</dd></div>
        {f.players != null && <div><dt>Players running {f.keys.length > 1 ? "both" : "it"}</dt><dd>{(f.players * 100).toFixed(f.players < 0.02 ? 1 : 0)}%</dd></div>}
      </dl>
      {f.warn.slice(0, open ? 3 : 1).map((w, i) => <div key={i} className="small ff-warn">{w.text} <span className="ff-src">{SRC[w.src] ?? w.src}</span></div>)}
      <div className="ff-h">Best maps</div>
      <Maps f={f} />
      {f.also.length > 0 && <div className="small"><span className="muted" title="Tick these under Build my own if you have points left">Also worth points: </span>{f.also.map((a) => a[1]).join(", ")}</div>}
      {open && (
        <div className="ff-more">
          {f.why.length > 0 && <><div className="ff-h">Why together</div><Reasons list={f.why} /></>}
          {f.altar && <><div className="ff-h">Altars</div><Reasons list={[f.altar]} /></>}
          {f.rolls && <><div className="ff-h">Roll your maps for</div><p className="small">{f.rolls} <span className="ff-src">maxroll</span></p></>}
          {f.maps.some((m) => m.why.length > 1) && <><div className="ff-h">More on the maps</div>
            <Reasons list={f.maps.flatMap((m) => m.why.slice(1).map((w) => ({ ...w, text: `${short(m.map)} · ${w.text}` })))} /></>}
          {f.unseen.length > 0 && <p className="muted small">Not counted (no price list): {f.unseen.join(", ")}.</p>}
          {f.creator && <button className="link small" onClick={() => invoke("open_url", { url: `https://maxroll.gg/poe/farming-strategy-planner/${f.creator![0]}` })}>
            See “{f.creator[1].replace(/^"[^"]*":\s*/, "")}” by {f.creator[2] || "a creator"} on maxroll</button>}
        </div>
      )}
      <footer>
        <button className={lead ? "primary" : ""} disabled={busy || now} title="Saves its tree and uses it, lists the Threads and maps to get, and shows your profit after costs"
          onClick={() => { setBusy(true); onFarm().finally(() => setBusy(false)); }}>{now ? "Farming now" : busy ? "Setting up…" : "Farm this"}</button>
        <button onClick={onBuild}>Build this tree</button>
        <button onClick={() => setOpen(!open)} aria-expanded={open}>{open ? "Less" : "Why?"}</button>
      </footer>
    </article>
  );
}

function readAsk(): Ask {
  const plain: Ask = { count: "all", max_cost: null, size: 0, want: [] };
  try { return { ...plain, ...JSON.parse(localStorage.getItem("farm-ask") ?? "{}") }; } catch { return plain; }
}

function Seg<T>({ opts, now, set }: { opts: [string, T][]; now: T; set: (v: T) => void }) {
  return <span className="ap-seg">{opts.map(([l, v]) => <button key={l} className={v === now ? "on" : ""} onClick={() => set(v)}>{l}</button>)}</span>;
}

/** "Copy for Claude" (the farm finder's AI part, planned 24 Nov): everything the finder knows, as plain text to paste
 *  into a chat with Claude and ask what to run. Numbers only, no account details. */
function forClaude(r: Finder, ask: Ask): string {
  const d = r.divine;
  const m = (c: number) => money(c, d);
  const lines: string[] = [
    "I play Path of Exile 1 (3.30). My editor's farm finder compared poe.ninja prices, players' atlas trees and creator strategies.",
    `Data: prices ${day(r.prices_from)}, ${r.trees.toLocaleString()} players' trees from ${day(r.trees_from)}. 1 divine = ${Math.round(d)} chaos.`,
    `My filters: ${ask.count === "steady" ? "regular drops only" : "all drops"}${ask.max_cost == null ? "" : ask.max_cost === 0 ? ", no Threads" : `, Threads up to ${ask.max_cost}c`}${ask.want.length ? `, I want to run: ${ask.want.join(", ")}` : ""}.`,
    "",
  ];
  const strats = r.strategies ?? [];
  if (strats.length && !ask.want.length) {
    lines.push("## Creator strategies at today's prices (div per hour by their own pace, after costs)");
    for (const x of strats.slice(0, 8)) {
      lines.push(`- ${x.title}: about ${x.div_hour.toFixed(1)} div/hour (${x.runs_hour.toFixed(0)} runs/hour${x.runs_guess ? ", pace guessed" : ""}), costs ${m(x.cost)} per ${x.unit || "run"}${x.missing.length ? `; not priced: ${x.missing.join(", ")}` : ""}`);
    }
    lines.push("");
  }
  lines.push("## Best mechanics and mixes (score out of 100: drops sold per player going for it)");
  for (const f of r.farms.slice(0, 8)) {
    lines.push(`- ${f.title}: ${Math.round(f.score)}/100, ${f.verdict}${f.estimate ? " (partly estimated)" : ""}; Threads ${f.cost > 0 ? m(f.cost) : "none"}${f.maps[0] ? `; maps: ${f.maps.slice(0, 3).map((x) => short(x.map)).join(", ")}` : ""}`);
    if (f.drops.length) lines.push(`  top drops: ${f.drops.slice(0, 4).map((x) => `${x.name} ${m(x.chaos)}`).join(", ")}`);
    for (const w of f.why.slice(0, 2)) lines.push(`  why: ${w.text}`);
    for (const w of f.warn.slice(0, 2)) lines.push(`  careful: ${w.text}`);
    if (f.own) lines.push(`  my own maps with it: ${f.own.div_hour.toFixed(1)} div/hour, ${m(f.own.chaos_map)} a map over ${f.own.maps} maps${f.own.few ? " (few maps)" : ""}`);
  }
  if (r.own.length) {
    lines.push("", "## My own atlas trees so far (from my map tracker)");
    for (const x of r.own.slice(0, 8)) lines.push(`- ${x.name}${x.title ? ` (${x.title})` : ""}: ${x.own.div_hour.toFixed(1)} div/hour, ${m(x.own.chaos_map)} a map, ${x.own.maps} maps${x.own.few ? " (few maps)" : ""}`);
  }
  lines.push("", "Question: which should I farm today, and why? Say what you'd check first in my own maps, and any risks (prices dropping, crowded mechanics).");
  return lines.join("\n");
}

/** Atlas page, top: what to farm today. Cross-checks prices and sales, players' trees, creators' trees and maps (core farm.rs). */
export default function FarmFinder({ onBuild, go }: { onBuild: () => void; go?: (tab: string) => void }) {
  const [now, setNow] = useFarmNow();
  const [ask, setAsk] = useState<Ask>(readAsk);
  const [r, setR] = useState<Finder | null>(null);
  const [err, setErr] = useState("");
  const [table, setTable] = useState(false);
  const [copied, setCopied] = useState(false);
  const copyForClaude = () => { if (!r) return; navigator.clipboard.writeText(forClaude(r, ask)).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 2500); }).catch(() => setErr("Couldn't copy")); };
  useEffect(() => {
    let on = true;
    const load = () => invoke<Finder>("farm_find", { ask: ask.want.length ? { ...ask, size: 0 } : ask }).then((x) => { if (on) { setR(x); setErr(""); } }).catch((e) => { if (on) setErr(friendly(e)); });
    load();
    try { localStorage.setItem("farm-ask", JSON.stringify(ask)); } catch { /* fine */ }
    const un = [listen("prices-updated", load), listen("temples-updated", load)];
    return () => { on = false; un.forEach((u) => u.then((f) => f())); };
  }, [ask]);

  const build = (f: Farm) => {
    try {
      // Only the farm itself: the optimiser treats every ticked tile alike, so "also worth points" would crowd it out.
      localStorage.setItem("atlas-picks-sel", JSON.stringify(f.keys)); localStorage.setItem("atlas-opt-goals", "[]");
      localStorage.setItem("atlas-picks-tab", "own");
      localStorage.setItem("atlas-opt-prio", "profit");
    } catch { /* fine */ }
    onBuild();
  };
  /** One button: its tree saved and in use, and everything else listed at the top. */
  const farmThis = async (f: Farm) => {
    setErr("");
    try {
      const v = await invoke<FarmView | null>("farm_start", {
        farm: { title: f.title, keys: f.keys, threads: f.threads.map((t) => [t.thread, t.count]), maps: f.maps.map((m) => m.map), rolls: f.rolls, drops: f.drops.map((d) => d.pic ? "" : d.name).filter(Boolean) },
        threadKeys: f.threads.map((t) => [t.key, t.count]) });
      setNow(v ?? null);
      window.setTimeout(() => document.querySelector(".fn")?.scrollIntoView({ behavior: "smooth", block: "start" }), 60);
    } catch (e) { setErr(friendly(e)); }
  };
  /** A creator farm: its own tree ask (goals, altar, Threads), costs and drops. */
  const farmStrat = async (s: Strat) => {
    setErr("");
    try {
      const v = await invoke<FarmView | null>("farm_start", {
        farm: { title: s.title, keys: s.mech, threads: s.threads, maps: s.maps, rolls: null, drops: s.drops, ask: s.ask, map_cost: Math.round(s.map_cost * 10) / 10,
          roll_cost: s.roll_cost, vaal: s.vaal, entry: s.entry, runs_hour: s.runs_hour, creator_div_hour: s.div_hour, unit: s.unit },
        threadKeys: s.thread_keys });
      setNow(v ?? null);
      window.setTimeout(() => document.querySelector(".fn")?.scrollIntoView({ behavior: "smooth", block: "start" }), 60);
    } catch (e) { setErr(friendly(e)); }
  };
  const rest = r?.farms.slice(3) ?? [];
  // Every mechanic by name, so each is always in the same place.
  const all = [...(r?.mechanics ?? [])].sort((a, b) => a.label.localeCompare(b.label));
  const want = ask.want;
  const pick = (keys: string[]) => {
    setAsk({ ...ask, want: keys });
    document.querySelector(".ff")?.scrollIntoView({ behavior: "smooth", block: "start" });
  };
  const toggle = (k: string) => setAsk({ ...ask, want: want.includes(k) ? want.filter((x) => x !== k) : [...want, k] });

  return (<>
    {now && <FarmNow view={now} setView={setNow} go={go} />}
    <div className="card plain ff">
      <h2>What to farm today <Tip>Ranks mechanics, and mixes of them people really run together, by how much their drops sell for each player
        going for them. It can't see drops per map, so treat it as a good starting point: your own maps are the proof. Raw currency, beasts
        and temples are estimates.</Tip></h2>
      {r && <p className="muted small">Prices from {day(r.prices_from)} (poe.ninja) · {r.trees.toLocaleString()} players' trees from {day(r.trees_from)} · creator trees from maxroll · temples from {day(r.temples_from)} (trade site)</p>}
      {all.length > 0 && <>
        <div className="ap-step">I want to run <span className="ap-step-note">{want.length === 0 ? "(pick one or more, or leave empty for the best of everything)" : ""}</span>
          {want.length > 0 && <button className="link small" onClick={() => setAsk({ ...ask, want: [] })}>Clear</button>}</div>
        <div className="ap-tiles ff-tiles">{all.map((m) => (
          <button key={m.key} className={`ap-tile ${want.includes(m.key) ? "on" : ""}`} aria-pressed={want.includes(m.key)} onClick={() => toggle(m.key)} title={m.label}>
            <MechArt m={m.key} size={44} /><span>{m.label}</span>
          </button>
        ))}</div>
      </>}
      <div className="row ff-opts">
        <Seg opts={[["All drops", "all"], ["Regular drops only", "steady"]] as [string, Ask["count"]][]} now={ask.count} set={(count) => setAsk({ ...ask, count })} />
        <Seg opts={COSTS} now={ask.max_cost} set={(max_cost) => setAsk({ ...ask, max_cost })} />
        {want.length === 0 && <Seg opts={[["Any mix", 0], ["One mechanic", 1], ["Two or more", 2]] as [string, number][]} now={ask.size} set={(size) => setAsk({ ...ask, size })} />}
      </div>
      {err && <div className="small at-msg">{err}</div>}
      {!r && !err && <div className="muted small">Comparing prices and trees…</div>}
      {r && r.farms.length === 0 && <div className="muted small">Nothing fits those choices.</div>}
      {r && want.length === 0 && <FarmBoard list={r.strategies ?? []} div={r.divine} steadyOnly={ask.count === "steady"} now={now?.farm.title ?? null} onFarm={farmStrat} />}
      {r && want.length === 0 && (r.strategies ?? []).length > 0 && <div className="ap-step">By mechanic</div>}
      {r && want.length > 0 && <div className="ap-step">Your pick, and what goes with it</div>}
      {r && <div className="ff-cards">{r.farms.slice(0, 3).map((f, i) => (
        <Card key={f.title} f={f} best={i === 0} lead={i === 0 && !(want.length === 0 && (r.strategies ?? []).length > 0)} div={r.divine} onBuild={() => build(f)} onFarm={() => farmThis(f)} now={now?.farm.title === f.title} />
      ))}</div>}
      {r && r.own.length > 0 && <>
        <div className="ap-step">Your trees so far <span className="ap-step-note">(loot in ordinary maps; each map counts its time inside plus your usual hideout turnaround, breaks left out)</span></div>
        <div className="ff-own">{r.own.slice(0, 6).map((x) => (
          <div key={x.name}>
            {x.keys.map((k) => <MechArt key={k} m={k} size={28} />)}
            <span><b>{x.name}</b><small>{x.title || "no farm of its own"}</small></span>
            <span className="ff-own-n"><Yours o={x.own} /></span>
          </div>
        ))}</div>
      </>}
      {r && rest.length > 0 && <>
        <div className="ap-step">{want.length ? "More mixes with it" : "More ideas"} <span className="ap-step-note">(click one to see it)</span></div>
        <div className="ff-rest">{rest.map((f) => (
          <button key={f.title} onClick={() => pick(f.keys)} title={["Show this one", ...f.why.map((w) => "• " + w.text), ...f.warn.map((w) => "• " + w.text)].join("\n")}>
            {f.keys.map((k) => <MechArt key={k} m={k} size={28} />)}
            <span><b>{f.title}</b><small>{Math.round(f.score)}/100{f.maps[0] ? ` · ${short(f.maps[0].map)}` : ""}{f.cost > 0 ? ` · ${money(f.cost, r.divine)} of Threads` : ""}</small></span>
          </button>
        ))}</div>
      </>}
      {r && <div className="row ff-foot">
        <button className="link small" onClick={() => setTable(!table)}>{table ? "Hide" : "Show"} every mechanic on its own</button>
        <button className="link small" onClick={copyForClaude} title="Copies these results (and your own maps' numbers) as text to paste into a chat with Claude, with a question to ask">{copied ? "Copied: paste it into Claude" : "Copy for Claude"}</button>
      </div>}
      {r && table && (
        <table className="ff-table">
          <caption className="muted small">Divines of drops sold for each 1% of players going for it. ~ is an estimate.</caption>
          <thead><tr><th>Mechanic</th><th>Regular drops</th><th>Big drops</th><th>Raw currency</th><th>Players</th><th>7 days</th><th>Sells most</th></tr></thead>
          <tbody>{r.mechanics.map((m) => (
            <tr key={m.key} className="ff-row" onClick={() => pick([m.key])} title={`Show ${m.label}`}>
              <td><span className="row ff-m"><MechArt m={m.key} size={28} /><b>{m.label}</b></span></td>
              {m.blind
                ? <td colSpan={3} className="muted small">can't be ranked: {m.unseen ?? "its drops"} have no price list</td>
                : <><td>{Math.round(m.steady)}</td><td>{Math.round(m.jackpots)}</td><td>{m.raw > 0 ? `~${Math.round(m.raw)}` : "–"}</td></>}
              <td>{(m.share * 100).toFixed(0)}%</td>
              <td><Change v={m.change} /></td>
              <td><span className="ff-drops tiny">{m.drops.slice(0, 5).map((d) => <span key={d.name} title={`${d.name}: ${money(d.chaos, r.divine)}`}><ItemIcon name={d.pic ?? d.name} base={d.pic ?? d.name} size={22} /></span>)}</span></td>
            </tr>
          ))}</tbody>
        </table>
      )}
    </div>
  </>);
}
