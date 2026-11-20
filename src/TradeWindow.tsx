import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";
import BaseUniques from "./BaseUniques";
import RelatedItems from "./RelatedItems";
import { Item, RARITY_COLOUR, itemTitle } from "./types";

type StatFilter = { id: string; label: string; value: number; min: number | null; enabled: boolean; kind: string; tier: number | null; junk?: boolean; roll?: number | null; range?: string | null; affix?: string | null };
type Setup = { item: Item; filters: StatFilter[]; unknown_kinds?: string[]; base_only: boolean; ilvl_min: number | null; ilvl_max?: number | null; links_min: number | null;
  gem_level: number | null; unknown: string[]; category: string | null; raw?: string; misc?: string[]; base?: string; affix_max?: [number, number] | null;
  /** Awakened-style "search as a base": why it's a crafting base, which mods to keep, item level, influences. */
  base_item?: { why: string[]; ids: string[]; ilvl_min: number | null; misc: string[] } | null };
type MapInfo = { mods: { text: string; warn: string | null }[]; unidentified: boolean; tier: number | null; stats: string[] };
type Check = { setup: Setup | null; ninja: [number, string] | null; league: string; from_ground: boolean; error: string | null; bulk?: string | null; map?: MapInfo | null; divine?: number | null; compare?: Comparison[]; instant_all?: boolean;
  /** poe.ninja's last 7 days: % change each day from a week ago (oldest first), and the total. */
  trend?: { points: number[]; change: number } | null };
type Comparison = { slot: string; current: string; rows: { stat: string; now: number; this: number }[]; gain: string[]; lose: string[] };
type Offer = { pay: number; pay_currency: string; get: number; get_currency: string; each: number; stock: number; account: string; whisper: string };
type Bulk = { offers: Offer[]; url: string; divine: number | null };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; instant?: boolean; ilvl: number | null };
type Result = { listings: Listing[]; summary: [number, number] | null; total: number; url: string; divine: number | null };

const UNIT: Record<string, string> = { chaos: "c", exalted: "ex", divine: "div" };
const KIND: Record<string, string> = { pseudo: "total", implicit: "implicit", crafted: "crafted", enchant: "enchant", fractured: "fractured", mutated: "foulborn", property: "" };

/** "3h ago" from the time it was listed. */
function ago(iso: string) {
  const t = Date.parse(iso);
  if (!t) return "";
  const m = Math.max(0, Math.round((Date.now() - t) / 60000));
  return m < 60 ? `${m}m` : m < 60 * 48 ? `${Math.round(m / 60)}h` : `${Math.round(m / 1440)}d`;
}

/** Chaos, or divines when it's worth more than one. */
function money(c: number, divine: number | null) {
  if (divine && c >= divine) return `${(c / divine).toFixed(c / divine < 10 ? 1 : 0)} div`;
  return `${c < 1 ? +c.toFixed(2) : c < 10 ? +c.toFixed(1) : Math.round(c)}c`;
}

/** A price with its orb picture, e.g. "180 [chaos]". */
function Price({ n, cur }: { n: number; cur: string }) {
  const v = n < 1 ? +n.toFixed(3) : n < 10 ? +n.toFixed(2) : n < 100 ? +n.toFixed(1) : Math.round(n);
  return <span className="tw-pr">{v}{["chaos", "divine"].includes(cur) ? <CurrencyIcon id={cur} /> : <> {UNIT[cur] ?? cur}</>}</span>;
}

/** A number that counts up to its value (a quarter second) when it first shows or changes: the big price arriving. */
function useCountUp(target: number, ms = 280) {
  const [v, setV] = useState(target);
  const from = useRef(0);
  useEffect(() => {
    const start = performance.now(), a = from.current;
    let raf = 0;
    const step = (t: number) => {
      const k = Math.min(1, (t - start) / ms);
      const e = 1 - Math.pow(1 - k, 3);
      setV(a + (target - a) * e);
      if (k < 1) raf = requestAnimationFrame(step); else from.current = target;
    };
    raf = requestAnimationFrame(step);
    return () => { cancelAnimationFrame(raf); from.current = target; };
  }, [target, ms]);
  return v;
}

function CountPrice({ n, cur }: { n: number; cur: string }) {
  return <Price n={useCountUp(n)} cur={cur} />;
}

/** A small turning ring while something loads. */
export function Spinner() { return <span className="spin" aria-hidden="true" />; }

/** The same price in the other currency: chaos -> divine, divine -> chaos. */
function Other({ n, cur, divine }: { n: number; cur: string; divine: number | null }) {
  if (!divine) return null;
  if (cur === "chaos") return n / divine < 0.01 ? null : <span className="tw-other">= <Price n={n / divine} cur="divine" /></span>;
  if (cur === "divine") return <span className="tw-other">= <Price n={n * divine} cur="chaos" /></span>;
  return null;
}

/** Every Ctrl+D searches the trade site straight away (bulk items use the exchange instead). */
/** Like Awakened: search straight away when the filters barely matter (uniques, unidentified items, gems,
 *  corpses and other items without a gear category, items read off the ground); rares, gear, jewels and maps wait
 *  for Search so you can pick the stats first. Bulk items use the exchange. Settings can make every item instant. */
const needsTrade = (c: Check) => {
  if (!c.setup || c.bulk) return false;
  const it = c.setup.item;
  return !!c.instant_all || it.rarity === "Unique" || it.unidentified || c.from_ground
    || (!c.setup.category && !c.map && it.item_class !== "Maps");
};
/** What a search depends on: when this changes after a search, the old prices no longer match the filters. */
const searchKey = (s: Setup) => JSON.stringify([s.filters.map((f) => [f.enabled, f.min]), s.base_only, s.ilvl_min, s.ilvl_max ?? null, s.links_min, s.misc ?? []]);

