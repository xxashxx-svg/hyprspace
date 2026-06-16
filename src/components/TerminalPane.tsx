import { useEffect, useRef, useState } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { makeTerminal, termTheme } from "../terminal/createTerminal";
import { useSettings } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { createPty, writePty, resizePty, killPty, claudeResumeMode } from "../api";
import { appendOutput, dropOutput } from "../terminal/buffers";
import { TerminalSearch } from "./TerminalSearch";

// Each claude pane owns its session id (= the pane's uuid), so on relaunch it resumes its OWN
// conversation — not just "the folder's latest", which broke open spaces with several panes in
// one folder. We claim the id with --session-id on first launch, then --resume <id> to return.
function injectClaudeArg(cmd: string, arg: string): string {
  if (/--session-id|--resume|--continue|(^|\s)-[cr](\s|$)/.test(cmd)) return cmd; // already pinned
  return cmd.replace(/^claude\b/, `claude ${arg}`);
}

interface Props {
  sessionId: string;
  cwd: string;
  command?: string;
  started?: boolean;
  active: boolean;
  focused: boolean;
  isMaxed: boolean;
  onFocus: () => void;
  onClose: () => void;
  onToggleMax: () => void;
  onGripDown: (e: RPointerEvent<HTMLDivElement>) => void;
  onGripMove: (e: RPointerEvent<HTMLDivElement>) => void;
  onGripUp: (e: RPointerEvent<HTMLDivElement>) => void;
}

export function TerminalPane({
  sessionId,
  cwd,
  command,
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
  const [showSearch, setShowSearch] = useState(false);
  const themeId = useSettings((s) => s.theme);
  const fontSize = useSettings((s) => s.fontSize);
  const fontFamily = useSettings((s) => s.fontFamily);
  const cursorStyle = useSettings((s) => s.cursorStyle);
  const cursorBlink = useSettings((s) => s.cursorBlink);

  const isClaude = !!command && command.includes("claude");

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
    // WebGL renderer with auto-recovery: if the GPU context drops (sleep/wake, driver
    // reset) re-attach instead of falling back to the slow DOM renderer for good.
    let webglTries = 0;
    const loadWebgl = () => {
      if (disposed || webglTries++ > 3) return;
      try {
        const addon = new WebglAddon();
        addon.onContextLoss(() => {
          addon.dispose();
          setTimeout(loadWebgl, 500);
        });
        term.loadAddon(addon);
      } catch {
        /* canvas/DOM fallback */
      }
    };
    loadWebgl();
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

    const enc = new TextEncoder();
    const dec = new TextDecoder();
    const dataDisp = term.onData((d) => {
      void writePty(sessionId, enc.encode(d));
    });
    // copy-on-select (opt-in) — via the Tauri clipboard so it works inside the webview
    const selDisp = term.onSelectionChange(() => {
      if (useSettings.getState().copyOnSelect && term.hasSelection()) {
        const sel = term.getSelection();
        if (sel) void writeText(sel);
      }
    });

    createPty(
      { id: sessionId, cwd, args: [], cols: term.cols, rows: term.rows },
      {
        onData: (bytes) => {
          if (disposed) return;
          term.write(bytes);
          appendOutput(sessionId, dec.decode(bytes, { stream: true }));
        },
        onControl: (c) => {
          if (!disposed && c.type === "exit") {
            setAlive(false);
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
    const ro = new ResizeObserver(() => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        if (el.clientWidth === 0 || el.clientHeight === 0) return;
        const atBottom = term.buffer.active.viewportY === term.buffer.active.baseY;
        try {
          fit.fit();
          void resizePty(sessionId, term.cols, term.rows);
          if (atBottom) term.scrollToBottom(); // don't fling scroll to the top on a reflow
        } catch {
          /* not ready */
        }
      });
    });
    ro.observe(el);

    // Ctrl+scroll zooms the font (persisted, so every pane tracks the same size)
    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey) return;
      e.preventDefault();
      const cur = useSettings.getState().fontSize;
      useSettings.getState().setFontSize(cur + (e.deltaY > 0 ? -1 : 1));
    };
    el.addEventListener("wheel", onWheel, { passive: false });

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
      ro.disconnect();
      el.removeEventListener("wheel", onWheel);
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("focus", redraw);
      dataDisp.dispose();
      selDisp.dispose();
      links.dispose();
      search.dispose();
      searchRef.current = null;
      dropOutput(sessionId);
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
    try {
      f?.fit();
      void resizePty(sessionId, t.cols, t.rows);
    } catch {
      /* not ready */
    }
  }, [fontFamily, fontSize, sessionId]);

  // live cursor change
  useEffect(() => {
    const t = termRef.current;
    if (!t) return;
    t.options.cursorStyle = cursorStyle;
    t.options.cursorBlink = isClaude ? false : cursorBlink;
  }, [cursorStyle, cursorBlink, isClaude]);

  const folder = cwd.split(/[\\/]/).filter(Boolean).pop();

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
    <div className={`terminal-pane ${focused ? "focused" : ""}`} onMouseDown={onFocus}>
      <div
        className="pane-header"
        onPointerDown={onGripDown}
        onPointerMove={onGripMove}
        onPointerUp={onGripUp}
      >
        <span className="pane-head-left">
          {!alive && <span className="pane-status" title="process exited" />}
          <span className="pane-title">{isClaude ? "claude" : "terminal"}</span>
          {folder && <span className="pane-cwd">· {folder}</span>}
        </span>
        <span className="pane-head-right">
          <button
            className="pane-btn"
            title={isMaxed ? "Restore" : "Maximize"}
            onPointerDown={(e) => e.stopPropagation()}
            onClick={onToggleMax}
          >
            {isMaxed ? "❐" : "▢"}
          </button>
          <button
            className="pane-btn close"
            title="Close pane"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={onClose}
          >
            ×
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
    </div>
  );
}
