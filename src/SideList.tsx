import { ReactNode, useState } from "react";

/** One part of a long page: shown on its own when picked in the side list. */
export type Part = { key: string; name: string; icon: string; badge?: ReactNode; hide?: boolean };

/**
 * Long pages are split into parts with a small list down the side, like Settings (28 Nov 2026: GROBBS picked
 * "side list" over tabs after the whole-app check found Atlas ~11 screens and Game windows ~9). The part you
 * were on is remembered per page.
 */
export function useSidePart(page: string, parts: Part[], first?: string) {
  const key = `side.${page}`;
  const ok = (k: string | null) => !!k && parts.some((p) => p.key === k && !p.hide);
  const [part, setPart] = useState(() => {
    try { const v = localStorage.getItem(key); if (ok(v)) return v!; } catch { /* not kept */ }
    return first && ok(first) ? first : parts.find((p) => !p.hide)?.key ?? "";
  });
  const pick = (k: string) => {
    setPart(k);
    try { localStorage.setItem(key, k); } catch { /* not kept */ }
    // Each part starts at its top, not where the last one was scrolled to.
    document.querySelector(".content")?.scrollTo({ top: 0 });
  };
  return [ok(part) ? part : parts.find((p) => !p.hide)?.key ?? "", pick] as const;
}

export default function SideList({ label, parts, part, onPick, children }: {
  label: string; parts: Part[]; part: string; onPick: (k: string) => void; children: ReactNode;
}) {
  return (
    <div className="set-layout side-layout">
      <nav className="set-nav" aria-label={label}>
        {parts.filter((p) => !p.hide).map((p) => (
          <button key={p.key} className={part === p.key ? "on" : ""} aria-current={part === p.key ? "page" : undefined} onClick={() => onPick(p.key)}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden><path d={p.icon} /></svg>
            <span className="side-name">{p.name}</span>
            {p.badge != null && <span className="side-badge">{p.badge}</span>}
          </button>
        ))}
      </nav>
      <div className="set-body">{children}</div>
    </div>
  );
}
