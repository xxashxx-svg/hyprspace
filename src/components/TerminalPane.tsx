import { useEffect, useRef, useState, memo } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { makeTerminal, termTheme, attachGpuRenderer } from "../terminal/createTerminal";
import { useSettings } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { useProjectConfigs } from "../stores/projectConfig";
import { useUi } from "../stores/ui";
import { useNotifications } from "../stores/notifications";
import { claudeCmd } from "../actions";
import { createPty, writePty, resizePty, killPty, claudeResumeMode, revealPath, worktreeCreate } from "../api";
import { appendOutput, dropOutput, recentOutput } from "../terminal/buffers";
import { noteUserInput, forgetSession } from "../ai/autoNameSession";
import { useActivity } from "../stores/activity";
import {
  Maximize2,
  Minimize2,
  X,
  FolderSymlink,
  Sparkles,
  Gem,
  Bot,
  SquareTerminal,
  SquareCode,
  Terminal as TerminalIcon,
  MoreHorizontal,
  Copy,
  FolderOpen,
  ClipboardList,
  GitBranch,
  GitPullRequestArrow,
  SplitSquareHorizontal,
} from "lucide-react";
import { TerminalSearch } from "./TerminalSearch";

// small lucide glyph per provider, shown at the start of the pane header
const PROVIDER_ICONS = {
  claude: Sparkles,
  gemini: Gem,
  codex: Bot,
  opencode: SquareCode,
  wsl: SquareTerminal,
  terminal: TerminalIcon,
} as const;

// friendly name for the brief "Starting …" boot indicator
const PROVIDER_LABEL = {
  claude: "Claude",
  gemini: "Gemini",
  codex: "Codex",
  opencode: "OpenCode",
  wsl: "WSL",
  terminal: "terminal",
} as const;

// Each claude pane owns its session id (= the pane's uuid), so on relaunch it resumes its OWN
// conversation — not just "the folder's latest", which broke open spaces with several panes in
// one folder. We claim the id with --session-id on first launch, then --resume <id> to return.
function injectClaudeArg(cmd: string, arg: string): string {
  if (/--session-id|--resume|--continue|(^|\s)-[cr](\s|$)/.test(cmd)) return cmd; // already pinned
  return cmd.replace(/^claude\b/, `claude ${arg}`);
}

interface Props {
  sessionId: string;
  wsId: string;
  cwd: string;
  guest?: boolean;
  command?: string;
  provider: "claude" | "gemini" | "codex" | "opencode" | "wsl" | "terminal";
  title?: string;
  started?: boolean;
  active: boolean;
  focused: boolean;
  isMaxed: boolean;
  // id-param callbacks so PaneGrid can pass ONE stable reference per handler (not a fresh closure
  // per pane per render) — that's what lets the memo below actually skip re-renders.
  onFocus: (sid: string) => void;
  onClose: (wsId: string, sid: string) => void;
  onToggleMax: (sid: string) => void;
  onGripDown: (e: RPointerEvent<HTMLDivElement>, wsId: string, sid: string) => void;
  onGripMove: (e: RPointerEvent<HTMLDivElement>) => void;
  onGripUp: (e: RPointerEvent<HTMLDivElement>) => void;
}

