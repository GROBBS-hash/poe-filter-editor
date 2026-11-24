import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { friendly } from "./errors";
import { CurrencyIcon } from "./icons";

type Group = "Uniques" | "DivinationCards";
type GroupSettings = { group: string; enabled: boolean; tiers: unknown[]; hide_below?: number | null };
type Settings = { enabled: boolean; groups: GroupSettings[] };
type Page = { settings: Settings; info?: { unit: string } | null };

const WORDS: Record<Group, [string, string]> = { DivinationCards: ["cards", "card"], Uniques: ["uniques", "unique"] };

/**
 * "Hide if worth under N chaos" at the top of a Tiers section (divination cards, uniques), from poe.ninja's prices.
 * It's the price settings' hide_below for that group, so it works with price tiers off and updates with the prices.
 * Cards count a stack at its total worth (3 x 2c cards = 6c). Anything poe.ninja doesn't price is left to NeverSink.
 */
export default function WorthLine({ profileId, group, onChange }: { profileId: string; group: Group; onChange: () => void }) {
  const [page, setPage] = useState<Page | null>(null);
  const [limit, setLimit] = useState<number | null>(null);
  const [count, setCount] = useState<[number, number] | null>(null);
  const [err, setErr] = useState("");
  const timer = useRef(0);
  const unit = page?.info?.unit ?? "chaos";

  useEffect(() => {
    invoke<Page>("get_price_settings", { id: profileId }).then((p) => {
      setPage(p);
      setLimit(p.settings.groups.find((g) => g.group === group)?.hide_below ?? null);
    }).catch((e) => setErr(friendly(e)));
  }, [profileId, group]);
  useEffect(() => {
    if (limit == null) { setCount(null); return; }
    invoke<[number, number]>("price_count_below", { id: profileId, group, limit }).then(setCount).catch(() => setCount(null));
  }, [profileId, group, limit]);

  const save = (next: number | null) => {
    setLimit(next);
    clearTimeout(timer.current);
    // Typing "15" shouldn't rebuild the filter for "1" first.
    timer.current = window.setTimeout(() => {
      if (!page) return;
      const s: Settings = { ...page.settings, groups: page.settings.groups.map((g) => g.group === group ? { ...g, hide_below: next } : g) };
      invoke("set_price_settings", { id: profileId, settings: s })
        .then(() => { setPage({ ...page, settings: s }); setErr(""); onChange(); })
        .catch((e) => setErr(friendly(e)));
    }, 600);
  };

  if (!page) return null;
  const [many, one] = WORDS[group];
  const on = limit != null;
  return (
    <div className={`worth-line ${on ? "on" : ""}`}>
      <label>
        <input type="checkbox" checked={on} onChange={(e) => save(e.target.checked ? (unit === "chaos" ? 5 : 20) : null)} />
        Hide {many} worth under
      </label>
      <input className="num" type="number" min={0} step="any" disabled={!on} value={limit ?? ""}
        onChange={(e) => save(Math.max(0, +e.target.value || 0))} />
      {unit === "chaos" ? <CurrencyIcon id="chaos" /> : <span className="muted small">{unit}</span>}
      <span className="muted small">
        {on && count ? <>hides <b>{count[0]}</b> of {count[1]} {many} right now (poe.ninja)
          {group === "DivinationCards" ? " · a stack counts at its total worth · cards under 50c that hardly anyone trades (under 5 sold) count as worthless" : ""}</>
          : `Uses poe.ninja prices; any ${one} it doesn't price stays as NeverSink has it.`}
      </span>
      {err && <span className="danger-text small">{err}</span>}
    </div>
  );
}

/** A worth in plain short form: "12c", "0.5c", "2.3 div" (when a Divine Orb's price is known), "40 ex" on PoE 2. */
export function worthText(v: number, unit: string, divine?: number | null): string {
  if (unit === "chaos" && divine && v >= divine) return `${(v / divine).toFixed(v >= divine * 10 ? 0 : 1)} div`;
  const n = v >= 10 ? Math.round(v).toString() : v >= 1 ? v.toFixed(1).replace(/\.0$/, "") : v.toFixed(2).replace(/0$/, "");
  return unit === "chaos" ? `${n}c` : `${n} ${unit === "exalted" ? "ex" : unit}`;
}

/**
 * "Hide items here worth under N" for any Tiers section with poe.ninja prices (corpses, essences, fossils…; 23 Nov:
 * "only show corpses worth x chaos"). Saved as the price settings' section_hide for this section; the filter hides the
 * items listed here that poe.ninja prices under the line, counting a stack at its total worth. Unpriced items stay.
 */
export function SectionWorth({ profileId, sectionKey, prices, limit: start, onChange }: {
  profileId: string; sectionKey: string; prices: Record<string, number>; limit: number | null; onChange: () => void;
}) {
  const [limit, setLimit] = useState<number | null>(start);
  const [err, setErr] = useState("");
  const [unit, setUnit] = useState("chaos");
  const timer = useRef(0);
  useEffect(() => setLimit(start), [start, sectionKey]);
  useEffect(() => { invoke<Page>("get_price_settings", { id: profileId }).then((p) => setUnit(p.info?.unit ?? "chaos")).catch(() => {}); }, [profileId]);
  // Threads are listed in Fragments but only their own page hides them.
  const values = Object.entries(prices).filter(([n]) => sectionKey !== "fragments" || !n.endsWith(" Thread")).map(([, v]) => v);
  const save = (next: number | null) => {
    setLimit(next);
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      invoke<Page & { settings: Settings & { section_hide?: Record<string, number> } }>("get_price_settings", { id: profileId }).then((p) => {
        const hides = { ...(p.settings.section_hide ?? {}) };
        if (next == null) delete hides[sectionKey]; else hides[sectionKey] = next;
        return invoke("set_price_settings", { id: profileId, settings: { ...p.settings, section_hide: hides } });
      }).then(() => { setErr(""); onChange(); }).catch((e) => setErr(friendly(e)));
    }, 600);
  };
  if (values.length === 0) return null;
  const on = limit != null;
  const below = on ? values.filter((v) => v < (limit ?? 0)).length : 0;
  return (
    <div className={`worth-line ${on ? "on" : ""}`}>
      <label>
        <input type="checkbox" checked={on} onChange={(e) => save(e.target.checked ? (unit === "chaos" ? 5 : 20) : null)} />
        Hide items here worth under
      </label>
      <input className="num" type="number" min={0} step="any" disabled={!on} value={limit ?? ""}
        onChange={(e) => save(Math.max(0, +e.target.value || 0))} />
      {unit === "chaos" ? <CurrencyIcon id="chaos" /> : <span className="muted small">{unit}</span>}
      <span className="muted small">
        {on ? <>hides <b>{below}</b> of the {values.length} priced items here right now · a stack counts at its total worth · prices from poe.ninja</>
          : `Each item shows its poe.ninja price. ${values.length} of the items here have one; the rest stay as NeverSink has them.`}
      </span>
      {err && <span className="danger-text small">{err}</span>}
    </div>
  );
}
