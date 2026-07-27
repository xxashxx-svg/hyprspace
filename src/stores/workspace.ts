import { create } from "zustand";

const AGENT_LABEL: Record<string, string> = {
  claude: "Claude",
  gemini: "Gemini",
  codex: "Codex",
  opencode: "OpenCode",
  grok: "Grok",
};

export interface Session {
  id: string;
  title: string;
  command?: string;
  cwd?: string;
  provider: "claude" | "gemini" | "codex" | "opencode" | "grok" | "wsl" | "terminal" | "image" | "editor";
  // set → this tab is an image viewer holding that file path (not a terminal)
  image?: string;
  // set → this tab is a code editor on that file (not a terminal)
  file?: string;
  // claude panes pin to their session id (id IS a uuid); once launched we resume it next time
  started?: boolean;
  // the claude conversation this pane is currently on — starts as `id`, but follows a manual
  // /resume so the right chat comes back next launch
  claudeSessionId?: string;
  // sessions sharing a group id stack as tabs in one grid slot (opt-in via the pane + button)
  group?: string;
  // an automation's run pane: mounts even in a space you haven't opened, and is never persisted
  // (a saved one would relaunch its agent on the next app start)
  ephemeral?: boolean;
}

// which viewer a path opens in — images get the image viewer, everything else the code editor
const IMAGE_EXT = /\.(?:png|jpe?g|gif|webp|bmp|svg|ico|avif)$/i;

export interface Workspace {
  id: string;
  name: string;
  cwd: string;
  color: string;
  kind: "project" | "open";
  sessions: Session[];
  renamed?: boolean; // user renamed it → never auto-rename again
  aiNamed?: boolean; // AI already titled it → don't re-title (a manual rename still wins)
  layouts?: Record<number, string>; // chosen pane-layout preset id per pane-count (else auto/default)
  activeTabByGroup?: Record<string, string>; // group id → the tab (session id) shown in that slot
  lastOpenedAt?: number; // when you last entered it — home's "Continue" ordering
}

const COLORS = ["#3fb6e0", "#46c98a", "#e0a23f", "#b06ae0", "#e5484d", "#7dc4e8"];
const uid = () => crypto.randomUUID();

export type Slot = { sessions: Session[]; group?: string };

// group a space's sessions into ordered grid slots: an ungrouped session is its own solo slot;
// sessions sharing a group id collapse into one slot (created at the group's first appearance).
export function toSlots(sessions: Session[]): Slot[] {
  const slots: Slot[] = [];
  const byGroup = new Map<string, Slot>();
  for (const sess of sessions) {
    if (!sess.group) {
      slots.push({ sessions: [sess] });
      continue;
    }
    let slot = byGroup.get(sess.group);
    if (!slot) {
      slot = { sessions: [], group: sess.group };
      byGroup.set(sess.group, slot);
      slots.push(slot);
    }
    slot.sessions.push(sess);
  }
  return slots;
}

// derive a pane's provider + starting title from its launch command (shared by addSession/addTab)
function deriveProviderTitle(
  command: string | undefined,
  effCwd: string,
  used: Set<string>,
): { provider: Session["provider"]; title: string } {
  let provider: Session["provider"] = "terminal";
  let title = "Terminal";
  if (command?.includes("opencode")) provider = "opencode";
  else if (command?.includes("claude")) provider = "claude";
  else if (command?.includes("gemini")) provider = "gemini";
  else if (command?.includes("codex")) provider = "codex";
  else if (command?.includes("grok")) provider = "grok";
  else if (command === "wsl") {
    provider = "wsl";
    title = "WSL";
  }
  // agent panes are named after their working folder so you can tell them apart at a glance.
  // duplicates get a plain " 2", " 3" suffix — this used to invent human names (Gus, Wynn, Theo…)
  // which just read as random. no folder (an open space) falls back to the agent's own name.
  if (provider === "claude" || provider === "gemini" || provider === "codex" || provider === "opencode" || provider === "grok") {
    const folder = effCwd.split(/[\\/]/).filter(Boolean).pop();
    const base = folder || AGENT_LABEL[provider] || "Agent";
    title = base;
    for (let i = 2; used.has(title); i++) title = `${base} ${i}`;
  }
  return { provider, title };
}

