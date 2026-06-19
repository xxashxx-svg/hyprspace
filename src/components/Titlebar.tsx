import { useEffect, useRef, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useGit } from "../stores/git";
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

      <div className="tb-controls">
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
