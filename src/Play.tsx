import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { ResizeGrips, WinBar } from "./WinFrame";
import syndicate from "./data/syndicate.json";
import "./play.css";

// ---------- shared types (mirror play_app.rs / journey.rs) ----------

export type RouteOptions = { league_start: boolean; bandit: string; library: boolean };
type StepView = { act: number; parts: [string, string][]; subs: string[]; rewards: [string, boolean, string][] };
export type LevelView = {
  pos: number; total: number; steps: StepView[]; current: number; zone: string | null; zone_level: number | null;
  char_level: number | null; class: string | null; xp_keep: number | null; gem_search: string; gems_left_out: string[];
  options: RouteOptions; manual: boolean; credit: string;
  trials: { zone: string; lab: string; done: boolean; here: boolean }[]; res_penalty: number;
};
export type MapRun = {
  name: string; area: string; level: number; seed: number; start: number; last_enter: number; in_map: number; left: number;
  deaths: number; portals: number; mods: string[]; made: number | null;
};
type MapStats = { runs: number; avg_inside: number; avg_cycle: number; per_hour: number; deaths: number; avg_made: number | null };
type TrackerView = {
  current: MapRun | null; inside: boolean; current_inside: number; recent: MapRun[]; today: MapStats; all: MapStats;
  xp_per_hour: number | null; xp_left: number | null; xp_frac: number | null; xp_minutes: number | null; xp_char: string | null;
  xp_level: number | null; xp_note: string; watch_ms: number; watch_running: boolean; laps: number[]; now: number;
  session: SessionSummary | null;
};
type SessionSummary = { start: number; played: number; maps: number; map_time: number; per_hour: number; deaths: number; levels: number;
  xp_gained: number | null; made: number | null; best: MapRun | null };

/** The end-of-session card: what tonight added up to. */
function SessionCard({ s, compact }: { s: SessionSummary; compact?: boolean }) {
  const items: [string, string][] = [
    ["Played", mmss(s.played)],
    ["Maps", s.maps ? `${s.maps}${s.per_hour ? ` · ${s.per_hour.toFixed(1)}/h` : ""}` : "none"],
    ...(s.made != null ? [["Made", `≈ ${Math.round(s.made)}c`] as [string, string]] : []),
    ...(s.xp_gained != null ? [["XP gained", big(s.xp_gained)] as [string, string]] : []),
    ...(s.levels ? [["Levels", `+${s.levels}`] as [string, string]] : []),
    ["Deaths", String(s.deaths)],
  ];
  return (
    <div className={compact ? "trk-sec" : ""}>
      {compact && <div className="pw-small">This session</div>}
      <div className="trk-grid">{items.map(([k, v]) => <span key={k}>{k} <b>{v}</b></span>)}</div>
      {s.best && !compact && <p className="muted small">Best map: {s.best.name} ({tier(s.best)}){s.best.made != null ? `, about ${Math.round(s.best.made)}c` : `, ${mmss(s.best.in_map)}`}</p>}
    </div>
  );
}
type PlayView = { tracker: TrackerView };
export type SearchString = { name: string; text: string };
type SearchView = { saved: SearchString[]; made: SearchString[] };
type Sheet = { name: string; data: string };

