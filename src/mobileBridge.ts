// The app half of the mobile bridge: publish a state mirror to connected phones, and answer the
// requests they make. The Rust side (src-tauri/src/bridge.rs) owns the socket and knows nothing
// about spaces or panes — it just relays.
//
// Push, don't poll: every store change debounces into one `bridge_publish`, so the phone's list
// updates the moment the desktop's does. Requests that need the app (launch a pane, git, usage) come
// back as `bridge://req` and are answered here with the same api wrappers the UI uses.
import { listen } from "@tauri-apps/api/event";
import {
  bridgePublish,
  bridgeReply,
  createProjectDir,
  getHomeDir,
  gitBranchInfo,
  gitChanges,
  gitCommit,
  gitDiff,
  gitInit,
  gitIsRepo,
  listDir,
  writePty,
} from "./api";
import { DEFAULT_GITIGNORE, joinPath, parentOf, projectsBaseDir } from "./lib/projects";
import { useWorkspaces } from "./stores/workspace";
import { useAgentStatus, displayState } from "./stores/agentStatus";
import { useActivity } from "./stores/activity";
import { useUsage, useLiveUsage, summarize } from "./stores/usage";
import { useBridge } from "./stores/bridge";
import { claudeCmd, codexCmd, geminiCmd, opencodeCmd, grokCmd, WSL_CMD, commandFor, resumeCmd } from "./actions";
import { queuePrompt } from "./lib/composer";
import { CATALOG, modelLabel, type ProviderId } from "./lib/models";
import { PROVIDER_NAME } from "./lib/brand";
import { useProviders, catalogFor } from "./stores/providers";
import { agentSessions } from "./api";

/**
 * What the phone's Usage screen draws. The meter here prefers the live endpoint, which knows the
 * account's limits whether or not an agent is mid-turn, so send that when it has answered and fall
 * back to the status-line summary otherwise. The wire shape stays the same either way.
 */
function usagePayload(now: number) {
  const sum = summarize(useUsage.getState().byPane, now);
  const live = useLiveUsage.getState().claude;
  if (!live?.windows.length) return sum;
  const five = live.windows.find((w) => w.key === "session" || w.key === "five_hour");
  return {
    five: five?.win,
    others: live.windows.filter((w) => w !== five),
    // the endpoint never says which model is running; only the status line knows that
    models: sum?.models ?? [],
    at: live.updatedAt ?? now,
    stale: false,
  };
}

function snapshot() {
  const { workspaces, activeId, focusedSessionId, activatedIds } = useWorkspaces.getState();
  const agents = useAgentStatus.getState().byPane;
  const live = useUsage.getState().byPane;
  const lastOut = useActivity.getState().lastOut;
  const codexModels = useProviders.getState().codexModels;
  const now = Date.now();

  return {
    activeId,
    focusedId: focusedSessionId,
    spaces: workspaces.map((w) => ({
      id: w.id,
      name: w.name,
      kind: w.kind,
      cwd: w.cwd,
      color: w.color,
      // panes only exist as PTYs in a space that's been opened; the phone offers to wake the rest
      activated: activatedIds.includes(w.id),
      panes: w.sessions
        .filter((s) => !s.image && !s.file && !s.media && !s.diff && !s.draft) // viewer tabs and drafts have no terminal to mirror
        .map((s) => {
          const a = agents[s.id];
          // the model the CLI is actually on beats the one it was launched with
          const model = live[s.id]?.model ?? (s.model ? modelLabel(s.provider as ProviderId, s.model, catalogFor(s.provider as ProviderId, codexModels)) : "");
          return {
            id: s.id,
            title: s.title,
            provider: s.provider,
            cwd: s.cwd ?? w.cwd,
            started: !!s.started,
            state: displayState(a, now),
            activity: a?.activity ?? null,
            model,
            /** when this pane last printed anything, for the phone's relative stamp */
            at: lastOut[s.id] ?? 0,
            subs: a?.subs.length ?? 0,
            subAgents: (a?.subs ?? []).map((x) => ({ id: x.id, label: x.label, state: x.state, at: x.startedAt })),
          };
        }),
    })),
    automations: [], // the feature is gone; the phone app still expects the key
    usage: usagePayload(now),
  };
}

// same ceiling the desktop dialog uses per provider, so a bad payload can't spawn 500 PTYs
const MAX_PANES = 6;

const PROVIDER_CMD: Record<string, () => string | undefined> = {
  claude: claudeCmd,
  gemini: geminiCmd,
  codex: codexCmd,
  opencode: opencodeCmd,
  grok: grokCmd,
  wsl: () => WSL_CMD,
  terminal: () => undefined,
};

