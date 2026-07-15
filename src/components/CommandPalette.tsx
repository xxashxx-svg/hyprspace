import { Fragment, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useLoops, newLoop } from "../stores/loops";
import { useProjectConfigs, folderKey } from "../stores/projectConfig";
import { runAction } from "../lib/startup";
import { pauseLoop } from "../lib/loops";
import { searchOutput } from "../terminal/buffers";
import { isWindows, kbd } from "../platform";
import { Search } from "lucide-react";
import {
  newClaude,
  newGemini,
  newCodex,
  newOpencode,
  newGrok,
  newWsl,
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
  const activeId = useWorkspaces((s) => s.activeId);
  const configs = useProjectConfigs((s) => s.configs);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const close = () => useUi.getState().setPalette(false);

  const commands = useMemo<Item[]>(() => {
    const base: Item[] = [
      {
        id: "launch",
        label: "Launch workspace — fan out many agents",
        run: () => useUi.getState().openLaunch(),
      },
      { id: "claude", label: "New Claude session", run: () => void newClaude() },
      { id: "gemini", label: "New Gemini session", run: () => void newGemini() },
      { id: "codex", label: "New Codex session", run: () => void newCodex() },
      { id: "opencode", label: "New OpenCode session", run: () => void newOpencode() },
      { id: "grok", label: "New Grok session", run: () => void newGrok() },
      ...(isWindows
        ? [{ id: "wsl", label: "New WSL (Linux) session", run: () => void newWsl() }]
        : []),
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
        label: "Toggle review dock — source & skills",
        hint: "Ctrl+Shift+G",
        run: () => useUi.getState().toggleDock(),
      },
      {
        id: "skills",
        label: "Skills — drag into terminals",
        run: () => useUi.getState().setDockTab("skills"),
      },
      {
        id: "loops-open",
        label: "Automations",
        run: () => useUi.getState().goLoops(),
      },
      {
        id: "loop-new",
        label: "New automation",
        run: () => {
          const w = useWorkspaces.getState();
          const ws = w.workspaces.find((x) => x.id === w.activeId);
          const folder = ws && ws.kind !== "open" ? ws.cwd : "";
          const def = newLoop(folder);
          def.name = "New automation";
          useLoops.getState().upsert(def);
          useUi.getState().goLoops();
        },
      },
      {
        id: "loops-pause",
        label: "Pause all running automations",
        run: () => {
          const runs = useLoops.getState().runs;
          for (const id of Object.keys(runs)) if (runs[id].status === "running") pauseLoop(id, true);
        },
      },
      { id: "settings", label: "Open settings", run: () => useUi.getState().openSettings() },
    ];
    // the active project's actions (run on demand)
    const aws = workspaces.find((w) => w.id === activeId);
    const folder = aws && aws.kind !== "open" ? aws.cwd : "";
    const actions: Item[] = folder
      ? (configs[folderKey(folder)]?.startup ?? []).map((a) => ({
          id: "action-" + a.id,
          label: `Run action: ${a.name || a.command}`,
          hint: a.keybinding,
          run: () => aws && runAction(aws.id, a),
        }))
      : [];
    const spaces: Item[] = workspaces.map((w, i) => ({
      id: "ws-" + w.id,
      label: `Switch to ${w.name}`,
      hint: i < 9 ? `Ctrl+${i + 1}` : undefined,
      run: () => switchSpaceByIndex(i),
    }));
    return [...base, ...actions, ...spaces];
  }, [workspaces, activeId, configs]);

  const filteredCommands = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return commands;
    return commands.filter((c) => c.label.toLowerCase().includes(s));
  }, [q, commands]);

  // search terminal scrollback → jump-to-pane results. scanning every pane's buffer is the
  // expensive part, so it runs on a deferred query — typing stays snappy, hits lag a beat.
  const dq = useDeferredValue(q);
  const termItems = useMemo<Item[]>(() => {
    if (dq.trim().length < 2) return [];
    const out: Item[] = [];
    for (const hit of searchOutput(dq)) {
      let wsId = "";
      let label = "";
      for (const w of workspaces) {
        const sess = w.sessions.find((s) => s.id === hit.sessionId);
        if (sess) {
          wsId = w.id;
          label = `${w.name} › ${sess.provider !== "terminal" ? sess.provider : "terminal"}`;
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
  }, [dq, workspaces]);

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
        <div className="cmdk-input-row">
          <Search size={16} className="cmdk-search-ico" />
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
        </div>
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
                  {c.hint && <span className="cmdk-hint">{kbd(c.hint)}</span>}
                </button>
              </Fragment>
            );
          })}
        </div>
      </div>
    </div>
  );
}
