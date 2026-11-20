import { getCurrentWindow } from "@tauri-apps/api/window";
import type { ReactNode } from "react";

/**
 * The title bar every window over the game shares: drag it to move the window, ✕ hides it.
 * `children` sit between the hint and the ✕ (e.g. a tick box or "Clear all").
 * Where it was and how big it was are remembered by the app (win_place.rs).
 */
export function WinBar({ title, hint, onClose, closeTip = "Hide", compact, children }: {
  title: ReactNode; hint?: string; onClose: () => void; closeTip?: string; compact?: boolean; children?: ReactNode;
}) {
  return (
    <div className={`bo-bar win-bar ${compact ? "compact" : ""}`}
      onMouseDown={(e) => {
        if (e.button === 0 && (e.target as HTMLElement).closest("button, select, label, input, a") == null) getCurrentWindow().startDragging();
      }}>
      <span className="bo-title">{title}</span>
      {hint && <span className="muted small win-hint">{hint}</span>}
      <span className="win-extra">{children}</span>
      <button className="link bo-close" title={closeTip} onClick={onClose}>✕</button>
    </div>
  );
}

type Dir = "East" | "South" | "West" | "SouthEast" | "SouthWest";

/**
 * Resize handles for a window without a frame: easy-to-grab left, right and bottom edges, and a
 * visible grip in the bottom-right corner that resizes both ways at once.
 * Put it inside the window's outer box (which needs `position: relative`).
 */
export function ResizeGrips() {
  const start = (dir: Dir) => (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    getCurrentWindow().startResizeDragging(dir).catch(() => {});
  };
  return (
    <>
      <div className="rg rg-e" onMouseDown={start("East")} />
      <div className="rg rg-w" onMouseDown={start("West")} />
      <div className="rg rg-s" onMouseDown={start("South")} />
      <div className="rg rg-sw" onMouseDown={start("SouthWest")} />
      <div className="rg rg-se" onMouseDown={start("SouthEast")} title="Drag to resize">
        <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
          <path d="M15 5 5 15M15 9.5 9.5 15M15 13.5 13.5 15" />
        </svg>
      </div>
    </>
  );
}
