import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useCallback, useEffect, useRef, useState } from "react";
import { ItemIcon } from "./icons";
import { Item, RARITY_COLOUR, itemTitle } from "./types";

type StatFilter = { id: string; label: string; value: number; min: number | null; enabled: boolean; kind: string; tier: number | null };
type Setup = { item: Item; filters: StatFilter[]; base_only: boolean; ilvl_min: number | null; links_min: number | null;
  gem_level: number | null; unknown: string[]; category: string | null };
type Check = { setup: Setup | null; ninja: [number, string] | null; league: string; from_ground: boolean; error: string | null };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; ilvl: number | null };
type Result = { listings: Listing[]; summary: [number, number] | null; total: number; url: string; divine: number | null };

const UNIT: Record<string, string> = { chaos: "c", exalted: "ex", divine: "div" };
const KIND: Record<string, string> = { pseudo: "total", implicit: "implicit", crafted: "crafted", enchant: "enchant", fractured: "fractured" };

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
  return `${c < 10 ? +c.toFixed(1) : Math.round(c)}c`;
}

/** Whether the trade site is worth asking: poe.ninja already prices currency, cards, etc. */
const needsTrade = (c: Check) => !!c.setup && (!c.ninja || ["Rare", "Magic", "Normal", "Gem"].includes(c.setup.item.rarity)
  || (c.setup.item.rarity === "Unique" && !c.setup.item.name));

/** The Ctrl+D price window. */
export default function TradeWindow() {
  const [check, setCheck] = useState<Check | null>(null);
  const [setup, setSetup] = useState<Setup | null>(null);
  const [res, setRes] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [copied, setCopied] = useState<number | null>(null);
  const box = useRef<HTMLDivElement | null>(null);

  const close = useCallback(() => { setCheck(null); setRes(null); setErr(""); invoke("hide_trade_window"); }, []);

  const run = useCallback(async (s: Setup) => {
    setBusy(true); setErr(""); setRes(null);
    try { setRes(await invoke<Result>("trade_search", { setup: s })); } catch (e) { setErr(String(e)); }
    setBusy(false);
  }, []);

  useEffect(() => {
    const a = listen<Check>("price-check", (e) => {
      setCheck(e.payload); setSetup(e.payload.setup); setRes(null); setErr(e.payload.error ?? ""); setCopied(null);
      if (e.payload.setup && needsTrade(e.payload)) run(e.payload.setup);
    });
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") close(); };
    const onBlur = () => close();
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => { a.then((f) => f()); window.removeEventListener("keydown", onKey); window.removeEventListener("blur", onBlur); };
  }, [close, run]);

  // Fit the window to what's shown.
  useEffect(() => {
    if (!box.current) return;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(box.current?.getBoundingClientRect().height ?? 300) + 4;
      getCurrentWindow().setSize(new LogicalSize(380, Math.max(100, Math.min(900, h)))).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  if (!check) return null;
  if (!setup) return <div className="qm tw" ref={box}><div className="qm-fail">{err || "Nothing to price."}</div></div>;

  const it = setup.item;
  const patch = (i: number, p: Partial<StatFilter>) => setSetup({ ...setup, filters: setup.filters.map((f, j) => (j === i ? { ...f, ...p } : f)) });
  const divine = res?.divine ?? null;

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
        <div className="tw-ninja">≈ <b>{money(check.ninja[0], divine)}</b>{check.ninja[1] !== "chaos" ? ` ${UNIT[check.ninja[1]] ?? check.ninja[1]}` : ""}
          <span className="muted small"> poe.ninja{it.rarity === "Unique" && !it.name ? " (best unique on this base)" : ""}</span></div>
      )}

      {setup.filters.length > 0 && (
        <div className="tw-stats">
          {setup.filters.map((f, i) => (
            <label key={f.id + i} className={`tw-stat ${f.enabled ? "on" : ""}`}>
              <input type="checkbox" checked={f.enabled} onChange={(e) => patch(i, { enabled: e.target.checked })} />
              <span className="tw-label">{f.label}{KIND[f.kind] && <span className="tw-kind">{KIND[f.kind]}</span>}
                {f.tier != null && <span className={`tw-tier ${f.tier <= 2 ? "good" : ""}`}>T{f.tier}</span>}</span>
              <input className="tw-min" type="number" value={f.min ?? ""} placeholder="min"
                onChange={(e) => patch(i, { min: e.target.value === "" ? null : +e.target.value, enabled: true })} />
            </label>
          ))}
        </div>
      )}
      {setup.unknown.length > 0 && <div className="muted small tw-unknown" title={setup.unknown.join("\n")}>{setup.unknown.length} mod{setup.unknown.length > 1 ? "s" : ""} not searchable</div>}

      <div className="tw-opts">
        {setup.category && it.rarity !== "Unique" && (
          <label><input type="checkbox" checked={setup.base_only} onChange={(e) => setSetup({ ...setup, base_only: e.target.checked })} /> {it.base_type} only</label>
        )}
        {it.item_level != null && it.rarity !== "Unique" && setup.category && (
          <label><input type="checkbox" checked={setup.ilvl_min != null} onChange={(e) => setSetup({ ...setup, ilvl_min: e.target.checked ? it.item_level ?? null : null })} /> ilvl {it.item_level}+</label>
        )}
        <button className="tw-search" disabled={busy} onClick={() => run(setup)}>{busy ? "Searching…" : res ? "Search again" : "Search trade"}</button>
      </div>

      {check.from_ground && it.rarity !== "Unique" && setup.filters.length === 0 && ["Rare", "Magic"].includes(it.rarity) && (
        <div className="muted small">Read off the ground, so no stats: pick it up and press Ctrl+D in your inventory for a proper price.</div>
      )}
      {err && <div className="qm-warn">{err}</div>}

      {res && (
        <div className="tw-res">
          <div className="tw-sum">
            {res.summary ? <>Around <b>{money(res.summary[1], divine)}</b> <span className="muted small">· cheapest {money(res.summary[0], divine)} · {res.total} listed</span></>
              : res.total === 0 ? <>None listed like this. Untick a stat or two.</> : <>{res.total} listed</>}
          </div>
          <ul className="tw-list">
            {res.listings.map((l, i) => (
              <li key={i}>
                <span className="tw-price">{+l.amount.toFixed(2)} {l.currency}</span>
                <span className="muted small">{l.ilvl ? `ilvl ${l.ilvl} · ` : ""}{ago(l.indexed)}</span>
                <span className="tw-seller muted small">{l.account}</span>
                {l.whisper && <button className="link small" onClick={() => { invoke("copy_text", { text: l.whisper }); setCopied(i); }}>
                  {copied === i ? "Copied" : "Whisper"}</button>}
              </li>
            ))}
          </ul>
          <button className="link small" onClick={() => invoke("open_url", { url: res.url })}>Open on the trade site</button>
        </div>
      )}
    </div>
  );
}
