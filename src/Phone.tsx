import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { friendly } from "./errors";

type Prefs = {
  ntfy_topic: string; ntfy_server: string; discord: string; on_whisper: boolean; on_arrive: boolean; on_sold: boolean; only_away: boolean;
  shop_watch: boolean; shop_minutes: number; page_on: boolean; page_port: number; page_key: string;
};
type Sale = { time: number; name: string; count: number; note: string; tab: string; chaos: number | null };
type PhoneView = { prefs: Prefs; sales: Sale[]; page_url: string | null; page_error: string | null };

const ago = (t: number) => { const m = Math.round((Date.now() / 1000 - t) / 60); return m < 60 ? `${m}m ago` : m < 1440 ? `${Math.round(m / 60)}h ago` : `${Math.round(m / 1440)}d ago`; };

/** Settings → Phone (24 Nov): alerts for buyers, arrivals and sales, the shop watcher, and the stats page. */
export default function PhoneSettings() {
  const [v, setV] = useState<PhoneView | null>(null);
  const [p, setP] = useState<Prefs | null>(null);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const load = () => invoke<PhoneView>("phone_get").then((x) => { if (!x) return; setV(x); setP((old) => old ?? x.prefs); }).catch((e) => setMsg(friendly(e)));
    load();
    const un = listen("phone-sales", load);
    return () => { un.then((f) => f()); };
  }, []);
  if (!v || !p) return <div className="card">{msg || "Loading…"}</div>;
  const set = (patch: Partial<Prefs>) => {
    const next = { ...p, ...patch };
    setP(next);
    invoke<PhoneView>("phone_save", { prefs: next }).then(setV).catch((e) => setMsg(friendly(e)));
  };
  const check = (k: keyof Prefs, label: string, hint?: string) => (
    <label className="check"><input type="checkbox" checked={!!p[k]} onChange={(e) => set({ [k]: e.target.checked } as Partial<Prefs>)} />
      <span>{label}{hint && <span className="muted small" style={{ display: "block" }}>{hint}</span>}</span></label>
  );
  const flash = (t: string) => { setMsg(t); setTimeout(() => setMsg(""), 4000); };
  return (
    <>
      <div className="card">
        <h2>Phone alerts</h2>
        <p className="muted small">A message on your phone when a buyer whispers, when they arrive in your hideout, and when something sells.
          Alerts only: you still trade at the PC (GGG's rules). The PC and the editor have to be on.</p>
        <h3>Where to send them</h3>
        <p className="muted small"><b>ntfy</b> (easiest): install the free "ntfy" app on your phone, tap +, and subscribe to a topic name
          only you know (for example "grobbs-shop-8241"). Type the same name here.</p>
        <div className="row"><span className="muted small" style={{ minWidth: 90 }}>ntfy topic</span>
          <input className="text" style={{ margin: 0, maxWidth: 280 }} value={p.ntfy_topic} placeholder="grobbs-shop-8241" onChange={(e) => setP({ ...p, ntfy_topic: e.target.value.trim() })} onBlur={() => set({})} /></div>
        <p className="muted small"><b>Or Discord</b>: in your own server, channel settings → Integrations → Webhooks → New webhook → Copy URL, then paste it here.</p>
        <div className="row"><span className="muted small" style={{ minWidth: 90 }}>Discord webhook</span>
          <input className="text" style={{ margin: 0 }} type="password" value={p.discord} placeholder="https://discord.com/api/webhooks/…" onChange={(e) => setP({ ...p, discord: e.target.value.trim() })} onBlur={() => set({})} /></div>
        <div className="row"><button onClick={() => invoke("phone_test").then(() => flash("Sent: check your phone")).catch((e) => flash(friendly(e)))}>Send a test message</button>
          {msg && <span className="muted small">{msg}</span>}</div>
        <h3>What to send</h3>
        {check("on_whisper", "A buyer whispers", "\"Buyer: Bob — Tabula Rasa for 5 chaos · Fair price\"")}
        {check("on_arrive", "The buyer arrives in your hideout")}
        {check("on_sold", "Something sold from a trade tab", "Needs \"Watch my trade tabs\" below")}
        {check("only_away", "Only while I'm away from the game", "Nothing is sent while Path of Exile is the window in front")}
      </div>

      <div className="card">
        <h2>Watch my trade tabs</h2>
        <p className="muted small">PoE has no "sold" message, so the editor reads your trade tabs every few minutes (with the Wealth page's login)
          and counts a listed item that's gone as sold. Moving a listed item out by hand looks the same, so messages say "sold (or moved)".
          Trade tabs = tabs named "~price …" or "~b/o …", and tabs where your last stash scan found listed items.</p>
        {check("shop_watch", "Watch my trade tabs")}
        <div className="row"><span className="muted small">Every</span>
          <input className="num" type="number" min={2} max={60} style={{ width: 70 }} value={p.shop_minutes} onChange={(e) => setP({ ...p, shop_minutes: +e.target.value })} onBlur={() => set({})} />
          <span className="muted small">minutes</span>
          <button disabled={busy} onClick={() => { setBusy(true); invoke<PhoneView>("phone_check_shop").then((x) => { setV(x); flash("Checked: the first check only remembers what's listed"); }).catch((e) => flash(friendly(e))).finally(() => setBusy(false)); }}>
            {busy ? "Checking…" : "Check now"}</button></div>
        <h3>Sales</h3>
        {v.sales.length === 0 ? <p className="muted small">None seen yet.</p> :
          <table className="w-table"><tbody>{v.sales.slice(0, 30).map((s, i) => (
            <tr key={i}><td>{s.count > 1 ? `${s.count} × ` : ""}{s.name}<div className="muted small">{s.tab} · {s.note}</div></td>
              <td className="small">{s.chaos != null ? `${Math.round(s.chaos)}c` : ""}</td><td className="muted small">{ago(s.time)}</td></tr>
          ))}</tbody></table>}
      </div>

      <div className="card">
        <h2>Stats page on your phone</h2>
        <p className="muted small">A small read-only page with your sales, stash worth, earnings per strategy and recent maps. Your phone has to be on
          the same Wi-Fi as the PC (away from home: install Tailscale on both, it's free and private). Windows may ask once whether
          to allow the editor on your network: say yes for private networks.</p>
        {check("page_on", "Show my stats on my phone")}
        {p.page_on && <>
          {v.page_error && <p className="error small">{v.page_error}</p>}
          {v.page_url && <div className="row"><span className="muted small">Open on your phone:</span><code style={{ userSelect: "all" }}>{v.page_url}</code>
            <button className="small" onClick={() => { invoke("copy_text", { text: v.page_url }); flash("Copied"); }}>Copy</button></div>}
          <div className="row"><span className="muted small">Port</span>
            <input className="num" type="number" min={1024} max={65535} style={{ width: 90 }} value={p.page_port} onChange={(e) => setP({ ...p, page_port: +e.target.value })} onBlur={() => set({})} />
            <span className="muted small">(a new port works after restarting the editor)</span>
            <button className="link small" onClick={() => invoke<PhoneView>("phone_new_key").then((x) => { setV(x); setP(x.prefs); })} title="The old link stops working">New secret link</button></div>
        </>}
      </div>
    </>
  );
}
