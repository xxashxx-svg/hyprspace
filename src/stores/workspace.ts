import { create } from "zustand";

export interface Session {
  id: string;
  title: string;
  command?: string;
  cwd?: string;
  // claude panes pin to their session id (id IS a uuid); once launched we resume it next time
  started?: boolean;
}

export interface Workspace {
  id: string;
  name: string;
  cwd: string;
  color: string;
  kind: "project" | "open";
  sessions: Session[];
  renamed?: boolean; // user renamed it → never auto-rename again
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
  setActive: (id: string) => void;
  reorderWorkspaces: (fromId: string, toId: string) => void;
  addSession: (wsId: string, command?: string, cwd?: string) => void;
  removeSession: (wsId: string, sessionId: string) => void;
  markStarted: (sessionId: string) => void;
  reorderSessions: (wsId: string, fromId: string, toId: string) => void;
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
      const title = command?.includes("claude") ? "Claude" : "Terminal";
      const workspaces = s.workspaces.map((w) => {
        if (w.id !== wsId) return w;
        const sessions = [...w.sessions, { id, title, command, cwd: cwd ?? w.cwd }];
        // auto-name an untouched open space after the folder it's working in (AI naming
        // can replace this heuristic later once accounts/an API key exist)
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

  // flips once a claude pane has launched, so the next launch resumes instead of starting fresh
  markStarted: (sessionId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => ({
        ...w,
        sessions: w.sessions.map((ss) =>
          ss.id === sessionId && !ss.started ? { ...ss, started: true } : ss,
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

  setFocused: (id) => set({ focusedSessionId: id }),

  hydrate: (workspaces, activeId) => set({ workspaces, activeId, hydrated: true }),
  markHydrated: () => set({ hydrated: true }),
}));
