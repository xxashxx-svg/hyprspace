import { create } from "zustand";
import { chatStart, chatTurn, chatStop, saveState, loadState } from "../api";
import { useSettings } from "./settings";
import { ORCHESTRATOR_PREAMBLE, runOperatorText } from "./orchestrator";

export type Block =
  | { kind: "text"; text: string }
  | { kind: "tool"; id: string; name: string; input: unknown; result?: string; _pj?: string }
  | { kind: "action"; label: string; ok: boolean; spaceId?: string };

export interface ChatMsg {
  id: string;
  role: "user" | "assistant";
  blocks: Block[];
  error?: string;
  usage?: { out: number; cost: number; ms?: number };
}

export interface Thread {
  id: string;
  title: string;
  messages: ChatMsg[];
  sessionId: string | null;
  cwd?: string; // pinned working dir — where this thread's claude session lives, for resume recovery
  updatedAt: number;
}

const uid = () => crypto.randomUUID();

const PERM_FLAG: Record<string, string> = {
  acceptEdits: "acceptEdits",
  plan: "plan",
  bypass: "bypassPermissions",
};

// One live claude process at a time — the active thread's. It stays alive across turns: we feed
// each user turn over its stdin and read a continuous event stream back. `live` is the session
// that owns the process; each chatStart binds its handler to a specific session object, so events
// from a superseded/dead process (which no longer === live) are ignored instead of clobbering.
interface Live {
  tid: string;
  asstId: string;
  gotStream: boolean;
  skipBlock: boolean;
  startedAt: number;
  pending: string;
  pj: string; // buffered partial tool-input json, flushed on the same timer as text
  timer: ReturnType<typeof setTimeout> | null;
  watchdog: ReturnType<typeof setTimeout> | null;
}
let live: Live | null = null;

// if a turn goes this long with no event at all, assume the engine wedged and unstick the UI.
// generous on purpose — legitimate agentic turns stream events well within this.
const WATCHDOG_MS = 6 * 60 * 1000;

// keep in-memory threads from growing forever — same cap the disk blob uses
const MAX_THREAD_MSGS = 200;

// ---- module-level store helpers (used by the persistent stream handler) ----
const patchMsg = (tid: string, asstId: string, fn: (m: ChatMsg) => ChatMsg) =>
  useChat.setState((s) => ({
    threads: s.threads.map((t) => {
      if (t.id !== tid) return t;
      // streaming almost always patches the tail — swap it without mapping every message
      const last = t.messages[t.messages.length - 1];
      if (last && last.id === asstId) {
        const messages = t.messages.slice();
        messages[messages.length - 1] = fn(last);
        return { ...t, messages };
      }
      return { ...t, messages: t.messages.map((m) => (m.id === asstId ? fn(m) : m)) };
    }),
  }));

const setThreadSession = (tid: string, sid: string) =>
  useChat.setState((s) => ({
    threads: s.threads.map((t) => (t.id === tid ? { ...t, sessionId: sid } : t)),
  }));

const persistNow = () => {
  const { threads, model, currentId } = useChat.getState();
  // cap what hits disk: newest 30 threads, last 200 messages each, tool results truncated —
  // keeps the blob (and next load) bounded even after very long sessions. (in-memory shares the
  // message cap but keeps tool results whole for the ui.)
  const trimmed = [...threads]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, 30)
    .map((t) => ({
      ...t,
      messages: t.messages.slice(-MAX_THREAD_MSGS).map((m) => ({
        ...m,
        blocks: m.blocks.map((b) =>
          b.kind === "tool" && typeof b.result === "string" && b.result.length > 4000
            ? { ...b, result: b.result.slice(0, 4000) }
            : b,
        ),
      })),
    }));
  void saveState("chat", JSON.stringify({ threads: trimmed, model, currentId })).catch(() => {});
};

const appendText = (tid: string, asstId: string, delta: string) =>
  patchMsg(tid, asstId, (m) => {
    const blocks = [...m.blocks];
    const last = blocks[blocks.length - 1];
    if (last && last.kind === "text") blocks[blocks.length - 1] = { ...last, text: last.text + delta };
    else blocks.push({ kind: "text", text: delta });
    return { ...m, blocks };
  });

