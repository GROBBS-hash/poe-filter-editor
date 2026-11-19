import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { clock, perHour, seconds, Worth, type WealthView } from "./Wealth";

/** The small in-game overlay: current strategy, time, made so far, per hour. */
export default function EarningsWindow() {
  const [v, setV] = useState<WealthView | null>(null);
  const [now, setNow] = useState(Date.now() / 1000);
  useEffect(() => {
    invoke<WealthView>("wealth_get").then(setV).catch(() => {});
    const un = listen<WealthView>("earn-update", (e) => setV(e.payload));
    const t = setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => { un.then((f) => f()); clearInterval(t); };
  }, []);
  const s = v?.current;
  if (!v || !s) return <div className="qm earn" data-tauri-drag-region><span className="muted small">No session running</span></div>;
  return (
    <div className="qm earn" data-tauri-drag-region>
      <div className="earn-top" data-tauri-drag-region>
        <b data-tauri-drag-region>{s.name}</b>
        <button className="th-x" title="Hide (the session keeps going)" onClick={() => invoke("earnings_overlay", { show: false })}>×</button>
      </div>
      <div className="earn-row" data-tauri-drag-region><span className="muted small">{s.paused_at ? "Paused" : clock(seconds(s, now))}</span>
        <span className="small"><Worth c={s.last_value - s.start_value} divine={v.divine} /></span></div>
      <div className="earn-rate" data-tauri-drag-region><Worth c={perHour(s, now)} divine={v.divine} /><span className="muted small"> /h</span></div>
    </div>
  );
}
