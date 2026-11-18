import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { ProfileInfo } from "./types";

export default function Edits({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [msg, setMsg] = useState("");
  const remove = async (index: number) => {
    await invoke("delete_rule", { id: profile.id, index }).catch((e) => setMsg(String(e)));
    onChange();
  };
  const rebuild = async () => {
    try {
      const w = await invoke<string[]>("rebuild", { id: profile.id });
      setMsg(w.length ? w.join(" · ") : "Rebuilt from the latest base filter");
    } catch (e) { setMsg(String(e)); }
  };

  return (
    <div className="card">
      <div className="row between">
        <h2>Edits in “{profile.name}”</h2>
        <button onClick={rebuild} title="Use after downloading a new NeverSink version">Rebuild</button>
      </div>
      <p className="muted">In game, select <b>{profile.output_file.replace(".filter", "")}</b> as your filter. After changes made here, press <b>F4</b> in game to reload it.</p>
      {profile.rules.length === 0 ? (
        <div className="empty">No edits yet. Hover an item in game and press F2.</div>
      ) : (
        <ul className="rules">
          {[...profile.rules].reverse().map((r) => (
            <li key={r.index}>
              <span>{r.label}</span>
              <button className="link" onClick={() => remove(r.index)}>Remove</button>
            </li>
          ))}
        </ul>
      )}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
