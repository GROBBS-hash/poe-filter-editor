import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Tip } from "./Help";
import { Overview, ProfileInfo, STRICTNESS_NAMES } from "./types";

/** "Built on": NeverSink at a strictness (kept up to date), or your own file. */
function BaseChooser({ p, dir, onChange, onMsg }: { p: ProfileInfo; dir: string; onChange: () => void; onMsg: (m: string) => void }) {
  const [busy, setBusy] = useState(false);
  const value = p.strictness != null ? String(p.strictness) : "file";
  const pick = async (v: string) => {
    setBusy(true);
    try {
      if (v === "choose") {
        const f = await open({ defaultPath: dir, filters: [{ name: "Loot filter", extensions: ["filter"] }] });
        if (typeof f !== "string") return;
        await invoke("set_base", { id: p.id, strictness: null, filePath: f });
        onMsg(`“${p.name}” is now built on your file. Press F4 in game to load it.`);
      } else if (v !== "file") {
        onMsg(`Downloading NeverSink's ${STRICTNESS_NAMES[+v]} filter…`);
        await invoke("set_base", { id: p.id, strictness: +v, filePath: null });
        onMsg(`“${p.name}” is now NeverSink ${STRICTNESS_NAMES[+v]}, with your edits, prices and theme kept. Press F4 in game.`);
      }
      onChange();
    } catch (e) { onMsg(String(e)); } finally { setBusy(false); }
  };
  const fileName = p.strictness == null ? p.base_file.split(/[\\/]/).pop() : null;
  return (
    <select className="base-pick" value={value} disabled={busy} onChange={(e) => pick(e.target.value)}>
      <optgroup label="NeverSink (updates automatically)">
        {STRICTNESS_NAMES.map((n, i) => <option key={i} value={String(i)}>{n}</option>)}
      </optgroup>
      <optgroup label="Your own file">
        {fileName && <option value="file">{fileName}</option>}
        <option value="choose">Choose a file (e.g. from FilterBlade)…</option>
      </optgroup>
    </select>
  );
}

export default function Profiles({ ov, active, onChange, onNew }: { ov: Overview; active: ProfileInfo; onChange: () => void; onNew: () => void }) {
  const [name, setName] = useState("");
  const [inherit, setInherit] = useState(true);
  const [msg, setMsg] = useState("");

  const fork = async () => {
    try {
      await invoke("fork_profile", { from: active.id, name: name.trim(), inherit });
      setMsg(`Saved “${name.trim()}”. It's now your active filter.`);
      setName("");
      onChange();
    } catch (e) { setMsg(String(e)); }
  };
  const del = async (id: string) => {
    if (!confirm("Delete this profile? Its filter file stays in your game folder.")) return;
    await invoke("delete_profile", { id }).catch((e) => setMsg(String(e)));
    onChange();
  };
  const nameOf = (id?: string | null) => ov.profiles.find((p) => p.id === id)?.name;

  return (
    <>
      <div className="card">
        <div className="row between">
          <h2>Your filters <Tip><b>Going stricter?</b> Change <b>Built on</b> to e.g. Uber Strict: your edits, tier changes, levelling setup,
            price tiers and theme all carry over. <b>Want both?</b> Save as new first, then change the copy, and switch between them in
            game with F4. <b>FilterBlade file?</b> Choose it under “Your own file”; your edits go on top of it.</Tip></h2>
          <button onClick={onNew}>New filter (e.g. for PoE 2)…</button>
        </div>
        <p className="muted small">Change how strict a filter is any time. Your edits, price tiers and theme carry over.
          Picking NeverSink here downloads his latest version and keeps it updated; picking a FilterBlade file keeps your FilterBlade choices instead.</p>
        <ul className="rules profiles-list">
          {ov.profiles.map((p) => (
            <li key={p.id}>
              <span>
                <b>{p.name}</b> <span className="muted small">
                  {p.game === "Poe1" ? "PoE 1" : "PoE 2"}{p.base_version ? ` · NeverSink ${p.base_version}` : ""}
                  {p.parent ? ` · follows ${nameOf(p.parent)}` : ""} · {p.rules.length} edits{p.theme ? ` · ${p.theme}` : ""}
                </span>
              </span>
              <span className="row">
                <BaseChooser p={p} dir={p.game === "Poe1" ? ov.poe1_dir : ov.poe2_dir} onChange={onChange} onMsg={setMsg} />
                <button className="link" onClick={() => del(p.id)}>Delete</button>
              </span>
            </li>
          ))}
        </ul>
        {msg && <div className="note">{msg}</div>}
      </div>
      <div className="card">
        <h2>Save “{active.name}” as a new filter</h2>
        <p className="muted">Great for going stricter as the league goes on, without touching your league-start filter.</p>
        <input className="text" placeholder="e.g. Strict 1" value={name} onChange={(e) => setName(e.target.value)} />
        <label className="check">
          <input type="checkbox" checked={inherit} onChange={(e) => setInherit(e.target.checked)} />
          Keep following “{active.name}” (fixes you make there carry over)
        </label>
        <button className="primary" disabled={!name.trim()} onClick={fork}>Save as new</button>
      </div>

    </>
  );
}
