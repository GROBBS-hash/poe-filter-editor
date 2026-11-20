import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";
import BaseUniques from "./BaseUniques";
import RelatedItems from "./RelatedItems";
import { Item, RARITY_COLOUR, itemTitle } from "./types";

type StatFilter = { id: string; label: string; value: number; min: number | null; enabled: boolean; kind: string; tier: number | null; junk?: boolean; roll?: number | null; range?: string | null };
type Setup = { item: Item; filters: StatFilter[]; unknown_kinds?: string[]; base_only: boolean; ilvl_min: number | null; ilvl_max?: number | null; links_min: number | null;
  gem_level: number | null; unknown: string[]; category: string | null; raw?: string };
type MapInfo = { mods: { text: string; warn: string | null }[]; unidentified: boolean; tier: number | null; stats: string[] };
type Check = { setup: Setup | null; ninja: [number, string] | null; league: string; from_ground: boolean; error: string | null; bulk?: string | null; map?: MapInfo | null; divine?: number | null; compare?: Comparison[] };
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

/** The same price in the other currency: chaos -> divine, divine -> chaos. */
function Other({ n, cur, divine }: { n: number; cur: string; divine: number | null }) {
  if (!divine) return null;
  if (cur === "chaos") return n / divine < 0.01 ? null : <span className="tw-other">= <Price n={n / divine} cur="divine" /></span>;
  if (cur === "divine") return <span className="tw-other">= <Price n={n * divine} cur="chaos" /></span>;
  return null;
}

/** Every Ctrl+D searches the trade site straight away (bulk items use the exchange instead). */
const needsTrade = (c: Check) => !!c.setup && !c.bulk;

/** Item-level band where base prices jump (75, 82, 84, 86 unlock better mod tiers): [low, high or null]. */
const ilvlBand = (l: number): [number, number | null] => l <= 67 ? [1, 67] : l <= 74 ? [68, 74] : l <= 81 ? [75, 81] : l <= 83 ? [82, 83] : l <= 85 ? [84, 85] : [86, null];
/** "73–74" or "86+": this item's level up to the top of its band. */
const ilvlText = (l: number) => { const top = ilvlBand(l)[1]; return top == null ? `${l}+` : top === l ? `${l}` : `${l}–${top}`; };

type Saved = { id: number; name: string; live: boolean; setup: Setup };

