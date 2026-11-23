import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { friendly } from "./errors";
import { ErrNote } from "./LoginNeeded";
import { CurrencyIcon, ItemIcon } from "./icons";
import { usePersist } from "./usePersist";
import { PageLoading } from "./Loading";
import { HighlightInStash, Worth } from "./Wealth";
import "./bulk.css";

/** Sell in bulk (Money → Sell in bulk, 23 Nov; GROBBS picked mock-up B "groups + sell basket"):
 *  group cards on the left with the items you have lots of, a sell basket on the right with the price to type in
 *  game for each, the total and a Discord post. Nothing is listed for you: the game's price box is yours to fill. */

type BulkItem = { name: string; group: string; have: number; each: number; icon: string; tabs: string; set: number | null };
type BulkView = { items: BulkItem[]; groups: string[]; divine: number | null; league: string; scanned: number | null; profile: string; has_sessid: boolean };
type Note = { amount: string; currency: string; each: number };
type Basket = { rows: { name: string; count: number; note: Note; total: number }[]; total: number; total_text: string; post: string };
type Live = { chaos: number; offers: number; url: string; fair: boolean };
/** What's in the basket: how many to sell, and the live exchange rate once checked. */
type Pick = { count: number; live?: number | null; odd?: number };

const ago = (t: number) => {
  const s = Math.max(0, Date.now() / 1000 - t);
  return s < 120 ? "just now" : s < 7200 ? `${Math.round(s / 60)} minutes ago` : s < 172800 ? `${Math.round(s / 3600)} hours ago` : `${Math.round(s / 86400)} days ago`;
};
const num = (c: number) => (c >= 100 ? Math.round(c).toLocaleString() : c >= 1 ? +c.toFixed(1) : +c.toFixed(3));

/** The stash's own picture when the scan kept it, else the app's picture list. */
function Pic({ it, size = 32 }: { it: { name: string; icon: string }; size?: number }) {
  const [broken, setBroken] = useState(false);
  if (it.icon && !broken) return <img className="item-icon" src={it.icon} alt="" width={size} height={size} loading="lazy" onError={() => setBroken(true)} />;
  return <ItemIcon name={it.name} base={it.name} size={size} />;
}

