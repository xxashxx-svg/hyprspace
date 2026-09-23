// Small working copies of parts of the app, for the intro's tour. Each one is self-contained, holds
// its own state, and behaves like the real thing does, so the intro shows instead of describing.
// Nothing here touches the real stores.
import { useEffect, useMemo, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { ArrowUp, Check, ChevronRight, CornerDownLeft, FolderOpen, GitBranch, Search, SquarePen, TextSearch } from "lucide-react";
import { PROVIDER_LOGO, PROVIDER_NAME } from "../../lib/brand";
import { CATALOG, EFFORT_LABEL } from "../../lib/models";
import { kbd } from "../../platform";

type Row = { p: string; title: string; state: "work" | "wait" | "idle" };

// ---- Folders: fold them open and shut ----
const FOLDERS: { name: string; threads: Row[] }[] = [
  {
    name: "api-server",
    threads: [
      { p: "claude", title: "Fix the login redirect", state: "work" },
      { p: "codex", title: "Add rate limiting", state: "wait" },
    ],
  },
  { name: "website", threads: [{ p: "gemini", title: "Rewrite the pricing page", state: "idle" }] },
  { name: "mobile-app", threads: [{ p: "claude", title: "Upgrade to Expo 54", state: "work" }] },
];

function ThreadRow({ r, fresh }: { r: Row; fresh?: boolean }) {
  return (
    <div className={`dm-thread${fresh ? " fresh" : ""}`}>
      {r.p === "draft" ? <SquarePen size={12} /> : <img src={PROVIDER_LOGO[r.p]} alt="" />}
      <span className="dm-thread-title">{r.title}</span>
      <i className={`dm-dot ${r.state}`} />
    </div>
  );
}

export function FoldersDemo() {
  const [open, setOpen] = useState<Record<string, boolean>>({ "api-server": true });
  return (
    <div className="dm-rail">
      {FOLDERS.map((f) => {
        const on = !!open[f.name];
        return (
          <div key={f.name} className="dm-group">
            <button className={`dm-folder${on ? " on" : ""}`} onClick={() => setOpen((o) => ({ ...o, [f.name]: !on }))}>
              <ChevronRight size={13} />
              <span>{f.name}</span>
              <span className="dm-count">{f.threads.length}</span>
            </button>
            {on && f.threads.map((r) => <ThreadRow key={r.title} r={r} />)}
          </div>
        );
      })}
    </div>
  );
}

// ---- Threads: the square button asks for a folder, then a composer waits there ----
export function ThreadsDemo() {
  const [folders, setFolders] = useState([{ name: "api-server", threads: [FOLDERS[0].threads[0]] }]);
  const [picking, setPicking] = useState(false);
  const [fresh, setFresh] = useState<string | null>(null);

  const pick = (name: string) => {
    const draft: Row = { p: "draft", title: "New thread", state: "idle" };
    setFolders((fs) =>
      fs.some((f) => f.name === name)
        ? fs.map((f) => (f.name === name ? { ...f, threads: [draft, ...f.threads.filter((t) => t.p !== "draft")] } : f))
        : [...fs, { name, threads: [draft] }],
    );
    setFresh(name);
    setPicking(false);
  };

  return (
    <div className="dm-rail">
      <div className="dm-top">
        <span className="dm-search">
          <Search size={12} />
          Search
        </span>
        <button className={`dm-new${picking ? " on" : ""}`} onClick={() => setPicking((p) => !p)} title="New thread">
          <SquarePen size={13} />
        </button>
        {picking && (
          <div className="dm-picker">
            <div className="dm-picker-h">Pick a folder</div>
            {["api-server", "website", "docs"].map((n) => (
              <button key={n} onClick={() => pick(n)}>
                <FolderOpen size={13} />
                {n}
              </button>
            ))}
          </div>
        )}
      </div>
      {folders.map((f) => (
        <div key={f.name} className="dm-group">
          <div className="dm-folder on static">
            <ChevronRight size={13} />
            <span>{f.name}</span>
          </div>
          {f.threads.map((r) => (
            <ThreadRow key={r.title} r={r} fresh={fresh === f.name && r.p === "draft"} />
          ))}
        </div>
      ))}
    </div>
  );
}

// ---- Composer: agent, model, effort, task, Enter ----
const AGENTS = ["claude", "codex", "gemini"] as const;
const EFFORTS = ["low", "medium", "high"];

export function ComposerDemo() {
  const [agent, setAgent] = useState<(typeof AGENTS)[number]>("claude");
  const [effort, setEffort] = useState("high");
  const [text, setText] = useState("");
  const [sent, setSent] = useState<string | null>(null);
  const model = CATALOG[agent].models[1]?.label ?? "Default";
  const hasEffort = CATALOG[agent].efforts.length > 0;

  const send = () => text.trim() && setSent(text.trim());

  if (sent)
    return (
      <div className="dm-pane solo">
        <div className="dm-pane-head">
          <img src={PROVIDER_LOGO[agent]} alt="" />
          <span className="dm-pane-title">{sent}</span>
          <button
            className="dm-link"
            onClick={() => {
              setSent(null);
              setText("");
            }}
          >
            Start over
          </button>
        </div>
        <div className="dm-term">
          <div className="dim">
            {PROVIDER_NAME[agent]} · {model}
            {hasEffort && ` · ${EFFORT_LABEL[effort] ?? effort} effort`}
          </div>
          <div>
            <span className="dm-prompt">›</span> {sent}
          </div>
          <div className="dm-working">
            <i />
            Working
          </div>
        </div>
      </div>
    );

  return (
    <div className="dm-composer">
      <textarea
        value={text}
        placeholder="Describe a task, then press Enter"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            send();
          }
        }}
      />
      <div className="dm-composer-bar">
        <div className="dm-agents">
          {AGENTS.map((a) => (
            <button key={a} className={a === agent ? "on" : ""} onClick={() => setAgent(a)} title={PROVIDER_NAME[a]}>
              <img src={PROVIDER_LOGO[a]} alt="" />
            </button>
          ))}
        </div>
        <span className="dm-model">{model}</span>
        {hasEffort && (
          <div className="dm-effort">
            {EFFORTS.map((e) => (
              <button key={e} className={e === effort ? "on" : ""} onClick={() => setEffort(e)}>
                {EFFORT_LABEL[e] ?? e}
              </button>
            ))}
          </div>
        )}
        <button className="dm-send" disabled={!text.trim()} onClick={send} title="Start">
          <ArrowUp size={14} />
        </button>
      </div>
    </div>
  );
}

