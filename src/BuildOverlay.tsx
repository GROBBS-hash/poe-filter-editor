import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import BuildView from "./BuildView";
import { ResizeGrips, WinBar } from "./WinFrame";

/** The F6 window over the game: drag the top bar to move it, the corner or edges to resize, Esc or F6 to hide.
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
      <WinBar title="Build" hint="drag to move · Esc / F6 to hide" onClose={() => invoke("hide_build_window")}>
        <label className="bo-tree" title={`Opens this window on the Tree tab when you press ${tree[1]} in game (the passive skill tree). ${tree[1]} or Esc closes it. Change the key in Settings.`}>
          <input type="checkbox" checked={tree[0]} onChange={(e) => setOn(e.target.checked)} />
          Open with the passive tree ({tree[1]})
        </label>
      </WinBar>
      <div className="bo-body"><BuildView compact /></div>
      <ResizeGrips />
    </div>
  );
}