// Send a prompt to a pane: the text, then Enter as a separate write. The gap is load-bearing — an
// agent TUI needs a beat to settle the pasted block, and an Enter that arrives early submits an
// empty or truncated prompt. 300ms is what the automation engine already proved works against a
// real claude TUI (lib/automations.ts).
async function sendPrompt(pane: string, text: string) {
  const enc = new TextEncoder();
  await writePty(pane, enc.encode(text));
  await new Promise((r) => setTimeout(r, 300));
  await writePty(pane, enc.encode("\r"));
}

type Req = { peer: number; id: number; m: string; p: Record<string, unknown> };

async function handle(r: Req): Promise<unknown> {
  const p = r.p ?? {};
  const str = (k: string) => String(p[k] ?? "");
  const ws = () => useWorkspaces.getState();

  switch (r.m) {
    // mount a space's panes without stealing the desktop's view — the phone calls this before
    // subscribing, since an unopened space has no live PTYs to mirror
    case "space.activate":
      ws().activateWorkspace(str("ws"));
      return { ok: true };

    case "space.open":
      ws().setActive(str("ws"));
      return { ok: true };

    // Start a thread, the way the desktop composer does: build the launch command from the chosen
     // model and effort, then type the task in once the CLI is up. `resume` reopens a saved
     // conversation instead of starting a new one.
    case "space.launch": {
      const id = str("ws");
      const space = ws().workspaces.find((w) => w.id === id);
      if (!space) throw new Error("no such space");
      const folder = str("cwd") || space.cwd;
      if (!folder) throw new Error("this space has no folder yet. Pick one on the desktop first.");
      const provider = str("provider") as ProviderId;
      if (!PROVIDER_CMD[provider]) throw new Error("unknown agent");
      const choice = { model: str("model") || undefined, effort: str("effort") || undefined };
      const resume = str("resume");
      // only claude and codex can reopen a conversation by id; anything else just starts fresh
      const cmd = (resume ? resumeCmd(provider, resume, choice) : undefined) ?? commandFor(provider, choice);
      ws().activateWorkspace(id);
      const pane = ws().addSession(id, cmd, folder, { focus: false });
      if (str("model")) ws().setSessionModel(pane, str("model"));
      // queuePrompt waits for the CLI to actually be ready; sendPrompt's flat delay does not
      const text = str("prompt").trim();
      if (text) queuePrompt(pane, cmd, text);
      return { pane };
    }

    // what the phone's agent picker offers: the installed CLIs and the models each one has
    case "agents.catalog": {
      const inst = useProviders.getState().status;
      const codex = useProviders.getState().codexModels;
      return {
        agents: (Object.keys(CATALOG) as ProviderId[]).map((pid) => ({
          id: pid,
          label: PROVIDER_NAME[pid] ?? pid,
          installed: inst[pid]?.installed !== false,
          models: (catalogFor(pid, codex).models ?? []).map((m) => ({ id: m.id, label: m.label })),
          efforts: catalogFor(pid, codex).efforts ?? [],
        })),
      };
    }

    // saved conversations for a folder, so the phone can reopen one
    case "agent.sessions":
      return { sessions: await agentSessions(str("provider"), str("cwd")) };

    case "pane.close":
      ws().removeSession(str("ws"), str("pane"));
      return { ok: true };

    case "pane.prompt":
      await sendPrompt(str("pane"), str("text"));
      return { ok: true };

    // ---- project creation from the phone -------------------------------------------------------
    // The phone never does path math: it can't know whether this desktop uses \ or /, so it sends a
    // parent + a name and we join. Same reason browsing is a request rather than the phone guessing
    // what's above a folder.

    // sensible places to start browsing — home, and the configured projects folder
    case "fs.roots": {
      const [home, projects] = await Promise.all([
        getHomeDir().catch(() => ""),
        projectsBaseDir().catch(() => ""),
      ]);
      return { home, projects };
    }

    case "fs.browse": {
      const asked = str("path");
      const from = asked || (await projectsBaseDir().catch(() => "")) || (await getHomeDir());
      // descending is `into`, not a path the phone built: it can't know our separator
      let path = str("into") ? joinPath(from, str("into")) : from;
      // The projects folder is only created when the first project lands in it, so the default
      // often does not exist yet. Falling back to home beats handing the phone a bare "os error 3"
      // it can do nothing about. A folder the phone asked for by name still errors, as it should.
      const entries = await listDir(path).catch(async (e) => {
        if (asked || str("into")) throw e;
        path = await getHomeDir();
        return listDir(path);
      });
      return {
        path,
        parent: parentOf(path),
        // so the phone can PREVIEW "<folder><sep><name>" without guessing. Real joins still happen here.
        sep: path.includes("\\") ? "\\" : "/",
        // folders first, then files, each alphabetical — the phone renders the list as given
        entries: [...entries].sort((a, b) => Number(b.dir) - Number(a.dir) || a.name.localeCompare(b.name)),
      };
    }

    // does this folder already exist / is it already a repo? lets the phone warn before creating
    case "project.inspect": {
      const folder = str("folder") || joinPath(str("parent"), str("name").trim());
      if (!folder) throw new Error("no folder");
      const repo = await gitIsRepo(folder).catch(() => false);
      return { folder, repo };
    }

    // The desktop's New Project flow (NewProjectDialog.create), driven remotely. Deliberately the
    // same order — folder, workspace, git, panes — so a project made from the phone is
    // indistinguishable from one made here. The state push that follows is what syncs it back.
    case "project.create": {
      const name = str("name").trim();
      if (!name) throw new Error("a project needs a name");
      // `folder` for an existing folder, else parent + name for a fresh one
      const folder = str("folder") || joinPath(str("parent"), name);
      if (!folder) throw new Error("a project needs a folder");

      await createProjectDir(folder, p.readme ? `# ${name}\n` : null, p.gitignore ? DEFAULT_GITIGNORE : null);
      // `activate: false` so a project made from the phone doesn't yank the desktop away from
      // whatever it's showing — the same courtesy space.activate and addSession({focus:false})
      // already observe. It still appears in the rail instantly via the state push. Pass open:true
      // to deliberately switch the desktop's view.
      const id = ws().addWorkspace(name, folder, { activate: false });
      if (p.git) await gitInit(folder).catch(() => {});

      // panes: { claude: 2, terminal: 1, … } — same provider map space.launch uses
      const panes = (p.panes ?? {}) as Record<string, unknown>;
      let launched = 0;
      // PTYs only mount for an ACTIVATED space, so without this the panes would sit in state and
      // never start (addWorkspace sets activeId, which is a different thing)
      if (Object.values(panes).some((n) => (Number(n) || 0) > 0)) ws().activateWorkspace(id);
      for (const [prov, n] of Object.entries(panes)) {
        const build = PROVIDER_CMD[prov];
        if (!build) continue; // unknown provider from a newer phone build — skip, don't fail the whole create
        for (let i = 0; i < Math.min(Number(n) || 0, MAX_PANES); i++) {
          ws().addSession(id, build(), folder, { focus: false });
          launched++;
        }
      }
      if (p.open) ws().setActive(id);
      return { ws: id, folder, panes: launched };
    }

    case "git.changes":
      return { files: await gitChanges(str("cwd")) };

    case "git.diff":
      return { diff: await gitDiff(str("cwd"), str("path")) };

    case "git.branch":
      return await gitBranchInfo(str("cwd"));

    case "git.commit":
      return { out: await gitCommit(str("cwd"), str("message"), !!p.push, true) };

    // a full refresh, for a phone coming back from the background
    case "state":
      return snapshot();

    default:
      throw new Error(`unknown method: ${r.m}`);
  }
}

