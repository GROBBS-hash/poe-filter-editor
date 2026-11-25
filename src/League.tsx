import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { friendly } from "./errors";
import { CurrencyIcon } from "./icons";
import { Tip } from "./Help";

type Choice = { id: string; name: string; file: string };
type View = { game: "Poe1" | "Poe2"; automatic: string | null; now: string | null; updated: string | null; picked: string | null; gone: boolean; choices: Choice[] };

/** Settings > Price check: the league prices, trade searches and your stash are about (mock-up B, 26 Nov). */
export default function LeaguePicker() {
  const [views, setViews] = useState<View[] | null>(null);
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");
  const load = () => invoke<View[]>("league_get").then(setViews).catch((e) => setMsg(friendly(e)));
  useEffect(() => { load(); const un = listen("prices-updated", load); return () => { un.then((f) => f()); }; }, []);
  const pick = async (v: View, id: string) => {
    setBusy(v.game); setMsg("");
    try {
      const got = await invoke<View[]>("league_set", { game: v.game, league: id || null });
      setViews(got);
      const now = got.find((x) => x.game === v.game)?.now;
      setMsg(now === v.now ? "Saved." : `${now} from now on: prices, trade searches and your stash. Press your reload key in game if your filter uses price tiers.`);
    } catch (e) { setMsg(friendly(e)); load(); }
    setBusy("");
  };
  return (
    <div className="card">
      <h2>League <Tip>Prices, trade searches, stash worth and price tiers all use this league. Automatic is the newest softcore
        league, and moves on by itself when a new one starts. Each league keeps its own stash tabs, scans and earnings session.</Tip></h2>
      {views === null ? <p className="muted small">Loading…</p> : views.map((v) => (
        <div key={v.game}>
          <div className="set-grid">
            <span className="key-label"><b><CurrencyIcon id={v.game === "Poe1" ? "chaos" : "exalted"} /> {v.game === "Poe1" ? "PoE 1" : "PoE 2"}</b>
              <span>{v.now ? <>Using {v.now}{v.updated ? `, prices from ${new Date(v.updated).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}` : ""}</> : "No prices yet"}</span></span>
            <select aria-label={`League for ${v.game === "Poe1" ? "PoE 1" : "PoE 2"}`} disabled={!!busy} value={v.picked ?? ""} onChange={(e) => pick(v, e.target.value)}>
              <option value="">{v.automatic ? `Automatic (${v.automatic})` : "Automatic"}</option>
              {v.gone && <option value={v.picked!}>{v.picked} (no prices)</option>}
              {v.choices.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </div>
          {v.gone && <p className="warn-text small">{v.picked} has no prices any more, so {v.now} is used instead.</p>}
          {!v.choices.length && <p className="muted small">The list of leagues hasn't been downloaded yet: try again in a few minutes.</p>}
        </div>
      ))}
      {busy ? <p className="muted small">Getting its prices…</p> : msg && <p className="note">{msg}</p>}
      <p className="muted small">Changes straight away. Leagues other than the newest get new prices less often.</p>
    </div>
  );
}
