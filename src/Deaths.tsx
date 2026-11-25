import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { friendly } from "./errors";
import { ItemIcon } from "./icons";
import { Tip } from "./Help";
import "./deaths.css";

type Death = { time: number; map: string; area: string; level: number; char_level: number | null; secs: number; mods: string[] };
type Row = { name: string; runs: number; bad_runs: number; deaths: number; other_runs: number; other_bad: number; stands_out: boolean };
type View = { recent: Death[]; patterns: { runs: number; deaths: number; bad_runs: number; with_mods: number; maps: Row[]; mods: Row[] }; avoid: ("on" | "can" | "no")[] };

const mmss = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
const when = (t: number) => new Date(t * 1000).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
const tier = (l: number) => (l >= 68 ? `T${l - 67}` : `level ${l}`);

function Bar({ r }: { r: Row }) {
  const pct = r.runs ? (r.bad_runs / r.runs) * 100 : 0;
  return (
    <span className="dn-bar" title={`${r.deaths} death${r.deaths === 1 ? "" : "s"} in all${r.other_runs ? `. In your other ${r.other_runs} maps you died in ${r.other_bad}` : ""}`}>
      <i className={r.stands_out ? "hot" : ""} style={{ width: `${Math.max(4, pct)}%` }} />
    </span>
  );
}

/** Play tools: where you died, and what those maps had in common. */
export default function Deaths() {
  const [v, setV] = useState<View | null>(null);
  const [err, setErr] = useState("");
  const [all, setAll] = useState(false);
  const [open, setOpen] = useState<number | null>(null);
  useEffect(() => {
    const load = () => invoke<View>("deaths_get").then(setV).catch((e) => setErr(friendly(e)));
    load();
    const un = listen("deaths-changed", load);
    return () => { un.then((f) => f()); };
  }, []);
  if (!v) return err ? <div className="card error small">{err}</div> : null;
  const p = v.patterns;
  const avoid = (text: string, on: boolean) => invoke<View>("deaths_avoid", { text, on }).then(setV).catch((e) => setErr(friendly(e)));
  const shown = all ? v.recent : v.recent.slice(0, 5);
  return (
    <div className="card">
      <h2>Death notes <Tip>The game doesn't say what killed you, so this shows what was around you: the map, its mods and how far in you were. Mods are
        known for maps you pressed Ctrl+D on before running them.</Tip></h2>
      {p.deaths === 0 ? <p className="muted small">No deaths in your last {p.runs} maps.</p> : <>
        <p className="muted small">{p.deaths} death{p.deaths === 1 ? "" : "s"} in {p.bad_runs} of your last {p.runs} maps. Gold means you die there at least twice as often as elsewhere.</p>
        <div className="dn-cols">
          <div>
            <b>Maps you died in</b>
            {p.maps.slice(0, 6).map((r) => (
              <div key={r.name} className="dn-row">
                <ItemIcon name={`${r.name} Map`} size={26} />
                <span className={r.stands_out ? "dn-hot" : ""}>{r.name}</span><Bar r={r} /><span className="muted small">{r.bad_runs} of {r.runs}</span>
              </div>
            ))}
          </div>
          <div>
            <b>Mods on maps you died in</b>
            {p.mods.length === 0
              ? <p className="muted small">None known yet. Press Ctrl+D on a map before you open it, and its mods are kept with it ({p.with_mods} of your last {p.runs} maps have them).</p>
              : p.mods.slice(0, all ? 12 : 6).map((r, i) => (
                <div key={r.name} className="dn-row mod">
                  <span className={r.stands_out ? "dn-hot" : ""}>{r.name}</span><Bar r={r} /><span className="muted small">{r.bad_runs} of {r.runs}</span>
                  {v.avoid[i] === "can" && <button className="link small" title="Ticks it on the Maps page: the map check warns about it, and your map search leaves it out" onClick={() => avoid(r.name, true)}>Can't run it</button>}
                  {v.avoid[i] === "on" && <button className="link small" title="On your can't-run list. Click to take it off." onClick={() => avoid(r.name, false)}>✓ can't run</button>}
                </div>
              ))}
            {p.mods.length > 0 && p.with_mods < 10 && <div className="muted small">From {p.with_mods} map{p.with_mods === 1 ? "" : "s"} with known mods: early days.</div>}
          </div>
        </div>
        {v.recent.length > 0 && <>
          <b>Latest</b>
          <div className="dn-list">
            {shown.map((d, i) => (
              <div key={d.time + d.map}>
                <button className="dn-death" onClick={() => setOpen(open === i ? null : i)} disabled={d.mods.length === 0}>
                  <ItemIcon name={`${d.map} Map`} size={26} />
                  <span><b>{d.map}</b> <span className="muted small">{tier(d.level)}</span></span>
                  <span className="muted small">{mmss(d.secs)} in</span>
                  <span className="muted small">{d.char_level ? `level ${d.char_level}` : ""}</span>
                  <span className="muted small">{when(d.time)}</span>
                  <span className="muted small">{d.mods.length ? `${d.mods.length} mods ${open === i ? "▾" : "▸"}` : "mods not known"}</span>
                </button>
                {open === i && <ul className="dn-mods">{d.mods.map((m) => <li key={m}>{m}</li>)}</ul>}
              </div>
            ))}
          </div>
        </>}
        {(v.recent.length > 5 || p.mods.length > 6) && <button className="link small" onClick={() => setAll(!all)}>{all ? "Show fewer" : "Show more"}</button>}
      </>}
      {err && <div className="error small">{err}</div>}
    </div>
  );
}

/** Ctrl+D on a map: next to a mod, how maps with it went before. */
export function useDeathMods(mods: string[]): (string | null)[] {
  const [h, setH] = useState<([number, number] | null)[]>([]);
  const key = mods.join("\n");
  useEffect(() => {
    let on = true;
    setH([]);
    if (key) invoke<([number, number] | null)[]>("deaths_for", { mods: key.split("\n") }).then((x) => on && setH(x ?? [])).catch(() => {});
    return () => { on = false; };
  }, [key]);
  return mods.map((_, i) => (h[i] ? `You died in ${h[i]![1]} of ${h[i]![0]} maps with it` : null));
}
