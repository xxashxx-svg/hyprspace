import { useEffect, useRef, useState, type ReactNode, type MouseEvent as ReactMouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useGit } from "../stores/git";
import { useServices } from "../stores/services";
import { useLoops } from "../stores/loops";
import { useProjectConfigs, folderKey, type Action } from "../stores/projectConfig";
import { useActionEditor } from "../stores/actionEditor";
import { runAction } from "../lib/startup";
import { isMac, isWindows } from "../platform";
import { pickFolder, gitIsRepo, revealPath } from "../api";
import { newClaude, newGemini, newCodex, newOpencode, newGrok, newWsl, newTerminal, newClaudeInWorktree } from "../actions";
import {
  PanelRight,
  Plus,
  ChevronDown,
  Sparkles,
  Gem,
  Bot,
  SquareCode,
  Atom,
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
  Zap,
  Loader2,
  Check,
  X,
} from "lucide-react";

const EMPTY_ACTIONS: Action[] = [];
import { Logo } from "./Logo";
import { NotificationPanel } from "./NotificationPanel";
import { LayoutPicker } from "./LayoutPicker";

const win = getCurrentWindow();

type MenuItem = { label: string; icon?: ReactNode; onClick: () => void };

// sidebar toggle glyph: the left rail is solid while the sidebar is out and hollow once it's tucked
// away — carrying the state in the fill keeps it two shapes, which stays readable at 16px
function SidebarIcon({ open }: { open: boolean }) {
  return (
    <svg
      width={16}
      height={16}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="3" y="3" width="18" height="18" rx="2" />
      {open ? (
        <path d="M5 3h4v18H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="currentColor" stroke="none" />
      ) : (
        <path d="M9 3v18" />
      )}
    </svg>
  );
}

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

