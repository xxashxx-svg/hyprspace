// Per-pane task naming, T3-style: title a pane from the user's FIRST submitted prompt (their
// request) — captured from keystrokes, since our panes are raw terminals — and ask Codex (your free
// `codex login`) for a structured {"title"} which we validate. Leak-proof like ai/autoName.ts:
// single-flight, one call per pane, a `disabled` kill switch, and every codex run is agentStop()ed
// (agent.rs taskkills the tree + drops it from its process map) so nothing can orphan.
import { useEffect } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useSettings } from "../stores/settings";
import { agentStart, agentStop } from "../api";

const SCAN_MS = 6000; // backstop cadence to drain captured prompts the single-flight gate deferred
const HARD_MS = 25000; // absolute cap on one codex run — guarantees we always reap the process

// every control char is built via fromCharCode so no literal NUL/ESC/CR/LF bytes live in this source.
const ESC = String.fromCharCode(27);
const NUL = String.fromCharCode(0);
const CR = String.fromCharCode(13);
const LF = String.fromCharCode(10);
const BS = String.fromCharCode(8);
const DEL = String.fromCharCode(127);
const EXIT_MARK = NUL + "__agent_exit__"; // agent.rs sentinel: process ended
const INPUT_ESC = new RegExp(ESC + "\\[[0-9;?]*[~A-Za-z]", "g"); // input escapes: arrows, paste markers…
const ANSI = new RegExp(ESC + "\\[[0-9;?]*[a-zA-Z]", "g");
const CTRL = new RegExp("[" + NUL + "-" + String.fromCharCode(31) + "]", "g");
const TITLE_RE = new RegExp('"title"\\s*:\\s*"([^"]{2,60})"');
const strip = (s: string) => s.replace(ANSI, "").split(CR).join("");

const AGENT = new Set(["claude", "gemini", "codex", "opencode"]);

const typing = new Map<string, string>(); // sessionId -> accumulating first-line keystrokes
const pending = new Map<string, string>(); // sessionId -> captured first prompt, awaiting a name
const inflight = new Set<string>();
const namedOnce = new Set<string>(); // panes already titled (or attempted) — never re-probe
let active = 0; // single-flight: at most one codex naming process at a time
let disabled = false; // flips off for the session if `codex` isn't installed

// feed raw user keystrokes (xterm onData) so we can capture the FIRST submitted prompt for a pane.
export function noteUserInput(sessionId: string, data: string) {
  if (namedOnce.has(sessionId) || pending.has(sessionId)) return;
  if (!useSettings.getState().autoNameAgents) return;
  let buf = typing.get(sessionId) ?? "";
  for (const ch of data.replace(INPUT_ESC, "")) {
    if (ch === CR || ch === LF) {
      const msg = buf.replace(CTRL, "").trim();
      buf = "";
      if (msg.length >= 3 && msg[0] !== "/") {
        // a real first prompt (skip empty lines and slash commands like /init, /help)
        typing.delete(sessionId);
        pending.set(sessionId, msg.slice(0, 400));
        void drain();
        return;
      }
      // otherwise keep waiting for the real first prompt
    } else if (ch === DEL || ch === BS) {
      buf = buf.slice(0, -1); // backspace
    } else if (ch >= " ") {
      buf += ch; // printable
    }
  }
  typing.set(sessionId, buf);
}

// drop a closed pane's capture state (keep namedOnce so its id can't be re-probed)
export function forgetSession(sessionId: string) {
  typing.delete(sessionId);
  pending.delete(sessionId);
}

function findSession(id: string) {
  for (const w of useWorkspaces.getState().workspaces) {
    const s = w.sessions.find((x) => x.id === id);
    if (s) return s;
  }
  return undefined;
}

