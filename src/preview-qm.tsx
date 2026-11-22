// Dev tool: the F2 pop-up (ItemPanel) in a browser with sample data. With the editor running, open
// http://localhost:1420/preview-qm.html  (?state=hide | icon | show picks how the sample item is styled).
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { createRoot } from "react-dom/client";
import ItemPanel from "./ItemPanel";
import "./styles.css";

const params = new URLSearchParams(location.search);
const state = params.get("state") ?? "hide";
// ?editor=1: the panel as the main window shows it (Find / Tiers), not over the game.
const editor = params.get("editor") === "1";
const style = { font_size: 40, text: [255, 255, 255, 255], border: null, background: [0, 0, 0, 200], beam: null, muted: false, label_hidden: state === "icon" };
const reply = {
  stackable: true, area_level: 83, game_filter: null,
  status: { assumptions: [], by_stack: [],
    verdict: { visibility: state === "hide" ? "Hide" : "Show", style, decided_by: "Orb of Unmaking", by_user_edit: state !== "show", reason: "", uncertain: [],
      // ?state=show: decided by a NeverSink step, as the Tiers page names it.
      source: state === "show"
        ? { kind: "neversink", label: "", type_path: ["crafting", "generalgear"], tier: "t1_86", section: "Crafting bases", tier_name: "Tier 1 bases, item level 86+",
            conds: ["not mirrored", "not corrupted", "item level 86+", "white, magic, rare"] }
        : { kind: "edit", label: "Orb of Unmaking", type_path: [], tier: null, section: null, tier_name: null } } },
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
  <div style={{ width: editor ? 900 : 400, margin: 16 }} className={editor ? "" : "qm"}><ItemPanel start={{ item: item as never, from_screen: !editor, screen_text: "ORB OF UNMAKING", confidence: 1 }} inGame={!editor} onClose={() => {}} /></div>,
);
