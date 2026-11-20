import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { CurrencyIcon, ItemIcon } from "./icons";

/** "What could this be?": every unique that drops on this base, with its poe.ninja price, most valuable first.
 *  Shown for uniques we can't name yet (unidentified, or read off the ground). */
export default function BaseUniques({ base, divine }: { base: string; divine?: number | null }) {
  const [list, setList] = useState<[string, number][]>([]);
  useEffect(() => {
    let live = true;
    invoke<[string, number][]>("base_uniques", { base }).then((l) => live && setList(l ?? [])).catch(() => live && setList([]));
    return () => { live = false; };
  }, [base]);
  if (list.length === 0) return null;
  const top = list[0][1];
  return (
    <div className="bu">
      <div className="bu-title">{list.length === 1 ? `The only unique ${base}` : `Could be one of ${list.length} uniques`}</div>
      <ul>
        {list.slice(0, 8).map(([name, v]) => (
          <li key={name} className={v >= 100 ? "hi" : v >= 20 ? "mid" : ""}>
            <ItemIcon name={name} base={base} size={22} />
            <span className="bu-name">{name}</span>
            <span className="bu-price">
              {divine && v >= divine ? <>{+(v / divine).toFixed(1)}<CurrencyIcon id="divine" /></> : <>{v >= 10 ? Math.round(v) : +v.toFixed(1)}<CurrencyIcon id="chaos" /></>}
            </span>
          </li>
        ))}
      </ul>
      {list.length > 1 && top >= 20 && list[list.length - 1][1] < top / 5 && (
        <div className="muted small">Prices differ a lot: identify it (or check the name) before getting excited.</div>
      )}
    </div>
  );
}