function buildPrompt(message: string): string {
  return `You write concise titles for coding sessions. Reply with ONLY a JSON object: {"title": "..."}.
Rules:
- Summarize the user's request below, do not restate it verbatim.
- Short and specific, 3 to 6 words, Title Case.
- No quotes inside the title, no trailing punctuation.

USER REQUEST:
${message}`;
}

// pull a title out of codex's output: prefer the structured {"title": "..."}, but fall back to the
// last short clean line (codex exec doesn't always emit pure JSON).
function parseTitle(raw: string): string | null {
  const clean = strip(raw);
  const m = clean.match(TITLE_RE);
  if (m) {
    const t = m[1].replace(CTRL, " ").trim().slice(0, 40);
    if (t.length >= 2 && /[a-z]/i.test(t)) return t;
  }
  const lines = clean
    .split("\n")
    .map((l) => l.replace(CTRL, " ").trim())
    .filter(Boolean);
  for (let i = lines.length - 1; i >= 0; i--) {
    if (/error|not logged|usage:|panic|enoent|sandbox|reasoning|tokens?|^codex|exec /i.test(lines[i])) continue;
    const words = lines[i].replace(/[^a-zA-Z0-9 ]+/g, " ").trim().split(/\s+/).filter(Boolean);
    if (words.length >= 1 && words.length <= 6) {
      const t = words.slice(0, 5).join(" ").slice(0, 40);
      if (t.length >= 3 && /[a-z]/i.test(t)) return t;
    }
  }
  return null;
}

// ONE codex exec; finish on the exit sentinel or a hard-timeout; ALWAYS agentStop (the anti-leak rule)
function codexName(runId: string, cwd: string, prompt: string): Promise<string> {
  return new Promise((resolve) => {
    let buf = "";
    let done = false;
    const finish = () => {
      if (done) return;
      done = true;
      clearTimeout(hard);
      void agentStop(runId).catch(() => {});
      resolve(buf);
    };
    const hard = setTimeout(finish, HARD_MS);
    const args = ["codex", "exec", "-s", "read-only", "--skip-git-repo-check"];
    void agentStart(runId, cwd || ".", args, {}, {}, prompt, (line) => {
      if (line === EXIT_MARK) {
        finish();
        return;
      }
      buf += line + "\n";
    }).catch((e) => {
      const m = String(e).toLowerCase();
      if (m.includes("not found") || m.includes("enoent")) disabled = true; // no codex → stop probing
      finish();
    });
  });
}

// name the next pending pane (single-flight). called on capture and on the scanner tick.
async function drain() {
  if (active > 0 || disabled || !useSettings.getState().autoNameAgents) return;
  let pick: { id: string; msg: string; cwd: string; title: string } | undefined;
  for (const [id, msg] of pending) {
    if (namedOnce.has(id) || inflight.has(id)) continue;
    const sess = findSession(id);
    if (!sess || !AGENT.has(sess.provider)) {
      pending.delete(id); // pane closed or isn't an agent
      continue;
    }
    pick = { id, msg, cwd: sess.cwd ?? "", title: sess.title };
    break;
  }
  if (!pick) return;

  inflight.add(pick.id);
  active++;
  try {
    const raw = await codexName(`name-${pick.id}`, pick.cwd, buildPrompt(pick.msg));
    const title = parseTitle(raw);
    if (title && findSession(pick.id) && title.toLowerCase() !== pick.title.toLowerCase()) {
      useWorkspaces.getState().renameSession(pick.id, title);
    }
  } catch {
    /* transient — one attempt per pane */
  } finally {
    namedOnce.add(pick.id);
    pending.delete(pick.id);
    inflight.delete(pick.id);
    active--;
    if (pending.size > 0) void drain(); // keep draining the queue, still single-flight
  }
}

// periodic backstop: drains any captured prompts the single-flight gate deferred while busy
export function useSessionNamer() {
  useEffect(() => {
    const iv = setInterval(() => void drain(), SCAN_MS);
    return () => clearInterval(iv);
  }, []);
}
