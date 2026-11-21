import { Spinner } from "./TradeWindow";
import { usePersist } from "./usePersist";
import { friendly } from "./errors";
import { PageLoading } from "./Loading";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useState } from "react";
import { CurrencyIcon } from "./icons";

export type Tab = { index: number; name: string; kind: string; colour: string };
export type Snapshot = { time: number; league: string; total: number; divine: number | null; tabs: [string, number][]; profile?: string };
export type Valued = { tab: string; name: string; stack: number; value: number; x: number; y: number; listed: boolean; icon: string; places?: number; rare?: boolean };
export type ScanProfile = { name: string; tabs: number[] };
export type Session = { name: string; league: string; tabs: number[]; start: number; end: number | null; start_value: number; last_value: number;
  paused: number; paused_at: number | null };
export type WealthView = { account: string; has_sessid: boolean; league: string; tabs: Tab[]; tracked: number[]; min: number; minutes: number;
  snapshots: Snapshot[]; missed: Valued[]; sessions: Session[]; current: Session | null; strategies: [string, number, number, number][];
  divine: number | null; now: number; profiles?: ScanProfile[]; active?: string | null; last_items?: Valued[];
  rares?: number; rares_priced?: number; rares_todo?: number; count_rares?: boolean };

/** Seconds a session has run (pauses taken out). */
export function seconds(s: Session, now: number) {
  const end = s.paused_at ?? s.end ?? now;
  return Math.max(0, end - s.start - s.paused);
}
export function perHour(s: Session, now: number) {
  const t = seconds(s, now);
  return t < 60 ? 0 : ((s.last_value - s.start_value) * 3600) / t;
}
export function clock(t: number) {
  const h = Math.floor(t / 3600), m = Math.floor((t % 3600) / 60), sec = Math.floor(t % 60);
  return h ? `${h}h ${String(m).padStart(2, "0")}m` : `${m}m ${String(sec).padStart(2, "0")}s`;
}

/** "1,234c (5.9 div)" with orb icons. */
export function Worth({ c, divine, big }: { c: number; divine: number | null; big?: boolean }) {
  const sign = c < 0 ? "−" : "";
  const a = Math.abs(c);
  return (
    <span className={big ? "w-big" : ""}>
      {sign}{a >= 100 ? Math.round(a).toLocaleString() : +a.toFixed(1)}<CurrencyIcon id="chaos" />
      {divine ? <span className="muted"> = {sign}{+(a / divine).toFixed(a / divine >= 10 ? 0 : 2)}<CurrencyIcon id="divine" /></span> : null}
    </span>
  );
}

/** Stash worth over time: a small line chart. */
function History({ snaps, divine }: { snaps: Snapshot[]; divine: number | null }) {
  if (snaps.length < 2) return <p className="muted small">Scan again later to see your worth over time.</p>;
  const pts = snaps.slice(-60);
  const W = 560, H = 120, pad = 6;
  const lo = Math.min(...pts.map((p) => p.total)), hi = Math.max(...pts.map((p) => p.total));
  const t0 = pts[0].time, t1 = pts[pts.length - 1].time || t0 + 1;
  const x = (t: number) => pad + ((t - t0) / Math.max(1, t1 - t0)) * (W - 2 * pad);
  const y = (v: number) => H - pad - ((v - lo) / Math.max(1, hi - lo)) * (H - 2 * pad);
  const d = pts.map((p, i) => `${i ? "L" : "M"}${x(p.time).toFixed(1)},${y(p.total).toFixed(1)}`).join(" ");
  return (
    <div>
      <svg className="w-chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none">
        <path d={d} fill="none" stroke="#e8c96a" strokeWidth={2} vectorEffect="non-scaling-stroke" />
        {pts.map((p) => <circle key={p.time} cx={x(p.time)} cy={y(p.total)} r={2.5} fill="#e8c96a"><title>{new Date(p.time * 1000).toLocaleString()}: {Math.round(p.total)}c</title></circle>)}
      </svg>
      <div className="muted small row-between"><span>{new Date(t0 * 1000).toLocaleDateString()}</span>
        <span>low <Worth c={lo} divine={divine} /> · high <Worth c={hi} divine={divine} /></span>
        <span>{new Date(t1 * 1000).toLocaleDateString()}</span></div>
    </div>
  );
}

