import { useEffect, useState, type ReactNode } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useLoops, newLoop, type LoopDef, type LoopStop } from "../stores/loops";
import { startLoop, stopLoop, pauseLoop, revealLoopWorktree } from "../lib/loops";
import { LOOP_TEMPLATES, type LoopTemplate } from "../lib/loopTemplates";
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
} from "lucide-react";

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

// Settings → Loops. Create/edit loops and drive them live (start/stop/pause, iteration count, logs).
export function LoopsManager() {
  const loops = useLoops((s) => s.loops);
  const runs = useLoops((s) => s.runs);
  const ids = Object.keys(loops);
  const [logsFor, setLogsFor] = useState<string | null>(null);
  const [listRef] = useAutoAnimate(); // smooth add/remove of loop cards

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
    def.name = "New loop";
    useLoops.getState().upsert(def);
  };
  const addTemplate = (t: LoopTemplate) => {
    useLoops.getState().upsert(t.build(activeCwd || ""));
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
        <span className="set-label">Loops</span>
        <button className="btn" onClick={add}>
          <Plus size={14} /> New loop
        </button>
      </div>
      <div className="svc-hint" style={{ marginBottom: 14 }}>
        Run an agent on a schedule, on an interval, or until a goal is met. Every loop needs a stop
        limit, so it can never run forever. Loops run while HyprSpace is open.
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
            <span className="loop-key-note">used only by Claude loops; never your subscription</span>
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
            <div className="loop-top">
              <span className={`loop-dot s-${status}`} />
              <input
                className="svc-in loop-name"
                placeholder="Loop name"
                value={def.name}
                onChange={(e) => update(id, { name: e.target.value })}
              />
              {active ? (
                <>
                  <button className="svc-stop" title="Stop" onClick={() => stopLoop(id)}>
                    <Square size={13} />
                  </button>
                  <button
                    className="svc-run"
                    title={status === "paused" ? "Resume" : "Pause"}
                    onClick={() => pauseLoop(id, status !== "paused")}
                  >
                    <Pause size={13} />
                  </button>
                </>
              ) : (
                <button
                  className="svc-run"
                  title={def.provider === "claude" && !hasKey ? "Add an Anthropic API key first" : "Run"}
                  disabled={!def.prompt.trim() || !def.folder || (def.provider === "claude" && !hasKey)}
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
                title="Delete loop"
                onClick={() => {
                  stopLoop(id);
                  useLoops.getState().remove(id);
                }}
              >
                <Trash2 size={13} />
              </button>
            </div>

            <textarea
              className="svc-in loop-prompt"
              placeholder="What should the agent do each iteration?"
              rows={2}
              value={def.prompt}
              onChange={(e) => update(id, { prompt: e.target.value })}
            />

            <div className="loop-grid">
              <label className="loop-field">
                <span>Backend</span>
                <select className="set-select" value={def.provider} onChange={(e) => update(id, { provider: e.target.value as LoopDef["provider"] })}>
                  <option value="claude">Claude (API key)</option>
                  <option value="codex">Codex</option>
                  <option value="gemini">Gemini</option>
                </select>
              </label>
              <label className="loop-field">
                <span>Mode</span>
                <select className="set-select" value={def.mode} onChange={(e) => update(id, { mode: e.target.value as LoopDef["mode"] })}>
                  <option value="until-done">Until done</option>
                  <option value="interval">Interval</option>
                  <option value="cron">Schedule</option>
                  <option value="manual">Manual only</option>
                </select>
              </label>

              {def.mode === "interval" && (
                <label className="loop-field">
                  <span>Every (seconds)</span>
                  <input
                    className="svc-in"
                    type="number"
                    min={5}
                    value={def.intervalSec ?? 60}
                    onChange={(e) => update(id, { intervalSec: Math.max(5, +e.target.value || 60) })}
                  />
                </label>
              )}
              {def.mode === "cron" && (
                <>
                  <label className="loop-field">
                    <span>Every (minutes)</span>
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
                    <span>Or daily at</span>
                    <input
                      className="svc-in"
                      placeholder="HH:MM"
                      value={def.schedule?.dailyAt ?? ""}
                      onChange={(e) => updateSched(id, { dailyAt: e.target.value || undefined })}
                    />
                  </label>
                </>
              )}

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

            <div className="loop-stops">
              <label className="loop-field">
                <span>Max iterations</span>
                <input
                  className="svc-in"
                  type="number"
                  min={1}
                  value={def.stop.maxIterations}
                  onChange={(e) => updateStop(id, { maxIterations: Math.max(1, +e.target.value || 1) })}
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
              <label className="loop-field">
                <span>Token budget</span>
                <input
                  className="svc-in"
                  type="number"
                  min={0}
                  placeholder="none"
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

            <div className="loop-flags">
              <label className="svc-auto">
                <input type="checkbox" checked={def.stop.noProgress} onChange={(e) => updateStop(id, { noProgress: e.target.checked })} />
                Stop if it stalls
              </label>
              <label className="svc-auto">
                <input type="checkbox" checked={def.worktree} onChange={(e) => update(id, { worktree: e.target.checked })} />
                Isolate edits (worktree)
              </label>
              <label className="svc-auto">
                <input type="checkbox" checked={def.enabled} onChange={(e) => update(id, { enabled: e.target.checked })} />
                Auto-start on open
              </label>
            </div>

            <button className="loop-folder" onClick={() => void browseFolder(id)} title={def.folder || "Pick a folder"}>
              <FolderOpen size={13} />
              <span>{def.folder || "Pick a folder…"}</span>
            </button>

            <div className="loop-status">
              <span className={`loop-status-badge s-${status}`}>{STATUS_LABEL[status] ?? status}</span>
              {run && run.iteration > 0 && <span>iteration {run.iteration}</span>}
              {run && (run.costUsed || run.tokensUsed) ? (
                <span className="loop-usage" title="cost · tokens this run">
                  {run.costUsed ? `$${run.costUsed.toFixed(3)}` : ""}
                  {run.tokensUsed ? `${run.costUsed ? " · " : ""}${fmtTokens(run.tokensUsed)} tok` : ""}
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
                  (run?.logs ?? []).map((l, i) => (
                    <div className="loop-logs-line" key={i}>
                      {l || " "}
                    </div>
                  ))
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
