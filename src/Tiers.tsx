import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useState } from "react";
import { ProfileInfo } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, Preview, fromCurrent } from "./StyleEditor";
import { ItemIcon } from "./icons";

type TierRow = { tier: string; name: string; visibility: string; style: CurrentStyle; bases: string[]; note: string };
type Section = { type_path: string[]; title: string; group: string; tiers: TierRow[] };

const SHOW_ITEMS = 24;

/** Like FilterBlade: NeverSink's tier lists. Tick to show/hide a tier, click its label to restyle it,
 *  drag items between tiers to re-rank them. */
export default function Tiers({ profile, onChange }: { profile: ProfileInfo; onChange: () => void }) {
  const [sections, setSections] = useState<Section[]>([]);
  const [sel, setSel] = useState<string>("");
  const [q, setQ] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [drag, setDrag] = useState<{ base: string; from: string } | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [msg, setMsg] = useState("");

  const load = () => invoke<Section[]>("get_tiers", { id: profile.id }).then((s) => {
    setSections(s);
    setSel((cur) => (cur && s.some((x) => x.type_path.join(">") === cur) ? cur : s[0] ? s[0].type_path.join(">") : ""));
  }).catch((e) => setMsg(String(e)));
  useEffect(() => { load(); }, [profile.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const groups = useMemo(() => {
    const g = new Map<string, Section[]>();
    for (const s of sections) g.set(s.group, [...(g.get(s.group) ?? []), s]);
    return [...g.entries()];
  }, [sections]);

  // Searching jumps to whichever section holds the item.
  const needle = q.trim().toLowerCase();
  const hits = needle.length >= 2
    ? sections.flatMap((s) => s.tiers.flatMap((t) => t.bases.filter((b) => b.toLowerCase().includes(needle)).map((b) => ({ s, t, b })))).slice(0, 12)
    : [];

  const section = sections.find((s) => s.type_path.join(">") === sel);
  const done = async (p: Promise<unknown>, m: string) => {
    try { await p; setMsg(`${m} Press F4 in game.`); onChange(); await load(); } catch (e) { setMsg(String(e)); }
  };
  const setTier = (t: TierRow, patch: { show?: boolean; style?: CustomStyle; reset?: boolean }) =>
    done(invoke("set_tier", { id: profile.id, typePath: section!.type_path, tier: t.tier, show: patch.show ?? null,
      style: patch.style ?? null, reset: !!patch.reset }),
      patch.reset ? `${t.name} is back to NeverSink's.` : patch.style ? `${t.name} restyled.` : `${t.name} ${patch.show ? "shown" : "hidden"}.`);
  const drop = (t: TierRow) => {
    if (!drag || drag.from === t.tier) return;
    done(invoke("move_tier_item", { id: profile.id, typePath: section!.type_path, base: drag.base, toTier: t.tier }),
      `${drag.base} moved to ${t.name}.`);
    setDrag(null); setOver(null);
  };

  return (
    <div className="tiers">
      <div className="tiers-nav">
        <input className="text" placeholder="Find an item…" value={q} onChange={(e) => setQ(e.target.value)} />
        {hits.length > 0 && (
          <ul className="results">
            {hits.map(({ s, t, b }) => (
              <li key={s.title + t.tier + b} onClick={() => { setSel(s.type_path.join(">")); setQ(""); }}>
                <span className="with-icon"><ItemIcon base={b} size={20} />{b}</span>
                <span className="muted small">{s.title} · {t.name}</span>
              </li>
            ))}
          </ul>
        )}
        {groups.map(([g, list]) => (
          <div key={g} className="tiers-group">
            <div className="tiers-gname">{g}</div>
            {list.map((s) => {
              const k = s.type_path.join(">");
              return <button key={k} className={`tiers-sec ${sel === k ? "on" : ""}`} onClick={() => { setSel(k); setEditing(null); }}>{s.title}</button>;
            })}
          </div>
        ))}
      </div>

      <div className="tiers-main">
        {!section ? <div className="empty">No tier lists found in this filter.</div> : (
          <div className="card">
            <h2>{section.title}</h2>
            <p className="muted small">Untick a tier to hide it. Click a tier's label to change its look.
              Drag an item onto another tier to move it there.</p>
            {section.tiers.map((t) => {
              const key = t.tier;
              const shown = t.visibility !== "Hide";
              const open = expanded[key];
              const items = open ? t.bases : t.bases.slice(0, SHOW_ITEMS);
              return (
                <div key={key} className={`tier ${over === key ? "over" : ""} ${shown ? "" : "off"}`}
                  onDragOver={(e) => { if (drag && drag.from !== key) { e.preventDefault(); setOver(key); } }}
                  onDragLeave={() => setOver((o) => (o === key ? null : o))}
                  onDrop={(e) => { e.preventDefault(); drop(t); }}>
                  <div className="tier-head">
                    <label className="tier-name">
                      <input type="checkbox" checked={shown} onChange={(e) => setTier(t, { show: e.target.checked })} />
                      <span>{t.name}</span>
                      {t.note && <span className="muted small">{t.note}</span>}
                    </label>
                    <button className="tier-look" title="Change this tier's look" onClick={() => setEditing(editing === key ? null : key)}>
                      <Preview s={fromCurrent(t.style, "Currency")} text={t.bases[0] ?? t.name} />
                    </button>
                    <button className="link small" title="Undo your changes to this tier" onClick={() => setTier(t, { reset: true })}>Reset</button>
                  </div>
                  {editing === key && (
                    <div className="qm tier-edit">
                      <StyleEditor start={fromCurrent(t.style, "Currency")} label={t.bases[0] ?? t.name}
                        onCancel={() => setEditing(null)}
                        onSave={(s) => { setEditing(null); setTier(t, { style: s }); }} />
                    </div>
                  )}
                  <div className="tier-items">
                    {items.map((b) => (
                      <span key={b} className={`chip ${drag?.base === b ? "dragging" : ""}`} draggable
                        onDragStart={(e) => { e.dataTransfer.setData("text/plain", b); e.dataTransfer.effectAllowed = "move"; setDrag({ base: b, from: key }); }}
                        onDragEnd={() => { setDrag(null); setOver(null); }}
                        title={`Drag to another tier`}>
                        <ItemIcon base={b} size={20} />{b}
                      </span>
                    ))}
                    {t.bases.length > SHOW_ITEMS && (
                      <button className="link small" onClick={() => setExpanded({ ...expanded, [key]: !open })}>
                        {open ? "Show fewer" : `+${t.bases.length - SHOW_ITEMS} more`}
                      </button>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
        {msg && <div className="note">{msg}</div>}
      </div>
    </div>
  );
}
