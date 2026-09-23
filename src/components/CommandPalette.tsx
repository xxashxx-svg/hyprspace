import { Fragment, useDeferredValue, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Bot,
  FolderPlus,
  Gauge,
  GitBranch,
  Maximize2,
  Moon,
  Palette,
  PanelLeft,
  PanelRight,
  Search,
  Settings as SettingsIcon,
  SquarePen,
  SquareTerminal,
  Sun,
  TextSearch,
  X,
  Zap,
} from "lucide-react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useSettings } from "../stores/settings";
import { PROVIDER_LOGO } from "../lib/brand";
import { searchOutput } from "../terminal/buffers";
import { isMac, kbd } from "../platform";
import { newSession, newTerminal, openFolderAsSpace, closeFocused, toggleMaxFocused } from "../actions";

interface Item {
  id: string;
  section: string;
  label: string;
  sub?: string; // a folder, or a snippet for terminal hits
  hint?: string; // shortcut, written the Windows way; kbd() turns it into glyphs on mac
  icon: ReactNode;
  run: () => void;
}

// a shortcut as separate keycaps: "Ctrl+Shift+N" on Windows, ⌘ ⇧ N on mac
function Keys({ hint }: { hint: string }) {
  const k = kbd(hint);
  const caps = isMac ? [...k] : k.split("+");
  return (
    <span className="cmdk-keys">
      {caps.map((c, i) => (
        <kbd key={i}>{c}</kbd>
      ))}
    </span>
  );
}

