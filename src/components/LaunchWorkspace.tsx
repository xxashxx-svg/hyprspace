import { useEffect, useState, type ReactNode } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useSettings, type ClaudePermission } from "../stores/settings";
import { useLaunchPresets, type LaunchPreset } from "../stores/launchPresets";
import { pickFolder, getHomeDir } from "../api";
import { claudeCmd, geminiCmd, codexCmd } from "../actions";
import { Sparkles, Gem, Bot, Terminal as TerminalIcon, Folder, Minus, Plus, Rocket, Bookmark, X } from "lucide-react";

type AgentKey = "claude" | "codex" | "gemini" | "terminal";
const AGENTS: { key: AgentKey; name: string; icon: ReactNode }[] = [
  { key: "claude", name: "Claude", icon: <Sparkles size={15} /> },
  { key: "codex", name: "Codex", icon: <Bot size={15} /> },
  { key: "gemini", name: "Gemini", icon: <Gem size={15} /> },
  { key: "terminal", name: "Terminal", icon: <TerminalIcon size={15} /> },
];

const TILES = [1, 2, 4, 6, 8, 10, 12];
const GRID: Record<number, string> = { 1: "1×1", 2: "2×1", 4: "2×2", 6: "3×2", 8: "4×2", 10: "5×2", 12: "4×3" };
const zero = (): Record<AgentKey, number> => ({ claude: 0, codex: 0, gemini: 0, terminal: 0 });

