import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import ItemPanel, { Picked } from "./ItemPanel";

/** The in-game pop-up window: shows the item panel next to your mouse. */
export default function QuickMenu() {
  const [picked, setPicked] = useState<Picked | null>(null);
  const [failed, setFailed] = useState("");
  const [seq, setSeq] = useState(0); // fresh panel for every F2
  const editingRef = useRef(false);
  const box = useRef<HTMLDivElement | null>(null);

  const close = useCallback(async () => {
    setPicked(null);
    setFailed("");
    editingRef.current = false;
    await getCurrentWindow().hide();
  }, []);

  // Fit the window to the menu as it grows (e.g. when the editor opens).
  useEffect(() => {
    if (!box.current) return;
    const ro = new ResizeObserver(() => {
      const h = Math.ceil(box.current?.getBoundingClientRect().height ?? 300) + 4;
      getCurrentWindow().setSize(new LogicalSize(300, Math.max(120, h))).catch(() => {});
    });
    ro.observe(box.current);
    return () => ro.disconnect();
  });

  useEffect(() => {
    const a = listen<Picked>("item-picked", (e) => {
      setFailed("");
      setPicked(e.payload);
      setSeq((n) => n + 1);
    });
    const b = listen<string>("pick-failed", (e) => {
      setPicked(null);
      setFailed(e.payload);
      setTimeout(close, 2500);
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !editingRef.current) close();
      if (e.key.toLowerCase() === "h" && !editingRef.current && document.activeElement?.tagName !== "INPUT")
        document.getElementById("qm-hide")?.click();
    };
    // Clicking back into the game closes the menu, but not while editing
    // (the colour picker opens its own window).
    const onBlur = () => { if (!editingRef.current) close(); };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", onBlur);
    return () => {
      a.then((f) => f()); b.then((f) => f());
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", onBlur);
    };
  }, [close]);

  const onEditingChange = useCallback((v: boolean) => { editingRef.current = v; }, []);

  if (failed) return <div className="qm" ref={box}><div className="qm-fail">{failed}</div></div>;
  if (!picked) return null;
  return (
    <div className="qm" ref={box}>
      <ItemPanel key={seq} start={picked} inGame onClose={close} onEditingChange={onEditingChange} />
    </div>
  );
}
