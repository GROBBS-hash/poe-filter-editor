import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { ProfileInfo } from "./types";
import RuleEditor, { RuleDetail, RuleEdit } from "./RuleEditor";

export default function Edits({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [msg, setMsg] = useState("");
  const [open, setOpen] = useState<{ index: number; detail: RuleDetail } | null>(null);

  const remove = async (index: number) => {
    await invoke("delete_rule", { id: profile.id, index }).catch((e) => setMsg(String(e)));
    setOpen(null);
    onChange();
  };
  const rebuild = async () => {
    try {
      const w = await invoke<string[]>("rebuild", { id: profile.id });
      setMsg(w.length ? w.join(" · ") : "Rebuilt from the latest base filter");
    } catch (e) { setMsg(String(e)); }
  };
  const edit = async (index: number) => {
    if (open?.index === index) return setOpen(null);
    try {
      const [, , detail] = await invoke<[string, number, RuleDetail]>("get_rule", { id: profile.id, index, label: null });
      setOpen({ index, detail });
    } catch (e) { setMsg(String(e)); }
  };
  const save = async (index: number, e: RuleEdit) => {
    try {
      const r = await invoke<{ label: string; warnings: string[] }>("update_rule", { id: profile.id, index, edit: e });
      setMsg(r.warnings[0] ?? `Saved “${r.label}”. Press F4 in game to reload.`);
      setOpen(null);
      onChange();
    } catch (err) { setMsg(String(err)); }
  };

  return (
    <div className="card">
      <div className="row between">
        <h2>Edits in “{profile.name}”</h2>
        <button onClick={rebuild} title="Use after downloading a new NeverSink version">Rebuild</button>
      </div>
      <p className="muted">In game, select <b>{profile.output_file.replace(".filter", "")}</b> as your filter (or press <b>F4</b> to switch to it). Newest edits are at the top and win over older ones.</p>
      {profile.rules.length === 0 ? (
        <div className="empty">No edits yet. Hover an item in game and press F2.</div>
      ) : (
        <ul className="rules">
          {[...profile.rules].reverse().map((r) => (
            <li key={r.index} className={open?.index === r.index ? "open" : ""}>
              <div className="rule-line">
                <span>{r.label}</span>
                <span>
                  <button className="link" onClick={() => edit(r.index)}>{open?.index === r.index ? "Close" : "Edit"}</button>
                  <button className="link" onClick={() => remove(r.index)}>Remove</button>
                </span>
              </div>
              {open?.index === r.index && (
                <div className="rule-edit">
                  <RuleEditor detail={open.detail} onSave={(e) => save(r.index, e)} onCancel={() => setOpen(null)} />
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