/** Item-level band where base prices jump (75, 82, 84, 86 unlock better mod tiers): [low, high or null]. */
const ilvlBand = (l: number): [number, number | null] => l <= 67 ? [1, 67] : l <= 74 ? [68, 74] : l <= 81 ? [75, 81] : l <= 83 ? [82, 83] : l <= 85 ? [84, 85] : [86, null];
/** "73–74" or "86+": this item's level up to the top of its band. */
const ilvlText = (l: number) => { const top = ilvlBand(l)[1]; return top == null ? `${l}+` : top === l ? `${l}` : `${l}–${top}`; };

type Saved = { id: number; name: string; live: boolean; setup: Setup };

/** Saved searches: run again, or turn on live (pops up new listings while you play). */
function SavedPanel({ list, setList, onRun }: { list: Saved[]; setList: (l: Saved[]) => void; onRun: (s: Saved) => void }) {
  const [err, setErr] = useState("");
  const call = (cmd: string, args: Record<string, unknown>) => invoke<Saved[]>(cmd, args).then((l) => { setList(l); setErr(""); }).catch((e) => setErr(friendly(e)));
  if (list.length === 0) return <div className="muted small tw-saved">No saved searches yet: price check an item, then press Save search.</div>;
  return (
    <div className="tw-saved">
      <div className="muted small">Live searches check the trade site every 90 seconds and pop up new listings.</div>
      <ul className="tw-list">
        {list.map((x) => (
          <li key={x.id}>
            <span className="tw-saved-name" title={x.setup.filters.filter((f) => f.enabled).map((f) => f.label).join("\n")}>{x.name}</span>
            <button className="link small" onClick={() => onRun(x)}>Run</button>
            <button className={`link small ${x.live ? "tw-live-on" : ""}`} onClick={() => call("saved_live", { id: x.id, on: !x.live })}
              title={x.live ? "Live: on (click to stop)" : "Pop up new listings while you play"}>{x.live ? "● Live" : "Live"}</button>
            <button className="link small" onClick={() => call("saved_delete", { id: x.id })} title="Delete">×</button>
          </li>
        ))}
      </ul>
      {err && <div className="qm-warn">{err}</div>}
    </div>
  );
}

/** "Is this an upgrade?": against the gear in the same slot of your PoB build. */
function CompareBlock({ list }: { list: Comparison[] }) {
  const [open, setOpen] = useState(() => { try { return localStorage.getItem("tw-compare") !== "0"; } catch { return true; } });
  const toggle = () => { setOpen(!open); try { localStorage.setItem("tw-compare", open ? "0" : "1"); } catch { /* ignore */ } };
  const score = (c: Comparison) => [c.rows.filter((r) => r.this > r.now).length, c.rows.filter((r) => r.this < r.now).length];
  return (
    <div className="tw-cmp">
      <button className="tw-cmp-toggle" onClick={toggle}>
        <span className="tw-sec">{open ? "▾" : "▸"} Against your build</span>
        {!open && list.map((c) => { const [u, d] = score(c);
          return <Verdict key={c.slot} up={u} down={d} />; })}
      </button>
      {open && list.map((c) => {
        const [up, down] = score(c);
        return (
          <div key={c.slot} className="tw-cmp-slot">
            <div className="tw-cmp-head">
              <span className="small"><span className="muted">{c.slot}:</span> {c.current}</span>
              <Verdict up={up} down={down} />
            </div>
            {c.rows.length > 0 && (
              <table className="tw-cmp-rows">
                <thead><tr><th></th><th>yours</th><th>this</th><th></th></tr></thead>
                <tbody>
                  {c.rows.map((r) => {
                    const d = r.this - r.now;
                    return <tr key={r.stat}><td>{r.stat}</td><td className="muted">{r.now}</td><td>{r.this}</td>
                      <td className={d > 0 ? "up" : "down"}>{d > 0 ? "▲ " : "▼ "}{Math.abs(d)}</td></tr>;
                  })}
                </tbody>
              </table>
            )}
            <ModList title="Only on this item" lines={c.gain} cls="up" mark="+" />
            <ModList title="You'd lose" lines={c.lose} cls="down" mark="−" />
          </div>
        );
      })}
    </div>
  );
}

/** How high a mod rolled: a small bar from its lowest to its highest roll, and the range itself. */
function Roll({ r, range, tier }: { r: number; range?: string | null; tier: number | null }) {
  const pct = Math.round(r * 100);
  const cls = pct >= 80 ? "hi" : pct >= 40 ? "mid" : "lo";
  const word = pct >= 80 ? "high roll" : pct >= 40 ? "mid roll" : "low roll";
  return (
    <span className={`tw-roll ${cls}`} title={`${pct}% of the way from the lowest to the highest roll${tier != null ? " for this tier" : ""}`}>
      <span className="tw-roll-bar"><span style={{ width: `${Math.max(4, pct)}%` }} /></span>{word}
      {range && <span className="tw-roll-range">{tier != null ? `T${tier} range` : "range"} {range}</span>}
    </span>
  );
}

/** Tier 1 is the best roll a mod can have: green for top tiers through to red for low ones. */
const tierClass = (t: number) => (t <= 1 ? "t1" : t === 2 ? "t2" : t === 3 ? "t3" : t <= 5 ? "t45" : "t6");

/** The going rate for a bulk item, like Awakened: price fixers list a handful at a silly price to drag the
 *  number down, so weigh every offer by how many it has in stock and take the middle (a stock-weighted median).
 *  Offers well under that with little stock are marked as likely fixers. */
function bulkTypical(offers: Offer[], divine: number | null, market: number | null = null) {
  const chaos = (o: Offer) => (o.pay_currency === "divine" && divine ? o.each * divine : o.each);
  const rows = offers.map((o) => ({ c: chaos(o), stock: Math.max(1, o.stock || 1) })).sort((a, b) => a.c - b.c);
  const total = rows.reduce((s, r) => s + r.stock, 0);
  let acc = 0, typical = rows[0]?.c ?? 0;
  for (const r of rows) { acc += r.stock; if (acc >= total / 2) { typical = r.c; break; } }
  const medianStock = [...rows].map((r) => r.stock).sort((a, b) => a - b)[Math.floor(rows.length / 2)] ?? 1;
  // Judged against the market price when poe.ninja has one (fixers can fill the whole cheap end of the list).
  const ref = market ?? typical;
  const fixer = (o: Offer) => chaos(o) < ref * 0.75 && (o.stock || 1) < Math.max(5, medianStock / 2);
  return { typical, cheapest: rows[0]?.c ?? 0, fixer };
}

