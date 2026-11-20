import { PageLoading } from "./Loading";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Settings = {
  key_quick_edit: string; key_batch: string; key_reload: string; key_cycle: string; key_build: string; key_price?: string; key_atlas?: string;
  cycle_enabled: boolean; fallback_area_level: number; claude_api_key: string; map_avoid?: string[] | null;
  trade_helper?: boolean; thanks_message?: string; chat_keys?: { key: string; command: string }[];
  price_compare?: boolean; price_rolls?: boolean; build_with_tree?: boolean; key_tree?: string;
};
const CHAT_IDEAS = ["/hideout", "/kingsmarch", "/menagerie", "/delve", "/exit", "/invite", "/kick", "/afk"];
type Suggestion = { name: string; why: string };

/** Which map mods Ctrl+D warns about, with a suggestion from the PoB build. */
function MapAvoid({ value, onChange }: { value: string[] | null | undefined; onChange: (v: string[] | null) => void }) {
  const [all, setAll] = useState<string[]>([]);
  const [why, setWhy] = useState<Record<string, string>>({});
  const [note, setNote] = useState("");
  useEffect(() => { invoke<string[]>("map_dangers").then(setAll).catch(() => {}); }, []);
  const on = (n: string) => value == null || value.includes(n);
  const toggle = (n: string) => {
    const cur = value ?? all;
    onChange(cur.includes(n) ? cur.filter((x) => x !== n) : [...cur, n]);
  };
  const suggest = async () => {
    try {
      const s = await invoke<Suggestion[]>("map_suggest");
      onChange(s.map((x) => x.name));
      setWhy(Object.fromEntries(s.map((x) => [x.name, x.why])));
      setNote(`Ticked ${s.length} from your build. Check them over, then Save.`);
    } catch (e) { setNote(String(e)); }
  };
  return (
    <div className="card">
      <h2>Map check</h2>
      <p className="muted small">Press your price check key on a map to see its mods. The ones ticked here show in red as mods you can't run.</p>
      <div className="row">
        <button onClick={suggest}>Suggest from my build</button>
        <button onClick={() => { onChange(null); setWhy({}); setNote(""); }}>Tick all</button>
        <button onClick={() => { onChange([]); setWhy({}); setNote(""); }}>Untick all</button>
      </div>
      {note && <p className="note small">{note}</p>}
      <div className="map-avoid">
        {all.map((n) => (
          <label key={n} className="check" title={why[n]}>
            <input type="checkbox" checked={on(n)} onChange={() => toggle(n)} />
            <span>{n}{why[n] && <span className="muted small"> · {why[n]}</span>}</span>
          </label>
        ))}
      </div>
    </div>
  );
}

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
  if (!s) return msg ? <div className="card">{msg}</div> : <PageLoading />;
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
          <span>Show/hide your atlas strategy (Endgame → Atlas)</span><KeyBox value={s.key_atlas ?? "F7"} onChange={(k) => set({ key_atlas: k })} />
          <label className="check"><input type="checkbox" checked={s.cycle_enabled} onChange={(e) => set({ cycle_enabled: e.target.checked })} />
            <span>Switch to your next filter (e.g. League Start → Strict 1 → Strict 2) and load it</span></label>
          <KeyBox value={s.key_cycle} disabled={!s.cycle_enabled} onChange={(k) => set({ key_cycle: k })} />
          <label className="check"><input type="checkbox" checked={s.build_with_tree ?? false} onChange={(e) => set({ build_with_tree: e.target.checked })} />
            <span>Open the build window (on its Tree tab) when the passive skill tree is opened in game; that key or Esc closes it.
              Set the box to the game's passive tree key (P unless you changed it). The game still gets the key.</span></label>
          <KeyBox value={s.key_tree ?? "P"} disabled={!(s.build_with_tree ?? false)} onChange={(k) => set({ key_tree: k })} />
        </div>
      </div>

      <div className="card">
        <h2>Price check</h2>
        <p className="muted small">What the price window shows besides the price.</p>
        <label className="check"><input type="checkbox" checked={s.price_compare ?? true} onChange={(e) => set({ price_compare: e.target.checked })} />
          <span>Compare the item with your build's gear (needs a PoB link on the Build page)</span></label>
        <label className="check"><input type="checkbox" checked={s.price_rolls ?? true} onChange={(e) => set({ price_rolls: e.target.checked })} />
          <span>Show how well each mod rolled (a bar from the lowest to the highest roll)</span></label>
      </div>

      <div className="card">
        <h2>Trade helper</h2>
        <p className="muted small">When someone whispers to buy from you, a small list pops up with Invite, Trade, Thanks and Kick buttons.
          Each button press types one command in game, as GGG's rules allow. It turns green when the buyer is in your area.</p>
        <label className="check"><input type="checkbox" checked={s.trade_helper ?? true} onChange={(e) => set({ trade_helper: e.target.checked })} />
          <span>Show incoming trade whispers</span></label>
        <div className="row">
          <span className="muted small">Thanks message</span>
          <input className="text" value={s.thanks_message ?? "thanks, good luck!"} onChange={(e) => set({ thanks_message: e.target.value })} />
        </div>
        <h3>Chat hotkeys</h3>
        <p className="muted small">One key, one command (for example F7 → /hideout). They only type while the game is in front.</p>
        <div className="chat-keys">
          {(s.chat_keys ?? []).map((c, i) => {
            const edit = (p: Partial<{ key: string; command: string }>) =>
              set({ chat_keys: (s.chat_keys ?? []).map((x, j) => (j === i ? { ...x, ...p } : x)) });
            return [
              <KeyBox key={"k" + i} value={c.key || "Pick a key"} onChange={(k) => edit({ key: k })} />,
              <input key={"c" + i} className="text" list="chat-ideas" value={c.command} placeholder="/hideout" onChange={(e) => edit({ command: e.target.value })} />,
              <button key={"x" + i} onClick={() => set({ chat_keys: (s.chat_keys ?? []).filter((_, j) => j !== i) })}>Remove</button>,
            ];
          })}
        </div>
        <datalist id="chat-ideas">{CHAT_IDEAS.map((c) => <option key={c} value={c} />)}</datalist>
        <div className="row"><button onClick={() => set({ chat_keys: [...(s.chat_keys ?? []), { key: "", command: "/hideout" }] })}>Add a chat hotkey</button></div>
      </div>

      <MapAvoid value={s.map_avoid} onChange={(v) => set({ map_avoid: v })} />

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
