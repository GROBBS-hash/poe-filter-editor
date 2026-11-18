import { ReactNode, useState } from "react";

/** A small "?" that opens a short explanation under a heading. */
export function Tip({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className={`tip-btn ${open ? "on" : ""}`} title="What's this?" onClick={() => setOpen(!open)}>?</button>
      {open && <div className="tip-body" onClick={() => setOpen(false)}>{children}</div>}
    </>
  );
}

type Go = (tab: string) => void;
type Faq = { q: string; a: ReactNode; tab?: [string, string] };

const FAQ: Faq[] = [
  {
    q: "Go stricter (e.g. Semi-Strict → Uber Strict) but keep all my settings",
    a: <>Open <b>Profiles</b> and change <b>Built on</b> next to your filter to the strictness you want. Your edits, tier changes,
      levelling setup, price tiers and theme are all kept: they're stored separately and put back on top of the new NeverSink file.
      Press <b>F4</b> in game. Want to keep the old one too? Use <b>Save as new</b> first, then change the copy.</>,
    tab: ["profiles", "Open Profiles"],
  },
  {
    q: "Change how one item looks or sounds in game",
    a: <>Hover the item in game (in your stash, or the label on the ground) and press <b>F2</b>. Pick Hide, a quick colour, or
      <b> Customise</b> for text, border, background, opacity, beam, map icon and sound. It reloads in game for you.</>,
  },
  {
    q: "Hide small gold piles or small currency stacks",
    a: <>Press <b>F2</b> on the gold, then use the stack slider: “Only show 500+”. Or open <b>Tiers → Gold</b> and change the
      pile sizes.</>,
    tab: ["tiers", "Open Tiers"],
  },
  {
    q: "Re-rank currency, div cards or uniques (like FilterBlade's drag & drop)",
    a: <>Open <b>Tiers</b>, pick a section, and drag an item onto another tier. Untick a tier to hide it; click a tier's label
      to restyle the whole tier; Copy/Paste copies a look between tiers.</>,
    tab: ["tiers", "Open Tiers"],
  },
  {
    q: "Set up the filter for levelling a new character",
    a: <>Open <b>Presets → Levelling setup</b>. Pick your build (or tick weapons yourself), the armour types you wear, and
      whether to keep rares for vendoring. Everything else gets hidden until maps. Turn it off when you reach maps, or leave it:
      it stops applying at area level 68 anyway.</>,
    tab: ["presets", "Open Presets"],
  },
  {
    q: "Make valuable drops stand out using live prices",
    a: <>Open <b>Prices</b> and switch on price tiering. Prices come from poe.ninja and update every hour; anything worth more
      than your tier 1 amount gets the big Divine-style sound.</>,
    tab: ["prices", "Open Prices"],
  },
  {
    q: "Undo a change",
    a: <>Press <b>Undo last</b> in the quick menu, or open <b>My edits → History</b> and go back to before any change.</>,
    tab: ["edits", "Open My edits"],
  },
  {
    q: "My change isn't showing in game",
    a: <>Press <b>F4</b> in game: it switches the game to this filter and reloads it. If the quick menu says the game is using a
      different filter, F4 fixes that too. Items read off the ground are a best guess; hover them in your stash for an exact answer.</>,
  },
  {
    q: "NeverSink released an update. Do I need to do anything?",
    a: <>No. Filters built on NeverSink check for his updates every few hours and rebuild with your settings on top. If an item
      you moved no longer exists in his new version, My edits tells you.</>,
  },
  {
    q: "Change the whole colour scheme",
    a: <>Open <b>Themes</b>. Themes only change colours: your sounds (including the Divine drop sound for tier 1) and your own
      edits always win.</>,
    tab: ["themes", "Open Themes"],
  },
];

export default function Help({ go }: { go: Go }) {
  const [open, setOpen] = useState<number | null>(0);
  return (
    <div className="card">
      <h2>How do I…?</h2>
      <ul className="faq">
        {FAQ.map((f, i) => (
          <li key={i} className={open === i ? "open" : ""}>
            <button className="faq-q" onClick={() => setOpen(open === i ? null : i)}>{f.q}</button>
            {open === i && (
              <div className="faq-a">
                <p>{f.a}</p>
                {f.tab && <button onClick={() => go(f.tab![0])}>{f.tab[1]}</button>}
              </div>
            )}
          </li>
        ))}
      </ul>
      <p className="muted small">Hotkeys (change them in Settings): F2 edit the item under your mouse · F3 batch mode · F4 load/reload this filter in game.</p>
    </div>
  );
}
