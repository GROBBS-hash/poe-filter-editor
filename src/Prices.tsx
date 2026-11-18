import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ProfileInfo } from "./types";
import StyleEditor, { CustomStyle, Preview } from "./StyleEditor";

type Group = "Uniques" | "DivinationCards" | "Currency" | "Other";
type Tier = { min: number; style: CustomStyle };
type GroupSettings = { group: Group; enabled: boolean; tiers: Tier[]; hide_below?: number | null };
type Settings = { enabled: boolean; groups: GroupSettings[] };
type Page = { settings: Settings; info?: { league: string; updated: string; unit: string; count: number } | null };

const TITLES: Record<Group, string> = {
  Uniques: "Uniques",
  DivinationCards: "Divination cards",
  Currency: "Currency & league items",
  Other: "Other (invitations, memories, incubators…)",
};
const HINTS: Record<Group, string> = {
  Uniques: "Uniques sharing a base use the most valuable one, so nothing good is ever hidden.",
  DivinationCards: "Priced per card.",
  Currency: "Scarabs, essences, oils, omens, fragments and more. Priced per single item.",
  Other: "",
};
const short = (unit: string) => ({ chaos: "c", exalted: "ex", divine: "div" } as Record<string, string>)[unit] ?? unit;

export default function Prices({ profile }: { profile: ProfileInfo }) {
  const [page, setPage] = useState<Page | null>(null);
  const [s, setS] = useState<Settings | null>(null);
  const [dirty, setDirty] = useState(false);
  const [msg, setMsg] = useState("");
  const [styling, setStyling] = useState<{ g: number; t: number } | null>(null);

  const load = async () => {
    try {
      const p = await invoke<Page>("get_price_settings", { id: profile.id });
      setPage(p);
      setS(p.settings);
      setDirty(false);
    } catch (e) { setMsg(String(e)); }
  };
  useEffect(() => { load(); }, [profile.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    const un = listen("prices-updated", () => { if (!dirty) load(); });
    return () => { un.then((f) => f()); };
  });

  if (!s || !page) return <div className="card">{msg || "Loading prices…"}</div>;
  const unit = page.info?.unit ?? "chaos";
  const u = short(unit);

  const change = (fn: (x: Settings) => void) => {
    const copy: Settings = JSON.parse(JSON.stringify(s));
    fn(copy);
    setS(copy);
    setDirty(true);
    setMsg("");
  };
  const save = async () => {
    try {
      const w = await invoke<string[]>("set_price_settings", { id: profile.id, settings: s });
      setDirty(false);
      setMsg(w[0] ?? (s.enabled ? "Saved. Press F4 in game to reload your filter." : "Saved. Price tiers are off for this filter."));
    } catch (e) { setMsg(String(e)); }
  };
  const refresh = async () => {
    setMsg("Checking for new prices…");
    const changed = await invoke<boolean>("refresh_prices").catch(() => false);
    await load();
    setMsg(changed ? "Got new prices." : "Prices are already up to date (they refresh hourly).");
  };
  const when = page.info ? new Date(page.info.updated).toLocaleString() : "";

  if (styling) {
    const tier = s.groups[styling.g].tiers[styling.t];
    return (
      <div className="card prices-style">
        <h2>{TITLES[s.groups[styling.g].group]}: look for tier {styling.t + 1} ({tier.min}{u}+)</h2>
        <div className="qm">
          <StyleEditor start={tier.style} label="Example item"
            onCancel={() => setStyling(null)}
            onSave={(style) => { change((x) => { x.groups[styling.g].tiers[styling.t].style = style; }); setStyling(null); }} />
        </div>
      </div>
    );
  }

  return (
    <>
      <div className="card">
        <div className="row between">
          <h2>Price tiers for “{profile.name}”</h2>
          <button onClick={refresh}>Check for new prices</button>
        </div>
        <p className="muted">
          {page.info
            ? <>Prices from poe.ninja for <b>{page.info.league}</b> · {page.info.count.toLocaleString()} items · updated {when}. They refresh every hour and your filter is rebuilt automatically.</>
            : <>No prices downloaded yet. Check your internet connection, then press “Check for new prices”.</>}
        </p>
        <label className="check big-check">
          <input type="checkbox" checked={s.enabled} onChange={(e) => change((x) => { x.enabled = e.target.checked; })} />
          <span>Use price tiers in this filter</span>
        </label>
        <p className="muted small">Order of priority: your own edits first, then these price tiers, then NeverSink.</p>
      </div>

      {s.groups.map((g, gi) => (
        <div key={g.group} className={`card ${!s.enabled ? "dim" : ""}`}>
          <label className="check">
            <input type="checkbox" checked={g.enabled} disabled={!s.enabled}
              onChange={(e) => change((x) => { x.groups[gi].enabled = e.target.checked; })} />
            <span className="group-title">{TITLES[g.group]}</span>
          </label>
          {HINTS[g.group] && <p className="muted small">{HINTS[g.group]}</p>}

          {g.enabled && s.enabled && (
            <>
              <ul className="tiers">
                {[...g.tiers.map((t, ti) => ({ t, ti }))].sort((a, b) => b.t.min - a.t.min).map(({ t, ti }, rank) => (
                  <li key={ti}>
                    <span className="tier-name">Tier {rank + 1}</span>
                    <span className="muted small">worth</span>
                    <input type="number" min={0} step="any" value={t.min}
                      onChange={(e) => change((x) => { x.groups[gi].tiers[ti].min = Math.max(0, +e.target.value || 0); })} />
                    <span className="muted small">{u} or more</span>
                    <span className="tier-preview"><Preview s={t.style} text="Example" /></span>
                    <button className="link" onClick={() => setStyling({ g: gi, t: ti })}>Change look</button>
                    <button className="link" onClick={() => change((x) => { x.groups[gi].tiers.splice(ti, 1); })}>✕</button>
                  </li>
                ))}
              </ul>
              <button onClick={() => change((x) => {
                const low = Math.min(...x.groups[gi].tiers.map((t) => t.min), 10);
                x.groups[gi].tiers.push({ min: Math.max(1, Math.round(low / 2)), style: JSON.parse(JSON.stringify(x.groups[gi].tiers.at(-1)?.style ?? g.tiers[0]?.style)) });
              })} disabled={g.tiers.length === 0}>Add a tier</button>

              {g.group !== "Currency" && (
                <label className="check hide-below">
                  <input type="checkbox" checked={g.hide_below != null}
                    onChange={(e) => change((x) => { x.groups[gi].hide_below = e.target.checked ? (unit === "chaos" ? 5 : 20) : null; })} />
                  <span>Only show ones worth at least</span>
                  <input type="number" min={0} step="any" disabled={g.hide_below == null} value={g.hide_below ?? ""}
                    onChange={(e) => change((x) => { x.groups[gi].hide_below = Math.max(0, +e.target.value || 0); })} />
                  <span>{u}</span>
                </label>
              )}
            </>
          )}
        </div>
      ))}

      <div className="save-bar">
        {msg && <span className="note">{msg}</span>}
        <button className="primary" disabled={!dirty} onClick={save}>{dirty ? "Save & rebuild filter" : "Saved"}</button>
      </div>
    </>
  );
}
