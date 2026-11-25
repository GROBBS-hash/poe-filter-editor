import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { friendly } from "./errors";
import { CurrencyIcon, ItemIcon } from "./icons";
import { MechArt } from "./mechArt";
import { usePersist } from "./usePersist";
import { Tip } from "./Help";
import "./tonight.css";

type Card = { kind: "sell" | "run" | "fix"; title: string; reason: string; worth: [number, number] | null; items: [string, string][]; mechs: string[];
  action: string; button: string; as_of: number | null };

const LABEL = { sell: "Sell", run: "Run", fix: "Fix" };
const c = (v: number) => (Math.abs(v) >= 1000 ? Math.round(v / 10) * 10 : Math.round(v)).toLocaleString();
const ago = (t: number) => {
  const h = (Date.now() / 1000 - t) / 3600;
  return h < 1 ? "just now" : h < 48 ? `${Math.round(h)} h ago` : `${Math.round(h / 24)} days ago`;
};

function Pic({ name, icon }: { name: string; icon: string }) {
  const [broken, setBroken] = useState(false);
  return icon && !broken ? <img className="item-icon" src={icon} alt="" width={34} height={34} title={name} onError={() => setBroken(true)} /> : <ItemIcon name={name} base={name} size={34} />;
}

/** Home: how long have you got? Up to three things worth doing, one button each. */
export default function Tonight({ go }: { go: (tab: string) => void }) {
  const [minutes, setMinutes] = usePersist("tonight.minutes", 30);
  const [cards, setCards] = useState<Card[] | null>(null);
  const [note, setNote] = useState("");
  useEffect(() => {
    let on = true;
    const load = () => invoke<Card[]>("tonight_get", { minutes }).then((x) => on && setCards(x ?? [])).catch(() => on && setCards([]));
    load();
    const un = ["prices-updated", "farm-now", "deaths-changed", "wealth-updated"].map((e) => listen(e, load));
    return () => { on = false; un.forEach((u) => u.then((f) => f())); };
  }, [minutes]);
  if (!cards || cards.length === 0) return null;
  const act = async (k: Card) => {
    setNote("");
    try {
      if (k.action.startsWith("tab:")) go(k.action.slice(4));
      else if (k.action === "highlight") {
        const [, n] = await invoke<[string, number]>("stash_highlight", { names: k.items.map((i) => i[0]), typeIt: true });
        setNote(`Typed in game: ${n} of ${k.items.length} names fit. Open your stash first if nothing lit up.`);
      } else if (k.action.startsWith("avoid:")) {
        await invoke("deaths_avoid", { text: k.action.slice(6), on: true });
        setNote("On your can't-run list: Ctrl+D warns about it, and your map search leaves it out.");
        setCards(await invoke<Card[]>("tonight_get", { minutes }));
      }
    } catch (e) { setNote(friendly(e)); }
  };
  return (
    <section className="tn">
      <div className="tn-head">
        <h3>Tonight</h3>
        <Tip>Worked out from your last stash scan, your own maps and poe.ninja's prices. A card is left out when there isn't enough to go by.</Tip>
        <span className="tn-time">{[15, 30, 60].map((m) => <button key={m} className={m === minutes ? "on" : ""} aria-pressed={m === minutes} onClick={() => setMinutes(m)}>{m} min</button>)}</span>
      </div>
      <div className="tn-cards">
        {cards.map((k) => (
          <article key={k.kind} className={`tn-card ${k.kind}`}>
            <div className="tn-kind">{LABEL[k.kind]}</div>
            {(k.mechs.length > 0 || k.items.length > 0) && <div className="tn-pics">
              {k.mechs.map((m) => <MechArt key={m} m={m} size={42} />)}
              {k.items.map(([n, i]) => <Pic key={n} name={n} icon={i} />)}
            </div>}
            <b className="tn-title">{k.title}</b>
            {k.worth && <div className="tn-worth">about {Math.round(k.worth[0]) === Math.round(k.worth[1]) ? c(k.worth[0]) : `${c(k.worth[0])} to ${c(k.worth[1])}`}<CurrencyIcon id="chaos" /></div>}
            <div className="small tn-why">{k.reason}{k.as_of != null && <span className="muted"> · stash read {ago(k.as_of)}</span>}</div>
            <button onClick={() => act(k)}>{k.button}</button>
          </article>
        ))}
      </div>
      {note && <div className="note small">{note}</div>}
    </section>
  );
}
