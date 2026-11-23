// Saved looks: a look saved once under a name ("My chase look") that many edits can use. Changing the look
// changes every edit that uses it. Used by F2 (ItemPanel), Batch mode and the My edits page.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import StyleEditor, { CustomStyle, Preview } from "./StyleEditor";
import { friendly } from "./errors";
import "./looks.css";

export type SavedLook = { name: string; style: CustomStyle; used_by: number };

/** Tells every look list in this window to reload (after a look is saved, renamed or deleted). */
const CHANGED = "saved-looks-changed";
export const looksChanged = () => window.dispatchEvent(new Event(CHANGED));

export function useLooks(): [SavedLook[], () => void] {
  const [list, setList] = useState<SavedLook[]>([]);
  const load = () => { invoke<SavedLook[] | null>("list_looks").then((l) => setList(l ?? [])).catch(() => setList([])); };
  useEffect(() => {
    load();
    window.addEventListener(CHANGED, load);
    return () => window.removeEventListener(CHANGED, load);
  }, []);
  return [list, load];
}

/** A short line on what a look has besides its colours: "red beam · map icon · silent". */
export function lookWords(s: CustomStyle) {
  return [s.label_hidden && "icon only", s.beam && `${s.beam.colour.toLowerCase()} beam`, s.minimap && "map icon",
    s.sound.kind === "Off" ? "silent" : s.sound.kind !== "Default" ? "sound" : ""].filter(Boolean).join(" · ");
}

/** F2 and batch mode: each saved look as a small label button; one click gives the item that look. */
export function LookChips({ text, looks, picked, onPick, onSave, small }: {
  text: string; looks: SavedLook[]; picked?: string | null; onPick: (l: SavedLook) => void;
  /** "+ Save this look": saves the item's current look under a name. */
  onSave?: (name: string) => Promise<void>; small?: boolean;
}) {
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState("");
  const [err, setErr] = useState("");
  const save = async () => {
    if (!onSave || !name.trim()) return;
    try { await onSave(name.trim()); setNaming(false); setName(""); setErr(""); }
    catch (e) { setErr(friendly(e)); }
  };
  return (
    <>
      <div className={`lk-chips ${small ? "small" : ""}`}>
        {looks.map((l) => (
          <button key={l.name} type="button" className={`lk-chip ${picked === l.name ? "on" : ""}`}
            title={`${l.name}${lookWords(l.style) ? ` (${lookWords(l.style)})` : ""}: click to use this look`} onClick={() => onPick(l)}>
            <Preview s={l.style} text={text || l.name} />
            <small>{l.name}</small>
          </button>
        ))}
        {onSave && !naming && (
          <button type="button" className="lk-chip lk-add" onClick={() => setNaming(true)}
            title="Keep this item's look under a name, to use on other items with one click">+ Save this<br />look</button>
        )}
      </div>
      {naming && (
        <div className="lk-name">
          <input autoFocus className="text" placeholder="Name it, e.g. My chase look" value={name} maxLength={40}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") save(); if (e.key === "Escape") { e.stopPropagation(); setNaming(false); } }} />
          <button className="primary" onClick={save} disabled={!name.trim()}>Save</button>
          <button onClick={() => { setNaming(false); setErr(""); }}>Cancel</button>
        </div>
      )}
      {err && <div className="lk-err">{err}</div>}
    </>
  );
}

const NEW_LOOK: CustomStyle = {
  font_size: 42, text: [255, 60, 60, 255], border: [255, 60, 60, 255], background: [20, 20, 20, 240],
  beam: { colour: "Red", temp: false }, minimap: { size: 1, colour: "Red", shape: "Star" }, sound: { kind: "Default" }, label_hidden: false,
};

