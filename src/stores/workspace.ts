import { create } from "zustand";
import { pickAgentName } from "../lib/names";

export interface Session {
  id: string;
  title: string;
  command?: string;
  cwd?: string;
  provider: "claude" | "gemini" | "codex" | "wsl" | "terminal";
  // claude panes pin to their session id (id IS a uuid); once launched we resume it next time
  started?: boolean;
  // the claude conversation this pane is currently on — starts as `id`, but follows a manual
  // /resume so the right chat comes back next launch (tracked in claude/sessionTracker)
  claudeSessionId?: string;
}

export interface Workspace {
  id: string;
  name: string;
  cwd: string;
  color: string;
  kind: "project" | "open";
  sessions: Session[];
  renamed?: boolean; // user renamed it → never auto-rename again
  aiNamed?: boolean; // AI already titled it → don't re-title (a manual rename still wins)
}

const COLORS = ["#3fb6e0", "#46c98a", "#e0a23f", "#b06ae0", "#e5484d", "#7dc4e8"];
const uid = () => crypto.randomUUID();

interface WorkspaceState {
  workspaces: Workspace[];
  activeId: string | null;
  focusedSessionId: string | null;
  hydrated: boolean;
  addWorkspace: (name?: string, cwd?: string) => string;
  addOpenSpace: (name?: string) => string;
  removeWorkspace: (id: string) => void;
  renameWorkspace: (id: string, name: string) => void;
  autoNameWorkspace: (id: string, name: string) => void;
  setActive: (id: string) => void;
  reorderWorkspaces: (fromId: string, toId: string) => void;
  addSession: (wsId: string, command?: string, cwd?: string) => void;
  removeSession: (wsId: string, sessionId: string) => void;
  markStarted: (sessionId: string) => void;
  setClaudeSessionId: (sessionId: string, claudeId: string) => void;
  reorderSessions: (wsId: string, fromId: string, toId: string) => void;
  moveSessionToWorkspace: (fromWsId: string, sessionId: string, toWsId: string) => void;
  setFocused: (id: string) => void;
  hydrate: (workspaces: Workspace[], activeId: string | null) => void;
  markHydrated: () => void;
}

