import { useEffect, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Action, COLOURS, Item, RARITY_COLOUR, itemSub, itemTitle } from "./types";
import { ItemIcon } from "./icons";
import { WinBar } from "./WinFrame";
import { friendly } from "./errors";
import { LookChips, useLooks } from "./Looks";

/** "Leave" = keep it as the filter has it (take it off the list without an edit). */
type Choice = Action | "leave" | "look";
type Status = { visibility: string; reason: string; by_user_edit: boolean };
/** `look`: the saved look picked for it (choice "look"). */
type Entry = { item: Item; choice: Choice; colour: string; look?: string; now?: Status | null };

/** Same as the app's batch_key: one entry per rarity, base and (uniques) name. */
const key = (i: Item) => `${i.rarity}|${i.base_type}|${i.rarity === "Unique" ? i.name ?? "" : ""}`;
const CHOICES: [Choice, string][] = [["hide", "Hide"], ["highlight", "Highlight"], ["louder", "Louder"], ["quieter", "Quieter"], ["leave", "Leave as is"]];
const NOW: Record<string, string> = { Show: "Shown", Hide: "Hidden", Minimal: "Shown small", None: "Shown plainly (no rule)" };

function useBatchKey() {
  const [k, setK] = useState("F3");
  useEffect(() => { invoke<{ key_batch?: string }>("get_settings").then((s) => setK(s?.key_batch || "F3")).catch(() => {}); }, []);
  return k;
}

