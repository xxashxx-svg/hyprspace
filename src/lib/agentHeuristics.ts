// Agent state for CLIs that have no hooks (Codex, Gemini, OpenCode, Grok, plain shells).
// Claude reports through its hooks; everything else is read off the terminal itself: how recently
// it printed, and whether the last lines look like a question waiting for an answer.
import { recentOutput } from "../terminal/buffers";
import type { AgentState } from "../stores/agentStatus";

const BUSY_MS = 2500;

// prompts the TUIs print when they need a decision or an answer
const WAITING = [
  /\(y\/n\)|\[y\/n\]|\(yes\/no\)|\[yes\/no\]/i,
  /\b(allow|approve|permission|confirm|proceed|continue)\b.*\?/i,
  /press (enter|any key)/i,
  /\b(do you want|would you like|are you sure)\b/i,
  /\bwaiting for (your )?(input|approval|confirmation)\b/i,
];

export interface HeuristicState {
  state: AgentState | "exited";
  activity?: string;
}

/** What a pane without hooks looks like it is doing right now. */
export function heuristicState(sessionId: string, lastOut: number | undefined, exited: boolean, now = Date.now()): HeuristicState {
  if (exited) return { state: "exited", activity: "Process exited" };
  if (!lastOut) return { state: "idle" };
  const tail = recentOutput(sessionId, 600, false); // polled from render — never force a flush
  const lines = tail.split("\n").map((l) => l.trim()).filter(Boolean).slice(-4);
  const question = lines.find((l) => WAITING.some((re) => re.test(l)));
  if (question && now - lastOut > 800) return { state: "waiting", activity: question.slice(0, 70) };
  if (now - lastOut < BUSY_MS) return { state: "working" };
  return { state: "idle" };
}
