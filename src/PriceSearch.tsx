/** Price search window (`pricesearch`, Alt+F by default; 26 Nov 2026, GROBBS: "press a key, type an item name and see
 *  its price, like poe.ninja but easier". He picked mock-up B: the list on the left, the chosen item big on the right
 *  with every version). Searches the hourly price file on this PC, so typing never waits on the internet. */
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ItemIcon } from "./icons";
import "./price-search.css";

type Version = { label: string; value: number; listings: number | null; sold: number | null; change: number | null; trend: number[] | null };
type Entry = { name: string; base: string; cat: string; kind: string; versions: Version[]; main: number; ninja: string; trade: string };
type Result = { poe2: boolean; league: string; unit: string; per_div: number; updated: string; results: Entry[] };

const num = (v: number) => v >= 100 ? Math.round(v).toLocaleString("en-US") : v >= 10 ? v.toFixed(1).replace(/\.0$/, "") : v.toFixed(2).replace(/0$/, "").replace(/\.0$/, "");
const short = (v: number) => v >= 1e6 ? `${(v / 1e6).toFixed(1).replace(/\.0$/, "")}m` : v >= 1e4 ? `${(v / 1e3).toFixed(1).replace(/\.0$/, "")}k` : num(v);

/** Name colour like the game's: uniques brown, cards light blue, gems teal, the rest pale currency yellow. */
function tone(cat: string) {
  if (cat.startsWith("Unique")) return "ps-u";
  if (cat === "DivinationCard") return "ps-dv";
  if (cat.includes("Gem")) return "ps-gm";
  return "ps-c";
}

function ago(iso: string) {
  const t = Date.parse(iso);
  if (!t) return "";
  const m = Math.max(0, Math.round((Date.now() - t) / 60000));
  return m < 60 ? `prices from ${m} min ago` : m < 48 * 60 ? `prices from ${Math.round(m / 60)} h ago` : `prices from ${Math.round(m / 1440)} days ago`;
}

function Change({ v }: { v: number | null }) {
  if (v == null || Math.abs(v) < 0.5) return null;
  return <span className={v > 0 ? "ps-up" : "ps-dn"}>{v > 0 ? "▲" : "▼"} {Math.abs(Math.round(v))}%</span>;
}

/** The week's line: each day's % change from a week ago. */
function Spark({ points, change }: { points: number[]; change: number | null }) {
  if (points.length < 2) return null;
  const lo = Math.min(...points), hi = Math.max(...points), span = hi - lo || 1;
  const W = 300, H = 44;
  const xy = points.map((p, i) => `${(i / (points.length - 1)) * W},${H - 4 - ((p - lo) / span) * (H - 8)}`).join(" ");
  return <svg className="ps-spark" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none"><polyline points={xy} fill="none" stroke={(change ?? 0) >= 0 ? "var(--green)" : "var(--red)"} strokeWidth="2" vectorEffect="non-scaling-stroke" /></svg>;
}