const appendPj = (tid: string, asstId: string, delta: string) =>
  patchMsg(tid, asstId, (m) => {
    const blocks = [...m.blocks];
    const last = blocks[blocks.length - 1];
    // partial json always belongs to the block being streamed, which is the last one
    if (last && last.kind === "tool") blocks[blocks.length - 1] = { ...last, _pj: (last._pj ?? "") + delta };
    return { ...m, blocks };
  });

// throttle token/json deltas so a long reply doesn't re-render markdown on every chunk.
// text and pj never accumulate at the same time (block boundaries flush), so order is safe.
const flush = () => {
  if (!live) return;
  if (live.timer) {
    clearTimeout(live.timer);
    live.timer = null;
  }
  if (!live.pending && !live.pj) return;
  const chunk = live.pending;
  const pj = live.pj;
  live.pending = "";
  live.pj = "";
  if (chunk) appendText(live.tid, live.asstId, chunk);
  if (pj) appendPj(live.tid, live.asstId, pj);
};
const queueText = (d: string) => {
  if (!live) return;
  live.pending += d;
  if (!live.timer) live.timer = setTimeout(flush, 55);
};
const queuePj = (d: string) => {
  if (!live) return;
  live.pj += d;
  if (!live.timer) live.timer = setTimeout(flush, 55);
};

// strip our ```hyprspace operator blocks, run them, and append the result chips
const runCommandsFor = async (tid: string, asstId: string) => {
  const th = useChat.getState().threads.find((t) => t.id === tid);
  const msg = th?.messages.find((m) => m.id === asstId);
  if (!msg) return;
  const kept: Block[] = [];
  const actions: Block[] = [];
  for (const b of msg.blocks) {
    if (b.kind === "text") {
      const { stripped, actions: res } = await runOperatorText(b.text);
      if (stripped) kept.push({ kind: "text", text: stripped });
      for (const a of res) actions.push({ kind: "action", label: a.label, ok: a.ok, spaceId: a.spaceId });
    } else {
      kept.push(b);
    }
  }
  if (actions.length) patchMsg(tid, asstId, (m) => ({ ...m, blocks: [...kept, ...actions] }));
};

// kill the live process (if any) and stop routing its events
const killLive = () => {
  if (!live) return;
  const tid = live.tid;
  if (live.timer) clearTimeout(live.timer);
  if (live.watchdog) clearTimeout(live.watchdog);
  live = null;
  void chatStop(tid).catch(() => {});
};

// (re)arm the inactivity watchdog for this turn; any event resets it
const pokeWatchdog = (sess: Live) => {
  if (sess.watchdog) clearTimeout(sess.watchdog);
  sess.watchdog = setTimeout(() => {
    if (live !== sess || !useChat.getState().busy) return; // only fire on a genuinely stuck turn
    patchMsg(sess.tid, sess.asstId, (m) => ({
      ...m,
      error: m.error || "No response from Claude — the session timed out. Try again.",
    }));
    killLive();
    useChat.setState({ busy: false });
    persistNow();
  }, WATCHDOG_MS);
};
const clearWatchdog = (sess: Live) => {
  if (sess.watchdog) {
    clearTimeout(sess.watchdog);
    sess.watchdog = null;
  }
};