export const useWorkspaces = create<WorkspaceState>()((set) => ({
  workspaces: [],
  activeId: null,
  focusedSessionId: null,
  hydrated: false,

  addWorkspace: (name, cwd = "") => {
    const id = uid();
    set((s) => {
      const color = COLORS[s.workspaces.length % COLORS.length];
      const ws: Workspace = {
        id,
        name: name || `Project ${s.workspaces.length + 1}`,
        cwd,
        color,
        kind: "project",
        sessions: [],
      };
      return { workspaces: [...s.workspaces, ws], activeId: id };
    });
    return id;
  },

  addOpenSpace: (name) => {
    const id = uid();
    set((s) => {
      const openCount = s.workspaces.filter((w) => w.kind === "open").length;
      const color = COLORS[s.workspaces.length % COLORS.length];
      const ws: Workspace = {
        id,
        name: name || `Open ${openCount + 1}`,
        cwd: "",
        color,
        kind: "open",
        sessions: [],
      };
      return { workspaces: [...s.workspaces, ws], activeId: id };
    });
    return id;
  },

  removeWorkspace: (id) =>
    set((s) => {
      const workspaces = s.workspaces.filter((w) => w.id !== id);
      const activeId = s.activeId === id ? (workspaces[0]?.id ?? null) : s.activeId;
      return { workspaces, activeId };
    }),

  renameWorkspace: (id, name) =>
    set((s) => ({
      // mark renamed so auto-naming never overrides a user-chosen name
      workspaces: s.workspaces.map((w) => (w.id === id ? { ...w, name, renamed: true } : w)),
    })),

  // AI-generated title. The periodic scanner skips already-aiNamed spaces itself, but a manual
  // "Rename with AI" routes through here too, so we only block on the user's own rename.
  autoNameWorkspace: (id, name) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) =>
        w.id === id && w.kind === "open" && !w.renamed ? { ...w, name, aiNamed: true } : w,
      ),
    })),

  setActive: (id) => set({ activeId: id }),

  // move one rail item to another's slot (drag-to-reorder; sections stay intact since
  // you can only drop onto a sibling that's visible in the same group)
  reorderWorkspaces: (fromId, toId) =>
    set((s) => {
      const arr = [...s.workspaces];
      const fi = arr.findIndex((w) => w.id === fromId);
      if (fi < 0) return {};
      const [moved] = arr.splice(fi, 1);
      const ti = arr.findIndex((w) => w.id === toId);
      if (ti < 0) return {};
      arr.splice(ti, 0, moved);
      return { workspaces: arr };
    }),

  addSession: (wsId, command, cwd) =>
    set((s) => {
      const id = uid();
      let provider: Session["provider"] = "terminal";
      let title = "Terminal";
      if (command?.includes("claude")) provider = "claude";
      else if (command?.includes("gemini")) provider = "gemini";
      else if (command?.includes("codex")) provider = "codex";
      else if (command === "wsl") {
        provider = "wsl";
        title = "WSL";
      }
      // agent panes get a short friendly name (the provider icon still shows what they are) so a
      // grid of identical agents is tellable apart; terminals/wsl keep their plain label.
      if (provider === "claude" || provider === "gemini" || provider === "codex") {
        const used = new Set(s.workspaces.flatMap((w) => w.sessions.map((ss) => ss.title)));
        title = pickAgentName(used);
      }

      const workspaces = s.workspaces.map((w) => {
        if (w.id !== wsId) return w;
        const sessions = [...w.sessions, { id, title, command, cwd: cwd ?? w.cwd, provider }];
        // instant placeholder: name an untouched open space after its folder. The AI namer
        // (ai/autoName) upgrades this to a descriptive title once there's real activity.
        let name = w.name;
        if (w.kind === "open" && !w.renamed && /^Open \d+$/.test(w.name)) {
          const folder = (cwd ?? "")
            .split(/[\\/]/)
            .filter(Boolean)
            .pop();
          if (folder) name = folder;
        }
        return { ...w, name, sessions };
      });
      return { workspaces, focusedSessionId: id };
    }),

  removeSession: (wsId, sessionId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) =>
        w.id === wsId
          ? { ...w, sessions: w.sessions.filter((ss) => ss.id !== sessionId) }
          : w,
      ),
    })),

  // flips once a claude pane has launched (we launch it with --session-id <id>, so its claude
  // conversation id starts as the pane id), so the next launch resumes instead of starting fresh
  markStarted: (sessionId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => ({
        ...w,
        sessions: w.sessions.map((ss) =>
          ss.id === sessionId && !ss.started
            ? { ...ss, started: true, claudeSessionId: ss.claudeSessionId ?? sessionId }
            : ss,
        ),
      })),
    })),

  // the tracker calls this when a pane switches to a different claude conversation (e.g. /resume)
  setClaudeSessionId: (sessionId, claudeId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => ({
        ...w,
        sessions: w.sessions.map((ss) =>
          ss.id === sessionId ? { ...ss, claudeSessionId: claudeId } : ss,
        ),
      })),
    })),

  reorderSessions: (wsId, fromId, toId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => {
        if (w.id !== wsId) return w;
        const arr = [...w.sessions];
        const fi = arr.findIndex((x) => x.id === fromId);
        const ti = arr.findIndex((x) => x.id === toId);
        if (fi < 0 || ti < 0 || fi === ti) return w;
        const t = arr[fi];
        arr[fi] = arr[ti];
        arr[ti] = t;
        return { ...w, sessions: arr };
      }),
    })),

  // move a whole session (its live PTY travels with it) from one space into another. The pane
  // grid renders every space's panes under one parent, so React keeps the component mounted —
  // the terminal doesn't restart. We stay on the source space; the dest just gains the pane.
  moveSessionToWorkspace: (fromWsId, sessionId, toWsId) =>
    set((s) => {
      if (fromWsId === toWsId) return {};
      let moved: Session | undefined;
      const stripped = s.workspaces.map((w) => {
        if (w.id !== fromWsId) return w;
        moved = w.sessions.find((ss) => ss.id === sessionId);
        return { ...w, sessions: w.sessions.filter((ss) => ss.id !== sessionId) };
      });
      if (!moved) return {};
      return {
        workspaces: stripped.map((w) =>
          w.id === toWsId ? { ...w, sessions: [...w.sessions, moved!] } : w,
        ),
      };
    }),

  setFocused: (id) => set({ focusedSessionId: id }),

  hydrate: (workspaces, activeId) => {
    // Migration: ensure every session has a 'provider' field (older saves won't have it)
    const migrated = workspaces.map((w) => ({
      ...w,
      sessions: w.sessions.map((s) => {
        if (s.provider) return s;
        let provider: Session["provider"] = "terminal";
        if (s.command?.includes("claude")) provider = "claude";
        else if (s.command?.includes("gemini")) provider = "gemini";
        else if (s.command?.includes("codex")) provider = "codex";
        else if (s.command === "wsl") provider = "wsl";
        return { ...s, provider };
      }),
    }));
    set({ workspaces: migrated, activeId, hydrated: true });
  },
  markHydrated: () => set({ hydrated: true }),
}));

