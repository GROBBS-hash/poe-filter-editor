import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { ItemIcon } from "./icons";
import { MechArt } from "./mechArt";

type Loot = { label: string; name: string; each: number | null; per_run: number; sell: number; chaos: number; jackpot: boolean; note: string | null };
type Cost = { label: string; item: string; count: number; chaos: number };
type Source = { who: string; video: string; date: string; sample: string; trust: string; said: string };
/** A creator farm at today's prices (core strategies.rs). */
export type Strat = { key: string; title: string; pic: string; mech: string[]; how: string; unit: string; runs_hour: number; runs_guess: boolean;
  loot: Loot[]; other: number; costs: Cost[]; cost: number; steady: number; jackpots: number; div_hour: number; div_hour_all: number; rank: number;
  big_sample: boolean; one_item: string | null; missing: string[]; source: Source; ask: Record<string, unknown>; maps: string[]; drops: string[];
  threads: [string, number][]; thread_keys: [string, number][]; map_cost: number; roll_cost: number; vaal: number; entry: [string, number][] };

const money = (c: number, div: number) => Math.abs(c) >= div ? `${(c / div).toFixed(Math.abs(c / div) >= 10 ? 0 : 1)} div` : Math.abs(c) >= 10 ? `${Math.round(c)}c` : `${c.toFixed(1)}c`;
const dh = (x: number) => (Math.abs(x) >= 10 ? Math.round(x).toString() : x.toFixed(1));
const TRUST: Record<string, string> = { high: "Many maps logged with costs", medium: "Some maps logged", low: "A showcase, few maps" };

/**
 * What to farm today, top: proven farms from creators' videos, best first (he picked mock-up B, the leaderboard, 26 Nov).
 * The bar is divines an hour after costs at today's prices; striped purple = jackpots spread over many maps.
 */
