import { useEffect, useRef, useState, type ReactNode, type MouseEvent as ReactMouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useGit } from "../stores/git";
import { useServices } from "../stores/services";
import { useLoops } from "../stores/loops";
import { isMac, isWindows } from "../platform";
import { pickFolder, gitIsRepo, revealPath } from "../api";
import { newClaude, newGemini, newCodex, newWsl, newTerminal, newClaudeInWorktree } from "../actions";
import {
  PanelRight,
  Plus,
  ChevronDown,
  Sparkles,
  Gem,
  Bot,
  Terminal,
  SquareTerminal,
  GitBranch,
  GitCommitVertical,
  GitPullRequest,
  ArrowUp,
  Upload,
  FolderOpen,
  FolderPlus,
  LayoutGrid,
  ExternalLink,
  Rocket,
  ScrollText,
  RotateCw,
  Play,
  Copy,
  Square,
} from "lucide-react";
import { Logo } from "./Logo";
import { NotificationPanel } from "./NotificationPanel";

const win = getCurrentWindow();

type MenuItem = { label: string; icon?: ReactNode; onClick: () => void };

// A compact outlined dropdown button for the topbar — T3's action-menu style.
function ActionMenu({ label, lead, items }: { label: string; lead?: ReactNode; items: MenuItem[] }) {
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

  return (
    <div className="tb-menu" ref={ref}>
      <button className="tb-action" onClick={() => setOpen((o) => !o)}>
        {lead}
        <span>{label}</span>
        <ChevronDown size={13} className="tb-action-caret" />
      </button>
      {open && (
        <div className="tb-menu-pop">
          {items.map((it) => (
            <button
              key={it.label}
              className="tb-menu-item"
              onClick={() => {
                setOpen(false);
                it.onClick();
              }}
            >
              {it.icon}
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// right-click menu for one background service: the full set of things you can do with it
function ServiceCtxMenu({ menu, onClose }: { menu: { x: number; y: number; id: string }; onClose: () => void }) {
  const meta = useServices((s) => s.running[menu.id]);
  const known = useServices((s) => s.known[menu.id]);
  const isRunning = !!meta;
  const name = meta?.name || known?.name || "service";
  const command = meta?.command || known?.command || "";
  const cwd = known?.cwd || "";
  const left = Math.min(menu.x, window.innerWidth - 210);
  return (
    <>
      <div
        className="ctx-backdrop"
        onClick={onClose}
        onContextMenu={(e) => {
          e.preventDefault();
          onClose();
        }}
      />
      <div className="ctx-menu" style={{ left, top: menu.y }}>
        <button
          className="ctx-item"
          onClick={() => {
            useUi.getState().openServiceLogs({ id: menu.id, name });
            onClose();
          }}
        >
          <ScrollText size={14} />
          <span>View logs</span>
        </button>
        <button
          className="ctx-item"
          onClick={() => {
            useServices.getState().restart(menu.id);
            onClose();
          }}
        >
          {isRunning ? <RotateCw size={14} /> : <Play size={14} />}
          <span>{isRunning ? "Restart" : "Start"}</span>
        </button>
        {cwd && (
          <button
            className="ctx-item"
            onClick={() => {
              void revealPath(cwd).catch(() => {});
              onClose();
            }}
          >
            <FolderOpen size={14} />
            <span>Open folder</span>
          </button>
        )}
        {command && (
          <button
            className="ctx-item"
            onClick={() => {
              void navigator.clipboard.writeText(command).catch(() => {});
              onClose();
            }}
          >
            <Copy size={14} />
            <span>Copy command</span>
          </button>
        )}
        {isRunning && (
          <>
            <div className="ctx-sep" />
            <button
              className="ctx-item danger"
              onClick={() => {
                useServices.getState().stop(menu.id);
                onClose();
              }}
            >
              <Square size={14} />
              <span>Stop</span>
            </button>
          </>
        )}
      </div>
    </>
  );
}

// Live indicator for background services. Hidden when nothing's running; left-click opens logs (a
// single service) or a picker list (several); right-click opens a full actions menu.
function ServicesIndicator() {
  const running = useServices((s) => s.running);
  const ids = Object.keys(running);
  const [open, setOpen] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);
  if (ids.length === 0) return null;
  const openLogs = (id: string) => {
    useUi.getState().openServiceLogs({ id, name: running[id]?.name || "service" });
    setOpen(false);
  };
  const showMenu = (e: ReactMouseEvent, id: string) => {
    e.preventDefault();
    e.stopPropagation();
    setOpen(false);
    setMenu({ x: e.clientX, y: e.clientY, id });
  };
  const label = ids.length === 1 ? running[ids[0]]?.name || "service" : `${ids.length} services`;
  return (
    <div className="tb-svc" ref={ref}>
      <button
        className="tb-svc-btn"
        title={`${ids.length} background service${ids.length > 1 ? "s" : ""} running — click for logs, right-click for actions`}
        onClick={() => (ids.length === 1 ? openLogs(ids[0]) : setOpen((o) => !o))}
        onContextMenu={(e) => {
          if (ids.length === 1) showMenu(e, ids[0]);
          else {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        <span className="tb-svc-dot" />
        <span className="tb-svc-label">{label}</span>
      </button>
      {open && ids.length > 1 && (
        <div className="tb-menu-pop tb-svc-pop">
          {ids.map((id) => (
            <button
              key={id}
              className="tb-menu-item"
              onClick={() => openLogs(id)}
              onContextMenu={(e) => showMenu(e, id)}
            >
              <span className="tb-svc-dot" />
              {running[id]?.name || "service"}
            </button>
          ))}
        </div>
      )}
      {menu && <ServiceCtxMenu menu={menu} onClose={() => setMenu(null)} />}
    </div>
  );
}

// Live indicator for running loops. Hidden when none are active; click jumps to Settings → Loops.
function LoopsIndicator() {
  const runs = useLoops((s) => s.runs);
  const loops = useLoops((s) => s.loops);
  const active = Object.keys(runs).filter((id) => {
    const st = runs[id]?.status;
    return st === "running" || st === "paused";
  });
  if (active.length === 0) return null;
  const only = active.length === 1 ? loops[active[0]] : null;
  const iter = active.length === 1 ? runs[active[0]]?.iteration : 0;
  const label = only ? `${only.name || "loop"}${iter ? ` · ${iter}` : ""}` : `${active.length} loops`;
  const running = active.some((id) => runs[id]?.status === "running");
  return (
    <div className="tb-svc">
      <button
        className={`tb-svc-btn${running ? "" : " paused"}`}
        title={`${active.length} loop${active.length > 1 ? "s" : ""} active — click to manage`}
        onClick={() => useUi.getState().goLoops()}
      >
        <span className={`tb-svc-dot${running ? " spin" : ""}`} />
        <span className="tb-svc-label">{label}</span>
      </button>
    </div>
  );
}

export function Titlebar() {
  const go = () => useUi.getState().goSpace();
  const openProjectFolder = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    const n = folder.split(/[\\/]/).filter(Boolean).pop() || "Project";
    useWorkspaces.getState().addWorkspace(n, folder);
    go();
  };

  const newItems: MenuItem[] = [
    { label: "Launch workspace…", icon: <Rocket size={14} />, onClick: () => useUi.getState().openLaunch() },
    { label: "Claude", icon: <Sparkles size={14} />, onClick: () => { void newClaude(); go(); } },
    { label: "Gemini", icon: <Gem size={14} />, onClick: () => { void newGemini(); go(); } },
    { label: "Codex", icon: <Bot size={14} />, onClick: () => { void newCodex(); go(); } },
    ...(isWindows
      ? [{ label: "WSL (Linux)", icon: <SquareTerminal size={14} />, onClick: () => { void newWsl(); go(); } }]
      : []),
    { label: "Terminal", icon: <Terminal size={14} />, onClick: () => { void newTerminal(); go(); } },
    {
      label: "Claude in worktree",
      icon: <GitBranch size={14} />,
      onClick: () => {
        void newClaudeInWorktree();
        go();
      },
    },
  ];
  const openItems: MenuItem[] = [
    {
      label: "New project…",
      icon: <FolderPlus size={14} />,
      onClick: () => useUi.getState().openNewProject(),
    },
    { label: "Open project folder…", icon: <FolderOpen size={14} />, onClick: () => void openProjectFolder() },
    {
      label: isWindows ? "Open folder in Explorer" : isMac ? "Open folder in Finder" : "Open folder",
      icon: <ExternalLink size={14} />,
      onClick: () => {
        const w = useWorkspaces.getState();
        const a = w.workspaces.find((x) => x.id === w.activeId);
        const folder = a?.sessions.find((s) => s.id === w.focusedSessionId)?.cwd || a?.cwd || "";
        if (folder) void revealPath(folder).catch(() => {});
      },
    },
    {
      label: "New open space",
      icon: <LayoutGrid size={14} />,
      onClick: () => {
        useWorkspaces.getState().addOpenSpace();
        go();
      },
    },
  ];

  // git actions target the focused pane's folder (or the active space's) — only show for a repo,
  // and only while you're actually in a space (the git menu makes no sense on the home screen)
  const view = useUi((s) => s.view);
  const activeWs = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId));
  const focusedId = useWorkspaces((s) => s.focusedSessionId);
  const repoCwd = activeWs?.sessions.find((s) => s.id === focusedId)?.cwd || activeWs?.cwd || "";
  const gitItems: MenuItem[] = [
    { label: "Commit & push…", icon: <Upload size={14} />, onClick: () => useGit.getState().openCommit(true) },
    { label: "Commit…", icon: <GitCommitVertical size={14} />, onClick: () => useGit.getState().openCommit(false) },
    { label: "Push", icon: <ArrowUp size={14} />, onClick: () => void useGit.getState().push() },
    { label: "Create PR", icon: <GitPullRequest size={14} />, onClick: () => void useGit.getState().createPr() },
  ];

  // is the active folder actually a git repo? gates the button (re-checks after a git init)
  const repoTick = useGit((s) => s.repoTick);
  const [isRepo, setIsRepo] = useState(false);
  useEffect(() => {
    if (!repoCwd) {
      setIsRepo(false);
      return;
    }
    let cancelled = false;
    gitIsRepo(repoCwd)
      .then((r) => {
        if (!cancelled) setIsRepo(r);
      })
      .catch(() => {
        if (!cancelled) setIsRepo(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoCwd, repoTick]);

  return (
    <div className={`titlebar${isMac ? " mac" : ""}`} data-tauri-drag-region>
      <div className="tb-left" data-tauri-drag-region>
        <button className="tb-home" title="Home" onClick={() => useUi.getState().goHome()}>
          <span className="tb-logo">
            <Logo size={16} />
          </span>
          <span className="tb-brand">HyprSpace</span>
        </button>
      </div>

      <div className="tb-controls" data-tauri-drag-region>
        <div className="tb-actions">
          <ActionMenu label="New" lead={<Plus size={14} />} items={newItems} />
          <ActionMenu label="Open" lead={<FolderOpen size={14} />} items={openItems} />
          {view === "space" &&
            repoCwd &&
            (isRepo ? (
              <ActionMenu label="Commit & push" lead={<GitBranch size={14} />} items={gitItems} />
            ) : (
              <ActionMenu
                label="Init repo"
                lead={<GitBranch size={14} />}
                items={[
                  {
                    label: "Initialize git repository",
                    icon: <GitBranch size={14} />,
                    onClick: () => void useGit.getState().init(),
                  },
                ]}
              />
            ))}
        </div>
        <LoopsIndicator />
        <ServicesIndicator />
        <NotificationPanel />
        <button
          className="tb-ctl"
          title="Review dock — changes & run (Ctrl+Shift+G)"
          onClick={() => useUi.getState().toggleDock()}
        >
          <PanelRight size={14} strokeWidth={1.75} />
        </button>
        {/* Windows/Linux: our own controls. macOS draws native traffic lights instead. */}
        {!isMac && (
          <>
            <button className="tb-ctl" title="Minimize" onClick={() => win.minimize()}>
              <svg width="10" height="10" viewBox="0 0 10 10">
                <rect x="1" y="4.5" width="8" height="1" fill="currentColor" />
              </svg>
            </button>
            <button className="tb-ctl" title="Maximize" onClick={() => win.toggleMaximize()}>
              <svg width="10" height="10" viewBox="0 0 10 10">
                <rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" />
              </svg>
            </button>
            <button className="tb-ctl close" title="Close" onClick={() => win.close()}>
              <svg width="10" height="10" viewBox="0 0 10 10">
                <path d="M1 1 L9 9 M9 1 L1 9" stroke="currentColor" strokeWidth="1.2" />
              </svg>
            </button>
          </>
        )}
      </div>
    </div>
  );
}
