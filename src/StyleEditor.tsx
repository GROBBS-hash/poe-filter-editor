import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Style as the filter currently shows it (from the status check). */
export type CurrentStyle = {
  font_size?: number | null; text?: number[] | null; border?: number[] | null; background?: number[] | null;
  muted: boolean; beam?: string | null; beam_temp?: boolean; sound_id?: string | null; sound_file?: string | null;
  volume?: number | null; minimap_parts?: [number, string, string] | null; label_hidden?: boolean;
};

type Sound =
  | { kind: "Default" } | { kind: "Off" }
  | { kind: "Builtin"; id: string; volume: number }
  | { kind: "Custom"; file: string; volume: number };

export type CustomStyle = {
  font_size: number; text: number[]; border: number[] | null; background: number[];
  beam: { colour: string; temp: boolean } | null;
  minimap: { size: number; colour: string; shape: string } | null;
  sound: Sound;
  /** Icon & beam only: the label is shrunk and see-through. */
  label_hidden: boolean;
};

export const GAME_COLOURS: Record<string, string> = {
  Red: "#ff4040", Green: "#40e040", Blue: "#4080ff", Brown: "#a0642a", White: "#ffffff", Yellow: "#ffe040",
  Cyan: "#40f0f0", Grey: "#a0a0a0", Orange: "#ff9020", Pink: "#ff70c0", Purple: "#b050ff",
};
const SHAPES = ["Circle", "Diamond", "Hexagon", "Square", "Star", "Triangle", "Cross", "Moon", "Raindrop", "Kite", "Pentagon", "UpsideDownHouse"];
const NAMED = ["ShAlchemy", "ShBlessed", "ShChaos", "ShDivine", "ShExalted", "ShFusing", "ShGeneral", "ShMirror", "ShRegal", "ShVaal"];
const RARITY_TEXT: Record<string, number[]> = {
  Normal: [200, 200, 200, 255], Magic: [136, 136, 255, 255], Rare: [255, 255, 119, 255], Unique: [175, 96, 37, 255],
  Gem: [27, 162, 155, 255], Currency: [170, 158, 130, 255],
};

/** Start the editor from exactly what the filter shows today. */
export function fromCurrent(st: CurrentStyle | undefined, rarity: string): CustomStyle {
  const p = st?.minimap_parts;
  const hidden = !!st?.label_hidden;
  // An icon-only label is stored see-through; give it visible colours to come back to.
  const solid = (c: number[] | null | undefined, fallback: number[]) => (c ? [c[0], c[1], c[2], hidden ? 255 : c[3] ?? 255] : fallback);
  return {
    label_hidden: hidden,
    font_size: hidden ? 36 : st?.font_size ?? 32,
    text: solid(st?.text, RARITY_TEXT[rarity] ?? [200, 200, 200, 255]),
    border: hidden ? null : st?.border ?? null,
    background: hidden ? [0, 0, 0, 240] : st?.background ?? [0, 0, 0, 240],
    beam: st?.beam ? { colour: st.beam, temp: !!st.beam_temp } : null,
    minimap: p ? { size: p[0], colour: p[1], shape: p[2] } : null,
    sound: st?.muted ? { kind: "Off" }
      : st?.sound_id ? { kind: "Builtin", id: st.sound_id, volume: st.volume ?? 300 }
      : st?.sound_file ? { kind: "Custom", file: st.sound_file, volume: st.volume ?? 300 }
      : { kind: "Default" },
  };
}

const hex = (c: number[]) => "#" + c.slice(0, 3).map((n) => n.toString(16).padStart(2, "0")).join("");
const fromHex = (h: string, a = 255) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(a);
export const css = (c?: number[] | null) => (c ? `rgba(${c[0]},${c[1]},${c[2]},${(c[3] ?? 255) / 255})` : "transparent");

export function Preview({ s, text }: { s: CustomStyle; text: string }) {
  if (s.label_hidden) {
    return <span className="se-preview se-ghost">no label · {[s.beam && `${s.beam.colour} beam`, s.minimap && "map icon"].filter(Boolean).join(" + ") || "nothing visible!"}</span>;
  }
  return (
    <span className="se-preview" style={{
      color: css(s.text), borderColor: css(s.border), background: css(s.background),
      fontSize: Math.max(11, Math.min(22, s.font_size / 2.1)),
    }}>{text}</span>
  );
}

function Chips({ value, onPick, allowOff }: { value: string | null; onPick: (v: string | null) => void; allowOff?: boolean }) {
  return (
    <div className="se-chips">
      {allowOff && <button className={value === null ? "se-off on" : "se-off"} onClick={() => onPick(null)}>Off</button>}
      {Object.entries(GAME_COLOURS).map(([name, c]) => (
        <button key={name} title={name} className={value?.toLowerCase() === name.toLowerCase() ? "se-chip on" : "se-chip"}
          style={{ background: c }} onClick={() => onPick(name)} />
      ))}
    </div>
  );
}

