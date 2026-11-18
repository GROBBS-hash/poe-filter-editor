import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Action, COLOURS, Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";

type Entry = { item: Item; action: Action; colour: string };
const key = (i: Item) => `${i.rarity}|${i.base_type}`;

export default function Batch({ on, onChange }: { on: boolean; onChange: () => void }) {
  const [list, setList] = useState<Entry[]>([]);
  const [msg, setMsg] = useState("");

  useEffect(() => {
    const un = listen<Item>("batch-item", (e) =>
      setList((l) => (l.some((x) => key(x.item) === key(e.payload)) ? l : [...l, { item: e.payload, action: "hide", colour: "purple" }])));
    return () => { un.then((f) => f()); };
  }, []);

  const toggle = async () => { await invoke("set_batch_mode", { on: !on }); onChange(); };
  const update = (i: number, patch: Partial<Entry>) => setList((l) => l.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  const setAll = (action: Action) => setList((l) => l.map((x) => ({ ...x, action })));

  const apply = async () => {
    let done = 0;
    for (const e of list) {
      try {
        await invoke("apply_quick_action", { item: e.item, action: e.action, colour: e.colour, opts: null });
        done++;
      } catch (err) { setMsg(String(err)); }
    }
    setList([]);
    setMsg(`Applied ${done} edit${done === 1 ? "" : "s"} — press F4 in game to reload`);
    onChange();
  };

  return (
    <div className="card">
      <div className="row between">
        <h2>Batch mode</h2>
        <button className={on ? "primary" : ""} onClick={toggle}>{on ? "Stop collecting" : "Start collecting"}</button>
      </div>
      <p className="muted">{on
        ? "Collecting: hover items in game and press Ctrl+C on each one. They'll appear here."
        : "Turn this on (or press F3 in game), then Ctrl+C every item you want to change."}</p>

      {list.length > 0 && (
        <>
          <div className="row">
            <span className="muted">Set all to:</span>
            {(["hide", "highlight", "louder", "quieter"] as Action[]).map((a) => (
              <button key={a} onClick={() => setAll(a)}>{a[0].toUpperCase() + a.slice(1)}</button>
            ))}
          </div>
          <ul className="batch">
            {list.map((e, i) => (
              <li key={key(e.item)}>
                <div>
                  <div style={{ color: RARITY_COLOUR[e.item.rarity] ?? "#ddd" }}>{itemTitle(e.item)}</div>
                  <div className="muted small">{itemSub(e.item)}</div>
                </div>
                <div className="row">
                  <select value={e.action} onChange={(ev) => update(i, { action: ev.target.value as Action })}>
                    <option value="hide">Hide</option>
                    <option value="highlight">Highlight</option>
                    <option value="louder">Louder</option>
                    <option value="quieter">Quieter</option>
                  </select>
                  {e.action === "highlight" && COLOURS.map((c) => (
                    <button key={c.name} className={e.colour === c.name ? "dot on" : "dot"} style={{ background: c.hex }}
                      onClick={() => update(i, { colour: c.name })} />
                  ))}
                  <button className="link" onClick={() => setList((l) => l.filter((_, j) => j !== i))}>✕</button>
                </div>
              </li>
            ))}
          </ul>
          <button className="primary wide" onClick={apply}>Apply {list.length} edit{list.length === 1 ? "" : "s"}</button>
        </>
      )}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}