/** The heading a stat row sits under, and a short hint: [title, hint]. */
/** The small heading over a row. `max` = how many prefixes / suffixes the item can hold (Cogwork Ring 2 / 4). */
function rowGroup(f: StatFilter, max: [number, number]): [string, string] {
  if (f.id.startsWith("prop.")) return [f.id.includes("dps") ? "Damage" : "Defences", "at 20% quality"];
  if (f.kind === "pseudo") return f.id.includes("empty") ? ["Open slots", "room for another mod"] : ["Totals", "added up from all its mods"];
  if (f.kind === "enchant") return ["Enchant", ""];
  if (f.kind === "implicit") return ["Implicit", "comes with the base"];
  if (f.affix === "prefix") return ["Prefixes", `up to ${max[0]} on this item`];
  if (f.affix === "suffix") return ["Suffixes", `up to ${max[1]} on this item`];
  return ["Other mods", ""];
}

/** A search found nothing: offers to try it without each ticked stat, then lists what each would find. */
function UntickHelp({ state, onAsk, onPick }: { state: null | "busy" | string | { key: string; label: string; total: number }[];
  onAsk: () => void; onPick: (key: string) => void }) {
  if (state === null) return (
    <button className="tw-untick-ask" onClick={onAsk}
      title="Searches again once without each ticked stat (one trade search each) and shows how many each would find">
      Which stat to untick?</button>
  );
  if (state === "busy") return <div className="tw-untick muted small">Trying the search without each stat…</div>;
  if (typeof state === "string") return <div className="tw-untick qm-warn">{state}</div>;
  const found = state.filter((t) => t.total > 0);
  return (
    <div className="tw-untick">
      {found.length === 0 && <div className="muted small">Still nothing with any one stat unticked: untick two or more.</div>}
      {found.map((t) => (
        <button key={t.key} onClick={() => onPick(t.key)} title="Untick it and search">
          Untick <b>{t.label}</b> <span className="tw-untick-n">{t.total} for sale</span></button>
      ))}
    </div>
  );
}

/** Awakened's "market ratio" row: poe.ninja's price, placed among the offers so you can see who's under or over it. */
function MarketRatio({ c, divine }: { c: number; divine: number | null }) {
  return (
    <li className="tw-ratio-row" title="poe.ninja's price, from real trades on the in-game Currency Exchange">
      <span className="tw-price"><Price n={c} cur="chaos" /></span>
      <span className="small">{divine && c / divine >= 0.01 ? <Other n={c} cur="chaos" divine={divine} /> : null}</span>
      <span className="small">market price (poe.ninja)</span>
      <span />
    </li>
  );
}

/** The answer first, big and clear: what this sells for. */
/** poe.ninja's price over the last 7 days: a small line and the week's change (green up, red down). */
function TrendLine({ t }: { t: { points: number[]; change: number } }) {
  const pts = t.points;
  const lo = Math.min(...pts), hi = Math.max(...pts), span = hi - lo || 1;
  const W = 120, H = 26;
  const xy = pts.map((v, i) => [(i / (pts.length - 1)) * W, H - 3 - ((v - lo) / span) * (H - 6)]);
  const d = xy.map(([x, y], i) => (i ? "L" : "M") + x.toFixed(1) + " " + y.toFixed(1)).join(" ");
  const up = t.change > 0.5, down = t.change < -0.5;
  const word = up ? "up" : down ? "down" : "steady";
  return (
    <div className={`tw-trend ${up ? "up" : down ? "down" : "flat"}`} title={"poe.ninja's price over the last 7 days, day by day"}>
      <span className="tw-trend-label">This week</span>
      <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} aria-hidden="true">
        <path d={d} fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" />
        <circle cx={xy[xy.length - 1][0]} cy={xy[xy.length - 1][1]} r="2.6" fill="currentColor" />
      </svg>
      <span className="tw-trend-pct">{up ? "▲" : down ? "▼" : ""} {Math.abs(t.change).toFixed(0)}% {word}</span>
    </div>
  );
}

