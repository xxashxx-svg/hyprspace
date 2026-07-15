import { memo, useEffect, useState, type ReactNode } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useLoops, newLoop, type LoopDef, type LoopStop } from "../stores/loops";
import { useUi } from "../stores/ui";
import { startLoop, stopLoop, pauseLoop, revealLoopWorktree, paneCapable } from "../lib/loops";
import { cronValid, nextCron } from "../lib/cron";
import { LOOP_TEMPLATES, type LoopTemplate } from "../lib/loopTemplates";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";
import { useWorkspaces } from "../stores/workspace";
import { pickFolder, secretSet, secretHas, secretClear } from "../api";
import {
  Play,
  Square,
  Pause,
  Trash2,
  ScrollText,
  Plus,
  FolderOpen,
  KeyRound,
  FolderGit2,
  FlaskConical,
  Hammer,
  ListChecks,
  Sparkles,
  RefreshCw,
  BookText,
  ScanEye,
  ChevronRight,
} from "lucide-react";

// compact labels for the collapsed-card summary line
const PROVIDER_SHORT: Record<string, string> = {
  claude: "Claude (API key)",
  "claude-sub": "Claude (sub)",
  "claude-hooks": "Claude (sub)",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
  grok: "Grok",
};
const RUN_LABEL: Record<string, string> = { headless: "Headless", pane: "Interactive" };
const MODE_LABEL: Record<string, string> = {
  "until-done": "Until done",
  interval: "Interval",
  cron: "Schedule",
  manual: "Manual",
};

// template icon key → element (size is constant, so a lookup map is simplest)
const TPL_ICON: Record<string, ReactNode> = {
  FlaskConical: <FlaskConical size={16} />,
  Hammer: <Hammer size={16} />,
  ListChecks: <ListChecks size={16} />,
  Sparkles: <Sparkles size={16} />,
  RefreshCw: <RefreshCw size={16} />,
  BookText: <BookText size={16} />,
  ScanEye: <ScanEye size={16} />,
};

const STATUS_LABEL: Record<string, string> = {
  idle: "idle",
  running: "running",
  paused: "paused",
  stopped: "stopped",
  done: "done",
  error: "error",
  crashloop: "stopped · no progress",
};

// compact token count: 980 → "980", 12345 → "12.3k", 1500000 → "1.5M"
function fmtTokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

// memoized so a growing log doesn't re-render every line already on screen
const LogLine = memo(function LogLine({ line }: { line: string }) {
  return <div className="loop-logs-line">{line || " "}</div>;
});

// backend picker chips — same brand marks the launcher / usage panel use
const BACKENDS: { id: LoopDef["provider"]; label: string; sub?: string; logo: string }[] = [
  { id: "claude-sub", label: "Claude", sub: "subscription", logo: claudeLogo },
  { id: "claude", label: "Claude", sub: "API key", logo: claudeLogo },
  { id: "codex", label: "Codex", logo: openaiLogo },
  { id: "gemini", label: "Gemini", logo: geminiLogo },
  { id: "opencode", label: "OpenCode", logo: opencodeLogo },
  { id: "grok", label: "Grok", logo: grokLogo },
];
// backends whose CLIs report token usage — the token budget is honest only for these
const BUDGET_BACKENDS = new Set(["claude", "claude-sub", "claude-hooks", "codex"]);

