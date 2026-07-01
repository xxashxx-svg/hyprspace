import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { openUrl } from "@tauri-apps/plugin-opener";
import { makeTerminal, attachGpuRenderer } from "../terminal/createTerminal";
import { createPty, writePty, killPty, detectRunCmd } from "../api";

// finds the dev-server URL the framework prints (vite/next/etc.)
const URL_RE = /(https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0):\d+)/i;

// A small terminal in the dock that runs the project's dev command and surfaces its URL.
export function RunPanel({ wsId, cwd }: { wsId: string; cwd: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const idRef = useRef<string | null>(null);
  const [running, setRunning] = useState(false);
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const term = makeTerminal(false);
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    attachGpuRenderer(term);
    try {
      fit.fit();
    } catch {
      /* not ready */
    }
    termRef.current = term;
    let raf = 0;
    const ro = new ResizeObserver(() => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        try {
          fit.fit();
          if (idRef.current) void writePty(idRef.current, new Uint8Array()); // nudge size
        } catch {
          /* not ready */
        }
      });
    });
    ro.observe(el);
    return () => {
      if (raf) cancelAnimationFrame(raf);
      ro.disconnect();
      if (idRef.current) void killPty(idRef.current);
      term.dispose();
      termRef.current = null;
    };
  }, []);

  const start = async () => {
    const term = termRef.current;
    if (running || !cwd || !term) return;
    const id = "run-" + wsId;
    idRef.current = id;
    setUrl(null);
    setRunning(true);
    const enc = new TextEncoder();
    const dec = new TextDecoder();
    await createPty(
      { id, cwd, args: [], cols: term.cols, rows: term.rows },
      {
        onData: (bytes) => {
          term.write(bytes);
          const m = dec.decode(bytes).match(URL_RE);
          if (m) setUrl((cur) => cur ?? m[1].replace("0.0.0.0", "localhost"));
        },
        onControl: (c) => {
          if (c.type === "exit") setRunning(false);
        },
      },
    ).catch(() => setRunning(false));
    const cmd = (await detectRunCmd(cwd).catch(() => "")) || "npm run dev";
    setTimeout(() => {
      if (idRef.current === id) void writePty(id, enc.encode(cmd + "\r"));
    }, 300);
  };

  const stop = () => {
    if (idRef.current) void killPty(idRef.current);
    idRef.current = null;
    setRunning(false);
  };

  if (!cwd) return <div className="dock-empty">Open a project workspace to run it.</div>;

  return (
    <div className="dock-body">
      <div className="run-bar">
        {running ? (
          <button className="dock-mini stop" onClick={stop}>
            ⏹ Stop
          </button>
        ) : (
          <button className="dock-mini run" onClick={() => void start()}>
            ▶ Run
          </button>
        )}
        {url && (
          <button className="dock-mini open" onClick={() => void openUrl(url)} title={url}>
            Open {url.replace(/^https?:\/\//, "")}
          </button>
        )}
      </div>
      <div className="run-term" ref={ref} />
    </div>
  );
}