// defensively normalize a persisted thread — a corrupt or old-format blob must never reach the
// render path with the wrong shape (it would throw and white-screen the chat).
function normalizeThread(raw: unknown): Thread | null {
  if (!raw || typeof raw !== "object") return null;
  const t = raw as Record<string, unknown>;
  if (typeof t.id !== "string") return null;
  const okBlock = (b: unknown): b is Block => {
    if (!b || typeof b !== "object") return false;
    const x = b as Record<string, unknown>;
    return (
      (x.kind === "text" && typeof x.text === "string") ||
      (x.kind === "tool" && typeof x.id === "string" && typeof x.name === "string") ||
      (x.kind === "action" && typeof x.label === "string" && typeof x.ok === "boolean")
    );
  };
  const messages: ChatMsg[] = [];
  for (const rm of Array.isArray(t.messages) ? (t.messages as unknown[]) : []) {
    if (!rm || typeof rm !== "object") continue;
    const m = rm as Record<string, unknown>;
    if (typeof m.id !== "string" || (m.role !== "user" && m.role !== "assistant")) continue;
    const u = m.usage && typeof m.usage === "object" ? (m.usage as Record<string, unknown>) : null;
    messages.push({
      id: m.id,
      role: m.role,
      blocks: Array.isArray(m.blocks) ? (m.blocks as unknown[]).filter(okBlock) : [],
      error: typeof m.error === "string" ? m.error : undefined,
      usage:
        u && typeof u.out === "number"
          ? { out: u.out, cost: typeof u.cost === "number" ? u.cost : 0, ms: typeof u.ms === "number" ? u.ms : undefined }
          : undefined,
    });
  }
  return {
    id: t.id,
    title: typeof t.title === "string" ? t.title : "Chat",
    messages,
    sessionId: typeof t.sessionId === "string" ? t.sessionId : null,
    cwd: typeof t.cwd === "string" ? t.cwd : undefined,
    updatedAt: typeof t.updatedAt === "number" ? t.updatedAt : Date.now(),
  };
}

// route one stdout line from a claude process to the current in-flight assistant message.
// `sess` is the session that owns the emitting process; if it's no longer the active `live`
// session we ignore the line (the process was superseded or killed).
function routeLine(sess: Live, line: string) {
  if (live !== sess) return;
  pokeWatchdog(sess); // any activity resets the stuck-turn timer
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let ev: any;
  try {
    ev = JSON.parse(line);
  } catch {
    return;
  }
  const cur = sess;
  const patch = (fn: (m: ChatMsg) => ChatMsg) => patchMsg(cur.tid, cur.asstId, fn);
  const t = ev.type;

  if (t === "system" && ev.subtype === "init") {
    if (ev.session_id) setThreadSession(cur.tid, ev.session_id);
  } else if (t === "stream_event") {
    cur.gotStream = true;
    const e = ev.event;
    if (!e) return;
    if (e.type === "content_block_start") {
      flush();
      const cb = e.content_block;
      if (cb?.type === "thinking") cur.skipBlock = true;
      else if (cb?.type === "text") {
        cur.skipBlock = false;
        patch((m) => ({ ...m, blocks: [...m.blocks, { kind: "text", text: "" }] }));
      } else if (cb?.type === "tool_use") {
        cur.skipBlock = false;
        patch((m) => ({
          ...m,
          blocks: [...m.blocks, { kind: "tool", id: cb.id, name: cb.name, input: {}, _pj: "" }],
        }));
      }
    } else if (e.type === "content_block_delta") {
      if (cur.skipBlock) return;
      const d = e.delta;
      if (d?.type === "text_delta" && d.text) queueText(d.text);
      else if (d?.type === "input_json_delta" && d.partial_json !== undefined) queuePj(d.partial_json);
    } else if (e.type === "content_block_stop") {
      flush();
      if (cur.skipBlock) {
        cur.skipBlock = false;
        return;
      }
      patch((m) => {
        const blocks = [...m.blocks];
        const last = blocks[blocks.length - 1];
        if (last && last.kind === "tool" && last._pj) {
          try {
            const input = JSON.parse(last._pj);
            blocks[blocks.length - 1] = { ...last, input, _pj: undefined };
          } catch {
            /* keep partial */
          }
        }
        return { ...m, blocks };
      });
    }
  } else if (t === "assistant") {
    const content = ev.message?.content ?? [];
    patch((m) => {
      // trust streamed text/tools when present; otherwise fall back to the complete message so a
      // thinking-only stream (or text that only arrived here) never renders blank.
      const hasText = m.blocks.some((b) => b.kind === "text" && b.text.trim() !== "");
      const hasTool = m.blocks.some((b) => b.kind === "tool");
      if (cur.gotStream && (hasText || hasTool)) return m;
      const blocks = m.blocks.filter((b) => !(b.kind === "text" && b.text.trim() === ""));
      for (const b of content) {
        if (b.type === "text" && b.text) blocks.push({ kind: "text", text: b.text });
        else if (b.type === "tool_use")
          blocks.push({ kind: "tool", id: b.id, name: b.name, input: b.input });
      }
      return { ...m, blocks };
    });
  } else if (t === "user") {
    const content = ev.message?.content ?? [];
    patch((m) => ({
      ...m,
      blocks: m.blocks.map((bl) => {
        if (bl.kind !== "tool") return bl;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const r = content.find((c: any) => c.type === "tool_result" && c.tool_use_id === bl.id);
        if (!r) return bl;
        const res =
          typeof r.content === "string"
            ? r.content
            : Array.isArray(r.content)
              ? // eslint-disable-next-line @typescript-eslint/no-explicit-any
                r.content.map((x: any) => x.text ?? "").join("")
              : "";
        return { ...bl, result: res };
      }),
    }));
  } else if (t === "result") {
    flush();
    const out = ev.usage?.output_tokens ?? 0;
    const cost = ev.total_cost_usd ?? 0;
    patch((m) => ({ ...m, usage: { out, cost, ms: Date.now() - cur.startedAt } }));
    if (ev.is_error === true) {
      const msg =
        (typeof ev.result === "string" && ev.result.trim()) ||
        `Claude ended with an error${ev.subtype ? ` (${ev.subtype})` : ""}.`;
      patch((m) => ({ ...m, error: m.error || msg }));
    }
    // never leave a finished turn stuck on the "Thinking…" placeholder
    patch((m) =>
      m.blocks.length === 0 && !m.error
        ? { ...m, blocks: [{ kind: "text", text: "_(no response)_" }] }
        : m,
    );
    // the turn finished — the process stays alive for the next one
    clearWatchdog(cur);
    useChat.setState({ busy: false });
    void runCommandsFor(cur.tid, cur.asstId).then(persistNow);
  } else if (t === "stderr") {
    if (ev.text && /no conversation found|session id/i.test(ev.text)) {
      // the resumed session is gone (created in a different folder, or pruned). forget it so the
      // next message starts a fresh conversation in this thread instead of failing again.
      useChat.setState((s) => ({
        threads: s.threads.map((t) => (t.id === cur.tid ? { ...t, sessionId: null } : t)),
      }));
      patch((m) => ({
        ...m,
        error: "Couldn't resume this conversation — its session is gone. Send your message again to start fresh.",
      }));
    } else if (ev.text && /error|not logged|unauthorized|usage limit|forbidden/i.test(ev.text)) {
      patch((m) => ({ ...m, error: (m.error ? m.error + "\n" : "") + ev.text }));
    }
  } else if (t === "exit") {
    // the process ended. since `live === sess` here, it wasn't an intentional kill (killLive nulls
    // live first) — it died. finalize the turn and drop the session so the next send respawns + resumes.
    flush();
    clearWatchdog(cur);
    patch((m) =>
      m.blocks.length === 0 && !m.error
        ? { ...m, blocks: [{ kind: "text", text: "_(engine stopped)_" }] }
        : m,
    );
    live = null;
    useChat.setState({ busy: false });
    persistNow();
  }
}

