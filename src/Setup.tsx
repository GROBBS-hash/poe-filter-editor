import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Game, Overview, STRICTNESS_NAMES } from "./types";

type Found = { name: string; path: string; modified: number; neversink: boolean; version?: string | null };
/** Where the new filter starts from. */
type Start = { kind: "neversink"; level: number } | { kind: "file"; path: string; name: string };

const ago = (secs: number) => {
  const d = Math.floor((Date.now() / 1000 - secs) / 86400);
  return d <= 0 ? "today" : d === 1 ? "yesterday" : d < 60 ? `${d} days ago` : `${Math.floor(d / 30)} months ago`;
};

/** First-time setup (also used for "New filter"). Three short steps, then how to load it in game. */
export default function Setup({ ov, onDone, onCancel }: { ov: Overview; onDone: () => void; onCancel?: () => void }) {
  const [step, setStep] = useState(0);
  const [game, setGame] = useState<Game>("Poe1");
  const [found, setFound] = useState<Found[] | null>(null);
  const [start, setStart] = useState<Start>({ kind: "neversink", level: 2 });
  const [name, setName] = useState(ov.profiles.length ? "" : "League Start");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  // Look in the game's filter folder; suggest the newest NeverSink file if there is one.
  useEffect(() => {
    if (step !== 1) return;
    invoke<Found[]>("list_filters", { game }).then((f) => {
      setFound(f);
      const best = f.find((x) => x.neversink);
      setStart(best ? { kind: "file", path: best.path, name: best.name } : { kind: "neversink", level: 2 });
    }).catch(() => setFound([]));
  }, [step, game]);

  const pick = async () => {
    const f = await open({ defaultPath: game === "Poe1" ? ov.poe1_dir : ov.poe2_dir, filters: [{ name: "Loot filter", extensions: ["filter"] }] });
    if (typeof f === "string") setStart({ kind: "file", path: f, name: f.split(/[\\/]/).pop() ?? f });
  };

  const finish = async () => {
    setError("");
    setBusy(true);
    try {
      await invoke("create_profile", {
        name: name.trim(), game,
        basePath: start.kind === "file" ? start.path : null,
        strictness: start.kind === "neversink" ? start.level : null,
      });
      setStep(3);
    } catch (e) { setError(String(e)); } finally { setBusy(false); }
  };

  const gameName = game === "Poe1" ? "Path of Exile" : "Path of Exile 2";

  return (
    <div className="page setup">
      <div className="row between">
        <div className="brand">{ov.profiles.length ? "New filter" : "Let's set up your filter"}</div>
        {onCancel && step < 3 && <button onClick={onCancel}>Cancel</button>}
      </div>
      {step < 3 && <div className="muted">Step {step + 1} of 3</div>}

      {step === 0 && (
        <div className="card">
          <h2>Which game?</h2>
          <div className="row">
            {(["Poe1", "Poe2"] as Game[]).map((g) => (
              <button key={g} className={game === g ? "big on" : "big"} onClick={() => setGame(g)}>
                {g === "Poe1" ? "Path of Exile" : "Path of Exile 2"}
              </button>
            ))}
          </div>
          <button className="primary" onClick={() => setStep(1)}>Next</button>
        </div>
      )}

      {step === 1 && (
        <div className="card">
          <h2>What should it start from?</h2>
          <p className="muted small">Your edits go on top of this. It's never changed, and you can switch later in Profiles.</p>

          <label className={`choice ${start.kind === "neversink" ? "on" : ""}`}>
            <input type="radio" checked={start.kind === "neversink"} onChange={() => setStart({ kind: "neversink", level: 2 })} />
            <span>
              <b>NeverSink's latest filter</b> <span className="muted small">(recommended, updates automatically)</span>
              <select disabled={start.kind !== "neversink"} value={start.kind === "neversink" ? start.level : 2}
                onChange={(e) => setStart({ kind: "neversink", level: +e.target.value })}>
                {STRICTNESS_NAMES.map((n, i) => <option key={i} value={i}>{n}{i === 2 ? " (most people)" : ""}</option>)}
              </select>
            </span>
          </label>

          {found === null && <div className="muted small">Looking in your {gameName} folder…</div>}
          {found && found.length > 0 && <div className="muted small found-head">Or one already in your {gameName} folder:</div>}
          {found?.map((f) => (
            <label key={f.path} className={`choice ${start.kind === "file" && start.path === f.path ? "on" : ""}`}>
              <input type="radio" checked={start.kind === "file" && start.path === f.path}
                onChange={() => setStart({ kind: "file", path: f.path, name: f.name })} />
              <span>
                <b>{f.name.replace(/\.filter$/i, "")}</b>{" "}
                <span className="muted small">
                  {f.neversink ? `NeverSink${f.version ? ` ${f.version}` : ""} (e.g. from FilterBlade)` : "not a NeverSink filter"} · {ago(f.modified)}
                </span>
              </span>
            </label>
          ))}
          <button className="link" onClick={pick}>Choose a file somewhere else…</button>
          {start.kind === "file" && !found?.some((f) => f.path === start.path) && <div className="note">Using {start.name}</div>}

          <div className="row between">
            <button onClick={() => setStep(0)}>Back</button>
            <button className="primary" onClick={() => setStep(2)}>Next</button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="card">
          <h2>Name this filter</h2>
          <p className="muted">This is the name you'll see in the game's filter list. You can save stricter copies later.</p>
          <input className="text" placeholder="e.g. League Start" value={name} onChange={(e) => setName(e.target.value)} autoFocus />
          <div className="row between">
            <button onClick={() => setStep(1)}>Back</button>
            <button className="primary" disabled={!name.trim() || busy} onClick={finish}>
              {busy ? (start.kind === "neversink" ? "Downloading NeverSink…" : "Creating…") : "Create filter"}
            </button>
          </div>
          {error && <div className="error">{error}</div>}
        </div>
      )}

      {step === 3 && (
        <div className="card done">
          <h2>“{name.trim()}” is ready</h2>
          <ol className="steps">
            <li>Go to {gameName} (Windowed Fullscreen works best).</li>
            <li>Press <b>F4</b>. It switches the game to “{name.trim()}” and loads it.</li>
            <li>Hover any item, in your bags or on the ground, and press <b>F2</b> to see what the filter does with it and change it.</li>
          </ol>
          <p className="muted small">The app sits in your system tray while you play. Close this window any time; the hotkeys keep working.</p>
          <button className="primary" onClick={onDone}>Got it</button>
        </div>
      )}
    </div>
  );
}
