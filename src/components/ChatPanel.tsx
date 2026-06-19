import { useEffect, useRef, useState, type ReactNode } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useChat, type ChatMsg, type Block } from "../stores/chat";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { relTime } from "../lib/time";
import {
  Sparkles,
  ArrowUp,
  Square,
  ChevronDown,
  ChevronUp,
  Check,
  Copy,
  X,
  SquareTerminal,
  Plus,
  MessageSquare,
} from "lucide-react";

type ToolBlock = Extract<Block, { kind: "tool" }>;

// pull a short one-liner out of a tool's input so the work-log row reads like a command
function oneLineInput(input: unknown): string {
  if (!input || typeof input !== "object") return "";
  const o = input as Record<string, unknown>;
  const v = o.command ?? o.file_path ?? o.path ?? o.pattern ?? o.query ?? o.url ?? o.prompt ?? "";
  return typeof v === "string" ? v.replace(/\s+/g, " ").trim().slice(0, 90) : "";
}

function fmtDur(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  return `${Math.floor(s / 60)}m ${s % 60}s`;
}

const EMPTY: ChatMsg[] = [];

const MODELS = [
  { id: "opus", label: "Opus", desc: "Most capable" },
  { id: "sonnet", label: "Sonnet", desc: "Balanced speed & smarts" },
  { id: "haiku", label: "Haiku", desc: "Fastest" },
];

