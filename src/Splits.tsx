// Campaign split timer (27 Nov, look C picked by GROBBS): its own small window like LiveSplit, plus the run history on
// the Play tools page. The times come from the game's log (`splits_app.rs` / core `splits.rs`): time played from level 2,
// breaks over 10 minutes and other characters left out.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { WinBar, useFitHeight } from "./WinFrame";
import "./play.css";
import "./splits.css";

type Row = { label: string; mine: number | null; best: number | null; seg: number | null; best_seg: number | null; gold: boolean; level: number | null };
type Past = { idx: number; name: string; class: string; started: number; acts: number; time: number | null; best: boolean; hidden: boolean };
export type SplitsView = {
  name: string; class: string; level: number; played: number; live: boolean; done: boolean; rows: Row[];
  best_name: string | null; best_total: number | null; sum_best: number | null; past: Past[];
};

export const clock = (secs: number) => {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}` : `${m}:${String(r).padStart(2, "0")}`;
};
/** "−1:05" ahead / "+2:30" behind. */
const delta = (d: number) => `${d <= 0 ? "−" : "+"}${clock(Math.abs(d))}`;

function useSplits() {
  const [v, setV] = useState<SplitsView | null | undefined>(undefined);
  const [at, setAt] = useState(Date.now());
  useEffect(() => {
    const load = () => invoke<SplitsView | null>("splits_view").then((x) => { setV(x); setAt(Date.now()); }).catch(() => setV(null));
    load();
    const un = listen("splits-update", load);
    const un2 = listen("play-shown", load);
    const poll = window.setInterval(load, 15000);
    return () => { un.then((f) => f()); un2.then((f) => f()); window.clearInterval(poll); };
  }, []);
  return { v, at };
}

/** The split timer window (Alt+T). */
export function SplitsWindow() {
  const { v, at } = useSplits();
  const [now, setNow] = useState(Date.now());
  const [key, setKey] = useState("Alt+T");
  useFitHeight(250, 120);
  useEffect(() => { const t = window.setInterval(() => setNow(Date.now()), 500); return () => window.clearInterval(t); }, []);
  useEffect(() => { invoke<Record<string, string>>("get_settings").then((s) => setKey(s?.key_splits ?? "")).catch(() => {}); }, []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("play_hide", { label: "splits" }); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const hide = () => invoke("play_hide", { label: "splits" });
  if (v === undefined) return <div className="bo pw" />;
  if (v === null) return (
    <div className="bo pw sp">
      <WinBar title="Split timer" hint={`Esc${key ? ` / ${key}` : ""} hides`} onClose={hide} />
      <div className="bo-body sp-empty">Make a new character: the clock starts at level 2 and splits each act by itself.</div>
    </div>
  );
  const elapsed = v.played + (v.live ? Math.max(0, (now - at) / 1000) : 0);
  const cur = v.rows.findIndex((r) => r.mine == null);
  const curRow = cur >= 0 ? v.rows[cur] : null;
  const liveDiff = curRow?.best != null ? elapsed - curRow.best : null;
  const lastDone = cur === -1 ? v.rows[v.rows.length - 1] : cur > 0 ? v.rows[cur - 1] : null;
  const ahead = !v.done ? (liveDiff != null && liveDiff > 0 ? false : lastDone?.mine != null && lastDone.best != null ? lastDone.mine <= lastDone.best : liveDiff != null ? liveDiff <= 0 : null)
    : lastDone?.mine != null && lastDone.best != null ? lastDone.mine <= lastDone.best : null;
  return (
    <div className="bo pw sp">
      <WinBar title={v.name} hint={`Esc${key ? ` / ${key}` : ""} hides`} onClose={hide} compact />
      <div className="bo-body">
        {v.rows.map((r, i) => {
          const now_ = i === cur;
          const d = r.mine != null && r.best != null ? r.mine - r.best : now_ ? liveDiff : null;
          const cls = r.gold ? "gold" : d == null ? "" : d <= 0 ? "ahead" : "behind";
          return (
            <div key={i} className={`sp-row ${now_ ? "now" : ""} ${r.mine == null && !now_ ? "later" : ""}`}
              title={r.seg != null ? `This act took ${clock(r.seg)}${r.best_seg != null ? ` · your fastest ever: ${clock(r.best_seg)}` : ""}${r.level ? ` · level ${r.level}` : ""}` : r.best_seg != null ? `Your fastest ever: ${clock(r.best_seg)}` : undefined}>
              <span className="sp-label">{r.label}</span>
              <span className={`sp-diff ${cls}`}>{d != null && (r.mine != null || (now_ && d > -120)) ? delta(d) : ""}</span>
              <span className="sp-time">{r.mine != null ? clock(r.mine) : r.best != null ? clock(r.best) : "—"}</span>
            </div>
          );
        })}
        <div className={`sp-big ${ahead == null ? "" : ahead ? "ahead" : "behind"} ${v.live ? "" : "still"}`}
          title={v.live ? "Time played since level 2" : v.done ? "Campaign done" : "Stopped: no play for a while, or you're on another character"}>
          {clock(v.done && lastDone?.mine != null ? lastDone.mine : elapsed)}
        </div>
        {curRow?.best_seg != null && <div className="sp-foot"><span>Best {curRow.label.split(" ·")[0].toLowerCase()} ever</span><span className="gold">{clock(curRow.best_seg)}</span></div>}
        {v.best_total != null && <div className="sp-foot" title={`Your fastest finished campaign: ${v.best_name}`}><span>Best run ({v.best_name})</span><span>{clock(v.best_total)}</span></div>}
        {v.sum_best != null && <div className="sp-foot"><span>Best possible</span><span>{clock(v.sum_best)}</span></div>}
        {v.best_total == null && <div className="sp-foot"><span>No finished run yet: this one becomes the one to beat</span></div>}
      </div>
    </div>
  );
}

/** Play tools page: every campaign in your log, best first marked. */
export function SplitsCard() {
  const { v } = useSplits();
  const [all, setAll] = useState(false);
  if (!v || v.past.length === 0) return null;
  const list = all ? v.past : v.past.slice(0, 8);
  return (
    <div className="card">
      <h2>Campaign runs</h2>
      <p className="muted small">Every character in your game's log, timed from level 2 to the end of act 10 (time played: breaks over 10 minutes
        and time on other characters don't count). The split timer window races you against the fastest one, ★. Hide a run to leave it out.</p>
      <table className="pt-table"><thead><tr><th>Character</th><th>Started</th><th>Reached</th><th>Time</th><th></th></tr></thead>
        <tbody>{list.map((p) => (
          <tr key={p.idx} className={p.hidden ? "muted" : ""}>
            <td>{p.best ? "★ " : ""}{p.name} <span className="muted small">{p.class}</span></td>
            <td className="muted">{new Date(p.started * 1000).toLocaleDateString()}</td>
            <td>{p.acts >= 10 ? "done" : `act ${p.acts + 1}`}</td>
            <td>{p.time != null ? clock(p.time) : ""}</td>
            <td><button className="link" onClick={() => invoke("splits_hide", { idx: p.idx, hidden: !p.hidden })}>{p.hidden ? "Show" : "Hide"}</button></td>
          </tr>
        ))}</tbody></table>
      {v.past.length > 8 && <button className="link" onClick={() => setAll(!all)}>{all ? "Fewer" : `All ${v.past.length}`}</button>}
    </div>
  );
}
