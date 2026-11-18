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