export function CommandPalette() {
  const open = useUi((s) => s.paletteOpen);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const focusedId = useWorkspaces((s) => s.focusedSessionId);
  const mode = useSettings((s) => s.mode);
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const close = () => useUi.getState().setPalette(false);

  // Starting work goes through the composer now: New thread opens it, and it picks the agent,
  // model and effort. So there are no per-agent "New X session" commands any more.
  const commands = useMemo<Item[]>(() => {
    const ui = useUi.getState();
    const settings = (tab: string) => () => ui.openSettings(tab);
    const I = 15;
    return [
      { id: "new", section: "Start", label: "New thread", hint: "Ctrl+Shift+N", icon: <SquarePen size={I} />, run: newSession },
      { id: "term", section: "Start", label: "New terminal", hint: "Ctrl+Shift+T", icon: <SquareTerminal size={I} />, run: () => void newTerminal() },
      { id: "folder", section: "Start", label: "Open a folder", sub: "Adds it to the sidebar", icon: <FolderPlus size={I} />, run: () => void openFolderAsSpace() },

      // every running thread, by its title, under the folder it lives in. file, image and diff
      // tabs aren't threads, and archived folders are parked on purpose, so both stay out
      ...workspaces.filter((w) => !w.archived).flatMap((w) =>
        w.sessions
          .filter((s) => !s.image && !s.file && !s.media && !s.diff)
          .map((s) => ({
            id: "th-" + s.id,
            section: "Threads",
            label: s.title || "New thread",
            sub: s.id === focusedId ? `${w.name} · current` : w.name,
            icon: PROVIDER_LOGO[s.provider] ? <img src={PROVIDER_LOGO[s.provider]} alt="" /> : <SquareTerminal size={I} />,
            run: () => {
              useWorkspaces.getState().setActive(w.id);
              useWorkspaces.getState().setFocused(s.id);
              ui.goSpace();
            },
          })),
      ),

      { id: "close", section: "Panes", label: "Close the focused pane", hint: "Ctrl+Shift+W", icon: <X size={I} />, run: closeFocused },
      { id: "max", section: "Panes", label: "Maximize or restore the focused pane", hint: "Ctrl+Shift+M", icon: <Maximize2 size={I} />, run: toggleMaxFocused },

      { id: "rail", section: "View", label: "Show or hide the sidebar", icon: <PanelLeft size={I} />, run: () => ui.toggleRail() },
      { id: "dock", section: "View", label: "Show or hide files and git", hint: "Ctrl+Shift+G", icon: <PanelRight size={I} />, run: () => ui.toggleDock() },
      { id: "git", section: "View", label: "Open the git panel", icon: <GitBranch size={I} />, run: () => ui.setDockTab("git") },
      {
        id: "scheme",
        section: "View",
        label: mode === "dark" ? "Switch to light mode" : "Switch to dark mode",
        icon: mode === "dark" ? <Sun size={I} /> : <Moon size={I} />,
        run: () => useSettings.getState().setColorScheme(mode === "dark" ? "light" : "dark"),
      },

      { id: "settings", section: "Settings", label: "Settings", icon: <SettingsIcon size={I} />, run: () => ui.openSettings() },
      { id: "appearance", section: "Settings", label: "Appearance", sub: "Theme, light or dark, fonts", icon: <Palette size={I} />, run: settings("appearance") },
      { id: "agents", section: "Settings", label: "Defaults", sub: "Model, effort and permissions for each agent", icon: <Bot size={I} />, run: settings("agents") },
      { id: "usage", section: "Settings", label: "Usage", sub: "What each agent has used", icon: <Gauge size={I} />, run: settings("usage") },
      { id: "skills", section: "Settings", label: "Skills", sub: "Reusable instructions for Claude", icon: <Zap size={I} />, run: settings("skills") },
    ];
  }, [workspaces, focusedId, mode]);

  // every word has to appear somewhere in the label, section or subtitle
  const filtered = useMemo(() => {
    const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return commands;
    return commands.filter((c) => {
      const hay = `${c.label} ${c.section} ${c.sub ?? ""}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
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
          label = `${w.name} › ${sess.title || sess.provider}`;
          break;
        }
      }
      if (!wsId) continue;
      out.push({
        id: "term-" + hit.sessionId,
        section: "In terminal output",
        label,
        sub: hit.snippet,
        icon: <TextSearch size={15} />,
        run: () => {
          useWorkspaces.getState().setActive(wsId);
          useWorkspaces.getState().setFocused(hit.sessionId);
        },
      });
    }
    return out;
  }, [dq, workspaces]);

  const items = useMemo(() => [...filtered, ...termItems], [filtered, termItems]);

  useEffect(() => {
    if (open) {
      setQ("");
      setSel(0);
      const id = requestAnimationFrame(() => inputRef.current?.focus());
      return () => cancelAnimationFrame(id);
    }
  }, [open]);
  useEffect(() => setSel(0), [q]);
  // keep the highlighted row in view as the arrows walk past the edge of the list
  useEffect(() => {
    listRef.current?.querySelector(".cmdk-item.active")?.scrollIntoView({ block: "nearest" });
  }, [sel]);

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
            placeholder="Type a command, a thread, or text from a terminal"
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
          <kbd className="cmdk-esc">Esc</kbd>
        </div>
        <div className="cmdk-list" ref={listRef}>
          {items.length === 0 && (
            <div className="cmdk-empty">
              {q.trim().length < 2 ? "No commands match" : "Nothing matches, in commands or in any terminal"}
            </div>
          )}
          {items.map((c, i) => (
            <Fragment key={c.id}>
              {(i === 0 || items[i - 1].section !== c.section) && <div className="cmdk-section">{c.section}</div>}
              <button className={`cmdk-item${i === sel ? " active" : ""}`} onMouseMove={() => setSel(i)} onClick={() => run(c)}>
                <span className="cmdk-ico">{c.icon}</span>
                <span className="cmdk-main">
                  <span className="cmdk-label">{c.label}</span>
                  {c.sub && <span className={`cmdk-sub${c.section === "In terminal output" ? " mono" : ""}`}>{c.sub}</span>}
                </span>
                {c.hint && <Keys hint={c.hint} />}
              </button>
            </Fragment>
          ))}
        </div>
        <div className="cmdk-foot">
          <span>
            <kbd>↑</kbd>
            <kbd>↓</kbd> move
          </span>
          <span>
            <kbd>↵</kbd> run
          </span>
          <span>
            <kbd>Esc</kbd> close
          </span>
        </div>
      </div>
    </div>
  );
}