/** Saved searches: run again, or turn on live (pops up new listings while you play). */
function SavedPanel({ list, setList, onRun }: { list: Saved[]; setList: (l: Saved[]) => void; onRun: (s: Saved) => void }) {
  const [err, setErr] = useState("");
  const call = (cmd: string, args: Record<string, unknown>) => invoke<Saved[]>(cmd, args).then((l) => { setList(l); setErr(""); }).catch((e) => setErr(String(e)));
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

/** The answer first, big and clear: what this sells for. */
function PriceHero({ res, bulk, busy, err, divine, ninja, ninjaNote }: { res: Result | null; bulk: Bulk | null; busy: boolean; err: string;
  divine: number | null; ninja: [number, string] | null; ninjaNote: string }) {
  const ninjaChaos = ninja ? (ninja[1] === "divine" && divine ? ninja[0] * divine : ninja[1] === "chaos" ? ninja[0] : null) : null;
  let main: number | null = null, label = "", sub: React.ReactNode = null;
  if (res?.summary) {
    main = res.summary[1]; label = "on trade now";
    sub = <>cheapest {money(res.summary[0], divine)} · {res.total.toLocaleString()} listed</>;
  } else if (bulk?.offers[0]) {
    const o = bulk.offers[0];
    main = o.pay_currency === "divine" && divine ? o.each * divine : o.each; label = "each, on the exchange";
    sub = <>{bulk.offers.length} sellers</>;
  } else if (ninjaChaos != null) {
    main = ninjaChaos; label = `poe.ninja${ninjaNote}`;
  }
  const showDiv = main != null && divine && main >= divine;
  return (
    <div className={`tw-hero ${main == null ? "empty" : ""}`}>
      {main != null ? (
        <div className="tw-hero-main">
          <span className="tw-hero-price">{showDiv ? <Price n={main / divine!} cur="divine" /> : <Price n={main} cur="chaos" />}</span>
          <span className="tw-hero-side">
            <span className="tw-hero-label">{label}</span>
            {divine && <span className="muted">{showDiv ? <>= <Price n={main} cur="chaos" /></> : main / divine >= 0.01 ? <>= <Price n={main / divine} cur="divine" /></> : null}</span>}
          </span>
        </div>
      ) : (
        <div className="tw-hero-main"><span className="tw-hero-label">
          {busy ? "Checking the trade site…" : res && res.total === 0 ? "None listed like this: untick a stat or two" : res ? `${res.total} listed, none priced` : err ? "No trade price" : "No price yet"}</span></div>
      )}
      <div className="tw-hero-foot muted small">
        {sub}
        {busy && main != null && <span className="tw-hero-busy"> · checking trade…</span>}
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
      <div className="muted small">Change which mods to warn about in Settings → Map check. Build the regex on the Maps page.</div>
    </div>
  );
}

/** The Ctrl+D price window. */
export default function TradeWindow() {
  const [check, setCheck] = useState<Check | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [res, setRes] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [copied, setCopied] = useState<string | null>(null);
  const [bulk, setBulk] = useState<Bulk | null>(null);
  const [pay, setPay] = useState<"chaos" | "divine">("chaos");
  const [ai, setAi] = useState<[number, number, string, number] | null>(null);
  const [saved, setSaved] = useState<Saved[]>([]);
  const [showSaved, setShowSaved] = useState(false);
  const [saving, setSaving] = useState<string | null>(null);
  const [preset, setPreset] = useState<number | null>(0);
  const [mapMods, setMapMods] = useState(false);
  const [reporting, setReporting] = useState<string | null>(null);
  const [reported, setReported] = useState("");
  const box = useRef<HTMLDivElement | null>(null);

  const close = useCallback(() => { setCheck(null); setRes(null); setBulk(null); setAi(null); setErr(""); invoke("hide_trade_window"); }, []);

  const runBulk = useCallback(async (want: string, p: "chaos" | "divine") => {
    setBusy(true); setErr(""); setBulk(null);
    try { setBulk(await invoke<Bulk>("trade_bulk", { want, pay: p })); } catch (e) { setErr(String(e)); }
    setBusy(false);
  }, []);

  const run = useCallback(async (s: Setup) => {
    setBusy(true); setErr(""); setRes(null);
    try { setRes(await invoke<Result>("trade_search", { setup: s })); } catch (e) { setErr(String(e)); }
    setBusy(false);
  }, []);

  useEffect(() => {
    const a = listen<Check>("price-check", (e) => {
      const c = e.payload;
      setCheck(c); setSetup(c.setup); setRes(null); setBulk(null); setAi(null); setErr(c.error ?? ""); setCopied(null); setPay("chaos");
      setSaving(null); setShowSaved(!c.setup); setPreset(null); setMapMods(false); setReporting(null); setReported(""); window.scrollTo(0, 0);
      invoke<Saved[]>("saved_list").then(setSaved).catch(() => {});
      if (c.bulk) runBulk(c.bulk, "chaos");
      else if (c.setup && needsTrade(c)) run(c.setup);
      // A rough second opinion on rares from poeprices.info.
      if (c.setup?.item.rarity === "Rare" && c.setup.raw && !c.setup.item.unidentified)
        invoke<[number, number, string, number] | null>("rare_estimate", { text: c.setup.raw }).then(setAi).catch(() => {});
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
      getCurrentWindow().setSize(new LogicalSize(380, Math.max(100, Math.min(cap, h)))).then(() => invoke("keep_on_screen")).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  const runSaved = (x: Saved) => {
    setCheck({ setup: x.setup, ninja: null, league: check?.league ?? "", from_ground: false, error: null });
    setSetup(x.setup); setBulk(null); setAi(null); setShowSaved(false); run(x.setup);
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
  // Maps: priced by tier and rarity; their mods are behind a link.
  const showStats = !check.map || mapMods;
  // Maps: their drop bonuses (quantity, atlas drops, pack size…) always; the map mods behind a link.
  const shown = (x: StatFilter) => showStats || x.kind === "property" || x.kind === "pseudo";
  const mapModCount = setup.filters.filter((x) => !(x.kind === "property" || x.kind === "pseudo")).length;
  const patch = (i: number, p: Partial<StatFilter>) => { if ("min" in p) setPreset(null); return setSetup({ ...setup, filters: setup.filters.map((f, j) => (j === i ? { ...f, ...p } : f)) }); };
  const divine = res?.divine ?? bulk?.divine ?? check?.divine ?? null;
  const copy = (text: string, key: string) => { invoke("copy_text", { text }); setCopied(key); };
  // Mouse wheel on a minimum: 1 at a time (Shift: 10).
  const wheel = (i: number, f: StatFilter, e: React.WheelEvent) => {
    const step = (e.shiftKey ? 10 : 1) * (e.deltaY < 0 ? 1 : -1);
    patch(i, { min: Math.max(0, (f.min ?? f.value) + step), enabled: true });
  };

  return (
    <div className="qm tw" ref={box}>
      <div className="qm-head with-icon">
        <ItemIcon name={it.rarity === "Unique" ? it.name : null} base={it.base_type || null} size={36} />
        <div className="qm-head-text">
          <div className="qm-name" style={{ color: RARITY_COLOUR[it.rarity] ?? "#ddd" }}>{itemTitle(it)}</div>
          <div className="qm-sub">{it.item_class}{it.item_level ? ` · ilvl ${it.item_level}` : ""}{setup.links_min ? ` · ${setup.links_min}-link` : ""}</div>
        </div>
      </div>

      <PriceHero res={res} bulk={check.bulk ? bulk : null} busy={busy} err={err} divine={divine}
        ninja={check.ninja} ninjaNote={it.rarity === "Unique" && !it.name ? " (best ground unique on this base)" : ""} />
      {err && <div className="qm-warn">{err}</div>}
      {it.rarity === "Unique" && !it.name && it.base_type && <BaseUniques base={it.base_type} divine={divine} />}

      {check.map && <MapCheck map={check.map} />}
      {check.compare && check.compare.length > 0 && <CompareBlock list={check.compare} />}

      {check.map && mapModCount > 0 && (
        <button className="link small tw-mapmods" onClick={() => setMapMods(!mapMods)}>
          {mapMods ? "▾" : "▸"} Search by map mods too ({mapModCount})</button>
      )}
      {setup.filters.some(shown) && (
        <div className="tw-range">
          <span className="tw-sec">Search for</span>
          <span className="tw-range-btns" title="Set every minimum relative to this item's rolls">
          <span className="muted small">min</span>
          {[0, 10, 20].map((p) => (
            <button key={p} className={preset === p ? "on" : ""} title={p ? `Every minimum ${p}% below this item's roll` : "Every minimum at this item's roll"}
              onClick={() => { setPreset(p); setSetup({ ...setup, filters: setup.filters.map((f) => {
                if (f.min == null || f.id.includes("empty")) return f;
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
          {setup.filters.map((f, i) => !shown(f) ? null : (
            <label key={f.id + i} className={`tw-stat ${f.enabled ? "on" : ""} ${f.junk ? "junk" : ""}`} title={f.junk ? "Rarely changes the price" : undefined}>
              <input type="checkbox" checked={f.enabled} onChange={(e) => patch(i, { enabled: e.target.checked })} />
              <span className={`tw-label mod-${f.kind}`}>{f.label}{KIND[f.kind] && <span className="tw-kind">{KIND[f.kind]}</span>}
                {f.tier != null && <span className={`tw-tier ${tierClass(f.tier)}`} title="Tier 1 is the best this mod can roll">T{f.tier}</span>}
                {f.roll != null && <Roll r={f.roll} range={f.range} tier={f.tier} />}</span>
              <input className="tw-min" type="number" value={f.min ?? ""} placeholder="min" onWheel={(e) => wheel(i, f, e)}
                onChange={(e) => patch(i, { min: e.target.value === "" ? null : +e.target.value, enabled: true })} />
            </label>
          ))}
        </div>
      )}
      {setup.filters.some((f) => f.kind === "fractured") && it.rarity !== "Unique" && setup.category && (
        <div className="tw-fract" title="Prices it as a crafting base: same base, same item-level band, only the fractured mods">
          <span className="tw-fract-tag">Fractured</span>
          <button onClick={() => { const band = it.item_level ? ilvlBand(it.item_level) : null;
              const s = { ...setup, base_only: true, ilvl_min: it.item_level ?? null, ilvl_max: band ? band[1] : null,
                filters: setup.filters.map((f) => ({ ...f, enabled: f.kind === "fractured", min: f.kind === "fractured" ? f.value : f.min })) };
              setPreset(null); setSetup(s); run(s); }}>
            Price as base{it.item_level ? ` · ilvl ${ilvlText(it.item_level)}` : ""}</button>
        </div>
      )}
      {setup.unknown.length > 0 && showStats && (
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
          <label><input type="checkbox" checked={setup.base_only} onChange={(e) => setSetup({ ...setup, base_only: e.target.checked })} /> {it.base_type} only</label>
        )}
        {it.item_level != null && it.rarity !== "Unique" && setup.category && (
          <label title="Bases in the same item-level band: better mod tiers unlock at 75, 82, 84 and 86"><input type="checkbox" checked={setup.ilvl_min != null}
            onChange={(e) => setSetup({ ...setup, ilvl_min: e.target.checked ? it.item_level ?? null : null, ilvl_max: e.target.checked && it.item_level ? ilvlBand(it.item_level)[1] : null })} /> ilvl {ilvlText(it.item_level)}</label>
        )}
        <button className="link small" onClick={() => setSaving(saving == null ? itemTitle(it) : null)} title="Keep this search to run again or watch live">Save search</button>
        <button className="tw-search" disabled={busy} onClick={() => run(setup)}>{busy ? "Searching…" : res ? "Search again" : "Search trade"}</button>
      </div>}
      {saving != null && (
        <form className="tw-save" onSubmit={(e) => { e.preventDefault();
          invoke<Saved[]>("saved_add", { name: saving, setup }).then((l) => { setSaved(l); setSaving(null); setShowSaved(true); }).catch((er) => setErr(String(er))); }}>
          <input className="text" autoFocus value={saving} onChange={(e) => setSaving(e.target.value)} placeholder="Name this search" />
          <button type="submit">Save</button>
        </form>
      )}

      {check.from_ground && it.rarity !== "Unique" && setup.filters.length === 0 && ["Rare", "Magic"].includes(it.rarity) && (
        <div className="muted small">Read off the ground, so no stats: pick it up and press Ctrl+D in your inventory for a proper price.</div>
      )}
      {ai && (
        <div className="tw-ai" title="poeprices.info's machine-learning estimate from the item's mods">
          AI estimate <b>{+ai[0].toFixed(1)}–{+ai[1].toFixed(1)} {UNIT[ai[2]] ?? ai[2]}</b>
          <span className="muted small"> · {Math.round(ai[3])}% sure · poeprices.info</span></div>
      )}
      {check.bulk && (
        <div className="tw-res">
          <div className="tw-sum tw-bulkhead">
            <span>{bulk?.offers[0] ? <>From <b><Price n={bulk.offers[0].each} cur={bulk.offers[0].pay_currency} /></b> each
              {" "}<Other n={bulk.offers[0].each} cur={bulk.offers[0].pay_currency} divine={bulk.divine} /></> : busy ? "checking…" : "no offers"}</span>
            <span className="tw-pay">
              {(["chaos", "divine"] as const).map((p) => (
                <button key={p} className={pay === p ? "on" : ""} onClick={() => { setPay(p); runBulk(check.bulk!, p); }}>
                  pay in <CurrencyIcon id={p} /></button>
              ))}
            </span>
          </div>
          {bulk?.offers[0] && bulk.divine && (() => {
            // How many you get for one divine / one chaos: the ratio people trade at.
            const eachChaos = bulk.offers[0].pay_currency === "divine" ? bulk.offers[0].each * bulk.divine : bulk.offers[0].each;
            const perDiv = bulk.divine / eachChaos;
            return <div className="tw-ratio muted small">
              1 <CurrencyIcon id="divine" /> ≈ <b>{perDiv >= 10 ? Math.round(perDiv) : +perDiv.toFixed(1)}</b> {it.base_type}
              {eachChaos < 1 && <> · 1 <CurrencyIcon id="chaos" /> ≈ <b>{+(1 / eachChaos).toFixed(1)}</b></>}
              {" "}· 1 <CurrencyIcon id="divine" /> = <Price n={bulk.divine} cur="chaos" />
            </div>;
          })()}
          <ul className="tw-list">
            {bulk?.offers.map((o, i) => (
              <li key={i}>
                <span className="tw-price"><Price n={o.each} cur={o.pay_currency} /></span>
                <span className="muted small"><Other n={o.each} cur={o.pay_currency} divine={bulk.divine} />{o.get > 1 ? ` · ${o.pay} for ${o.get}` : ""}</span>
                <span className="tw-seller muted small">{o.stock} in stock · {o.account}</span>
                {o.whisper && <button className="link small" onClick={() => copy(o.whisper, "b" + i)}>{copied === "b" + i ? "Copied" : "Whisper"}</button>}
              </li>
            ))}
          </ul>
          {bulk && <button className="link small" onClick={() => invoke("open_url", { url: bulk.url })}>Open on the trade site</button>}
        </div>
      )}
      {res && res.listings.length > 0 && (
        <div className="tw-res">
          <div className="tw-sec tw-res-sec">Cheapest listed</div>
          <div className="tw-table">
            <div className="tw-th"><span>Price</span><span>{res.listings.some((l) => l.ilvl) ? "ilvl" : ""}</span><span>Listed</span><span>Seller</span><span /></div>
            {res.listings.map((l, i) => (
              <div key={i} className="tw-tr">
                <span className="tw-price"><Price n={l.amount} cur={l.currency} />
                  {l.chaos != null && !["chaos", "divine"].includes(l.currency) && <span className="tw-eq">{money(l.chaos, divine)}</span>}</span>
                <span className="muted">{l.ilvl || ""}</span>
                <span className="muted">{ago(l.indexed)}</span>
                <span className="tw-seller muted">{l.account}</span>
                {l.instant ? <span className="tw-instant" title="Instant buyout: buy it straight from the trade site or the in-game market, no whisper">Instant</span>
                  : l.whisper ? <button className="link small" onClick={() => copy(l.whisper, "l" + i)}>
                  {copied === "l" + i ? "Copied" : "Whisper"}</button> : <span />}
              </div>
            ))}
          </div>
          <button className="link small" onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>
        </div>
      )}
      {/* Awakened-style: a fragment's set / parts and what the boss drops. Below the prices: extra info. */}
      {(it.rarity === "Unique" ? it.name : it.base_type) && <RelatedItems name={(it.rarity === "Unique" ? it.name : it.base_type)!} divine={divine} />}
      {!check.bulk && savedBlock}
      {reporting == null ? (
        <button className="link small tw-report-toggle" onClick={() => { setReporting(""); setReported(""); }}
          title="Saves this price check with your note, so Claude can see exactly what went wrong">
          {reported ? "✓ Saved: mention it to Claude in chat" : "Something's wrong?"}</button>
      ) : (
        <form className="tw-report" onSubmit={(e) => { e.preventDefault();
          invoke<string>("report_problem", { note: reporting, data: { check, setup, res, bulk, err, ai } })
            .then(() => { setReporting(null); setReported("ok"); }).catch((er) => setErr(String(er))); }}>
          <textarea className="text" autoFocus rows={2} value={reporting} onChange={(e) => setReporting(e.target.value)}
            placeholder="What looks wrong? e.g. price too high, wrong mod ticked" />
          <div className="row"><button type="submit">Save for Claude</button>
            <button type="button" className="link small" onClick={() => setReporting(null)}>Cancel</button></div>
        </form>
      )}
    </div>
  );
}
