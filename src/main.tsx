import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import QuickMenu from "./QuickMenu";
import App from "./App";
import BuildOverlay from "./BuildOverlay";
import "./styles.css";

const label = getCurrentWindow().label;
document.body.classList.add(label === "overlay" || label === "build" ? "overlay" : "main");
createRoot(document.getElementById("root")!).render(label === "overlay" ? <QuickMenu /> : label === "build" ? <BuildOverlay /> : <App />);
