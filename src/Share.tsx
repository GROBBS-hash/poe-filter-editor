// Share packs (Filters page): send your filter's edits, saved looks, theme and price settings to a friend as a
// short code or a file, and add a friend's pack as a new filter on your own NeverSink base.
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ProfileInfo, STRICTNESS_NAMES } from "./types";
import { friendly } from "./errors";
import { looksChanged } from "./Looks";

type Summary = { name: string; game: "Poe1" | "Poe2"; strictness?: number | null; edits: number; edits_off: number;
  looks: string[]; theme?: string | null; prices: boolean };

/** "23 edits (2 off) · looks: Chase, Quiet · Dark theme · price tiers". */
function what(s: Summary) {
  return [`${s.edits} edit${s.edits === 1 ? "" : "s"}${s.edits_off ? ` (${s.edits_off} off)` : ""}`,
    s.looks.length ? `looks: ${s.looks.join(", ")}` : "", s.theme ? `${s.theme} theme` : "", s.prices ? "price tiers" : ""]
    .filter(Boolean).join(" · ");
}

export default function SharePacks({ active, names, onChange }: { active: ProfileInfo; names: string[]; onChange: () => void }) {
  const [code, setCode] = useState("");
  const [mine, setMine] = useState<Summary | null>(null);
  const [sendMsg, setSendMsg] = useState("");
  const [paste, setPaste] = useState("");
  const [file, setFile] = useState<string | null>(null);
  const [theirs, setTheirs] = useState<Summary | null>(null);
  const [name, setName] = useState("");
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);

  const copy = async () => {
    try {
      const out = await invoke<{ code: string; summary: Summary }>("export_pack", { id: active.id });
      setCode(out.code); setMine(out.summary);
      try { await navigator.clipboard.writeText(out.code); setSendMsg("Copied. Paste it to your friend in Discord or a whisper."); }
      catch { setSendMsg("Copy the code below and send it to your friend."); }
    } catch (e) { setSendMsg(friendly(e)); }
  };
  const saveFile = async () => {
    try {
      const out = await invoke<{ file_name: string; summary: Summary }>("export_pack", { id: active.id });
      setMine(out.summary);
      const path = await save({ defaultPath: out.file_name, filters: [{ name: "Share pack", extensions: ["lodestone"] }] });
      if (!path) return;
      await invoke("save_pack_file", { id: active.id, path });
      setSendMsg(`Saved ${path.split(/[\\/]/).pop()}. Send that file to your friend.`);
    } catch (e) { setSendMsg(friendly(e)); }
  };

  /** Free name for the new filter: the pack's name, or "<name> 2", "<name> 3"... */
  const freeName = (n: string) => {
    const taken = (x: string) => names.some((y) => y.toLowerCase() === x.toLowerCase());
    let out = n, i = 2;
    while (taken(out)) out = `${n} ${i++}`;
    return out;
  };
  const check = async (text: string | null, path: string | null) => {
    setMsg(""); setTheirs(null);
    if (!text?.trim() && !path) return;
    try {
      const s = await invoke<Summary>("read_pack", { text, path });
      setTheirs(s); setName(freeName(s.name));
    } catch (e) { setMsg(friendly(e)); }
  };
  const pick = async () => {
    const f = await open({ filters: [{ name: "Share pack", extensions: ["lodestone", "json"] }] }).catch(() => null);
    if (typeof f !== "string") return;
    setFile(f); setPaste("");
    check(null, f);
  };
  const add = async () => {
    setBusy(true);
    try {
      await invoke("import_pack", { text: file ? null : paste, path: file, name: name.trim() });
      setMsg(`Added “${name.trim()}” and switched to it. Press F4 in game to load it.`);
      setTheirs(null); setPaste(""); setFile(null);
      looksChanged();
      onChange();
    } catch (e) { setMsg(friendly(e)); }
    setBusy(false);
  };

  return (
    <div className="card share-card">
      <h2>Share with friends</h2>
      <div className="share-cols">
        <div>
          <h3>Send “{active.name}”</h3>
          <p className="muted small">{mine ? what(mine) : `${active.rules.length} edits, plus the saved looks they use, your theme and price settings`}.
            Not NeverSink's file: your friend's own is used.</p>
          <div className="row">
            <button className="primary" onClick={copy}>Copy share code</button>
            <button onClick={saveFile}>Save as a file…</button>
          </div>
          {code && <textarea className="text share-code" readOnly rows={3} value={code} onFocus={(e) => e.target.select()} />}
          {sendMsg && <div className="note">{sendMsg}</div>}
        </div>
        <div>
          <h3>Use a friend's</h3>
          <textarea className="text" rows={2} placeholder="Paste a share code (starts with LODE1:)" value={paste}
            onChange={(e) => { setPaste(e.target.value); setFile(null); check(e.target.value, null); }} />
          <div className="row"><button onClick={pick}>Open a pack file…</button>
            {file && <span className="muted small">{file.split(/[\\/]/).pop()}</span>}</div>
          {theirs && (
            <div className="share-sum">
              <b>“{theirs.name}”</b> · {theirs.game === "Poe1" ? "PoE 1" : "PoE 2"}
              {theirs.strictness != null ? ` · ${STRICTNESS_NAMES[theirs.strictness] ?? ""}` : ""}
              <div className="muted small">{what(theirs)}</div>
              <label className="muted small share-name">Name for the new filter
                <input className="text" value={name} maxLength={60} onChange={(e) => setName(e.target.value)} /></label>
              <div className="row">
                <button className="primary" disabled={busy || !name.trim()} onClick={add}>{busy ? "Adding…" : "Add as a new filter"}</button>
                <span className="muted small">Your own filters aren't changed.</span>
              </div>
            </div>
          )}
          {msg && <div className="note">{msg}</div>}
        </div>
      </div>
    </div>
  );
}
