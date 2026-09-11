// The composer's launch path. It is the terminal path, not an SDK: open a real pane, let the CLI
// boot the normal way, then type the prompt in as keystrokes. Nothing here touches a token.
import { useWorkspaces } from "../stores/workspace";
import { useUsage } from "../stores/usage";
import { useNotifications } from "../stores/notifications";
import { writePty } from "../api";

// Claude reports its status line once the TUI is on screen, which is the "ready" signal. Other
// CLIs give no signal, so they get a flat delay that covers their startup. A bare shell is ready
// almost as soon as the pane is.
const CLAUDE_BOOT_TIMEOUT_MS = 30_000;
const AGENT_BOOT_DELAY_MS = 4500;
const SHELL_BOOT_DELAY_MS = 1500;

const enc = new TextEncoder();

function paneExists(paneId: string): boolean {
  return useWorkspaces.getState().workspaces.some((w) => w.sessions.some((s) => s.id === paneId));
}

/**
 * The prompt with attached files after it, each path quoted when it holds a space so the CLI
 * reads it as one token. Agents open images and files by path, so this is all an attachment is.
 */
export function withFiles(prompt: string, paths: string[]): string {
  const quoted = paths.map((p) => (p.includes(" ") ? `"${p}"` : p));
  return [prompt.trim(), ...quoted].filter(Boolean).join(" ");
}

/** Type a prompt into a pane once the CLI launched by `cmd` is ready for input. */
export function queuePrompt(paneId: string, cmd: string | undefined, prompt: string): void {
  const task = prompt.replace(/\s+/g, " ").trim();
  if (!task) return;

  const type = () => {
    if (!paneExists(paneId)) return;
    void writePty(paneId, enc.encode(task));
    // Enter goes a beat later so the TUI settles the pasted text first
    setTimeout(() => void writePty(paneId, enc.encode("\r")), 300);
  };

  if (!cmd?.includes("claude")) {
    const isShell = !cmd || cmd === "wsl";
    setTimeout(type, isShell ? SHELL_BOOT_DELAY_MS : AGENT_BOOT_DELAY_MS);
    return;
  }

  let done = false;
  const unsub = useUsage.subscribe((s) => {
    if (done || !s.byPane[paneId]) return;
    done = true;
    unsub();
    clearTimeout(timer);
    type();
  });
  const timer = setTimeout(() => {
    if (done) return;
    done = true;
    unsub();
    useNotifications.getState().add({
      title: "Prompt not sent",
      body: "Claude did not start in that pane, so the prompt was not typed in:\n" + task,
    });
  }, CLAUDE_BOOT_TIMEOUT_MS);
}