interface ChatState {
  threads: Thread[];
  currentId: string | null;
  model: string;
  busy: boolean;
  loaded: boolean;
  load: () => Promise<void>;
  newThread: () => void;
  switchThread: (id: string) => void;
  deleteThread: (id: string) => void;
  setModel: (m: string) => void;
  stop: () => void;
  send: (text: string, cwd: string) => void;
}

export const useChat = create<ChatState>((set, get) => ({
  threads: [],
  currentId: null,
  model: "opus",
  busy: false,
  loaded: false,

  load: async () => {
    if (get().loaded) return;
    const raw = await loadState("chat").catch(() => null);
    if (raw) {
      try {
        const p = JSON.parse(raw);
        if (Array.isArray(p?.threads)) {
          const threads = (p.threads as unknown[])
            .map(normalizeThread)
            .filter((t): t is Thread => t !== null);
          set({
            threads,
            model: p.model || "opus",
            currentId:
              p.currentId && threads.some((t) => t.id === p.currentId)
                ? p.currentId
                : threads[0]?.id ?? null,
          });
        }
      } catch {
        /* ignore a bad blob */
      }
    }
    set({ loaded: true });
  },

  setModel: (m) => {
    // model only takes effect on a fresh process — drop the live one so the next turn relaunches
    killLive();
    set({ model: m, busy: false });
    persistNow();
  },
  stop: () => {
    killLive();
    set({ busy: false });
  },
  newThread: () => {
    killLive();
    const id = uid();
    set((s) => ({
      threads: [{ id, title: "New chat", messages: [], sessionId: null, updatedAt: Date.now() }, ...s.threads],
      currentId: id,
      busy: false,
    }));
    persistNow();
  },
  switchThread: (id) => {
    killLive();
    set({ currentId: id, busy: false });
  },
  deleteThread: (id) => {
    killLive();
    set((s) => {
      const threads = s.threads.filter((t) => t.id !== id);
      const currentId = s.currentId === id ? threads[0]?.id ?? null : s.currentId;
      return { threads, currentId, busy: false };
    });
    persistNow();
  },

  send: (text, cwd) => {
    const st = get();
    if (st.busy || !text.trim()) return;

    // ensure a current thread
    let tid = st.currentId;
    if (!tid || !st.threads.some((t) => t.id === tid)) {
      tid = uid();
      set((s) => ({
        threads: [
          { id: tid!, title: "New chat", messages: [], sessionId: null, updatedAt: Date.now() },
          ...s.threads,
        ],
        currentId: tid,
      }));
    }
    const thread = get().threads.find((t) => t.id === tid)!;
    const runCwd = thread.cwd ?? cwd;
    const sessionId = thread.sessionId;
    const asstId = uid();
    const title =
      thread.messages.length === 0 ? text.trim().replace(/\s+/g, " ").slice(0, 48) : thread.title;

    set((s) => ({
      threads: s.threads.map((t) =>
        t.id !== tid
          ? t
          : {
              ...t,
              title,
              cwd: t.cwd ?? cwd, // pin the cwd on first use so a resume always runs in the right folder
              updatedAt: Date.now(),
              // cap in memory too (disk already does) so a long-lived session stays bounded
              messages: [
                ...t.messages,
                { id: uid(), role: "user" as const, blocks: [{ kind: "text" as const, text }] },
                { id: asstId, role: "assistant" as const, blocks: [] },
              ].slice(-MAX_THREAD_MSGS),
            },
      ),
      busy: true,
    }));

    // reuse the live process if it belongs to this thread; otherwise (re)start one
    const myTid = tid;
    let startP: Promise<void> = Promise.resolve();
    if (!live || live.tid !== myTid) {
      killLive();
      const args = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--verbose",
      ];
      if (st.model && st.model !== "default") args.push("--model", st.model);
      if (sessionId) args.push("--resume", sessionId); // recover an existing conversation from disk
      const perm = PERM_FLAG[useSettings.getState().claudePermission];
      if (perm) args.push("--permission-mode", perm);
      const sess: Live = {
        tid: myTid,
        asstId,
        gotStream: false,
        skipBlock: false,
        startedAt: Date.now(),
        pending: "",
        pj: "",
        timer: null,
        watchdog: null,
      };
      live = sess;
      startP = chatStart(myTid, runCwd, args, (line) => routeLine(sess, line));
    } else {
      // same process — just start a new turn on it
      live.asstId = asstId;
      live.gotStream = false;
      live.skipBlock = false;
      live.startedAt = Date.now();
      live.pending = "";
      live.pj = "";
      if (live.timer) {
        clearTimeout(live.timer);
        live.timer = null;
      }
    }

    // the operator preamble only needs to go in once per conversation — on its first turn
    const turnText = sessionId ? text : ORCHESTRATOR_PREAMBLE + "\n\n---\n\n" + text;
    const envelope = JSON.stringify({
      type: "user",
      message: { role: "user", content: [{ type: "text", text: turnText }] },
    });
    if (live) pokeWatchdog(live); // arm now, in case the engine never emits a single event
    startP
      .then(() => chatTurn(myTid, envelope))
      .catch((e) => {
        patchMsg(myTid, asstId, (m) => ({ ...m, error: String(e) }));
        killLive();
        set({ busy: false });
      });
  },
}));
