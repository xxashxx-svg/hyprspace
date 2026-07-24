import { useCallback, useEffect, useRef, useState, memo } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { Terminal } from "@xterm/xterm";
import type { ILink } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { makeTerminal, termTheme, attachGpuRenderer } from "../terminal/createTerminal";
import { applyUnicode } from "../terminal/unicodeProvider";
import { termSurface } from "../terminal/palettes";
import type { WebglAddon } from "@xterm/addon-webgl";
import { useSettings } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { useProjectConfigs } from "../stores/projectConfig";
import { useUi } from "../stores/ui";
import { useNotifications } from "../stores/notifications";
import { claudeCmd } from "../actions";
import { createPty, writePty, resizePty, pausePty, resumePty, killPty, claudeResumeMode, revealPath, worktreeCreate, clipboardImageToTemp } from "../api";
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
  Atom,
  Terminal as TerminalIcon,
  Image as ImageIcon,
  MoreHorizontal,
  Copy,
  FolderOpen,
  ClipboardList,
  GitBranch,
  GitPullRequestArrow,
  Plus,
} from "lucide-react";
import { TerminalSearch } from "./TerminalSearch";
import { PaneAddMenu } from "./PaneAddMenu";

// small lucide glyph per provider, shown at the start of the pane header
export const PROVIDER_ICONS = {
  claude: Sparkles,
  gemini: Gem,
  codex: Bot,
  opencode: SquareCode,
  grok: Atom,
  wsl: SquareTerminal,
  terminal: TerminalIcon,
  image: ImageIcon,
} as const;

// friendly name for the brief "Starting …" boot indicator
const PROVIDER_LABEL = {
  claude: "Claude",
  gemini: "Gemini",
  codex: "Codex",
  opencode: "OpenCode",
  grok: "Grok",
  wsl: "WSL",
  terminal: "terminal",
  image: "image",
} as const;

// Each claude pane owns its session id (= the pane's uuid), so on relaunch it resumes its OWN
// conversation — not just "the folder's latest", which broke open spaces with several panes in
// one folder. We claim the id with --session-id on first launch, then --resume <id> to return.
function injectClaudeArg(cmd: string, arg: string): string {
  if (/--session-id|--resume|--continue|(^|\s)-[cr](\s|$)/.test(cmd)) return cmd; // already pinned
  return cmd.replace(/^claude\b/, `claude ${arg}`);
}