// ---- Panes: drag a header onto another pane to swap, double-click to maximize ----
const PANES: Row[] = [
  { p: "claude", title: "Fix the login redirect", state: "work" },
  { p: "codex", title: "Add rate limiting", state: "wait" },
  { p: "gemini", title: "Rewrite the pricing page", state: "idle" },
  { p: "claude", title: "Write the tests", state: "work" },
];

export function PanesDemo() {
  const [order, setOrder] = useState([0, 1, 2, 3]);
  const [max, setMax] = useState<number | null>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const [over, setOver] = useState<number | null>(null);

  // pointer events, not HTML drag and drop: the webview keeps native drops for files
  useEffect(() => {
    if (drag == null) return;
    const up = () => {
      if (over != null && over !== drag)
        setOrder((o) => {
          const n = [...o];
          [n[drag], n[over]] = [n[over], n[drag]];
          return n;
        });
      setDrag(null);
      setOver(null);
    };
    window.addEventListener("pointerup", up);
    return () => window.removeEventListener("pointerup", up);
  }, [drag, over]);

  const slots = max == null ? order.map((_, i) => i) : [max];
  return (
    <div className={`dm-grid${max != null ? " max" : ""}${drag != null ? " dragging" : ""}`}>
      {slots.map((slot) => {
        const r = PANES[order[slot]];
        return (
          <div
            key={order[slot]}
            className={`dm-pane${drag === slot ? " lifted" : ""}${over === slot && drag !== slot ? " target" : ""}`}
            onPointerEnter={() => drag != null && setOver(slot)}
            onPointerLeave={() => drag != null && setOver(null)}
          >
            <div
              className="dm-pane-head grab"
              onPointerDown={(e) => {
                if (e.button === 0 && max == null) setDrag(slot);
              }}
              onDoubleClick={() => setMax((m) => (m == null ? slot : null))}
            >
              <img src={PROVIDER_LOGO[r.p]} alt="" />
              <span className="dm-pane-title">{r.title}</span>
              <i className={`dm-dot ${r.state}`} />
            </div>
            <div className="dm-term">
              <span className="dm-bar" style={{ width: `${62 - (order[slot] % 3) * 12}%` }} />
              <span className="dm-bar" style={{ width: `${40 + order[slot] * 9}%` }} />
              <span className="dm-bar dim" style={{ width: `${50 - order[slot] * 5}%` }} />
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---- Command palette: type to filter, arrows to move, Enter to run ----
type Cmd = { label: string; section: string; sub?: string; logo?: string; mono?: boolean };
const COMMANDS: Cmd[] = [
  { label: "New thread", section: "Start" },
  { label: "New terminal", section: "Start" },
  { label: "Open a folder", section: "Start" },
  { label: "Fix the login redirect", section: "Threads", sub: "api-server", logo: "claude" },
  { label: "Add rate limiting", section: "Threads", sub: "api-server", logo: "codex" },
  { label: "Rewrite the pricing page", section: "Threads", sub: "website", logo: "gemini" },
  { label: "Show or hide files and git", section: "View" },
  { label: "Switch to light mode", section: "View" },
];
const OUTPUT = { label: "api-server › Fix the login redirect", sub: "TypeError: Cannot read properties of undefined (reading 'user')" };

export function PaletteDemo() {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [ran, setRan] = useState<string | null>(null);

  const items = useMemo<Cmd[]>(() => {
    const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const hits = COMMANDS.filter((c) => words.every((w) => `${c.label} ${c.section} ${c.sub ?? ""}`.toLowerCase().includes(w)));
    // like the real one, text inside the terminals shows up once the query is two letters long
    if (q.trim().length >= 2 && OUTPUT.sub.toLowerCase().includes(q.trim().toLowerCase()))
      hits.push({ label: OUTPUT.label, section: "In terminal output", sub: OUTPUT.sub, mono: true });
    return hits;
  }, [q]);

  const onKey = (e: ReactKeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((i) => Math.min(items.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter" && items[sel]) setRan(items[sel].label);
  };

  return (
    <div className="dm-palette">
      <div className="dm-palette-input">
        <Search size={13} />
        <input
          value={q}
          placeholder="Try: login, light, or undefined"
          spellCheck={false}
          onChange={(e) => {
            setQ(e.target.value);
            setSel(0);
            setRan(null);
          }}
          onKeyDown={onKey}
        />
        <kbd>{kbd("Ctrl K")}</kbd>
      </div>
      <div className="dm-palette-list">
        {items.length === 0 && <div className="dm-empty">Nothing matches</div>}
        {items.map((c, i) => (
          <div key={c.section + c.label}>
            {(i === 0 || items[i - 1].section !== c.section) && <div className="dm-section">{c.section}</div>}
            <button className={`dm-cmd${i === sel ? " on" : ""}`} onMouseMove={() => setSel(i)} onClick={() => setRan(c.label)}>
              {c.logo ? <img src={PROVIDER_LOGO[c.logo]} alt="" /> : c.mono ? <TextSearch size={13} /> : <ChevronRight size={13} />}
              <span className="dm-cmd-label">{c.label}</span>
              {c.sub && <span className={`dm-cmd-sub${c.mono ? " mono" : ""}`}>{c.sub}</span>}
            </button>
          </div>
        ))}
      </div>
      {ran && (
        <div className="dm-toast">
          <CornerDownLeft size={12} />
          Would run: {ran}
        </div>
      )}
    </div>
  );
}

// ---- Terminal: ctrl+click a path to open it, hover an image marker to see it ----
export function TerminalDemo() {
  const [ctrl, setCtrl] = useState(false);
  const [tab, setTab] = useState<"term" | "file">("term");
  const [opened, setOpened] = useState(false);
  const [peek, setPeek] = useState(false);

  useEffect(() => {
    const on = (e: KeyboardEvent) => setCtrl(e.ctrlKey || e.metaKey);
    const off = () => setCtrl(false);
    window.addEventListener("keydown", on);
    window.addEventListener("keyup", on);
    window.addEventListener("blur", off);
    return () => {
      window.removeEventListener("keydown", on);
      window.removeEventListener("keyup", on);
      window.removeEventListener("blur", off);
    };
  }, []);

  const path = (label: string) => (
    <span
      className="dm-path"
      onClick={(e) => {
        if (!(e.ctrlKey || e.metaKey)) return;
        setOpened(true);
        setTab("file");
      }}
    >
      {label}
    </span>
  );

  return (
    <div className={`dm-pane solo${ctrl ? " ctrl" : ""}`}>
      <div className="dm-tabs">
        <button className={tab === "term" ? "on" : ""} onClick={() => setTab("term")}>
          <img src={PROVIDER_LOGO.claude} alt="" />
          Fix the login redirect
        </button>
        {opened && (
          <button className={tab === "file" ? "on" : ""} onClick={() => setTab("file")}>
            login.ts
          </button>
        )}
      </div>
      {tab === "term" ? (
        <div className="dm-term text">
          <div>
            <span className="dm-prompt">›</span>{" "}
            <span className="dm-image" onMouseEnter={() => setPeek(true)} onMouseLeave={() => setPeek(false)}>
              [Image #1]
              {peek && (
                // a drawing of the pasted screenshot: the login page's error, in a browser window
                <span className="dm-peek">
                  <span className="dm-shot">
                    <span className="dm-shot-bar">
                      <i />
                      <i />
                      <i />
                      <span className="dm-shot-url">localhost:3000/login</span>
                    </span>
                    <span className="dm-shot-page">
                      <b>500</b>
                      <span>Session expired</span>
                      <span className="dm-shot-btn">Sign in again</span>
                    </span>
                  </span>
                  <span className="dm-peek-cap">
                    <span>1280 × 720</span>
                    <span>{kbd("Ctrl")}+click to open</span>
                  </span>
                </span>
              )}
            </span>{" "}
            the login page does this after I sign in
          </div>
          <div>
            <span className="dm-bullet">●</span> Read {path("src/auth/login.ts")}
          </div>
          <div>
            <span className="dm-bullet">●</span> The redirect reads <code>next</code> before the session cookie is set.
          </div>
          <div>
            <span className="dm-bullet">●</span> Update({path("src/auth/login.ts:42")})
          </div>
          <div className="dim">&nbsp;&nbsp;⎿ 3 lines added, 1 removed</div>
        </div>
      ) : (
        <div className="dm-code">
          {["export async function login(req, res) {", "  const user = await verify(req.body);", "  await setSession(res, user);", "  const next = safeNext(req.query.next);", "  return res.redirect(next);", "}"].map((l, i) => (
            <div key={i} className={i === 3 ? "hl" : ""}>
              <span className="ln">{40 + i}</span>
              {l}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ---- Git: tick files, write a summary, commit, push ----
export function GitDemo() {
  const [files, setFiles] = useState([
    { p: "src/auth/login.ts", a: 3, d: 1, on: true },
    { p: "src/auth/session.ts", a: 12, d: 4, on: true },
    { p: "README.md", a: 2, d: 0, on: false },
  ]);
  const [summary, setSummary] = useState("");
  const [ahead, setAhead] = useState(0);
  const [pushed, setPushed] = useState(false);
  const ticked = files.filter((f) => f.on).length;

  const commit = () => {
    setFiles((fs) => fs.filter((f) => !f.on));
    setSummary("");
    setAhead((a) => a + 1);
    setPushed(false);
  };

  return (
    <div className="dm-git">
      <div className="dm-git-head">
        <GitBranch size={13} />
        main
        <span className="dm-git-state">{ahead ? `${ahead} to push` : pushed ? "Up to date" : `${files.length} changed`}</span>
      </div>
      <div className="dm-git-files">
        {files.length === 0 && <div className="dm-empty">No changes</div>}
        {files.map((f) => (
          <button key={f.p} className="dm-file" onClick={() => setFiles((fs) => fs.map((x) => (x.p === f.p ? { ...x, on: !x.on } : x)))}>
            <span className={`dm-check${f.on ? " on" : ""}`}>{f.on && <Check size={10} strokeWidth={3} />}</span>
            <span className="dm-file-p">{f.p}</span>
            <span className="dm-add">+{f.a}</span>
            <span className="dm-del">-{f.d}</span>
          </button>
        ))}
      </div>
      <input className="dm-summary" value={summary} placeholder="Summary" onChange={(e) => setSummary(e.target.value)} />
      <div className="dm-git-actions">
        <button className="dm-btn" disabled={!ticked || !summary.trim()} onClick={commit}>
          Commit {ticked ? `${ticked} file${ticked > 1 ? "s" : ""}` : ""}
        </button>
        {ahead > 0 && (
          <button
            className="dm-btn"
            onClick={() => {
              setAhead(0);
              setPushed(true);
            }}
          >
            Push
          </button>
        )}
      </div>
    </div>
  );
}

// ---- Usage: what's left of each plan's limits, and when they reset ----
const LIMITS: Record<string, { label: string; left: number; reset: string }[]> = {
  claude: [
    { label: "5-hour limit", left: 62, reset: "Resets in 2h 14m" },
    { label: "Weekly limit", left: 81, reset: "Resets Mon 9:00" },
  ],
  codex: [
    { label: "5-hour limit", left: 24, reset: "Resets in 3h 50m" },
    { label: "Weekly limit", left: 40, reset: "Resets Thu 18:00" },
  ],
};

export function UsageDemo() {
  const [who, setWho] = useState<"claude" | "codex">("claude");
  return (
    <div className="dm-usage">
      <div className="dm-usage-tabs">
        {(["claude", "codex"] as const).map((w) => (
          <button key={w} className={w === who ? "on" : ""} onClick={() => setWho(w)}>
            <img src={PROVIDER_LOGO[w]} alt="" />
            {PROVIDER_NAME[w]}
          </button>
        ))}
      </div>
      {LIMITS[who].map((l) => (
        <div key={l.label} className="dm-limit">
          <div className="dm-limit-top">
            <span>{l.label}</span>
            <b>{l.left}% left</b>
          </div>
          <div className={`dm-meter${l.left < 30 ? " low" : ""}`}>
            <span style={{ width: `${l.left}%` }} />
          </div>
          <div className="dm-limit-reset">{l.reset}</div>
        </div>
      ))}
    </div>
  );
}
