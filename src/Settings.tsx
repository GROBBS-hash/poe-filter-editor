import { Tip } from "./Help";
import { PageLoading } from "./Loading";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TW_LAYOUTS, TW_LAYOUT_KEY, TW_STYLES, TW_STYLE_KEY } from "./TradeWindow";

type Settings = {
  key_quick_edit: string; key_batch: string; key_reload: string; key_cycle: string; key_build: string; key_price?: string; key_atlas?: string;
  cycle_enabled: boolean; fallback_area_level: number; claude_api_key: string; map_avoid?: string[] | null;
  trade_helper?: boolean; thanks_message?: string; chat_keys?: { key: string; command: string }[];
  price_compare?: boolean; price_rolls?: boolean; price_instant_all?: boolean; price_buyout_only?: boolean; build_with_tree?: boolean; key_tree?: string;
};
const CHAT_IDEAS = ["/hideout", "/kingsmarch", "/menagerie", "/delve", "/exit", "/invite", "/kick", "/afk"];

/** Map mods you can't run are one list on the Maps page: Ctrl+D shows them in red and the map stash search hides them. */
function MapCheckLink() {
  return (
    <div className="card">
      <h2>Map check</h2>
      <p className="muted small">Mods you can't run are set on the Maps page; Ctrl+D and the stash search both use them.</p>
      <div className="row"><button onClick={() => window.dispatchEvent(new CustomEvent("goto-tab", { detail: "maps" }))}>Open the Maps page</button></div>
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
      <MainLook />
      <div className="card">
        <h2>Hotkeys</h2>
        <p className="muted small">Click a box and press the key you want. Pick keys the game and your other tools don't use.</p>
        <div className="set-grid">
          <span>Quick edit the item under your mouse</span><KeyBox value={s.key_quick_edit} onChange={(k) => set({ key_quick_edit: k })} />
          <span>Batch mode on/off</span><KeyBox value={s.key_batch} onChange={(k) => set({ key_batch: k })} />
          <span>Reload your filter in game</span><KeyBox value={s.key_reload} onChange={(k) => set({ key_reload: k })} />
          <span>Show/hide your build (PoB)</span><KeyBox value={s.key_build ?? "F6"} onChange={(k) => set({ key_build: k })} />
          <span>Price check the item under your mouse</span><KeyBox value={s.key_price ?? "Ctrl+D"} onChange={(k) => set({ key_price: k })} />
          <span>Show/hide your atlas strategy (Atlas page)</span><KeyBox value={s.key_atlas ?? "F7"} onChange={(k) => set({ key_atlas: k })} />
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
        <label className="check"><input type="checkbox" checked={s.price_buyout_only ?? true} onChange={(e) => set({ price_buyout_only: e.target.checked })} />
          <span>Instant buyout only: price from listings you can buy straight away, not sellers you have to whisper</span></label>
        <label className="check"><input type="checkbox" checked={s.price_instant_all ?? false} onChange={(e) => set({ price_instant_all: e.target.checked })} />
          <span>Search every item straight away. Off (like Awakened): uniques, currency and other bulk items, and
            unidentified items search at once; rares and gear wait so you can pick the stats, then move your mouse onto Search.</span></label>
        <PriceLook />
      </div>

      <div className="card">
        <h2>Trade helper <Tip>When someone whispers to buy from you, a small list pops up with Invite, Trade, Thanks and Kick
          buttons. Each button press types one command in game, as GGG's rules allow. It turns green when the buyer is in your area.</Tip></h2>
        <p className="muted small">Invite, Trade, Thanks and Kick buttons when someone whispers to buy.</p>
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

      <MapCheckLink />

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


/** The main window's look: a colour per page, or calm (plain icons, gold only for the page you're on). */
function MainLook() {
  const KEY = "app.look";
  const [v, setV] = useState(() => { try { return localStorage.getItem(KEY) || "colourful"; } catch { return "colourful"; } });
  const pick = (k: string) => {
    setV(k);
    try { localStorage.setItem(KEY, k); } catch { /* not kept */ }
    window.dispatchEvent(new CustomEvent("app-look", { detail: k }));
  };
  const looks: [string, string, string][] = [
    ["colourful", "Colourful", "Each page has its own colour in the menu"],
    ["calm", "Calm", "Plain menu icons, gold only for the page you're on; Home leads with your filter"],
  ];
  return (
    <div className="card">
      <h2>Main window look</h2>
      <div className="tw-layout-pick">
        {looks.map(([k, name, what]) => (
          <label key={k} className={v === k ? "check sel" : "check"}>
            <input type="radio" name="app-look" checked={v === k} onChange={() => pick(k)} />
            <span><b>{name}</b> <span className="muted small">{what}</span></span>
          </label>
        ))}
      </div>
    </div>
  );
}

/** The price window's look: same choice as its Style button (shared through localStorage). */
function PriceLook() {
  const [v, setV] = useState(() => { try { return localStorage.getItem(TW_STYLE_KEY) || "classic"; } catch { return "classic"; } });
  const pick = (k: string) => { setV(k); try { localStorage.setItem(TW_STYLE_KEY, k); } catch { /* not kept */ } };
  const [lay, setLay] = useState(() => { try { return localStorage.getItem(TW_LAYOUT_KEY) || "grouped"; } catch { return "grouped"; } });
  const pickLay = (k: string) => { setLay(k); try { localStorage.setItem(TW_LAYOUT_KEY, k); } catch { /* not kept */ } };
  return (
    <div className="tw-look-pick">
      <h3>Look of the price window</h3>
      <p className="muted small">Try them for a while: "More" at the bottom of the price window has Style and Layout too.</p>
      <div className="tw-looks">
        {TW_STYLES.map(([k, name, what]) => (
          <button key={k} className={`tw-look ${v === k ? "sel" : ""}`} onClick={() => pick(k)} title={what}>
            <LookPreview k={k} />
            <b>{name}</b><span className="muted small">{what}</span>
          </button>
        ))}
      </div>
      <h3>How the stats are listed</h3>
      <div className="tw-layout-pick">
        {TW_LAYOUTS.map(([k, name, what]) => (
          <label key={k} className={lay === k ? "check sel" : "check"}>
            <input type="radio" name="tw-layout" checked={lay === k} onChange={() => pickLay(k)} />
            <span><b>{name}</b> <span className="muted small">{what}</span></span>
          </label>
        ))}
      </div>
    </div>
  );
}

/** A tiny drawing of each look (not the real window, just its shape). */
function LookPreview({ k }: { k: string }) {
  const head = <div className={`lp-head ${k === "tooltip" ? "tt" : ""}`}><i /><span /></div>;
  const price = <div className={`lp-price ${k === "price" ? "big" : ""}`}><b /><span /></div>;
  const mods = <div className="lp-mods">{[0, 1, 2, 3].map((i) => <div key={i} className={`lp-mod ${i < 2 ? "on" : ""}`}><i /><span style={{ width: `${70 - i * 9}%` }} /></div>)}</div>;
  const listings = <div className="lp-list">{[0, 1, 2].map((i) => <div key={i} className="lp-row"><b /><span />{k === "compare" && <em><u className="g" /><u className="r" /><u /></em>}</div>)}</div>;
  const spread = <div className="lp-spread">{[30, 60, 100, 70, 40, 25].map((h, i) => <i key={i} className={i === 2 ? "me" : ""} style={{ height: `${h}%` }} />)}</div>;
  return (
    <div className={`lp lp-k-${k}`} aria-hidden>
      {k === "split" ? <div className="lp-split"><div>{head}{mods}</div><div className="lp-side">{price}{spread}</div></div>
        : k === "price" ? <>{price}{spread}{head}{mods}</>
        : k === "tooltip" ? <>{head}{mods}{price}</>
        : k === "compare" ? <>{head}{price}{mods}{listings}</>
        : <>{head}{price}{mods}{listings}</>}
    </div>
  );
}