function TerminalPaneInner({
  sessionId,
  wsId,
  cwd,
  guest,
  command,
  provider,
  title,
  started,
  active,
  focused,
  isMaxed,
  onFocus,
  onClose,
  onToggleMax,
  onGripDown,
  onGripMove,
  onGripUp,
}: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const [alive, setAlive] = useState(true);
  const [booting, setBooting] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const themeId = useSettings((s) => s.theme);
  const fontSize = useSettings((s) => s.fontSize);
  const fontFamily = useSettings((s) => s.fontFamily);
  const lineHeight = useSettings((s) => s.lineHeight);
  const cursorStyle = useSettings((s) => s.cursorStyle);
  const cursorBlink = useSettings((s) => s.cursorBlink);

  const isClaude = provider === "claude";

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const term = makeTerminal(isClaude);
    const fit = new FitAddon();
    const search = new SearchAddon();
    term.loadAddon(fit);
    term.loadAddon(search);
    // ctrl/cmd + click opens links in the system browser, like Windows Terminal.
    // plain click does nothing so it doesn't fight text selection.
    const links = new WebLinksAddon((e, uri) => {
      if (e.ctrlKey || e.metaKey) void openUrl(uri).catch(() => {});
    });
    term.loadAddon(links);
    searchRef.current = search;
    term.open(el);
    attachGpuRenderer(term); // GPU (WebGL) renderer if enabled — must come after open()

    // Ctrl+Shift+F search · Ctrl+C copies selection (else SIGINT) · Ctrl+V / Ctrl+Shift+V paste
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown" || !e.ctrlKey || e.altKey) return true;
      if (e.shiftKey && e.code === "KeyF") {
        setShowSearch((p) => !p);
        return false;
      }
      if (e.code === "KeyC") {
        if (term.hasSelection()) {
          const sel = term.getSelection();
          if (sel) void writeText(sel);
          term.clearSelection();
          return false;
        }
        return !e.shiftKey; // nothing selected: plain Ctrl+C interrupts; Ctrl+Shift+C no-ops
      }
      if (e.code === "KeyV") {
        void readText()
          .then((t) => {
            if (t) term.paste(t);
          })
          .catch(() => {});
        return false;
      }
      return true;
    });

    let disposed = false;
    // Renderer is a setting (Settings → Terminal): GPU/WebGL (default) draws block art seamlessly
    // like Alacritty; DOM gives crisp ClearType text but needs line-height 1.0 for clean blocks.
    fit.fit();
    termRef.current = term;
    fitRef.current = fit;

    // A pane can open before the bundled webfont finishes loading, so WebGL rasterizes its
    // glyph atlas with a fallback font → garbled text. Rebuild the atlas once fonts are ready.
    document.fonts?.ready?.then(() => {
      if (disposed) return;
      try {
        term.clearTextureAtlas?.();
        fit.fit();
        term.refresh(0, term.rows - 1);
      } catch {
        /* not ready */
      }
    });

    // show a brief "starting…" overlay only when the shell/agent is slow to print its first byte
    // (cold launch). warm panes output within ~250ms and never flash it.
    let gotOutput = false;
    const markBooted = () => {
      if (gotOutput) return;
      gotOutput = true;
      clearTimeout(bootShow);
      clearTimeout(bootMax);
      if (!disposed) setBooting(false);
    };
    const bootShow = setTimeout(() => {
      if (!gotOutput && !disposed) setBooting(true);
    }, 250);
    const bootMax = setTimeout(markBooted, 20000); // never leave the overlay stuck

    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const dataDisp = term.onData((d) => {
      noteUserInput(sessionId, d); // capture the first prompt for the auto-namer (T3-style)
      void writePty(sessionId, enc.encode(d));
    });
    // copy-on-select (opt-in) — via the Tauri clipboard so it works inside the webview
    const selDisp = term.onSelectionChange(() => {
      if (useSettings.getState().copyOnSelect && term.hasSelection()) {
        const sel = term.getSelection();
        if (sel) void writeText(sel);
      }
    });

    useActivity.getState().markStart(sessionId);
    // apply the owning project's per-project env vars + default shell, if set
    const ownerWs = useWorkspaces
      .getState()
      .workspaces.find((w) => w.sessions.some((s) => s.id === sessionId));
    const cfg = ownerWs ? useProjectConfigs.getState().getConfig(ownerWs.cwd) : null;
    const projEnv = cfg && Object.keys(cfg.env).length ? cfg.env : undefined;
    createPty(
      {
        id: sessionId,
        cwd,
        args: [],
        cols: term.cols,
        rows: term.rows,
        ...(projEnv ? { env: projEnv } : {}),
        ...(cfg?.defaultShell ? { shell: cfg.defaultShell } : {}),
      },
      {
        onData: (bytes) => {
          if (disposed) return;
          markBooted();
          term.write(bytes);
          appendOutput(sessionId, dec.decode(bytes, { stream: true }));
          useActivity.getState().markOutput(sessionId);
        },
        onControl: (c) => {
          if (!disposed && c.type === "exit") {
            markBooted();
            setAlive(false);
            useActivity.getState().markExit(sessionId);
            term.write(`\r\n\x1b[38;5;245m[process exited: ${c.code}]\x1b[0m\r\n`);
          }
        },
      },
    )
      .then(async () => {
        if (!command) return;
        let toRun = command;
        if (isClaude && started) {
          // each pane owns its conversation under its own id, so resume that exact chat reliably;
          // panes created before we owned the id fall back to the folder's latest, else fresh
          const mode = await claudeResumeMode(cwd, sessionId).catch(() => "fresh");
          if (mode === "resume") toRun = injectClaudeArg(command, `--resume ${sessionId}`);
          else if (mode === "continue") toRun = injectClaudeArg(command, "--continue");
          else toRun = injectClaudeArg(command, `--session-id ${sessionId}`);
        } else if (isClaude) {
          // first launch: claim this pane's uuid as claude's session id so we can resume it later
          toRun = injectClaudeArg(command, `--session-id ${sessionId}`);
        }
        setTimeout(() => {
          if (disposed) return;
          void writePty(sessionId, enc.encode(toRun + "\r"));
          if (isClaude && !started) useWorkspaces.getState().markStarted(sessionId);
        }, 700);
      })
      .catch((e) => term.write(`\r\n\x1b[31mfailed to start: ${e}\x1b[0m\r\n`));

    let raf = 0;
    let tSettled: ReturnType<typeof setTimeout> | undefined;

    // Robust resize: sync to RAF for smoothness, but also trigger a debounced "settle" fit
    // to ensure the PTY gets the exact final pixel dimensions after grid animations finish.
    const syncFit = () => {
      if (el.clientWidth < 40 || el.clientHeight < 20) return; // guard against 0-size intermediate frames
      const atBottom = term.buffer.active.viewportY === term.buffer.active.baseY;
      try {
        fit.fit();
        void resizePty(sessionId, term.cols, term.rows);
        if (atBottom) term.scrollToBottom();
      } catch {
        /* not ready */
      }
    };

    const ro = new ResizeObserver(() => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        syncFit();
        // grid layouts can have a 100-150ms "spring" or transition; ensure we capture the end.
        clearTimeout(tSettled);
        tSettled = setTimeout(syncFit, 150);
      });
    });
    ro.observe(el);

    // Ctrl+scroll zooms the font (persisted, so every pane tracks the same size). capture phase +
    // stopPropagation so xterm's scrollback viewport can't eat the scroll-up (that broke zoom-in:
    // scrolling up had history to consume, scrolling down at the bottom didn't — hence the asymmetry).
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      e.stopPropagation();
      const cur = useSettings.getState().fontSize;
      useSettings.getState().setFontSize(cur + (e.deltaY > 0 ? -1 : 1));
    };
    el.addEventListener("wheel", onWheel, { passive: false, capture: true });

    // after sleep/resume or refocus the WebGL atlas can go stale → ghost cursor; invalidate + repaint
    const redraw = () => {
      try {
        term.clearTextureAtlas?.();
        requestAnimationFrame(() => {
          try {
            term.refresh(0, term.rows - 1);
          } catch {
            /* not ready */
          }
        });
      } catch {
        /* not ready */
      }
    };
    const onVis = () => {
      if (document.visibilityState === "visible") redraw();
    };
    document.addEventListener("visibilitychange", onVis);
    window.addEventListener("focus", redraw);

    return () => {
      disposed = true;
      if (raf) cancelAnimationFrame(raf);
      clearTimeout(tSettled);
      clearTimeout(bootShow);
      clearTimeout(bootMax);
      ro.disconnect();
      el.removeEventListener("wheel", onWheel, { capture: true });
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("focus", redraw);
      dataDisp.dispose();
      selDisp.dispose();
      links.dispose();
      search.dispose();
      searchRef.current = null;
      dropOutput(sessionId);
      forgetSession(sessionId); // drop the auto-namer's capture state for this pane
      void killPty(sessionId);
      term.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // re-fit when revealed or when maximize toggles the pane size
  useEffect(() => {
    if (!active) return;
    const id = requestAnimationFrame(() => {
      const term = termRef.current;
      const fit = fitRef.current;
      if (!term || !fit) return;
      try {
        fit.fit();
        void resizePty(sessionId, term.cols, term.rows);
      } catch {
        /* not ready */
      }
      term.scrollToBottom();
    });
    return () => cancelAnimationFrame(id);
  }, [active, isMaxed, sessionId]);

  useEffect(() => {
    if (focused) termRef.current?.focus();
  }, [focused]);

  // live theme change → re-skin the terminal
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.theme = termTheme();
    try {
      t.clearTextureAtlas?.();
      t.refresh(0, t.rows - 1);
    } catch {
      /* renderer not ready */
    }
  }, [themeId]);

  // live font change → re-fit + resize the pty
  useEffect(() => {
    const t = termRef.current;
    const f = fitRef.current;
    if (!t) return;
    t.options.fontFamily = fontFamily;
    t.options.fontSize = fontSize;
    t.options.lineHeight = lineHeight ?? 1.1;
    try {
      f?.fit();
      void resizePty(sessionId, t.cols, t.rows);
    } catch {
      /* not ready */
    }
  }, [fontFamily, fontSize, lineHeight, sessionId]);

  // live cursor change
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.cursorStyle = cursorStyle;
    t.options.cursorBlink = isClaude ? false : cursorBlink;
  }, [cursorStyle, cursorBlink, isClaude]);

  const folder = cwd.split(/[\\/]/).filter(Boolean).pop();
  const PIcon = PROVIDER_ICONS[provider] ?? TerminalIcon;

  // ---- pane actions menu (… button / right-click the header) ----
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const note = (title: string) => useNotifications.getState().add({ title });
  const openMenuAt = (e: RMouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: Math.min(e.clientX, window.innerWidth - 210), y: e.clientY });
  };
  const openMenuFromBtn = (e: RMouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setMenu({ x: Math.min(r.right - 196, window.innerWidth - 210), y: r.bottom + 4 });
  };
  const newWorktreeHere = async () => {
    try {
      const ws = useWorkspaces.getState().workspaces.find((w) => w.id === wsId);
      const path = await worktreeCreate(cwd, `agent-${(ws?.sessions.length ?? 0) + 1}`);
      useWorkspaces.getState().addSession(wsId, claudeCmd(), path);
    } catch (e) {
      note(`Couldn't create worktree: ${e}`);
    }
  };

  // right-click = paste (xterm.paste handles bracketed paste so multi-line input won't pre-submit)
  const handlePaste = (e: RMouseEvent) => {
    e.preventDefault();
    readText()
      .then((text) => {
        if (text) termRef.current?.paste(text);
      })
      .catch(() => {});
  };

  return (
    <div
      className={`terminal-pane ${focused ? "focused" : ""} p-${provider}${guest ? " guest" : ""}`}
      onMouseDown={() => onFocus(sessionId)}
    >
      <div
        className="pane-header"
        onPointerDown={(e) => onGripDown(e, wsId, sessionId)}
        onPointerMove={onGripMove}
        onPointerUp={onGripUp}
        onContextMenu={openMenuAt}
      >
        <span className="pane-head-left">
          {!alive && <span className="pane-status" title="process exited" />}
          <PIcon size={12} className="pane-prov-ico" />
          <span className="pane-title">{title || provider}</span>
          {folder && (
            <span
              className={`pane-cwd${guest ? " guest" : ""}`}
              title={guest ? `${cwd} — outside this space's folder` : cwd}
            >
              {guest && <FolderSymlink size={11} className="pane-cwd-ico" />}· {folder}
            </span>
          )}
        </span>
        <span className="pane-head-right">
          <button
            className="pane-btn"
            title={`Split — add another ${provider === "terminal" ? "terminal" : provider} pane here`}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => useWorkspaces.getState().addSession(wsId, command, cwd)}
          >
            <SplitSquareHorizontal size={12} />
          </button>
          <button
            className="pane-btn"
            title="Pane actions"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={openMenuFromBtn}
          >
            <MoreHorizontal size={13} />
          </button>
          <button
            className="pane-btn"
            title={isMaxed ? "Restore" : "Maximize"}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => onToggleMax(sessionId)}
          >
            {isMaxed ? <Minimize2 size={12} /> : <Maximize2 size={12} />}
          </button>
          <button
            className="pane-btn close"
            title="Close pane"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={() => onClose(wsId, sessionId)}
          >
            <X size={13} />
          </button>
        </span>
      </div>
      {showSearch && searchRef.current && (
        <TerminalSearch
          search={searchRef.current}
          onClose={() => {
            setShowSearch(false);
            termRef.current?.focus();
          }}
        />
      )}
      <div className="pane-term" ref={ref} onContextMenu={handlePaste} />
      {booting && (
        <div className="pane-boot" aria-hidden>
          <span className="pane-boot-spin" />
          <span className="pane-boot-text">Starting {PROVIDER_LABEL[provider] ?? "session"}…</span>
        </div>
      )}
      {menu && (
        <>
          <div
            className="pane-menu-backdrop"
            onMouseDown={(e) => {
              e.stopPropagation();
              setMenu(null);
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              setMenu(null);
            }}
          />
          <div className="pane-menu" style={{ left: menu.x, top: menu.y }} onMouseDown={(e) => e.stopPropagation()}>
            <button
              className="pane-menu-item"
              onClick={() => {
                void writeText(cwd);
                note("Path copied");
                setMenu(null);
              }}
            >
              <Copy size={14} /> Copy current path
            </button>
            <button
              className="pane-menu-item"
              onClick={() => {
                void revealPath(cwd).catch(() => {});
                setMenu(null);
              }}
            >
              <FolderOpen size={14} /> Open current directory
            </button>
            <button
              className="pane-menu-item"
              onClick={() => {
                void writeText(recentOutput(sessionId, 24000));
                note("Output copied");
                setMenu(null);
              }}
            >
              <ClipboardList size={14} /> Copy output
            </button>
            <div className="pane-menu-sep" />
            <button
              className="pane-menu-item"
              onClick={() => {
                void newWorktreeHere();
                setMenu(null);
              }}
            >
              <GitBranch size={14} /> New worktree from here
            </button>
            <button
              className="pane-menu-item"
              onClick={() => {
                useUi.getState().setDockTab("changes");
                setMenu(null);
              }}
            >
              <GitPullRequestArrow size={14} /> Open Git panel
            </button>
            <div className="pane-menu-sep" />
            <button
              className="pane-menu-item danger"
              onClick={() => {
                onClose(wsId, sessionId);
                setMenu(null);
              }}
            >
              <X size={14} /> Close pane
            </button>
          </div>
        </>
      )}
    </div>
  );
}

// memo: with stable (id-param) callbacks from PaneGrid, a pane only re-renders when ITS own
// active/focused/isMaxed/cwd/etc. change — not when a sibling is focused or a drag updates overId.
export const TerminalPane = memo(TerminalPaneInner);

