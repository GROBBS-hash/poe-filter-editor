import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import QuickMenu from "./QuickMenu";
import App from "./App";
import BuildOverlay from "./BuildOverlay";
import TradeWindow from "./TradeWindow";
import TradesWindow from "./TradesWindow";
import EarningsWindow from "./EarningsWindow";
import "./styles.css";

const label = getCurrentWindow().label;
document.body.classList.add(label === "overlay" || label === "build" || label === "trade" || label === "trades" || label === "earnings" ? "overlay" : "main");
createRoot(document.getElementById("root")!).render(label === "overlay" ? <QuickMenu /> : label === "build" ? <BuildOverlay /> : label === "trade" ? <TradeWindow /> : label === "trades" ? <TradesWindow /> : label === "earnings" ? <EarningsWindow /> : <App />);
