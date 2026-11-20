// Dev tool: the F2 pop-up (ItemPanel) in a browser with sample data. With the editor running, open
// http://localhost:1420/preview-qm.html  (?state=hide | icon | show picks how the sample item is styled).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import ItemPanel from "./ItemPanel";
import "./styles.css";

const state = new URLSearchParams(location.search).get("state") ?? "hide";
const style = { font_size: 40, text: [255, 255, 255, 255], border: null, background: [0, 0, 0, 200], beam: null, muted: false, label_hidden: state === "icon" };
const reply = {
  stackable: true, area_level: 83, game_filter: null,
  status: { assumptions: [], by_stack: [],
    verdict: { visibility: state === "hide" ? "Hide" : "Show", style, decided_by: "Orb of Unmaking", by_user_edit: state !== "show", reason: "", uncertain: [],
      source: { kind: "edit", label: "Orb of Unmaking", type_path: [], tier: null, section: null, tier_name: null } } },
};
mockWindows("overlay");
mockIPC((cmd) => {
  if (cmd === "check_item" || cmd === "item_status") return reply;
  if (cmd === "item_price") return [0.82, "chaos"];
  if (cmd === "stack_choices") return [];
  if (cmd === "get_icons") return {};
  return null;
}, { shouldMockEvents: true });
document.body.className = "overlay";
const item = { item_class: "Stackable Currency", rarity: "Normal", name: null, base_type: "Orb of Unmaking", stack_size: 3, corrupted: false, mirrored: false, unidentified: false };
createRoot(document.getElementById("root")!).render(
  <div style={{ width: 400, margin: 16 }} className="qm"><ItemPanel start={{ item: item as never, from_screen: true, screen_text: "ORB OF UNMAKING", confidence: 1 }} inGame={true} onClose={() => {}} /></div>,
);
