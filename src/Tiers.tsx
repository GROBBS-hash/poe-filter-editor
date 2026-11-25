import WorthLine, { SectionWorth, worthText } from "./WorthLine";
import { invoke } from "@tauri-apps/api/core";
import { Tip } from "./Help";
import { useEffect, useMemo, useState } from "react";
import { ProfileInfo } from "./types";
import StyleEditor, { CurrentStyle, CustomStyle, LookButton, Preview, fromCurrent } from "./StyleEditor";
import { ItemIcon, useIcons } from "./icons";
import MapTiers, { MapSection } from "./MapTiers";
import Threads from "./Threads";
import StepConds, { Cond, Menu } from "./StepConds";
import { JewelMod, JewelRule, jewelModList, readJewelRule, saveJewelRule } from "./jewelMods";

/** Which "+ Add a condition" choices a section gets. */
function menuFor(s: { group: string; title: string }): Menu {
  switch (s.group) {
    case "Jewels": return s.title.startsWith("Cluster") ? "cluster" : "jewels";
    case "Gear": return "gear";
    case "Flasks and gems": return /gem/i.test(s.title) ? "gems" : "flasks";
    case "Currency": case "League currency": case "Fragments": case "Divination cards": return "currency";
    case "Uniques": return "uniques";
    case "Before maps": return "levelling";
    case "Heist": case "League items": return "league";
    default: return "any";
  }
}

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
  "Rare gear by item tier": { item: "Hubris Circlet" }, "6-linked items": { item: "Orb of Fusing" },
  "Influenced gear": { item: "Crusader's Exalted Orb" }, "Searing Exarch implicits": { item: "Exceptional Eldritch Ember" },
  "Eater of Worlds implicits": { item: "Exceptional Eldritch Ichor" }, "Special mods (veiled, Incursion…)": { item: "Veiled Exalted Orb" },
  "High-quality bases": { item: "Blacksmith's Whetstone" }, "Crafting bases": { item: "Exalted Orb" }, "Corrupted gear": { item: "Vaal Orb" },
  "Enchanted and anointed": { item: "Golden Oil" }, "Sockets and links": { item: "Jeweller's Orb" }, "Rare gear by base": { item: "Regal Orb" },
  "Rare rings and amulets": { item: "Amethyst Ring" }, "Rare belts": { item: "Stygian Vise" },
  "Identified rares with good mods": { item: "Scroll of Wisdom" }, "Identified rares by base": { item: "Scroll of Wisdom" },
  "Identified corrupted gear": { item: "Vaal Orb" }, "Identified magic items with good mods": { item: "Orb of Alteration" },
  "Memory strand gear": { item: "Hubris Circlet" }, "Rare gear by size": { item: "Regal Orb" }, "Crucible gear": { item: "Orb of Alchemy" },
  "Abyss socket gear": { item: "Stygian Vise" }, "Unique maps": { item: "Map (Tier 16)" }, "Timeless splinters": { item: "Timeless Karui Splinter" },
  "Ducats": { item: "Brinehook's Ducat" }, "Wombgifts": { item: "Ancient Wombgift" }, "Allflame embers": { item: "Allflame Ember of Kulemak" },
  "Other currency": { item: "Scrying Orb" }, "League-only currency": { item: "Silver Coin" },
  "Flasks": { url: CDN + "Flasks/quicksilver.png" }, "Tinctures": { item: "Prismatic Tincture" },
  "Exceptional gems": { item: "Awakened Empower Support" }, "Quality and level gems": { item: "Gemcutter's Prism" }, "Other gems": { item: "Vaal Orb" },
  "Contracts": { item: "Contract: Bunker" }, "Blueprints": { item: "Blueprint: Bunker" }, "Cloaks": { item: "Whisper-woven Cloak" },
  "Brooches": { item: "Foliate Brooch" }, "Heist gear": { item: "Precise Arrowhead" }, "Tools": { item: "Master Lockpick" },
  "Rogue's Markers": { item: "Rogue's Marker" }, "Idols": { item: "Kamasan Idol" }, "Expedition logbooks": { item: "Expedition Logbook" },
  "Voidstones, trinkets and other": { item: "Ceremonial Voidstone" }, "Sanctum relics": { item: "Censer Relic" }, "Sanctum research": { item: "Forbidden Tome" },
  "Reliquary keys and league items": { item: "Ancient Reliquary Key" }, "Invitations and Labyrinth items": { item: "Maven's Invitation: The Feared" },
  "3-linked items": { item: "Orb of Fusing" }, "Rare bows": { item: "Imperial Bow" }, "Rare one-hand weapons": { item: "Jewelled Foil" },
  "Rare two-hand weapons": { item: "Reaver Axe" }, "Rare caster weapons": { item: "Convoking Wand" }, "Rare armour": { item: "Astral Plate" },
  "Rare jewellery": { item: "Two-Stone Ring" },
  "Act 2 gear": { item: "Rustic Sash" }, "Later acts gear": { item: "Leather Belt" }, "Minion gear": { item: "Convoking Wand" },
  "Quality flasks": { item: "Glassblower's Bauble" }, "Tinctures while levelling": { item: "Prismatic Tincture" },
  "Movement boots and veiled": { item: "Two-Toned Boots" }, "Other rares": { item: "Regal Orb" }, "Rares by size": { item: "Regal Orb" },
  "Magic items by size": { item: "Orb of Alteration" }, "Animate Weapon bases": { item: "Driftwood Club" },
  "Splinter stacks (high)": { item: "Splinter of Chayula" }, "Splinter stacks (low)": { item: "Splinter of Xoph" },
  "Sulphur": { item: "Sulphur Flask" }, "Corpses": { item: "Perfect Fiend" }, "Viscera": { item: "Ichor-Coated Viscera" },
  "Heist targets": { item: "Rogue's Marker" }, "Unique contracts and blueprints": { item: "Blueprint: Bunker" }, "Charts": { item: "Coral Reef Chart" },
  "Enshrouded gear": { item: "Karui Enshrouding Crystal" }, "Exotic bases": { item: "Simplex Amulet" }, "Rare minion gear": { item: "Bone Ring" }, "Rare 4-links": { item: "Orb of Fusing" }, "Fractured bases": { item: "Fractured Fossil" }, "Veiled items": { item: "Veiled Chaos Orb" },
  "Jewels": { item: "Cobalt Jewel" }, "Abyss jewels": { item: "Searching Eye Jewel" }, "Cluster jewels": { item: "Large Cluster Jewel" },
  "Cluster jewels (cheap)": { item: "Small Cluster Jewel" }, "Special jewels": { item: "Timeless Jewel" }, "Jewels while levelling": { item: "Crimson Jewel" },
};