/** What each tab is worth after a scan; pick one to see its items, most valuable first (same item stacks added up). */
function ByTab({ tabs, items, divine }: { tabs: [string, number][]; items: Valued[]; divine: number | null }) {
  const [pick, setPick] = useState<string | null>(null);
  const tab = pick && tabs.some(([t]) => t === pick) ? pick : tabs[0]?.[0] ?? null;
  const top = Math.max(...tabs.map(([, c]) => c), 1);
  const list = useMemo(() => {
    const groups: (Valued & { places: number })[] = [];
    for (const it of items.filter((i) => i.tab === tab)) {
      const g = groups.find((x) => x.name === it.name);
      if (g) { g.stack += it.stack; g.value += it.value; g.places += 1; g.listed = g.listed && it.listed; }
      else groups.push({ ...it, places: 1 });
    }
    return groups.sort((a, b) => b.value - a.value);
  }, [items, tab]);
  const total = tabs.find(([t]) => t === tab)?.[1] ?? 0;
  return (
    <div className="card">
      <h2>By tab</h2>
      <p className="muted small">What each tab was worth in your last scan. Click a tab to see what's in it.</p>
      <div className="w-bytab">
        {tabs.map(([t, c]) => (
          <button key={t} className={`w-bytab-chip ${t === tab ? "on" : ""}`} onClick={() => setPick(t)}>
            <span className="w-bytab-name">{t}</span>
            <span className="w-bytab-val"><Worth c={c} divine={null} /></span>
            <span className="w-bytab-bar"><span style={{ width: `${Math.max(3, (c / top) * 100)}%` }} /></span>
          </button>
        ))}
      </div>
      {tab && (
        <>
          <div className="row-between w-bytab-head">
            <b>{tab}</b>
            <span><Worth c={total} divine={divine} /> <span className="muted small">· {list.length} kinds of item</span></span>
          </div>
          {list.length === 0 ? <p className="muted small">Scan again to see this tab's items (older scans only kept the totals).</p> : (
            <table className="w-table w-missed"><tbody>
              {list.slice(0, 200).map((m, i) => (
                <tr key={i}>
                  <td>{m.icon && <img src={m.icon} alt="" width={24} height={24} />}</td>
                  <td>{m.name}{m.stack > 1 ? ` ×${m.stack}` : ""}{m.places > 1 && <span className="muted small"> in {m.places} places</span>}
                    {m.rare && <RareTag />}
                    {m.stack > 1 && <div className="muted small">{+(m.value / m.stack).toFixed(2)}c each</div>}</td>
                  <td className="muted small">{m.listed ? <span className="w-listed">listed</span> : m.places === 1 ? `${m.x + 1},${m.y + 1}` : ""}</td>
                  <td><Worth c={m.value} divine={divine} /></td>
                </tr>
              ))}
            </tbody></table>
          )}
        </>
      )}
    </div>
  );
}

type Found = { tab_index: number; tab: string; name: string; kind: string; stack: number; x: number; y: number; icon: string;
  ilvl: number; mods: string; note: string; value: number | null };
type FindResult = { items: number; time: number; league: string; total: number; hits: Found[] };

