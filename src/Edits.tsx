import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { ProfileInfo } from "./types";
import ItemPanel, { RuleDetail, itemFromRule } from "./ItemPanel";

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
                <div className="rule-edit qm">
                  <ItemPanel key={r.index} inGame={false}
                    start={{ item: itemFromRule(open.detail), from_screen: false, screen_text: null, confidence: 1 }}
                    replacing={{ id: profile.id, index: r.index, detail: open.detail }}
                    onClose={() => setOpen(null)} onChanged={onChange} />
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
