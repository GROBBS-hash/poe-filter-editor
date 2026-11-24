// Tiers → Threads (24 Nov: "We don't have a section for threads… set currency threshold and they drop in stacks").
// NeverSink has no Threads section (every Thread sits in Fragments tier 3 with the Goddess items), so this page lists
// the Threads poe.ninja prices and keeps a "hide if worth under" line plus "always show stacks of N+" in the price
// settings (key "threads", core prices.rs section_hide_blocks). Stacks count at their total worth.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { friendly } from "./errors";
import { CurrencyIcon, ItemIcon } from "./icons";
import { worthText } from "./WorthLine";

type Page = { items: [string, number][]; limit: number | null; stack: number | null; unit: string };
type Settings = { section_hide?: Record<string, number>; section_stack?: Record<string, number> } & Record<string, unknown>;
const KEY = "threads";

export default function Threads({ profileId, onChange, onOpenLook }: { profileId: string; onChange: () => void; onOpenLook?: () => void }) {
  const [page, setPage] = useState<Page | null>(null);
  const [limit, setLimit] = useState<number | null>(null);
  const [stack, setStack] = useState<number | null>(null);
  const [err, setErr] = useState("");
  const [saved, setSaved] = useState("");
  const timer = useRef(0);

  useEffect(() => {
    invoke<Page>("get_threads", { id: profileId }).then((p) => { setPage(p); setLimit(p.limit); setStack(p.stack); }).catch((e) => setErr(friendly(e)));
  }, [profileId]);

  // Typing "15" shouldn't rebuild the filter for "1" first.
  const save = (nextLimit: number | null, nextStack: number | null) => {
    setLimit(nextLimit); setStack(nextStack);
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      invoke<{ settings: Settings }>("get_price_settings", { id: profileId }).then((p) => {
        const hide = { ...(p.settings.section_hide ?? {}) };
        const stacks = { ...(p.settings.section_stack ?? {}) };
        if (nextLimit == null) delete hide[KEY]; else hide[KEY] = nextLimit;
        if (nextStack == null || nextLimit == null) delete stacks[KEY]; else stacks[KEY] = nextStack;
        return invoke("set_price_settings", { id: profileId, settings: { ...p.settings, section_hide: hide, section_stack: stacks } });
      }).then(() => { setErr(""); setSaved("Saved. Press F4 in game."); onChange(); }).catch((e) => setErr(friendly(e)));
    }, 600);
  };

  if (!page) return err ? <div className="card">{err}</div> : null;
  const unit = page.unit;
  const on = limit != null;
  /** From what stack size a Thread shows: 1 = every drop. */
  const from = (v: number) => {
    if (!on || v >= (limit ?? 0)) return 1;
    const need = Math.ceil((limit ?? 0) / v);
    return stack && stack > 1 ? Math.min(need, stack) : need;
  };
  const hidden = on ? page.items.filter(([, v]) => from(v) > 1).length : 0;

  return (
    <div className="card threads">
      <h2 className="with-icon"><ItemIcon base="Breach Thread" size={28} />Threads</h2>
      <div className="note">
        NeverSink shows every Thread with one purple look (it keeps them in <b>Fragments › Tier 3</b>, with the Goddess items).
        Here you choose which ones are worth picking up, from poe.ninja's prices. A stack counts at its total worth, so
        a pile of cheap Threads still shows.
        {onOpenLook && <> <button className="link small" onClick={onOpenLook}>Change how they look</button></>}
      </div>

      <div className={`worth-line ${on ? "on" : ""}`}>
        <label>
          <input type="checkbox" checked={on} onChange={(e) => save(e.target.checked ? (unit === "chaos" ? 1 : 5) : null, stack)} />
          Hide Threads worth under
        </label>
        <input className="num" type="number" min={0} step="any" disabled={!on} value={limit ?? ""}
          onChange={(e) => save(Math.max(0, +e.target.value || 0), stack)} />
        {unit === "chaos" ? <CurrencyIcon id="chaos" /> : <span className="muted small">{unit}</span>}
        <span className="muted small">{on ? <>hides single drops of <b>{hidden}</b> of {page.items.length} Threads right now · updates with the prices</> : "Off: NeverSink shows them all."}</span>
      </div>
      <div className={`worth-line ${on && stack ? "on" : ""}`}>
        <label title={on ? "" : "Set a price line first"}>
          <input type="checkbox" disabled={!on} checked={on && stack != null} onChange={(e) => save(limit, e.target.checked ? 5 : null)} />
          Always show stacks of
        </label>
        <input className="num" type="number" min={2} step={1} disabled={!on || stack == null} value={stack ?? ""}
          onChange={(e) => save(limit, Math.max(2, Math.round(+e.target.value) || 2))} />
        <span className="muted small">or more, whatever they're worth</span>
      </div>

      {page.items.length === 0 ? <div className="empty">No Thread prices yet (poe.ninja's prices load in the background).</div> : (
        <div className="thr-list">
          {[...page.items].reverse().map(([name, v]) => {
            const n = from(v);
            return (
              <div key={name} className={`thr-row ${n > 1 ? "thr-stack" : ""}`}>
                <ItemIcon base={name} size={30} />
                <b>{name}</b>
                <span className="thr-price">{worthText(v, unit)}</span>
                <span className={n > 1 ? "thr-when muted" : "thr-when ok"}>{n > 1 ? `shows in stacks of ${n}+` : "always shows"}</span>
              </div>
            );
          })}
        </div>
      )}
      {saved && !err && <div className="note">{saved}</div>}
      {err && <div className="danger-text small">{err}</div>}
    </div>
  );
}
