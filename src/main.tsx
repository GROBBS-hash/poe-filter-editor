import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import QuickMenu from "./QuickMenu";
import App from "./App";
import BuildOverlay from "./BuildOverlay";
import TradeWindow from "./TradeWindow";
import TradesWindow from "./TradesWindow";
import EarningsWindow from "./EarningsWindow";
import AtlasOverlay from "./AtlasOverlay";
import "./styles.css";

const label = getCurrentWindow().label;
document.body.classList.add(label === "overlay" || label === "build" || label === "trade" || label === "trades" || label === "earnings" || label === "atlas" ? "overlay" : "main");
if (label === "trade") document.body.classList.add("scroll");

// Windows over the game fade and settle in each time they appear, instead of popping in hard.
if (label !== "main") {
  const pop = () => {
    const r = document.getElementById("root");
    if (!r) return;
    r.classList.remove("pop-in");
    void r.offsetWidth;
    r.classList.add("pop-in");
  };
  document.addEventListener("visibilitychange", () => { if (document.visibilityState === "visible") pop(); });
  for (const ev of ["price-check", "item-picked", "build-shown"]) listen(ev, pop);
  pop();
}
createRoot(document.getElementById("root")!).render(label === "overlay" ? <QuickMenu /> : label === "build" ? <BuildOverlay /> : label === "trade" ? <TradeWindow /> : label === "trades" ? <TradesWindow /> : label === "earnings" ? <EarningsWindow /> : label === "atlas" ? <AtlasOverlay /> : <App />);
