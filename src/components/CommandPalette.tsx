import { useEffect, useMemo, useRef, useState } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import {
  newClaude,
  newTerminal,
  closeFocused,
  toggleMaxFocused,
  switchSpaceByIndex,
} from "../actions";

interface Cmd {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
}

export function CommandPalette() {
  const open = useUi((s) => s.paletteOpen);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const close = () => useUi.getState().setPalette(false);

  const commands = useMemo<Cmd[]>(() => {
    const base: Cmd[] = [
      { id: "claude", label: "New Claude session", run: () => void newClaude() },
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
    const spaces: Cmd[] = workspaces.map((w, i) => ({
      id: "ws-" + w.id,
      label: `Switch to ${w.name}`,
      hint: i < 9 ? `Ctrl+${i + 1}` : undefined,
      run: () => switchSpaceByIndex(i),
    }));
    return [...base, ...spaces];
  }, [workspaces]);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return commands;
    return commands.filter((c) => c.label.toLowerCase().includes(s));
  }, [q, commands]);

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

  const run = (c?: Cmd) => {
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
          placeholder="Type a command…"
          value={q}
          spellCheck={false}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              close();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setSel((i) => Math.min(filtered.length - 1, i + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setSel((i) => Math.max(0, i - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              run(filtered[sel]);
            }
          }}
        />
        <div className="cmdk-list">
          {filtered.length === 0 && <div className="cmdk-empty">No matching commands</div>}
          {filtered.map((c, i) => (
            <button
              key={c.id}
              className={`cmdk-item${i === sel ? " active" : ""}`}
              onMouseEnter={() => setSel(i)}
              onClick={() => run(c)}
            >
              <span className="cmdk-label">{c.label}</span>
              {c.hint && <span className="cmdk-hint">{c.hint}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
