// Engine-owned terminals for interactive-terminal loops. The loop's claude session runs in a PTY
// that lives for as long as the loop runs (created on start, killed on stop) — NOT tied to a React
// component, so navigating away from the Loops page doesn't kill the session. The on-screen terminal
// (LoopTerminal) just attaches a sink to receive live bytes and re-paints via a resize when it mounts.
import { createPty, writePty, killPty } from "../api";

type Sink = { write: (b: Uint8Array) => void };
const sinks = new Map<string, Set<Sink>>();
const liveIds = new Set<string>();

export function isLoopTermLive(id: string): boolean {
  return liveIds.has(id);
}

// start the loop's claude session in a PTY and type the launch command once the shell settles
export async function startLoopTerm(
  id: string,
  cwd: string,
  env: Record<string, string>,
  launchCmd: string,
  onExit: () => void,
): Promise<void> {
  await createPty(
    { id, cwd, args: [], env, cols: 120, rows: 34 },
    {
      onData: (bytes) => {
        const set = sinks.get(id);
        if (set) for (const s of set) s.write(bytes);
      },
      onControl: (c) => {
        if (c.type === "exit") {
          // stopLoopTerm removes the id BEFORE killing — if it's already gone, this exit was
          // engine-initiated and onExit must not fire (it would double-finish the loop)
          if (!liveIds.delete(id)) return;
          onExit();
        }
      },
    },
  );
  liveIds.add(id);
  const enc = new TextEncoder();
  // give the shell a beat to print its prompt, then fire the claude command
  setTimeout(() => {
    if (liveIds.has(id)) void writePty(id, enc.encode(launchCmd + "\r")).catch(() => {});
  }, 900);
}

export function stopLoopTerm(id: string): void {
  liveIds.delete(id);
  void killPty(id).catch(() => {});
}

// a mounted LoopTerminal attaches its sink to get live bytes; returns a detach fn
export function attachLoopTerm(id: string, sink: Sink): () => void {
  let set = sinks.get(id);
  if (!set) {
    set = new Set();
    sinks.set(id, set);
  }
  set.add(sink);
  return () => {
    const s = sinks.get(id);
    if (s) {
      s.delete(sink);
      if (s.size === 0) sinks.delete(id);
    }
  };
}
