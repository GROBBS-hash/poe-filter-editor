import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProfileInfo } from "./types";

type LevelLook = { text: number[]; border: number[] | null; background: number[] };
type Theme = { name: string; levels: LevelLook[]; classic?: Record<string, number[]> | null; neversink_style?: string | null; game?: string | null };

/** Classic style, same maths as the app's filter builder: the item kind's
 *  colour, more solid and brighter the better the item. */
const CATEGORIES = ["Unique", "Rare", "Magic", "Normal", "Currency", "Divination", "Gem", "Fractured", "Top base", "Map", "Quest"];
function classicLook(hue: number[], i: number): LevelLook {
  const tint = [0.42, 0.30, 0.20, 0.11, 0.05, 0.0][i];
  const alpha = [255, 245, 230, 210, 180, 140][i];
  const shade = [0.45, 0.25, 0.10, 0.0, -0.22, -0.42][i];
  const mix = (c: number, k: number) => Math.max(0, Math.min(255, Math.round(k >= 0 ? c + (255 - c) * k : c * (1 + k))));
  const text = [mix(hue[0], shade), mix(hue[1], shade), mix(hue[2], shade), 255];
  const bg = [Math.floor(hue[0] * tint), Math.floor(hue[1] * tint), Math.floor(hue[2] * tint), alpha];
  const border = i === 0 ? [255, 255, 255, 255]
    : i === 1 ? [mix(hue[0], 0.3), mix(hue[1], 0.3), mix(hue[2], 0.3), 255]
    : i === 2 ? [hue[0], hue[1], hue[2], 255]
    : i === 3 ? [hue[0], hue[1], hue[2], 140] : null;
  return { text, border, background: bg };
}
/** Six examples showing off a classic theme: [text, kind, level]. */
const CLASSIC_SAMPLES: [string, string, number][] = [
  ["Headhunter", "Unique", 0], ["The Doctor", "Divination", 0], ["Divine Orb", "Currency", 1],
  ["Fractured Vise", "Fractured", 2], ["Rare Ring", "Rare", 3], ["Plain Unique", "Unique", 4],
];

const LEVELS = [
  { name: "Chase", example: "Mirror of Kalandra" },
  { name: "High", example: "Divine Orb" },
  { name: "Good", example: "Chaos Orb" },
  { name: "Medium", example: "Stygian Vise" },
  { name: "Low", example: "Rare Ring" },
  { name: "Junk", example: "Scroll of Wisdom" },
];
const NEVERSINK: Theme = { name: "", levels: [
  { text: [255, 0, 0, 255], border: [255, 0, 0, 255], background: [255, 255, 255, 255] },
  { text: [255, 255, 255, 255], border: [255, 255, 255, 255], background: [240, 90, 35, 255] },
  { text: [0, 0, 0, 255], border: [0, 0, 0, 255], background: [213, 159, 0, 255] },
  { text: [255, 255, 255, 255], border: [255, 255, 255, 255], background: [40, 40, 40, 255] },
  { text: [255, 255, 119, 255], border: null, background: [0, 0, 0, 220] },
  { text: [180, 180, 180, 255], border: null, background: [0, 0, 0, 180] },
] };

const PALETTES = ["Dracula", "Nord", "Catppuccin", "Gruvbox", "Tokyo Night", "Solarized", "Rosé Pine", "Monokai", "One Dark"];
const css = (c: number[] | null) => (c ? `rgba(${c[0]},${c[1]},${c[2]},${c[3] / 255})` : "transparent");
const hex = (c: number[]) => "#" + c.slice(0, 3).map((n) => n.toString(16).padStart(2, "0")).join("");
const fromHex = (h: string, a = 255) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).concat(a);

function Samples({ t }: { t: Theme }) {
  if (t.classic) {
    return (
      <div className="th-samples">
        {CLASSIC_SAMPLES.map(([text, kind, level]) => {
          const l = classicLook(t.classic![kind] ?? [210, 210, 210], level);
          return <span key={text} className="th-label" style={{ color: css(l.text), borderColor: css(l.border), background: css(l.background) }}>{text}</span>;
        })}
      </div>
    );
  }
  return (
    <div className="th-samples">
      {t.levels.map((l, i) => (
        <span key={i} className="th-label" style={{ color: css(l.text), borderColor: css(l.border), background: css(l.background) }}>
          {LEVELS[i].example}
        </span>
      ))}
    </div>
  );
}

