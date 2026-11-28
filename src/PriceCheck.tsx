import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { friendly } from "./errors";
import { ErrNote } from "./LoginNeeded";
import { Tip } from "./Help";
import { keyOf } from "./hotkeys";
import SideList, { type Part, useSidePart } from "./SideList";
import { ExchangeCheck, OilsCard } from "./Play";

/** Price check in parts (28 Nov 2026: the exchange check and blight oils moved here from Game windows). */
const PC_PARTS: Part[] = [
  { key: "check", name: "Price check", icon: "M12 3v18M16.5 7.5c0-1.7-2-3-4.5-3s-4.5 1.3-4.5 3 2 2.6 4.5 3 4.5 1.3 4.5 3-2 3-4.5 3-4.5-1.3-4.5-3" },
  { key: "exchange", name: "Currency exchange", icon: "M4 8h13l-3-3M20 16H7l3 3" },
  { key: "oils", name: "Blight oils", icon: "M12 3c3 4 6 7.5 6 11a6 6 0 0 1-12 0c0-3.5 3-7 6-11z" },
];

type Saved = { id: number; name: string; live: boolean; setup: { filters: { enabled: boolean; label: string }[] } };

/** Open Settings on one of its parts (Settings remembers its part in localStorage and reads it when it opens). */
const openSettings = (part: string) => {
  try { localStorage.setItem("settings.section", part); } catch { /* opens on the last part instead */ }
  window.dispatchEvent(new CustomEvent("goto-tab", { detail: "settings" }));
};
const goTab = (tab: string) => window.dispatchEvent(new CustomEvent("goto-tab", { detail: tab }));

/** Price check page (22 Nov 2026, GROBBS: "price checker should have its own heading"): the Ctrl+D window itself
 *  lives over the game, so this page says how to use it, keeps your saved and live searches, and links its settings. */
export default function PriceCheck({ keys }: { keys: Record<string, string> }) {
  const key = keyOf(keys, "key_price");
  const [saved, setSaved] = useState<Saved[] | null>(null);
  const [err, setErr] = useState("");
  useEffect(() => { invoke<Saved[] | null>("saved_list").then((l) => setSaved(l ?? [])).catch(() => setSaved([])); }, []);
  const call = (cmd: string, args: Record<string, unknown>) =>
    invoke<Saved[] | null>(cmd, args).then((l) => { setSaved(l ?? []); setErr(""); }).catch((e) => setErr(friendly(e)));

  const [part, pick] = useSidePart("pricecheck", PC_PARTS);

  return (
    <SideList page="pricecheck" label="Price check parts" parts={PC_PARTS} part={part} onPick={pick}>
      {part === "exchange" && <ExchangeCheck />}
      {part === "oils" && <OilsCard />}
      {part === "check" && <>
      <div className="card">
        <h2>Price check an item</h2>
        <ol className="pc-steps">
          <li>In game, put your mouse on an item: in your stash, your bags, or its label on the ground.</li>
          <li>Press <kbd className="pc-key">{key}</kbd>. A window shows what it sells for straight away (poe.ninja), then listings
            from the trade site.</li>
          <li>Tick or untick stats to search again. <b>Whisper</b> on a listing copies the message for the seller.</li>
        </ol>
        <div className="pc-also">
          <div><b>Maps</b><span>Says "Safe to run" or "Don't run" for your build, and how to reroll it.</span></div>
          <div><b>Currency, scarabs, fragments</b><span>Shows the bulk exchange rate, in chaos or divines.</span></div>
          <div><b>Gear</b><span>Compares it with your build's gear, and <b>Craft this</b> opens it on the Crafting page.</span></div>
        </div>
      </div>

      <div className="card">
        <h2>Saved searches <Tip>Save a search from the price window (<b>Save search</b> at the bottom). <b>Live</b> checks the
          trade site every 90 seconds and pops up new listings over the game, up to 3 at a time. To run a saved search, press
          {" "}{key} in game with nothing under your mouse.</Tip></h2>
        {saved === null ? <div className="muted small">Loading…</div>
          : saved.length === 0 ? <div className="empty">No saved searches yet. Price check an item, then press <b>Save search</b> in the price window.</div>
          : <ul className="pc-saved">
            {saved.map((x) => (
              <li key={x.id}>
                <span className="pc-name" title={x.setup.filters.filter((f) => f.enabled).map((f) => f.label).join("\n")}>{x.name}</span>
                <button className={x.live ? "primary" : ""} onClick={() => call("saved_live", { id: x.id, on: !x.live })}
                  title={x.live ? "Live is on: new listings pop up over the game. Click to stop." : "Pop up new listings over the game while you play"}>
                  {x.live ? "Live: on" : "Go live"}</button>
                <button className="link" title="Delete this search" aria-label={`Delete ${x.name}`} onClick={() => call("saved_delete", { id: x.id })}>✕</button>
              </li>
            ))}
          </ul>}
        <ErrNote msg={err} className="qm-warn" />
      </div>

      <div className="card">
        <h2>Settings</h2>
        <div className="row">
          <button onClick={() => openSettings("price")}>What the price window shows</button>
          <button onClick={() => openSettings("looks")}>How the price window looks</button>
          <button onClick={() => openSettings("keys")}>Change the key</button>
          <button onClick={() => goTab("maps")}>Map mods you can't run</button>
        </div>
      </div>
      </>}
    </SideList>
  );
}