/** A menu entry's picture (fixed size, so names line up even when there's no picture). */
function SectionIcon({ title, first }: { title: string; first?: string }) {
  const pick = SECTION_ICON[title];
  const [broken, setBroken] = useState(false);
  const icons = useIcons();
  // The chosen item has no picture (league items poe.ninja leaves out): the section's first item instead.
  const base = pick?.item && (icons[pick.item] || /Jewel$/.test(pick.item)) ? pick.item : first ?? pick?.item ?? null;
  return (
    <span className="tiers-ico">
      {pick?.url && !broken ? <img src={pick.url} alt="" width={22} height={22} onError={() => setBroken(true)} />
        : <ItemIcon base={base} size={22} />}
      {pick?.badge && <span className="tiers-ico-badge">{pick.badge}</span>}
    </span>
  );
}

const MAPS_KEY = "__maps";
/** Tiers → Threads (24 Nov): NeverSink has no Threads section, so the page is the app's own. */
const THREADS_KEY = "__threads";

type TierRow = { tier: string; name: string; visibility: string; style: CurrentStyle; bases: string[]; note: string; min_stack: number | null; min_item_tier?: number | null;
  conds?: Cond[]; changed?: string[] };

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
type Section = { type_path: string[]; title: string; group: string; tiers: TierRow[]; shared_bases?: boolean;
  prices?: Record<string, number>; hide_below?: number | null;
  /** Items listed here that your own edits hide, and from what stack size they show anyway (null = never). */
  edit_hides?: Record<string, EditHide> };
