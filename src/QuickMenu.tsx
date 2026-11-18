import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Action, COLOURS, Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";

type Loc = { type_path: string[]; tier?: string | null; visibility: string };

export default function QuickMenu() {
  const [item, setItem] = useState<Item | null>(null);
  const [where, setWhere] = useState("");
  const [toast, setToast] = useState("");
  const [anyRarity, setAnyRarity] = useState(false);

  useEffect(() => {
    const un = listen<Item>("item-picked", async (e) => {
      setItem(e.payload);
      setToast("");
      setAnyRarity(false);
      const locs = await invoke<Loc[]>("locate_item", { item: e.payload }).catch(() => []);
      setWhere(locs.length ? `${locs[0].type_path.join(" › ")} · ${locs[0].tier ?? ""}` : "");
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
      if (e.key.toLowerCase() === "h") document.getElementById("qm-hide")?.click();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", close);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", close);
    };
  }, []);

  const close = async () => {
    setItem(null);
    await getCurrentWindow().hide();
  };

  const act = async (action: Action, colour?: string) => {
    if (!item) return;
    try {
      const r = await invoke<{ label: string; warnings: string[] }>("apply_quick_action", {
        item, action, colour, opts: { same_rarity: !anyRarity, max_item_level: null },
      });
      setToast(r.warnings[0] ?? `${r.label} — type /itemfilter to reload`);
    } catch (e) {
      setToast(String(e));
      return;
    }
    setTimeout(close, 1400);
  };

  const undo = async () => {
    const r = await invoke<string | null>("undo_last").catch((e) => String(e));
    setToast(r ? `Undone: ${r}` : "Nothing to undo");
  };

  if (!item) return null;
  const rarityCanMatter = ["Normal", "Magic", "Rare", "Unique"].includes(item.rarity);

  return (
    <div className="qm">
      <div className="qm-head">
        <div className="qm-name" style={{ color: RARITY_COLOUR[item.rarity] ?? "#ddd" }}>{itemTitle(item)}</div>
        <div className="qm-sub">{itemSub(item)}</div>
        {where && <div className="qm-where">NeverSink: {where}</div>}
      </div>

      <div className="qm-row">
        <button id="qm-hide" className="qm-btn hide" onClick={() => act("hide")}>Hide</button>
        <button className="qm-btn" onClick={() => act("louder")}>Louder</button>
        <button className="qm-btn" onClick={() => act("quieter")}>Quieter</button>
      </div>

      <div className="qm-label">Highlight</div>
      <div className="qm-row">
        {COLOURS.map((c) => (
          <button key={c.name} className="qm-swatch" title={c.name} style={{ background: c.hex }}
            onClick={() => act("highlight", c.name)} />
        ))}
      </div>

      {rarityCanMatter && (
        <label className="qm-check">
          <input type="checkbox" checked={anyRarity} onChange={(e) => setAnyRarity(e.target.checked)} />
          Apply to every rarity, not just {item.rarity}
        </label>
      )}

      <div className="qm-foot">
        <button className="qm-link" onClick={undo}>Undo last</button>
        <button className="qm-link" onClick={close}>Close (Esc)</button>
      </div>
      {toast && <div className="qm-toast">{toast}</div>}
    </div>
  );
}
