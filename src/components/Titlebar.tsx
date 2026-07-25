import { useEffect, useRef, useState, type ReactNode, type MouseEvent as ReactMouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useGit } from "../stores/git";
import { useLoops } from "../stores/loops";
import { useProjectConfigs, folderKey, type Action } from "../stores/projectConfig";
import { useActionEditor } from "../stores/actionEditor";
import { runAction } from "../lib/startup";
import { isMac, isWindows } from "../platform";
import { pickFolder, gitIsRepo, revealPath } from "../api";
import { launchInActive, newTerminal, newClaudeInWorktree } from "../actions";
import { PROVIDERS } from "../lib/providers";
import {
  PanelRight,
  Plus,
  ChevronDown,
  ChevronRight,
  Sparkles,
  Terminal,
  GitBranch,
  GitCommitVertical,
  GitPullRequest,
  ArrowUp,
  Upload,
  FolderOpen,
  FolderPlus,
  Layers,
  ExternalLink,
  Rocket,
  Play,
  Zap,
  Loader2,
  Check,
  X,
} from "lucide-react";

const EMPTY_ACTIONS: Action[] = [];
import { Logo } from "./Logo";
import { NotificationPanel } from "./NotificationPanel";
import { UsageMeter } from "./UsageMeter";
import { LayoutPicker } from "./LayoutPicker";

const win = getCurrentWindow();

// `items` turns a row into a nested submenu — used to keep the New menu short now that there are
// six agents to choose from.
type MenuItem = {
  label?: string;
  icon?: ReactNode;
  onClick?: () => void;
  items?: MenuItem[];
  head?: boolean; // a small uppercase section label
  sep?: boolean; // a divider
};

// one row of a topbar dropdown; a row with children opens a panel beside it on hover
function TbMenuRow({ item, close }: { item: MenuItem; close: () => void }) {
  /* eslint-disable react-hooks/rules-of-hooks -- head/sep return before hooks, but an item never
     changes kind across renders, so the hook order per row is stable */
  const [open, setOpen] = useState(false);
  const [left, setLeft] = useState(false); // the New button sits near the right edge
  const rowRef = useRef<HTMLDivElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const show = () => {
    clearTimeout(timer.current);
    const r = rowRef.current?.getBoundingClientRect();
    if (r) setLeft(r.right + 190 > window.innerWidth);
    setOpen(true);
  };
  const hide = () => {
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setOpen(false), 160); // survive the diagonal mouse travel
  };
  if (item.sep) return <div className="tb-menu-sep" />;
  if (item.head) return <div className="tb-menu-head">{item.label}</div>;
  if (!item.items) {
    return (
      <button
        className="tb-menu-item"
        onClick={() => {
          close();
          item.onClick?.();
        }}
      >
        {item.icon}
        {item.label}
      </button>
    );
  }
  return (
    <div className="tb-sub" ref={rowRef} onMouseEnter={show} onMouseLeave={hide}>
      <button className="tb-menu-item" onClick={show}>
        {item.icon}
        {item.label}
        <ChevronRight size={13} className="tb-sub-caret" />
      </button>
      {open && (
        <div className={`tb-menu-pop tb-sub-pop${left ? " left" : ""}`} onMouseEnter={show} onMouseLeave={hide}>
          {item.items.map((c) => (
            <TbMenuRow key={c.label} item={c} close={close} />
          ))}
        </div>
      )}
    </div>
  );
}

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
function ActionMenu({
  label,
  lead,
  items,
  prime,
  iconOnly,
  title,
}: {
  label: string;
  lead?: ReactNode;
  items: MenuItem[];
  prime?: boolean; // the one filled call-to-action button
  iconOnly?: boolean; // collapse to a square icon button, label moves to the tooltip
  title?: string;
}) {
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
      <button
        className={`tb-action${prime ? " prime" : ""}${iconOnly ? " icon-only" : ""}`}
        title={title ?? label}
        onClick={() => setOpen((o) => !o)}
      >
        {lead}
        {!iconOnly && <span>{label}</span>}
        {!iconOnly && <ChevronDown size={13} className="tb-action-caret" />}
      </button>
      {open && (
        <div className="tb-menu-pop">
          {items.map((it) => (
            <TbMenuRow key={it.label} item={it} close={() => setOpen(false)} />
          ))}
        </div>
      )}
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
    { head: true, label: "In this space" },
    {
      label: "New agent",
      icon: <Sparkles size={14} />,
      items: PROVIDERS.filter((p) => p.id !== "terminal").map((p) => ({
        label: p.label,
        icon: <p.icon size={14} />,
        onClick: () => {
          void launchInActive(p.cmd());
          go();
        },
      })),
    },
    { label: "Terminal", icon: <Terminal size={14} />, onClick: () => { void newTerminal(); go(); } },
    {
      label: "Claude in worktree",
      icon: <GitBranch size={14} />,
      onClick: () => {
        void newClaudeInWorktree();
        go();
      },
    },
    { label: "Launch workspace…", icon: <Rocket size={14} />, onClick: () => useUi.getState().openLaunch() },
    { sep: true },
    { head: true, label: "Spaces" },
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
      icon: <Layers size={14} />,
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
          <ActionMenu iconOnly label="New" lead={<Plus size={16} />} items={newItems} />
          {view === "space" && repoCwd && (
            <ActionMenu
              iconOnly
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
                  <GitBranch size={16} />
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
            <ActionMenu iconOnly label="Actions" lead={<Zap size={16} />} items={actionItems} />
          )}
          <LayoutPicker />
        </div>
        )}
        <UsageMeter />
        <LoopsIndicator />
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