/** My edits page: your saved looks, each with how many edits use it, and Change / Rename / Delete. */
export function SavedLooksCard({ onChange }: { onChange: () => void }) {
  const [looks, reload] = useLooks();
  const [editing, setEditing] = useState<string | null>(null); // name, or "" for a new look
  const [renaming, setRenaming] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [sure, setSure] = useState<string | null>(null);
  const [msg, setMsg] = useState("");

  const done = (text: string) => { setMsg(text); reload(); looksChanged(); onChange(); };
  const save = async (name: string, style: CustomStyle, old: string | null) => {
    try {
      const w = await invoke<string[]>("save_look", { look: { name, style }, oldName: old });
      const n = looks.find((l) => l.name === old)?.used_by ?? 0;
      setEditing(null); setRenaming(null);
      done(old && n > 0 ? `Saved. ${n} edit${n === 1 ? "" : "s"} using it changed too: press F4 in game.` : `Saved “${name}”.`);
      if (w.length) setMsg((m) => `${m} ${w.join(" · ")}`);
    } catch (e) { setMsg(friendly(e)); }
  };
  const remove = async (l: SavedLook) => {
    if (sure !== l.name) { setSure(l.name); return; }
    try { await invoke("delete_look", { name: l.name }); setSure(null); done(`Deleted “${l.name}”.${l.used_by ? " Its edits keep their colours." : ""}`); }
    catch (e) { setMsg(friendly(e)); }
  };

  return (
    <div className="card lk-card">
      <div className="row between">
        <h2>My saved looks</h2>
        {editing === null && <button onClick={() => { setEditing(""); setNewName(""); setMsg(""); }}>+ New look</button>}
      </div>
      {looks.length === 0 && editing === null && (
        <p className="muted small">Save a look once and use it on many items. In game, press F2 on an item and pick
          <b> + Save this look</b>; then any item takes that look with one click. Change the look here and every item using it changes too.</p>
      )}
      {editing === "" && (
        <div className="lk-edit">
          <input className="text" autoFocus placeholder="Name, e.g. My chase look" value={newName} maxLength={40} onChange={(e) => setNewName(e.target.value)} />
          <StyleEditor start={NEW_LOOK} label={newName || "New look"} onCancel={() => setEditing(null)}
            onSave={(s) => (newName.trim() ? save(newName.trim(), s, null) : setMsg("Give the look a name first"))} />
        </div>
      )}
      <ul className="lk-list">
        {looks.map((l) => (
          <li key={l.name}>
            <div className="lk-row">
              <button type="button" className="lk-prev" title="Change this look" onClick={() => setEditing(editing === l.name ? null : l.name)}>
                <Preview s={l.style} text={l.name} />
              </button>
              {renaming === l.name ? (
                <span className="lk-rename">
                  <input className="text" autoFocus value={newName} maxLength={40} onChange={(e) => setNewName(e.target.value)}
                    onKeyDown={(e) => { if (e.key === "Enter" && newName.trim()) save(newName.trim(), l.style, l.name); if (e.key === "Escape") setRenaming(null); }} />
                  <button className="primary small" disabled={!newName.trim()} onClick={() => save(newName.trim(), l.style, l.name)}>Save</button>
                  <button className="link" onClick={() => setRenaming(null)}>Cancel</button>
                </span>
              ) : (
                <span className="lk-what"><span className="muted small">{lookWords(l.style)}</span></span>
              )}
              <span className="lk-used">{l.used_by ? `used by ${l.used_by} edit${l.used_by === 1 ? "" : "s"}` : "not used yet"}</span>
              <span className="lk-btns">
                <button className="link" onClick={() => setEditing(editing === l.name ? null : l.name)}>{editing === l.name ? "Close" : "Change"}</button>
                <button className="link" onClick={() => { setRenaming(l.name); setNewName(l.name); }}>Rename</button>
                <button className={`link ${sure === l.name ? "lk-sure" : ""}`} onClick={() => remove(l)} onBlur={() => setSure(null)}>
                  {sure === l.name ? "Sure? Delete" : "Delete"}</button>
              </span>
            </div>
            {editing === l.name && (
              <div className="lk-edit">
                {l.used_by > 0 && <div className="muted small">Saving changes all {l.used_by} edit{l.used_by === 1 ? "" : "s"} using this look.</div>}
                <StyleEditor start={l.style} label={l.name} onCancel={() => setEditing(null)} onSave={(s) => save(l.name, s, l.name)} />
              </div>
            )}
          </li>
        ))}
      </ul>
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
