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
      workspaces: s.workspaces.map((w) => (w.id === id ? { ...w, name } : w)),
    })),

  setActive: (id) => set({ activeId: id }),

  addSession: (wsId, command, cwd) =>
    set((s) => {
      const id = uid();
      const title = command?.includes("claude") ? "Claude" : "Terminal";
      return {
        workspaces: s.workspaces.map((w) =>
          w.id === wsId
            ? { ...w, sessions: [...w.sessions, { id, title, command, cwd: cwd ?? w.cwd }] }
            : w,
        ),
        focusedSessionId: id,
      };
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