/** Find in stash: every item from the last scans (all tabs), by name, kind, tab or mod. Like PoeStash's search. */
function FindInStash({ divine, scanned }: { divine: number | null; scanned: number }) {
  const [q, setQ] = usePersist("wealth.find", "");
  const [r, setR] = useState<FindResult | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [copied, setCopied] = useState<number | null>(null);
  useEffect(() => {
    const t = setTimeout(() => invoke<FindResult>("wealth_find", { query: q }).then(setR).catch(() => {}), 120);
    return () => clearTimeout(t);
  }, [q, scanned]);
  const copy = (i: number, f: Found) => {
    // The game's stash search (Ctrl+F) lights up items matching this.
    invoke("copy_text", { text: `"${f.name.replace(/"/g, "")}"` }).then(() => { setCopied(i); setTimeout(() => setCopied(null), 1500); });
  };
  return (
    <div className="card">
      <h2>Find in stash</h2>
      <input className="text w-find" value={q} placeholder="Search every tab, e.g. divine, maximum life, ring -unique"
        onChange={(e) => { setQ(e.target.value); setOpen(null); }} />
      <p className="muted small">
        {!r || r.items === 0 ? "Scan your stash first: the search looks through what the last scan saw."
          : <>Searching {r.items.toLocaleString()} items from your scans ({new Date(r.time * 1000).toLocaleString()}). Names, kinds (rare, unique,
            gem…), tabs and mods all count; put a minus in front to leave a word out.</>}
      </p>
      {r && q.trim() && (r.total === 0 ? <p className="muted small">Nothing matches.</p> : (
        <>
          {r.total > r.hits.length && <p className="muted small">{r.total} found, showing the first {r.hits.length}.</p>}
          <table className="w-table w-missed"><tbody>
            {r.hits.map((f, i) => (
              <tr key={i} className="w-found" onClick={() => setOpen(open === i ? null : i)}>
                <td>{f.icon && <img src={f.icon} alt="" width={24} height={24} />}</td>
                <td>{f.name}{f.stack > 1 ? ` ×${f.stack}` : ""}
                  <div className="muted small">{f.kind}{f.ilvl ? ` · ilvl ${f.ilvl}` : ""}{f.note && ` · ${f.note}`}</div>
                  {open === i && f.mods && <pre className="w-mods">{f.mods}</pre>}</td>
                <td className="muted small">{f.tab || `Tab ${f.tab_index + 1}`} · {f.x + 1},{f.y + 1}</td>
                <td>{f.value != null ? <Worth c={f.value} divine={divine} /> : <span className="muted small">–</span>}</td>
                <td><button className="link small" title="Copies the name: paste it into the stash search in game (Ctrl+F) to light it up"
                  onClick={(e) => { e.stopPropagation(); copy(i, f); }}>{copied === i ? "Copied" : "Copy"}</button></td>
              </tr>
            ))}
          </tbody></table>
        </>
      ))}
    </div>
  );
}

/** A rare priced from similar items on the trade site: a rough guess. */
function RareTag() {
  return <span className="w-rare" title="Priced from similar rares for sale on the trade site (a rough guess)">rare · trade guess</span>;
}

/** Scan profiles, like WealthyExile: named sets of tabs ("Dump tabs", "Currency"), or the whole stash. */
function Profiles({ v, setV, setErr, busy }: { v: WealthView; setV: (w: WealthView) => void; setErr: (e: string) => void; busy: boolean }) {
  // Editing: [name, tabs, the name it had before (null for a new one)].
  const [edit, setEdit] = useState<[string, number[], string | null] | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const list = v.profiles ?? [];
  const call = (cmd: string, args: Record<string, unknown>) => invoke<WealthView>(cmd, args).then((w) => { setV(w); setErr(""); return w; });
  const active = list.find((p) => p.name === v.active) ?? null;
  return (
    <div className="w-profiles">
      <div className="w-prof-row">
        <span className="muted small">Scan</span>
        <button className={`w-prof ${!active ? "on" : ""}`} disabled={busy} onClick={() => call("wealth_profile_use", { name: null })}>Whole stash</button>
        {list.map((p) => (
          <button key={p.name} className={`w-prof ${active?.name === p.name ? "on" : ""}`} disabled={busy}
            title={`${p.tabs.length} tab${p.tabs.length === 1 ? "" : "s"}`} onClick={() => call("wealth_profile_use", { name: p.name })}>{p.name}</button>
        ))}
        <button className="link small" disabled={busy} onClick={() => setEdit(["", [], null])}>+ New profile</button>
        {active && <button className="link small" disabled={busy} onClick={() => setEdit([active.name, active.tabs, active.name])}>Edit</button>}
      </div>
      {edit && (
        <div className="w-prof-edit">
          <div className="row">
            <input className="text" autoFocus value={edit[0]} placeholder="Profile name, e.g. Dump tabs" onChange={(e) => setEdit([e.target.value, edit[1], edit[2]])} />
            <button className="link small" disabled={refreshing} onClick={() => { setRefreshing(true);
              invoke<WealthView>("wealth_tabs_refresh").then(setV).catch((e) => setErr(friendly(e))).finally(() => setRefreshing(false)); }}>
              {refreshing ? "Reading tabs…" : "Refresh tab list"}</button>
          </div>
          <p className="muted small">Tick the tabs this profile scans. Its worth and history are kept apart from the whole stash's.</p>
          <div className="w-tabpick">
            {v.tabs.map((t) => (
              <label key={t.index} className={`w-tabchip ${edit[1].includes(t.index) ? "on" : ""}`} style={{ borderColor: t.colour }}>
                <input type="checkbox" checked={edit[1].includes(t.index)}
                  onChange={() => setEdit([edit[0], edit[1].includes(t.index) ? edit[1].filter((x) => x !== t.index) : [...edit[1], t.index], edit[2]])} />
                {t.name || `Tab ${t.index + 1}`}
              </label>
            ))}
          </div>
          <div className="row">
            <button className="primary" disabled={!edit[0].trim() || edit[1].length === 0}
              onClick={() => call("wealth_profile_save", { name: edit[0], tabs: edit[1], old: edit[2] }).then(() => setEdit(null)).catch((e) => setErr(friendly(e)))}>
              Save profile</button>
            <button className="link small" onClick={() => setEdit(null)}>Cancel</button>
            {edit[2] && <button className="link small danger-text" onClick={() => call("wealth_profile_delete", { name: edit[2] }).then(() => setEdit(null))}>Delete profile</button>}
            <span className="muted small">{edit[1].length} tab{edit[1].length === 1 ? "" : "s"} ticked</span>
          </div>
        </div>
      )}
    </div>
  );
}

