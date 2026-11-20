import { invoke } from "@tauri-apps/api/core";
import { Tip } from "./Help";
import { useEffect, useMemo, useState } from "react";
import { ProfileInfo } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, LookButton, Preview, fromCurrent } from "./StyleEditor";
import { ItemIcon } from "./icons";
import MapTiers, { MapSection } from "./MapTiers";

const CDN = "https://web.poecdn.com/image/Art/2DItems/";
/** The picture next to each section in the menu: an item from the game ("×3" etc. for stacks). Sections not listed
 *  here use their first item; ones with no items and no picture get none. */
const SECTION_ICON: Record<string, { item?: string; url?: string; badge?: string }> = {
  "Map tiers": { item: "Map (Tier 16)" },
  "Currency": { item: "Chaos Orb" }, "Stacks of 3+": { item: "Chaos Orb", badge: "×3" }, "Stacks of 6+": { item: "Chaos Orb", badge: "×6" },
  "Wisdom scroll stacks": { item: "Scroll of Wisdom" }, "Portal scroll stacks": { item: "Portal Scroll" },
  "Gold": { url: CDN + "Currency/Ruthless/CoinPileTier3.png" },
  "Campaign currency": { item: "Orb of Transmutation" }, "Campaign currency stacks": { item: "Orb of Transmutation", badge: "×3" },
  "Life flasks": { url: CDN + "Flasks/lifeflask11.png" }, "Mana flasks": { url: CDN + "Flasks/manaflask11.png" }, "Hybrid flasks": { url: CDN + "Flasks/hybridflask1.png" },
  "Utility flasks": { url: CDN + "Flasks/sprint.png" }, "Wand upgrades": { item: "Kinetic Wand" }, "Weapon upgrades": { item: "Graceful Sword" },
  "First areas": { item: "Astral Plate" }, "Act 1 gear": { item: "Iron Ring" }, "R-G-B linked items": { item: "Chromatic Orb" },
  "4-linked items": { item: "Orb of Fusing" }, "Other magic items": { item: "Orb of Alteration" },
  "Divination cards": { url: CDN + "Divination/InventoryIcon.png" },
  "Uniques": { item: "Headhunter" }, "Replica uniques": { item: "Replica Headhunter" }, "Foulborn uniques": { item: "Foulborn Exalted Orb" },
  "Fragments": { item: "Fragment of the Phoenix" }, "Simulacrum splinters": { item: "Simulacrum Splinter" },
  "Essences": { item: "Deafening Essence of Greed" }, "Fossils": { item: "Dense Fossil" }, "Oils": { item: "Golden Oil" },
  "Delirium orbs": { item: "Fine Delirium Orb" }, "Omens": { item: "Omen of Amelioration" }, "Tattoos": { item: "Tattoo of the Arohongui Warrior" },
  "Lifeforce": { item: "Vivid Crystallised Lifeforce" }, "Runegrafts": { item: "Runegraft of Blasphemy" },
  "Rare gear by item tier": { item: "Hubris Circlet" }, "Fractured bases": { item: "Fractured Fossil" }, "Veiled items": { item: "Veiled Chaos Orb" },
};

/** A menu entry's picture (fixed size, so names line up even when there's no picture). */
function SectionIcon({ title, first }: { title: string; first?: string }) {
  const pick = SECTION_ICON[title];
  const [broken, setBroken] = useState(false);
  return (
    <span className="tiers-ico">
      {pick?.url && !broken ? <img src={pick.url} alt="" width={22} height={22} onError={() => setBroken(true)} />
        : <ItemIcon base={pick?.item ?? first ?? null} size={22} />}
      {pick?.badge && <span className="tiers-ico-badge">{pick.badge}</span>}
    </span>
  );
}

const MAPS_KEY = "__maps";

type TierRow = { tier: string; name: string; visibility: string; style: CurrentStyle; bases: string[]; note: string; min_stack: number | null; min_item_tier?: number | null };

/** "+ Add item": search any item and put it in this tier. */
function AddItem({ onAdd }: { onAdd: (base: string) => void }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [res, setRes] = useState<{ base_type: string; item_class: string }[]>([]);
  useEffect(() => {
    if (q.trim().length < 2) { setRes([]); return; }
    const id = window.setTimeout(() => invoke<{ base_type: string; item_class: string }[]>("search_items", { q })
      .then((r) => setRes(r.filter((x) => x.base_type).slice(0, 8))).catch(() => setRes([])), 150);
    return () => window.clearTimeout(id);
  }, [q]);
  if (!open) return <button className="link small add-item" onClick={() => setOpen(true)}>+ Add item</button>;
  return (
    <span className="add-box">
      <input autoFocus placeholder="Item name…" value={q} onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => e.key === "Escape" && setOpen(false)} />
      <button className="link small" onClick={() => { setOpen(false); setQ(""); }}>Cancel</button>
      {res.length > 0 && (
        <ul className="results add-results">
          {res.map((r) => (
            <li key={r.base_type} onClick={() => { onAdd(r.base_type); setOpen(false); setQ(""); }}>
              <span className="with-icon"><ItemIcon base={r.base_type} size={20} />{r.base_type}</span>
              <span className="muted small">{r.item_class}</span>
            </li>
          ))}
        </ul>
      )}
    </span>
  );
}

