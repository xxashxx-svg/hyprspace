import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { openUrl } from "@tauri-apps/plugin-opener";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { makeTerminal } from "../terminal/createTerminal";
import { attachLoopTerm } from "../terminal/loopTerm";
import { writePty, resizePty } from "../api";

// Live terminal for an interactive-terminal loop. The PTY is owned by the loop engine (it outlives
// this component), so on mount we just attach a sink for live bytes + wire keystrokes back, and a
// fit/resize makes claude repaint the current screen. We never create or kill the PTY here.
export function LoopTerminal({ id }: { id: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const term = makeTerminal(true);
    const fit = new FitAddon();
    term.loadAddon(fit);
    const links = new WebLinksAddon((e, uri) => {
      if (e.ctrlKey || e.metaKey) void openUrl(uri).catch(() => {});
    });
    term.loadAddon(links);
    term.open(el);
    termRef.current = term;
    try {
      fit.fit();
    } catch {
      /* not ready */
    }

    // Ctrl+C copies a selection (else interrupts) · Ctrl+V pastes
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== "keydown" || !e.ctrlKey || e.altKey) return true;
      if (e.code === "KeyC" && term.hasSelection()) {
        const sel = term.getSelection();
        if (sel) void writeText(sel);
        term.clearSelection();
        return false;
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

    const enc = new TextEncoder();
    const dataDisp = term.onData((d) => void writePty(id, enc.encode(d)).catch(() => {}));
    const detach = attachLoopTerm(id, { write: (b) => term.write(b) });

    // fit → resize the PTY; a size change makes the TUI repaint the current screen (so re-attaching
    // after navigating away shows where the session is now, e.g. the question it's waiting on)
    const syncFit = () => {
      if (el.clientWidth < 40 || el.clientHeight < 20) return;
      try {
        fit.fit();
        void resizePty(id, term.cols, term.rows);
      } catch {
        /* not ready */
      }
    };
    requestAnimationFrame(syncFit);
    // the PTY is created async by the engine and may not exist yet on mount — re-fit a couple of
    // times so the resize (which makes the TUI repaint at our width) lands once the session is up
    const t1 = setTimeout(syncFit, 700);
    const t2 = setTimeout(syncFit, 1800);
    let raf = 0;
    const ro = new ResizeObserver(() => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        syncFit();
      });
    });
    ro.observe(el);
    term.focus();

    return () => {
      if (raf) cancelAnimationFrame(raf);
      clearTimeout(t1);
      clearTimeout(t2);
      ro.disconnect();
      dataDisp.dispose();
      links.dispose();
      detach();
      term.dispose();
      termRef.current = null;
    };
  }, [id]);

  // right-click pastes (bracketed paste keeps multi-line input from pre-submitting)
  const onPaste = (e: React.MouseEvent) => {
    e.preventDefault();
    void readText()
      .then((t) => {
        if (t) termRef.current?.paste(t);
      })
      .catch(() => {});
  };

  return <div className="loop-term" ref={ref} onContextMenu={onPaste} />;
}