// image-file paths in terminal output become ctrl+clickable — matches windows/posix absolute +
// relative paths ending in an image extension (a run of non-space/quote chars before the ext)
const IMG_RE = /[^\s"'<>|]+\.(?:png|jpe?g|gif|webp|bmp|svg)\b/gi;

// resolve a matched path against the pane cwd: absolute stays as-is, relative joins onto cwd
// (separator-aware, so a windows cwd keeps backslashes and a posix cwd keeps forward slashes)
function resolveImgPath(p: string, cwd: string): string {
  if (/^[A-Za-z]:[\\/]/.test(p) || /^[\\/]/.test(p)) return p;
  const rel = p.replace(/^\.[\\/]/, "");
  const sep = cwd.includes("\\") ? "\\" : "/";
  const base = cwd.replace(/[\\/]+$/, "");
  return base ? `${base}${sep}${rel}` : rel;
}

interface Props {
  sessionId: string;
  wsId: string;
  cwd: string;
  guest?: boolean;
  command?: string;
  provider: "claude" | "gemini" | "codex" | "opencode" | "grok" | "wsl" | "terminal" | "image";
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
  const glRef = useRef<WebglAddon | null>(null);
  const [alive, setAlive] = useState(true);
  const [booting, setBooting] = useState(false);
  const [showSearch, setShowSearch] = useState(false);
  const themeId = useSettings((s) => s.theme);
  const terminalTheme = useSettings((s) => s.terminalTheme);
  const fontSize = useSettings((s) => s.fontSize);
  const fontFamily = useSettings((s) => s.fontFamily);
  const lineHeight = useSettings((s) => s.lineHeight);
  const cursorStyle = useSettings((s) => s.cursorStyle);
  const cursorBlink = useSettings((s) => s.cursorBlink);
  const gpuRender = useSettings((s) => s.gpuRender);

  const isClaude = provider === "claude";

  // skip no-op resize_pty invokes — fit() usually lands on the same cols/rows
  const lastSize = useRef({ cols: -1, rows: -1 });
  const fitResize = useCallback(
    (cols: number, rows: number) => {
      if (cols === lastSize.current.cols && rows === lastSize.current.rows) return;
      lastSize.current = { cols, rows };
      void resizePty(sessionId, cols, rows);
    },
    [sessionId],
  );

  // the refocus redraw listener below only repaints visible panes; ref so it sees the latest
  const activeRef = useRef(active);
  useEffect(() => {
    activeRef.current = active;
  }, [active]);

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
    // ctrl/cmd + click an image path → open it as an image tab in this pane's slot
    const imgLinks = term.registerLinkProvider({
      provideLinks(lineNo, cb) {
        const line = term.buffer.active.getLine(lineNo - 1);
        if (!line) return cb(undefined);
        const text = line.translateToString(false);
        const out: ILink[] = [];
        IMG_RE.lastIndex = 0;
        let m: RegExpExecArray | null;
        while ((m = IMG_RE.exec(text))) {
          const startX = m.index + 1; // xterm ranges are 1-based, end inclusive
          out.push({
            text: m[0],
            range: { start: { x: startX, y: lineNo }, end: { x: startX + m[0].length - 1, y: lineNo } },
            decorations: { underline: true, pointerCursor: true },
            activate(event, txt) {
              if (!event.ctrlKey && !event.metaKey) return;
              useWorkspaces.getState().openImageTab(wsId, sessionId, resolveImgPath(txt, cwd));
            },
          });
        }
        cb(out.length ? out : undefined);
      },
    });
    searchRef.current = search;
    term.open(el);
    applyUnicode(term); // Unicode 11 widths + ZWJ-emoji glued to one cell so agent output doesn't mis-width
    // GPU (WebGL) renderer attaches in the visibility effect below, not here — only the active
    // space's panes hold a GL context, so N mounted spaces can't hit the browser's context cap

    // Ctrl+Shift+F search · Ctrl+C copies selection (else SIGINT) · Ctrl+V / Ctrl+Shift+V paste
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown") return true;
      // Ctrl+Tab / Ctrl+Shift+Tab cycles the tabs in this pane's slot (only if it's a tabbed group)
      if (e.ctrlKey && !e.altKey && e.code === "Tab") {
        const st = useWorkspaces.getState();
        const w = st.workspaces.find((x) => x.sessions.some((ss) => ss.id === sessionId));
        const me = w?.sessions.find((ss) => ss.id === sessionId);
        if (!w || !me?.group) return true; // solo pane → let the terminal have Tab
        const sibs = w.sessions.filter((ss) => ss.group === me.group);
        const idx = sibs.findIndex((ss) => ss.id === sessionId);
        const next = sibs[(idx + (e.shiftKey ? -1 : 1) + sibs.length) % sibs.length];
        st.setActiveTab(w.id, me.group, next.id);
        return false;
      }
      // Alt+V = image-aware paste. HyprSpace reads the clipboard image itself and drops a temp
      // file path into the prompt (the agents read images by path), which sidesteps the CLI's
      // flaky first clipboard read so it lands on the first try. No image → forward Alt+V as-is.
      if (e.altKey && !e.ctrlKey && !e.shiftKey && e.code === "KeyV") {
        const fallback = () => void writePty(sessionId, Uint8Array.from([0x1b, 0x76])); // ESC v
        clipboardImageToTemp()
          .then((path) => (path ? term.paste(path + " ") : fallback()))
          .catch(fallback);
        return false;
      }
      if (!e.ctrlKey || e.altKey) return true;
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
        // text first (like Orca): if the clipboard has text, paste it; only when there's none do we
        // fall back to an image — drop it to a temp file and paste the path (agents read images by path)
        void readText()
          .then((t) => {
            if (t) {
              term.paste(t);
              return;
            }
            return clipboardImageToTemp().then((path) => {
              if (path) term.paste(path + " ");
            });
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

    // flow control: xterm parses async, so a firehose child (cat bigfile, verbose build) can pile
    // megabytes into its write buffer. past HIGH_WATER we pause the backend reader thread (the
    // kernel pipe then backpressures the child) and resume once xterm drains below LOW_WATER.
    const HIGH_WATER = 1024 * 1024;
    const LOW_WATER = 128 * 1024;
    let pending = 0;
    let paused = false;

    useActivity.getState().markStart(sessionId);
    // apply the owning project's per-project env vars + default shell, if set
    const ownerWs = useWorkspaces
      .getState()
      .workspaces.find((w) => w.sessions.some((s) => s.id === sessionId));
    const cfg = ownerWs ? useProjectConfigs.getState().getConfig(ownerWs.cwd) : null;
    const projEnv = cfg && Object.keys(cfg.env).length ? cfg.env : undefined;
    // Interactive Claude panes: force a full alt-screen repaint every frame so a resize can't leave
    // stale/duplicated rows or a mid-screen status line. Claude only auto-enables this for
    // background/agent-view sessions on Windows, so interactive panes have to opt in themselves.
    const env = isClaude
      ? { ...(projEnv ?? {}), CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT: "1" }
      : projEnv;
    lastSize.current = { cols: term.cols, rows: term.rows }; // create_pty carries the initial size
    createPty(
      {
        id: sessionId,
        cwd,
        args: [],
        cols: term.cols,
        rows: term.rows,
        ...(env ? { env } : {}),
        ...(cfg?.defaultShell ? { shell: cfg.defaultShell } : {}),
      },
      {
        onData: (bytes) => {
          if (disposed) return;
          markBooted();
          pending += bytes.length;
          term.write(bytes, () => {
            pending -= bytes.length;
            if (paused && !disposed && pending < LOW_WATER) {
              paused = false;
              void resumePty(sessionId).catch(() => {});
            }
          });
          if (!paused && pending > HIGH_WATER) {
            paused = true;
            void pausePty(sessionId).catch(() => {});
          }
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
        fitResize(term.cols, term.rows);
        if (atBottom) term.scrollToBottom();
      } catch {
        /* not ready */
      }
    };

    // Resize: fit + resize the PTY on a light throttle, never every frame. Hammering ConPTY (and
    // SIGWINCH → the TUI) on every drag frame is what smeared/duplicated Claude mid-resize. ~16/s
    // keeps the reflow live without the storm; a "settle" fit ~140ms after the drag stops locks the
    // exact final size. (Claude also full-repaints via CLAUDE_CODE_ALT_SCREEN_FULL_REPAINT — no smear.)
    let lastFit = 0;
    const LIVE_MS = 60;
    const ro = new ResizeObserver(() => {
      clearTimeout(tSettled);
      tSettled = setTimeout(syncFit, 140); // authoritative final fit once the drag settles
      if (raf) return;
      const now = Date.now();
      if (now - lastFit < LIVE_MS) return; // throttle live fits; the settle catches the final size
      lastFit = now;
      raf = requestAnimationFrame(() => {
        raf = 0;
        syncFit();
      });
    });
    ro.observe(el);

    // Ctrl+scroll zooms the font (persisted, so every pane tracks the same size). capture phase +
    // stopPropagation so xterm's scrollback viewport can't eat the scroll-up (that broke zoom-in:
    // scrolling up had history to consume, scrolling down at the bottom didn't — hence the asymmetry).
    // coalesce fast gestures to one setFontSize per frame — every notch used to trigger a font
    // re-measure + atlas rebuild on every mounted pane
    let wheelDelta = 0;
    let wheelRaf = 0;
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      e.stopPropagation();
      wheelDelta += e.deltaY > 0 ? -1 : 1;
      if (wheelRaf) return;
      wheelRaf = requestAnimationFrame(() => {
        wheelRaf = 0;
        if (disposed || !wheelDelta) return;
        const cur = useSettings.getState().fontSize;
        useSettings.getState().setFontSize(cur + wheelDelta);
        wheelDelta = 0;
      });
    };
    el.addEventListener("wheel", onWheel, { passive: false, capture: true });

    // after sleep/resume or refocus the WebGL atlas can go stale → ghost cursor; invalidate + repaint
    const redraw = () => {
      if (!activeRef.current) return; // hidden panes rebuild their atlas when re-activated anyway
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
      if (wheelRaf) cancelAnimationFrame(wheelRaf);
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
      imgLinks.dispose();
      search.dispose();
      searchRef.current = null;
      dropOutput(sessionId);
      forgetSession(sessionId); // drop the auto-namer's capture state for this pane
      if (paused) void resumePty(sessionId).catch(() => {}); // unstick the reader before the kill
      void killPty(sessionId);
      term.dispose(); // also disposes the webgl addon if attached
      glRef.current = null;
      termRef.current = null;
      fitRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // GPU renderer rides visibility: attach while our space is shown, detach when it's hidden.
  // Hidden panes are display:none so the DOM renderer costs nothing there, and this keeps live
  // WebGL contexts ≤ one grid's worth (~12) — browsers evict them past ~16 per page.
  // Also reacts to the Settings toggle, so flipping it applies without reopening panes.
  useEffect(() => {
    const term = termRef.current;
    if (!term) return;
    const want = active && gpuRender;
    if (want && !glRef.current) {
      // the onLoss callback runs before the addon disposes itself — registering it after would
      // never fire (the emitter cancels remaining listeners mid-dispose)
      const gl = attachGpuRenderer(term, () => {
        glRef.current = null; // next activation re-attaches
      });
      if (gl) glRef.current = gl;
    } else if (!want && glRef.current) {
      try {
        glRef.current.dispose();
      } catch {
        /* terminal already torn down */
      }
      glRef.current = null;
    }
  }, [active, gpuRender]);

  // re-fit when revealed or when maximize toggles the pane size
  useEffect(() => {
    if (!active) return;
    const id = requestAnimationFrame(() => {
      const term = termRef.current;
      const fit = fitRef.current;
      if (!term || !fit) return;
      try {
        fit.fit();
        fitResize(term.cols, term.rows);
      } catch {
        /* not ready */
      }
      term.scrollToBottom();
    });
    return () => cancelAnimationFrame(id);
  }, [active, isMaxed, fitResize]);

  useEffect(() => {
    if (focused) termRef.current?.focus();
  }, [focused]);

  // live theme change → re-skin the terminal
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.theme = termTheme();
    // paint the pane surface (padding ring + corners) with the same bg so edges don't show the old color
    document.documentElement.style.setProperty("--term-surface", termSurface(terminalTheme));
    try {
      t.clearTextureAtlas?.();
      t.refresh(0, t.rows - 1);
    } catch {
      /* renderer not ready */
    }
  }, [themeId, terminalTheme]);

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
      fitResize(t.cols, t.rows);
    } catch {
      /* not ready */
    }
  }, [fontFamily, fontSize, lineHeight, fitResize]);

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
  // ---- + button: open a new pane as a tab in this slot (provider picker) ----
  const [addMenu, setAddMenu] = useState<{ x: number; y: number } | null>(null);
  const openAddMenu = (e: RMouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setAddMenu({ x: Math.min(r.right - 196, window.innerWidth - 210), y: r.bottom + 4 });
  };
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
            title="Open a pane in this folder"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={openAddMenu}
          >
            <Plus size={13} />
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
      {addMenu && (
        <PaneAddMenu
          x={addMenu.x}
          y={addMenu.y}
          wsId={wsId}
          anchorId={sessionId}
          anchorCommand={command}
          cwd={cwd}
          onClose={() => setAddMenu(null)}
        />
      )}
    </div>
  );
}

// memo: with stable (id-param) callbacks from PaneGrid, a pane only re-renders when ITS own
// active/focused/isMaxed/cwd/etc. change — not when a sibling is focused or a drag updates overId.
export const TerminalPane = memo(TerminalPaneInner);

