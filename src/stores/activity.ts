import { create } from "zustand";

// Per-session live activity, lifted out of TerminalPane so the sidebar can show it.
// Runtime-only (not persisted): exited = PTY has ended; lastOut = ms of last output chunk.
// From these the Rail derives a status dot — working (recent output) / ready (alive, quiet) / exited.
interface ActivityState {
  exited: Record<string, boolean>;
  lastOut: Record<string, number>;
  markStart: (id: string) => void;
  markExit: (id: string) => void;
  markOutput: (id: string) => void;
  drop: (id: string) => void;
}

// throttle output bumps so a chatty terminal doesn't thrash the store (and the Rail)
const lastBump: Record<string, number> = {};

export const useActivity = create<ActivityState>()((set) => ({
  exited: {},
  lastOut: {},
  markStart: (id) =>
    set((s) => (s.exited[id] ? { exited: { ...s.exited, [id]: false } } : s)),
  markExit: (id) => set((s) => ({ exited: { ...s.exited, [id]: true } })),
  markOutput: (id) => {
    const now = Date.now();
    if (now - (lastBump[id] ?? 0) < 500) return;
    lastBump[id] = now;
    set((s) => ({
      lastOut: { ...s.lastOut, [id]: now },
      exited: s.exited[id] ? { ...s.exited, [id]: false } : s.exited,
    }));
  },
  drop: (id) => {
    delete lastBump[id];
    set((s) => {
      const exited = { ...s.exited };
      const lastOut = { ...s.lastOut };
      delete exited[id];
      delete lastOut[id];
      return { exited, lastOut };
    });
  },
}));
