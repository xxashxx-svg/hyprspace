// Lightweight per-session output buffers so the command palette can search terminal
// history and jump to the matching pane. ANSI/control noise is stripped; capped per session.
//
// Perf: indexing is kept OFF the hot output path. onData just parks raw chunks (an O(1) array
// push); a throttled flush does the costly ANSI-strip + concat once per ~400ms — so a firehose
// no longer runs a regex on every frame. The flush also strips only the tail that can survive the
// cap, so a 4MB/s stream costs one small regex per flush, not one over the whole stream.
const CAP = 24000; // chars kept per session

// strip CSI / OSC escape sequences + carriage returns so searches match plain text
const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][AB0]/g;

const buffers = new Map<string, string>();
const pending = new Map<string, string[]>();
let flushTimer: ReturnType<typeof setTimeout> | undefined;

function flush(): void {
  flushTimer = undefined;
  for (const [id, chunks] of pending) {
    let raw = chunks.join("");
    // only the tail can outlive the cap, so don't regex megabytes to keep 24KB
    if (raw.length > CAP * 3) raw = raw.slice(raw.length - CAP * 3);
    const clean = raw.replace(ANSI, "").replace(/\r/g, "");
    if (!clean) continue;
    const next = (buffers.get(id) ?? "") + clean;
    buffers.set(id, next.length > CAP ? next.slice(next.length - CAP) : next);
  }
  pending.clear();
}

export function appendOutput(id: string, text: string): void {
  const arr = pending.get(id);
  if (arr) arr.push(text);
  else pending.set(id, [text]);
  if (!flushTimer) flushTimer = setTimeout(flush, 400);
}

export function dropOutput(id: string): void {
  buffers.delete(id);
  pending.delete(id);
}

// most-recent slice of a session's output (used for AI auto-naming a space)
export function recentOutput(id: string, max = 4000): string {
  flush(); // drain parked chunks so callers see current output
  const t = buffers.get(id) ?? "";
  return t.length > max ? t.slice(t.length - max) : t;
}

export interface TermHit {
  sessionId: string;
  snippet: string;
}

// find sessions whose recent output contains the query, with a tidy one-line snippet
export function searchOutput(query: string): TermHit[] {
  const q = query.trim().toLowerCase();
  if (q.length < 2) return [];
  flush(); // index any parked output before searching
  const hits: TermHit[] = [];
  for (const [id, text] of buffers) {
    const idx = text.toLowerCase().lastIndexOf(q); // most recent occurrence
    if (idx < 0) continue;
    const start = text.lastIndexOf("\n", idx) + 1;
    let end = text.indexOf("\n", idx);
    if (end < 0) end = text.length;
    const snippet = text.slice(start, end).trim().slice(0, 120);
    hits.push({ sessionId: id, snippet: snippet || query });
  }
  return hits;
}