function ThemeEditor({ start, isNew, onSave, onCancel }: {
  start: Theme; isNew: boolean; onSave: (t: Theme) => void; onCancel: () => void;
}) {
  const [t, setT] = useState<Theme>(JSON.parse(JSON.stringify(start)));
  const setLevel = (i: number, patch: Partial<LevelLook>) =>
    setT((x) => ({ ...x, levels: x.levels.map((l, j) => (j === i ? { ...l, ...patch } : l)) }));
  if (t.classic) {
    const setHue = (cat: string, h: string) => setT((x) => ({ ...x, classic: { ...x.classic!, [cat]: fromHex(h).slice(0, 3) } }));
    return (
      <div className="card">
        <h2>{isNew ? "New theme" : `Edit “${start.name}”`}</h2>
        <input className="text" placeholder="Theme name" value={t.name} onChange={(e) => setT({ ...t, name: e.target.value })} />
        <p className="muted small">Pick a colour for each kind of item. Better items get a more solid, brighter version of it (left = Chase, right = Junk).</p>
        <div className="th-edit">
          {CATEGORIES.map((cat) => {
            const hue = t.classic![cat] ?? [210, 210, 210];
            return (
              <div key={cat} className="th-edit-row">
                <span className="th-level wide">{cat}</span>
                <input type="color" value={hex(hue)} onChange={(e) => setHue(cat, e.target.value)} />
                <span className="th-strip">
                  {[0, 1, 2, 3, 4, 5].map((i) => {
                    const l = classicLook(hue, i);
                    return <span key={i} className="th-chip" title={LEVELS[i].name}
                      style={{ color: css(l.text), borderColor: css(l.border), background: css(l.background) }}>{LEVELS[i].name}</span>;
                  })}
                </span>
              </div>
            );
          })}
        </div>
        <div className="row between">
          <button onClick={onCancel}>Cancel</button>
          <button className="primary" disabled={!t.name.trim()} onClick={() => onSave(t)}>Save theme</button>
        </div>
      </div>
    );
  }

  return (
    <div className="card">
      <h2>{isNew ? "New theme" : `Edit “${start.name}”`}</h2>
      <input className="text" placeholder="Theme name" value={t.name} onChange={(e) => setT({ ...t, name: e.target.value })} />
      <div className="th-edit">
        {t.levels.map((l, i) => (
          <div key={i} className="th-edit-row">
            <span className="th-level">{LEVELS[i].name}</span>
            <span className="th-label" style={{ color: css(l.text), borderColor: css(l.border), background: css(l.background) }}>
              {LEVELS[i].example}
            </span>
            <label>Text <input type="color" value={hex(l.text)} onChange={(e) => setLevel(i, { text: fromHex(e.target.value) })} /></label>
            <label>Border <input type="checkbox" checked={!!l.border}
              onChange={(e) => setLevel(i, { border: e.target.checked ? [...l.text] : null })} />
              <input type="color" disabled={!l.border} value={hex(l.border ?? [0, 0, 0])}
                onChange={(e) => setLevel(i, { border: fromHex(e.target.value) })} /></label>
            <label>Back <input type="color" value={hex(l.background)}
              onChange={(e) => setLevel(i, { background: fromHex(e.target.value, l.background[3]) })} /></label>
            <label title="How see-through the background is">Solid
              <input type="range" min={60} max={255} value={l.background[3]}
                onChange={(e) => setLevel(i, { background: [...l.background.slice(0, 3), +e.target.value] })} /></label>
          </div>
        ))}
      </div>
      <div className="row between">
        <button onClick={onCancel}>Cancel</button>
        <button className="primary" disabled={!t.name.trim()} onClick={() => onSave(t)}>Save theme</button>
      </div>
    </div>
  );
}

