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
        <svg viewBox="0 0 24 24" width="16" height="16"><path d="M12 2.6l6.6 4.6-1.6 10.9L12 21.4l-5-3.3-1.6-10.9z" fill="currentColor" /><path d="M12 5.6l1.4 6.2L12 18l-1.4-6.2z" fill="#16110a" /></svg>
      </span>
      <span className="mt-title">PoE Lodestone</span>
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
const SCALES = [1, 1.2, 1.4, 1.6, 1.8, 2];

/**
 * Bigger / smaller for a window over the game (23 Nov: "can we have an option to enlarge" the map tracker): scales the
 * text and pictures and grows the window to match, from its normal size `base` (logical pixels). Remembered on this PC.
 */
export function useWinScale(key: string, base: [number, number]) {
  const [scale, setScale] = useState(() => {
    try { const v = parseFloat(localStorage.getItem(`scale-${key}`) ?? ""); return SCALES.includes(v) ? v : 1; } catch { return 1; }
  });
  useEffect(() => {
    document.documentElement.style.setProperty("zoom", String(scale));
    import("@tauri-apps/api/dpi").then(({ LogicalSize }) =>
      getCurrentWindow().setSize(new LogicalSize(Math.round(base[0] * scale), Math.round(base[1] * scale)))).catch(() => {});
    try { localStorage.setItem(`scale-${key}`, String(scale)); } catch { /* not kept */ }
  }, [scale]); // eslint-disable-line react-hooks/exhaustive-deps
  const i = SCALES.indexOf(scale);
  return { scale, bigger: () => setScale(SCALES[Math.min(SCALES.length - 1, i + 1)]), smaller: () => setScale(SCALES[Math.max(0, i - 1)]),
    canBigger: i < SCALES.length - 1, canSmaller: i > 0 };
}

/** The A− / A+ buttons for a window's title bar. */
export function SizeButtons({ s }: { s: ReturnType<typeof useWinScale> }) {
  return (
    <span className="win-size" title={`Size ${Math.round(s.scale * 100)}%`}>
      <button className="link" onClick={s.smaller} disabled={!s.canSmaller} aria-label="Smaller">A−</button>
      <button className="link" onClick={s.bigger} disabled={!s.canBigger} aria-label="Bigger">A+</button>
    </span>
  );
}
