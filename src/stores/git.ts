import { create } from "zustand";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useWorkspaces } from "./workspace";
import { useNotifications } from "./notifications";
import { gitCommit, gitPush, gitCreatePr, gitPrDefaults, gitInit, gitInitRepo, type PrDefaults } from "../api";

export interface InitRepoOpts {
  name: string;
  branch: string;
  gitignore: string;
  readme: boolean;
  commit: boolean;
  commitMsg: string;
  github: boolean;
  private: boolean;
  description: string;
}

// the repo the topbar git actions operate on: the focused pane's folder, else the active space's
export function gitCwd(): string {
  const { workspaces, activeId, focusedSessionId } = useWorkspaces.getState();
  const ws = workspaces.find((w) => w.id === activeId);
  const focused = ws?.sessions.find((s) => s.id === focusedSessionId);
  return focused?.cwd || ws?.cwd || "";
}

const note = (title: string, body?: string) =>
  useNotifications.getState().add({ title, body, kind: "info" });

// a short-lived status shown on the topbar git button: "Pushing…" (busy), then "Pushed" (ok) / a
// failure (err), which auto-clears after a couple seconds so you actually see the action land.
export type GitActivity = { label: string; kind: "busy" | "ok" | "err" };
let clearTimer: ReturnType<typeof setTimeout> | undefined;
// clear a finished (ok/err) status after a beat, unless another op has since gone busy
function autoClearActivity(set: (fn: (s: GitState) => Partial<GitState>) => void) {
  clearTimeout(clearTimer);
  clearTimer = setTimeout(
    () => set((s) => (s.activity && s.activity.kind !== "busy" ? { activity: null } : {})),
    2400,
  );
}

interface GitState {
  dialogOpen: boolean;
  withPush: boolean;
  busy: boolean;
  activity: GitActivity | null;
  repoTick: number; // bump to re-check whether the active folder is a repo (e.g. after init)
  prOpen: boolean;
  prDefaults: PrDefaults | null; // null while loading
  prBusy: boolean;
  openCommit: (push: boolean) => void;
  close: () => void;
  commit: (message: string) => Promise<void>;
  push: () => Promise<void>;
  openPr: () => Promise<void>;
  closePr: () => void;
  createPr: (opts: { title: string; body: string; base: string; draft: boolean; push: boolean }) => Promise<void>;
  initOpen: boolean;
  initBusy: boolean;
  openInitRepo: () => void;
  closeInitRepo: () => void;
  runInitRepo: (opts: InitRepoOpts) => Promise<void>;
  init: () => Promise<void>;
}

export const useGit = create<GitState>((set, get) => ({
  dialogOpen: false,
  withPush: false,
  busy: false,
  activity: null,
  repoTick: 0,
  prOpen: false,
  prDefaults: null,
  prBusy: false,
  initOpen: false,
  initBusy: false,
  openCommit: (push) => set({ dialogOpen: true, withPush: push }),
  close: () => {
    if (!get().busy) set({ dialogOpen: false });
  },
  commit: async (message) => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    const push = get().withPush;
    set({ busy: true, activity: { label: push ? "Committing & pushing…" : "Committing…", kind: "busy" } });
    try {
      const res = await gitCommit(cwd, message, push, true);
      note(push ? "Committed & pushed" : "Committed", res);
      set({ dialogOpen: false, activity: { label: push ? "Committed & pushed" : "Committed", kind: "ok" } });
    } catch (e) {
      note("Commit failed", String(e));
      set({ activity: { label: "Commit failed", kind: "err" } });
    } finally {
      set({ busy: false });
      autoClearActivity(set);
    }
  },
  push: async () => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    set({ busy: true, activity: { label: "Pushing…", kind: "busy" } });
    try {
      const res = await gitPush(cwd);
      note("Pushed", res);
      set({ activity: { label: "Pushed", kind: "ok" } });
    } catch (e) {
      note("Push failed", String(e));
      set({ activity: { label: "Push failed", kind: "err" } });
    } finally {
      set({ busy: false });
      autoClearActivity(set);
    }
  },
  openPr: async () => {
    const cwd = gitCwd();
    if (!cwd) {
      note("No git folder", "Focus a pane inside a repo first.");
      return;
    }
    set({ prOpen: true, prDefaults: null });
    try {
      const d = await gitPrDefaults(cwd);
      set({ prDefaults: d });
    } catch (e) {
      set({ prOpen: false });
      note("Couldn't read the repo", String(e));
    }
  },
  closePr: () => {
    if (!get().prBusy) set({ prOpen: false });
  },
  createPr: async (opts) => {
    const cwd = gitCwd();
    if (!cwd || get().prBusy) return;
    set({ prBusy: true, activity: { label: "Opening PR…", kind: "busy" } });
    try {
      const url = await gitCreatePr({ cwd, ...opts });
      note("Pull request created", url);
      if (/^https?:\/\//.test(url)) void openUrl(url).catch(() => {});
      set({ prOpen: false, activity: { label: "PR opened", kind: "ok" } });
    } catch (e) {
      note("Create PR failed", String(e));
      set({ activity: { label: "Create PR failed", kind: "err" } });
    } finally {
      set({ prBusy: false });
      autoClearActivity(set);
    }
  },
  openInitRepo: () => {
    if (gitCwd()) set({ initOpen: true });
    else note("No folder", "Focus a pane in a folder first.");
  },
  closeInitRepo: () => {
    if (!get().initBusy) set({ initOpen: false });
  },
  runInitRepo: async (opts) => {
    const cwd = gitCwd();
    if (!cwd || get().initBusy) return;
    set({ initBusy: true, activity: { label: opts.github ? "Creating repo…" : "Initializing…", kind: "busy" } });
    try {
      const res = await gitInitRepo({ cwd, ...opts });
      note(opts.github ? "Repository created" : "Repository initialized", res);
      if (/^https?:\/\//.test(res)) void openUrl(res).catch(() => {});
      set((s) => ({ initOpen: false, repoTick: s.repoTick + 1, activity: { label: opts.github ? "Repo created" : "Initialized", kind: "ok" } }));
    } catch (e) {
      note("Couldn't initialize repo", String(e));
      set({ activity: { label: "Init failed", kind: "err" } });
    } finally {
      set({ initBusy: false });
      autoClearActivity(set);
    }
  },
  init: async () => {
    const cwd = gitCwd();
    if (!cwd || get().busy) return;
    set({ busy: true, activity: { label: "Initializing…", kind: "busy" } });
    try {
      const res = await gitInit(cwd);
      note("Repository initialized", res);
      set((s) => ({ repoTick: s.repoTick + 1, activity: { label: "Initialized", kind: "ok" } }));
    } catch (e) {
      note("Couldn't initialize repo", String(e));
      set({ activity: { label: "Init failed", kind: "err" } });
    } finally {
      set({ busy: false });
      autoClearActivity(set);
    }
  },
}));
