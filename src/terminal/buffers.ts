// Lightweight per-session output buffers so the command palette can search terminal
// history and jump to the matching pane. ANSI/control noise is stripped; capped per session.
//
// Perf: indexing is kept OFF the hot output path. onData just parks the raw bytes (an O(1) array
// push — no decode, no string), and a throttled flush does the costly decode + ANSI-strip + concat
// once per ~400ms. The flush also keeps only the tail that can survive the cap, so a firehose costs
// one bounded decode and one small regex per flush rather than one of each per chunk.
//
// The parked chunks are the same views handed to xterm's write(), which holds them across frames
// for the same reason: the backend allocates a fresh buffer per message and never reuses it.
const CAP = 24000; // chars kept per session

// Raw bytes worth decoding at all. Everything earlier is guaranteed to fall off the CAP: escape
// sequences and \r are stripped after decoding, so the raw tail always yields less text than bytes.
const MAX_RAW = CAP * 3;

// strip CSI / OSC escape sequences + carriage returns so searches match plain text
const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][AB0]/g;

const buffers = new Map<string, string>();
const pending = new Map<string, Uint8Array[]>();
// one decoder per session, kept between flushes: a multi-byte character can straddle two of them
const decoders = new Map<string, TextDecoder>();
let flushTimer: ReturnType<typeof setTimeout> | undefined;

/** The parked chunks as one buffer, or just their last `limit` bytes when there are more. */
function joinTail(chunks: Uint8Array[], limit: number): { bytes: Uint8Array; dropped: boolean } {
  let total = 0;
  for (const c of chunks) total += c.length;
  if (total <= limit) {
    const out = new Uint8Array(total);
    let at = 0;
    for (const c of chunks) {
      out.set(c, at);
      at += c.length;
    }
    return { bytes: out, dropped: false };
  }
  const out = new Uint8Array(limit);
  let need = limit;
  let at = limit;
  for (let i = chunks.length - 1; i >= 0 && need > 0; i--) {
    const c = chunks[i];
    const take = Math.min(need, c.length);
    at -= take;
    out.set(c.subarray(c.length - take), at);
    need -= take;
  }
  return { bytes: out, dropped: true };
}

function flush(): void {
  flushTimer = undefined;
  for (const [id, chunks] of pending) {
    const { bytes, dropped } = joinTail(chunks, MAX_RAW);
    // dropping the head breaks the stream anyway, so start that session's decoder clean rather
    // than gluing a half-finished character onto bytes that no longer follow it
    if (dropped) decoders.delete(id);
    let dec = decoders.get(id);
    if (!dec) {
      dec = new TextDecoder();
      decoders.set(id, dec);
    }
    const clean = dec.decode(bytes, { stream: true }).replace(ANSI, "").replace(/\r/g, "");
    if (!clean) continue;
    const next = (buffers.get(id) ?? "") + clean;
    buffers.set(id, next.length > CAP ? next.slice(next.length - CAP) : next);
  }
  pending.clear();
}

export function appendOutput(id: string, bytes: Uint8Array): void {
  const arr = pending.get(id);
  if (arr) arr.push(bytes);
  else pending.set(id, [bytes]);
  if (!flushTimer) flushTimer = setTimeout(flush, 400);
}

export function dropOutput(id: string): void {
  buffers.delete(id);
  pending.delete(id);
  decoders.delete(id);
}

/**
 * Most-recent slice of a session's output.
 *
 * `drain` decides whether parked chunks are indexed first. Leave it on for anything a person just
 * asked for (copying a pane's output). Turn it off for anything that polls, like the row-state
 * heuristics: draining from inside a render defeats the batching above, since a grid of rows
 * re-rendering every second forces a flush each time instead of one every 400ms. Those callers
 * re-read constantly and tolerate being a moment behind.
 */
export function recentOutput(id: string, max = 4000, drain = true): string {
  if (drain) flush();
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
