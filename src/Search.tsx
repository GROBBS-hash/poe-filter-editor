import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FAQ } from "./Help";
import { ItemIcon } from "./icons";

/** Words players type that NeverSink's names don't use ("quality gems" -> the gem sections). */
const SYNONYMS: Record<string, string> = { qual: "quality", gem: "gems", "6l": "6-linked", "6link": "6-linked", "5l": "5-links", links: "linked",
  clusters: "cluster", abyss: "abyss", ilvl: "item level", corrupt: "corrupted", div: "divination", cards: "divination" };

export type Page = { id: string; name: string; hint: string };
type Found = { base_type: string; item_class: string };
/** A Tiers page section or one of its steps (searched by name, group and conditions). */
export type TierHit = { type_path: string[]; tier: string | null; title: string; sub: string; text: string; icon: string | null };
type TierSection = { type_path: string[]; title: string; group: string; tiers: { tier: string; name: string; bases: string[]; conds?: { words: string }[] }[] };
type Hit =
  | { kind: "tier"; hit: TierHit }
  | { kind: "page"; page: Page }
  | { kind: "help"; q: string; index: number }
  | { kind: "item"; item: Found };

const norm = (s: string) => s.toLowerCase();
/** Every word typed appears somewhere in the text. */
const matches = (text: string, q: string) => { const t = norm(text); return norm(q).split(/\s+/).filter(Boolean).every((w) => t.includes(w)); };

/** The Tiers page's sections and steps for searching (loaded once per filter). */
export function useTierHits(profileId?: string): TierHit[] {
  const [tiers, setTiers] = useState<TierHit[]>([]);
  useEffect(() => {
    if (!profileId) return;
    invoke<TierSection[]>("get_tiers", { id: profileId }).then((secs) => setTiers(secs.flatMap((s) => [
      { type_path: s.type_path, tier: null, title: s.title, sub: `Tiers · ${s.group}`, text: `${s.title} ${s.group}`, icon: s.tiers.find((t) => t.bases.length)?.bases[0] ?? null },
      ...s.tiers.map((t) => ({ type_path: s.type_path, tier: t.tier, title: t.name, sub: `Tiers · ${s.title}`,
        text: `${t.name} ${s.title} ${(t.conds ?? []).map((c) => c.words).join(" ")}`, icon: t.bases[0] ?? null })),
    ]))).catch(() => setTiers([]));
  }, [profileId]);
  return tiers;
}

/** Tiers sections first, then steps; typed words also match their usual synonyms ("qual" -> "quality"), plurals too. */
export function matchTiers(tiers: TierHit[], q: string, max = 8): TierHit[] {
  const words = norm(q).split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const fits = (text: string) => { const t = norm(text); return words.every((w) => t.includes(w) || t.includes(w.replace(/s$/, "")) || (SYNONYMS[w] && t.includes(SYNONYMS[w]))); };
  return tiers.filter((t) => fits(t.text)).sort((a, b) => (a.tier ? 1 : 0) - (b.tier ? 1 : 0)).slice(0, max);
}

/** Ctrl+K: one box that finds items, pages and help answers. Enter opens the highlighted one. */
export default function Search({ pages, onClose, goPage, goHelp, goItem, goTier, profileId }: {
  pages: Page[]; onClose: () => void; goPage: (id: string) => void; goHelp: (index: number) => void; goItem: (f: Found) => void;
  goTier?: (h: TierHit) => void; profileId?: string;
}) {
  const tiers = useTierHits(goTier ? profileId : undefined);
  const [q, setQ] = useState("");
  const [items, setItems] = useState<Found[]>([]);
  const [sel, setSel] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);

  useEffect(() => { input.current?.focus(); }, []);
  useEffect(() => {
    if (q.trim().length < 2) { setItems([]); return; }
    const t = setTimeout(() => invoke<Found[]>("search_items", { q }).then((r) => setItems(r.slice(0, 8))).catch(() => setItems([])), 120);
    return () => clearTimeout(t);
  }, [q]);

  const hits: Hit[] = useMemo(() => {
    const s = q.trim();
    const pageHits = pages.filter((p) => !s || matches(`${p.name} ${p.hint}`, s)).map((page) => ({ kind: "page" as const, page }));
    if (!s) return pageHits;
    const helpHits = FAQ.map((f, index) => ({ kind: "help" as const, q: f.q, index })).filter((h) => matches(h.q, s)).slice(0, 4);
    const itemHits = items.map((item) => ({ kind: "item" as const, item }));
    const tierHits = matchTiers(tiers, s).map((hit) => ({ kind: "tier" as const, hit }));
    return [...tierHits, ...itemHits, ...pageHits.slice(0, 5), ...helpHits];
  }, [q, items, pages, tiers]);

  useEffect(() => { setSel(0); }, [q, items.length]);
  useEffect(() => { list.current?.querySelector<HTMLElement>(".on")?.scrollIntoView({ block: "nearest" }); }, [sel]);

  const open = (h: Hit) => {
    onClose();
    if (h.kind === "page") goPage(h.page.id);
    else if (h.kind === "help") goHelp(h.index);
    else if (h.kind === "tier") goTier?.(h.hit);
    else goItem(h.item);
  };
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); onClose(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); setSel((i) => Math.min(hits.length - 1, i + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((i) => Math.max(0, i - 1)); }
    else if (e.key === "Enter" && hits[sel]) { e.preventDefault(); open(hits[sel]); }
  };

  let lastKind = "";
  const heading: Record<Hit["kind"], string> = { tier: "Tiers page", item: "Items", page: "Pages", help: "Help" };
  return (
    <div className="srch-back" onMouseDown={onClose}>
      <div className="srch" role="dialog" aria-label="Search" onMouseDown={(e) => e.stopPropagation()} onKeyDown={onKey}>
        <div className="srch-in">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm10 17-5.2-5.2" /></svg>
          <input ref={input} value={q} onChange={(e) => setQ(e.target.value)} placeholder="Type an item, a page or a question" spellCheck={false} />
          <kbd>Esc</kbd>
        </div>
        <ul className="srch-list" ref={list}>
          {hits.length === 0 && <li className="srch-none muted">Nothing found for “{q}”</li>}
          {hits.map((h, i) => {
            const head = h.kind !== lastKind ? <li key={`h-${h.kind}`} className="srch-head">{heading[h.kind]}</li> : null;
            lastKind = h.kind;
            const label = h.kind === "tier" ? h.hit.title : h.kind === "page" ? h.page.name : h.kind === "help" ? h.q : h.item.base_type;
            const sub = h.kind === "tier" ? h.hit.sub : h.kind === "page" ? h.page.hint : h.kind === "help" ? "How do I…?" : `${h.item.item_class} · change how it drops`;
            const icon = h.kind === "tier" ? h.hit.icon : h.kind === "item" ? h.item.base_type : null;
            return [head,
              <li key={`${h.kind}-${i}`} className={i === sel ? "srch-hit on" : "srch-hit"} onMouseEnter={() => setSel(i)} onClick={() => open(h)}>
                <span className="srch-label with-icon">{icon && <ItemIcon base={icon} size={20} />}{label}</span><span className="srch-sub muted small">{sub}</span>
              </li>];
          })}
        </ul>
        <div className="srch-foot muted small"><kbd>↑</kbd><kbd>↓</kbd> choose · <kbd>Enter</kbd> open · <kbd>Esc</kbd> close</div>
      </div>
    </div>
  );
}
