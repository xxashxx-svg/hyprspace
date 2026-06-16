// Lightweight per-session output buffers so the command palette can search terminal
// history and jump to the matching pane. ANSI/control noise is stripped; capped per session.
const CAP = 24000; // chars kept per session

// strip CSI / OSC escape sequences + carriage returns so searches match plain text
const ANSI = /\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[()][AB0]/g;

const buffers = new Map<string, string>();

export function appendOutput(id: string, text: string): void {
  const clean = text.replace(ANSI, "").replace(/\r/g, "");
  if (!clean) return;
  const next = (buffers.get(id) ?? "") + clean;
  buffers.set(id, next.length > CAP ? next.slice(next.length - CAP) : next);
}

export function dropOutput(id: string): void {
  buffers.delete(id);
}

// most-recent slice of a session's output (used for AI auto-naming a space)
export function recentOutput(id: string, max = 4000): string {
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