function PriceHero({ res, bulk, busy, err, divine, ninja, ninjaNote, waiting, stack, name }: { res: Result | null; bulk: Bulk | null; busy: boolean; err: string; waiting?: string | null; stack?: number; name?: string;
  divine: number | null; ninja: [number, string] | null; ninjaNote: string }) {
  const ninjaChaos = ninja ? (ninja[1] === "divine" && divine ? ninja[0] * divine : ninja[1] === "chaos" ? ninja[0] : null) : null;
  let main: number | null = null, label = "", sub: React.ReactNode = null, source = "";
  if (res?.summary) {
    main = res.summary[1]; label = "on trade now";
    sub = <>cheapest {money(res.summary[0], divine)} · {res.total.toLocaleString()} listed</>;
  } else if (bulk?.offers[0]) {
    // Like Awakened: the headline is poe.ninja's price (from real in-game Currency Exchange trades, which price
    // fixers can't move); our stock-weighted going rate only when poe.ninja doesn't have the item.
    const t = bulkTypical(bulk.offers, divine, ninjaChaos);
    const o = bulk.offers[0];
    main = ninjaChaos ?? t.typical; label = "each";
    source = ninjaChaos != null ? "poe.ninja's price, from real trades on the in-game Currency Exchange" : "Going rate on the trade site's exchange, weighted by stock";
    sub = <>cheapest listed {money(t.cheapest, divine)}{o.stock ? ` (${o.stock} in stock)` : ""}
      {ninjaChaos == null && <> · going rate {money(t.typical, divine)}</>} · {bulk.offers.length} sellers</>;
  } else if (ninjaChaos != null) {
    main = ninjaChaos; label = `poe.ninja${ninjaNote}`;
  }
  // With a stack (e.g. 732 lifeforce): two tiles, "your 732" (bright) and "each". The ratio line ("11,880 Wild
  // Crystallised Lifeforce = 1 [divine]") is for bulk items: how they're traded.
  const many = stack != null && stack > 1 && main != null;
  const big = main != null ? (many ? main * stack! : main) : null;
  const showDiv = big != null && divine && big >= divine;
  // Chaos, or divines once it's worth one, always with the orb picture.
  const amount = (c: number) => (divine && c >= divine ? <Price n={c / divine} cur="divine" /> : <Price n={c} cur="chaos" />);
  const perDiv = bulk && main && divine ? divine / main : null;
  return (
    <div className={`tw-hero ${main == null ? "empty" : ""}`}>
      {main != null && big != null && many ? (
        <div className="tw-tiles2" title={source || undefined}>
          <div className="tw-tile2 hi"><div className="tw-tile2-lab">YOUR {stack!.toLocaleString()}</div><div className="tw-tile2-val">{amount(big)}</div>
            {divine && big < divine && big / divine >= 0.01 && <div className="tw-tile2-sub">= <Price n={big / divine} cur="divine" /></div>}</div>
          <div className="tw-tile2"><div className="tw-tile2-lab">EACH</div><div className="tw-tile2-val">{amount(main)}</div></div>
        </div>
      ) : main != null && big != null ? (
        <div className="tw-hero-main">
          <span className="tw-hero-price" title={source || undefined}>{showDiv ? <CountPrice n={big / divine!} cur="divine" /> : <CountPrice n={big} cur="chaos" />}</span>
          <span className="tw-hero-side">
            <span className="tw-hero-label">{label}</span>
            {divine && <span className="muted">{showDiv ? <>= <Price n={big} cur="chaos" /></> : big / divine >= 0.01 ? <>= <Price n={big / divine} cur="divine" /></> : null}</span>}
          </span>
        </div>
      ) : (
        <div className="tw-hero-main"><span className="tw-hero-label">
          {busy ? <><Spinner /> Checking the trade site…</> : res && res.total === 0 ? "None listed like this: untick a stat or two" : res ? `${res.total} listed, none priced` : err ? "No trade price" : waiting ?? "No price yet"}</span></div>
      )}
      {perDiv != null && name && (
        <div className="tw-perdiv">{perDiv >= 1
          ? <><b>{perDiv >= 10 ? Math.round(perDiv).toLocaleString() : +perDiv.toFixed(1)}</b> {name} = 1 <CurrencyIcon id="divine" /></>
          : <>1 {name} = <b>{+(1 / perDiv).toFixed(1)}</b> <CurrencyIcon id="divine" /></>}</div>
      )}
      <div className="tw-hero-foot muted small">
        {sub}
        {busy && main != null && <span className="tw-hero-busy"> · <Spinner /> checking trade…</span>}
        {res?.summary && ninjaChaos != null && <span> · poe.ninja {money(ninjaChaos, divine)}</span>}
      </div>
    </div>
  );
}

/** "4 better · 1 worse" as a coloured badge. */
function Verdict({ up, down }: { up: number; down: number }) {
  const cls = up > down ? "up" : down > up ? "down" : "";
  return <span className={`tw-verdict ${cls}`}>{up > 0 && `${up} better`}{up > 0 && down > 0 && " · "}{down > 0 && `${down} worse`}{!up && !down && "same stats"}</span>;
}

/** Other mods, one per line; long lists fold after four. */
function ModList({ title, lines, cls, mark }: { title: string; lines: string[]; cls: string; mark: string }) {
  const [all, setAll] = useState(false);
  if (lines.length === 0) return null;
  const shown = all ? lines : lines.slice(0, 4);
  return (
    <div className="tw-cmp-mods">
      <div className={`tw-cmp-mods-title ${cls}`}>{title}</div>
      <ul>{shown.map((l, i) => <li key={i}><span className={cls}>{mark}</span> {l}</li>)}</ul>
      {lines.length > 4 && <button className="link small" onClick={() => setAll(!all)}>{all ? "show fewer" : `+ ${lines.length - 4} more`}</button>}
    </div>
  );
}

/** Ctrl+D on a map: its mods, the ones you can't run in red. */
function MapCheck({ map }: { map: MapInfo }) {
  const bad = map.mods.filter((m) => m.warn);
  const [copied, setCopied] = useState("");
  return (
    <div className="tw-map">
      <div className={`tw-map-head ${bad.length ? "bad" : map.unidentified ? "" : "ok"}`}>
        {map.unidentified ? "Unidentified: its mods are hidden until you identify it."
          : bad.length ? `⚠ ${bad.length} mod${bad.length > 1 ? "s" : ""} you can't run` : "✓ Nothing on your can't-run list"}
        {map.tier != null && <span className="muted small"> · tier {map.tier}</span>}
      </div>
      {map.stats.length > 0 && <div className="muted small tw-map-stats">{map.stats.join(" · ")}</div>}
      <ul className="tw-map-mods">
        {map.mods.map((m, i) => (
          <li key={i} className={m.warn ? "bad" : ""}>{m.text}{m.warn && <span className="tw-map-tag">{m.warn}</span>}</li>
        ))}
      </ul>
      <div className="row">
        <button className="small" onClick={() => invoke<string>("map_regex_copy").then((r) => setCopied(r ? "Copied: paste it in your map stash search" : "Nothing to copy yet: set it up on the Maps page")).catch((e) => setCopied(String(e)))}>
          Copy map regex</button>
        {copied && <span className="muted small">{copied}</span>}
      </div>
      <div className="muted small">Change which mods you can't run on the Maps page: this warning and the stash search use the same list.</div>
    </div>
  );
}

