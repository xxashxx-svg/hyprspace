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
  gitBranchInfo,
  gitChanges,
  gitCommit,
  gitDiff,
  writePty,
} from "./api";
import { useWorkspaces } from "./stores/workspace";
import { useAgentStatus, displayState } from "./stores/agentStatus";
import { useUsage, summarize } from "./stores/usage";
import { useLoops } from "./stores/loops";
import { useBridge } from "./stores/bridge";
import { startLoop, stopLoop } from "./lib/automations";
import { claudeCmd, codexCmd, geminiCmd, opencodeCmd, grokCmd, WSL_CMD } from "./actions";

function snapshot() {
  const { workspaces, activeId, focusedSessionId, activatedIds } = useWorkspaces.getState();
  const agents = useAgentStatus.getState().byPane;
  const { loops, runs } = useLoops.getState();
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
        .filter((s) => !s.image && !s.file) // viewer tabs have no terminal to mirror
        .map((s) => {
          const a = agents[s.id];
          return {
            id: s.id,
            title: s.title,
            provider: s.provider,
            cwd: s.cwd ?? w.cwd,
            started: !!s.started,
            state: displayState(a, now),
            activity: a?.activity ?? null,
            subs: a?.subs.length ?? 0,
          };
        }),
    })),
    automations: Object.values(loops).map((l) => {
      const r = runs[l.id];
      return {
        id: l.id,
        name: l.name || "Untitled",
        mode: l.mode,
        enabled: l.enabled,
        folder: l.folder,
        status: r?.status ?? "idle",
        lastRunAt: r?.lastRunAt ?? null,
        nextRunAt: r?.nextRunAt ?? null,
        lastResult: r?.lastResult ?? null,
        wsId: r?.wsId ?? null,
        paneId: r?.paneId ?? null,
      };
    }),
    usage: summarize(useUsage.getState().byPane, now),
  };
}

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

    case "space.launch": {
      const id = str("ws");
      const space = ws().workspaces.find((w) => w.id === id);
      if (!space) throw new Error("no such space");
      const folder = str("cwd") || space.cwd;
      if (!folder) throw new Error("open spaces need a folder — launch this one from the desktop");
      const build = PROVIDER_CMD[str("provider")];
      if (!build) throw new Error("unknown provider");
      ws().activateWorkspace(id);
      return { pane: ws().addSession(id, build(), folder, { focus: false }) };
    }

    case "pane.close":
      ws().removeSession(str("ws"), str("pane"));
      return { ok: true };

    case "pane.prompt":
      await sendPrompt(str("pane"), str("text"));
      return { ok: true };

    case "git.changes":
      return { files: await gitChanges(str("cwd")) };

    case "git.diff":
      return { diff: await gitDiff(str("cwd"), str("path")) };

    case "git.branch":
      return await gitBranchInfo(str("cwd"));

    case "git.commit":
      return { out: await gitCommit(str("cwd"), str("message"), !!p.push, true) };

    case "automation.run":
      startLoop(str("id"));
      return { ok: true };

    case "automation.stop":
      stopLoop(str("id"));
      return { ok: true };

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
    useLoops.subscribe(schedule),
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