/** Start publishing + answering. Safe to call once per window; returns a disposer. */
export function initMobileBridge(): () => void {
  let lastSig = "";
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = () => {
    // nothing is listening while the bridge is off — don't build snapshots for no one
    if (!useBridge.getState().info?.running) return;
    const snap = snapshot();
    const sig = JSON.stringify(snap);
    if (sig === lastSig) return;
    lastSig = sig;
    void bridgePublish(JSON.stringify({ at: Date.now(), ...snap })).catch(() => {});
  };

  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(flush, 250);
  };

  const unsubs = [
    useWorkspaces.subscribe(schedule),
    useAgentStatus.subscribe(schedule),
    useUsage.subscribe(schedule),
    // a phone that connects while the desktop is idle still needs the current picture
    useBridge.subscribe(() => {
      lastSig = "";
      schedule();
    }),
  ];

  const unlistenP = listen<Req>("bridge://req", (e) => {
    const req = e.payload;
    handle(req).then(
      (data) => void bridgeReply(req.peer, req.id, true, data ?? null),
      (err) => void bridgeReply(req.peer, req.id, false, { error: String(err?.message ?? err) }),
    );
  });

  return () => {
    clearTimeout(timer);
    unsubs.forEach((u) => u());
    void unlistenP.then((u) => u());
  };
}
