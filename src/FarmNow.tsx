import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { friendly } from "./errors";
import { CurrencyIcon, ItemIcon } from "./icons";
import { MechArt } from "./mechArt";
import { Money } from "./Bases";
import { standOut } from "./Find";
import "./farm-now.css";

export type FarmPlan = { title: string; keys: string[]; threads: [string, number][]; maps: string[]; rolls: string | null; drops: string[] };
type FarmData = FarmPlan & { strategy: number; strategy_name: string; started: number; plan: number; map_cost: number; other_cost: number; thread_cost: number | null };
type ThreadNeed = { name: string; each_map: number; need: number; have: number | null; buy: number; each: number | null; bulk: string | null };
type Profit = { maps: number; loot: number; cost: number; net: number; hour: number; likely: [number, number] | null; few: boolean };
export type FarmView = { farm: FarmData; threads: ThreadNeed[]; threads_now: number; cost: number; profit: Profit | null; divine: number | null;
  tree_in_use: boolean; tree_saved: boolean; scanned: number | null; in_filter: boolean };

const c = (v: number) => (Math.abs(v) >= 10 ? Math.round(v).toLocaleString() : +v.toFixed(1));
const perHour = (v: number, div: number | null) => (div ? `${(v / div).toFixed(Math.abs(v / div) >= 10 ? 0 : 1)} div` : `${c(v)}c`);

function Step({ done, icon, title, children }: { done: boolean; icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <div className="fn-step">
      <span className={`fn-tick ${done ? "on" : ""}`}>{done ? "✓" : ""}</span>
      <span className="fn-pic">{icon}</span>
      <div><b>{title}</b><div className="small">{children}</div></div>
    </div>
  );
}

/** A number box that saves a moment after you stop typing. */
function Num({ value, onSave, width = 64, step = 1 }: { value: number; onSave: (v: number) => void; width?: number; step?: number }) {
  const [t, setT] = useState(String(value));
  const timer = useRef(0);
  useEffect(() => { setT(String(value)); }, [value]);
  return <input type="number" min={0} step={step} value={t} style={{ width }} onChange={(e) => {
    setT(e.target.value);
    const v = +e.target.value;
    clearTimeout(timer.current);
    if (e.target.value !== "" && v >= 0) timer.current = window.setTimeout(() => onSave(v), 700);
  }} />;
}

/** Profit a map and an hour after maps and Threads. */
export function ProfitBlock({ v }: { v: FarmView }) {
  const p = v.profit;
  if (!p) return <div className="muted small">No maps on this tree yet. Run one with the map tracker on and what it made shows here.</div>;
  return (
    <div className="fn-profit">
      <div className="fn-sum">
        <span>Loot a map</span><b>{c(p.loot)}<CurrencyIcon id="chaos" /></b>
        <span>Maps and Threads</span><b className="ff-dn">−{c(p.cost)}<CurrencyIcon id="chaos" /></b>
        <span className="fn-net">Profit a map</span><b className={`fn-net ${p.net >= 0 ? "ff-up" : "ff-dn"}`}>{p.net < 0 ? "−" : ""}{c(Math.abs(p.net))}<CurrencyIcon id="chaos" /></b>
      </div>
      <div className="fn-hour">
        <b className={p.hour >= 0 ? "ff-up" : "ff-dn"}>{perHour(p.hour, v.divine)} an hour</b>
        <span className="muted small">
          {p.likely ? `Likely ${perHour(p.likely[0], v.divine).replace(" div", "")} to ${perHour(p.likely[1], v.divine)}, going by your ${p.maps} maps`
            : `From ${p.maps} map${p.maps > 1 ? "s" : ""}: too few to say how sure`}
          {p.likely && p.few && " (still early)"}
        </span>
      </div>
    </div>
  );
}

