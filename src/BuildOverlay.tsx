import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import BuildView from "./BuildView";

/** The F6 window over the game: drag the top bar to move it, Esc or F6 to hide.
 *  Can also open by itself when the game's passive tree key is pressed. */
export default function BuildOverlay() {
  const [tree, setTree] = useState<[boolean, string]>([false, "P"]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") invoke("hide_build_window"); };
    window.addEventListener("keydown", onKey);
    const get = () => invoke<[boolean, string]>("build_tree_opt").then(setTree).catch(() => {});
    get();
    const a = listen("build-tree-opt", get);
    const b = listen("build-shown", get);
    return () => { window.removeEventListener("keydown", onKey); a.then((f) => f()); b.then((f) => f()); };
  }, []);
  const setOn = (on: boolean) => { setTree([on, tree[1]]); invoke("set_build_tree", { on }).catch(() => {}); };
  return (
    <div className="bo">
      <div className="bo-bar" onMouseDown={(e) => { if (e.button === 0 && (e.target as HTMLElement).closest("button, label") == null) getCurrentWindow().startDragging(); }}>
        <span className="bo-title">Build</span>
        <span className="muted small">drag to move · Esc / F6 to hide</span>
        <label className="bo-tree" title={`Opens this window on the Tree tab when you press ${tree[1]} in game (the passive skill tree). ${tree[1]} or Esc closes it. Change the key in Settings.`}>
          <input type="checkbox" checked={tree[0]} onChange={(e) => setOn(e.target.checked)} />
          Open with the passive tree ({tree[1]})
        </label>
        <button className="link bo-close" title="Hide" onClick={() => invoke("hide_build_window")}>✕</button>
      </div>
      <div className="bo-body"><BuildView compact /></div>
    </div>
  );
}
