// The chat can operate HyprSpace: the model emits ```hyprspace fenced commands, and we run
// them against the stores (create projects/spaces, spawn agents). Kept in-process — no MCP
// bridge — so it's instant and fully under our control.
import { useWorkspaces } from "./workspace";
import { createProjectDir, getHomeDir } from "../api";
import { projectsBaseDir, joinPath } from "../lib/projects";
import { claudeCmd, geminiCmd, codexCmd, WSL_CMD } from "../actions";

export const ORCHESTRATOR_PREAMBLE = `You are the operator of HyprSpace, a multi-agent terminal workspace. In addition to answering normally, you can control the workspace. When the user asks you to create a project or open space, or to spawn agents/terminals, DO IT by emitting a fenced command block:

\`\`\`hyprspace
{"action":"...", ...}
\`\`\`

Available actions:
- {"action":"create_project","name":"<name>"} — make a project (created inside the user's Projects folder) and switch to it.
- {"action":"new_open_space","name":"<name>"} — make a scratch open space and switch to it.
- {"action":"spawn_agents","provider":"claude"|"gemini"|"codex"|"wsl"|"terminal","count":<n>} — open <n> panes in the current space.
- {"action":"switch_space","name":"<name>"} — switch to an existing space by name.

Guidelines:
- Say one short sentence about what you're doing, then emit the block(s). Chain steps by emitting several blocks in order (e.g. create_project then spawn_agents).
- Only use these for workspace actions. For coding help or normal questions, just answer — no block.`;

interface Cmd {
  action: string;
  [k: string]: unknown;
}
export interface ActionResult {
  ok: boolean;
  label: string;
  spaceId?: string;
}

// pull ```hyprspace fenced JSON blocks out of a chunk of assistant text
function parseCommands(text: string): { cmds: Cmd[]; stripped: string } {
  const cmds: Cmd[] = [];
  const stripped = text.replace(/```hyprspace\s*([\s\S]*?)```/g, (_m, body) => {
    try {
      const c = JSON.parse(String(body).trim());
      if (c && typeof c.action === "string") cmds.push(c);
    } catch {
      /* ignore malformed */
    }
    return "";
  });
  return { cmds, stripped: stripped.trim() };
}

const slug = (s: string) =>
  s.trim().replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-+|-+$/g, "") || "project";

let homeCache = "";
const ensureHome = async () => {
  if (!homeCache) homeCache = await getHomeDir().catch(() => "");
  return homeCache;
};

const PROVIDER_CMD: Record<string, () => string | undefined> = {
  claude: () => claudeCmd(),
  gemini: () => geminiCmd(),
  codex: () => codexCmd(),
  wsl: () => WSL_CMD,
  terminal: () => undefined,
};

async function executeCommand(c: Cmd): Promise<ActionResult> {
  const ws = useWorkspaces.getState();
  try {
    if (c.action === "create_project") {
      const name = String(c.name || "Project");
      // always confine chat-created projects to the projects folder. we IGNORE any model-supplied
      // path: slug() strips separators/".." so the name can't escape, and we double-check the result
      // stays under the base — stops a prompt-injected absolute path from creating folders anywhere.
      const base = await projectsBaseDir();
      const path = base ? joinPath(base, slug(name)) : slug(name);
      if (base && !path.startsWith(base)) {
        return { ok: false, label: "Refused: path outside the projects folder" };
      }
      await createProjectDir(path, null, null).catch(() => {});
      const id = ws.addWorkspace(name, path);
      return { ok: true, label: `Created project “${name}”`, spaceId: id };
    }
    if (c.action === "new_open_space") {
      const name = c.name ? String(c.name) : undefined;
      const id = ws.addOpenSpace(name);
      return { ok: true, label: `Created open space${name ? ` “${name}”` : ""}`, spaceId: id };
    }
    if (c.action === "spawn_agents") {
      const provider = String(c.provider || "claude");
      const count = Math.max(1, Math.min(8, Number(c.count) || 1));
      const build = PROVIDER_CMD[provider] ?? (() => undefined);
      const active = ws.workspaces.find((w) => w.id === ws.activeId);
      if (!active) return { ok: false, label: "No active space to spawn into" };
      const cwd = active.cwd || (await ensureHome()) || "";
      for (let i = 0; i < count; i++) ws.addSession(active.id, build(), cwd);
      const nm = provider.charAt(0).toUpperCase() + provider.slice(1);
      return {
        ok: true,
        label: `Spawned ${count} ${nm} ${count === 1 ? "pane" : "panes"}`,
        spaceId: active.id,
      };
    }
    if (c.action === "switch_space") {
      const want = String(c.name || "").toLowerCase();
      const target = ws.workspaces.find((w) => w.name.toLowerCase() === want);
      if (!target) return { ok: false, label: `No space named “${c.name}”` };
      ws.setActive(target.id);
      return { ok: true, label: `Switched to “${target.name}”`, spaceId: target.id };
    }
    return { ok: false, label: `Unknown action: ${c.action}` };
  } catch (e) {
    return { ok: false, label: `Failed: ${String(e)}` };
  }
}

// run any operator commands in a chunk of text; returns the text with blocks removed + results
export async function runOperatorText(
  text: string,
): Promise<{ stripped: string; actions: ActionResult[] }> {
  const { cmds, stripped } = parseCommands(text);
  const actions: ActionResult[] = [];
  for (const c of cmds) actions.push(await executeCommand(c));
  return { stripped, actions };
}
