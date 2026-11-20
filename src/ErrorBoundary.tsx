import { Component, type ReactNode } from "react";

/** If a page crashes, show what happened in that page instead of leaving the whole window blank. */
export default class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  componentDidCatch(error: Error) { console.error("Page crashed:", error); }
  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="card">
        <h2>This page hit a problem</h2>
        <p className="muted small">The rest of the app still works. Tell Claude what you clicked, and include this:</p>
        <pre className="err-box">{String(this.state.error.message || this.state.error)}</pre>
        <div className="row"><button onClick={() => this.setState({ error: null })}>Try again</button></div>
      </div>
    );
  }
}