export default function StyleEditor({ start, label, onSave, onCancel }: {
  start: CustomStyle; label: string; onSave: (s: CustomStyle) => void; onCancel: () => void;
}) {
  const [s, setS] = useState<CustomStyle>(start);
  const [files, setFiles] = useState<string[]>([]);
  const audio = useRef<HTMLAudioElement | null>(null);
  const set = (patch: Partial<CustomStyle>) => setS((x) => ({ ...x, ...patch }));

  useEffect(() => { invoke<string[]>("list_sounds").then(setFiles).catch(() => setFiles([])); }, []);

  const soundValue =
    s.sound.kind === "Builtin" ? `b:${s.sound.id}` : s.sound.kind === "Custom" ? `c:${s.sound.file}` : s.sound.kind;
  const volume = s.sound.kind === "Builtin" || s.sound.kind === "Custom" ? s.sound.volume : 0;
  const setSound = (v: string) => {
    const vol = volume || 300;
    if (v.startsWith("b:")) set({ sound: { kind: "Builtin", id: v.slice(2), volume: vol } });
    else if (v.startsWith("c:")) set({ sound: { kind: "Custom", file: v.slice(2), volume: vol } });
    else set({ sound: { kind: v as "Default" | "Off" } });
  };
  const preview = async () => {
    if (s.sound.kind !== "Custom") return;
    try {
      audio.current?.pause();
      audio.current = new Audio(await invoke<string>("sound_data", { file: s.sound.file }));
      audio.current.volume = Math.min(1, volume / 300);
      await audio.current.play();
    } catch { /* file missing or unplayable */ }
  };

  return (
    <div className="se">
      <div className="se-top"><Preview s={s} text={label} /></div>

      <div className="se-row se-mode">
        <button className={!s.label_hidden ? "on" : ""} onClick={() => set({ label_hidden: false })}>Label shown</button>
        <button className={s.label_hidden ? "on" : ""} onClick={() => set({
          label_hidden: true,
          // Make sure there's something to see.
          beam: s.beam ?? { colour: "White", temp: false },
          minimap: s.minimap ?? { size: 1, colour: "White", shape: "Circle" },
        })}>Icon & beam only</button>
      </div>
      {s.label_hidden && (
        <div className="se-hint">No label on screen; you'll see just the beam and minimap icon below.
          {!s.beam && !s.minimap && <b> Pick a beam or icon, or you won't see it at all.</b>}</div>
      )}

      {!s.label_hidden && <>
      <div className="se-row">
        <span>Text</span>
        <input type="color" value={hex(s.text)} onChange={(e) => set({ text: fromHex(e.target.value) })} />
        <span>Border</span>
        <input type="color" value={hex(s.border ?? [0, 0, 0])} disabled={!s.border}
          onChange={(e) => set({ border: fromHex(e.target.value) })} />
        <input type="checkbox" title="Show a border" checked={!!s.border}
          onChange={(e) => set({ border: e.target.checked ? [255, 255, 255, 255] : null })} />
        <span>Back</span>
        <input type="color" value={hex(s.background)}
          onChange={(e) => set({ background: fromHex(e.target.value, s.background[3]) })} />
      </div>

      <div className="se-row">
        <span>Size</span>
        <input type="range" min={18} max={45} value={s.font_size} onChange={(e) => set({ font_size: +e.target.value })} />
        <span className="se-num">{s.font_size}</span>
      </div>
      </>}

      <div className="se-label">Beam {s.beam && (
        <label className="se-inline"><input type="checkbox" checked={s.beam.temp}
          onChange={(e) => set({ beam: { ...s.beam!, temp: e.target.checked } })} /> only while dropping</label>
      )}</div>
      <Chips value={s.beam?.colour ?? null} allowOff
        onPick={(c) => set({ beam: c ? { colour: c, temp: s.beam?.temp ?? false } : null })} />

      <div className="se-label">Map icon</div>
      <Chips value={s.minimap?.colour ?? null} allowOff
        onPick={(c) => set({ minimap: c ? { size: s.minimap?.size ?? 1, shape: s.minimap?.shape ?? "Star", colour: c } : null })} />
      {s.minimap && (
        <div className="se-row">
          <select value={s.minimap.shape} onChange={(e) => set({ minimap: { ...s.minimap!, shape: e.target.value } })}>
            {SHAPES.map((x) => <option key={x} value={x}>{x === "UpsideDownHouse" ? "Upside-down house" : x}</option>)}
          </select>
          <select value={s.minimap.size} onChange={(e) => set({ minimap: { ...s.minimap!, size: +e.target.value } })}>
            <option value={0}>Large</option><option value={1}>Medium</option><option value={2}>Small</option>
          </select>
        </div>
      )}

      <div className="se-label">Sound</div>
      <div className="se-row">
        <select className="se-grow" value={soundValue} onChange={(e) => setSound(e.target.value)}>
          <option value="Default">Normal drop sound</option>
          <option value="Off">Off (silent)</option>
          <optgroup label="Game sounds">
            {Array.from({ length: 16 }, (_, i) => <option key={i} value={`b:${i + 1}`}>Sound {i + 1}</option>)}
            {NAMED.map((n) => <option key={n} value={`b:${n}`}>{n.slice(2)}</option>)}
          </optgroup>
          {files.length > 0 && (
            <optgroup label="Your sound files">
              {files.map((f) => <option key={f} value={`c:${f}`}>{f}</option>)}
            </optgroup>
          )}
        </select>
        {s.sound.kind === "Custom" && <button onClick={preview} title="Play">▶</button>}
      </div>
      {(s.sound.kind === "Builtin" || s.sound.kind === "Custom") && (
        <div className="se-row">
          <span>Volume</span>
          <input type="range" min={0} max={300} step={10} value={volume}
            onChange={(e) => set({ sound: { ...(s.sound as { kind: "Builtin"; id: string; volume: number }), volume: +e.target.value } })} />
          <span className="se-num">{Math.round((volume / 300) * 100)}%</span>
        </div>
      )}
      {s.sound.kind === "Builtin" && <div className="se-hint">Game sounds play in game; they can't be previewed here.</div>}

      <div className="se-actions">
        <button onClick={onCancel}>Cancel</button>
        <button className="primary" onClick={() => onSave(s)}>Save look</button>
      </div>
    </div>
  );
}