function Seg<T extends string>({
  value,
  opts,
  onChange,
  disabled,
}: {
  value: T;
  opts: { v: T; label: string; hint?: string; off?: boolean }[];
  onChange: (v: T) => void;
  disabled?: boolean;
}) {
  return (
    <div className="loop-seg">
      {opts.map((o) => (
        <button
          key={o.v}
          className={`loop-seg-btn${value === o.v ? " on" : ""}`}
          disabled={disabled || o.off}
          title={o.hint}
          onClick={() => onChange(o.v)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

function ToggleRow({
  label,
  desc,
  on,
  onChange,
}: {
  label: string;
  desc?: string;
  on: boolean;
  onChange: (b: boolean) => void;
}) {
  return (
    <div className="loop-toggle-row">
      <div className="loop-toggle-info">
        <span>{label}</span>
        {desc && <em>{desc}</em>}
      </div>
      <button className={`toggle ${on ? "on" : ""}`} onClick={() => onChange(!on)} aria-pressed={on}>
        <span className="toggle-knob" />
      </button>
    </div>
  );
}

// number input that lets you actually clear/retype — clamps + commits on blur/Enter, not per keystroke
function NumField({
  value,
  min,
  onCommit,
}: {
  value: number;
  min: number;
  onCommit: (n: number) => void;
}) {
  const [txt, setTxt] = useState(String(value));
  useEffect(() => setTxt(String(value)), [value]);
  const commit = () => {
    const n = Math.max(min, Math.round(+txt) || min);
    setTxt(String(n));
    if (n !== value) onCommit(n);
  };
  return (
    <input
      className="svc-in"
      type="number"
      min={min}
      value={txt}
      onChange={(e) => setTxt(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && commit()}
    />
  );
}

function fmtWhen(t: number): string {
  const d = new Date(t);
  const today = new Date().toDateString() === d.toDateString();
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  return today
    ? `today ${time}`
    : `${d.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })} ${time}`;
}

// human preview of the next fire, mirroring the engine's priority (cron → everyMin → dailyAt)
function nextFirePreview(s?: LoopDef["schedule"]): string {
  const now = Date.now();
  if (s?.cron?.trim()) {
    if (!cronValid(s.cron)) return "invalid cron — using the simple fields instead";
    const t = nextCron(s.cron, now);
    // engine falls back to the simple fields when a valid cron can never match — say so
    if (!t) return "cron never matches — falling back to the simple fields";
    return `next run ${fmtWhen(t)}`;
  }
  if (s?.everyMin && s.everyMin > 0) return `next run ${fmtWhen(now + s.everyMin * 60000)}`;
  if (s?.dailyAt && /^\d{1,2}:\d{2}$/.test(s.dailyAt)) {
    const [h, m] = s.dailyAt.split(":").map(Number);
    const d = new Date();
    d.setHours(h, m, 0, 0);
    if (d.getTime() <= now) d.setDate(d.getDate() + 1);
    return `next run ${fmtWhen(d.getTime())}`;
  }
  return "no schedule set — defaults to hourly";
}

// Settings → Loops. Create/edit loops and drive them live (start/stop/pause, iteration count, logs).
export function LoopsManager() {
  const loops = useLoops((s) => s.loops);
  const runs = useLoops((s) => s.runs);
  const ids = Object.keys(loops);
  const [logsFor, setLogsFor] = useState<string | null>(null);
  const [listRef] = useAutoAnimate(); // smooth add/remove of loop cards
  // one expanded card at a time keeps the list scannable; open the loop you came here to edit
  const focused = useUi((s) => s.openLoopId);
  const [openId, setOpenId] = useState<string | null>(() => useUi.getState().openLoopId);
  // keep "next run …" previews fresh — they're computed from Date.now() at render time
  const [, setTick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setTick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);
  useEffect(() => {
    if (focused && useLoops.getState().loops[focused]) setOpenId(focused);
  }, [focused]);

  // Anthropic API key lives in the OS keychain; we only ever know whether it's set, never its value.
  const [hasKey, setHasKey] = useState(false);
  const [keyInput, setKeyInput] = useState("");
  const [editingKey, setEditingKey] = useState(false);
  useEffect(() => {
    void secretHas("anthropic").then(setHasKey).catch(() => {});
  }, []);
  const saveKey = async () => {
    const v = keyInput.trim();
    if (v) {
      await secretSet("anthropic", v).catch(() => {});
      setHasKey(true);
    }
    setKeyInput("");
    setEditingKey(false);
  };
  const clearKey = async () => {
    await secretClear("anthropic").catch(() => {});
    setHasKey(false);
  };

  const activeCwd = useWorkspaces((s) => {
    const w = s.workspaces.find((x) => x.id === s.activeId);
    return w && w.kind !== "open" ? w.cwd : "";
  });

  const add = () => {
    const def = newLoop(activeCwd || "");
    def.name = "New automation";
    useLoops.getState().upsert(def);
    setOpenId(def.id); // expand the fresh loop so it's ready to fill in
  };
  const addTemplate = (t: LoopTemplate) => {
    const d = t.build(activeCwd || "");
    useLoops.getState().upsert(d);
    setOpenId(d.id);
  };
  const update = (id: string, patch: Partial<LoopDef>) => {
    const def = useLoops.getState().loops[id];
    if (def) useLoops.getState().upsert({ ...def, ...patch });
  };
  const updateStop = (id: string, patch: Partial<LoopStop>) => {
    const def = loops[id];
    if (def) update(id, { stop: { ...def.stop, ...patch } });
  };
  const updateSched = (id: string, patch: Partial<NonNullable<LoopDef["schedule"]>>) => {
    const def = loops[id];
    if (def) update(id, { schedule: { ...def.schedule, ...patch } });
  };
  const browseFolder = async (id: string) => {
    const f = await pickFolder();
    if (f) update(id, { folder: f });
  };

  return (
    <div className="loops">
      <div className="loops-bar">
        <span className="set-label">Automations</span>
        <button className="btn" onClick={add}>
          <Plus size={14} /> New automation
        </button>
      </div>
      <div className="svc-hint" style={{ marginBottom: 14 }}>
        Run an agent on a schedule, on an interval, or until a goal is met. Every automation needs a
        stop limit, so it can never run forever. Automations run while HyprSpace is open.
      </div>

      <div className="loop-key">
        <KeyRound size={14} />
        <span className="loop-key-label">Anthropic API key</span>
        {editingKey ? (
          <>
            <input
              className="svc-in"
              type="password"
              placeholder="sk-ant-…"
              value={keyInput}
              autoFocus
              onChange={(e) => setKeyInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void saveKey();
                if (e.key === "Escape") {
                  setKeyInput("");
                  setEditingKey(false);
                }
              }}
            />
            <button className="btn" onClick={() => void saveKey()}>Save</button>
            <button className="btn" onClick={() => { setKeyInput(""); setEditingKey(false); }}>Cancel</button>
          </>
        ) : (
          <>
            <span className={`loop-key-status${hasKey ? " ok" : ""}`}>
              {hasKey ? "stored in keychain" : "not set"}
            </span>
            <button className="btn" onClick={() => setEditingKey(true)}>{hasKey ? "Update" : "Set key"}</button>
            {hasKey && <button className="btn" onClick={() => void clearKey()}>Clear</button>}
            <span className="loop-key-note">used only by Claude automations; never your subscription</span>
          </>
        )}
      </div>

      <div className="loop-templates">
        <div className="loop-templates-head">
          {ids.length === 0 ? "Start from a template" : "Add from a template"}
        </div>
        <div className="loop-templates-grid">
          {LOOP_TEMPLATES.map((t) => (
            <button key={t.id} className="loop-tpl" onClick={() => addTemplate(t)} title={t.blurb}>
              <span className="loop-tpl-ico">{TPL_ICON[t.icon]}</span>
              <span className="loop-tpl-text">
                <span className="loop-tpl-title">{t.title}</span>
                <span className="loop-tpl-blurb">{t.blurb}</span>
              </span>
            </button>
          ))}
        </div>
      </div>

      <div ref={listRef}>
      {ids.map((id) => {
        const def = loops[id];
        const run = runs[id];
        const status = run?.status ?? "idle";
        const active = status === "running" || status === "paused";
        return (
          <div className={`loop-card${active ? " on" : ""}`} key={id}>
            <div className="loop-card-head" onClick={() => setOpenId(openId === id ? null : id)}>
              <span className="loop-card-twist" aria-hidden>
                <ChevronRight size={14} />
              </span>
              <span className={`loop-dot s-${status}`} />
              <input
                className="loop-name-in"
                placeholder="Automation name"
                value={def.name}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => update(id, { name: e.target.value })}
              />
              {openId !== id && (
                <span className="loop-card-summary">
                  {PROVIDER_SHORT[def.provider] ?? def.provider} · {RUN_LABEL[def.run] ?? def.run} ·{" "}
                  {MODE_LABEL[def.mode] ?? def.mode}
                </span>
              )}
              <span className={`loop-status-badge s-${status} loop-head-badge`}>
                {STATUS_LABEL[status] ?? status}
              </span>
              <span className="loop-card-actions" onClick={(e) => e.stopPropagation()}>
              {active ? (
                <>
                  <button className="svc-stop" title="Stop" onClick={() => stopLoop(id)}>
                    <Square size={13} />
                  </button>
                  {def.run !== "pane" && ( // pane sessions can't pause — the flag isn't read there
                    <button
                      className="svc-run"
                      title={status === "paused" ? "Resume" : "Pause"}
                      onClick={() => pauseLoop(id, status !== "paused")}
                    >
                      <Pause size={13} />
                    </button>
                  )}
                </>
              ) : (
                <button
                  className="svc-run"
                  title={def.provider === "claude" && def.run !== "pane" && !hasKey ? "Add an Anthropic API key first" : "Run"}
                  disabled={!def.prompt.trim() || !def.folder || (def.provider === "claude" && def.run !== "pane" && !hasKey)}
                  onClick={() => startLoop(id)}
                >
                  <Play size={13} />
                </button>
              )}
              <button className="svc-run" title="Logs" onClick={() => setLogsFor(logsFor === id ? null : id)}>
                <ScrollText size={13} />
              </button>
              <button
                className="svc-del"
                title="Delete automation"
                onClick={() => {
                  stopLoop(id);
                  useLoops.getState().remove(id);
                }}
              >
                <Trash2 size={13} />
              </button>
              </span>
            </div>

            {openId === id && (
            <div className="loop-card-body">
            <div className="loop-section">
            <div className="loop-section-head">Task</div>
            <textarea
              className="svc-in loop-prompt"
              placeholder="What should the agent do each iteration?"
              rows={3}
              value={def.prompt}
              onChange={(e) => update(id, { prompt: e.target.value })}
            />
            </div>

            {active && (
              <div className="loop-live-hint">
                <RefreshCw size={12} />
                <span>
                  Running — edits apply on the <strong>next iteration</strong>. Backend, run mode and
                  folder are locked until you stop it.
                </span>
              </div>
            )}

            <div className="loop-section">
            <div className="loop-section-head">Backend</div>
            <div className="loop-chips">
              {BACKENDS.map((b) => {
                const on = def.provider === b.id || (def.provider === "claude-hooks" && b.id === "claude-sub");
                return (
                  <button
                    key={b.id}
                    className={`loop-chip${on ? " on" : ""}`}
                    disabled={active}
                    title={active ? "Stop the automation to change its backend" : undefined}
                    onClick={() => {
                      const patch: Partial<LoopDef> = { provider: b.id };
                      if (!paneCapable(b.id) && def.run === "pane") patch.run = "headless";
                      update(id, patch);
                    }}
                  >
                    <img src={b.logo} alt="" />
                    <span className="loop-chip-name">
                      {b.label}
                      {b.sub && <em>{b.sub}</em>}
                    </span>
                  </button>
                );
              })}
            </div>
            </div>

            <div className="loop-section">
            <div className="loop-section-head">How it runs</div>
            <div className="loop-seg-row">
              <div className="loop-seg-group">
                <span className="loop-field-label">Run as</span>
                <Seg
                  value={def.run}
                  disabled={active}
                  onChange={(v) =>
                    update(id, {
                      run: v,
                      // interval doesn't apply to interactive sessions — fall back gracefully
                      ...(v === "pane" && def.mode === "interval" ? { mode: "until-done" as const } : {}),
                    })
                  }
                  opts={[
                    { v: "headless", label: "Headless" },
                    {
                      v: "pane",
                      label: "In a terminal",
                      off: !paneCapable(def.provider),
                      hint: !paneCapable(def.provider)
                        ? "This backend has no interactive session mode"
                        : undefined,
                    },
                  ]}
                />
              </div>
              <div className="loop-seg-group">
                <span className="loop-field-label">When</span>
                <Seg
                  value={def.mode}
                  onChange={(v) => update(id, { mode: v })}
                  opts={[
                    { v: "until-done", label: "Until done" },
                    {
                      v: "interval",
                      label: "Interval",
                      off: def.run === "pane",
                      hint: def.run === "pane" ? "Interactive sessions can't re-launch on an interval" : undefined,
                    },
                    { v: "cron", label: "Schedule" },
                    { v: "manual", label: "Once" },
                  ]}
                />
              </div>
            </div>

            {def.mode === "interval" && (
              <div className="loop-grid">
                <label className="loop-field">
                  <span>Every (seconds)</span>
                  <NumField
                    key={`${id}-int`}
                    value={def.intervalSec ?? 60}
                    min={5}
                    onCommit={(n) => update(id, { intervalSec: n })}
                  />
                </label>
              </div>
            )}
            {def.mode === "cron" && (
              <>
                <div className="loop-grid">
                  <label className="loop-field">
                    <span>Cron expression</span>
                    <input
                      className="svc-in"
                      placeholder="*/30 9-18 * * 1-5"
                      value={def.schedule?.cron ?? ""}
                      onChange={(e) => updateSched(id, { cron: e.target.value || undefined })}
                    />
                  </label>
                  <label className="loop-field">
                    <span>…or every (minutes)</span>
                    <input
                      className="svc-in"
                      type="number"
                      min={1}
                      placeholder="60"
                      value={def.schedule?.everyMin ?? ""}
                      onChange={(e) => updateSched(id, { everyMin: e.target.value ? +e.target.value : undefined })}
                    />
                  </label>
                  <label className="loop-field">
                    <span>…or daily at</span>
                    <input
                      className="svc-in"
                      placeholder="HH:MM"
                      value={def.schedule?.dailyAt ?? ""}
                      onChange={(e) => updateSched(id, { dailyAt: e.target.value || undefined })}
                    />
                  </label>
                </div>
                <div className="loop-next-fire">{nextFirePreview(def.schedule)}</div>
              </>
            )}

            <div className="loop-grid">
              <label className="loop-field">
                <span>Model</span>
                <input
                  className="svc-in"
                  placeholder="default"
                  value={def.model ?? ""}
                  onChange={(e) => update(id, { model: e.target.value || undefined })}
                />
              </label>
              <label className="loop-field">
                <span>Session</span>
                <select className="set-select" value={def.session} onChange={(e) => update(id, { session: e.target.value as LoopDef["session"] })}>
                  <option value="fresh">Fresh each run</option>
                  <option value="continue">Continue session</option>
                </select>
              </label>
              <label className="loop-field">
                <span>Permission</span>
                <select className="set-select" value={def.permissionMode ?? "acceptEdits"} onChange={(e) => update(id, { permissionMode: e.target.value })}>
                  <option value="plan">Plan (read-only)</option>
                  <option value="acceptEdits">Accept edits</option>
                  <option value="bypass">Bypass</option>
                  <option value="default">Ask</option>
                </select>
              </label>
            </div>

            {def.run === "pane" ? (
              <div className="svc-hint loop-sub-note">
                Launches a real interactive session in a terminal on the Runs tab, seeded with your
                task — it can ask you questions or for approval, which you answer{" "}
                <strong>right in the terminal</strong>. Claude self-loops via <code>/goal</code> and
                pings you when it needs input; Codex and Gemini run as a live seeded session.
              </div>
            ) : def.provider === "claude-sub" ? (
              <div className="svc-hint loop-sub-note">
                Runs headless <code>claude -p</code> on your logged-in subscription — no API key, the
                same CLI the terminal panes use. Each iteration is one turn; it keeps going until your
                until-check passes, the sentinel appears, or it hits max iterations.
              </div>
            ) : null}
            </div>

            <div className="loop-section">
            <div className="loop-section-head">
              When it stops <span className="loop-section-sub">at least one hard limit — it can never run forever</span>
            </div>
            <div className="loop-stops">
              <label className="loop-field">
                <span>Max iterations</span>
                <NumField
                  key={`${id}-max`}
                  value={def.stop.maxIterations}
                  min={1}
                  onCommit={(n) => updateStop(id, { maxIterations: n })}
                />
              </label>
              <label className="loop-field">
                <span>Stop when output has</span>
                <input
                  className="svc-in"
                  placeholder='e.g. "LOOP_DONE"'
                  value={def.stop.sentinel ?? ""}
                  onChange={(e) => updateStop(id, { sentinel: e.target.value || undefined })}
                />
              </label>
              <label className="loop-field">
                <span>Time budget (min)</span>
                <input
                  className="svc-in"
                  type="number"
                  min={0}
                  placeholder="none"
                  value={def.stop.timeBudgetMin ?? ""}
                  onChange={(e) => updateStop(id, { timeBudgetMin: e.target.value ? +e.target.value : undefined })}
                />
              </label>
              <label
                className="loop-field"
                title={BUDGET_BACKENDS.has(def.provider) ? undefined : "Only Claude and Codex report token usage"}
              >
                <span>Token budget</span>
                <input
                  className="svc-in"
                  type="number"
                  min={0}
                  placeholder={BUDGET_BACKENDS.has(def.provider) ? "none" : "n/a for this backend"}
                  disabled={!BUDGET_BACKENDS.has(def.provider)}
                  value={def.stop.tokenBudget ?? ""}
                  onChange={(e) => updateStop(id, { tokenBudget: e.target.value ? +e.target.value : undefined })}
                />
              </label>
              <label className="loop-field loop-field-wide">
                <span>Stop when this passes</span>
                <input
                  className="svc-in"
                  placeholder="e.g. cargo build"
                  value={def.stop.untilCheck ?? ""}
                  onChange={(e) => updateStop(id, { untilCheck: e.target.value || undefined })}
                />
              </label>
            </div>
            <ToggleRow
              label="Stop if it stalls"
              desc="3 iterations with no new output ends the run"
              on={def.stop.noProgress}
              onChange={(b) => updateStop(id, { noProgress: b })}
            />
            </div>

            <div className="loop-section">
            <div className="loop-section-head">Options</div>
            <ToggleRow
              label="Isolate edits (worktree)"
              desc="run in a throwaway branch — review the diff before it touches your tree"
              on={def.worktree}
              onChange={(b) => update(id, { worktree: b })}
            />
            <ToggleRow
              label="Auto-start on open"
              desc="kick off whenever HyprSpace launches"
              on={def.enabled}
              onChange={(b) => update(id, { enabled: b })}
            />
            <button
              className="loop-folder"
              disabled={active}
              onClick={() => void browseFolder(id)}
              title={active ? "Stop the automation to change its folder" : def.folder || "Pick a folder"}
            >
              <FolderOpen size={13} />
              <span>{def.folder || "Pick a folder…"}</span>
            </button>
            </div>

            <div className="loop-status">
              <span className={`loop-status-badge s-${status}`}>{STATUS_LABEL[status] ?? status}</span>
              {run && run.iteration > 0 && <span>iteration {run.iteration}</span>}
              {run?.nextRunAt && status === "running" ? (
                <span className="loop-next-fire">next run {fmtWhen(run.nextRunAt)}</span>
              ) : null}
              {run && (run.costUsed || run.tokensUsed) ? (
                <span className="loop-usage" title="cost · tokens this run">
                  {run.costUsed ? `$${run.costUsed.toFixed(3)}` : ""}
                  {run.tokensUsed ? `${run.costUsed ? " · " : ""}${fmtTokens(run.tokensUsed)} tok` : ""}
                </span>
              ) : null}
              {run?.filesChanged ? (
                <span className="loop-diff" title="files this run changed">
                  {run.filesChanged} file{run.filesChanged === 1 ? "" : "s"} · +{run.additions} −{run.deletions}
                </span>
              ) : null}
              {run?.lastResult && <span className="loop-last" title={run.lastResult}>{run.lastResult}</span>}
              {run?.worktreePath && (
                <button className="loop-review" title={`Open the isolated worktree:\n${run.worktreePath}`} onClick={() => revealLoopWorktree(id)}>
                  <FolderGit2 size={12} /> Review changes
                </button>
              )}
            </div>

            {logsFor === id && (
              <div className="loop-logs">
                {(run?.logs ?? []).length === 0 ? (
                  <div className="loop-logs-empty">No output yet.</div>
                ) : (
                  (run?.logs ?? []).map((l, i) => <LogLine key={i} line={l} />)
                )}
              </div>
            )}
            </div>
            )}
          </div>
        );
      })}
      </div>
    </div>
  );
}