// Live indicator for running automations. Hidden when none are active; click jumps to the Automations page.
// primitive selectors only — `runs` re-mints on every streamed batch, and this sits in the titlebar
function LoopsIndicator() {
  const activeCount = useLoops((s) => {
    let n = 0;
    for (const r of Object.values(s.runs)) if (r.status === "running" || r.status === "paused") n++;
    return n;
  });
  const running = useLoops((s) => Object.values(s.runs).some((r) => r.status === "running"));
  // "name · iteration" when exactly one automation is active, null otherwise
  const soloLabel = useLoops((s) => {
    let id: string | null = null;
    for (const [k, r] of Object.entries(s.runs)) {
      if (r.status === "running" || r.status === "paused") {
        if (id) return null;
        id = k;
      }
    }
    if (!id) return null;
    const iter = s.runs[id].iteration;
    return `${s.loops[id]?.name || "automation"}${iter ? ` · ${iter}` : ""}`;
  });
  if (activeCount === 0) return null;
  const label = soloLabel ?? `${activeCount} automations`;
  return (
    <div className="tb-loop">
      <button
        className={`tb-loop-btn${running ? "" : " paused"}`}
        title={`${activeCount} automation${activeCount > 1 ? "s" : ""} active — click to manage`}
        onClick={() => useUi.getState().goLoops()}
      >
        <span className={`tb-loop-dot${running ? " live" : ""}`} />
        <span className="tb-loop-label">{label}</span>
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

  // macOS won't move the window via data-tauri-drag-region when titleBarStyle is Overlay (tauri #9503),
  // so we drive the drag ourselves. Skip clicks that land on a button/menu/input so they still work.
  const onChrome = (e: ReactMouseEvent) =>
    !(e.target as HTMLElement).closest("button, a, input, select, textarea, [role='menuitem']");
  // the top-left traffic-light corner is native (not DOM), so onChrome can't see it. starting a drag
  // there swallows the mousedown meant for the close/min/max buttons — the long-standing macOS
  // "can't close the window" flakiness. leave that corner entirely to the native buttons.
  const inTrafficLights = (e: ReactMouseEvent) => e.clientX < 80 && e.clientY < 36;
  const onTbDown = (e: ReactMouseEvent) => {
    if (inTrafficLights(e)) return;
    if (e.button === 0 && onChrome(e)) void win.startDragging().catch(() => {});
  };
  const onTbDblClick = (e: ReactMouseEvent) => {
    if (inTrafficLights(e)) return;
    if (onChrome(e)) void win.toggleMaximize().catch(() => {});
  };

  const newItems: MenuItem[] = [
    { label: "Launch workspace…", icon: <Rocket size={14} />, onClick: () => useUi.getState().openLaunch() },
    { label: "Claude", icon: <Sparkles size={14} />, onClick: () => { void newClaude(); go(); } },
    { label: "Gemini", icon: <Gem size={14} />, onClick: () => { void newGemini(); go(); } },
    { label: "Codex", icon: <Bot size={14} />, onClick: () => { void newCodex(); go(); } },
    { label: "OpenCode", icon: <SquareCode size={14} />, onClick: () => { void newOpencode(); go(); } },
    { label: "Grok", icon: <Atom size={14} />, onClick: () => { void newGrok(); go(); } },
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
  const railHidden = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);
  const settingsOpen = useUi((s) => s.settingsOpen);
  const activeWs = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId));
  const focusedId = useWorkspaces((s) => s.focusedSessionId);
  const repoCwd = activeWs?.sessions.find((s) => s.id === focusedId)?.cwd || activeWs?.cwd || "";
  // project-scoped actions for the active project's folder (on-demand commands)
  const projFolder = activeWs && activeWs.kind !== "open" ? activeWs.cwd : "";
  const projActions = useProjectConfigs((s) =>
    projFolder ? s.configs[folderKey(projFolder)]?.startup ?? EMPTY_ACTIONS : EMPTY_ACTIONS,
  );
  const actionItems: MenuItem[] = [
    ...projActions.map((a) => ({
      label: a.name || a.command || "action",
      icon: <Play size={13} />,
      onClick: () => activeWs && runAction(activeWs.id, a),
    })),
    {
      label: "Add action…",
      icon: <Plus size={14} />,
      onClick: () =>
        projFolder && useActionEditor.getState().openEditor(projFolder, { wsId: activeWs?.id }),
    },
  ];

  const gitItems: MenuItem[] = [
    { label: "Commit & push…", icon: <Upload size={14} />, onClick: () => useGit.getState().openCommit(true) },
    { label: "Commit…", icon: <GitCommitVertical size={14} />, onClick: () => useGit.getState().openCommit(false) },
    { label: "Push", icon: <ArrowUp size={14} />, onClick: () => void useGit.getState().push() },
    { label: "Create PR…", icon: <GitPullRequest size={14} />, onClick: () => void useGit.getState().openPr() },
  ];

  // is the active folder actually a git repo? gates the button (re-checks after a git init)
  const repoTick = useGit((s) => s.repoTick);
  const gitAct = useGit((s) => s.activity); // live "Pushing… / Pushed" status on the git button
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
    <div
      className={`titlebar${isMac ? " mac" : ""}`}
      data-tauri-drag-region
      onMouseDown={isMac ? onTbDown : undefined}
      onDoubleClick={isMac ? onTbDblClick : undefined}
    >
      <div className="tb-left" data-tauri-drag-region>
        <button className="tb-home" title="Home" onClick={() => useUi.getState().goHome()}>
          <span className="tb-logo">
            <Logo size={16} />
          </span>
        </button>
        <button
          className="tb-sidebar"
          title={railHidden ? "Show sidebar" : "Hide sidebar"}
          onClick={toggleRail}
        >
          <SidebarIcon open={!railHidden} />
        </button>
      </div>

      <div className="tb-controls" data-tauri-drag-region>
        {/* workspace action dropdowns — irrelevant on the settings screen, so hide them there */}
        {!settingsOpen && (
        <div className="tb-actions">
          <ActionMenu label="New" lead={<Plus size={14} />} items={newItems} />
          <ActionMenu label="Open" lead={<FolderOpen size={14} />} items={openItems} />
          {view === "space" && repoCwd && (
            <ActionMenu
              label={gitAct ? gitAct.label : isRepo ? "Commit & push" : "Init repo"}
              lead={
                gitAct ? (
                  gitAct.kind === "busy" ? (
                    <Loader2 size={14} className="tb-spin" />
                  ) : gitAct.kind === "ok" ? (
                    <Check size={14} className="tb-git-ok" />
                  ) : (
                    <X size={14} className="tb-git-err" />
                  )
                ) : (
                  <GitBranch size={14} />
                )
              }
              items={
                isRepo
                  ? gitItems
                  : [
                      {
                        label: "Initialize git repository…",
                        icon: <GitBranch size={14} />,
                        onClick: () => useGit.getState().openInitRepo(),
                      },
                    ]
              }
            />
          )}
          {view === "space" && projFolder && (
            <ActionMenu label="Actions" lead={<Zap size={14} />} items={actionItems} />
          )}
          <LayoutPicker />
        </div>
        )}
        <LoopsIndicator />
        <ServicesIndicator />
        <NotificationPanel />
        {view === "space" && (
          <button
            className="tb-ctl"
            title="Review dock — changes & run (Ctrl+Shift+G)"
            onClick={() => useUi.getState().toggleDock()}
          >
            <PanelRight size={14} strokeWidth={1.75} />
          </button>
        )}
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