// remove a session from a space and repair its tab group: dissolve the group down to a plain solo
// pane when ≤1 member is left, else re-point the slot's active tab to a surviving sibling.
function stripSession(w: Workspace, sessionId: string): Workspace {
  const removed = w.sessions.find((ss) => ss.id === sessionId);
  let sessions = w.sessions.filter((ss) => ss.id !== sessionId);
  const group = removed?.group;
  if (!group) return { ...w, sessions };
  const siblings = w.sessions.filter((ss) => ss.group === group); // still includes the removed one
  const members = sessions.filter((ss) => ss.group === group);
  const activeTabByGroup = { ...(w.activeTabByGroup ?? {}) };
  if (members.length <= 1) {
    // keep the survivor's `group` — a 1-member group already renders untabbed, and clearing it would
    // change the slot's react key and remount (= kill) the surviving pane's PTY
    delete activeTabByGroup[group];
  } else if (activeTabByGroup[group] === sessionId) {
    const idx = siblings.findIndex((ss) => ss.id === sessionId);
    activeTabByGroup[group] = (siblings[idx - 1] ?? siblings[idx + 1]).id; // prev sibling, else next
  }
  return { ...w, sessions, activeTabByGroup };
}

interface WorkspaceState {
  workspaces: Workspace[];
  activeId: string | null;
  focusedSessionId: string | null;
  hydrated: boolean;
  /** spaces whose panes are mounted. Lazy by design — a dozen unopened spaces would spawn many GB
   *  of agent processes at startup — so anything that needs a space's panes running (opening it, or
   *  an automation firing into it) has to say so explicitly. An array, not a Set: selectors must
   *  return a stable reference or the grid re-renders forever. */
  activatedIds: string[];
  activateWorkspace: (id: string) => void;
  /** `activate: false` adds it without making it the active space — used by automations */
  addWorkspace: (name?: string, cwd?: string, opts?: { activate?: boolean }) => string;
  addOpenSpace: (name?: string) => string;
  removeWorkspace: (id: string) => void;
  renameWorkspace: (id: string, name: string) => void;
  autoNameWorkspace: (id: string, name: string) => void;
  setActive: (id: string) => void;
  setLayout: (id: string, count: number, presetId: string) => void;
  reorderWorkspaces: (fromId: string, toId: string) => void;
  /** returns the new pane's id. `focus: false` launches it without stealing the view — used by
   *  automations, which must never yank you out of what you're doing. */
  addSession: (wsId: string, command?: string, cwd?: string, opts?: { focus?: boolean; ephemeral?: boolean }) => string;
  /** stack a pane as a tab in the anchor's slot. returns the new pane's id; `focus: false` leaves
   *  the slot showing whatever it was showing — used by automations. */
  addTab: (wsId: string, anchorSessionId: string, command?: string, cwd?: string, opts?: { focus?: boolean; ephemeral?: boolean }) => string;
  // open a file as a viewer tab (image → image viewer, anything else → code editor). `anchor` pins
  // it into a specific pane's slot (ctrl+click in a terminal); without one it lands on the focused
  // pane of the active space.
  openPathTab: (path: string, anchor?: { wsId: string; sessionId: string }) => void;
  setActiveTab: (wsId: string, group: string, sessionId: string) => void;
  renameSession: (sessionId: string, title: string) => void;
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
  activatedIds: [],
  focusedSessionId: null,
  hydrated: false,

