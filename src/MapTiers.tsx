import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Tip } from "./Help";
import StyleEditor, { CurrentStyle, Preview, fromCurrent } from "./StyleEditor";

type Row = { tier: string; name: string; visibility: string; style: CurrentStyle; note: string; bases: string[]; min_stack: number | null; min_item_tier?: number | null };
type MapRow = Row & { map_tier: number; band: string };
export type MapSection = { type_path: string[]; title: string; group: string; tiers: Row[]; shared_bases?: boolean };
type MapData = { tiers: MapRow[]; special: MapSection[] };

const BAND_NAMES: Record<string, string> = { a: "high reds", b: "low reds", c: "yellows", d: "whites" };
const MAPS = ["maps"];

/** Like FilterBlade's map options: from which tier maps show on the floor, a look per band of tiers
 *  (whites / yellows / low reds / high reds), and the special kinds NeverSink handles first.
 *  Everything is saved as tier edits, so it survives NeverSink updates. */
export default function MapTiers({ profileId, onChange, onOpen }: { profileId: string; onChange: () => void;
  onOpen: (s: MapSection) => void }) {
  const [data, setData] = useState<MapData | null>(null);
  const [msg, setMsg] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () => invoke<MapData>("get_map_tiers", { id: profileId }).then(setData).catch((e) => setMsg(String(e)));
  useEffect(() => { load(); }, [profileId]); // eslint-disable-line react-hooks/exhaustive-deps

  const change = async (typePath: string[], tiers: string[], patch: { show?: boolean; style?: unknown; reset?: boolean }, m: string) => {
    if (tiers.length === 0) return;
    setBusy(true);
    try {
      await invoke("set_tiers", { id: profileId, typePath, tiers, show: patch.show ?? null, style: patch.style ?? null, reset: !!patch.reset });
      setMsg(`${m} Press F4 in game.`); onChange(); await load();
    } catch (e) { setMsg(String(e)); }
    setBusy(false);
  };

  if (!data) return <div className="card"><h2>Maps</h2><div className="muted">{msg || "Loading NeverSink's map rules…"}</div></div>;
  if (data.tiers.length === 0) return <div className="card"><h2>Maps</h2><div className="empty">This filter has no map tier rules.</div></div>;

  const shown = (r: Row) => r.visibility !== "Hide";
  // The lowest tier from which every tier up is shown.
  let from = 17;
  for (let i = data.tiers.length - 1; i >= 0 && shown(data.tiers[i]); i--) from = data.tiers[i].map_tier;
  const setFrom = (n: number) => {
    const hide = data.tiers.filter((t) => t.map_tier < n && shown(t)).map((t) => t.tier);
    const show = data.tiers.filter((t) => t.map_tier >= n && !shown(t)).map((t) => t.tier);
    (async () => {
      if (hide.length) await change(MAPS, hide, { show: false }, "");
      if (show.length) await change(MAPS, show, { show: true }, "");
      setMsg(n <= 1 ? "All map tiers show. Press F4 in game." : `Maps below tier ${n} are hidden now. Press F4 in game.`);
    })();
  };

  const bands = [...new Set(data.tiers.map((t) => t.band))].sort();
  const bandRows = (b: string) => data.tiers.filter((t) => t.band === b);
  const range = (rows: MapRow[]) => {
    const ts = rows.map((r) => r.map_tier);
    const lo = Math.min(...ts), hi = Math.max(...ts);
    return lo === hi ? `Tier ${lo}` : `Tier ${lo}–${hi}`;
  };

  return (
    <div className="card map-tiers">
      <h2>Maps on the floor <Tip>Pick the <b>lowest map tier</b> you still want to see; lower ones are hidden. Or click single
        tiers below to show/hide them. Each <b>band</b> (whites, yellows, reds) has one look: click it to change how big and
        bright those maps are. <b>Special maps</b> (blighted, influenced, Valdo…) are handled first by NeverSink, so they keep
        showing even when their tier is hidden, unless you untick them. Your changes survive NeverSink updates.</Tip></h2>

      <div className="mt-from">
        <span>Show maps from</span>
        <select value={from > 16 ? "" : from} disabled={busy} onChange={(e) => setFrom(parseInt(e.target.value, 10))}>
          {from > 16 && <option value="">(all hidden)</option>}
          {data.tiers.map((t) => <option key={t.tier} value={t.map_tier}>tier {t.map_tier}</option>)}
        </select>
        <span>and up</span>
        <span className="muted small">{from <= 1 ? "Every tier shows." : from > 16 ? "" : `Tiers 1–${from - 1} are hidden.`}</span>
      </div>

      <div className="mt-strip" role="group" aria-label="Map tiers">
        {data.tiers.map((t) => (
          <button key={t.tier} className={`mt-chip band-${t.band} ${shown(t) ? "on" : "off"}`} disabled={busy}
            title={`${shown(t) ? "Shown" : "Hidden"}: click to ${shown(t) ? "hide" : "show"} tier ${t.map_tier} maps`}
            onClick={() => change(MAPS, [t.tier], { show: !shown(t) }, `Tier ${t.map_tier} maps ${shown(t) ? "hidden" : "shown"}.`)}>
            {t.map_tier}
          </button>
        ))}
      </div>

      <h3 className="mt-h">Look per band</h3>
      {bands.map((b) => {
        const rows = bandRows(b);
        const all = rows.every(shown), none = !rows.some(shown);
        const label = `${range(rows)} · ${BAND_NAMES[b] ?? b}`;
        return (
          <div key={b} className={`tier ${none ? "off" : ""}`}>
            <div className="tier-head">
              <label className="tier-name">
                <input type="checkbox" checked={all} disabled={busy}
                  ref={(el) => { if (el) el.indeterminate = !all && !none; }}
                  onChange={(e) => change(MAPS, rows.map((r) => r.tier), { show: e.target.checked }, `${label} ${e.target.checked ? "shown" : "hidden"}.`)} />
                <span>{label}</span>
              </label>
              <button className="tier-look" title="Change the look of every map in this band" onClick={() => setEditing(editing === b ? null : b)}>
                <Preview s={fromCurrent(rows[rows.length - 1].style, "Maps")} text={`Map (Tier ${rows[rows.length - 1].map_tier})`} />
              </button>
              <span className="tier-tools">
                <button className="link small" disabled={busy} title="Put this band back to NeverSink's"
                  onClick={() => change(MAPS, rows.map((r) => r.tier), { reset: true }, `${label} is back to NeverSink's.`)}>Reset</button>
              </span>
            </div>
            {editing === b && (
              <div className="qm tier-edit">
                <StyleEditor start={fromCurrent(rows[rows.length - 1].style, "Maps")} label={`Map (Tier ${rows[rows.length - 1].map_tier})`}
                  onCancel={() => setEditing(null)}
                  onSave={(s) => { setEditing(null); change(MAPS, rows.map((r) => r.tier), { style: s }, `${label} restyled.`); }} />
              </div>
            )}
          </div>
        );
      })}

      {data.special.length > 0 && <>
        <h3 className="mt-h">Special maps <span className="muted small">(checked before the tiers above)</span></h3>
        {data.special.map((s) => {
          const all = s.tiers.every(shown), none = !s.tiers.some(shown);
          return (
            <div key={s.type_path.join(">")} className={`tier mt-special ${none ? "off" : ""}`}>
              <div className="tier-head">
                <label className="tier-name">
                  <input type="checkbox" checked={all} disabled={busy}
                    ref={(el) => { if (el) el.indeterminate = !all && !none; }}
                    onChange={(e) => change(s.type_path, s.tiers.map((r) => r.tier), { show: e.target.checked }, `${s.title} ${e.target.checked ? "shown" : "hidden"}.`)} />
                  <span>{s.title}</span>
                  <span className="muted small">{s.tiers.length} rule{s.tiers.length === 1 ? "" : "s"}</span>
                </label>
                <span className="tier-look"><Preview s={fromCurrent(s.tiers[0].style, "Maps")} text="Map" /></span>
                <span className="tier-tools">
                  <button className="link small" onClick={() => onOpen(s)}>Open</button>
                </span>
              </div>
            </div>
          );
        })}
      </>}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
