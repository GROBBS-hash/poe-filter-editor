import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { Game, Overview } from "./types";

export default function Setup({ ov, onDone }: { ov: Overview; onDone: () => void }) {
  const [step, setStep] = useState(0);
  const [game, setGame] = useState<Game>("Poe1");
  const [file, setFile] = useState("");
  const [name, setName] = useState("League Start");
  const [error, setError] = useState("");

  const pick = async () => {
    const f = await open({
      defaultPath: game === "Poe1" ? ov.poe1_dir : ov.poe2_dir,
      filters: [{ name: "Loot filter", extensions: ["filter"] }],
    });
    if (typeof f === "string") setFile(f);
  };

  const finish = async () => {
    setError("");
    try {
      await invoke("create_profile", { name: name.trim(), game, basePath: file });
      onDone();
    } catch (e) { setError(String(e)); }
  };

  const fileName = file.split(/[\\/]/).pop();

  return (
    <div className="page setup">
      <div className="brand">Let's set up your filter</div>
      <div className="muted">Step {step + 1} of 3</div>

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
          <h2>Pick your NeverSink filter</h2>
          <p className="muted">The one you downloaded from FilterBlade or GitHub. We never change it; your edits are layered on top in a new file.</p>
          <button className="big" onClick={pick}>{fileName ?? "Choose file…"}</button>
          <div className="row">
            <button onClick={() => setStep(0)}>Back</button>
            <button className="primary" disabled={!file} onClick={() => setStep(2)}>Next</button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="card">
          <h2>Name this filter</h2>
          <p className="muted">This is the name you'll pick in the game's options. You can save stricter versions later.</p>
          <input className="text" value={name} onChange={(e) => setName(e.target.value)} />
          <div className="row">
            <button onClick={() => setStep(1)}>Back</button>
            <button className="primary" disabled={!name.trim()} onClick={finish}>Create filter</button>
          </div>
          {error && <div className="error">{error}</div>}
        </div>
      )}
    </div>
  );
}