export const mmss = (secs: number) => {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}` : `${m}:${String(r).padStart(2, "0")}`;
};
const big = (n: number) => n >= 1e9 ? `${(n / 1e9).toFixed(2)}B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(0)}k` : `${Math.round(n)}`;
const tier = (r: MapRun) => r.level >= 68 ? `T${r.level - 67}` : `lvl ${r.level}`;

/** Esc hides the window (and gives the game back its focus). */
function useEsc(label: string) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("play_hide", { label }); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [label]);
}

function useKeys() {
  const [k, setK] = useState<Record<string, string>>({});
  useEffect(() => { invoke<Record<string, string>>("get_settings").then((s) => setK(s ?? {})).catch(() => {}); }, []);
  return k;
}

// ---------- levelling ----------

function Part({ kind, text }: { kind: string; text: string }) {
  const cls: Record<string, string> = { enter: "lv-enter", kill: "lv-kill", quest: "lv-quest", waypoint: "lv-wp", waypoint_get: "lv-wp", quest_text: "lv-item", generic: "lv-item", arena: "lv-enter", logout: "lv-wp", trial: "lv-quest", ascend: "lv-quest" };
  return <span className={cls[kind]}>{text}</span>;
}

function Step({ s, cls }: { s: StepView; cls: string }) {
  return (
    <div className={`lv-step ${cls}`}>
      <div>{s.parts.map(([k, t], i) => <Part key={i} kind={k} text={t} />)}</div>
      {cls === "now" && s.subs.map((x, i) => <div key={i} className="lv-sub">· {x}</div>)}
      {cls !== "done" && s.rewards.map(([g, pick, npc], i) => (
        <div key={i} className={`lv-reward ${pick ? "" : "buy"}`}>{pick ? `Take ${g} as your reward` : `Buy ${g} from ${npc}`}</div>
      ))}
    </div>
  );
}

/** The levelling overlay (F8): where you are on the route, following the zones you enter. */
export function LevelWindow() {
  const [v, setV] = useState<LevelView | null>(null);
  const [copied, setCopied] = useState(false);
  const keys = useKeys();
  useEsc("level");
  const load = () => invoke<LevelView>("level_view").then(setV).catch(() => {});
  useEffect(() => {
    load();
    const un = listen("play-update", load);
    const un2 = listen("play-shown", load);
    return () => { un.then((f) => f()); un2.then((f) => f()); };
  }, []);
  if (!v) return <div className="bo pw" />;
  const cur = v.steps[v.current];
  const keep = v.xp_keep;
  const lvlGap = v.char_level != null && v.zone_level != null ? v.char_level - v.zone_level : null;
  return (
    <div className="bo pw">
      <WinBar title={cur ? `Act ${cur.act}` : "Levelling"} hint={`Esc / ${keys.key_level || "F8"} hides`} onClose={() => invoke("play_hide", { label: "level" })}>
        <button className="link pw-btn" title="Back one step" onClick={() => invoke<LevelView>("level_move", { by: -1 }).then(setV)}>‹</button>
        <span className="pw-small">{v.pos + 1}/{v.total}</span>
        <button className="link pw-btn" title="Forward one step" onClick={() => invoke<LevelView>("level_move", { by: 1 }).then(setV)}>›</button>
      </WinBar>
      <div className="bo-body">
        <div className="pw-row between">
          <span className="trk-name">{v.zone ?? "Waiting for the game's log…"}</span>
          <span className="pw-row">
            {v.zone_level != null && <span className="pw-tag">zone {v.zone_level}</span>}
            {v.char_level != null && <span className={`pw-tag ${keep != null && keep < 0.9 ? "bad" : lvlGap != null && lvlGap < -2 ? "warn" : "good"}`}
              title={keep != null && keep < 1 ? `You keep ${Math.round(keep * 100)}% of the XP here` : "No XP penalty here"}>
              you {v.char_level}{keep != null && keep < 0.99 ? ` · ${Math.round(keep * 100)}% XP` : ""}</span>}
          </span>
        </div>
        <LevelChips v={v} />
        {v.steps.map((s, i) => <Step key={`${v.pos}-${i}`} s={s} cls={i < v.current ? "done" : i === v.current ? "now" : "next"} />)}
        {v.gem_search && <div className="lv-search">
          <div className="pw-row between"><span className="pw-small">Your build's gems at a vendor</span>
            <span className="pw-row">
              <button className="pw-btn" onClick={() => { invoke("copy_text", { text: v.gem_search }); setCopied(true); setTimeout(() => setCopied(false), 1500); }}>{copied ? "Copied" : "Copy"}</button>
              <button className="pw-btn" title="Types it into the vendor's search box (open the vendor first)" onClick={() => invoke("search_type", { text: v.gem_search })}>Type it</button>
            </span></div>
          <code title={v.gem_search}>{v.gem_search}</code>
        </div>}
      </div>
    </div>
  );
}

/** Trials, lab and resistance reminders under the zone line. */
function LevelChips({ v }: { v: LevelView }) {
  const lab = (name: string) => v.trials.filter((t) => t.lab === name);
  const normal = lab("Normal");
  const here = v.trials.find((t) => t.here);
  const nextLab = ["Normal", "Cruel", "Merciless"].find((n) => lab(n).some((t) => !t.done));
  const labReady = (n: string) => lab(n).length > 0 && lab(n).every((t) => t.done);
  return (
    <div className="pw-row" style={{ flexWrap: "wrap", marginTop: 5, gap: 5 }}>
      {here && <span className="pw-tag warn">Trial of Ascendancy in this zone</span>}
      {nextLab && <span className="pw-tag" title={lab(nextLab).map((t) => `${t.done ? "✓" : "·"} ${t.zone}`).join("\n")}>
        {nextLab} trials {lab(nextLab).filter((t) => t.done).length}/{lab(nextLab).length}</span>}
      {["Normal", "Cruel", "Merciless"].some(labReady) && normal.length > 0 &&
        <button className="pw-btn link" title="Today's lab layouts and where the treasure keys are (poelab.com)" onClick={() => invoke("open_url", { url: "https://www.poelab.com/" })}>Today's lab ↗</button>}
      {v.res_penalty < 0 && <span className="pw-tag bad" title="Every resistance is lowered after Kitava: check your gear still reaches 75%">res {v.res_penalty}% since {v.res_penalty === -30 ? "act 5" : "act 10"}</span>}
    </div>
  );
}

// ---------- map tracker ----------

/** The map tracker (F9): this map's time, maps per hour, deaths, XP per hour, a stopwatch. */
export function TrackerWindow() {
  const [v, setV] = useState<TrackerView | null>(null);
  const [now, setNow] = useState(Date.now());
  const keys = useKeys();
  useEsc("tracker");
  useEffect(() => {
    const load = () => invoke<PlayView>("play_view").then((p) => setV(p.tracker)).catch(() => {});
    load();
    const un = listen<PlayView>("play-update", (e) => setV(e.payload.tracker));
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => { un.then((f) => f()); clearInterval(t); };
  }, []);
  if (!v) return <div className="bo pw" />;
  // Tick the clocks locally between updates.
  const drift = Math.max(0, now / 1000 - v.now);
  const inside = v.current_inside + (v.inside ? drift : 0);
  const watch = v.watch_ms + (v.watch_running ? drift * 1000 : 0);
  const act = (a: string) => invoke<PlayView>("watch_action", { action: a }).then((p) => setV(p.tracker));
  const c = v.current;
  return (
    <div className="bo pw">
      <WinBar title="Map tracker" hint={`Esc / ${keys.key_tracker || "F9"}`} onClose={() => invoke("play_hide", { label: "tracker" })} />
      <div className="bo-body">
        {c ? <>
          <div className="pw-row between"><span className="trk-name">{c.name} <span className="pw-small">{tier(c)}</span></span>
            <span className={`pw-tag ${v.inside ? "good" : ""}`}>{v.inside ? "in map" : "out"}</span></div>
          <div className="pw-row between">
            <span className="trk-big">{mmss(inside)}</span>
            <button className="pw-btn" title="Finished with this map (otherwise it ends when you open the next one)" onClick={() => invoke<PlayView>("maps_done").then((p) => setV(p.tracker))}>Done</button>
          </div>
          {(c.deaths > 0 || c.portals > 0) && <div className="pw-small">{c.deaths > 0 ? `${c.deaths} death${c.deaths > 1 ? "s" : ""}` : ""}{c.deaths > 0 && c.portals > 0 ? " · " : ""}{c.portals > 0 ? `${c.portals} portal${c.portals > 1 ? "s" : ""} back in` : ""}</div>}
          {c.mods.length > 0 && <div className="trk-mods" title={c.mods.join("\n")}>{c.mods.join(" · ")}</div>}
        </> : <div className="pw-small">Open a map: the timer starts by itself.</div>}
        {v.today.runs > 0 && <div className="trk-grid">
          <span>Today <b>{v.today.runs}</b> maps</span><span><b>{v.today.per_hour.toFixed(1)}</b> per hour</span>
          <span>Avg <b>{mmss(v.today.avg_inside)}</b> in map</span>
          <span>{v.today.avg_made != null ? <>≈ <b>{Math.round(v.today.avg_made)}c</b> a map</> : <>{v.today.deaths} deaths</>}</span>
        </div>}
        <div className="trk-sec">
          {v.xp_per_hour != null ? <>
            <div className="pw-row between"><span><b>{big(v.xp_per_hour)}</b> <span className="pw-small">XP / hour</span></span>
              <span className="pw-small">{v.xp_minutes != null ? `level ${(v.xp_level ?? 0) + 1} in ${mmss(v.xp_minutes * 60)}` : ""}</span></div>
            {v.xp_frac != null && <div className="trk-bar"><span style={{ width: `${Math.round(v.xp_frac * 100)}%` }} /></div>}
          </> : <div className="pw-small">{v.xp_note || "XP per hour: measuring…"}</div>}
        </div>
        <div className="trk-sec pw-row between">
          <span className="trk-name" style={{ fontVariantNumeric: "tabular-nums" }}>{mmss(watch / 1000)}<span className="pw-small">.{Math.floor((watch % 1000) / 100)}</span></span>
          <span className="pw-row">
            {v.watch_running ? <button className="pw-btn" onClick={() => act("pause")}>Pause</button> : <button className="pw-btn" onClick={() => act("start")}>{watch > 0 ? "Go on" : "Start"}</button>}
            <button className="pw-btn" onClick={() => act("lap")} disabled={watch === 0}>Lap</button>
            <button className="pw-btn" onClick={() => act("reset")} disabled={watch === 0}>Reset</button>
          </span>
        </div>
        {v.laps.length > 0 && <div className="pw-small">{v.laps.slice(0, 4).map((l) => mmss(l / 1000)).join(" · ")}</div>}
        {v.session && v.session.played > 300 && <SessionCard s={v.session} compact />}
      </div>
    </div>
  );
}

// ---------- searches ----------

/** One-click searches (F10): click one and it's typed into the game's stash or vendor search box. */
export function SearchesWindow() {
  const [v, setV] = useState<SearchView | null>(null);
  const [err, setErr] = useState("");
  const keys = useKeys();
  useEsc("searches");
  useEffect(() => {
    const load = () => invoke<SearchView>("searches_get").then(setV).catch(() => {});
    load();
    const un = listen("play-shown", load);
    return () => { un.then((f) => f()); };
  }, []);
  const go = (t: string) => { setErr(""); invoke("search_type", { text: t }).catch((e) => setErr(String(e))); };
  const item = (s: SearchString, i: number) => (
    <button key={i} className="ss-item" onClick={() => go(s.text)} title={s.text}><span className="ss-name">{s.name}</span><code>{s.text}</code></button>
  );
  return (
    <div className="bo pw">
      <WinBar title="Searches" hint={`Esc / ${keys.key_searches || "F10"}`} onClose={() => invoke("play_hide", { label: "searches" })} />
      <div className="bo-body">
        <div className="pw-small">Open your stash or a vendor, then click one: it's typed into the search box.</div>
        {err && <div className="error small">{err}</div>}
        {v && v.made.length > 0 && <><div className="ss-head">Made for you</div><div className="ss-list">{v.made.map(item)}</div></>}
        {v && <><div className="ss-head">Saved</div><div className="ss-list">{v.saved.map(item)}</div></>}
        <div className="pw-small" style={{ marginTop: 8 }}>Add your own on the Play tools page.</div>
      </div>
    </div>
  );
}

// ---------- cheat sheets ----------

type Job = { kind: string; what: string };
type Member = { id: string; name: string; jobs: Record<string, Job[]> };
const JOBS = ["transportation", "fortification", "research", "intervention"];
const SYN_KEY = "syndicate.picks";

/** The Syndicate table: click cells to mark the rewards you want (kept on this PC). */
export function SyndicateTable() {
  const [picks, setPicks] = useState<string[]>(() => { try { return JSON.parse(localStorage.getItem(SYN_KEY) || "[]"); } catch { return []; } });
  const [find, setFind] = useState("");
  const toggle = (k: string) => {
    const next = picks.includes(k) ? picks.filter((x) => x !== k) : [...picks, k];
    setPicks(next);
    try { localStorage.setItem(SYN_KEY, JSON.stringify(next)); } catch { /* not kept */ }
  };
  const q = find.trim().toLowerCase();
  const members = (syndicate as { members: Member[] }).members;
  return (
    <div>
      <div className="pw-row">
        <input className="text" style={{ margin: 0, maxWidth: 260 }} placeholder="Find a reward (e.g. veiled, jewel)" value={find} onChange={(e) => setFind(e.target.value)} />
        <span className="pw-small">Click a reward to mark it (green). Data from Exile UI, updated {(syndicate as { updated: string }).updated.split(",")[0]}.</span>
      </div>
      <table className="syn">
        <thead><tr><th>Member</th>{JOBS.map((j) => <th key={j}>{j[0].toUpperCase() + j.slice(1)}</th>)}</tr></thead>
        <tbody>{members.map((m) => (
          <tr key={m.id}><td className="who">{m.name}</td>{JOBS.map((j) => {
            const list = m.jobs[j] ?? [];
            const k = `${m.id}:${j}`;
            const text = list.map((x) => `${x.kind} ${x.what}`).join(" ").toLowerCase();
            const dim = q !== "" && !text.includes(q);
            return (
              <td key={j} className={`${picks.includes(k) ? "pick" : ""} ${dim ? "dim" : ""}`} onClick={() => toggle(k)}>
                {list[0] && <>{list[0].kind && <span className="k">{list[0].kind}</span>}<span className="w">{list[0].what}</span></>}
                {list[1] && <div className="w2">{list[1].kind ? `${list[1].kind}: ` : ""}{list[1].what}</div>}
              </td>
            );
          })}</tr>
        ))}</tbody>
      </table>
    </div>
  );
}

/** Cheat sheets (F11): the Syndicate table, then your own pictures (add them on the Play tools page). */
export function SheetWindow() {
  const [sheets, setSheets] = useState<Sheet[]>([]);
  const [tab, setTab] = useState(() => { try { return localStorage.getItem("sheet.tab") || "syndicate"; } catch { return "syndicate"; } });
  const keys = useKeys();
  useEsc("sheet");
  useEffect(() => {
    const load = () => invoke<Sheet[]>("sheets_list").then(setSheets).catch(() => {});
    load();
    const un = listen("play-shown", load);
    return () => { un.then((f) => f()); };
  }, []);
  const pick = (t: string) => { setTab(t); try { localStorage.setItem("sheet.tab", t); } catch { /* fine */ } };
  const shown = sheets.find((s) => s.name === tab);
  return (
    <div className="bo pw">
      <WinBar title="Cheat sheets" hint={`Esc / ${keys.key_sheet || "F11"} hides`} onClose={() => invoke("play_hide", { label: "sheet" })} />
      <div className="bo-body">
        <div className="cs-tabs">
          <button className={tab === "syndicate" || !shown ? "on" : ""} onClick={() => pick("syndicate")}>Syndicate</button>
          {sheets.map((s) => <button key={s.name} className={tab === s.name ? "on" : ""} onClick={() => pick(s.name)}>{s.name}</button>)}
        </div>
        {shown ? <img className="cs-img" src={shown.data} alt={shown.name} /> : <SyndicateTable />}
      </div>
      <ResizeGrips />
    </div>
  );
}

// ---------- notes ----------

/** Notes over the game: saved as you type. */
export function NotesWindow() {
  const [text, setText] = useState<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const keys = useKeys();
  useEsc("notes");
  useEffect(() => {
    invoke<string>("notes_get").then(setText).catch(() => setText(""));
    const un = listen("play-shown", () => invoke<string>("notes_get").then(setText));
    return () => { un.then((f) => f()); };
  }, []);
  const change = (t: string) => {
    setText(t);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => invoke("notes_save", { text: t }), 400);
  };
  return (
    <div className="bo pw">
      <WinBar title="Notes" hint={`Esc${keys.key_notes ? ` / ${keys.key_notes}` : ""} hides · saved as you type`} onClose={() => invoke("play_hide", { label: "notes" })} />
      <div className="bo-body nt-body">
        <textarea className="nt-area" autoFocus value={text ?? ""} placeholder="Anything you want to remember: what to buy, which map to run next…" onChange={(e) => change(e.target.value)} />
      </div>
      <ResizeGrips />
    </div>
  );
}

// ---------- the main window's page ----------

function Tile({ title, k, children, label }: { title: string; k?: string; children: ReactNode; label?: string }) {
  return (
    <div className="pt-tile">
      <div className="pw-row between"><h3>{title}</h3>{k ? <kbd>{k}</kbd> : <span className="pw-small">no key</span>}</div>
      <div className="muted small">{children}</div>
      {label && <div className="row" style={{ margin: 0 }}><button onClick={() => invoke("play_toggle", { label })}>Show / hide</button></div>}
    </div>
  );
}

/** Play tools page: what each play window does, levelling choices, your searches, cheat sheet pictures, map runs. */
export default function PlayTools() {
  const keys = useKeys();
  const [lv, setLv] = useState<LevelView | null>(null);
  const [searches, setSearches] = useState<SearchString[] | null>(null);
  const [sheets, setSheets] = useState<Sheet[]>([]);
  const [runs, setRuns] = useState<MapRun[]>([]);
  const [msg, setMsg] = useState("");
  const [session, setSession] = useState<SessionSummary | null>(null);
  useEffect(() => {
    invoke<PlayView>("play_view").then((p) => setSession(p.tracker.session)).catch(() => {});
    invoke<LevelView>("level_view").then(setLv).catch(() => {});
    invoke<SearchView>("searches_get").then((v) => setSearches(v.saved)).catch(() => {});
    invoke<Sheet[]>("sheets_list").then(setSheets).catch(() => {});
    invoke<MapRun[]>("maps_history").then(setRuns).catch(() => {});
    const un = listen<PlayView>("play-update", (e) => { setSession(e.payload.tracker.session); invoke<MapRun[]>("maps_history").then(setRuns); });
    return () => { un.then((f) => f()); };
  }, []);
  const setOpts = (o: Partial<RouteOptions>, manual?: boolean) => {
    if (!lv) return;
    invoke<LevelView>("level_options", { options: { ...lv.options, ...o }, manual: manual ?? lv.manual }).then(setLv);
  };
  const saveSearches = (list: SearchString[]) => { setSearches(list); invoke("searches_save", { list }).catch((e) => setMsg(String(e))); };
  const addSheets = async () => {
    const picked = await open({ multiple: true, filters: [{ name: "Pictures", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }] });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    invoke<Sheet[]>("sheets_add", { paths }).then(setSheets).catch((e) => setMsg(String(e)));
  };
  const recent = useMemo(() => runs.slice().reverse().slice(0, 30), [runs]);
  const k = (name: string, def: string) => keys[name] ?? def;

  return (
    <div>
      <div className="card">
        <h2>Play tools</h2>
        <p className="muted small">Small windows over the game. They read the game's log file and copied item text only (nothing that reads the
          game's memory). Change the keys under Settings → Hotkeys.</p>
        <div className="pt-grid">
          <Tile title="Levelling" k={k("key_level", "F8")} label="level">The campaign route step by step. It moves on by itself as you enter zones,
            says which quest gem rewards your PoB build wants, warns when you're too high or low for the zone, and has a vendor search for your build's gems.</Tile>
          <Tile title="Map tracker" k={k("key_tracker", "F9")} label="tracker">Time in each map, maps per hour, deaths, the mods from a Ctrl+D just
            before you opened it, XP per hour, and a stopwatch. With an earnings session running (Wealth), roughly what each map made.</Tile>
          <Tile title="Searches" k={k("key_searches", "F10")} label="searches">Click a saved search and it's typed into the stash or vendor search box.
            Your build's gems and your map regex are added for you.</Tile>
          <Tile title="Cheat sheets" k={k("key_sheet", "F11")} label="sheet">The Syndicate rewards table (click the ones you want), plus any pictures you add below:
            Heist, lab layouts, Delve, anything.</Tile>
          <Tile title="Notes" k={k("key_notes", "") || undefined} label="notes">A notepad over the game, saved as you type.</Tile>
          <Tile title="Wiki" k={k("key_wiki", "Alt+W")}>Hover an item and press the key: its wiki page opens in your browser (uniques by name, the rest by base).</Tile>
        </div>
      </div>

      {session && <div className="card">
        <h2>This session</h2>
        <p className="muted small">Started {new Date(session.start * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}. A new session starts after 30 minutes without playing.</p>
        <SessionCard s={session} />
      </div>}

      {lv && <div className="card">
        <h2>Levelling route</h2>
        <p className="muted small">{lv.credit} Your place is kept per character.</p>
        <label className="check"><input type="checkbox" checked={lv.options.league_start} onChange={(e) => setOpts({ league_start: e.target.checked })} />
          <span>League start (first character): grab every waypoint and do the side areas</span></label>
        <label className="check"><input type="checkbox" checked={lv.options.library} onChange={(e) => setOpts({ library: e.target.checked })} />
          <span>Do the Library quest in act 3</span></label>
        <div className="row"><span className="muted small">Bandits</span>
          <select value={lv.options.bandit} onChange={(e) => setOpts({ bandit: e.target.value })}>
            <option value="kill">Kill all three</option><option value="alira">Help Alira</option><option value="oak">Help Oak</option><option value="kraityn">Help Kraityn</option>
          </select>
          <label className="check" style={{ margin: 0 }}><input type="checkbox" checked={!lv.manual} onChange={(e) => setOpts({}, !e.target.checked)} />
            <span>Move on by itself as I enter zones</span></label>
        </div>
        <div className="row"><span className="muted small">Jump to act</span>
          {[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((a) => <button key={a} className="pw-btn" onClick={() => invoke<LevelView>("level_jump_act", { act: a }).then(setLv)}>{a}</button>)}
          <span className="muted small">step {lv.pos + 1} of {lv.total}</span>
        </div>
      </div>}

      <div className="card">
        <h2>Searches</h2>
        <p className="muted small">Up to 250 characters, as the game allows. Quotes and | work like in the game ("nt sp|life").</p>
        {(searches ?? []).map((s, i) => (
          <div key={i} className="pt-search">
            <input className="text" value={s.name} placeholder="Name" onChange={(e) => saveSearches(searches!.map((x, j) => j === i ? { ...x, name: e.target.value } : x))} />
            <input className="text" value={s.text} maxLength={250} placeholder="What to type" onChange={(e) => saveSearches(searches!.map((x, j) => j === i ? { ...x, text: e.target.value } : x))} />
            <button className="link" title="Remove" onClick={() => saveSearches(searches!.filter((_, j) => j !== i))}>✕</button>
          </div>
        ))}
        <button onClick={() => setSearches([...(searches ?? []), { name: "", text: "" }])}>Add a search</button>
      </div>

      <div className="card">
        <h2>Cheat sheet pictures</h2>
        <p className="muted small">Shown as tabs in the cheat sheets window, after the Syndicate table.</p>
        <div className="row">{sheets.map((s) => (
          <span key={s.name} className="pw-tag">{s.name} <button className="link" title="Remove" onClick={() => invoke<Sheet[]>("sheets_remove", { name: s.name }).then(setSheets)}>✕</button></span>
        ))}</div>
        <button onClick={addSheets}>Add pictures…</button>
      </div>

      <div className="card">
        <div className="pw-row between"><h2>Map runs</h2>
          {runs.length > 0 && <button className="link" onClick={() => { if (confirm("Forget every map run?")) invoke("maps_clear").then(() => setRuns([])); }}>Clear</button>}</div>
        {recent.length === 0 ? <p className="muted small">None yet: open a map and the tracker counts it.</p> :
          <table className="pt-table"><thead><tr><th>Map</th><th>Tier</th><th>In map</th><th>Deaths</th><th>Made</th><th>When</th></tr></thead>
            <tbody>{recent.map((r, i) => (
              <tr key={i} title={r.mods.join("\n")}><td>{r.name}</td><td>{tier(r)}</td><td>{mmss(r.in_map)}</td><td>{r.deaths || ""}</td>
                <td>{r.made != null ? `≈ ${Math.round(r.made)}c` : ""}</td><td className="muted">{new Date(r.start * 1000).toISOString().slice(5, 16).replace("T", " ")}</td></tr>
            ))}</tbody></table>}
        {msg && <div className="error small">{msg}</div>}
      </div>
    </div>
  );
}
