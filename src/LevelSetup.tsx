import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GAME_COLOURS } from "./StyleEditor";
import { standOut } from "./Find";

type Build = { id: string; title: string; keep: string[]; armour: string[] };
type Opts = {
  build: string; keep: string[]; armour_types: string[]; vendor_rares: number[];
  aggressive: boolean; minion: boolean; rares_from: number; hide_armour: boolean; progression: string[];
};
type State = { builds: Build[]; weapons: string[]; armour_types: [string, string][]; vendor_ranges: [number, number][]; current: Opts | null };

const CHIPS = ["Purple", "Yellow", "Cyan", "Red", "Green", "Orange", "Pink", "White"];
const DEFAULT: Opts = { build: "custom", keep: [], armour_types: [], vendor_rares: [0, 1], aggressive: false, minion: false, rares_from: 16, hide_armour: false, progression: [] };

/** Like FilterBlade's "Auto-Adjust: Campaign": pick a build, weapon and armour types, and what to keep for vendoring. */
export default function LevelSetup({ onDone }: { onDone: (msg: string) => void }) {
  const [st, setSt] = useState<State | null>(null);
  const [o, setO] = useState<Opts>(DEFAULT);
  const [colour, setColour] = useState("Cyan");
  const [err, setErr] = useState("");

  const load = () => invoke<State>("list_level_presets").then((s) => {
    setSt(s);
    if (s.current) {
      // Empty armour list means "all".
      setO({ ...DEFAULT, ...s.current, armour_types: s.current.armour_types.length ? s.current.armour_types : s.armour_types.map((a) => a[0]) });
    } else {
      setO({ ...DEFAULT, armour_types: s.armour_types.map((a) => a[0]) });
    }
  }).catch((e) => setErr(String(e)));
  useEffect(() => { load(); }, []);
  if (!st) return null;

  const set = (p: Partial<Opts>) => setO({ ...o, ...p });
  const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);
  const pickBuild = (b: Build) => set({
    build: b.id,
    keep: b.id === "custom" ? o.keep : b.keep,
    armour_types: b.id === "custom" ? o.armour_types : b.armour,
  });
  // Changing a tick by hand makes it a custom build.
  const custom = (p: Partial<Opts>) => set({ ...p, build: "custom" });

  const apply = async (off = false) => {
    setErr("");
    try {
      const all = st.armour_types.length === o.armour_types.length;
      await invoke("apply_level_preset", { options: off ? null : { ...o, armour_types: all ? [] : o.armour_types, hide_armour: o.hide_armour && o.rares_from > 0 }, style: standOut(colour) });
      await load();
      onDone(off ? "Levelling setup turned off. Press F4 in game."
        : "Levelling setup applied: your picks stand out until maps, everything else is hidden. Press F4 in game.");
    } catch (e) { setErr(String(e)); }
  };

  return (
    <div className="card lvl">
      <div className="row between">
        <h2>Levelling setup</h2>
        {st.current && <button className="link" onClick={() => apply(true)}>Turn off</button>}
      </div>
      <p className="muted small">Like FilterBlade's “Auto-Adjust: Campaign”. Only until maps (area level 68); uniques always show,
        and linked items too (3-links until area 28, 4-links always) unless you hide unpicked gear aggressively.</p>

      <div className="lvl-builds">
        {st.builds.map((b) => (
          <button key={b.id} className={`lvl-tile ${o.build === b.id ? "on" : ""}`} onClick={() => pickBuild(b)}>{b.title}</button>
        ))}
      </div>

      <div className="se-label">Weapons to keep</div>
      <div className="lvl-grid">
        {st.weapons.map((w) => (
          <button key={w} className={`lvl-tile ${o.keep.includes(w) ? "on" : ""}`} onClick={() => custom({ keep: toggle(o.keep, w) })}>{w}</button>
        ))}
      </div>

      <div className="se-label">Armour types to keep</div>
      <div className="lvl-grid six">
        {st.armour_types.map(([id, name]) => (
          <button key={id} className={`lvl-tile ${o.armour_types.includes(id) ? "on" : ""}`}
            onClick={() => custom({ armour_types: toggle(o.armour_types, id) })}>{name}</button>
        ))}
      </div>

      <div className="se-label">Item progression <span className="muted small">(pick up to 3: the newest white/blue base of each stands out as you level, e.g. Quartz Wand at 18 replacing Carved Wand)</span></div>
      <div className="lvl-grid">
        {st.weapons.filter((w) => !["Quivers", "Shields"].includes(w)).map((w) => {
          const on = (o.progression ?? []).includes(w);
          const full = !on && (o.progression ?? []).length >= 3;
          return (
            <button key={w} disabled={full} className={`lvl-tile ${on ? "on" : ""}`}
              onClick={() => set({ progression: on ? o.progression.filter((x) => x !== w) : [...(o.progression ?? []), w] })}>{w}</button>
          );
        })}
      </div>

      <div className="se-label">Extras</div>
      <div className="lvl-grid six">
        {st.vendor_ranges.map(([a, b], i) => (
          <button key={i} title="Still show rares of the gear you didn't pick, to sell to vendors"
            className={`lvl-tile ${o.vendor_rares.includes(i) ? "on" : ""}`}
            onClick={() => set({ vendor_rares: o.vendor_rares.includes(i) ? o.vendor_rares.filter((x) => x !== i) : [...o.vendor_rares, i] })}>
            Vendor rares: lvl {a}–{b}
          </button>
        ))}
        <button className={`lvl-tile ${o.minion ? "on" : ""}`} title="Convoking Wand, Bone Helmet and other minion bases always stand out"
          onClick={() => set({ minion: !o.minion })}>Minion focused</button>
        <button className={`lvl-tile ${o.aggressive ? "on" : ""}`} title="Also hide linked items of the gear you didn't pick"
          onClick={() => set({ aggressive: !o.aggressive })}>Hide unselected gear aggressively</button>
      </div>

      <div className="row">
        <span>White & blue weapons:</span>
        <select value={o.rares_from} onChange={(e) => set({ rares_from: +e.target.value })}>
          <option value={0}>show the whole campaign</option>
          {[8, 12, 16, 24, 28, 45].map((n) => <option key={n} value={n}>until area level {n}{n === 16 ? " (NeverSink’s choice)" : ""}, then rares only</option>)}
        </select>
      </div>
      {o.rares_from > 0 && (
        <label className="tick">
          <input type="checkbox" checked={o.hide_armour} onChange={(e) => set({ hide_armour: e.target.checked })} />
          Also hide white & blue armour (body, helmet, gloves, boots) from area level {o.rares_from}
        </label>
      )}
      <div className="row">
        <span>Your picks stand out in</span>
        <span className="se-chips">
          {CHIPS.map((c) => (
            <button key={c} title={c} className={colour === c ? "se-chip on" : "se-chip"} style={{ background: GAME_COLOURS[c] }} onClick={() => setColour(c)} />
          ))}
        </span>
      </div>
      <div className="row">
        <button className="primary" onClick={() => apply()} disabled={o.keep.length === 0 && o.armour_types.length === st.armour_types.length}>
          {st.current ? "Update" : "Apply"}
        </button>
        {err && <span className="note">{err}</span>}
      </div>
    </div>
  );
}
