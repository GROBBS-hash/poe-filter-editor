import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import React, { useCallback, useEffect, useRef, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";
import { Item, RARITY_COLOUR, itemTitle } from "./types";

type StatFilter = { id: string; label: string; value: number; min: number | null; enabled: boolean; kind: string; tier: number | null; junk?: boolean };
type Setup = { item: Item; filters: StatFilter[]; unknown_kinds?: string[]; base_only: boolean; ilvl_min: number | null; links_min: number | null;
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

/** Whether the trade site is worth asking: poe.ninja already prices currency, cards, etc. */
const needsTrade = (c: Check) => !!c.setup && !c.bulk && (!c.ninja || ["Rare", "Magic", "Normal", "Gem"].includes(c.setup.item.rarity)
  || (c.setup.item.rarity === "Unique" && !c.setup.item.name));

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
  return (
    <div className="tw-cmp">
      <button className="link small tw-cmp-toggle" onClick={toggle}>{open ? "▾" : "▸"} Compared with your build (PoB)</button>
      {open && list.map((c) => {
        const up = c.rows.filter((r) => r.this > r.now).length, down = c.rows.filter((r) => r.this < r.now).length;
        return (
          <div key={c.slot} className="tw-cmp-slot">
            <div className="small"><b>{c.slot}</b> <span className="muted">· now: {c.current}</span>
              <span className={`tw-cmp-sum ${up > down ? "up" : down > up ? "down" : ""}`}>{up} better · {down} worse</span></div>
            {c.rows.length > 0 && (
              <table className="tw-cmp-rows"><tbody>
                {c.rows.map((r) => {
                  const d = r.this - r.now;
                  return <tr key={r.stat}><td>{r.stat}</td><td className="muted">{r.now} → {r.this}</td>
                    <td className={d > 0 ? "up" : "down"}>{d > 0 ? "+" : ""}{d}</td></tr>;
                })}
              </tbody></table>
            )}
            {c.gain.length > 0 && <div className="small up">+ Only on this: {c.gain.join(" · ")}</div>}
            {c.lose.length > 0 && <div className="small down">− You'd lose: {c.lose.join(" · ")}</div>}
          </div>
        );
      })}
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
      setSaving(null); setShowSaved(!c.setup);
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
      getCurrentWindow().setSize(new LogicalSize(380, Math.max(100, Math.min(900, h)))).then(() => invoke("keep_on_screen")).catch(() => {});
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
  const patch = (i: number, p: Partial<StatFilter>) => setSetup({ ...setup, filters: setup.filters.map((f, j) => (j === i ? { ...f, ...p } : f)) });
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

      {check.ninja && (
        <div className="tw-ninja">≈ <b><Price n={check.ninja[0]} cur={check.ninja[1]} /></b> <Other n={check.ninja[0]} cur={check.ninja[1]} divine={divine} />
          <span className="muted small"> poe.ninja{it.rarity === "Unique" && !it.name ? " (best unique on this base)" : ""}</span></div>
      )}

      {check.map && <MapCheck map={check.map} />}
      {check.compare && check.compare.length > 0 && <CompareBlock list={check.compare} />}

      {setup.filters.length > 0 && !check.map && (
        <div className="tw-range">
          <span className="muted small">Minimums:</span>
          {[0, 10, 20].map((p) => (
            <button key={p} className="link small" title={p ? `Every minimum ${p}% below this item's roll` : "Every minimum at this item's roll"}
              onClick={() => setSetup({ ...setup, filters: setup.filters.map((f) => {
                if (f.min == null || f.id.includes("empty")) return f;
                if (p === 0 || Math.abs(f.value) <= 3) return { ...f, min: f.value };
                const v = f.value * (1 - p / 100);
                return { ...f, min: Math.abs(f.value) >= 10 ? Math.floor(v) : Math.floor(v * 10) / 10 };
              }) })}>
              {p === 0 ? "exact" : `−${p}%`}</button>
          ))}
        </div>
      )}
      {setup.filters.length > 0 && !check.map && (
        <div className="tw-stats">
          {setup.filters.map((f, i) => (
            <label key={f.id + i} className={`tw-stat ${f.enabled ? "on" : ""} ${f.junk ? "junk" : ""}`} title={f.junk ? "Rarely changes the price" : undefined}>
              <input type="checkbox" checked={f.enabled} onChange={(e) => patch(i, { enabled: e.target.checked })} />
              <span className={`tw-label mod-${f.kind}`}>{f.label}{KIND[f.kind] && <span className="tw-kind">{KIND[f.kind]}</span>}
                {f.tier != null && <span className={`tw-tier ${f.tier <= 2 ? "good" : ""}`}>T{f.tier}</span>}</span>
              <input className="tw-min" type="number" value={f.min ?? ""} placeholder="min" onWheel={(e) => wheel(i, f, e)}
                onChange={(e) => patch(i, { min: e.target.value === "" ? null : +e.target.value, enabled: true })} />
            </label>
          ))}
        </div>
      )}
      {setup.filters.some((f) => f.kind === "fractured") && it.rarity !== "Unique" && setup.category && (
        <button className="link small tw-fract" title="Search for the same base with these fractured mods, ignoring the rest (crafting bases)"
          onClick={() => { const s = { ...setup, base_only: true, filters: setup.filters.map((f) => ({ ...f, enabled: f.kind === "fractured" })) }; setSetup(s); run(s); }}>
          Search {it.base_type} with just its fractured mods</button>
      )}
      {setup.unknown.length > 0 && (
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
          <label><input type="checkbox" checked={setup.ilvl_min != null} onChange={(e) => setSetup({ ...setup, ilvl_min: e.target.checked ? it.item_level ?? null : null })} /> ilvl {it.item_level}+</label>
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
      {err && <div className="qm-warn">{err}</div>}

      {res && (
        <div className="tw-res">
          <div className="tw-sum">
            {res.summary ? <>Around <b><Price n={res.summary[1]} cur="chaos" /></b> <Other n={res.summary[1]} cur="chaos" divine={divine} />
              <span className="muted small"> · cheapest {money(res.summary[0], divine)} · {res.total} listed</span></>
              : res.total === 0 ? <>None listed like this. Untick a stat or two.</> : <>{res.total} listed</>}
          </div>
          <ul className="tw-list">
            {res.listings.map((l, i) => (
              <li key={i}>
                <span className="tw-price"><Price n={l.amount} cur={l.currency} /></span>
                <span className="muted small"><Other n={l.amount} cur={l.currency} divine={divine} />{l.ilvl ? ` · ilvl ${l.ilvl}` : ""} · {ago(l.indexed)}</span>
                <span className="tw-seller muted small">{l.account}</span>
                {l.instant ? <span className="tw-instant" title="Instant buyout: buy it straight from the trade site or the in-game market, no whisper">Instant</span>
                  : l.whisper && <button className="link small" onClick={() => copy(l.whisper, "l" + i)}>
                  {copied === "l" + i ? "Copied" : "Whisper"}</button>}
              </li>
            ))}
          </ul>
          <button className="link small" onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>
        </div>
      )}
      {!check.bulk && savedBlock}
    </div>
  );
}