export default function FarmBoard({ list, div, steadyOnly, now, onFarm }: {
  list: Strat[]; div: number; steadyOnly: boolean; now: string | null; onFarm: (s: Strat) => Promise<void>;
}) {
  const [open, setOpen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  if (list.length === 0) return null;
  const rows = [...list].sort((a, b) => steadyOnly ? b.div_hour - a.div_hour : b.rank - a.rank);
  const top = Math.max(1, ...rows.map((s) => steadyOnly ? s.div_hour : s.div_hour_all));
  return (
    <div className="fb">
      <div className="ap-step">Proven farms, best first <span className="ap-step-note">(from creators' logs this week, at today's prices, after every cost)</span></div>
      {rows.map((s, i) => {
        const jack = !steadyOnly && s.jackpots > 0;
        const isOpen = open === s.key;
        return (
          <div key={s.key} className={`fb-row ${isOpen ? "open" : ""}`}>
            <button className="fb-main" aria-expanded={isOpen} onClick={() => setOpen(isOpen ? null : s.key)} title="Where its money comes from">
              <span className="fb-rank">{i + 1}</span>
              <span className="fb-pic"><ItemIcon name={s.pic} size={40} /></span>
              <span className="fb-mech">{s.mech.map((m) => <MechArt key={m} m={m} size={20} />)}</span>
              <span className="fb-title"><b>{s.title}</b><small>{s.source.who} · {s.source.sample}</small></span>
              <span className="fb-bar">
                <span className="fb-track">
                  <i className="steady" style={{ width: `${Math.max(0, s.div_hour) / top * 100}%` }} />
                  {jack && <i className="jack" style={{ width: `${Math.max(0, s.div_hour_all - Math.max(0, s.div_hour)) / top * 100}%` }} />}
                </span>
                <span className="fb-n"><b className={s.div_hour >= 0 ? "ff-up" : "ff-dn"}>{s.div_hour < 0 ? "−" : ""}{dh(Math.abs(s.div_hour))}</b>{jack ? " + 🎲" : ""} div/h</span>
              </span>
              <span className="fb-cost" title={s.costs.map((c) => `${c.count !== 1 ? `${c.count} × ` : ""}${c.label}: ${money(c.chaos, div)}`).join("\n") || "Nothing to buy"}>
                {s.costs.slice(0, 4).map((c) => <ItemIcon key={c.label} name={c.item} size={22} />)}
                <small>{s.cost > 0 ? `${money(s.cost, div)}/${s.unit}` : "free"}</small>
              </span>
              <span className="fb-badge">{s.big_sample && <span className="fb-sample" title="Most of its money comes from rare big drops: you need many runs before your own numbers mean much">🎲 Needs a big sample</span>}</span>
            </button>
            <button className={i === 0 ? "primary fb-go" : "fb-go"} disabled={busy != null || now === s.title} title="Saves its tree and uses it, lists what to buy, and shows your profit after costs"
              onClick={() => { setBusy(s.key); onFarm(s).finally(() => setBusy(null)); }}>{now === s.title ? "Farming now" : busy === s.key ? "Setting up…" : "Farm this"}</button>
            {isOpen && (
              <div className="fb-more">
                <p className="small">{s.how}</p>
                <table className="fb-receipt">
                  <tbody>
                    {s.loot.map((l) => (
                      <tr key={l.label + l.name} className={l.jackpot ? "jack" : ""} title={l.note ?? ""}>
                        <td><ItemIcon name={l.name} size={22} /></td>
                        <td>{l.label}{l.jackpot ? " (jackpot)" : ""} <span className="muted">{l.per_run < 0.1 ? `1 in ${Math.round(1 / l.per_run)} ${s.unit}s` : `${+l.per_run.toFixed(2)} a ${s.unit}`}{l.each != null ? ` at ${money(l.each, div)}` : ", no price today"}{l.sell < 1 ? `, sells for ${Math.round(l.sell * 100)}%` : ""}</span></td>
                        <td className="ff-up">+{money(l.chaos, div)}</td>
                      </tr>
                    ))}
                    {s.other > 0 && <tr title="The creator's total minus the items above: gold, Lost Shipments, invitations, other drops"><td><ItemIcon name="Chaos Orb" size={22} /></td><td>Everything else they got <span className="muted">(their number)</span></td><td className="ff-up">+{money(s.other, div)}</td></tr>}
                    {s.costs.map((c) => (
                      <tr key={c.label}><td><ItemIcon name={c.item} size={22} /></td><td>{c.count !== 1 ? `${c.count} × ` : ""}{c.label}</td><td className="ff-dn">−{money(c.chaos, div)}</td></tr>
                    ))}
                    <tr className="tot"><td /><td>Profit a {s.unit}</td><td>{money(s.steady + (steadyOnly ? 0 : s.jackpots) - s.cost, div)}</td></tr>
                    <tr className="tot"><td /><td>× {s.runs_hour} {s.unit}s an hour{s.runs_guess ? " (our guess)" : ""}</td><td className="ff-up"><b>{dh(steadyOnly ? s.div_hour : s.div_hour_all)} div an hour</b></td></tr>
                  </tbody>
                </table>
                {s.one_item && <div className="small ff-warn">Rides on one item's price ({s.one_item}): if it drops, so does this farm.</div>}
                {s.missing.length > 0 && <div className="small muted">No price today for: {s.missing.join(", ")} (counted as nothing).</div>}
                <div className="small">
                  <b>{s.source.who}</b>, {s.source.date}: “{s.source.said}”. <span className="muted" title={TRUST[s.source.trust] ?? ""}>{s.source.sample}.</span>{" "}
                  {s.source.video && <button className="link small" onClick={() => invoke("open_url", { url: `https://www.youtube.com/watch?v=${s.source.video}` })}>Watch the video</button>}
                </div>
              </div>
            )}
          </div>
        );
      })}
      <p className="small muted">Click a row to see where its money comes from. Striped purple = jackpots (Headhunter, Mageblood…) spread over many runs.</p>
    </div>
  );
}
