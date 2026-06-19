import { create } from "zustand";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useWorkspaces } from "./workspace";
import { useNotifications } from "./notifications";
import { gitCommit, gitPush, gitCreatePr, gitInit } from "../api";

// the repo the topbar git actions operate on: the focused pane's folder, else the active space's
export function gitCwd(): string {
  const { workspaces, activeId, focusedSessionId } = useWorkspaces.getState();
  const ws = workspaces.find((w) => w.id === activeId);
  const focused = ws?.sessions.find((s) => s.id === focusedSessionId);
  return focused?.cwd || ws?.cwd || "";
}

const note = (title: string, body?: string) =>
  useNotifications.getState().add({ title, body, kind: "info" });

interface GitState {
  dialogOpen: boolean;
  withPush: boolean;
  busy: boolean;
  repoTick: number; // bump to re-check whether the active folder is a repo (e.g. after init)
  openCommit: (push: boolean) => void;
  close: () => void;
  commit: (message: string) => Promise<void>;
  push: () => Promise<void>;
  createPr: () => Promise<void>;
  init: () => Promise<void>;
}

export const useGit = create<GitState>((set, get) => ({
  dialogOpen: false,
  withPush: false,
  busy: false,
  repoTick: 0,
  openCommit: (push) => set({ dialogOpen: true, withPush: push }),
  close: () => {
    if (!get().busy) set({ dialogOpen: false });
  },
  commit: async (message) => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    const push = get().withPush;
    set({ busy: true });
    try {
      const res = await gitCommit(cwd, message, push, true);
      note(push ? "Committed & pushed" : "Committed", res);
      set({ dialogOpen: false });
    } catch (e) {
      note("Commit failed", String(e));
    } finally {
      set({ busy: false });
    }
  },
  push: async () => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    set({ busy: true });
    try {
      const res = await gitPush(cwd);
      note("Pushed", res);
    } catch (e) {
      note("Push failed", String(e));
    } finally {
      set({ busy: false });
    }
  },
  createPr: async () => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    set({ busy: true });
    try {
      const url = await gitCreatePr(cwd);
      note("Pull request created", url);
      if (/^https?:\/\//.test(url)) void openUrl(url).catch(() => {});
    } catch (e) {
      note("Create PR failed", String(e));
    } finally {
      set({ busy: false });
    }
  },
  init: async () => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    set({ busy: true });
    try {
      const res = await gitInit(cwd);
      note("Repository initialized", res);
      set((s) => ({ repoTick: s.repoTick + 1 }));
    } catch (e) {
      note("Couldn't initialize repo", String(e));
    } finally {
      set({ busy: false });
    }
  },
}));