/** "Stacks of N+": saves when you leave the box or press Enter. */
function StackBox({ value, onSave, label = "stacks of", title = "Smallest stack this tier applies to" }: { value: number; onSave: (n: number) => void; label?: string; title?: string }) {
  const [v, setV] = useState(String(value));
  useEffect(() => setV(String(value)), [value]);
  const save = () => { const n = parseInt(v, 10); if (n > 0 && n !== value) onSave(n); else setV(String(value)); };
  return (
    <span className="stackbox" title={title}>
      {label} <input type="number" min={1} value={v} onChange={(e) => setV(e.target.value)} onBlur={save}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />+
    </span>
  );
}
type Section = { type_path: string[]; title: string; group: string; tiers: TierRow[]; shared_bases?: boolean };

const SHOW_ITEMS = 24;

/** Like FilterBlade: NeverSink's tier lists. Tick to show/hide a tier, click its label to restyle it,
 *  drag items between tiers to re-rank them. */
export default function Tiers({ profile, onChange, focus }: { profile: ProfileInfo; onChange: () => void;
  focus?: { type_path: string[]; tier: string | null } | null }) {
  const [sections, setSections] = useState<Section[]>([]);
  const [sel, setSel] = useState<string>("");
  const [q, setQ] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [drag, setDrag] = useState<{ base: string; from: string } | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const [msg, setMsg] = useState("");
  const [focusTier, setFocusTier] = useState<string | null>(null);
  const [mapFocus, setMapFocus] = useState<string | null>(null);
  // One of NeverSink's rules that isn't a tier list, opened from "Modify rule".
  const [extra, setExtra] = useState<Section | null>(null);
  const extraKey = extra?.type_path.join(">") ?? "";
  // Opened from the quick menu's "Modify rule": show that section and make the tier glow.
  useEffect(() => {
    if (!focus || sections.length === 0) return;
    // NeverSink's plain map tier rule: the Map tiers panel explains it far better than its raw rows.
    // Special maps (blighted, Valdo…) too: the panel's step 4 row glows.
    if (focus.type_path[0] === "maps") {
      setSel(MAPS_KEY); setQ(""); setEditing(null);
      setMapFocus(focus.type_path.length > 1 ? focus.type_path.join(">") + (focus.tier?.startsWith("zana") ? ">zana" : "") : null);
      window.setTimeout(() => setMapFocus(null), 3500);
      return;
    }
    const k = focus.type_path.join(">");
    const glow = () => {
      setSel(k); setQ(""); setEditing(null); setFocusTier(focus.tier);
      window.setTimeout(() => document.querySelector(`[data-tier="${CSS.escape(focus.tier ?? "")}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" }), 80);
      window.setTimeout(() => setFocusTier(null), 3500);
    };
    if (sections.some((s) => s.type_path.join(">") === k)) { glow(); return; }
    invoke<Section | null>("get_rule_section", { id: profile.id, typePath: focus.type_path }).then((s) => {
      if (s) { setExtra(s); glow(); }
    }).catch(() => {});
  }, [focus, sections]); // eslint-disable-line react-hooks/exhaustive-deps
  const [found, setFound] = useState<{ base: string; tier: string } | null>(null);
  const [copied, setCopied] = useState<{ style: CustomStyle; from: string } | null>(null);

  const load = () => invoke<Section[]>("get_tiers", { id: profile.id }).then((s) => {
    setSections(s);
    setSel((cur) => (cur && (cur === extraKey || cur === MAPS_KEY || s.some((x) => x.type_path.join(">") === cur)) ? cur : s[0] ? s[0].type_path.join(">") : ""));
    if (extra) invoke<Section | null>("get_rule_section", { id: profile.id, typePath: extra.type_path }).then(setExtra).catch(() => {});
  }).catch((e) => setMsg(String(e)));
  useEffect(() => { load(); }, [profile.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const groups = useMemo(() => {
    const g = new Map<string, Section[]>();
    if (extra) g.set(extra.group, [extra]);
    for (const s of sections) g.set(s.group, [...(g.get(s.group) ?? []), s]);
    return [...g.entries()];
  }, [sections, extra]);

  // Searching jumps to whichever section holds the item.
  const needle = q.trim().toLowerCase();
  const hits = needle.length >= 2
    ? sections.flatMap((s) => s.tiers.flatMap((t) => t.bases.filter((b) => b.toLowerCase().includes(needle)).map((b) => ({ s, t, b })))).slice(0, 12)
    : [];
  // Typing "map" (or a section's name) offers that page too, not just items.
  const pageHits = needle.length >= 2
    ? [...("maps map tiers".includes(needle) || needle.startsWith("map") ? [{ key: MAPS_KEY, title: "Map tiers", group: "Maps" }] : []),
      ...sections.filter((s) => s.title.toLowerCase().includes(needle)).map((s) => ({ key: s.type_path.join(">"), title: s.title, group: s.group }))]
    : [];

  useEffect(() => {
    if (!found) return;
    const id = window.setTimeout(() => {
      document.querySelector(`[data-chip="${CSS.escape(found.tier + "|" + found.base)}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 50);
    const off = window.setTimeout(() => setFound(null), 2500);
    return () => { window.clearTimeout(id); window.clearTimeout(off); };
  }, [found, sel]);

  const section = sections.find((s) => s.type_path.join(">") === sel) ?? (extra && extraKey === sel ? extra : undefined);
  const isExtra = !!extra && section === extra;
  const done = async (p: Promise<unknown>, m: string) => {
    try { await p; setMsg(`${m} Press F4 in game.`); onChange(); await load(); } catch (e) { setMsg(String(e)); }
  };
  const setTier = (t: TierRow, patch: { show?: boolean; style?: CustomStyle; reset?: boolean; minStack?: number; minItemTier?: number }) =>
    done(invoke("set_tier", { id: profile.id, typePath: section!.type_path, tier: t.tier, show: patch.show ?? null,
      style: patch.style ?? null, minStack: patch.minStack ?? null, minItemTier: patch.minItemTier ?? null, reset: !!patch.reset }),
      patch.reset ? `${t.name} is back to NeverSink's.` : patch.style ? `${t.name} restyled.`
        : patch.minStack ? `${t.name} now applies to stacks of ${patch.minStack}+.` : `${t.name} ${patch.show ? "shown" : "hidden"}.`);
  const addTo = (t: TierRow, base: string) => {
    // Already listed in an earlier section of this filter? Then that one wins in game; say so.
    const elsewhere = sections.find((x) => x !== section && x.tiers.some((y) => y.bases.includes(base)));
    done(invoke("move_tier_item", { id: profile.id, typePath: section!.type_path, base, toTier: t.tier }),
      `${base} added to ${t.name}.` + (elsewhere ? ` Note: it's also in “${elsewhere.title}”; if that comes first in NeverSink's filter, that one still decides.` : ""));
  };
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
        {(hits.length > 0 || pageHits.length > 0) && (
          <ul className="results">
            {pageHits.map((h) => (
              <li key={"page" + h.key} onClick={() => { setSel(h.key); setQ(""); setEditing(null); }}>
                <span className="with-icon"><b>{h.title}</b></span>
                <span className="muted small">{h.group} · whole list</span>
              </li>
            ))}
            {hits.map(({ s, t, b }) => (
              <li key={s.title + t.tier + b} onClick={() => {
                setSel(s.type_path.join(">")); setQ(""); setEditing(null);
                // Make sure it's not tucked behind "+N more", then scroll to it and make it glow.
                if (t.bases.indexOf(b) >= SHOW_ITEMS) setExpanded((x) => ({ ...x, [t.tier]: true }));
                setFound({ base: b, tier: t.tier });
              }}>
                <span className="with-icon"><ItemIcon base={b} size={20} />{b}</span>
                <span className="muted small">{s.title} · {t.name}</span>
              </li>
            ))}
          </ul>
        )}
        <div className="tiers-group">
          <div className="tiers-gname">Maps</div>
          <button className={"tiers-sec " + (sel === MAPS_KEY ? "on" : "")} onClick={() => { setSel(MAPS_KEY); setEditing(null); }}><SectionIcon title="Map tiers" />Map tiers</button>
        </div>
        {groups.map(([g, list]) => (
          <div key={g} className="tiers-group">
            <div className="tiers-gname">{g}</div>
            {list.map((s) => {
              const k = s.type_path.join(">");
              return <button key={k + (s === extra ? "*" : "")} className={`tiers-sec ${sel === k ? "on" : ""}`} onClick={() => { setSel(k); setEditing(null); }}>
                <SectionIcon title={s.title} first={s.tiers.find((t) => t.bases.length)?.bases[0]} />{s.title}</button>;
            })}
          </div>
        ))}
      </div>

      <div className="tiers-main">
        {sel === MAPS_KEY ? (
          <MapTiers profileId={profile.id} onChange={onChange} focus={mapFocus}
            onOpen={(s: MapSection) => { setExtra({ ...s, group: "Opened rule" }); setSel(s.type_path.join(">")); setEditing(null); }} />
        ) : !section ? <div className="empty">No tier lists found in this filter.</div> : (
          <div className="card">
            {isExtra && (
              <div className="note">This is one of NeverSink's own rules (not a tier list). <b>Untick</b> a row to hide what it
                catches, <b>click its look</b> to restyle it, <b>Reset</b> to undo your change. Your change survives NeverSink updates.</div>
            )}
            {section.shared_bases && !isExtra && section.group !== "Before maps" && (
              <div className="note">Each base sits in several rows here (one per item tier), so dragging is off: use the
                item tier boxes, ticks and looks instead. Top bases are the most valuable to pick up.</div>
            )}
            {section.group === "Before maps" && (
              <div className="note">Rules for the campaign, before maps: each row is one step NeverSink shows while you level
                (most only up to a certain area level). <b>Untick</b> a step to hide it, or change how it looks.</div>
            )}
            <h2>{section.title} <Tip>NeverSink sorts items into tiers, best at the top. <b>Untick</b> a tier to hide everything in it.
              <b> Click the label</b> to change how the whole tier looks. <b>Drag</b> an item onto another tier to move it there
              (e.g. drag Chaos Orb up a tier). <b>+ Add item</b> puts any item in a tier, e.g. add a currency to “Stacks of 6+” to give its stacks that look while single drops keep their normal one. <b>Copy/Paste</b> gives one tier another tier's look. <b>Reset</b> puts a tier back
              to NeverSink's. Your changes survive NeverSink updates and strictness changes.</Tip></h2>
            {section.tiers.map((t) => {
              const key = t.tier;
              const shown = t.visibility !== "Hide";
              const open = expanded[key];
              const items = open ? t.bases : t.bases.slice(0, SHOW_ITEMS);
              return (
                <div key={key} data-tier={key} className={`tier ${over === key ? "over" : ""} ${shown ? "" : "off"} ${focusTier === key ? "focus" : ""}`}
                  onDragOver={(e) => { if (drag && drag.from !== key && !section.shared_bases) { e.preventDefault(); setOver(key); } }}
                  onDragLeave={() => setOver((o) => (o === key ? null : o))}
                  onDrop={(e) => { e.preventDefault(); drop(t); }}>
                  <div className="tier-head">
                    <label className="tier-name">
                      <input type="checkbox" checked={shown} onChange={(e) => setTier(t, { show: e.target.checked })} />
                      <span>{t.name}</span>
                      {t.note && (t.min_stack == null || isExtra) && <span className="muted small">{isExtra ? `Catches: ${t.note}` : t.note}</span>}
                    </label>
                    {t.min_stack != null && <StackBox value={t.min_stack} onSave={(n) => setTier(t, { minStack: n })} />}
                    {t.min_item_tier != null && <StackBox label="item tier" title="Lowest item tier (shown on the label in 3.30) this row applies to"
                      value={t.min_item_tier} onSave={(n) => setTier(t, { minItemTier: n })} />}
                    <LookButton className="tier-look" onClick={() => setEditing(editing === key ? null : key)}>
                      <Preview s={fromCurrent(t.style, "Currency")} text={t.bases[0] ?? t.name} />
                    </LookButton>
                    <span className="tier-tools">
                      <button className="link small" title="Copy this tier's look"
                        onClick={() => { setCopied({ style: fromCurrent(t.style, "Currency"), from: t.name }); setMsg(`Copied the look of ${t.name}. Use Paste on another tier.`); }}>Copy</button>
                      {copied && copied.from !== t.name && (
                        <button className="link small" title={`Give this tier the look of ${copied.from}`} onClick={() => setTier(t, { style: copied.style })}>Paste</button>
                      )}
                      <button className="link small" title="Undo your changes to this tier" onClick={() => setTier(t, { reset: true })}>Reset</button>
                    </span>
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
                      <span key={b} data-chip={`${key}|${b}`}
                        className={`chip ${drag?.base === b ? "dragging" : ""} ${found?.base === b && found.tier === key ? "found" : ""}`} draggable={!section.shared_bases}
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
                    {!section.shared_bases && <AddItem onAdd={(b) => addTo(t, b)} />}
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
