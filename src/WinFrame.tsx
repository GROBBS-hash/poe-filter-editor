import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState, type ReactNode } from "react";

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

/**
 * The editor's own title bar (the window has no Windows frame), in the same gold-trimmed style as the windows over the
 * game: drag it to move, double-click to maximise, and minimise / maximise / close (close hides to the tray, as before).
 */
export function MainTitleBar() {
  const w = getCurrentWindow();
  const [max, setMax] = useState(false);
  useEffect(() => {
    w.isMaximized().then(setMax).catch(() => {});
    const un = w.onResized(() => { w.isMaximized().then(setMax).catch(() => {}); });
    return () => { un.then((f) => f()).catch(() => {}); };
  }, []);
  return (
    <div className="mt-bar" onMouseDown={(e) => {
        if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
        if (e.detail === 2) w.toggleMaximize(); else w.startDragging();
      }}>
      <span className="mt-logo" aria-hidden>
        <svg viewBox="0 0 24 24" width="15" height="15"><path d="M3 4h18l-7 8v6l-4 2v-8z" fill="currentColor" /></svg>
      </span>
      <span className="mt-title">PoE Filter Editor</span>
      <span className="mt-btns">
        <button className="mt-btn" title="Minimise" onClick={() => w.minimize()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M2 6.5h8" /></svg></button>
        <button className="mt-btn" title={max ? "Restore" : "Maximise"} onClick={() => w.toggleMaximize()}>
          {max ? <svg viewBox="0 0 12 12" width="12" height="12"><path d="M3.5 4.5h5v5h-5zM5 4.5V3h4.5v4.5H8.5" /></svg>
            : <svg viewBox="0 0 12 12" width="12" height="12"><path d="M2.5 2.5h7v7h-7z" /></svg>}</button>
        <button className="mt-btn close" title="Close (keeps running in the tray)" onClick={() => w.close()}><svg viewBox="0 0 12 12" width="12" height="12"><path d="M3 3l6 6M9 3l-6 6" /></svg></button>
      </span>
    </div>
  );
}