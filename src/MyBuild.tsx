import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import BuildView from "./BuildView";
import { Tip } from "./Help";

type Saved = { id: string; name: string; link: string; class: string; level: number; active: boolean };

const isLink = (l: string) => /^https?:\/\//.test(l);
const shortLink = (l: string) => l.replace(/^https?:\/\/(www\.)?/, "");

/** Save several Path of Building builds and swap between them; the shown one is used here, over the game (F6) and by the levelling guide. */
export default function MyBuild() {
  const [list, setList] = useState<Saved[]>([]);
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [key, setKey] = useState(0);
  // Editing a build's code in place (28 Nov, he asked to copy or edit the PoB code here).
  const [editing, setEditing] = useState<{ id: string; code: string } | null>(null);
  const copyCode = async (b: Saved) => {
    try { await navigator.clipboard.writeText(await invoke<string>("pob_code", { id: b.id })); setMsg(`Copied the code of ${b.name}. In Path of Building: Import/Export Build, paste it in.`); }
    catch (e) { setMsg(String(e)); }
  };
  const editCode = async (b: Saved) => {
    try { setEditing({ id: b.id, code: await invoke<string>("pob_code", { id: b.id }) }); } catch (e) { setMsg(String(e)); }
  };

  const load = () => invoke<Saved[]>("pob_list").then(setList).catch(() => setList([]));
  useEffect(() => {
    load();
    const c = listen("pob-imported", () => { load(); setKey((k) => k + 1); });
    return () => { c.then((f) => f()); };
  }, []);
  const shown = list.find((b) => b.active);

  const add = async (from: string, replace?: string) => {
    setBusy(replace ?? "add"); setMsg("");
    try {
      const before = list.length;
      const s = await invoke<{ build: { ascendancy: string; class: string; stages: unknown[] } }>("pob_import", { link: from, replace: replace ?? null });
      const after = await invoke<Saved[]>("pob_list");
      setList(after);
      const n = s.build.stages.length;
      const what = `${s.build.ascendancy || s.build.class} with ${n} stage${n > 1 ? "s" : ""}`;
      setMsg(replace || after.length === before ? `Updated ${what}. It's the one showing now.` : `Saved ${what} and switched to it. Press F6 in game to see it.`);
      if (!replace) setLink("");
      return true;
    } catch (e) { setMsg(String(e)); return false; } finally { setBusy(""); }
  };
  const show = async (id: string) => {
    setBusy(id); setMsg("");
    try { await invoke("pob_switch", { id }); await load(); } catch (e) { setMsg(String(e)); } finally { setBusy(""); }
  };
  const rename = async () => {
    if (!renaming) return;
    try { await invoke("pob_rename", { id: renaming.id, name: renaming.name }); setRenaming(null); load(); } catch (e) { setMsg(String(e)); }
  };
  const remove = async (b: Saved) => {
    if (!confirm(`Remove “${b.name}” from your saved builds? You can add it again from its link any time.`)) return;
    try { await invoke("pob_remove", { id: b.id }); await load(); setMsg(`Removed ${b.name}.`); } catch (e) { setMsg(String(e)); }
  };

  return (
    <>
      <div className="card">
        <h2>Your builds <Tip>Save as many Path of Building builds as you like (a league starter, a second character, a build
          you're trying out) and click <b>Show this one</b> to swap. The one showing is used here, over the game with <b>F6</b>, and by
          the levelling guide. Each build remembers which stage you were on. <b>Update</b> picks up changes you made in PoB.</Tip></h2>
        {list.length > 0 && (
          <ul className="rules profiles-list mb-list">
            {list.map((b) => (
              <li key={b.id} className={b.active ? "mb-on" : ""}>
                <span>
                  {renaming?.id === b.id ? (
                    <span className="row">
                      <input className="text" autoFocus value={renaming.name} onChange={(e) => setRenaming({ id: b.id, name: e.target.value })}
                        onKeyDown={(e) => { if (e.key === "Enter") rename(); if (e.key === "Escape") setRenaming(null); }} />
                      <button className="primary" onClick={rename}>Save</button>
                      <button className="link" onClick={() => setRenaming(null)}>Cancel</button>
                    </span>
                  ) : (
                    <><b>{b.name}</b> {b.active && <span className="tag">Showing</span>}</>
                  )}
                  <div className="muted small">
                    {b.class}{b.level ? ` · level ${b.level} in PoB` : ""}{b.link ? ` · ${isLink(b.link) ? shortLink(b.link) : b.link}` : ""}
                  </div>
                </span>
                <span className="row">
                  {!b.active && <button className="primary" disabled={!!busy} onClick={() => show(b.id)}>{busy === b.id ? "Switching…" : "Show this one"}</button>}
                  {isLink(b.link) && <button disabled={!!busy} onClick={() => add(b.link, b.id)}>{busy === b.id ? "Updating…" : "Update"}</button>}
                  <button className="link" title="Copy its Path of Building code, to paste into PoB (Import/Export Build) or share" onClick={() => copyCode(b)}>Copy code</button>
                  <button className="link" title="Paste in a newer code from Path of Building (after you changed the build there)" onClick={() => editing?.id === b.id ? setEditing(null) : editCode(b)}>Edit code</button>
                  <button className="link" onClick={() => setRenaming({ id: b.id, name: b.name })}>Rename</button>
                  <button className="link" onClick={() => remove(b)}>Remove</button>
                </span>
                {editing?.id === b.id && (
                  <div className="mb-code">
                    <div className="muted small">The build's Path of Building code. Changed the build in PoB? Export it there (Import/Export Build, Copy), paste it here over this one and press Save. The stages, gear and tree here update; your ticks stay.</div>
                    <textarea className="text" rows={4} spellCheck={false} value={editing.code} onChange={(e) => setEditing({ id: b.id, code: e.target.value })} />
                    <span className="row">
                      <button className="primary" disabled={!!busy || !editing.code.trim()} onClick={async () => { if (await add(editing.code, b.id)) setEditing(null); }}>{busy === b.id ? "Saving…" : "Save"}</button>
                      <button className="link" onClick={() => setEditing(null)}>Cancel</button>
                    </span>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
        <div className="row">
          <input className="text grow" placeholder="Add a build: https://pobb.in/… or paste the code from PoB" value={link}
            onChange={(e) => setLink(e.target.value)} onKeyDown={(e) => e.key === "Enter" && link.trim() && !busy && add(link)} />
          <button className="primary" disabled={!!busy || !link.trim()} onClick={() => add(link)}>{busy === "add" ? "Loading…" : list.length ? "Add build" : "Load build"}</button>
        </div>
        {msg && <div className="note">{msg}</div>}
      </div>
      {shown && <div className="card"><BuildView key={key} /></div>}
    </>
  );
}
