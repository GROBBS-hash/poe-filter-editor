import { Component, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

const ISSUES = "https://github.com/GROBBS-hash/poe-filter-editor/issues/new";

/**
 * If a page (or one part of a page) crashes, show a plain note in its place instead of leaving the window blank.
 * 28 Nov 2026 (whole-app check, error recovery 1/4): plain words for strangers, the technical details folded away with
 * Copy details and Report it, and `onError` so a page that remembers its last part can forget a part that crashed
 * (otherwise it reopens straight into the crash).
 */
export default class ErrorBoundary extends Component<{ children: ReactNode; where?: string; onError?: () => void }, { error: Error | null; copied: boolean }> {
  state = { error: null as Error | null, copied: false };
  static getDerivedStateFromError(error: Error) { return { error }; }
  componentDidCatch(error: Error) { console.error("Page crashed:", error); this.props.onError?.(); }

  private async details() {
    const e = this.state.error;
    let version = "?";
    try { version = await getVersion(); } catch { /* preview */ }
    return [`PoE Lodestone ${version}`, `Where: ${this.props.where ?? "a page"}`, `Error: ${e?.message || String(e)}`, e?.stack ?? ""].join("\n").trim();
  }
  private async copy() {
    try { await navigator.clipboard.writeText(await this.details()); this.setState({ copied: true }); } catch { /* clipboard blocked */ }
  }

  render() {
    const e = this.state.error;
    if (!e) return this.props.children;
    return (
      <div className="card err-card" role="alert">
        <h2>This part didn't load</h2>
        <p>Something went wrong{this.props.where ? ` in ${this.props.where}` : ""}. The rest of the editor still works, and nothing you saved is lost.</p>
        <div className="row">
          <button onClick={() => this.setState({ error: null, copied: false })}>Try again</button>
          <button onClick={() => this.copy()}>{this.state.copied ? "Copied" : "Copy details"}</button>
          <button className="link" onClick={() => invoke("open_url", { url: ISSUES }).catch(() => window.open(ISSUES))}>Report it</button>
        </div>
        <details className="err-more"><summary className="muted small">Technical details</summary>
          <pre className="err-box">{e.message || String(e)}</pre>
        </details>
      </div>
    );
  }
}