export default function Bulk() {
  const [v, setV] = useState<BulkView | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState("");
  const [progress, setProgress] = useState<[number, number, string] | null>(null);
  const [group, setGroup] = usePersist<string>("bulk.group", "");
  const [discount, setDiscount] = usePersist("bulk.discount", 10);
  const [min, setMin] = usePersist("bulk.min", 5);
  const [ign, setIgn] = usePersist("bulk.ign", "");
  const [picks, setPicks] = useState<Record<string, Pick>>({});
  const [basket, setBasket] = useState<Basket | null>(null);
  const [copied, setCopied] = useState("");
  const [title, setTitle] = useState("");

  const load = useCallback(() => invoke<BulkView>("bulk_get").then(setV).catch((e) => setErr(friendly(e))), []);
  useEffect(() => {
    load();
    const a = listen<[number, number, string]>("wealth-progress", (e) => setProgress(e.payload));
    const b = listen("prices-updated", () => load());
    return () => { a.then((f) => f()); b.then((f) => f()); };
  }, [load]);

  const byName = useMemo(() => new Map((v?.items ?? []).map((i) => [i.name, i])), [v]);
  const groupOf = (g: string) => (v?.items ?? []).filter((i) => i.group === g);
  const shownGroup = v?.groups.includes(group) ? group : v?.groups[0] ?? "";
  const rows = groupOf(shownGroup);
  const cheap = rows.filter((i) => i.each * i.have < min);
  const listed = rows.filter((i) => i.each * i.have >= min);

  // The basket's notes, total and post come from the app (same rounding as the game's price box).
  const market = (name: string) => picks[name]?.live ?? byName.get(name)?.each ?? 0;
  const autoTitle = useMemo(() => [...new Set(Object.keys(picks).map((n) => byName.get(n)?.group).filter(Boolean))].join(" & "), [picks, byName]);
  useEffect(() => {
    const list = Object.entries(picks).filter(([n]) => byName.has(n)).map(([name, p]) => ({ name, count: p.count, each: market(name) }));
    invoke<Basket>("bulk_basket", { picks: list, discount, title: title || autoTitle, character: ign }).then(setBasket).catch((e) => setErr(friendly(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [picks, discount, title, autoTitle, ign, byName]);

  const toggle = (it: BulkItem) => setPicks((p) => {
    const n = { ...p };
    if (n[it.name]) delete n[it.name]; else n[it.name] = { count: it.have };
    return n;
  });
  const tickAll = (on: boolean) => setPicks((p) => {
    const n = { ...p };
    for (const it of listed) { if (on) n[it.name] = n[it.name] ?? { count: it.have }; else delete n[it.name]; }
    return n;
  });
  const setCount = (name: string, count: number) => setPicks((p) => (p[name] ? { ...p, [name]: { ...p[name], count } } : p));

  const copy = (key: string, text: string) => invoke("copy_text", { text }).then(() => { setCopied(key); setTimeout(() => setCopied(""), 1500); });

  // What to do again after logging in from an expired-login message.
  const again = useRef<(() => void) | null>(null);
  const scan = async () => {
    again.current = scan;
    setBusy("scan"); setErr(""); setProgress(null);
    try { await invoke("wealth_scan"); await load(); } catch (e) { setErr(friendly(e)); }
    setBusy(""); setProgress(null);
  };

  // Asks the trade site's bulk exchange, one item at a time (its limits are strict; the app waits its turn).
  const checkLive = async () => {
    again.current = checkLive;
    const names = Object.keys(picks).filter((n) => byName.has(n)).sort((a, b) => market(b) * (picks[b]?.count ?? 0) - market(a) * (picks[a]?.count ?? 0)).slice(0, 12);
    setBusy("live"); setErr("");
    for (const [i, name] of names.entries()) {
      setProgress([i, names.length, name]);
      try {
        const l = await invoke<Live | null>("bulk_live", { name, each: byName.get(name)!.each });
        setPicks((p) => (p[name] ? { ...p, [name]: { ...p[name], live: l && l.fair ? l.chaos : null, odd: l && !l.fair ? l.chaos : undefined } } : p));
      } catch (e) { setErr(friendly(e)); break; }
    }
    setBusy(""); setProgress(null);
  };

  if (!v) return err ? <ErrNote msg={err} className="card" onRetry={() => { setErr(""); load(); }} /> : <PageLoading />;
  const divine = v.divine;
  const inBasket = Object.keys(picks).filter((n) => byName.has(n));

  return (
    <div className="bk">
      <div className="card bk-top">
        <div>
          {v.scanned
            ? <>From your stash scan {ago(v.scanned)}{v.profile ? <> (profile “{v.profile}”)</> : " (whole stash)"}. Items you've already priced in game are left out.</>
            : <>Your stash hasn't been read yet.</>}
          {!v.has_sessid && <div className="muted small">Connect your stash on the <b>Stash worth</b> page first.</div>}
        </div>
        <div className="row">
          {busy === "scan" && progress && <span className="muted small">Reading {progress[2]} ({progress[0] + 1} of {progress[1]})…</span>}
          <button onClick={scan} disabled={!!busy || !v.has_sessid}>{busy === "scan" ? "Reading…" : "Read my stash again"}</button>
        </div>
      </div>
      <ErrNote msg={err} className="card note warn-text" onRetry={() => { setErr(""); (again.current ?? load)(); }} />

      {v.items.length === 0 ? (
        <div className="card"><h2>Nothing to sell in bulk yet</h2>
          <p className="muted">Read your stash (here or on Stash worth). Currency, essences, cards, fragments, lifeforce, threads and the like
            that you haven't priced in game show up here, grouped.</p></div>
      ) : (
        <div className="bk-grid">
          <div>
            <div className="bk-cats">
              {v.groups.map((g) => {
                const its = groupOf(g);
                const worth = its.reduce((s, i) => s + i.each * i.have, 0);
                const picked = its.filter((i) => picks[i.name]).length;
                return (
                  <button key={g} className={`bk-cat${g === shownGroup ? " on" : ""}`} onClick={() => setGroup(g)}>
                    <Pic it={its[0]} />
                    <span><b>{g}</b><span className="muted small">{its.length} {its.length === 1 ? "kind" : "kinds"} · {num(worth)}c{picked ? ` · ${picked} picked` : ""}</span></span>
                  </button>
                );
              })}
            </div>

            <div className="card bk-list">
              <div className="row bk-list-head">
                <h2>{shownGroup}</h2>
                <span className="row small">
                  <button className="link" onClick={() => tickAll(true)}>Tick all</button>
                  <button className="link" onClick={() => tickAll(false)}>None</button>
                </span>
              </div>
              <table className="w-table bk-table">
                <thead><tr><th></th><th></th><th>Item</th><th className="n">Have</th><th className="n">poe.ninja each</th><th className="n">Stack</th></tr></thead>
                <tbody>
                  {listed.map((it) => (
                    <tr key={it.name} className={picks[it.name] ? "on" : ""} onClick={() => toggle(it)}>
                      <td><input type="checkbox" checked={!!picks[it.name]} readOnly /></td>
                      <td><Pic it={it} /></td>
                      <td>{it.name}
                        {it.set && it.have >= it.set && <span className="bk-tag">{Math.floor(it.have / it.set)} full set{it.have >= it.set * 2 ? "s" : ""}</span>}
                        <div className="muted small">{it.tabs}</div></td>
                      <td className="n">{it.have.toLocaleString()}</td>
                      <td className="n">{num(it.each)}<CurrencyIcon id="chaos" /></td>
                      <td className="n">{num(it.each * it.have)}<CurrencyIcon id="chaos" /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div className="row muted small bk-min">
                {cheap.length > 0 && <span>{cheap.length} worth under {min}c a stack hidden ·</span>}
                <label>Hide stacks under <input type="number" min={0} value={min} onChange={(e) => setMin(Math.max(0, +e.target.value || 0))} /> chaos</label>
              </div>
            </div>
          </div>

          <div className="card bk-basket">
            <h2>Sell basket</h2>
            <div className="row bk-disc">
              <span title="poe.ninja, or the live exchange rate once you click Check live prices">Price: market</span>
              <input type="range" min={0} max={30} step={1} value={discount} onChange={(e) => setDiscount(+e.target.value)} />
              <b className="bk-gold">{discount ? `−${discount}%` : "exactly"}</b>
            </div>
            {inBasket.length === 0 ? (
              <p className="muted">Tick items on the left to sell them. Each gets the price to type in game, and the whole lot a post for Discord.</p>
            ) : (
              <>
                <div className="bk-total">
                  <span className="w-big">{basket ? <Worth c={basket.total} divine={divine} /> : "…"}</span>
                  <span className="muted small">{inBasket.length} kinds · {Object.values(picks).reduce((s, p) => s + p.count, 0).toLocaleString()} items</span>
                </div>

                <h3>What to type in game</h3>
                <p className="muted small">Put them in a public premium tab, right-click each → <b>Exact price</b>, type the amount, pick the currency.
                  “43/12” means 12 for 43 chaos: the game works out the price of one.</p>
                <table className="w-table bk-notes"><tbody>
                  {(basket?.rows ?? []).map((r) => {
                    const it = byName.get(r.name);
                    const live = picks[r.name]?.live;
                    return (
                      <tr key={r.name}>
                        <td>{it && <Pic it={it} size={28} />}</td>
                        <td><span>{r.name}</span>
                          <div className="bk-sub">
                            sell <input className="bk-count" type="number" min={1} max={it?.have} value={r.count}
                              onChange={(e) => setCount(r.name, Math.max(1, Math.min(it?.have ?? 1, Math.round(+e.target.value || 1))))} /> of {it?.have.toLocaleString()}
                            {live != null && <> · live {num(live)}c (poe.ninja {num(it?.each ?? 0)}c)</>}
                            {live === null && picks[r.name]?.odd == null && <> · nobody selling on the exchange</>}
                            {picks[r.name]?.odd != null && <> · exchange asks {num(picks[r.name]!.odd!)}c, too far off: kept poe.ninja</>}
                          </div></td>
                        <td className="n"><span className="bk-note">{r.note.amount}</span> <CurrencyIcon id={r.note.currency} /></td>
                        <td><button className="small" onClick={() => copy(r.name, r.note.amount)}>{copied === r.name ? "Copied" : "Copy"}</button></td>
                        <td><button className="link small" title="Take out of the basket" onClick={() => it && toggle(it)}>✕</button></td>
                      </tr>
                    );
                  })}
                </tbody></table>
                <div className="row">
                  <HighlightInStash names={inBasket} />
                </div>
                <div className="row">
                  <button onClick={checkLive} disabled={!!busy} title="Asks the trade site's bulk exchange what people sell each for now (the 12 most valuable; slow on purpose, to stay within the site's limits). The basket then prices from that instead of poe.ninja.">
                    {busy === "live" ? `Checking ${progress ? `${progress[0] + 1} of ${progress[1]}` : ""}…` : "Check live prices"}</button>
                  <button className="link" onClick={() => setPicks({})}>Empty the basket</button>
                </div>

                <h3>Or sell it all at once (Discord / TFT)</h3>
                <div className="bk-inputs">
                  <label>Title <input value={title} placeholder={autoTitle || "Bulk"} onChange={(e) => setTitle(e.target.value)} /></label>
                  <label>Your character <input value={ign} placeholder="for “IGN: @name”" onChange={(e) => setIgn(e.target.value)} /></label>
                </div>
                <pre className="bk-post">{basket?.post}</pre>
                <button className="primary" onClick={() => basket && copy("post", basket.post)}>{copied === "post" ? "Copied" : "Copy post"}</button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