type EditHide = { from: number | null; label: string; rule: number | null; parent: string | null };
/** Your edit keeps this row's stacks (some or all of them) hidden. */
const editHides = (s: Section, t: TierRow, b: string) => {
  const h = s.edit_hides?.[b];
  return h && (h.from == null || h.from > Math.max(1, t.min_stack ?? 1)) ? h : null;
};

const SHOW_ITEMS = 24;

/** Like FilterBlade: NeverSink's tier lists. Tick to show/hide a tier, click its label to restyle it,
 *  drag items between tiers to re-rank them. */
export default function Tiers({ profile, onChange, focus }: { profile: ProfileInfo; onChange: () => void;
  focus?: { type_path: string[]; tier: string | null; section?: string | null; kind?: string } | null }) {
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
    // A worth line: its section, with the line glowing.
    if (focus.kind === "price") {
      const to = focus.section === "Threads" ? THREADS_KEY : sections.find((s) => s.title === focus.section)?.type_path.join(">");
      if (!to) return;
      setSel(to); setQ(""); setEditing(null);
      window.setTimeout(() => {
        const line = document.querySelector<HTMLElement>(".worth-line, .thr-line");
        line?.scrollIntoView({ behavior: "smooth", block: "center" });
        line?.classList.add("glow");
        window.setTimeout(() => line?.classList.remove("glow"), 3500);
      }, 500);
      return;
    }
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
      window.setTimeout(() => {
        // The menu shows where you are, then the step (when one was asked for) glows in view.
        document.querySelector(".tiers-sec.on")?.scrollIntoView({ block: "nearest" });
        if (focus.tier) document.querySelector(`[data-tier="${CSS.escape(focus.tier)}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" });
      }, 80);
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
    setSel((cur) => (cur && (cur === extraKey || cur === MAPS_KEY || cur === THREADS_KEY || s.some((x) => x.type_path.join(">") === cur)) ? cur : s[0] ? s[0].type_path.join(">") : ""));
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
      ...("threads".includes(needle) || needle.startsWith("thread") ? [{ key: THREADS_KEY, title: "Threads", group: "Maps" }] : []),
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

  // Prices are in chaos on PoE 1; big ones read better in Divine Orbs.
  const divine = sections.find((s) => s.prices?.["Divine Orb"])?.prices?.["Divine Orb"] ?? null;
  const unit = profile.game === "Poe2" ? "exalted" : "chaos";
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
  const setConds = (t: TierRow, set: Record<string, string>, undo: string[], m: string) =>
    done(invoke("set_tier_conds", { id: profile.id, typePath: section!.type_path, tier: t.tier, set, undo }), `${t.name}: ${m}`);
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
          <button className={"tiers-sec " + (sel === THREADS_KEY ? "on" : "")} onClick={() => { setSel(THREADS_KEY); setEditing(null); }}>
            <span className="tiers-ico"><ItemIcon base="Breach Thread" size={22} /></span>Threads</button>
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
        ) : sel === THREADS_KEY ? (
          <Threads profileId={profile.id} onChange={onChange} onOpenLook={(() => {
            // Their look is NeverSink's Fragments tier 3 (the tier listing the Threads).
            const frag = sections.find((x) => x.tiers.some((t) => t.bases.some((b) => / Thread$/.test(b))));
            if (!frag) return undefined;
            const tier = frag.tiers.find((t) => t.bases.some((b) => / Thread$/.test(b)))?.tier ?? null;
            return () => { setSel(frag.type_path.join(">")); setEditing(null); setFocusTier(tier); window.setTimeout(() => setFocusTier(null), 3500);
              window.setTimeout(() => { if (tier) document.querySelector(`[data-tier="${CSS.escape(tier)}"]`)?.scrollIntoView({ behavior: "smooth", block: "center" }); }, 80); };
          })()} />
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
            {section.group === "Jewels" && section.title === "Jewels" && (
              <>
                <CorruptedJewels onDone={(p, m) => done(p, m)} />
                <JewelModsCard onDone={(p, m) => done(p, m)} />
              </>
            )}
            {section.group === "Gear" && !isExtra && (
              <div className="note">An item takes the <b>first step it fits</b>, from the top. <b>Click a condition</b> (gold edge) to change it,
                e.g. "item level 86+" on crafting bases, or <b>+ Add a condition</b>: item tier, number of mods, corrupted, quality, links…
                Your changes survive NeverSink updates.</div>
            )}
            {section.group === "Jewels" && (
              <div className="note">A jewel takes the <b>first step it fits</b>, from the top. <b>Click a condition</b> (gold edge) to change
                it, e.g. raise “item level 84+” to 86+, or <b>+ Add a condition</b> such as corrupted, a corruption mod, or how many mods.
                Jewels that fit no step stay hidden. Your changes survive NeverSink updates.</div>
            )}
            {section.group === "Before maps" && (
              <div className="note">Rules for the campaign, before maps: each row is one step NeverSink shows while you level
                (most only up to a certain area level). <b>Untick</b> a step to hide it, or change how it looks.</div>
            )}
            <h2>{section.title} <Tip>NeverSink sorts items into tiers, best at the top. <b>Untick</b> a tier to hide everything in it.
              <b> Click the label</b> to change how the whole tier looks. <b>Drag</b> an item onto another tier to move it there
              (e.g. drag Chaos Orb up a tier). <b>+ Add item</b> puts any item in a tier, e.g. add a currency to “Stacks of 6+” to give its stacks that look while single drops keep their normal one. <b>Copy/Paste</b> gives one tier another tier's look. <b>Reset</b> puts a tier back
              to NeverSink's. Your changes survive NeverSink updates and strictness changes.</Tip></h2>
            {!isExtra && section.tiers.some((t) => /^t2(D|$)/.test(t.tier)) && !section.tiers.some((t) => /^t1(D|$)/.test(t.tier)) && (
              <div className="note">No tier 1 here: NeverSink keeps that tier empty at the moment, because nothing in this group is
                worth that much. <b>Tier 2</b> is the best one. To make an item stand out more, restyle Tier 2 or give the item its own look from Find.</div>
            )}
            {!isExtra && (section.title === "Divination cards" || section.title === "Uniques") && (
              <WorthLine profileId={profile.id} group={section.title === "Uniques" ? "Uniques" : "DivinationCards"} onChange={onChange} />
            )}
            {!isExtra && section.group !== "Uniques" && section.title !== "Divination cards" && section.prices && (
              <SectionWorth profileId={profile.id} sectionKey={section.type_path.join("->")} prices={section.prices}
                limit={section.hide_below ?? null} onChange={() => { onChange(); load(); }} />
            )}
            <EditHidesNote section={section} onRemove={(b, h) => done(invoke("delete_rule", { id: profile.id, index: h.rule }), `${b} shows as its tier again.`)} />
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
                      <ItemIcon base={t.bases[0] ?? SECTION_ICON[section.title]?.item ?? null} size={26} />
                      <span>{t.name}</span>
                      {t.note && !t.conds && (t.min_stack == null || isExtra) && <span className="muted small">{isExtra ? `Catches: ${t.note}` : t.note}</span>}
                    </label>
                    {t.min_stack != null && <StackBox value={t.min_stack} onSave={(n) => setTier(t, { minStack: n })} />}
                    {t.min_item_tier != null && <StackBox label="item tier" title="Lowest item tier (shown on the label in 3.30) this row applies to"
                      value={t.min_item_tier} onSave={(n) => setTier(t, { minItemTier: n })} />}
                    <LookButton className="tier-look" onClick={() => setEditing(editing === key ? null : key)}>
                      <Preview s={fromCurrent(t.style, "Currency")} text={t.bases[0] ?? (section.group === "Jewels" ? SECTION_ICON[section.title]?.item : null) ?? t.name} />
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
                  {t.conds && <StepConds conds={t.conds.filter((c) => !(c.key === "StackSize" && t.min_stack != null) && !(c.key === "UnidentifiedItemTier" && t.min_item_tier != null))} changed={t.changed ?? []} menu={menuFor(section)}
                    onSet={(set, undo, m) => setConds(t, set, undo, m)} />}
                  <div className="tier-items">
                    {items.map((b) => (
                      <span key={b} data-chip={`${key}|${b}`}
                        className={`chip ${editHides(section, t, b) ? "edit-hid" : ""} ${drag?.base === b ? "dragging" : ""} ${found?.base === b && found.tier === key ? "found" : ""}`} draggable={!section.shared_bases}
                        onDragStart={(e) => { e.dataTransfer.setData("text/plain", b); e.dataTransfer.effectAllowed = "move"; setDrag({ base: b, from: key }); }}
                        onDragEnd={() => { setDrag(null); setOver(null); }}
                        title={editHides(section, t, b) ? `Your edit “${section.edit_hides![b].label}” hides it` : `Drag to another tier`}>
                        <ItemIcon base={b} size={20} />{b}
                        {section.prices?.[b] != null && <span className={`chip-price ${section.hide_below != null && section.prices[b] < section.hide_below ? "below" : ""}`}
                          title={section.group === "Uniques" ? "poe.ninja: the most a unique on this base is worth" : "poe.ninja price"}>
                          {section.group === "Uniques" ? "up to " : ""}{worthText(section.prices[b], unit, divine)}</span>}
                      </span>
                    ))}
                    {t.bases.length > SHOW_ITEMS && (
                      <button className="link small" onClick={() => setExpanded({ ...expanded, [key]: !open })}>
                        {open ? "Show fewer" : `+${t.bases.length - SHOW_ITEMS} more`}
                      </button>
                    )}
                    {!section.shared_bases && t.bases.length > 0 && <AddItem onAdd={(b) => addTo(t, b)} />}
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

/** Items in this list that your own edits (F2, My edits) hide, whatever their tier says, with a button to undo each. */
function EditHidesNote({ section, onRemove }: { section: Section; onRemove: (base: string, h: EditHide) => void }) {
  const list = [...new Set(section.tiers.flatMap((t) => t.bases.filter((b) => editHides(section, t, b))))];
  if (list.length === 0) return null;
  return (
    <div className="warn-line edit-hides">
      <b>Your own edits hide {list.length === 1 ? "this one" : `these ${list.length}`}, whatever the tier says:</b>
      {list.map((b) => {
        const h = section.edit_hides![b];
        return (
          <div key={b} className="eh-row">
            <ItemIcon base={b} size={24} /><b>{b}</b>
            <span className="small">{h.from == null ? "every stack hidden" : `only stacks of ${h.from}+ show`}
              <span className="muted"> · “{h.label}”{h.parent ? `, an edit in ${h.parent}: change it there` : ""}</span></span>
            {h.rule != null && <button onClick={() => onRemove(b, h)}>Remove this edit</button>}
          </div>
        );
      })}
    </div>
  );
}

/** The rule "every corrupted rare jewel" (Cobalt, Crimson and Viridian): one switch on the Jewels page, since NeverSink
 *  hides most rare jewels but corrupted ones can be worth a lot (21 Nov). */
const CJ_ITEM = { item_class: "Jewels", rarity: "Rare", name: null, base_type: "", item_level: null, stack_size: null, quality: null,
  sockets: null, map_tier: null, corrupted: true, mirrored: false, unidentified: false };
const CJ_OPTS = { same_rarity: true, max_item_level: null, corrupted: true };
// "Show them" makes a highlight; changing its look turns it into a "Style:" edit with the same match.
const CJ_LABELS = ["Highlight: Corrupted Rare every Jewels", "Hide: Corrupted Rare every Jewels", "Style: Corrupted Rare every Jewels"];
function CorruptedJewels({ onDone }: { onDone: (p: Promise<unknown>, msg: string) => Promise<void> }) {
  const [state, setState] = useState<"shown" | "hidden" | "none">("none");
  const [look, setLook] = useState<CurrentStyle | null>(null);
  const [editing, setEditing] = useState(false);
  const check = () => {
    Promise.all(CJ_LABELS.map((label) => invoke<[string, number, { style: CurrentStyle }]>("get_rule", { id: null, index: null, label }).then((r) => r[2].style).catch(() => null)))
      .then(([shown, hidden, styled]) => { setState(shown || styled ? "shown" : hidden ? "hidden" : "none"); setLook(styled ?? shown ?? null); });
  };
  useEffect(check, []);
  const remove = async () => {
    for (const label of CJ_LABELS) {
      try { const [id, index] = await invoke<[string, number]>("get_rule", { id: null, index: null, label }); await invoke("delete_rule", { id, index }); } catch { /* not there */ }
    }
  };
  const restyle = (style: CustomStyle) => { setEditing(false); onDone((async () => {
    await remove();
    await invoke("apply_custom_style", { item: CJ_ITEM, style, opts: CJ_OPTS });
  })().finally(check), "Corrupted rare jewels restyled."); };
  const act = (what: "show" | "hide" | "reset") => onDone((async () => {
    await remove();
    if (what !== "reset") await invoke("apply_quick_action", { item: CJ_ITEM, action: what === "show" ? "highlight" : "hide", colour: what === "show" ? "purple" : null, opts: CJ_OPTS });
  })().finally(check), what === "show" ? "Corrupted rare jewels now show, highlighted." : what === "hide" ? "Corrupted rare jewels hidden." : "Corrupted rare jewels back to NeverSink's.");
  return (
    <div className="note cj-card">
      <span className="with-icon"><ItemIcon base="Cobalt Jewel" size={22} /><b>Corrupted rare jewels</b></span>
      <span className="muted small">Cobalt, Crimson and Viridian. Corrupted ones can be worth a lot.</span>
      <span className="cj-btns">
        <button className={state === "shown" ? "primary" : ""} onClick={() => act("show")}>{state === "shown" ? "✓ Shown" : "Show them"}</button>
        <button className={state === "hidden" ? "primary" : ""} onClick={() => act("hide")}>{state === "hidden" ? "✓ Hidden" : "Hide them"}</button>
        {state !== "none" && <button className="link small" onClick={() => act("reset")}>Back to NeverSink's</button>}
      </span>
      {state === "shown" && look && (
        <span className="cj-look">
          <span className="muted small">How they look (click to change):</span>
          <LookButton className="tier-look" onClick={() => setEditing(!editing)}>
            <Preview s={fromCurrent(look, "Rare")} text="Cobalt Jewel" />
          </LookButton>
        </span>
      )}
      {editing && look && (
        <div className="qm tier-edit cj-edit">
          <StyleEditor start={fromCurrent(look, "Rare")} label="Cobalt Jewel" onCancel={() => setEditing(false)} onSave={restyle} />
        </div>
      )}
    </div>
  );
}

/** "Jewels with my mods" (24 Nov, option C): identified Cobalt / Crimson / Viridian / Prismatic jewels with the picked
 *  mods get highlighted. F2 on a jewel adds to the same list (jewelMods.ts). */
function JewelModsCard({ onDone }: { onDone: (p: Promise<unknown>, msg: string) => Promise<void> }) {
  const [rule, setRule] = useState<JewelRule | null>(null);
  const [all, setAll] = useState<JewelMod[]>([]);
  const [q, setQ] = useState("");
  const [editing, setEditing] = useState(false);
  const check = () => { readJewelRule().then(setRule); };
  useEffect(() => { check(); jewelModList().then(setAll); }, []);
  const mods = rule?.mods ?? [];
  const min = rule?.min ?? 0;
  const keep = rule?.styled && rule.look ? fromCurrent(rule.look, "Rare") : null;
  const textOf = (n: string) => all.find((m) => m.name === n)?.text.replace(/\n/g, " · ") ?? "";
  const save = (next: string[], nextMin: number, msg: string, style?: CustomStyle) =>
    onDone(saveJewelRule(next, nextMin, style ?? null, keep).finally(check), msg);
  const needle = q.trim().toLowerCase();
  const found = needle.length < 2 ? [] : all.filter((m) => !mods.includes(m.name)
    && (m.text.toLowerCase().includes(needle) || m.name.toLowerCase().includes(needle))).slice(0, 12);
  return (
    <div className="note jm-card">
      <span className="with-icon"><ItemIcon base="Crimson Jewel" size={22} /><b>Jewels with my mods</b></span>
      <span className="muted small">Identified jewels with these mods get highlighted. F2 on a jewel in game adds its mods here.</span>
      {mods.length > 0 && (
        <div className="jm-chosen">
          {mods.map((n) => (
            <span key={n} className="jm-chip" title={n}>
              {textOf(n) || n}
              <button className="link small" aria-label={`Remove ${n}`} onClick={() => save(mods.filter((x) => x !== n), min, `Removed ${textOf(n) || n}.`)}>✕</button>
            </span>
          ))}
        </div>
      )}
      {mods.length > 1 && (
        <span className="jm-min">
          <span className="muted small">Highlight a jewel with</span>
          {[1, 2, 3].filter((n) => n <= mods.length).map((n) => (
            <button key={n} className={(min || 1) === n ? "primary" : ""} onClick={() => save(mods, n, n === 1 ? "Any one of the mods is enough." : `Needs ${n} of the mods.`)}>
              {n === 1 ? "any 1" : `${n}+`}
            </button>
          ))}
        </span>
      )}
      <input className="text jm-find" placeholder="Add a mod: energy shield, life, attack speed…" value={q} onChange={(e) => setQ(e.target.value)} />
      {found.length > 0 && (
        <div className="jm-found">
          {found.map((m) => (
            <button key={m.name} className="jm-add" onClick={() => { setQ(""); save([...mods, m.name], min, `Added ${m.text.replace(/\n/g, " · ")}.`); }}>
              <span>+ {m.text.replace(/\n/g, " · ")}</span><span className="muted small">{m.name} · {m.prefix ? "prefix" : "suffix"}</span>
            </button>
          ))}
        </div>
      )}
      {needle.length >= 2 && found.length === 0 && <span className="muted small">No jewel mod matches that.</span>}
      {mods.length > 0 && rule?.look && (
        <span className="cj-look">
          <span className="muted small">How they look (click to change):</span>
          <LookButton className="tier-look" onClick={() => setEditing(!editing)}>
            <Preview s={fromCurrent(rule.look, "Rare")} text="Cobalt Jewel" />
          </LookButton>
          <button className="link small" onClick={() => save([], 0, "Jewels with my mods removed: jewels back to NeverSink's.")}>Remove all</button>
        </span>
      )}
      {editing && rule?.look && (
        <div className="qm tier-edit cj-edit">
          <StyleEditor start={fromCurrent(rule.look, "Rare")} label="Cobalt Jewel" onCancel={() => setEditing(false)}
            onSave={(st) => { setEditing(false); save(mods, min, "Jewels with my mods restyled.", st); }} />
        </div>
      )}
    </div>
  );
}
