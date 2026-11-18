import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Overview, ProfileInfo } from "./types";

export default function Profiles({ ov, active, onChange }: { ov: Overview; active: ProfileInfo; onChange: () => void }) {
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
        <h2>Save “{active.name}” as a new filter</h2>
        <p className="muted">Great for going stricter as the league goes on, without touching your league-start filter.</p>
        <input className="text" placeholder="e.g. Strict 1" value={name} onChange={(e) => setName(e.target.value)} />
        <label className="check">
          <input type="checkbox" checked={inherit} onChange={(e) => setInherit(e.target.checked)} />
          Keep following “{active.name}” (fixes you make there carry over)
        </label>
        <button className="primary" disabled={!name.trim()} onClick={fork}>Save as new</button>
        {msg && <div className="note">{msg}</div>}
      </div>

      <div className="card">
        <h2>Your filters</h2>
        <ul className="rules">
          {ov.profiles.map((p) => (
            <li key={p.id}>
              <span>
                <b>{p.name}</b> <span className="muted small">
                  {p.game === "Poe1" ? "PoE 1" : "PoE 2"} · built on {p.base_file}
                  {p.parent ? ` · follows ${nameOf(p.parent)}` : ""} · {p.rules.length} edits
                </span>
              </span>
              <button className="link" onClick={() => del(p.id)}>Delete</button>
            </li>
          ))}
        </ul>
      </div>
    </>
  );
}
