import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect } from "react";
import BuildView from "./BuildView";

/** The F6 window over the game: drag the top bar to move it, Esc or F6 to hide. */
export default function BuildOverlay() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("hide_build_window"); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <div className="bo">
      <div className="bo-bar" onMouseDown={(e) => { if (e.button === 0 && (e.target as HTMLElement).closest("button") == null) getCurrentWindow().startDragging(); }}>
        <span>Build</span>
        <span className="muted small">drag to move · Esc / F6 to hide</span>
        <button className="link" onClick={() => invoke("hide_build_window")}>✕</button>
      </div>
      <div className="bo-body"><BuildView compact /></div>
    </div>
  );
}