/** Wealth tracker page: stash worth, things worth selling, and earnings per strategy. */
export default function Wealth() {
  const [v, setV] = useState<WealthView | null>(null);
  const [account, setAccount] = useState("");
  const [sessid, setSessid] = useState("");
  const [busy, setBusy] = useState("");
  const [err, setErr] = useState("");
  const [progress, setProgress] = useState<[number, number, string] | null>(null);
  const [name, setName] = useState("");
  const [now, setNow] = useState(Date.now() / 1000);
  const [showSetup, setShowSetup] = useState(false);

  useEffect(() => {
    invoke<WealthView>("wealth_get").then((w) => { setV(w); setAccount(w.account); }).catch((e) => setErr(friendly(e)));
    const a = listen<[number, number, string]>("wealth-progress", (e) => setProgress(e.payload));
    const b = listen<WealthView>("earn-update", (e) => setV(e.payload));
    const c = listen<string>("earn-error", (e) => setErr(e.payload));
    const t = setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => { a.then((f) => f()); b.then((f) => f()); c.then((f) => f()); clearInterval(t); };
  }, []);

  const run = async (label: string, cmd: string, args: Record<string, unknown> = {}): Promise<WealthView | null> => {
    setBusy(label); setErr(""); setProgress(null);
    let out: WealthView | null = null;
    try { out = await invoke<WealthView>(cmd, args); setV(out); } catch (e) { setErr(friendly(e)); }
    setBusy(""); setProgress(null);
    return out;
  };
  // "Also price new rares": a scan goes straight on to pricing the rares that need it.
  const [autoRares, setAutoRares] = usePersist("wealth.autoRares", true);
  const scan = async () => {
    const w = await run("scan", "wealth_scan");
    if (w && autoRares && (w.rares_todo ?? 0) > 0) await run("rares", "wealth_price_rares");
  };
  const prefs = (p: Partial<{ tracked: number[]; min: number; minutes: number }>) => {
    if (!v) return;
    invoke<WealthView>("wealth_prefs", { tracked: p.tracked ?? v.tracked, min: p.min ?? v.min, minutes: p.minutes ?? v.minutes })
      .then(setV).catch((e) => setErr(friendly(e)));
  };
  const names = useMemo(() => [...new Set((v?.sessions ?? []).map((s) => s.name))], [v]);

  if (!v) return err ? <div className="card">{err}</div> : <PageLoading />;
  const connected = v.has_sessid && v.account && v.tabs.length > 0;
  // History and totals for the profile in use ("" = whole stash) in this league.
  const mine = v.snapshots.filter((s) => s.league === v.league && (s.profile ?? "") === (v.active ?? ""));
  const last = mine[mine.length - 1];
  const prev = mine[mine.length - 2];
  const cur = v.current;

  return (
    <>
      {(!connected || showSetup) && (
        <div className="card">
          <h2>Connect your stash</h2>
          <p className="muted small">The wealth tracker reads your stash tabs from pathofexile.com, the same way tools like Acquisition do.
            It needs your account name and your <b>POESESSID</b>, the cookie that keeps you logged in on pathofexile.com.
            It's saved on this PC only and only ever sent to pathofexile.com. Treat it like a password: never paste it anywhere else.</p>
          <ol className="muted small w-steps">
            <li>Log in on pathofexile.com in your browser.</li>
            <li>Press F12, open <b>Application</b> (Chrome/Edge) or <b>Storage</b> (Firefox), then <b>Cookies → pathofexile.com</b>.</li>
            <li>Copy the value next to <b>POESESSID</b> and paste it below. Logging out of the website ends it; just paste a new one.</li>
          </ol>
          <div className="w-form">
            <label><span>Account name</span><input className="text" value={account} placeholder="Name#1234" onChange={(e) => setAccount(e.target.value)} /></label>
            <label><span>POESESSID</span><input className="text" type="password" value={sessid}
              placeholder={v.has_sessid ? "saved (paste a new one to change it)" : "32 letters and numbers"} onChange={(e) => setSessid(e.target.value)} /></label>
          </div>
          <div className="row">
            <button className="primary" disabled={!!busy || !account.trim()} onClick={() => run("setup", "wealth_setup", { account, sessid: sessid || null }).then(() => { setSessid(""); setShowSetup(false); })}>
              {busy === "setup" ? "Checking…" : "Connect"}</button>
            <span className="muted small">League: {v.league} (from your price settings)</span>
          </div>
        </div>
      )}
      {err && <div className="card note warn-text">{err}</div>}
      {connected && last && (
        <div className="tiles">
          <div className="tile gold">
            <div className="tile-lab">{v.active ? v.active : "Stash"} worth</div>
            <div className="tile-val">{Math.round(last.total).toLocaleString()}<CurrencyIcon id="chaos" /></div>
            {(v.divine ?? last.divine) ? <div className="tile-sub">= {+(last.total / (v.divine ?? last.divine)!).toFixed(2)} div</div> : null}
          </div>
          <div className="tile">
            <div className="tile-lab">Since the scan before</div>
            {prev ? <div className={`tile-val ${last.total >= prev.total ? "up" : "down"}`}>{last.total >= prev.total ? "+" : "−"}{Math.round(Math.abs(last.total - prev.total)).toLocaleString()}<CurrencyIcon id="chaos" /></div>
              : <div className="tile-val muted">–</div>}
            <div className="tile-sub">scanned {new Date(last.time * 1000).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</div>
          </div>
          <div className="tile">
            <div className="tile-lab">Worth selling</div>
            <div className="tile-val">{v.missed.length}</div>
            <div className="tile-sub">{Math.round(v.missed.reduce((a, m) => a + m.value, 0)).toLocaleString()}c not listed yet</div>
          </div>
          <div className="tile">
            <div className="tile-lab">Earning now</div>
            {cur ? <div className="tile-val">{Math.round(perHour(cur, now)).toLocaleString()}<CurrencyIcon id="chaos" /><span className="tile-sub"> /h</span></div>
              : <div className="tile-val muted">–</div>}
            <div className="tile-sub">{cur ? `${cur.name} · ${clock(seconds(cur, now))}` : "Start a session below"}</div>
          </div>
        </div>
      )}

      {connected && (
        <div className="card">
          <div className="row-between">
            <h2>{v.active ? `${v.active} is worth` : "Your stash is worth"}</h2>
            <button className="link small" onClick={() => setShowSetup(!showSetup)}>{v.account} · {v.league}</button>
          </div>
          <Profiles v={v} setV={setV} setErr={setErr} busy={!!busy} />
          {last ? (
            <div className="w-total">
              <div className="muted small">Scanned {new Date(last.time * 1000).toLocaleString()} · counts what poe.ninja prices (currency, uniques, cards, maps, fragments, gems…; not cards hardly anyone trades)
                {(v.rares ?? 0) === 0 || v.count_rares === false ? ", not rares"
                  : `, plus ${v.rares_priced ?? 0} of your ${v.rares} rares priced on the trade site`}.</div>
            </div>
          ) : <p className="muted">Scan your stash to see what it's worth.</p>}
          <div className="row">
            <button className="primary" disabled={!!busy} onClick={scan}>{busy === "scan" ? <><Spinner /> Scanning…</> : v.active ? `Scan ${v.active}` : "Scan whole stash"}</button>
            {progress && busy === "scan" && <span className="muted small">Tab {progress[0]} of {progress[1]} {progress[2] && `· ${progress[2]}`}</span>}
            {!busy && <label className="small muted" title="After the scan, look up the rares that don't have a recent trade price (about 10 a minute)">
              <input type="checkbox" checked={autoRares} onChange={(e) => setAutoRares(e.target.checked)} /> also price new rares</label>}
          </div>
          {(v.rares ?? 0) > 0 && (
            <div className="row">
              <button disabled={!!busy || (v.rares_todo ?? 0) === 0} onClick={() => run("rares", "wealth_price_rares")}
                title="Searches the trade site for each rare, like Ctrl+D does, and counts the going price">
                {busy === "rares" ? <><Spinner /> Pricing rares…</> : (v.rares_todo ?? 0) === 0 ? "Rares priced"
                  : `Price ${Math.min(40, v.rares_todo ?? 0)} rare${(v.rares_todo ?? 0) === 1 ? "" : "s"}`}</button>
              {busy === "rares" && progress
                ? <span className="muted small">{Math.min(progress[0] + 1, progress[1])} of {progress[1]} {progress[2] && `· ${progress[2]}`}</span>
                : <span className="muted small">{v.rares_priced ?? 0} of {v.rares} rares priced{(v.rares_todo ?? 0) > 40 ? " · 40 at a time" : ""}.
                  About 10 a minute (the trade site's limit); your own Ctrl+D checks still go first.</span>}
            </div>
          )}
          {(v.rares ?? 0) > 0 && (
            <label className="row small" title="Rare prices are rough guesses from similar items for sale. Unticked, they stay out of your worth, tabs and earnings, but still show in Worth selling and Find in stash.">
              <input type="checkbox" checked={v.count_rares ?? true} disabled={!!busy}
                onChange={(e) => invoke<WealthView>("wealth_count_rares", { count: e.target.checked }).then(setV).catch((x) => setErr(friendly(x)))} />
              Count rares in my worth</label>
          )}
          {busy === "rares" && progress && <div className="w-bar"><div style={{ width: `${(progress[0] / Math.max(1, progress[1])) * 100}%` }} /></div>}
          {busy === "scan" && progress && <div className="w-bar"><div style={{ width: `${(progress[0] / Math.max(1, progress[1])) * 100}%` }} /></div>}
          <p className="muted small">pathofexile.com only allows so many requests a minute, so a big stash takes a few minutes. The app waits its turn.</p>
          <History snaps={mine} divine={v.divine} />
        </div>
      )}

      {connected && <FindInStash divine={v.divine} scanned={last?.time ?? 0} />}

      {connected && last && last.tabs.length > 0 && <ByTab tabs={last.tabs} items={v.last_items ?? []} divine={v.divine} />}

      {connected && (
        <div className="card">
          <h2>Worth selling ({v.missed.length})</h2>
          <p className="muted small">Items from your last scan worth at least
            <input className="num w-min" type="number" min={0} value={v.min} onChange={(e) => prefs({ min: +e.target.value })} /> chaos
            that don't have a price note yet. Scan again after changing this.</p>
          {v.missed.length === 0 ? <p className="muted small">{last ? "Nothing unlisted above that value." : "Scan your stash first."}</p> : (
            <table className="w-table w-missed"><tbody>
              {v.missed.map((m, i) => (
                <tr key={i}>
                  <td>{m.icon && <img src={m.icon} alt="" width={24} height={24} />}</td>
                  <td>{m.name}{m.stack > 1 ? ` ×${m.stack}` : ""}{(m.places ?? 1) > 1 && <span className="muted small"> in {m.places} places</span>}
                    {m.rare && <RareTag />}
                    {m.stack > 1 && <div className="muted small">{+(m.value / m.stack).toFixed(2)}c each</div>}</td>
                  <td className="muted small">{m.tab}{(m.places ?? 1) === 1 ? ` · ${m.x + 1},${m.y + 1}` : ""}</td>
                  <td><Worth c={m.value} divine={v.divine} /></td>
                </tr>
              ))}
            </tbody></table>
          )}
        </div>
      )}

      {connected && (
        <div className="card">
          <h2>Earnings per strategy</h2>
          <p className="muted small">Pick the tabs your loot goes into. Start a session named after what you're running.
            The app checks those tabs every {v.minutes} minutes and shows your chaos and divines per hour in a small overlay.
            Sell or move loot out of those tabs only after you stop, or it counts as a loss.</p>
          <div className="w-tabpick">
            {v.tabs.map((t) => (
              <label key={t.index} className={`w-tabchip ${v.tracked.includes(t.index) ? "on" : ""}`} style={{ borderColor: t.colour }}>
                <input type="checkbox" checked={v.tracked.includes(t.index)}
                  onChange={() => prefs({ tracked: v.tracked.includes(t.index) ? v.tracked.filter((x) => x !== t.index) : [...v.tracked, t.index] })} />
                {t.name || `Tab ${t.index + 1}`}
              </label>
            ))}
          </div>
          <div className="row">
            <span className="muted small">Check every</span>
            <input className="num" type="number" min={1} max={60} value={v.minutes} onChange={(e) => prefs({ minutes: +e.target.value })} />
            <span className="muted small">minutes</span>
          </div>

          {cur ? (
            <div className="w-session">
              <div className="row-between"><b>{cur.name}</b><span className="muted small">{cur.paused_at ? "Paused" : "Running"} · {clock(seconds(cur, now))}</span></div>
              <div className="w-session-nums">
                <div><span className="muted small">Made so far</span><Worth c={cur.last_value - cur.start_value} divine={v.divine} big /></div>
                <div><span className="muted small">Per hour</span><Worth c={perHour(cur, now)} divine={v.divine} big /></div>
              </div>
              <div className="row">
                <button onClick={() => run("check", "session_check")} disabled={!!busy}>{busy === "check" ? "Checking…" : "Check now"}</button>
                <button onClick={() => invoke<WealthView>("session_pause", { pause: !cur.paused_at }).then(setV)}>{cur.paused_at ? "Resume" : "Pause"}</button>
                <button onClick={() => invoke("earnings_overlay", { show: true })}>Show overlay</button>
                <button className="danger" disabled={!!busy} onClick={() => run("stop", "session_stop")}>{busy === "stop" ? "Stopping…" : "Stop & save"}</button>
              </div>
            </div>
          ) : (
            <div className="row">
              <input className="text" list="w-names" value={name} placeholder="Strategy name, e.g. Strand alch & go" onChange={(e) => setName(e.target.value)} />
              <datalist id="w-names">{names.map((n) => <option key={n} value={n} />)}</datalist>
              <button className="primary" disabled={!!busy || v.tracked.length === 0} onClick={() => run("start", "session_start", { name })}>
                {busy === "start" ? "Starting…" : "Start session"}</button>
            </div>
          )}

          {v.strategies.length > 0 && (
            <>
              <h3>Best strategies</h3>
              <table className="w-table"><thead><tr><th>Strategy</th><th>Sessions</th><th>Hours</th><th>Per hour</th></tr></thead><tbody>
                {v.strategies.map(([n, count, hours, ph]) => (
                  <tr key={n}><td>{n}</td><td>{count}</td><td>{hours.toFixed(1)}</td><td><Worth c={ph} divine={v.divine} /></td></tr>
                ))}
              </tbody></table>
            </>
          )}
          {v.sessions.length > 0 && (
            <details className="w-tabs"><summary className="small">Past sessions ({v.sessions.length})</summary>
              <table className="w-table"><tbody>
                {[...v.sessions].reverse().map((s) => (
                  <tr key={s.start}><td>{s.name}</td><td className="muted small">{new Date(s.start * 1000).toLocaleString()} · {clock(seconds(s, now))}</td>
                    <td><Worth c={s.last_value - s.start_value} divine={v.divine} /></td><td><Worth c={perHour(s, now)} divine={v.divine} />/h</td>
                    <td><button className="link small" title="Delete this session" onClick={() => invoke<WealthView>("session_delete", { start: s.start }).then(setV)}>×</button></td></tr>
                ))}
              </tbody></table>
            </details>
          )}
        </div>
      )}
    </>
  );
}
