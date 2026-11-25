import { useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ProfileInfo, STRICTNESS_NAMES } from "./types";
import type { PobState } from "./BuildView";
import { currentStrategy, type AtlasStore } from "./AtlasTreeView";
import KeyList from "./KeyList";
import Tonight from "./Tonight";

type Change = { index: number; at: number; what: string };
type PriceInfo = { settings: { enabled: boolean }; info?: { league?: string; updated?: string | null } };

/** "5 min ago", "3 h ago", "2 days ago". */
function ago(secs: number) {
  const s = Math.max(0, Date.now() / 1000 - secs);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  const d = Math.floor(s / 86400);
  return d === 1 ? "yesterday" : `${d} days ago`;
}

function Tile({ title, children, action, onAction }: { title: string; children: ReactNode; action: string; onAction: () => void }) {
  return (
    <section className="home-tile">
      <h3>{title}</h3>
      <div className="home-tile-body">{children}</div>
      <button className="link" onClick={onAction}>{action} →</button>
    </section>
  );
}

/** The first page: what's set up, what changed lately, and the keys that matter in game. */
export default function Home({ profile, keys, go, search }: {
  profile: ProfileInfo; keys: Record<string, string>; go: (tab: string) => void; search: () => void;
}) {
  const [history, setHistory] = useState<Change[] | null>(null);
  const [prices, setPrices] = useState<PriceInfo | null>(null);
  const [build, setBuild] = useState<PobState | null | undefined>(undefined);
  const [atlas, setAtlas] = useState<AtlasStore | null>(null);
  const [reloaded, setReloaded] = useState("");

  useEffect(() => {
    invoke<Change[]>("get_history", { id: profile.id }).then(setHistory).catch(() => setHistory([]));
    invoke<PriceInfo>("get_price_settings", { id: profile.id }).then(setPrices).catch(() => setPrices(null));
  }, [profile.id]);
  useEffect(() => {
    invoke<PobState | null>("pob_get").then(setBuild).catch(() => setBuild(null));
    invoke<AtlasStore>("atlas_list").then(setAtlas).catch(() => setAtlas(null));
  }, []);

  const built = profile.strictness != null
    ? <>NeverSink <b>{STRICTNESS_NAMES[profile.strictness]}</b>{profile.base_version ? <span className="muted"> · version {profile.base_version}</span> : null}</>
    : <>Your own filter file</>;
  const recent = (history ?? []).slice().sort((a, b) => b.at - a.at).slice(0, 3);
  const strat = currentStrategy(atlas);
  const updated = prices?.info?.updated ? ago(Date.parse(prices.info.updated) / 1000) : null;


  return (
    <div className="home">
      <button className="home-search" onClick={search}>
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm10 17-5.2-5.2" /></svg>
        <span>Search items, pages and help</span>
        <kbd>Ctrl+F</kbd>
      </button>

      <Tonight go={go} />

      <div className="home-grid">
        <Tile title="Your filter" action="Filters" onAction={() => go("profiles")}>
          <div className="home-big">{profile.name} <span className="muted small">{profile.game === "Poe1" ? "PoE 1" : "PoE 2"}</span></div>
          <div>{built}</div>
          <div className="muted small">{profile.rules.length ? `${profile.rules.length} of your own changes on top` : "No changes of your own yet"}
            {profile.theme ? ` · ${profile.theme} theme` : ""}</div>
          <button className="home-reload" onClick={() => {
            setReloaded("Reloading…");
            invoke("reload_filter").then(() => setReloaded("Reloaded in game")).catch(() => setReloaded("Open the game first, then press F4"));
          }}>Reload in game</button>
          {reloaded && <span className="muted small"> {reloaded}</span>}
        </Tile>

        <Tile title="Recent changes" action="My edits" onAction={() => go("edits")}>
          {history === null ? <span className="muted">Loading…</span>
            : recent.length === 0 ? <span className="muted">Nothing changed yet. Press {keys.key_quick_edit ?? "F2"} on an item in game to start.</span>
            : <ul className="home-list">{recent.map((c) => <li key={c.index}><span>{c.what}</span><span className="muted small">{ago(c.at)}</span></li>)}</ul>}
        </Tile>

        <Tile title="Price tiers" action="Price tiers" onAction={() => go("prices")}>
          {prices === null ? <span className="muted">Not loaded</span> : prices.settings.enabled
            ? <><div><span className="home-on">On</span> valuable drops stand out by price</div>
                <div className="muted small">{prices.info?.league ?? ""}{updated ? ` · updated ${updated}` : ""}</div></>
            : <><div><span className="home-off">Off</span> NeverSink's own tiers</div>
                <div className="muted small">Switch on to tier uniques and cards by live price</div></>}
        </Tile>

        <Tile title="Your build" action={build ? "My build" : "Import a build"} onAction={() => go("build")}>
          {build === undefined ? <span className="muted">Loading…</span> : build
            ? <><div className="home-big">{build.build.ascendancy || build.build.class}</div>
                <div className="muted small">Level {build.build.level} · {keys.key_build ?? "F6"} shows it in game</div></>
            : <span className="muted">No build yet. Paste a Path of Building link to see your gems, gear and tree in game.</span>}
        </Tile>

        <Tile title="Atlas strategy" action="Atlas tree" onAction={() => go("atlas")}>
          {strat
            ? <><div className="home-big">{strat.name}</div><div className="muted small">{keys.key_atlas ?? "F7"} shows it in game</div></>
            : <span className="muted">None yet. Import one from maxroll or pathofexile.com.</span>}
        </Tile>

        <Tile title="Crafting" action="Crafting" onAction={() => go("craft")}>
          <span className="muted">Press {keys.key_price ?? "Ctrl+D"} on an item and click <b>Craft this</b>, or pick a base here to find the cheapest way to a mod.</span>
        </Tile>
      </div>

      <section className="home-keys">
        <h3>In game</h3>
        <KeyList keys={keys} />
        <button className="link small" onClick={() => go("settings")}>Change hotkeys</button>
      </section>
    </div>
  );
}
