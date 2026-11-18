import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Settings = {
  key_quick_edit: string; key_batch: string; key_reload: string; key_cycle: string; key_build: string; key_price?: string;
  cycle_enabled: boolean; fallback_area_level: number; claude_api_key: string;
};

/** Click the box, then press the key (or key combo) you want. */
function KeyBox({ value, onChange, disabled }: { value: string; onChange: (k: string) => void; disabled?: boolean }) {
  const [listening, setListening] = useState(false);
  return (
    <input className={`keybox ${listening ? "on" : ""}`} readOnly disabled={disabled}
      value={listening ? "Press a key…" : value}
      onFocus={() => setListening(true)} onBlur={() => setListening(false)}
      onKeyDown={(e) => {
        e.preventDefault();
        if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;
        if (e.key === "Escape") { (e.target as HTMLInputElement).blur(); return; }
        const key = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : e.key.length === 1 ? e.key.toUpperCase() : e.code;
        const combo = [e.ctrlKey && "Ctrl", e.shiftKey && "Shift", e.altKey && "Alt", key].filter(Boolean).join("+");
        onChange(combo);
        (e.target as HTMLInputElement).blur();
      }} />
  );
}

export default function SettingsPage() {
  const [s, setS] = useState<Settings | null>(null);
  const [dirty, setDirty] = useState(false);
  const [msg, setMsg] = useState("");
  useEffect(() => { invoke<Settings>("get_settings").then(setS).catch((e) => setMsg(String(e))); }, []);
  if (!s) return <div className="card">{msg || "Loading…"}</div>;
  const set = (patch: Partial<Settings>) => { setS({ ...s, ...patch }); setDirty(true); setMsg(""); };
  const save = async () => {
    try { await invoke("save_settings", { settings: s }); setDirty(false); setMsg("Saved. New keys work straight away."); }
    catch (e) { setMsg(String(e)); }
  };

  return (
    <>
      <div className="card">
        <h2>Hotkeys</h2>
        <p className="muted small">Click a box and press the key you want. Pick keys the game and your other tools don't use.</p>
        <div className="set-grid">
          <span>Quick edit the item under your mouse</span><KeyBox value={s.key_quick_edit} onChange={(k) => set({ key_quick_edit: k })} />
          <span>Batch mode on/off</span><KeyBox value={s.key_batch} onChange={(k) => set({ key_batch: k })} />
          <span>Reload your filter in game</span><KeyBox value={s.key_reload} onChange={(k) => set({ key_reload: k })} />
          <span>Show/hide your build (PoB)</span><KeyBox value={s.key_build ?? "F6"} onChange={(k) => set({ key_build: k })} />
          <span>Price check the item under your mouse</span><KeyBox value={s.key_price ?? "Ctrl+D"} onChange={(k) => set({ key_price: k })} />
          <label className="check"><input type="checkbox" checked={s.cycle_enabled} onChange={(e) => set({ cycle_enabled: e.target.checked })} />
            <span>Switch to your next filter (e.g. League Start → Strict 1 → Strict 2) and load it</span></label>
          <KeyBox value={s.key_cycle} disabled={!s.cycle_enabled} onChange={(k) => set({ key_cycle: k })} />
        </div>
      </div>

      <div className="card">
        <h2>Area level</h2>
        <p className="muted small">The app reads your current area level from the game's log. If it can't find the log, it assumes this level.</p>
        <div className="row">
          <input type="number" className="num" min={1} max={86} value={s.fallback_area_level}
            onChange={(e) => set({ fallback_area_level: Math.max(1, Math.min(86, +e.target.value || 83)) })} />
          <span className="muted small">{s.fallback_area_level >= 68 ? "(maps)" : "(campaign)"}</span>
        </div>
      </div>

      <div className="card">
        <h2>AI for the “Just say what you want” box (optional)</h2>
        <p className="muted small">The box works without this for common phrases. With your own Claude API key it understands looser wording.
          Each request costs a fraction of a cent on your Anthropic account. The key is kept only on this PC.</p>
        <input className="text" type="password" placeholder="sk-ant-…" value={s.claude_api_key}
          onChange={(e) => set({ claude_api_key: e.target.value.trim() })} />
      </div>

      <div className="save-bar">
        {msg && <span className="note">{msg}</span>}
        <button className="primary" disabled={!dirty} onClick={save}>{dirty ? "Save settings" : "Saved"}</button>
      </div>
    </>
  );
}
