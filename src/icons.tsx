import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Item pictures (name -> URL), loaded once and refreshed when prices update. */
let cache: Record<string, string> | null = null;
let loading: Promise<Record<string, string>> | null = null;
const subscribers = new Set<(m: Record<string, string>) => void>();

function load(force = false) {
  if (!loading || force) {
    loading = invoke<Record<string, string>>("get_icons").catch(() => ({})).then((m) => {
      cache = m;
      subscribers.forEach((f) => f(m));
      return m;
    });
  }
  return loading;
}
listen("prices-updated", () => load(true)).catch(() => {});
listen("active-changed", () => load(true)).catch(() => {});

export function useIcons(): Record<string, string> {
  const [m, setM] = useState<Record<string, string>>(cache ?? {});
  useEffect(() => {
    subscribers.add(setM);
    if (!cache) load();
    // A fresh install has no pictures until the first price download; try again shortly.
    const retry = window.setTimeout(() => { if (!cache || Object.keys(cache).length === 0) load(true); }, 8000);
    return () => { subscribers.delete(setM); window.clearTimeout(retry); };
  }, []);
  return m;
}

/** A unique's own picture first, then its base type's. */
export function ItemIcon({ name, base, size = 32 }: { name?: string | null; base?: string | null; size?: number }) {
  const icons = useIcons();
  const url = (name && icons[name]) || (base && icons[base]) || null;
  const [broken, setBroken] = useState(false);
  if (!url || broken) return null;
  return <img className="item-icon" src={url} alt="" width={size} height={size} loading="lazy" onError={() => setBroken(true)} />;
}

/** Fallback pictures for the two trade currencies (the game's own art on GGG's CDN). */
const CURRENCY_ART: Record<string, string> = {
  chaos: "https://web.poecdn.com/image/Art/2DItems/Currency/CurrencyRerollRare.png",
  divine: "https://web.poecdn.com/image/Art/2DItems/Currency/CurrencyModValues.png",
};
const CURRENCY_NAME: Record<string, string> = { chaos: "Chaos Orb", divine: "Divine Orb", exalted: "Exalted Orb" };

/** A small chaos / divine orb picture next to a price. */
export function CurrencyIcon({ id, size = 16 }: { id: string; size?: number }) {
  const icons = useIcons();
  const url = icons[CURRENCY_NAME[id] ?? ""] || CURRENCY_ART[id] || null;
  const [broken, setBroken] = useState(false);
  // No picture: say it in words instead.
  if (!url || broken) return <span className="cur-text"> {id === "divine" ? "div" : id === "chaos" ? "c" : id}</span>;
  return <img className="cur-icon" src={url} alt={id} title={CURRENCY_NAME[id] ?? id} width={size} height={size} onError={() => setBroken(true)} />;
}