/** Batch mode: press the key in game, Ctrl+C items, press it again, then pick what happens to each one here. */
export default function Batch({ on, onChange }: { on: boolean; onChange: () => void }) {
  const [list, setList] = useState<Entry[]>([]);
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const bk = useBatchKey();
  const [looks] = useLooks();
  const choices: [Choice, string][] = looks.length ? [...CHOICES.slice(0, 4), ["look", "Saved look"], CHOICES[4]] : CHOICES;

  // What the filter does to an item now: hidden things default to Highlight, shown things to Hide.
  const withStatus = (item: Item): Entry => ({ item, choice: "hide", colour: "purple" });
  const fill = (items: Item[]) => {
    setList((old) => {
      const keep = new Map(old.map((e) => [key(e.item), e]));
      return items.map((i) => keep.get(key(i)) ?? withStatus(i));
    });
    for (const i of items) {
      invoke<{ status: { verdict: Status } }>("item_status", { item: i, fromScreen: false }).then((r) => {
        const now = r.status.verdict;
        setList((l) => l.map((e) => key(e.item) !== key(i) || e.now !== undefined ? e
          : { ...e, now, choice: now.visibility === "Hide" ? "highlight" : "hide" }));
      }).catch(() => setList((l) => l.map((e) => key(e.item) === key(i) && e.now === undefined ? { ...e, now: null } : e)));
    }
  };
  useEffect(() => {
    invoke<Item[]>("batch_list").then(fill).catch(() => {});
    const un = listen<Item>("batch-item", () => { invoke<Item[]>("batch_list").then(fill).catch(() => {}); });
    return () => { un.then((f) => f()); };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const toggle = async () => { await invoke("set_batch_mode", { on: !on }); onChange(); };
  const update = (i: number, patch: Partial<Entry>) => setList((l) => l.map((x, j) => (j === i ? { ...x, ...patch } : x)));
  const setAll = (choice: Choice) => setList((l) => l.map((x) => ({ ...x, choice, look: x.look ?? looks[0]?.name })));
  /** Everything takes one saved look. */
  const allLook = (name: string) => setList((l) => l.map((x) => ({ ...x, choice: "look", look: name })));
  const forget = (keys: string[] | null) => invoke("batch_forget", { keys }).catch(() => {});
  const remove = (e: Entry) => { forget([key(e.item)]); setList((l) => l.filter((x) => x !== e)); };

  const edits = list.filter((e) => e.choice !== "leave");
  const apply = async () => {
    setBusy(true);
    const done: string[] = [];
    const failed: string[] = [];
    for (const e of list) {
      if (e.choice === "leave") { done.push(key(e.item)); continue; }
      try {
        if (e.choice === "look") {
          if (!e.look) throw new Error("Pick a saved look");
          await invoke("apply_custom_style", { item: e.item, style: null, opts: null, look: e.look });
        } else await invoke("apply_quick_action", { item: e.item, action: e.choice, colour: e.colour, opts: null });
        done.push(key(e.item));
      } catch (err) { failed.push(`${itemTitle(e.item)}: ${friendly(err)}`); }
    }
    await forget(done);
    setList((l) => l.filter((e) => !done.includes(key(e.item))));
    const n = edits.length - failed.length;
    setMsg((n > 0 ? `Saved ${n} edit${n === 1 ? "" : "s"}. Press F4 in game to see them.` : "Nothing to save.")
      + (failed.length ? ` Couldn't change: ${failed.join("; ")}` : ""));
    setBusy(false);
    onChange();
  };

  return (
    <div className="card batch-page">
      <div className="row between">
        <h2>Batch mode</h2>
        <button className={on ? "primary" : ""} onClick={toggle}>{on ? "Stop collecting" : "Start collecting"}</button>
      </div>
      {on ? (
        <p className="batch-on"><span className="batch-dot" /> Collecting. In game, put your mouse on an item and press <b>Ctrl+C</b>. Do that for
          every item you want to change, then press <b>{bk}</b> to come back here.</p>
      ) : (
        <ol className="batch-steps muted">
          <li>Press <b>{bk}</b> in game (or Start collecting). A small "Batch mode" box shows over the game.</li>
          <li>Put your mouse on each item you want to change and press <b>Ctrl+C</b>. The box counts them.</li>
          <li>Press <b>{bk}</b> again. This page opens: pick what happens to each item and save.</li>
        </ol>
      )}

      {list.length > 0 && (
        <>
          <div className="row batch-all">
            <span className="muted small">Set all to</span>
            {CHOICES.map(([c, label]) => <button key={c} className="small" onClick={() => setAll(c)}>{label}</button>)}
            {looks.length > 0 && (
              <select className="small" value="" onChange={(e) => e.target.value && allLook(e.target.value)} title="Give every item one of your saved looks">
                <option value="">A saved look…</option>
                {looks.map((l) => <option key={l.name} value={l.name}>{l.name}</option>)}
              </select>
            )}
            <button className="link small batch-clear" onClick={() => { forget(null); setList([]); setMsg(""); }}>Clear the list</button>
          </div>
          <ul className="batch">
            {list.map((e, i) => (
              <li key={key(e.item)}>
                <span className="batch-item">
                  <ItemIcon name={e.item.rarity === "Unique" ? e.item.name : null} base={e.item.base_type} size={30} />
                  <span>
                    <span className="batch-name" style={{ color: RARITY_COLOUR[e.item.rarity] ?? "#ddd" }}>{itemTitle(e.item)}</span>
                    <span className="muted small batch-sub">{itemSub(e.item)}{e.now ? ` · now ${NOW[e.now.visibility] ?? e.now.visibility}${e.now.by_user_edit ? " (your edit)" : ""}` : ""}</span>
                  </span>
                </span>
                <span className="batch-choice">
                  <span className="seg">
                    {choices.map(([c, label]) => (
                      <button key={c} className={e.choice === c ? "on" : ""}
                        onClick={() => update(i, { choice: c, look: c === "look" ? e.look ?? looks[0]?.name : e.look })}>{label}</button>
                    ))}
                  </span>
                  {e.choice === "highlight" && <span className="batch-colours">{COLOURS.map((c) => (
                    <button key={c.name} className={e.colour === c.name ? "dot on" : "dot"} style={{ background: c.hex }} title={c.name}
                      aria-label={c.name} aria-pressed={e.colour === c.name} onClick={() => update(i, { colour: c.name })} />
                  ))}{e.colour && <span className="se-chip-name">{e.colour}</span>}</span>}
                  <button className="link small" title="Take it off the list" onClick={() => remove(e)}>✕</button>
                </span>
                {e.choice === "look" && (
                  <span className="batch-looks">
                    <LookChips small text={e.item.base_type || e.item.item_class} looks={looks} picked={e.look}
                      onPick={(l) => update(i, { look: l.name })} />
                  </span>
                )}
              </li>
            ))}
          </ul>
          <button className="primary wide" disabled={busy} onClick={apply}>
            {busy ? "Saving…" : edits.length ? `Save ${edits.length} edit${edits.length === 1 ? "" : "s"}` : "Done (no changes)"}</button>
        </>
      )}
      {list.length === 0 && !on && !msg && <div className="empty small">Nothing collected yet.</div>}
      {msg && <div className="note">{msg}</div>}
    </div>
  );
}

/** The small box over the game while batch mode is on: how many items so far and the last one copied. */
export function BatchWindow() {
  const [c, setC] = useState<{ count: number; last: string; added: boolean } | null>(null);
  const bk = useBatchKey();
  useEffect(() => {
    invoke<Item[]>("batch_list").then((l) => setC((x) => x ?? { count: l.length, last: "", added: false })).catch(() => {});
    const un = listen<{ count: number; last: string; added: boolean }>("batch-count", (e) => setC(e.payload));
    return () => { un.then((f) => f()); };
  }, []);
  const n = c?.count ?? 0;
  // Fit the window to the box, so the title, the close button and the last line are never cut off.
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!box.current) return;
    const r = box.current.getBoundingClientRect();
    getCurrentWindow().setSize(new LogicalSize(Math.ceil(r.width) + 2, Math.ceil(r.height) + 2))
      .then(() => invoke("keep_on_screen")).catch(() => {});
  });
  return (
    <div className="qm batch-win" ref={box} data-tauri-drag-region>
      <WinBar compact title={<><span className="batch-dot" /> Batch mode · {n} item{n === 1 ? "" : "s"}</>}
        onClose={() => invoke("set_batch_mode", { on: false })} closeTip="Stop collecting" />
      <div className="batch-win-line small" data-tauri-drag-region>
        {c?.last ? <>{c.added ? "Added" : "Already listed"}: <b>{c.last}</b></> : <span className="muted">Ctrl+C items · {bk} when done</span>}
      </div>
    </div>
  );
}
