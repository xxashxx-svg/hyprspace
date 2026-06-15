import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { searchOutput } from "../terminal/buffers";
import {
  newClaude,
  newTerminal,
  newClaudeInWorktree,
  closeFocused,
  toggleMaxFocused,
  switchSpaceByIndex,
} from "../actions";

interface Item {
  id: string;
  label: string;
  hint?: string;
  sub?: string; // snippet for terminal hits
  kind?: "term";
  run: () => void;
}

export function CommandPalette() {
  const open = useUi((s) => s.paletteOpen);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const close = () => useUi.getState().setPalette(false);

  const commands = useMemo<Item[]>(() => {
    const base: Item[] = [
      { id: "claude", label: "New Claude session", run: () => void newClaude() },
      {
        id: "claude-wt",
        label: "New Claude in isolated worktree",
        run: () => void newClaudeInWorktree(),
      },
      { id: "term", label: "New terminal", hint: "Ctrl+Shift+T", run: () => void newTerminal() },
      { id: "close", label: "Close focused pane", hint: "Ctrl+Shift+W", run: closeFocused },
      { id: "max", label: "Maximize / restore pane", hint: "Ctrl+Shift+M", run: toggleMaxFocused },
      {
        id: "dock",
        label: "Toggle review dock — changes & run",
        hint: "Ctrl+Shift+G",
        run: () => useUi.getState().toggleDock(),
      },
      { id: "settings", label: "Open settings", run: () => useUi.getState().toggleSettings() },
    ];
    const spaces: Item[] = workspaces.map((w, i) => ({
      id: "ws-" + w.id,
      label: `Switch to ${w.name}`,
      hint: i < 9 ? `Ctrl+${i + 1}` : undefined,
      run: () => switchSpaceByIndex(i),
    }));
    return [...base, ...spaces];
  }, [workspaces]);

  const filteredCommands = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return commands;
    return commands.filter((c) => c.label.toLowerCase().includes(s));
  }, [q, commands]);

  // search terminal scrollback → jump-to-pane results
  const termItems = useMemo<Item[]>(() => {
    if (q.trim().length < 2) return [];
    const out: Item[] = [];
    for (const hit of searchOutput(q)) {
      let wsId = "";
      let label = "";
      for (const w of workspaces) {
        const sess = w.sessions.find((s) => s.id === hit.sessionId);
        if (sess) {
          wsId = w.id;
          label = `${w.name} › ${sess.command?.includes("claude") ? "claude" : "terminal"}`;
          break;
        }
      }
      if (!wsId) continue;
      out.push({
        id: "term-" + hit.sessionId,
        label,
        sub: hit.snippet,
        kind: "term",
        run: () => {
          useWorkspaces.getState().setActive(wsId);
          useWorkspaces.getState().setFocused(hit.sessionId);
        },
      });
    }
    return out;
  }, [q, workspaces]);

  const items = useMemo(() => [...filteredCommands, ...termItems], [filteredCommands, termItems]);

  useEffect(() => {
    if (open) {
      setQ("");
      setSel(0);
      const id = requestAnimationFrame(() => inputRef.current?.focus());
      return () => cancelAnimationFrame(id);
    }
  }, [open]);
  useEffect(() => setSel(0), [q]);

  if (!open) return null;

  const run = (c?: Item) => {
    if (!c) return;
    close();
    c.run();
  };

  return (
    <div className="cmdk-overlay" onMouseDown={close}>
      <div className="cmdk" onMouseDown={(e) => e.stopPropagation()}>
        <input
          ref={inputRef}
          className="cmdk-input"
          placeholder="Search commands or terminal output…"
          value={q}
          spellCheck={false}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              close();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setSel((i) => Math.min(items.length - 1, i + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setSel((i) => Math.max(0, i - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              run(items[sel]);
            }
          }}
        />
        <div className="cmdk-list">
          {items.length === 0 && <div className="cmdk-empty">No matches</div>}
          {items.map((c, i) => {
            const firstTerm = c.kind === "term" && (i === 0 || items[i - 1].kind !== "term");
            return (
              <Fragment key={c.id}>
                {firstTerm && <div className="cmdk-section">Jump to terminal</div>}
                <button
                  className={`cmdk-item${i === sel ? " active" : ""}`}
                  onMouseEnter={() => setSel(i)}
                  onClick={() => run(c)}
                >
                  <span className="cmdk-main">
                    <span className="cmdk-label">{c.label}</span>
                    {c.sub && <span className="cmdk-sub">{c.sub}</span>}
                  </span>
                  {c.hint && <span className="cmdk-hint">{c.hint}</span>}
                </button>
              </Fragment>
            );
          })}
        </div>
      </div>
    </div>
  );
}
