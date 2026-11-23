import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { friendly } from "./errors";

/** "Log in with pathofexile.com": opens the site's login page in a small window; when you're logged in there, your
 *  account and POESESSID are saved and onDone gets the account name ("Name#1234"). */
export function PoeLogin({ onDone, onError, big, label }: { onDone?: (account: string) => void; onError?: (e: string) => void; big?: boolean; label?: string }) {
  const [waiting, setWaiting] = useState(false);
  const [done, setDone] = useState("");
  useEffect(() => {
    const un = listen<{ ok: boolean; account: string | null; error: string | null }>("poe-login", (e) => {
      setWaiting(false);
      if (e.payload.ok && e.payload.account) { setDone(e.payload.account); onDone?.(e.payload.account); }
      else if (e.payload.error) onError?.(e.payload.error);
    });
    return () => { un.then((f) => f()); };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  const go = () => { setWaiting(true); setDone(""); invoke("poe_login").catch((e) => { setWaiting(false); onError?.(friendly(e)); }); };
  return (
    <div className="row" style={{ margin: "8px 0" }}>
      <button className={big ? "primary" : ""} onClick={go}>{waiting ? "Waiting for you to log in…" : label ?? "Log in with pathofexile.com"}</button>
      {waiting && <span className="muted small">Log in in the window that opened (it closes by itself).</span>}
      {done && !waiting && <span className="w-full small">Logged in as {done}</span>}
    </div>
  );
}

/** A message that means your pathofexile.com login is missing or has run out. */
export const needsLogin = (msg: string) => /login has expired|needs you logged in|log in with pathofexile\.com/i.test(msg);

/**
 * An error line that, when it's about your pathofexile.com login, carries its own "Log in again" button (he asked
 * 23 Nov: log in from whatever page you're on, not hunt for Settings). Once you're logged in, `onRetry` runs what
 * failed again. Any other error shows as plain text, as before.
 */
export function ErrNote({ msg, className, onRetry }: { msg: string; className?: string; onRetry?: () => void }) {
  const [err, setErr] = useState("");
  if (!msg) return null;
  if (!needsLogin(msg)) return <div className={className}>{msg}</div>;
  return (
    <div className={`${className ?? ""} login-needed`}>
      <div><b>Your pathofexile.com login has run out.</b> Log in again and {onRetry ? "this carries on by itself" : "try again"}.</div>
      <PoeLogin big label="Log in again" onDone={() => { setErr(""); onRetry?.(); }} onError={setErr} />
      {err && <div className="small">{err}</div>}
    </div>
  );
}
