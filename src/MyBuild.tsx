import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import BuildView, { PobState } from "./BuildView";
import { Tip } from "./Help";

/** Paste a Path of Building link once; see its gems, gear and tree here or over the game (F6). */
export default function MyBuild() {
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [has, setHas] = useState<PobState | null>(null);
  const [key, setKey] = useState(0);

  useEffect(() => { invoke<PobState | null>("pob_get").then((s) => { setHas(s); if (s) setLink(s.link); }).catch(() => {}); }, [key]);
  const go = async () => {
    setBusy(true); setMsg("");
    try {
      const s = await invoke<PobState>("pob_import", { link });
      setMsg(`Loaded ${s.build.ascendancy || s.build.class} with ${s.build.stages.length} stage${s.build.stages.length > 1 ? "s" : ""}. Press F6 in game to see it.`);
      setKey((k) => k + 1);
    } catch (e) { setMsg(String(e)); } finally { setBusy(false); }
  };

  return (
    <>
      <div className="card">
        <h2>Your Path of Building <Tip>Paste your build's share link (pobb.in, pastebin or poe.ninja) or the export code from PoB.
          Press <b>F6</b> in game to show it over the game without alt-tabbing, and F6 again to hide it.
          Builds with stages (Act 1, Act 3, Early maps…) get a stage picker; the app remembers which one you're on.
          Re-import any time to pick up changes you made in PoB.</Tip></h2>
        <div className="row">
          <input className="text grow" placeholder="https://pobb.in/… or paste the code" value={link}
            onChange={(e) => setLink(e.target.value)} onKeyDown={(e) => e.key === "Enter" && link.trim() && go()} />
          <button className="primary" disabled={busy || !link.trim()} onClick={go}>{busy ? "Loading…" : has ? "Update" : "Load build"}</button>
        </div>
        {msg && <div className="note">{msg}</div>}
      </div>
      {has && <div className="card"><BuildView key={key} /></div>}
    </>
  );
}
