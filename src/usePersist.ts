import { useEffect, useState } from "react";

/** useState that's remembered on this PC (search boxes, picked options): comes back after a restart. */
export function usePersist<T>(key: string, start: T) {
  const [v, setV] = useState<T>(() => {
    try { const s = localStorage.getItem(key); return s == null ? start : (JSON.parse(s) as T); } catch { return start; }
  });
  useEffect(() => { try { localStorage.setItem(key, JSON.stringify(v)); } catch { /* not kept */ } }, [key, v]);
  return [v, setV] as const;
}
