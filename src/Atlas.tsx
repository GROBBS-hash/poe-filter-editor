import { Tip } from "./Help";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import AtlasTreeView, { AtlasStore, AtlasStrategy, ChoiceList, StepBar, StepList, StrategyNotes, useStep } from "./AtlasTreeView";
import AtlasTotals from "./AtlasTotals";
import TreeEditor from "./AtlasEditor";
import { PageLoading } from "./Loading";
import AtlasPicks from "./AtlasPicks";

const SITE: Record<string, string> = { maxroll: "maxroll", official: "pathofexile.com", optimiser: "built by the optimiser", planner: "Atlas Tree Planner" };

/** Endgame → Atlas: import atlas strategies from a link, keep them by name, pick the one shown in game. */
export default function Atlas() {
  const [store, setStore] = useState<AtlasStore | null>(null);
  const [link, setLink] = useState("");
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [sel, setSel] = useState<number | null>(null);
  const [key, setKey] = useState("F7");

  useEffect(() => {
    const load = () => invoke<AtlasStore>("atlas_list").then(setStore).catch((e) => setMsg(String(e)));
    load();
    invoke<{ key_atlas?: string }>("get_settings").then((s) => setKey(s?.key_atlas || "F7")).catch(() => {});
    const un = listen("atlas-changed", load);
    return () => { un.then((f) => f()); };
  }, []);

  const run = async (cmd: string, args: Record<string, unknown>) => {
    try { setStore(await invoke<AtlasStore>(cmd, args)); setMsg(""); } catch (e) { setMsg(String(e)); }
  };
  const doImport = async () => {
    setBusy(true); setMsg("");
    try {
      const s = await invoke<AtlasStore>("atlas_import", { link, name });
      setStore(s); setLink(""); setName(""); setSel(s.active);
      setMsg(`Saved “${s.list.find((x) => x.id === s.active)?.name}”. It's the one shown in game now (${key}).`);
    } catch (e) { setMsg(String(e)); }
    setBusy(false);
  };

  if (!store) return msg ? <div className="card">{msg}</div> : <PageLoading />;
  const shown = store.list.find((x) => x.id === sel) ?? store.list.find((x) => x.id === store.active) ?? store.list[0];

  return (
    <div className="atlas-page">
      <AtlasPicks store={store} onShow={(id) => setSel(id)}
        onImported={(s, m) => { setStore(s); setSel(s.active); setMsg(`${m} (${key})`); }} />
      <div className="card">
        <h2>Add a strategy from a link <Tip>A <b>maxroll farming strategy</b> link (maxroll.gg/poe/farming-strategy-planner/…) or a
          <b> pathofexile.com atlas tree</b> link, or an <b>Atlas Tree Planner</b> link (atlas-blush-beta.vercel.app/#…). maxroll strategies bring their notes, map device and threads too.</Tip></h2>
        <p className="muted small">Paste a maxroll strategy, pathofexile.com atlas tree or Atlas Tree Planner link.</p>
        <div className="row">
          <input className="text grow" placeholder="https://maxroll.gg/poe/farming-strategy-planner/…" value={link}
            onChange={(e) => setLink(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter" && link.trim() && !busy) doImport(); }} />
        </div>
        <div className="row">
          <input className="text grow" placeholder="Name (optional: uses the site's name)" value={name} onChange={(e) => setName(e.target.value)} />
          <button className="primary" disabled={!link.trim() || busy} onClick={doImport}>{busy ? "Reading…" : "Import"}</button>
        </div>
        {msg && <div className="small at-msg">{msg}</div>}
      </div>

      {store.list.length === 0 ? (
        <div className="empty">No strategies yet. Import one above, then press <b>{key}</b> in game to see it.</div>
      ) : (
        <>
          <div className="card">
            <h2>My strategies</h2>
            <p className="muted small">The one ticked “In game” shows when you press <b>{key}</b> (change the key in Settings).</p>
            <div className="at-list">
              {store.list.map((s) => (
                <StrategyRow key={s.id} s={s} active={store.active === s.id} selected={shown?.id === s.id}
                  onSelect={() => setSel(s.id)}
                  onUse={() => run("atlas_use", { id: s.id, variant: store.active === s.id ? store.variant : 0 })}
                  onRename={(n) => run("atlas_rename", { id: s.id, name: n })}
                  onDelete={() => { if (confirm(`Delete “${s.name}”?`)) run("atlas_delete", { id: s.id }); }} />
              ))}
            </div>
          </div>
          {shown && <Preview key={shown.id} s={shown} store={store} run={run} onSel={setSel} />}
        </>
      )}
    </div>
  );
}

function StrategyRow({ s, active, selected, onSelect, onUse, onRename, onDelete }: {
  s: AtlasStrategy; active: boolean; selected: boolean; onSelect: () => void; onUse: () => void; onRename: (n: string) => void; onDelete: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [n, setN] = useState(s.name);
  const pts = Math.max(0, ...s.variants.map((v) => v.nodes.length - 1));
  return (
    <div className={`at-row ${selected ? "sel" : ""}`} onClick={onSelect}>
      <label className="at-use" onClick={(e) => e.stopPropagation()} title="Show this one in game">
        <input type="radio" checked={active} onChange={onUse} /> <span className="small">In game</span>
      </label>
      {editing ? (
        <input className="text grow" autoFocus value={n} onClick={(e) => e.stopPropagation()} onChange={(e) => setN(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") { onRename(n); setEditing(false); } if (e.key === "Escape") setEditing(false); }}
          onBlur={() => { if (n.trim() && n !== s.name) onRename(n); setEditing(false); }} />
      ) : (
        <div className="at-name">
          <b>{s.name}</b>
          <span className="muted small"> · {SITE[s.site] ?? s.site}{s.author ? ` · by ${s.author}` : ""} · {s.variants.length > 1 ? `${s.variants.length} trees, up to ` : ""}{pts} points</span>
        </div>
      )}
      <span className="grow" />
      <button className="link small" onClick={(e) => { e.stopPropagation(); setN(s.name); setEditing(true); }}>Rename</button>
      <button className="link small" onClick={(e) => { e.stopPropagation(); onDelete(); }}>Delete</button>
    </div>
  );
}

function Preview({ s, store, run, onSel }: { s: AtlasStrategy; store: AtlasStore; run: (cmd: string, args: Record<string, unknown>) => Promise<void>; onSel: (id: number | null) => void }) {
  const [editing, setEditing] = useState(false);
  const [vi, setVi] = useState(store.active === s.id ? Math.min(store.variant, s.variants.length - 1) : 0);
  const [own, setOwn] = useState(s.own_notes);
  const [saved, setSaved] = useState(true);
  const v = s.variants[vi];
  const steps = v?.steps ?? [];
  const [step, setStep] = useStep(`${s.id}-${vi}`, steps.length);
  const [find, setFind] = useState<{ id: number; n: number } | null>(null);
  const finder = (id: number) => setFind((f) => ({ id, n: (f?.n ?? 0) + 1 }));
  const pick = (i: number) => { setVi(i); if (store.active === s.id) run("atlas_use", { id: s.id, variant: i }); };
  const openOfficial = async () => {
    try { invoke("open_url", { url: await invoke<string>("atlas_official_link", { id: s.id, variant: vi }) }); } catch { /* shown nowhere */ }
  };
  return (
    <>
      <div className="card">
        <div className="row-between">
          <h2>{s.name}</h2>
          <div className="row" style={{ margin: 0 }}>
            {s.link && s.site === "maxroll" && <button className="link small" onClick={() => invoke("open_url", { url: s.link })}>Open on maxroll</button>}
            {s.link && s.site === "planner" && <button className="link small" onClick={() => invoke("open_url", { url: s.link })}>Open on Atlas Tree Planner</button>}
            <button className="link small" onClick={openOfficial} title="Opens this tree on pathofexile.com">Open on pathofexile.com</button>
            {v && !editing && <button className="primary at-edit-btn" onClick={() => setEditing(true)}>Edit this tree</button>}
          </div>
        </div>
        {s.variants.length > 1 && (
          <div className="row at-variants">
            {s.variants.map((x, i) => <button key={i} className={`chip ${i === vi ? "on" : ""}`} onClick={() => pick(i)}>{x.name}</button>)}
          </div>
        )}
        {v && editing ? (
          <TreeEditor s={s} vi={vi} run={run} onDone={(id) => { setEditing(false); if (id === -1) onSel(null); else if (id != null) onSel(id); }} />
        ) : v && steps.length > 0 ? (
          <div className="at-stepgrid">
            <div className="at-stepcol">
              <div className="at-stephead"><b>Step by step</b><span className="muted small">{steps[steps.length - 1].total} points</span></div>
              <StepList steps={steps} step={step} onStep={setStep} />
            </div>
            <div>
              <StepBar steps={steps} step={step} onStep={setStep} />
              <AtlasTreeView key={vi} nodes={v.nodes} choices={v.choices} steps={steps} step={step} height={540} find={find} />
              <ChoiceList nodes={v.nodes} choices={v.choices} steps={steps} step={step} onFind={finder} />
            </div>
          </div>
        ) : v && <>
          <AtlasTreeView key={vi} nodes={v.nodes} choices={v.choices} height={560} find={find} />
          <ChoiceList nodes={v.nodes} choices={v.choices} onFind={finder} />
        </>}
        {v && !editing && <AtlasTotals nodes={v.nodes} choices={v.choices} />}
      </div>
      <div className="card">
        <h2>Notes</h2>
        <h3>My notes</h3>
        <p className="muted small">Shown in game above the site's notes: map to run, what to buy, what to sell…</p>
        <textarea className="at-own" rows={4} value={own} placeholder="e.g. Run City Square, sell lifeforce in bulk every 5 maps"
          onChange={(e) => { setOwn(e.target.value); setSaved(false); }} />
        <div className="row">
          <button disabled={saved} onClick={async () => { await run("atlas_own_notes", { id: s.id, text: own }); setSaved(true); }}>{saved ? "Saved" : "Save my notes"}</button>
        </div>
        <h3>From {SITE[s.site] ?? "the site"}</h3>
        <StrategyNotes s={{ ...s, own_notes: "" }} v={v} open={["About", "Map device & setup"]} />
      </div>
    </>
  );
}
