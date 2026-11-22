import { friendly } from "./errors";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { WinBar } from "./WinFrame";

type Whisper = { time: string; from: string; message: string; item: string | null; price: string | null; league: string | null;
  tab: string | null; left: number | null; top: number | null };
type LogEvent = ({ kind: "whisper" } & Whisper) | { kind: "joined"; name: string } | { kind: "left"; name: string };
type PriceCheck = { asking: number; worth: number; verdict: "low" | "fair" | "high" };
type Entry = { id: number; w: Whisper; here: boolean; pressed: string[]; at: number; check?: PriceCheck | null };
type Listing = { amount: number; currency: string; chaos: number | null; account: string; indexed: string; whisper: string; instant?: boolean; ilvl: number | null };
type Hit = { id: number; search: string; listings: Listing[]; url: string; at: number; sent: number[] };

const BUTTONS: [string, string, string][] = [
  ["invite", "Invite", "Invite them to your party"],
  ["trade", "Trade", "Open a trade window with them"],
  ["thanks", "Thanks", "Whisper your thank-you message (set it in Settings)"],
  ["kick", "Kick", "Remove them from your party (also clears this trade)"],
];

/** "3m" since the whisper arrived. */
function since(at: number, now: number) {
  const m = Math.floor((now - at) / 60000);
  return m < 1 ? "now" : m < 60 ? `${m}m` : `${Math.floor(m / 60)}h`;
}

/** The trade helper: incoming trade whispers with one button per chat command. */
export default function TradesWindow() {
  const [list, setList] = useState<Entry[]>([]);
  const [hits, setHits] = useState<Hit[]>([]);
  const [err, setErr] = useState("");
  const [now, setNow] = useState(Date.now());
  const next = useRef(1);
  const box = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const un = listen<LogEvent>("trade-event", (e) => {
      const ev = e.payload;
      if (ev.kind === "whisper") {
        if (!ev.item) return; // only trade requests
        const { kind: _k, ...w } = ev;
        const id = next.current++;
        setList((l) => [{ id, w, here: false, pressed: [], at: Date.now() }, ...l].slice(0, 12));
        // Your asking price next to poe.ninja: underpriced, fair, or a good sale.
        if (w.item && w.price) invoke<PriceCheck | null>("trade_price_check", { item: w.item, price: w.price })
          .then((check) => setList((l) => l.map((x) => x.id === id ? { ...x, check } : x))).catch(() => {});
      } else {
        setList((l) => l.map((x) => (x.w.from === ev.name ? { ...x, here: ev.kind === "joined" } : x)));
      }
    });
    const un2 = listen<Omit<Hit, "id" | "at" | "sent">>("live-hit", (e) => {
      setHits((h) => [{ ...e.payload, id: next.current++, at: Date.now(), sent: [] }, ...h].slice(0, 6));
    });
    const t = setInterval(() => setNow(Date.now()), 30000);
    return () => { un.then((f) => f()); un2.then((f) => f()); clearInterval(t); };
  }, []);

  // Fit the window to the list; hide it when there's nothing left.
  useEffect(() => {
    if (list.length === 0 && hits.length === 0) { invoke("hide_trades_window"); return; }
    if (!box.current) return;
    const h = Math.ceil(box.current.getBoundingClientRect().height) + 4;
    getCurrentWindow().setSize(new LogicalSize(340, Math.max(80, Math.min(700, h)))).then(() => invoke("keep_on_screen")).catch(() => {});
  });

  const remove = (id: number) => setList((l) => l.filter((x) => x.id !== id));
  const press = async (x: Entry, action: string) => {
    setErr("");
    try {
      await invoke("trade_action", { action, name: x.w.from });
      if (action === "kick") remove(x.id);
      else setList((l) => l.map((y) => (y.id === x.id ? { ...y, pressed: [...y.pressed, action] } : y)));
    } catch (e) { setErr(friendly(e)); }
  };

  const whisper = async (h: Hit, i: number) => {
    setErr("");
    try { await invoke("send_whisper", { text: h.listings[i].whisper }); setHits((hs) => hs.map((y) => (y.id === h.id ? { ...y, sent: [...y.sent, i] } : y))); }
    catch (e) { setErr(friendly(e)); }
  };

  if (list.length === 0 && hits.length === 0) return null;
  return (
    <div className="qm th" ref={box}>
      <WinBar compact title="Trades" onClose={() => invoke("hide_trades_window")} closeTip="Hide (new trades show it again)">
        <button className="link small" onClick={() => { setList([]); setHits([]); }} title="Clear the list">Clear all</button>
      </WinBar>
      {hits.map((h) => (
        <div key={h.id} className="th-row th-live">
          <div className="th-top">
            <b className="th-item">● New on “{h.search}”</b>
            <button className="th-x" onClick={() => setHits((hs) => hs.filter((y) => y.id !== h.id))} title="Dismiss">×</button>
          </div>
          {h.listings.slice(0, 5).map((l, i) => (
            <div key={i} className="th-live-row">
              <span className="th-price">{+l.amount.toFixed(2)} {l.currency}</span>
              <span className="muted small">{l.account}</span>
              {l.instant ? <span className="tw-instant">Instant</span> : l.whisper && <button className={h.sent.includes(i) ? "done" : ""} onClick={() => whisper(h, i)} title="Send the seller the trade whisper in game">Whisper</button>}
            </div>
          ))}
          <div className="muted small">{since(h.at, now)} · <button className="link small" onClick={() => invoke("open_url", { url: h.url })}>Open on the trade site</button></div>
        </div>
      ))}
      {list.map((x) => (
        <div key={x.id} className={`th-row ${x.here ? "here" : ""}`}>
          <div className="th-top">
            <b className="th-item" title={x.w.message}>{x.w.item}</b>
            <button className="th-x" onClick={() => remove(x.id)} title="Dismiss">×</button>
          </div>
          <div className="th-sub muted small">
            {x.w.price && <span className="th-price">{x.w.price}</span>}
            <span>{x.w.from}</span>
            {x.here && <span className="th-here">in your area</span>}
            <span>{since(x.at, now)}</span>
          </div>
          {x.check && <div className={`th-check ${x.check.verdict}`}>{x.check.verdict === "low" ? `Underpriced? poe.ninja says about ${Math.round(x.check.worth)}c`
            : x.check.verdict === "high" ? `Good sale: poe.ninja says about ${Math.round(x.check.worth)}c` : `Fair price (poe.ninja about ${Math.round(x.check.worth)}c)`}</div>}
          {x.w.tab && <div className="muted small th-tab">Tab “{x.w.tab}”{x.w.left != null ? ` · left ${x.w.left}, top ${x.w.top}` : ""}
            {x.w.left != null && x.w.top != null && <>
              <button className="link small" title="Open that stash tab, then click: a box shows which square the item is in" onClick={() => invoke("show_stash_spot", { left: x.w.left, top: x.w.top, quad: false }).catch((e) => setErr(friendly(e)))}>Show in stash</button>
              {x.w.left <= 12 && x.w.top <= 12 && <button className="link small" title="The same, for a quad tab" onClick={() => invoke("show_stash_spot", { left: x.w.left, top: x.w.top, quad: true }).catch((e) => setErr(friendly(e)))}>(quad)</button>}
            </>}</div>}
          <div className="th-btns">
            {BUTTONS.map(([a, label, tip]) => (
              <button key={a} className={x.pressed.includes(a) ? "done" : ""} title={tip} onClick={() => press(x, a)}>{label}</button>
            ))}
          </div>
        </div>
      ))}
      {err && <div className="qm-warn">{err}</div>}
    </div>
  );
}
