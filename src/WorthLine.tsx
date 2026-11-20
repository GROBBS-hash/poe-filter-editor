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
          {group === "DivinationCards" ? " · a stack counts at its total worth" : ""}</>
          : `Uses poe.ninja prices; any ${one} it doesn't price stays as NeverSink has it.`}
      </span>
      {err && <span className="danger-text small">{err}</span>}
    </div>
  );
}