export default function PriceSearch() {
  const [q, setQ] = useState("");
  const [r, setR] = useState<Result | null>(null);
  const [sel, setSel] = useState(0);
  const [ver, setVer] = useState<number | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const seq = useRef(0);
  const listRef = useRef<HTMLDivElement>(null);

  const run = (text: string) => {
    const n = ++seq.current;
    invoke<Result>("price_search", { q: text }).then((res) => { if (n === seq.current) { setR(res); setSel(0); setVer(null); } }).catch(() => {});
  };
  useEffect(() => {
    run("");
    // Each time the key brings it back: ready to type over the last search, with fresh prices.
    const un = listen("pricesearch-shown", () => {
      setTimeout(() => { input.current?.focus(); input.current?.select(); }, 30);
      run(input.current?.value ?? "");
    });
    input.current?.focus();
    return () => { un.then((f) => f()); };
  }, []);

  const list = r?.results ?? [];
  const cur = list[sel];
  const vi = cur ? (ver ?? cur.main) : 0;
  const v = cur?.versions[vi];
  const unit = r?.unit ?? "c";
  const div = (x: number) => r && r.per_div > 0 && r.unit !== "div" ? x / r.per_div : null;
  const hide = (back: boolean) => invoke("price_search_hide", { backToGame: back }).catch(() => {});
  const open = (url: string) => { invoke("open_url", { url }).catch(() => {}); hide(false); };

  useEffect(() => { listRef.current?.querySelector(".ps-row.sel")?.scrollIntoView({ block: "nearest" }); }, [sel]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); hide(true); }
    else if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(s + 1, list.length - 1)); setVer(null); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(s - 1, 0)); setVer(null); }
    else if (cur && cur.versions.length > 1 && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
      // Left/right pick a version when the item has versions (the text cursor stays put).
      e.preventDefault();
      setVer(Math.max(0, Math.min(cur.versions.length - 1, vi + (e.key === "ArrowRight" ? 1 : -1))));
    }
    else if (e.key === "Enter" && cur) { e.preventDefault(); open(e.ctrlKey ? cur.ninja : cur.trade); }
  };

  const range = (e: Entry) => {
    const vals = e.versions.map((x) => x.value);
    const lo = Math.min(...vals), hi = Math.max(...vals);
    return { lo, hi, one: e.versions.length === 1 || hi <= lo * 1.05 };
  };

  return (
    <div className="ps" onKeyDown={onKey}>
      <div className="ps-search" data-tauri-drag-region>
        <span className="ps-glass" data-tauri-drag-region>🔍</span>
        <input ref={input} value={q} spellCheck={false} placeholder="Type an item name…"
          onChange={(e) => { setQ(e.target.value); run(e.target.value); }} />
        {r?.league && <span className="ps-league" data-tauri-drag-region>{r.league} · PoE {r.poe2 ? 2 : 1}</span>}
        <button className="ps-key" title="Close (Esc)" onClick={() => hide(true)}>Esc</button>
      </div>
      <div className="ps-split">
        <div className="ps-list" ref={listRef}>
          {!r?.updated && r && <div className="ps-empty">No price file yet. It comes with the next hourly price download.</div>}
          {r?.updated && q.trim() === "" && <div className="ps-empty">Type a few letters of any item: currency, uniques, cards, gems, fragments, essences…</div>}
          {r?.updated && q.trim() !== "" && list.length === 0 && <div className="ps-empty">Nothing called “{q.trim()}” in today's prices.</div>}
          {list.map((e, i) => {
            const { lo, hi, one } = range(e);
            const m = e.versions[e.main];
            const n = e.versions.length;
            return (
              <div key={e.name + e.base + e.cat} className={`ps-row ${i === sel ? "sel" : ""}`} onMouseDown={(ev) => ev.preventDefault()}
                onClick={() => { setSel(i); setVer(null); }} onDoubleClick={() => open(e.trade)}>
                <ItemIcon name={e.name} base={e.base} size={36} />
                <div className="ps-nm">
                  <div className={`ps-n ${tone(e.cat)}`}>{e.name}</div>
                  <div className="ps-k">{e.kind}{e.base !== e.name ? ` · ${e.base}` : ""}{n > 1 ? ` · ${n} versions` : ""}</div>
                </div>
                <div className="ps-vol">{m.sold != null ? `${m.sold.toLocaleString("en-US")} sold` : m.listings != null ? `${m.listings.toLocaleString("en-US")} listed` : ""}<br /><Change v={m.change} /></div>
                <div className="ps-pr">
                  <div className="ps-a">{one ? `${num(m.value)}${unit}` : `${short(lo)} – ${short(hi)} ${unit}`}</div>
                  {div(m.value) != null && <div className="ps-b">{one ? `${num(div(m.value)!)} div` : `${num(div(lo)!)} – ${num(div(hi)!)} div`}</div>}
                </div>
              </div>
            );
          })}
        </div>
        <div className="ps-detail">
          {cur && v ? <>
            <div className="ps-big"><ItemIcon name={cur.name} base={cur.base} size={64} />
              <div><h3 className={tone(cur.cat)}>{cur.name}</h3><div className="ps-k">{cur.base !== cur.name ? `${cur.base} · ` : ""}{cur.kind}</div></div></div>
            <div className="ps-price">{num(v.value)}{unit}{div(v.value) != null && <small> = {num(div(v.value)!)} div</small>}</div>
            <div className="ps-k">{[v.label, v.sold != null ? `${v.sold.toLocaleString("en-US")} sold lately` : v.listings != null ? `${v.listings.toLocaleString("en-US")} listed` : ""].filter(Boolean).join(" · ")}
              {v.change != null && Math.abs(v.change) >= 0.5 && <> · <Change v={v.change} /> this week</>}</div>
            {v.trend && <Spark points={v.trend} change={v.change} />}
            {cur.versions.length > 1 && <div className="ps-vers">
              {cur.versions.map((x, i) => (
                <button key={i} className={i === vi ? "on" : ""} onMouseDown={(ev) => ev.preventDefault()} onClick={() => setVer(i)}>
                  <span>{x.label || "Plain"}</span><span>{num(x.value)}{unit}{div(x.value) != null ? ` · ${num(div(x.value)!)} div` : ""}{x.listings != null ? ` · ${x.listings} listed` : ""}</span>
                </button>))}
            </div>}
            {cur.cat === "SkillGem" && <div className="ps-note">Gems: this is the price of the cheapest version (level and quality aren't in the price file).</div>}
            {(v.listings ?? 99) < 10 && v.sold == null && <div className="ps-note">Only a few listed, so this price is a rough guess.</div>}
            <div className="ps-btns">
              <button className="primary" onMouseDown={(ev) => ev.preventDefault()} onClick={() => open(cur.trade)}>Open on trade</button>
              <button onMouseDown={(ev) => ev.preventDefault()} onClick={() => open(cur.ninja)}>poe.ninja</button>
            </div>
          </> : <div className="ps-empty">Prices come from poe.ninja.</div>}
        </div>
      </div>
      <div className="ps-foot" data-tauri-drag-region title="Drag here to move the window"><span>↑↓ choose</span>{cur && cur.versions.length > 1 && <span>←→ version</span>}<span>Enter: trade site</span><span>Ctrl+Enter: poe.ninja</span>
        <span className="ps-ago">{r?.updated ? ago(r.updated) : ""}</span></div>
    </div>
  );
}
