import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Tip } from "./Help";
import StyleEditor, { CurrentStyle, LookButton, Preview, fromCurrent } from "./StyleEditor";

type Row = { tier: string; name: string; visibility: string; style: CurrentStyle; note: string; bases: string[]; min_stack: number | null; min_item_tier?: number | null };
type MapRow = Row & { map_tier: number; band: string };
export type MapSection = { type_path: string[]; title: string; group: string; tiers: Row[]; shared_bases?: boolean };
type MapExtra = { key: string; title: string; what: string; tiers: string[]; on: boolean; border?: number[]; sample_tier?: number };
type MapData = { tiers: MapRow[]; special: MapSection[]; extras?: MapExtra[] };

const BAND_NAMES: Record<string, string> = { a: "red maps, top", b: "red maps", c: "yellow maps", d: "white maps" };
const MAPS = ["maps"];

/** Like FilterBlade's map options: from which tier maps show on the floor, a look per band of tiers
 *  (whites / yellows / low reds / high reds), and the special kinds NeverSink handles first.
 *  Everything is saved as tier edits, so it survives NeverSink updates. */
export default function MapTiers({ profileId, onChange, onOpen, focus }: { profileId: string; onChange: () => void;
  onOpen: (s: MapSection) => void;
  /** A special map kind to scroll to and make glow ("maps>blighted"), from "Modify this rule". */
  focus?: string | null }) {
  const [data, setData] = useState<MapData | null>(null);
  const [msg, setMsg] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () => invoke<MapData>("get_map_tiers", { id: profileId }).then(setData).catch((e) => setMsg(String(e)));
  useEffect(() => { load(); }, [profileId]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!focus || !data) return;
    window.setTimeout(() => document.querySelector(`[data-special="${CSS.escape(focus)}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" }), 80);
  }, [focus, data]);

  const change = async (typePath: string[], tiers: string[], patch: { show?: boolean; style?: unknown; reset?: boolean; clear?: boolean }, m: string) => {
    if (tiers.length === 0) return;
    setBusy(true);
    try {
      await invoke("set_tiers", { id: profileId, typePath, tiers, show: patch.show ?? null, style: patch.style ?? null, reset: !!patch.reset, clearStyle: !!patch.clear });
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
      <h2>Maps on the floor <Tip>Everything here changes NeverSink's map rules for this filter and survives NeverSink updates.
        Press <b>F4</b> in game after a change to see it.</Tip></h2>
      <p className="muted small mt-intro">Which maps show on the ground and how they look. Press F4 in game after a change.</p>

      <h3 className="mt-h">1 · Which tiers show</h3>
      <p className="mt-what">Maps below the tier you pick are hidden on the ground. Special maps (section 4) still show.</p>
      <div className="mt-from">
        <span>Show maps from</span>
        <select value={from > 16 ? "" : from} disabled={busy} onChange={(e) => setFrom(parseInt(e.target.value, 10))}>
          {from > 16 && <option value="">(all hidden)</option>}
          {data.tiers.map((t) => <option key={t.tier} value={t.map_tier}>tier {t.map_tier}</option>)}
        </select>
        <span>and up</span>
        <span className="muted small">{from <= 1 ? "Every tier shows." : from > 16 ? "" : `Tiers 1–${from - 1} are hidden.`}</span>
      </div>

      <p className="mt-what">Or click a tier to hide or show just that one (crossed out = hidden).</p>
      <div className="mt-strip" role="group" aria-label="Map tiers">
        {data.tiers.map((t) => (
          <button key={t.tier} className={`mt-chip band-${t.band} ${shown(t) ? "on" : "off"}`} disabled={busy}
            title={`${shown(t) ? "Shown" : "Hidden"}: click to ${shown(t) ? "hide" : "show"} tier ${t.map_tier} maps`}
            onClick={() => change(MAPS, [t.tier], { show: !shown(t) }, `Tier ${t.map_tier} maps ${shown(t) ? "hidden" : "shown"}.`)}>
            {t.map_tier}
          </button>
        ))}
      </div>

      <h3 className="mt-h">2 · How each group looks</h3>
      <p className="mt-what">Click a label to change its size, colours, beam, map icon and sound for every tier in that group. The tick hides or shows the whole group.</p>
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
              <LookButton className="tier-look" title="Change the look of every map in this group" onClick={() => setEditing(editing === b ? null : b)}>
                <Preview s={fromCurrent(rows[rows.length - 1].style, "Maps")} text={`Map (Tier ${rows[rows.length - 1].map_tier})`} />
              </LookButton>
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

      {(data.extras ?? []).length > 0 && <>
        <h3 className="mt-h">3 · Extra borders</h3>
        <p className="mt-what">NeverSink adds these borders on top of the looks above. Untick one to drop that border; the map itself still shows.</p>
        {(data.extras ?? []).map((x) => {
          const ek = "x:" + x.key;
          const t = data.tiers.find((r) => r.map_tier === (x.sample_tier ?? 14)) ?? data.tiers[data.tiers.length - 1];
          const base = t ? fromCurrent(t.style, "Maps") : null;
          const text = t ? `Map (Tier ${t.map_tier})` : "Map";
          return (
          <div key={x.key} className={"tier mt-extra " + (x.on ? "" : "off")}>
            <div className="tier-head">
              <label className="tier-name">
                <input type="checkbox" checked={x.on} disabled={busy}
                  onChange={(e) => change(MAPS, x.tiers, e.target.checked ? { reset: true } : { clear: true }, `${x.title} ${e.target.checked ? "back on" : "off"}.`)} />
                <span>{x.title}</span>
              </label>
              <span className="muted small mt-extra-what">{x.what}</span>
              {base && x.border && (
                // Example: a map of that tier with this border, and the same map without it. Click to change the border.
                <span className="mt-demo">
                  <span className="mt-demo-one"><LookButton hint="edit border" title="Click to change this border's colour" onClick={() => setEditing(editing === ek ? null : ek)}>
                    <Preview s={{ ...base, border: x.border }} text={text} /></LookButton><span className="muted small">with</span></span>
                  <span className="mt-demo-one"><Preview s={{ ...base, border: null }} text={text} /><span className="muted small">without</span></span>
                </span>
              )}
            </div>
            {editing === ek && base && (
              <div className="qm tier-edit">
                <p className="mt-what">Only the <b>border</b> counts here: the map's own group look (step 2) sets everything else.</p>
                <StyleEditor start={{ ...base, border: x.border ?? base.border }} label={text}
                  onCancel={() => setEditing(null)}
                  onSave={(st) => { setEditing(null); change(MAPS, x.tiers, { style: st }, `${x.title}: new border.`); }} />
              </div>
            )}
          </div>
          );
        })}
      </>}

      {data.special.length > 0 && <>
        <h3 className="mt-h">4 · Special maps</h3>
        <p className="mt-what">NeverSink shows these before the tier rules, so they show even when their tier is hidden. Untick a kind to hide it; Open shows its separate rules.</p>
        {data.special.map((s) => {
          const all = s.tiers.every(shown), none = !s.tiers.some(shown);
          // Originator maps share NeverSink's "influenced" rules but get their own row ("maps>influenced>zana").
          const zana = s.tiers.every((t) => t.tier.startsWith("zana"));
          // Rows shown band by band with a "plain map" example: Originator and citadel (conqueror) maps.
          const citadel = s.type_path[1] === "implicitmod";
          const bands = zana || citadel;
          const rk = s.type_path.join(">") + (zana ? ">zana" : "");
          const sk = "s:" + rk;
          return (
            <div key={rk} data-special={rk} className={`tier mt-special ${none ? "off" : ""} ${focus === rk ? "focus" : ""}`}>
              <div className="tier-head">
                <label className="tier-name">
                  <input type="checkbox" checked={all} disabled={busy}
                    ref={(el) => { if (el) el.indeterminate = !all && !none; }}
                    onChange={(e) => change(s.type_path, s.tiers.map((r) => r.tier), { show: e.target.checked }, `${s.title} ${e.target.checked ? "shown" : "hidden"}.`)} />
                  <span>{s.title}</span>
                </label>
                {!bands && <LookButton className="tier-look" title={`Change the look of every rule in ${s.title}`} onClick={() => setEditing(editing === sk ? null : sk)}>
                  <Preview s={fromCurrent(s.tiers[0].style, "Maps")} text="Map" /></LookButton>}
                <span className="tier-tools">
                  <button className="link small" onClick={() => onOpen(s)}>Open</button>
                </span>
              </div>
              {!bands && <div className="muted small mt-special-rules">{s.tiers.map((r) => r.name).join(" · ")}</div>}
              {zana && <p className="mt-what mt-special-rules">Maps from the Originator's memories. Their label only says
                "Map (Tier N)"; NeverSink gives them this look instead of the plain tier look. Click a look to change it.</p>}
              {citadel && <p className="mt-what mt-special-rules">Maps with a conqueror's citadel ("Map contains Drox's Citadel",
                Veritania's, Baran's, Al-Hezmin's). NeverSink ships this rule switched off; the app switches it on so they stand out.
                Click a look to change it.</p>}
              {bands && s.tiers.map((r) => {
                // Example: this rule's look ("with") next to the plain map look of a tier it covers ("without").
                const t = r.tier === "zanamap" || r.tier === "t1" ? 16 : 10;
                const plain = data.tiers.find((x) => x.map_tier === t);
                const text = `Map (Tier ${t})`;
                const ek = "z:" + rk + ":" + r.tier;
                return (
                  <div key={r.tier} className={`mt-zana ${shown(r) ? "" : "off"}`}>
                    <span className="mt-zana-name">{r.name}</span>
                    <span className="mt-demo">
                      <span className="mt-demo-one"><LookButton title={`Change the look of ${r.name}`} onClick={() => setEditing(editing === ek ? null : ek)}>
                        <Preview s={fromCurrent(r.style, "Maps")} text={text} /></LookButton><span className="muted small">{citadel ? "Citadel" : "Originator"}</span></span>
                      {plain && <span className="mt-demo-one"><Preview s={fromCurrent(plain.style, "Maps")} text={text} /><span className="muted small">plain map</span></span>}
                    </span>
                    {editing === ek && (
                      <div className="qm tier-edit">
                        <StyleEditor start={fromCurrent(r.style, "Maps")} label={text}
                          onCancel={() => setEditing(null)}
                          onSave={(st) => { setEditing(null); change(s.type_path, [r.tier], { style: st }, `${r.name} restyled.`); }} />
                      </div>
                    )}
                  </div>
                );
              })}
              {editing === sk && (
                <div className="qm tier-edit">
                  <StyleEditor start={fromCurrent(s.tiers[0].style, "Maps")} label="Map"
                    onCancel={() => setEditing(null)}
                    onSave={(st) => { setEditing(null); change(s.type_path, s.tiers.map((r) => r.tier), { style: st }, `${s.title} restyled.`); }} />
                </div>
              )}
            </div>
          );
        })}
      </>}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