function ModelPicker() {
  const model = useChat((s) => s.model);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const cur = MODELS.find((m) => m.id === model) ?? MODELS[0];
  return (
    <div className="chat-modelpick" ref={ref}>
      <button className="chat-model-btn" onClick={() => setOpen((o) => !o)}>
        <Sparkles size={13} />
        <span>Claude {cur.label}</span>
        <ChevronDown size={12} className="chat-model-caret" />
      </button>
      {open && (
        <div className="chat-model-pop">
          {MODELS.map((m) => (
            <button
              key={m.id}
              className={`chat-model-opt${m.id === model ? " active" : ""}`}
              onClick={() => {
                useChat.getState().setModel(m.id);
                setOpen(false);
              }}
            >
              <span className="chat-model-opt-ico">
                <Sparkles size={14} />
              </span>
              <span className="chat-model-opt-body">
                <span className="chat-model-opt-name">Claude {m.label}</span>
                <span className="chat-model-opt-desc">{m.desc}</span>
              </span>
              {m.id === model && <Check size={14} className="chat-model-opt-check" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ThreadBar() {
  const threads = useChat((s) => s.threads);
  const currentId = useChat((s) => s.currentId);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  if (threads.length === 0) return null;
  const cur = threads.find((t) => t.id === currentId);
  const sorted = [...threads].sort((a, b) => b.updatedAt - a.updatedAt);

  return (
    <div className="chat-threads" ref={ref}>
      <button className="chat-thread-cur" onClick={() => setOpen((o) => !o)}>
        <MessageSquare size={13} />
        <span className="chat-thread-title">{cur?.title || "New chat"}</span>
        <ChevronDown size={12} />
      </button>
      <button className="chat-thread-new" title="New chat" onClick={() => useChat.getState().newThread()}>
        <Plus size={14} />
      </button>
      {open && (
        <div className="chat-thread-pop">
          {sorted.map((t) => (
            <div key={t.id} className={`chat-thread-row${t.id === currentId ? " active" : ""}`}>
              <button
                className="chat-thread-pick"
                onClick={() => {
                  useChat.getState().switchThread(t.id);
                  setOpen(false);
                }}
              >
                <span className="chat-thread-name">{t.title || "New chat"}</span>
                <span className="chat-thread-time">{relTime(t.updatedAt)}</span>
              </button>
              <button
                className="chat-thread-del"
                title="Delete"
                onClick={() => useChat.getState().deleteThread(t.id)}
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function WorkRow({ t }: { t: ToolBlock }) {
  const [open, setOpen] = useState(false);
  const running = t.result === undefined;
  const sum = oneLineInput(t.input);
  return (
    <div className="chat-wl-item">
      <button className="chat-wl-row" onClick={() => setOpen((o) => !o)}>
        <span className="chat-wl-ico">
          {running ? <span className="chat-wl-spin" /> : <Check size={12} />}
        </span>
        <span className="chat-wl-name">{t.name}</span>
        {sum && <span className="chat-wl-sum">{sum}</span>}
        <ChevronDown size={12} className={`chat-wl-twist${open ? " open" : ""}`} />
      </button>
      {open && (
        <div className="chat-wl-detail">
          <pre className="chat-tool-in">{JSON.stringify(t.input, null, 2)}</pre>
          {t.result !== undefined && t.result !== "" && (
            <pre className="chat-tool-out">{t.result.slice(0, 4000)}</pre>
          )}
        </div>
      )}
    </div>
  );
}

function WorkLog({ tools }: { tools: ToolBlock[] }) {
  const running = tools.some((t) => t.result === undefined);
  // live responses show the work expanded; reloaded (all done) ones start collapsed
  const [open, setOpen] = useState(running);
  return (
    <div className="chat-worklog">
      <button className="chat-worklog-head" onClick={() => setOpen((o) => !o)}>
        <span className="chat-worklog-title">Work log ({tools.length})</span>
        {running && <span className="chat-worklog-run">running…</span>}
        <ChevronDown size={13} className={`chat-worklog-twist${open ? " open" : ""}`} />
      </button>
      {open && (
        <div className="chat-worklog-body">
          {tools.map((t) => (
            <WorkRow key={t.id} t={t} />
          ))}
        </div>
      )}
    </div>
  );
}

function ActionChip({ b }: { b: Extract<Block, { kind: "action" }> }) {
  const go = () => {
    if (!b.spaceId) return;
    useWorkspaces.getState().setActive(b.spaceId);
    useUi.getState().goSpace();
  };
  return (
    <button className={`chat-action${b.ok ? "" : " bad"}`} onClick={go} disabled={!b.spaceId}>
      {b.ok ? <Check size={13} /> : <X size={13} />}
      <span className="chat-action-label">{b.label}</span>
      {b.ok && b.spaceId && <span className="chat-action-open">Open →</span>}
    </button>
  );
}

function CopyBtn({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="chat-msg-act"
      title="Copy"
      onClick={() => {
        void writeText(text);
        setDone(true);
        setTimeout(() => setDone(false), 1200);
      }}
    >
      {done ? <Check size={12} /> : <Copy size={12} />}
      {done ? "Copied" : "Copy"}
    </button>
  );
}

function Message({ m, live }: { m: ChatMsg; live?: boolean }) {
  if (m.role === "user") {
    return (
      <div className="chat-msg user">
        <div className="chat-bubble">
          {m.blocks.map((b, i) => (b.kind === "text" ? <span key={i}>{b.text}</span> : null))}
        </div>
      </div>
    );
  }
  // coalesce runs of tool calls into a single work-log group; text/actions render on their own
  const items: ReactNode[] = [];
  for (let i = 0; i < m.blocks.length; ) {
    const b = m.blocks[i];
    if (b.kind === "tool") {
      const run: ToolBlock[] = [];
      while (i < m.blocks.length && m.blocks[i].kind === "tool") run.push(m.blocks[i++] as ToolBlock);
      items.push(<WorkLog key={`wl-${i}`} tools={run} />);
      continue;
    }
    if (b.kind === "text")
      items.push(
        <div key={i} className="chat-md">
          <Markdown remarkPlugins={[remarkGfm]}>{b.text}</Markdown>
        </div>,
      );
    else items.push(<ActionChip key={i} b={b} />);
    i++;
  }

  const u = m.usage;
  const footer = u
    ? [u.ms ? `Worked ${fmtDur(u.ms)}` : "", u.out ? `${u.out.toLocaleString()} tokens` : "", u.cost ? `$${u.cost.toFixed(4)}` : ""]
        .filter(Boolean)
        .join(" · ")
    : "";
  const copyText = m.blocks
    .map((b) => (b.kind === "text" ? b.text : ""))
    .filter(Boolean)
    .join("\n\n")
    .trim();

  return (
    <div className="chat-msg asst">
      {m.blocks.length === 0 && !m.error && (
        <div className="chat-think">{live ? "Thinking…" : "(no response)"}</div>
      )}
      {items}
      {m.error && <div className="chat-err">{m.error}</div>}
      <div className="chat-msg-foot">
        {footer && <span className="chat-usage">{footer}</span>}
        {!live && copyText && (
          <div className="chat-msg-actions">
            <CopyBtn text={copyText} />
          </div>
        )}
      </div>
    </div>
  );
}

export function ChatPanel() {
  const messages = useChat((s) => s.threads.find((t) => t.id === s.currentId)?.messages ?? EMPTY);
  const sessionId = useChat((s) => s.threads.find((t) => t.id === s.currentId)?.sessionId ?? null);
  const busy = useChat((s) => s.busy);
  const [text, setText] = useState("");
  const [open, setOpen] = useState(false); // whole chat closed by default — just a launcher
  const [expanded, setExpanded] = useState(false); // when open, the history starts collapsed
  const scrollRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);

  // grow the input with its content, up to the CSS max-height (then it scrolls)
  const resizeInput = () => {
    const el = taRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  };

  useEffect(() => {
    void useChat.getState().load();
  }, []);

  // the chat runs in the active project's folder (or the focused pane's), else the home dir
  const cwd = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    return w?.sessions.find((ss) => ss.id === s.focusedSessionId)?.cwd || w?.cwd || "";
  });

  // resume this thread's conversation as a full Claude pane. the session lives in the thread's
  // pinned folder, so the pane MUST run there for --resume to find it. prefer a project already at
  // that folder; otherwise drop it into the active space but still cwd'd to the session's folder.
  const openInPane = () => {
    if (!sessionId) return;
    const ws = useWorkspaces.getState();
    const chat = useChat.getState();
    const thread = chat.threads.find((t) => t.id === chat.currentId);
    const folder = thread?.cwd || cwd || undefined;
    const match = folder ? ws.workspaces.find((w) => w.cwd === folder) : undefined;
    const target = match ?? ws.workspaces.find((w) => w.id === ws.activeId) ?? ws.workspaces[0];
    if (!target) return;
    ws.setActive(target.id);
    ws.addSession(target.id, `claude --resume ${sessionId}`, folder);
    useUi.getState().goSpace();
  };

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages]);

  const submit = () => {
    const t = text.trim();
    if (!t || busy) return;
    useChat.getState().send(t, cwd);
    setText("");
    setExpanded(true); // show the conversation once you send
    requestAnimationFrame(resizeInput);
  };

  // fully collapsed — a slim launcher in place of the chat. click opens the composer.
  if (!open) {
    return (
      <div className="chat chat-closed">
        <button
          className="chat-launcher"
          onClick={() => {
            setOpen(true);
            requestAnimationFrame(() => taRef.current?.focus());
          }}
        >
          <Sparkles size={14} />
          <span>Ask Claude or spawn agents…</span>
          <ChevronUp size={15} className="chat-launcher-caret" />
        </button>
      </div>
    );
  }

  return (
    <div className={`chat${expanded ? " expanded" : ""}`}>
      {expanded && <ThreadBar />}
      {expanded && messages.length > 0 && (
        <div className="chat-scroll" ref={scrollRef}>
          {messages.map((m, i) => (
            <Message key={m.id} m={m} live={busy && i === messages.length - 1} />
          ))}
        </div>
      )}
      <div className="chat-box">
        <textarea
          ref={taRef}
          className="chat-input"
          placeholder="Ask Claude anything, or tell it to spawn agents…"
          value={text}
          rows={1}
          onChange={(e) => {
            setText(e.target.value);
            resizeInput();
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
          }}
        />
        <div className="chat-bar">
          <ModelPicker />
          <span className="chat-spacer" />
          {messages.length > 0 && (
            <button
              className="chat-pane"
              title={expanded ? "Collapse conversation" : "Show conversation"}
              onClick={() => setExpanded((v) => !v)}
            >
              {expanded ? <ChevronDown size={15} /> : <ChevronUp size={15} />}
            </button>
          )}
          {sessionId && !busy && (
            <button className="chat-pane" title="Continue in a terminal pane" onClick={openInPane}>
              <SquareTerminal size={13} />
            </button>
          )}
          <button className="chat-pane" title="Close chat" onClick={() => setOpen(false)}>
            <X size={15} />
          </button>
          {busy ? (
            <button className="chat-send stop" title="Stop" onClick={() => useChat.getState().stop()}>
              <Square size={13} />
            </button>
          ) : (
            <button className="chat-send" title="Send (Enter)" onClick={submit} disabled={!text.trim()}>
              <ArrowUp size={16} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
