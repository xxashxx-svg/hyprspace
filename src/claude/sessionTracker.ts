// Keep each claude pane's stored session id in sync with the conversation it's ACTUALLY on, so a
// manual /resume inside the pane persists across restarts — we resume by the exact id, never by
// "the folder's latest chat" (which can be a different pane's, or a stale one).
//
// claude writes each chat to <session-id>.jsonl in the folder, bumping its mtime as you talk, so
// the newest transcript in a folder is the live conversation. We only auto-track when a folder has
// a SINGLE claude pane — with several panes in one folder we can't tell which switched to what, so
// those stay pinned to their launch id (safe, no cross-talk).
import { useEffect } from "react";
import { useWorkspaces, type Session } from "../stores/workspace";
import { claudeSessions } from "../api";

const SCAN_MS = 5000;

let running = false; // single-flight: never let scans overlap or pile up

async function tick() {
  if (running) return;
  running = true;
  try {
    const panes = useWorkspaces
      .getState()
      .workspaces.flatMap((w) => w.sessions)
      .filter((s) => s.command?.includes("claude") && s.cwd && s.started);
    if (!panes.length) return;

    const byCwd = new Map<string, Session[]>();
    for (const p of panes) {
      const arr = byCwd.get(p.cwd!) ?? [];
      arr.push(p);
      byCwd.set(p.cwd!, arr);
    }

    for (const [cwd, here] of byCwd) {
      if (here.length !== 1) continue; // ambiguous folder → leave those panes pinned
      const pane = here[0];
      let list: [string, number][];
      try {
        list = await claudeSessions(cwd); // async on the backend — off the UI thread
      } catch {
        continue;
      }
      if (!list.length) continue;
      const newest = list.reduce((a, b) => (b[1] > a[1] ? b : a))[0]; // freshest transcript = live chat
      const cur = useWorkspaces
        .getState()
        .workspaces.flatMap((w) => w.sessions)
        .find((s) => s.id === pane.id);
      if (cur && cur.claudeSessionId !== newest) {
        useWorkspaces.getState().setClaudeSessionId(pane.id, newest);
      }
    }
  } finally {
    running = false;
  }
}

export function useClaudeSessionTracker() {
  useEffect(() => {
    const iv = setInterval(() => void tick(), SCAN_MS);
    const onHide = () => {
      if (document.visibilityState === "hidden") void tick(); // catch a /resume right before quitting
    };
    document.addEventListener("visibilitychange", onHide);
    return () => {
      clearInterval(iv);
      document.removeEventListener("visibilitychange", onHide);
    };
  }, []);
}
