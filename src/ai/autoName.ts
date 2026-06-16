// AI auto-naming for open spaces: peek at what the terminals are actually doing and title
// the space after it (like ChatGPT/Claude naming a conversation). Runs the local `claude`
// CLI headlessly via the backend — no API key, since HyprSpace already runs claude in panes.
//
// Layering: addSession gives a space an instant folder-name placeholder; this upgrades it to
// a real title once there's genuine activity; a manual rename locks both out for good.
//
// A fresh terminal is mostly Claude Code's own chrome — the welcome banner, "what's new" notes,
// plugin/hook/MCP setup, slash commands — so we strip that out and only call the model once
// there's real work to name, always handing it the folder name as the safe default.
//
// Naming is SINGLE-FLIGHT: each `claude -p` is a heavyweight process, so we never run more than
// one at a time (and the backend command is async, so it can't stall the UI thread either).
import { useEffect } from "react";
import { useWorkspaces } from "../stores/workspace";
import { recentOutput } from "../terminal/buffers";
import { aiNameSpace } from "../api";

const MIN_REAL = 400; // chars of *non-chrome* output before naming is worth a model call
const RETRY_MS = 20000; // don't re-probe the same space more often than this
const SCAN_MS = 6000;

const inflight = new Set<string>();
const lastTried = new Map<string, number>();
let active = 0; // how many naming calls are running right now (single-flight gate)
let disabled = false; // flips off for the session if `claude` isn't installed

// lines that are tool/shell chrome, not the user's work
const NOISE = [
  /claude code v\d/i,
  /welcome back/i,
  /tips for getting started/i,
  /setup issues?:/i,
  /resume cancelled/i,
  /reloaded:\s*\d+/i,
  /operation blocked by hook/i,
  /original prompt:/i,
  /claude-mem|worker-service|bun-runner|CLAUDE_PLUGIN_ROOT|CLAUDE_CONFIG_DIR|PLUGIN_ROOT/i,
  /auto mode on|shift\+tab to cycle|↵ for agents|for agents$/i,
  /opus [\d.]+ \(1m context\)|\[opus|\[sonnet|\[haiku|\[claude/i,
  /'s organization/i,
  /windows powershell|copyright \(c\) microsoft|try the new cross-platform|aka\.ms\/pscore6/i,
  /^[\s│|`└├─.>*+-]*$/, // box-drawing / blank / lone prompt-arrow lines
  /^\s*\/(plugin|reload-plugins|resume|init|doctor|model|status|clear|help|mcp|cost|login)\b/i,
  /\/(release-notes|doctor)\b/i,
];

// strip startup/plugin/shell chrome so naming reflects actual work, not the banner
function stripChrome(text: string): string {
  const out: string[] = [];
  let skip = 0;
  for (const line of text.split("\n")) {
    if (skip > 0) {
      skip--;
      continue;
    }
    if (/what's new/i.test(line)) {
      skip = 6; // the release-notes block that follows the header
      continue;
    }
    if (NOISE.some((re) => re.test(line))) continue;
    out.push(line);
  }
  return out.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

function buildPrompt(folder: string, activity: string): string {
  return `You are titling a workspace tab in a developer tool. Reply with ONLY a 2 to 4 word title in Title Case — no quotes, no punctuation, no explanation.

The project folder is named "${folder}". A clean, readable version of the folder name is usually the best title (e.g. "magic-subtitle" -> "Magic Subtitle", "cold-email-outreach" -> "Cold Email Outreach").

Below is recent terminal activity. IGNORE tool startup banners, "what's new" / release notes, plugin / hook / MCP / LSP setup messages, and slash commands like /init, /resume, /plugin, /reload-plugins — none of that is the user's work. Only title a specific task if the activity clearly shows real work on that task. When in doubt, title the folder name.

TERMINAL ACTIVITY:
${activity || "(nothing meaningful yet)"}`;
}

function folderOf(ws: { name: string; sessions: { cwd?: string }[] }): string {
  const cwd = ws.sessions.map((s) => s.cwd).find((c) => c && c.trim());
  return cwd?.split(/[\\/]/).filter(Boolean).pop() ?? ws.name;
}

type Ws = ReturnType<typeof useWorkspaces.getState>["workspaces"][number];

async function runNaming(wsId: string, ws: Ws, activity: string, force: boolean) {
  inflight.add(wsId);
  active++;
  lastTried.set(wsId, Date.now());
  try {
    const name = (await aiNameSpace(buildPrompt(folderOf(ws), activity))).trim();
    // re-check guards — the user may have renamed it while we were waiting on claude
    const cur = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
    if (name && cur && !cur.renamed && (force || !cur.aiNamed)) {
      useWorkspaces.getState().autoNameWorkspace(wsId, name);
    }
  } catch (e) {
    const msg = String(e).toLowerCase();
    if (msg.includes("not found") || msg.includes("enoent")) disabled = true; // no claude → stop probing
    // anything else is transient — leave it for the next scan
  } finally {
    inflight.delete(wsId);
    active--;
  }
}

export async function maybeAutoName(wsId: string) {
  if (disabled || active > 0) return; // single-flight: one naming process at a time
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws || ws.kind !== "open" || ws.renamed || ws.aiNamed) return;
  if (inflight.has(wsId)) return;
  if (Date.now() - (lastTried.get(wsId) ?? 0) < RETRY_MS) return;

  const activity = stripChrome(ws.sessions.map((s) => recentOutput(s.id, 3000)).join("\n\n"));
  if (activity.length < MIN_REAL) return; // only boot chrome so far → keep the folder placeholder
  await runNaming(wsId, ws, activity, false);
}

// user-triggered ("Rename with AI"): always asks the model, even with thin context and even if
// it's already been auto-named — folder hint keeps it sensible. Only a manual rename outranks it.
export async function forceAutoName(wsId: string) {
  disabled = false; // user explicitly asked, so give claude another shot even if a probe failed before
  const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
  if (!ws || ws.kind !== "open" || ws.renamed || inflight.has(wsId)) return;
  const activity = stripChrome(ws.sessions.map((s) => recentOutput(s.id, 3000)).join("\n\n"));
  await runNaming(wsId, ws, activity, true);
}

// periodically nudge ONE open space that still needs a name; idle once everything's named
export function useAutoNamer() {
  useEffect(() => {
    const tick = () => {
      if (active > 0) return;
      const next = useWorkspaces
        .getState()
        .workspaces.find((w) => w.kind === "open" && !w.renamed && !w.aiNamed);
      if (next) void maybeAutoName(next.id);
    };
    const iv = setInterval(tick, SCAN_MS);
    return () => clearInterval(iv);
  }, []);
}