/** The Ctrl+D price window. */
export default function TradeWindow() {
  const [check, setCheck] = useState<Check | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [res, setRes] = useState<Result | null>(null);
  // "Which stat to untick?": the search tried once without each ticked stat (only when you ask).
  const [untick, setUntick] = useState<null | "busy" | string | { key: string; label: string; total: number }[]>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const [bulk, setBulk] = useState<Bulk | null>(null);
  const [pay, setPay] = useState<"chaos" | "divine">("chaos");
  // Awakened's stock filter: only sellers with at least as many as your stack (off by default).
  const [minStock, setMinStock] = useState(false);
  // Likely price fixers are hidden until you ask to see them.
  const [showFixers, setShowFixers] = useState(false);
  const [saved, setSaved] = useState<Saved[]>([]);
  const [showSaved, setShowSaved] = useState(false);
  const [saving, setSaving] = useState<string | null>(null);
  const [preset, setPreset] = useState<number | null>(0);
  // Awakened-style tabs: price this item, or price it as a crafting base. Your own search is kept while you look.
  const [asBase, setAsBase] = useState(false);
  const [ownSetup, setOwnSetup] = useState<Setup | null>(null);
  const [mapMods, setMapMods] = useState(false);
  const [reporting, setReporting] = useState<string | null>(null);
  const [reported, setReported] = useState("");
  const [hasSide, setHasSide] = useState(false);
  const [searched, setSearched] = useState<string | null>(null);
  // Like Awakened: change a filter after a search and the old prices go, until you search again.
  useEffect(() => { if (res && setup && searched && searchKey(setup) !== searched) setRes(null); }, [setup, res, searched]);
  const box = useRef<HTMLDivElement | null>(null);

  const close = useCallback(() => { setCheck(null); setRes(null); setBulk(null); setErr(""); invoke("hide_trade_window"); }, []);

  const runBulk = useCallback(async (want: string, p: "chaos" | "divine", minimum: number | null = null) => {
    setBusy(true); setErr(""); setBulk(null);
    try {
      let b = await invoke<Bulk>("trade_bulk", { want, pay: p, minimum });
      // Pricier items are usually only sold for divines: when chaos finds nobody, try divines.
      if (b.offers.length === 0 && p === "chaos") {
        const d = await invoke<Bulk>("trade_bulk", { want, pay: "divine", minimum }).catch(() => null);
        if (d && d.offers.length) { b = d; setPay("divine"); }
      }
      setBulk(b);
    } catch (e) { setErr(friendly(e)); }
    setBusy(false);
  }, []);

  const run = useCallback(async (s: Setup) => {
    setBusy(true); setErr(""); setRes(null); setUntick(null); setSearched(searchKey(s));
    try { setRes(await invoke<Result>("trade_search", { setup: s })); } catch (e) { setErr(friendly(e)); }
    setBusy(false);
  }, []);

  useEffect(() => {
    const a = listen<Check>("price-check", (e) => {
      const c = e.payload;
      setCheck(c); setSetup(c.setup); setRes(null); setSearched(null); setBulk(null); setErr(c.error ? friendly(c.error) : ""); setCopied(null); setPay("chaos"); setMinStock(false); setShowFixers(false);
      setSaving(null); setShowSaved(!c.setup); setPreset(null); setMapMods(false); setReporting(null); setReported(""); setAsBase(false); setOwnSetup(null); setUntick(null); window.scrollTo(0, 0);
      invoke<Saved[]>("saved_list").then(setSaved).catch(() => {});
      if (c.bulk) {
        // Worth half a divine or more: sellers mostly take divines, so ask for those first.
        const nin = c.ninja ? (c.ninja[1] === "divine" && c.divine ? c.ninja[0] * c.divine : c.ninja[1] === "chaos" ? c.ninja[0] : 0) : 0;
        const p = c.divine && nin >= c.divine * 0.5 ? "divine" : "chaos";
        setPay(p); runBulk(c.bulk, p);
      }
      else if (c.setup && needsTrade(c)) run(c.setup);
    });
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    const onBlur = () => close();
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => { a.then((f) => f()); window.removeEventListener("keydown", onKey); window.removeEventListener("blur", onBlur); };
  }, [close, run, runBulk]);

  // Fit the window to what's shown.
  useEffect(() => {
    if (!box.current) return;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(box.current?.getBoundingClientRect().height ?? 300) + 4;
      // As tall as the screen allows; past that the window scrolls.
      const cap = Math.max(400, (window.screen.availHeight || 900) - 40);
      // Wider when the related-items panel sits beside the prices.
      const w = Math.ceil(box.current?.getBoundingClientRect().width ?? 370) + 10;
      getCurrentWindow().setSize(new LogicalSize(Math.max(380, w), Math.max(100, Math.min(cap, h)))).then(() => invoke("keep_on_screen")).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  const runSaved = (x: Saved) => {
    setCheck({ setup: x.setup, ninja: null, league: check?.league ?? "", from_ground: false, error: null });
    setSetup(x.setup); setBulk(null); setShowSaved(false); run(x.setup);
  };
  const savedBlock = (
    <>
      <button className="link small tw-saved-toggle" onClick={() => setShowSaved(!showSaved)}>
        {showSaved ? "Hide saved searches" : `Saved searches (${saved.length})`}{saved.some((x) => x.live) ? " · live on" : ""}</button>
      {showSaved && <SavedPanel list={saved} setList={setSaved} onRun={runSaved} />}
    </>
  );

  if (!check) return null;
  if (!setup) return <div className="qm tw" ref={box}><div className="qm-fail">{err || "Nothing to price."}</div>{savedBlock}</div>;

  const it = setup.item;
  // Awakened-style side panel: a fragment's set or parts, lifeforce types, what a boss drops (shown when there are any).
  const relName = (it.rarity === "Unique" ? it.name : it.base_type) || null;
  // Maps: priced by tier and rarity; their mods are behind a link.
  const showStats = !check.map || mapMods;
  // Maps: their drop bonuses (quantity, atlas drops, pack size…) always; the map mods behind a link.
  // "As a base": only the rows the base search uses (the rest come back on "This item").
  const shown = (x: StatFilter) => (!asBase || !!setup.base_item?.ids.includes(x.id)) && (showStats || x.kind === "property" || x.kind === "pseudo");
  const mapModCount = setup.filters.filter((x) => !(x.kind === "property" || x.kind === "pseudo")).length;
  const patch = (i: number, p: Partial<StatFilter>) => { if ("min" in p) setPreset(null); return setSetup({ ...setup, filters: setup.filters.map((f, j) => (j === i ? { ...f, ...p } : f)) }); };
  const divine = res?.divine ?? bulk?.divine ?? check?.divine ?? null;
  const stack = it.stack_size ?? 1;
  const ninjaC = check.ninja ? (check.ninja[1] === "divine" && divine ? check.ninja[0] * divine : check.ninja[1] === "chaos" ? check.ninja[0] : null) : null;
  const bt = bulk?.offers.length ? bulkTypical(bulk.offers, bulk.divine, ninjaC) : null;
  const fixers = bulk && bt ? bulk.offers.filter((o) => bt.fixer(o)).length : 0;
  // Where poe.ninja's price sits among the offers (Awakened's "market ratio" row).
  const ratioAt = bulk && ninjaC != null ? (() => { const i = bulk.offers.findIndex((o) => (o.pay_currency === "divine" && bulk.divine ? o.each * bulk.divine : o.each) >= ninjaC); return i === -1 ? bulk.offers.length : i; })() : -1;
  // Item listings: the same seller at the same price several times in a row shows once, with "× N" (like Awakened).
  const collapsed = (res?.listings ?? []).reduce<{ l: Listing; n: number; i: number }[]>((acc, l, i) => {
    const last = acc[acc.length - 1];
    if (last && last.l.account === l.account && last.l.amount === l.amount && last.l.currency === l.currency) last.n += 1; else acc.push({ l, n: 1, i });
    return acc;
  }, []);
  const copy = (text: string, key: string) => { invoke("copy_text", { text }); setCopied(key); };
  // Mouse wheel on a minimum: 1 at a time (Shift: 10).
  const wheel = (i: number, f: StatFilter, e: React.WheelEvent) => {
    const step = (e.shiftKey ? 10 : 1) * (e.deltaY < 0 ? 1 : -1);
    patch(i, { min: Math.max(0, (f.min ?? f.value) + step), enabled: true });
  };

  return (
    <div className="tw-wrap" ref={box}>
    <div className="qm tw">
      <div className="qm-head with-icon">
        <ItemIcon name={it.rarity === "Unique" ? it.name : null} base={it.base_type || null} size={36} />
        <div className="qm-head-text">
          <div className="qm-name" style={{ color: RARITY_COLOUR[it.rarity] ?? "#ddd" }}>{itemTitle(it)}</div>
          <div className="qm-sub">{["Rare", "Magic"].includes(it.rarity) && it.base_type && it.name ? it.base_type : it.item_class}{it.item_level ? ` · ilvl ${it.item_level}` : ""}{setup.links_min ? ` · ${setup.links_min}-link` : ""}</div>
        </div>
      </div>

      <PriceHero res={res} bulk={check.bulk ? bulk : null} busy={busy} err={err} divine={divine}
        waiting={!res && !busy && !check.bulk ? (searched ? "Filters changed: move onto Search again" : "Pick the stats, then move your mouse onto Search") : null}
        ninja={check.ninja} ninjaNote={it.rarity === "Unique" && !it.name ? " (best ground unique on this base)" : ""} stack={it.stack_size ?? undefined} name={it.base_type} />
      {check.trend && <TrendLine t={check.trend} />}
      {err && <div className="qm-warn">{err}</div>}
      {res && res.total === 0 && searched === searchKey(setup) && (setup.filters.some((f) => f.enabled) || (setup.base_only && setup.category) || setup.ilvl_min) && (
        <UntickHelp state={untick} onAsk={() => { setUntick("busy");
            invoke<{ key: string; label: string; total: number }[]>("trade_untick_one", { setup }).then(setUntick).catch((e) => setUntick(String(e))); }}
          onPick={(key) => {
            const s = key === "base" ? { ...setup, base_only: false } : key === "ilvl" ? { ...setup, ilvl_min: null, ilvl_max: null }
              : { ...setup, filters: setup.filters.map((f) => f.id === key ? { ...f, enabled: false } : f) };
            setUntick(null); setSetup(s); run(s); }} />
      )}
      {it.rarity === "Unique" && !it.name && it.base_type && <BaseUniques base={it.base_type} divine={divine} />}

      {check.map && <MapCheck map={check.map} />}
      {check.compare && check.compare.length > 0 && <CompareBlock list={check.compare} />}

      {check.map && mapModCount > 0 && (
        <button className="link small tw-mapmods" onClick={() => setMapMods(!mapMods)}>
          {mapMods ? "▾" : "▸"} Search by map mods too ({mapModCount})</button>
      )}
      {setup.base_item && (() => {
        const b = setup.base_item;
        const pick = (base: boolean) => {
          if (base === asBase) return;
          if (base) {
            setOwnSetup(setup);
            setSetup({ ...setup, base_only: true, ilvl_min: b.ilvl_min, ilvl_max: null, misc: b.misc,
              filters: setup.filters.map((f) => ({ ...f, enabled: b.ids.includes(f.id), min: b.ids.includes(f.id) ? f.value : f.min })) });
          } else if (ownSetup) setSetup(ownSetup);
          setPreset(null); setAsBase(base);
        };
        return (
          <div className="tw-tabs" title={"Awakened PoE Trade's two searches: this item with its stats, or what it's worth as a base to craft on (" + b.why.join(", ") + ")"}>
            <button className={asBase ? "" : "on"} onClick={() => pick(false)}>This item</button>
            <button className={asBase ? "on" : ""} onClick={() => pick(true)}>As a base <span className="tw-tab-why">{b.why.join(" · ")}</span></button>
          </div>
        );
      })()}
      {asBase && (
        <div className="tw-base-sum">
          <b>Searching for:</b> {[setup.base || it.base_type, setup.ilvl_min ? `item level ${setup.ilvl_min}+` : null,
            ...(setup.misc ?? []).map((m) => m.replace("_item", "").replace(/^./, (c) => c.toUpperCase())),
            ...setup.filters.filter((f) => f.enabled).map((f) => f.label)].filter(Boolean).join(" · ")}
          {!setup.filters.some((f) => f.enabled) && <span className="muted"> (no mods: just the base)</span>}
        </div>
      )}
      {setup.filters.some(shown) && (
        <div className="tw-range">
          <span className="tw-sec">Search for</span>
          <span className="tw-range-btns" title="Set every minimum relative to this item's rolls">
          <span className="muted small">min</span>
          {[0, 10, 20].map((p) => (
            <button key={p} className={preset === p ? "on" : ""} title={p ? `Every minimum ${p}% below this item's roll` : "Every minimum at this item's roll"}
              onClick={() => { setPreset(p); setSetup({ ...setup, filters: setup.filters.map((f) => {
                // Open-slot counts and timeless jewel seeds are exact: never lowered.
                if (f.min == null || f.id.includes("empty") || f.id.includes("timeless")) return f;
                if (p === 0 || Math.abs(f.value) <= 3) return { ...f, min: f.value };
                const v = f.value * (1 - p / 100);
                return { ...f, min: Math.abs(f.value) >= 10 ? Math.floor(v) : Math.floor(v * 10) / 10 };
              }) }); }}>
              {p === 0 ? "exact" : `−${p}%`}</button>
          ))}
          </span>
        </div>
      )}
      {setup.filters.some(shown) && (
        <div className="tw-stats">
          {setup.filters.map((f, i) => {
            if (!shown(f)) return null;
            // A small heading where the kind of row changes (rows are already in the game's order), with the full
            // words "Prefixes" / "Suffixes" and how many an item can have, so players learn how mods split.
            const jewel = setup.category === "jewel.base" || setup.category === "jewel.abyss";
            const g = rowGroup(f, setup.affix_max ?? (jewel ? [2, 2] : [3, 3]));
            const prev = setup.filters.slice(0, i).filter(shown).pop();
            const head = !prev || rowGroup(prev, [3, 3])[0] !== g[0];
            return (
              <React.Fragment key={f.id + i}>
                {head && <div className="tw-group"><span>{g[0]}</span>{g[1] && <span className="tw-group-hint">{g[1]}</span>}</div>}
                <label className={`tw-stat ${f.enabled ? "on" : ""} ${f.junk ? "junk" : ""}`} title={f.junk ? "Rarely changes the price" : undefined}>
                  <input type="checkbox" checked={f.enabled} onChange={(e) => patch(i, { enabled: e.target.checked })} />
                  <span className={`tw-label mod-${f.kind}`}>{f.label}{KIND[f.kind] && f.kind !== "pseudo" && f.kind !== "implicit" && <span className="tw-kind">{KIND[f.kind]}</span>}
                    {f.tier != null && <span className={`tw-tier ${tierClass(f.tier)}`} title="Tier 1 is the best this mod can roll">T{f.tier}</span>}
                    {f.roll != null && <Roll r={f.roll} range={f.range} tier={f.tier} />}</span>
                  <input className="tw-min" type="number" value={f.min ?? ""} placeholder="min" onWheel={(e) => wheel(i, f, e)}
                    onChange={(e) => patch(i, { min: e.target.value === "" ? null : +e.target.value, enabled: true })} />
                </label>
              </React.Fragment>
            );
          })}
        </div>
      )}
      {setup.unknown.length > 0 && showStats && !asBase && (
        <div className="tw-unknown">
          {setup.unknown.map((u, i) => {
            const k = setup.unknown_kinds?.[i] ?? "explicit";
            return <div key={i} className={`tw-label mod-${k} small`} title="The trade site has no search for this mod">
              {u}{KIND[k] && <span className="tw-kind">{KIND[k]}</span>}<span className="tw-kind">not searchable</span></div>;
          })}
        </div>
      )}

      {!check.bulk && <div className="tw-opts">
        {setup.category && it.rarity !== "Unique" && (
          <label><input type="checkbox" checked={setup.base_only} onChange={(e) => setSetup({ ...setup, base_only: e.target.checked })} /> {setup.base || it.base_type} only</label>
        )}
        {it.item_level != null && it.rarity !== "Unique" && setup.category && (
          <label title="Bases in the same item-level band: better mod tiers unlock at 75, 82, 84 and 86"><input type="checkbox" checked={setup.ilvl_min != null}
            onChange={(e) => setSetup({ ...setup, ilvl_min: e.target.checked ? it.item_level ?? null : null, ilvl_max: e.target.checked && it.item_level ? ilvlBand(it.item_level)[1] : null })} /> ilvl {ilvlText(it.item_level)}</label>
        )}
        <button className="link small" onClick={() => setSaving(saving == null ? itemTitle(it) : null)} title="Keep this search to run again or watch live">Save search</button>
        <button className="tw-search" disabled={busy} onClick={() => run(setup)}
          title="Move your mouse onto this button to search (or click)"
          onMouseEnter={(e) => {
            // Hover to search, like Awakened, but only when coming from inside the window (not when it opens under the mouse).
            const from = e.relatedTarget as Node | null;
            if (!busy && !res && from && box.current?.contains(from)) run(setup);
          }}>{busy ? <><Spinner /> Searching…</> : searched ? "Search again" : "Search trade"}</button>
      </div>}
      {saving != null && (
        <form className="tw-save" onSubmit={(e) => { e.preventDefault();
          invoke<Saved[]>("saved_add", { name: saving, setup }).then((l) => { setSaved(l); setSaving(null); setShowSaved(true); }).catch((er) => setErr(friendly(er))); }}>
          <input className="text" autoFocus value={saving} onChange={(e) => setSaving(e.target.value)} placeholder="Name this search" />
          <button type="submit">Save</button>
        </form>
      )}

      {check.from_ground && it.rarity !== "Unique" && setup.filters.length === 0 && ["Rare", "Magic"].includes(it.rarity) && (
        <div className="muted small">Read off the ground, so no stats: pick it up and press Ctrl+D in your inventory for a proper price.</div>
      )}
      {check.bulk && (
        <div className="tw-res">
          <div className="tw-sum tw-bulkhead">
            <span>{bulk?.offers[0] && bt ? <>{ninjaC != null ? "Market price" : "Going rate"} <b><Price n={ninjaC ?? bt.typical} cur="chaos" /></b> each
              {" "}<Other n={ninjaC ?? bt.typical} cur="chaos" divine={bulk.divine} /></> : busy ? "checking…" : "no offers"}</span>
            <span className="tw-pay">
              {(["chaos", "divine"] as const).map((p) => (
                <button key={p} className={pay === p ? "on" : ""} onClick={() => { setPay(p); runBulk(check.bulk!, p, minStock ? stack : null); }}>
                  pay in <CurrencyIcon id={p} /></button>
              ))}
            </span>
          </div>
          {stack > 1 && (
            <label className="tw-minstock" title="Like Awakened's stock filter: hides sellers who can't take your whole stack">
              <input type="checkbox" checked={minStock} onChange={(e) => { setMinStock(e.target.checked); runBulk(check.bulk!, pay, e.target.checked ? stack : null); }} />
              Only sellers with at least {stack} in stock
            </label>
          )}
          <ul className="tw-list">
            {bulk?.offers.map((o, i) => {
              // Like Awakened's "fulfill": how many trades of this offer's size the seller can actually fill.
              const fills = o.get > 0 ? Math.floor(o.stock / o.get) : o.stock;
              return (
                <React.Fragment key={i}>
                  {i === ratioAt && ninjaC != null && <MarketRatio c={ninjaC} divine={bulk.divine} />}
                  {(!bt?.fixer(o) || showFixers) && <li className={bt?.fixer(o) ? "fixer" : ""} title={bt?.fixer(o) ? "Much cheaper than the market price with very little stock: often a price fixer" : undefined}>
                    <span className="tw-price"><Price n={o.each} cur={o.pay_currency} /></span>
                    <span className="muted small"><Other n={o.each} cur={o.pay_currency} divine={bulk.divine} />{o.get > 1 ? ` · ${o.pay} for ${o.get}` : ""}{bt?.fixer(o) && <span className="tw-fixer">low stock</span>}</span>
                    <span className="tw-seller muted small" title={`${o.stock} in stock: fills ${fills} trade${fills === 1 ? "" : "s"} of ${o.get}`}>
                      {o.stock < o.get && <span className="tw-warn-ico">⚠ </span>}{o.stock} in stock{o.get > 1 ? ` · fills ${fills}` : ""} · {o.account}</span>
                    {o.whisper && <button className="link small" onClick={() => copy(o.whisper, "b" + i)}>{copied === "b" + i ? "Copied" : "Whisper"}</button>}
                  </li>}
                </React.Fragment>
              );
            })}
            {bulk && ratioAt === bulk.offers.length && ninjaC != null && <MarketRatio c={ninjaC} divine={bulk.divine} />}
          </ul>
          {fixers > 0 && (
            <button className="link small tw-fixers-toggle" onClick={() => setShowFixers(!showFixers)}
              title="Offers far below the market price with only a few in stock: usually there to drag the price down">
              {showFixers ? `Hide the ${fixers} likely price fixer${fixers === 1 ? "" : "s"}` : `${fixers} likely price fixer${fixers === 1 ? "" : "s"} hidden · show`}</button>
          )}
          {bulk && <button className="link small" onClick={() => invoke("open_url", { url: bulk.url })}>Open on the trade site</button>}
        </div>
      )}
      {res && res.listings.length > 0 && (
        <div className="tw-res">
          <div className="tw-sec tw-res-sec">Cheapest listed</div>
          <div className="tw-table">
            <div className="tw-th"><span>Price</span><span>{res.listings.some((l) => l.ilvl) ? "ilvl" : ""}</span><span>Listed</span><span>Seller</span><span /></div>
            {collapsed.map(({ l, n, i }) => (
              <div key={i} className="tw-tr">
                <span className="tw-price"><Price n={l.amount} cur={l.currency} />
                  {l.chaos != null && !["chaos", "divine"].includes(l.currency) && <span className="tw-eq">{money(l.chaos, divine)}</span>}
                  {n > 1 && <span className="tw-times" title={`The same seller lists ${n} of these at this price`}>× {n}</span>}</span>
                <span className="muted">{l.ilvl || ""}</span>
                <span className="muted">{ago(l.indexed)}</span>
                <span className="tw-seller muted">{l.account}</span>
                {l.instant ? <span className="tw-instant" title="Instant buyout: buy it straight from the trade site or the in-game market, no whisper">Instant</span>
                  : l.whisper ? <button className="link small" onClick={() => copy(l.whisper, "l" + i)}>
                  {copied === "l" + i ? "Copied" : "Whisper"}</button> : <span />}
              </div>
            ))}
          </div>
        </div>
      )}
      {res && res.url && <button className="link small tw-open-site" onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>}
      {!check.bulk && savedBlock}
      {reporting == null ? (
        <button className="link small tw-report-toggle" onClick={() => { setReporting(""); setReported(""); }}
          title="Saves this price check with your note, so Claude can see exactly what went wrong">
          {reported ? "✓ Saved: mention it to Claude in chat" : "Something's wrong?"}</button>
      ) : (
        <form className="tw-report" onSubmit={(e) => { e.preventDefault();
          invoke<string>("report_problem", { note: reporting, data: { check, setup, res, bulk, err } })
            .then(() => { setReporting(null); setReported("ok"); }).catch((er) => setErr(friendly(er))); }}>
          <textarea className="text" autoFocus rows={2} value={reporting} onChange={(e) => setReporting(e.target.value)}
            placeholder="What looks wrong? e.g. price too high, wrong mod ticked" />
          <div className="row"><button type="submit">Save for Claude</button>
            <button type="button" className="link small" onClick={() => setReporting(null)}>Cancel</button></div>
        </form>
      )}
    </div>
    {relName && (
      <aside className="qm tw-side" hidden={!hasSide}>
        <RelatedItems name={relName} divine={divine} onData={setHasSide} />
      </aside>
    )}
    </div>
  );
}
