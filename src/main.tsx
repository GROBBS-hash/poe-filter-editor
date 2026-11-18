import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import QuickMenu from "./QuickMenu";
import App from "./App";
import "./styles.css";

const isOverlay = getCurrentWindow().label === "overlay";
document.body.classList.add(isOverlay ? "overlay" : "main");
createRoot(document.getElementById("root")!).render(isOverlay ? <QuickMenu /> : <App />);