/** Top of the Atlas page: the farm you picked with "Farm this", set up step by step, and what it makes after costs. */
export default function FarmNow({ view, setView, go }: { view: FarmView; setView: (v: FarmView | null) => void; go?: (tab: string) => void }) {
  const [err, setErr] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const f = view.farm;
  const run = async (cmd: string, args: Record<string, unknown>, said = "") => {
    setBusy(true); setErr(""); setNote("");
    try { const v = await invoke<FarmView | null>(cmd, args); if (v !== undefined) setView(v); setNote(said); } catch (e) { setErr(friendly(e)); }
    setBusy(false);
  };
  const costs = (x: Partial<{ plan: number; map: number; other: number; thread: number | null }>) => run("farm_costs", {
    plan: Math.round(x.plan ?? f.plan), mapCost: x.map ?? f.map_cost, otherCost: x.other ?? f.other_cost, threadCost: x.thread !== undefined ? x.thread : f.thread_cost });
  const sellers = async (t: ThreadNeed) => {
    setErr("");
    try {
      const b = await invoke<{ url: string }>("trade_bulk", { want: t.bulk, pay: "chaos", minimum: null, market: t.each, sold: null, card: false });
      await invoke("open_url", { url: b.url });
    } catch (e) { setErr(friendly(e)); }
  };
  const short = view.threads.filter((t) => t.buy > 0);
  const buyTotal = short.reduce((s, t) => s + t.buy * (t.each ?? 0), 0);
  return (
    <div className="card fn">
      <header>
        {f.keys.map((k) => <MechArt key={k} m={k} size={44} />)}
        <div><div className="ff-picked">Farming now</div><h2>{f.title}</h2></div>
        <button className="link small" disabled={busy} onClick={() => run("farm_stop", {}).then(() => setView(null))}>Stop</button>
      </header>
      <div className="fn-grid">
        <div className="fn-steps">
          <Step done={view.tree_in_use} icon={<span className="fn-key">F7</span>} title="Atlas tree">
            {!view.tree_saved ? <>Its tree was deleted. <span className="muted">Press Farm this on its card again.</span></>
              : view.tree_in_use ? <>Saved as “{f.strategy_name}”. Take the points in game: F7 shows them step by step.</>
              : <>Another tree is in use, so maps aren't counted for this farm. <button className="link small" onClick={() => run("farm_use_tree", {})}>Use its tree</button></>}
          </Step>
          <Step done={short.length === 0} icon={<ItemIcon name={view.threads[0]?.name ?? "Legion Thread"} size={30} />} title="Threads">
            {view.threads.length === 0 ? "None needed." : <>
              <div className="fn-plan">For <Num value={f.plan} onSave={(plan) => costs({ plan })} width={56} /> maps:</div>
              {view.threads.map((t) => (
                <div key={t.name} className="fn-thread">
                  <ItemIcon name={t.name} size={22} /><span>{t.each_map} × {t.name} a map</span>
                  <span className="muted">{t.have == null ? `need ${t.need}` : `have ${Math.min(t.have, t.need)} of ${t.need}`}</span>
                  {t.buy > 0 && t.bulk && <button className="link small" onClick={() => sellers(t)}>Buy {t.buy}{t.each != null && <> (≈ {c(t.buy * t.each)}c)</>}</button>}
                </div>
              ))}
              {short.length > 1 && <div className="muted">All of it: about {c(buyTotal)}c</div>}
              {view.scanned == null && <div className="muted">Scan your stash (Stash worth) and it knows what you already have.</div>}
            </>}
          </Step>
          <Step done={false} icon={<ItemIcon name={f.maps[0] ?? "Map (Tier 16)"} size={30} />} title="Maps">
            {f.maps.length > 0 ? <>{f.maps.map((m) => m.replace(/ Map$/, "")).join(", ")}</> : "Any map you run well."}
            {f.rolls && <div className="muted">Creators roll for: {f.rolls}</div>}
            <div className="row fn-btns">
              <button className="link small" onClick={() => invoke("map_regex_copy").then(() => setNote("Your map search is copied: paste it in the stash search box.")).catch((e) => setErr(friendly(e)))}>Copy my map search</button>
              {go && <button className="link small" onClick={() => go("maps")}>Find maps to buy</button>}
            </div>
          </Step>
          <Step done={view.in_filter} icon={<ItemIcon name={f.drops[0] ?? "Chaos Orb"} size={30} />} title="Filter">
            {view.in_filter
              ? <>Its drops stand out. Press F4 in game. <button className="link small" disabled={busy} onClick={() => run("farm_filter", { on: false, style: standOut("Orange") })}>Back to normal</button></>
              : <button className="link small" disabled={busy} onClick={() => run("farm_filter", { on: true, style: standOut("Orange") }, "Press F4 in game to see it.")}>Make its drops stand out</button>}
          </Step>
        </div>
        <div className="fn-side">
          <ProfitBlock v={view} />
          <div className="fn-costs">
            <b>What a map costs you</b>
            <label><span>The map</span><Num value={f.map_cost} step={0.5} onSave={(map) => costs({ map })} /><CurrencyIcon id="chaos" /></label>
            <label title={f.thread_cost == null ? "At today's prices. Type your own if you get some free (Shaped Regions) or farm them." : "Your own number"}>
              <span>Threads {f.thread_cost == null ? "(today's price)" : <button className="link small" onClick={() => costs({ thread: null })}>use today's price</button>}</span>
              <Num value={+(f.thread_cost ?? view.threads_now).toFixed(1)} step={0.5} onSave={(thread) => costs({ thread })} /><CurrencyIcon id="chaos" /></label>
            <label title="Rolling currency, invitations and so on"><span>Anything else</span><Num value={f.other_cost} step={0.5} onSave={(other) => costs({ other })} /><CurrencyIcon id="chaos" /></label>
            <div className="fn-total"><span>Altogether</span><Money chaos={view.cost} divine={view.divine} /></div>
          </div>
        </div>
      </div>
      {note && <div className="note small">{note}</div>}
      {err && <div className="qm-warn small">{err}</div>}
    </div>
  );
}

/** The farm in use, kept fresh (new prices, a map finished, a change on another page). */
export function useFarmNow(): [FarmView | null, (v: FarmView | null) => void, () => void] {
  const [v, setV] = useState<FarmView | null>(null);
  const load = useCallback(() => { invoke<FarmView | null>("farm_now").then((x) => setV(x ?? null)).catch(() => setV(null)); }, []);
  useEffect(() => {
    load();
    const un = ["farm-now", "prices-updated", "atlas-changed"].map((e) => listen(e, load));
    const t = window.setInterval(load, 60000);
    return () => { un.forEach((u) => u.then((x) => x())); clearInterval(t); };
  }, [load]);
  return [v, setV, load];
}
