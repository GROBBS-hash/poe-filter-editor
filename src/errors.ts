/**
 * Errors in plain English. The app's own messages are already written for him; raw ones from the network
 * ("Trade site error 400: Unknown item base type", "error sending request…") get a short explanation instead.
 */
export function friendly(e: unknown): string {
  const s = String(e instanceof Error ? e.message : e).replace(/^Error:\s*/, "").trim();
  const t = s.toLowerCase();
  let m: RegExpMatchArray | null;
  if ((m = s.match(/Trade site error 400:?\s*(.*)$/i))) return `The trade site couldn't search for this${m[1] ? ` (it said: ${m[1]})` : ""}. Try unticking a stat, or report it with "Something's wrong?".`;
  if (/trade site error 5\d\d|\b50[234]\b/.test(t)) return "The trade site is having trouble right now. Try again in a minute.";
  if (/trade site error 404/.test(t)) return "The trade site couldn't find that search any more. Search again.";
  if (/error sending request|dns|failed to lookup|connection (refused|reset)|could not connect|timed out|timeout|os error 10060|os error 11001|network/.test(t))
    return "Couldn't reach the website. Check your internet connection, then try again.";
  if (/^(json|expected value|eof while parsing|invalid type)/.test(t)) return "The website sent back something unexpected. Try again in a minute.";
  return s;
}