  addWorkspace: (name, cwd = "", opts) => {
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
      return {
        workspaces: [...s.workspaces, ws],
        ...(opts?.activate === false ? {} : { activeId: id }),
      };
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

  activateWorkspace: (id) =>
    set((s) => (s.activatedIds.includes(id) ? {} : { activatedIds: [...s.activatedIds, id] })),

  setActive: (id) =>
    set((s) => ({
      activeId: id,
      activatedIds: s.activatedIds.includes(id) ? s.activatedIds : [...s.activatedIds, id],
      // stamp it so home can offer the spaces you were actually just in
      workspaces: s.workspaces.map((w) => (w.id === id ? { ...w, lastOpenedAt: Date.now() } : w)),
    })),

  // remember the chosen pane-layout preset for this space at this pane-count
  setLayout: (id, count, presetId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) =>
        w.id === id ? { ...w, layouts: { ...(w.layouts ?? {}), [count]: presetId } } : w,
      ),
    })),

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

  addSession: (wsId, command, cwd, opts) => {
    const id = uid();
    set((s) => {
      const effCwd = cwd ?? s.workspaces.find((w) => w.id === wsId)?.cwd ?? "";
      const used = new Set(s.workspaces.flatMap((w) => w.sessions.map((ss) => ss.title)));
      const { provider, title } = deriveProviderTitle(command, effCwd, used);

      const workspaces = s.workspaces.map((w) => {
        if (w.id !== wsId) return w;
        const sessions = [
          ...w.sessions,
          { id, title, command, cwd: cwd ?? w.cwd, provider, ...(opts?.ephemeral ? { ephemeral: true } : {}) },
        ];
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
      return { workspaces, focusedSessionId: opts?.focus === false ? s.focusedSessionId : id };
    });
    return id;
  },

  // open a new pane stacked as a tab in the anchor's slot, inheriting its folder. if the anchor
  // isn't grouped yet we mint a group id and stamp it on BOTH panes so they form the group together.
  addTab: (wsId, anchorSessionId, command, cwd, opts) => {
    const id = uid();
    set((s) => {
      const w = s.workspaces.find((x) => x.id === wsId);
      const anchor = w?.sessions.find((ss) => ss.id === anchorSessionId);
      if (!w || !anchor) return {};
      const group = anchor.group ?? anchor.id; // group id == the anchor session id, so the slot's react key never changes when it goes solo->tabbed or when the first tab is closed
      const effCwd = cwd ?? anchor.cwd ?? w.cwd ?? "";
      const used = new Set(s.workspaces.flatMap((x) => x.sessions.map((ss) => ss.title)));
      const { provider, title } = deriveProviderTitle(command, effCwd, used);
      const tab: Session = { id, title, command, cwd: effCwd, provider, group, ...(opts?.ephemeral ? { ephemeral: true } : {}) };
      const workspaces = s.workspaces.map((x) => {
        if (x.id !== wsId) return x;
        const sessions = [...x.sessions];
        const ai = sessions.findIndex((ss) => ss.id === anchorSessionId);
        if (!anchor.group) sessions[ai] = { ...sessions[ai], group }; // pull the anchor into the group
        sessions.splice(ai + 1, 0, tab); // insert right after the anchor
        // a background tab must not become the slot's visible one
        const activeTabByGroup =
          opts?.focus === false
            ? (x.activeTabByGroup ?? {})
            : { ...(x.activeTabByGroup ?? {}), [group]: id };
        return { ...x, sessions, activeTabByGroup };
      });
      return { workspaces, focusedSessionId: opts?.focus === false ? s.focusedSessionId : id };
    });
    return id;
  },

  // ctrl+click an image path in a terminal → open it as an image-viewer tab in that pane's slot.
  // same group logic as addTab; dedupes on the image path so the same file doesn't stack twice.
  openPathTab: (path, anchor) =>
    set((s) => {
      const w = anchor
        ? s.workspaces.find((x) => x.id === anchor.wsId)
        : s.workspaces.find((x) => x.id === s.activeId);
      if (!w) return {};
      // anchor on the given pane, else the focused one, else the first — that's whose slot it joins
      const anchorSess =
        (anchor && w.sessions.find((ss) => ss.id === anchor.sessionId)) ??
        w.sessions.find((ss) => ss.id === s.focusedSessionId) ??
        w.sessions[0];
      const isImg = IMAGE_EXT.test(path);
      const title = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
      const mk = (group?: string): Session => ({
        id: uid(),
        title,
        cwd: anchorSess?.cwd ?? w.cwd,
        provider: isImg ? "image" : "editor",
        ...(isImg ? { image: path } : { file: path }),
        group,
      });

      // nothing to anchor to (empty space) — the viewer becomes its own solo pane
      if (!anchorSess) {
        const tab = mk();
        return {
          workspaces: s.workspaces.map((x) => (x.id === w.id ? { ...x, sessions: [...x.sessions, tab] } : x)),
          focusedSessionId: tab.id,
        };
      }

      const group = anchorSess.group ?? anchorSess.id; // group id == anchor session id, so the slot's react key never changes
      // already open in this slot → just focus it instead of stacking a duplicate
      const dupe = w.sessions.find((ss) => ss.group === group && (ss.image ?? ss.file) === path);
      if (dupe) {
        return {
          workspaces: s.workspaces.map((x) =>
            x.id === w.id
              ? { ...x, activeTabByGroup: { ...(x.activeTabByGroup ?? {}), [group]: dupe.id } }
              : x,
          ),
          focusedSessionId: dupe.id,
        };
      }
      const tab = mk(group);
      const workspaces = s.workspaces.map((x) => {
        if (x.id !== w.id) return x;
        const sessions = [...x.sessions];
        const ai = sessions.findIndex((ss) => ss.id === anchorSess.id);
        if (!anchorSess.group) sessions[ai] = { ...sessions[ai], group }; // pull the anchor into the group
        sessions.splice(ai + 1, 0, tab);
        return { ...x, sessions, activeTabByGroup: { ...(x.activeTabByGroup ?? {}), [group]: tab.id } };
      });
      return { workspaces, focusedSessionId: tab.id };
    }),

  setActiveTab: (wsId, group, sessionId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) =>
        w.id === wsId
          ? { ...w, activeTabByGroup: { ...(w.activeTabByGroup ?? {}), [group]: sessionId } }
          : w,
      ),
      focusedSessionId: sessionId,
    })),

  // set a pane's title (used by the Codex task-namer to upgrade the folder placeholder)
  renameSession: (sessionId, title) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => ({
        ...w,
        sessions: w.sessions.map((ss) => (ss.id === sessionId ? { ...ss, title } : ss)),
      })),
    })),

  removeSession: (wsId, sessionId) =>
    set((s) => ({
      workspaces: s.workspaces.map((w) => (w.id === wsId ? stripSession(w, sessionId) : w)),
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
        return stripSession(w, sessionId); // repairs the source group if it was tabbed
      });
      if (!moved) return {};
      const guest = { ...moved, group: undefined }; // lands as a plain solo pane in the destination
      return {
        workspaces: stripped.map((w) =>
          w.id === toWsId ? { ...w, sessions: [...w.sessions, guest] } : w,
        ),
      };
    }),

  // focusing a session that's a HIDDEN tab has to reveal it too, or the rail/palette/cycle-pane
  // click looks like it did nothing. no-op for a visible pane (the group already points at it).
  setFocused: (id) =>
    set((s) => {
      const w = s.workspaces.find((x) => x.sessions.some((ss) => ss.id === id));
      const me = w?.sessions.find((ss) => ss.id === id);
      if (!w || !me?.group || w.activeTabByGroup?.[me.group] === id) return { focusedSessionId: id };
      return {
        focusedSessionId: id,
        workspaces: s.workspaces.map((x) =>
          x.id === w.id
            ? { ...x, activeTabByGroup: { ...(x.activeTabByGroup ?? {}), [me.group!]: id } }
            : x,
        ),
      };
    }),

  hydrate: (workspaces, activeId) => {
    // Migration: ensure every session has a 'provider' field (older saves won't have it)
    const migrated = workspaces.map((w) => ({
      ...w,
      sessions: w.sessions.map((s) => {
        if (s.provider) return s;
        let provider: Session["provider"] = "terminal";
        if (s.command?.includes("opencode")) provider = "opencode";
        else if (s.command?.includes("claude")) provider = "claude";
        else if (s.command?.includes("gemini")) provider = "gemini";
        else if (s.command?.includes("codex")) provider = "codex";
        else if (s.command?.includes("grok")) provider = "grok";
        else if (s.command === "wsl") provider = "wsl";
        return { ...s, provider };
      }),
    }));
    // Migration: panes we'd previously auto-titled from the old friendly-name pool (Gus, Wynn, …)
    // get renamed to their folder. Only exact pool matches are touched, so a name you typed
    // yourself — or one the task-namer produced — is left alone.
    const RETIRED_NAMES = new Set(
      ("Gus Wynn Theo Remy Enzo Dara Zoe Faye Otto Ivy Cleo Cy Nico Juno Knox Nell Vera Milo Lena " +
        "Rex Iris Hugo Maya Finn Ada Leo Nina Kai Ruby Sage").split(" "),
    );
    const renamed = migrated.map((w) => {
      const used = new Set<string>();
      return {
        ...w,
        sessions: w.sessions.map((s) => {
          const stem = s.title?.replace(/ \d+$/, "") ?? "";
          // generic = an old pool name, or the bare agent name ("Claude", "Claude 2"). Both tell you
          // nothing the icon doesn't, and leaving them means the strip prints a name AND a folder.
          const generic = RETIRED_NAMES.has(stem) || AGENT_LABEL[s.provider] === stem;
          if (!generic) {
            used.add(s.title);
            return s;
          }
          const folder = (s.cwd ?? w.cwd ?? "").split(/[\\/]/).filter(Boolean).pop();
          const base = folder || AGENT_LABEL[s.provider] || s.title;
          let next = base;
          for (let i = 2; used.has(next); i++) next = `${base} ${i}`;
          used.add(next);
          return { ...s, title: next };
        }),
      };
    });
    set({ workspaces: renamed, activeId, hydrated: true });
  },
  markHydrated: () => set({ hydrated: true }),
}));