export default function Themes({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [list, setList] = useState<{ builtin: Theme[]; custom: Theme[] } | null>(null);
  const [editing, setEditing] = useState<{ theme: Theme; oldName: string | null } | null>(null);
  const [msg, setMsg] = useState("");

  const load = () => invoke<{ builtin: Theme[]; custom: Theme[] }>("list_themes").then(setList).catch((e) => setMsg(String(e)));
  useEffect(() => { load(); }, []);

  const use = async (name: string | null) => {
    try {
      await invoke("set_profile_theme", { id: profile.id, name });
      setMsg(name ? `“${profile.name}” now uses ${name}. Press F4 in game to see it.` : `“${profile.name}” is back to NeverSink's own colours. Press F4 in game.`);
      onChange();
    } catch (e) { setMsg(String(e)); }
  };
  const save = async (t: Theme) => {
    try {
      await invoke("save_theme", { theme: t, oldName: editing?.oldName ?? null });
      setEditing(null);
      setMsg(`Saved “${t.name.trim()}”.`);
      load();
      onChange();
    } catch (e) { setMsg(String(e)); }
  };
  const remove = async (name: string) => {
    if (!confirm(`Delete the theme “${name}”? Filters using it go back to NeverSink's colours.`)) return;
    await invoke("delete_theme", { name }).catch((e) => setMsg(String(e)));
    load();
    onChange();
  };
  const copyOf = (t: Theme) => {
    const taken = new Set([...(list?.builtin ?? []), ...(list?.custom ?? [])].map((x) => x.name));
    let n = `${t.name || "NeverSink"} (mine)`, i = 2;
    while (taken.has(n)) n = `${t.name || "NeverSink"} (mine ${i++})`;
    // A copy of a NeverSink style becomes an editable six-level theme from its colours.
    setEditing({ theme: { ...JSON.parse(JSON.stringify(t)), name: n, neversink_style: null, game: null }, oldName: null });
  };

  if (editing) {
    return <ThemeEditor start={editing.theme} isNew={!editing.oldName} onSave={save} onCancel={() => setEditing(null)} />;
  }
  if (!list) return <div className="card">{msg || "Loading themes…"}</div>;

  const card = (t: Theme, kind: "neversink" | "builtin" | "custom") => {
    const name = kind === "neversink" ? null : t.name;
    const inUse = (profile.theme ?? null) === name;
    return (
      <div key={kind + t.name} className={`th-card ${inUse ? "on" : ""}`}>
        <div className="row between">
          <b>{kind === "neversink" ? "NeverSink's own colours" : t.name}</b>
          <span>
            {kind === "custom" && <>
              <button className="link" onClick={() => setEditing({ theme: t, oldName: t.name })}>Edit</button>
              <button className="link" onClick={() => remove(t.name)}>Delete</button>
            </>}
            <button className="link" onClick={() => copyOf(t)}>Copy & make my own</button>
          </span>
        </div>
        <Samples t={t} />
        <button className={inUse ? "primary wide" : "wide"} disabled={inUse} onClick={() => use(name)}>
          {inUse ? `Used by “${profile.name}”` : `Use for “${profile.name}”`}
        </button>
      </div>
    );
  };

  return (
    <>
      <div className="card">
        <h2>Themes for “{profile.name}”</h2>
        <p className="muted">A theme recolours NeverSink's labels by how valuable they are, from Chase down to Junk. Sizes, beams, icons and sounds stay the same, and your own edits and price tiers keep their own looks.</p>
        {msg && <div className="note">{msg}</div>}
      </div>
      {(() => {
        const game = profile.game === "Poe1" ? "poe1" : "poe2";
        const fits = (t: Theme) => !t.game || t.game === game;
        const ours = list.builtin.filter((t) => !t.neversink_style && !PALETTES.includes(t.name));
        const palettes = list.builtin.filter((t) => PALETTES.includes(t.name));
        const neversink = list.builtin.filter((t) => t.neversink_style && fits(t));
        return <>
          <div className="th-grid">
            {card(NEVERSINK, "neversink")}
            {list.custom.map((t) => card(t, "custom"))}
            {ours.map((t) => card(t, "builtin"))}
          </div>
          <h2 className="th-section">NeverSink's styles <span className="muted small">the same ones FilterBlade offers, copied rule by rule</span></h2>
          <div className="th-grid">{neversink.map((t) => card(t, "builtin"))}</div>
          <h2 className="th-section">Popular colour palettes <span className="muted small">from well-loved app and code-editor themes</span></h2>
          <div className="th-grid">{palettes.map((t) => card(t, "builtin"))}</div>
        </>;
      })()}
    </>
  );
}