// Launch a whole workspace at once: pick a folder, choose how many terminals, fan them out across
// agents. The fast path to "many Claude instances working in parallel" instead of waiting on one.
export function LaunchWorkspace() {
  const open = useUi((s) => s.launchOpen);
  const close = useUi((s) => s.closeLaunch);
  const presets = useLaunchPresets((s) => s.presets);

  const [folder, setFolder] = useState("");
  const [count, setCount] = useState(4);
  const [agents, setAgents] = useState<Record<AgentKey, number>>({ claude: 4, codex: 0, gemini: 0, terminal: 0 });
  const [claudeMode, setClaudeMode] = useState<ClaudePermission>("acceptEdits");

  useEffect(() => {
    if (!open) return;
    void useLaunchPresets.getState().load();
    setCount(4);
    setAgents({ claude: 4, codex: 0, gemini: 0, terminal: 0 });
    setClaudeMode(useSettings.getState().claudePermission);
    // prefill with the active project's folder, else the home dir
    const ws = useWorkspaces.getState();
    const active = ws.workspaces.find((w) => w.id === ws.activeId);
    if (active && active.kind !== "open" && active.cwd) setFolder(active.cwd);
    else void getHomeDir().then((h) => setFolder((cur) => cur || h)).catch(() => {});
  }, [open]);

  if (!open) return null;

  const assigned = AGENTS.reduce((n, a) => n + agents[a.key], 0);
  const folderName = folder.split(/[\\/]/).filter(Boolean).pop() || "workspace";

  const pickCount = (n: number) => {
    setCount(n);
    // keep the mix if it still fits, else reset to all-Claude at the new size (predictable)
    setAgents((a) => (AGENTS.reduce((s, x) => s + a[x.key], 0) > n ? { ...zero(), claude: n } : a));
  };
  const bump = (k: AgentKey, d: number) =>
    setAgents((a) => {
      const cur = AGENTS.reduce((s, x) => s + a[x.key], 0);
      if (d > 0 && cur >= count) return a; // can't exceed the chosen terminal count
      return { ...a, [k]: Math.max(0, a[k] + d) };
    });

  // quick-fill presets over the chosen terminal count
  const allClaude = () => setAgents({ ...zero(), claude: count });
  const oneEach = () => {
    const a = zero();
    for (let i = 0; i < count; i++) a[AGENTS[i % AGENTS.length].key]++;
    setAgents(a);
  };
  const splitEvenly = () => {
    const a = zero();
    const ai: AgentKey[] = ["claude", "codex", "gemini"];
    for (let i = 0; i < count; i++) a[ai[i % ai.length]]++;
    setAgents(a);
  };
  const clear = () => setAgents(zero());

  const applyPreset = (p: LaunchPreset) => {
    setFolder(p.folder);
    setCount(p.count);
    setAgents({ ...p.agents });
    setClaudeMode(p.claudeMode);
  };
  const savePreset = () => {
    if (!folder) return;
    useLaunchPresets.getState().save({ name: folderName, folder, count, agents, claudeMode });
  };

  const launch = () => {
    if (!folder) return;
    const id = useWorkspaces.getState().addWorkspace(folderName, folder);
    const cmds: (string | undefined)[] = [];
    for (let i = 0; i < agents.claude; i++) cmds.push(claudeCmd(claudeMode));
    for (let i = 0; i < agents.codex; i++) cmds.push(codexCmd());
    for (let i = 0; i < agents.gemini; i++) cmds.push(geminiCmd());
    for (let i = 0; i < agents.terminal; i++) cmds.push(undefined);
    // any slots left over from the chosen count become plain terminals
    while (cmds.length < count) cmds.push(undefined);
    cmds.forEach((cmd) => useWorkspaces.getState().addSession(id, cmd, folder));
    useUi.getState().goSpace();
    close();
  };

  return (
    <div className="np-overlay" onMouseDown={close}>
      <div
        className="np lw"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") close();
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") launch();
        }}
      >
        <div className="np-head">
          <span className="np-title">Launch a workspace</span>
          <span className="np-sub">Pick a folder and fan out as many agents as you want.</span>
        </div>

        <div className="np-body">
          {presets.length > 0 && (
            <div className="np-field">
              <span className="np-label">Presets</span>
              <div className="lw-presets">
                {presets.map((p) => (
                  <span key={p.id} className="lw-preset">
                    <button className="lw-preset-apply" onClick={() => applyPreset(p)} title={`${p.folder} · ${p.count} terminals`}>
                      <Bookmark size={12} /> {p.name}
                      <span className="lw-preset-n">{p.count}</span>
                    </button>
                    <button
                      className="lw-preset-del"
                      title="Delete preset"
                      onClick={() => useLaunchPresets.getState().remove(p.id)}
                    >
                      <X size={11} />
                    </button>
                  </span>
                ))}
              </div>
            </div>
          )}

          <div className="np-field">
            <span className="np-label">Working folder</span>
            <div className="np-browse">
              <button className="btn" onClick={() => void pickFolder().then((f) => f && setFolder(f))}>
                <Folder size={14} /> Choose folder…
              </button>
            </div>
            <div className={`np-path${folder ? "" : " np-path-empty"}`} title={folder || undefined}>
              {folder || "No folder chosen yet"}
            </div>
          </div>

          <div className="np-field">
            <span className="np-label">
              How many terminals <span className="lw-grid-label">{GRID[count] ?? `${count}`} grid</span>
            </span>
            <div className="lw-tiles">
              {TILES.map((n) => (
                <button
                  key={n}
                  className={`lw-tile${count === n ? " active" : ""}`}
                  onClick={() => pickCount(n)}
                >
                  <span className="lw-tile-dots" style={{ ["--n" as string]: n }}>
                    {Array.from({ length: n }).map((_, i) => (
                      <i key={i} />
                    ))}
                  </span>
                  <span className="lw-tile-num">{n}</span>
                </button>
              ))}
            </div>
          </div>

          <div className="np-field">
            <span className="np-label">
              Agents
              <span className={`lw-meter${assigned >= count ? " full" : ""}`}>
                {assigned} / {count} filled
              </span>
            </span>
            <div className="lw-quick">
              <button onClick={allClaude}>All Claude</button>
              <button onClick={oneEach}>One of each</button>
              <button onClick={splitEvenly}>Split evenly</button>
              <button className="lw-quick-clear" onClick={clear}>
                Clear
              </button>
            </div>
            <div className="np-agents">
              {AGENTS.map((a) => (
                <div className="np-agent" key={a.key}>
                  <span className="np-agent-ico">{a.icon}</span>
                  <span className="np-agent-name">{a.name}</span>
                  <div className="np-step">
                    <button onClick={() => bump(a.key, -1)} disabled={agents[a.key] === 0} aria-label={`fewer ${a.name}`}>
                      <Minus size={13} />
                    </button>
                    <span>{agents[a.key]}</span>
                    <button onClick={() => bump(a.key, 1)} disabled={assigned >= count} aria-label={`more ${a.name}`}>
                      <Plus size={13} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
            {agents.claude > 0 && (
              <label className="np-claude-mode">
                <span>Claude permission mode</span>
                <select value={claudeMode} onChange={(e) => setClaudeMode(e.target.value as ClaudePermission)}>
                  <option value="default">Ask each time</option>
                  <option value="acceptEdits">Accept edits</option>
                  <option value="plan">Plan mode</option>
                  <option value="bypass">Bypass permissions</option>
                </select>
              </label>
            )}
          </div>
        </div>

        <div className="np-foot">
          <span className="np-foot-hint">
            Opens {folderName} with {count} {count === 1 ? "terminal" : "terminals"}
            {assigned > 0 ? ` (${assigned} agent${assigned === 1 ? "" : "s"})` : ""}.
          </span>
          <div className="np-actions">
            <button className="btn ghost" onClick={savePreset} disabled={!folder} title="Save this folder + grid + agents as a preset">
              <Bookmark size={14} /> Save preset
            </button>
            <button className="btn" onClick={close}>
              Cancel
            </button>
            <button className="btn primary" onClick={launch} disabled={!folder}>
              <Rocket size={14} /> Launch {count}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
