import { useEffect, useRef, useState, type ComponentType } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useSettings, type ClaudePermission } from "../stores/settings";
import { useLaunchPresets, type LaunchPreset } from "../stores/launchPresets";
import { pickFolder, getHomeDir } from "../api";
import { claudeCmd, geminiCmd, codexCmd, opencodeCmd, grokCmd } from "../actions";
import { getLayout } from "../lib/grid";
import { Terminal as TerminalIcon, Folder, Minus, Plus, Rocket, Bookmark, X, Layers } from "lucide-react";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";

type AgentKey = "claude" | "codex" | "gemini" | "opencode" | "grok" | "terminal";
// agents wear their real brand marks (svg); plain terminal keeps a lucide glyph
const AGENTS: { key: AgentKey; name: string; desc: string; iconSrc?: string; Icon?: ComponentType<{ size?: number }> }[] = [
  { key: "claude", name: "Claude", desc: "Anthropic's coding agent", iconSrc: claudeLogo },
  { key: "codex", name: "Codex", desc: "OpenAI's Codex CLI", iconSrc: openaiLogo },
  { key: "gemini", name: "Gemini", desc: "Google's Gemini CLI", iconSrc: geminiLogo },
  { key: "opencode", name: "OpenCode", desc: "SST's open-source agent", iconSrc: opencodeLogo },
  { key: "grok", name: "Grok", desc: "xAI's Grok Build CLI", iconSrc: grokLogo },
  { key: "terminal", name: "Terminal", desc: "Plain shell", Icon: TerminalIcon },
];
const byKey = (k: AgentKey) => AGENTS.find((a) => a.key === k)!;

const TILES = [1, 2, 4, 6, 8, 10, 12];
const GRID: Record<number, string> = { 1: "1×1", 2: "2×1", 4: "2×2", 6: "3×2", 8: "4×2", 10: "5×2", 12: "4×3" };
const zero = (): Record<AgentKey, number> => ({ claude: 0, codex: 0, gemini: 0, opencode: 0, grok: 0, terminal: 0 });

// Launch a whole workspace at once: pick a folder, choose how many terminals, fan them out across
// agents. The fast path to "many Claude instances working in parallel" instead of waiting on one.
// Lives as its own full page (the "launch" view) rather than a modal.
export function LaunchWorkspace() {
  const close = useUi((s) => s.closeLaunch);
  const presets = useLaunchPresets((s) => s.presets);
  const [presetRef] = useAutoAnimate(); // smooth add/remove of preset chips

  const [folder, setFolder] = useState("");
  const [count, setCount] = useState(4);
  const [agents, setAgents] = useState<Record<AgentKey, number>>({ claude: 4, codex: 0, gemini: 0, opencode: 0, grok: 0, terminal: 0 });
  const [claudeMode, setClaudeMode] = useState<ClaudePermission>("acceptEdits");

  // fresh start every time the page opens (it mounts/unmounts with the view)
  useEffect(() => {
    void useLaunchPresets.getState().load();
    setClaudeMode(useSettings.getState().claudePermission);
    // prefill with the active project's folder, else the home dir
    const ws = useWorkspaces.getState();
    const active = ws.workspaces.find((w) => w.id === ws.activeId);
    if (active && active.kind !== "open" && active.cwd) setFolder(active.cwd);
    else void getHomeDir().then((h) => setFolder((cur) => cur || h)).catch(() => {});
  }, []);

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
    const ai: AgentKey[] = ["claude", "codex", "gemini", "opencode", "grok"];
    for (let i = 0; i < count; i++) a[ai[i % ai.length]]++;
    setAgents(a);
  };
  const clear = () => setAgents(zero());

  const applyPreset = (p: LaunchPreset) => {
    setFolder(p.folder);
    setCount(p.count);
    setAgents({ ...zero(), ...p.agents });
    setClaudeMode(p.claudeMode);
  };
  const savePreset = () => {
    if (!folder) return;
    useLaunchPresets.getState().save({ name: folderName, folder, count, agents, claudeMode });
  };

  // the ordered pane list — same fan-out launch() builds, so the preview mirrors reality
  const slots: AgentKey[] = [];
  for (let i = 0; i < agents.claude; i++) slots.push("claude");
  for (let i = 0; i < agents.codex; i++) slots.push("codex");
  for (let i = 0; i < agents.gemini; i++) slots.push("gemini");
  for (let i = 0; i < agents.opencode; i++) slots.push("opencode");
  for (let i = 0; i < agents.grok; i++) slots.push("grok");
  for (let i = 0; i < agents.terminal; i++) slots.push("terminal");
  while (slots.length < count) slots.push("terminal"); // leftover slots become plain terminals
  const preview = getLayout(count);

  const launch = () => {
    if (!folder) return;
    const id = useWorkspaces.getState().addWorkspace(folderName, folder);
    const cmds: (string | undefined)[] = [];
    for (let i = 0; i < agents.claude; i++) cmds.push(claudeCmd(claudeMode));
    for (let i = 0; i < agents.codex; i++) cmds.push(codexCmd());
    for (let i = 0; i < agents.gemini; i++) cmds.push(geminiCmd());
    for (let i = 0; i < agents.opencode; i++) cmds.push(opencodeCmd());
    for (let i = 0; i < agents.grok; i++) cmds.push(grokCmd());
    for (let i = 0; i < agents.terminal; i++) cmds.push(undefined);
    // any slots left over from the chosen count become plain terminals
    while (cmds.length < count) cmds.push(undefined);
    cmds.forEach((cmd) => useWorkspaces.getState().addSession(id, cmd, folder));
    useUi.getState().goSpace(); // navigating to the new space leaves the launch view
  };

  // Esc closes, Ctrl/Cmd+Enter launches — full page has no overlay to click away, so bind globally.
  // launch reads component state, so reach it through a ref to dodge a stale closure.
  const launchRef = useRef(launch);
  launchRef.current = launch;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        launchRef.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  return (
    <div className="launchpage">
      <div className="lpg-scroll">
        <div className="lpg-inner">
          <div className="lpg-hero">
            <div className="lpg-mark">
              <Layers size={26} />
            </div>
            <div className="lpg-heading">
              <h1 className="lpg-title">Launch a workspace</h1>
              <p className="lpg-sub">
                Pick a folder and fan out as many agents as you want — they open side by side, ready to work in parallel.
              </p>
            </div>
          </div>

          {presets.length > 0 && (
            <div className="lpg-presets-row" ref={presetRef}>
              <span className="lpg-presets-label">Presets</span>
              {presets.map((p) => (
                <span key={p.id} className="lw-preset">
                  <button className="lw-preset-apply" onClick={() => applyPreset(p)} title={`${p.folder} · ${p.count} terminals`}>
                    <Bookmark size={12} /> {p.name}
                    <span className="lw-preset-n">{p.count}</span>
                  </button>
                  <button className="lw-preset-del" title="Delete preset" onClick={() => useLaunchPresets.getState().remove(p.id)}>
                    <X size={11} />
                  </button>
                </span>
              ))}
            </div>
          )}

          <div className="lpg-cols">
            <div className="lpg-col">
              <section className="lpg-card">
                <div className="lpg-card-head">
                  <span className="lpg-card-title">Working folder</span>
                </div>
                <div className="lpg-folder">
                  <button className="btn" onClick={() => void pickFolder().then((f) => f && setFolder(f))}>
                    <Folder size={14} /> Choose folder…
                  </button>
                  <div className={`lpg-path${folder ? "" : " lpg-path-empty"}`} title={folder || undefined}>
                    {folder || "No folder chosen yet"}
                  </div>
                </div>
              </section>

              <section className="lpg-card">
                <div className="lpg-card-head">
                  <span className="lpg-card-title">Layout</span>
                  <span className="lpg-card-note">
                    {GRID[count] ?? `${count}`} grid · {count} {count === 1 ? "terminal" : "terminals"}
                  </span>
                </div>
                <div className="lw-tiles">
                  {TILES.map((n) => (
                    <button key={n} className={`lw-tile${count === n ? " active" : ""}`} onClick={() => pickCount(n)}>
                      <span className="lw-tile-dots" style={{ ["--n" as string]: n }}>
                        {Array.from({ length: n }).map((_, i) => (
                          <i key={i} />
                        ))}
                      </span>
                      <span className="lw-tile-num">{n}</span>
                    </button>
                  ))}
                </div>
                <div className="lpg-preview" aria-hidden>
                  <div className="lpg-preview-grid" style={{ gridTemplateColumns: preview.cols }}>
                    {slots.slice(0, count).map((k, i) => {
                      const a = byKey(k);
                      return (
                        <div key={i} className={`lpg-cell lpg-cell-${k}`} style={{ gridColumn: preview.span(i) }} title={a.name}>
                          {a.iconSrc ? (
                            <img src={a.iconSrc} alt="" />
                          ) : a.Icon ? (
                            <a.Icon size={15} />
                          ) : null}
                        </div>
                      );
                    })}
                  </div>
                </div>
              </section>
            </div>

            <div className="lpg-col">
              <section className="lpg-card">
                <div className="lpg-card-head">
                  <span className="lpg-card-title">Agents</span>
                  <span className={`lw-meter${assigned >= count ? " full" : ""}`}>
                    {assigned} / {count} filled
                  </span>
                </div>
                <div className="lw-quick">
                  <button onClick={allClaude}>All Claude</button>
                  <button onClick={oneEach}>One of each</button>
                  <button onClick={splitEvenly}>Split evenly</button>
                  <button className="lw-quick-clear" onClick={clear}>
                    Clear
                  </button>
                </div>
                <div className="lpg-agents">
                  {AGENTS.map((a) => (
                    <div className="lpg-agent" key={a.key}>
                      <span className="lpg-agent-ico">
                        {a.iconSrc ? <img src={a.iconSrc} alt="" /> : a.Icon ? <a.Icon size={17} /> : null}
                      </span>
                      <span className="lpg-agent-body">
                        <span className="lpg-agent-name">{a.name}</span>
                        <span className="lpg-agent-desc">{a.desc}</span>
                      </span>
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
              </section>
            </div>
          </div>
        </div>
      </div>

      <div className="lpg-foot">
        <div className="lpg-foot-inner">
          <span className="lpg-foot-hint">
            Opens <b>{folderName}</b> with {count} {count === 1 ? "terminal" : "terminals"}
            {assigned > 0 ? ` (${assigned} agent${assigned === 1 ? "" : "s"})` : ""}. <kbd>Esc</kbd> to cancel
          </span>
          <div className="lpg-foot-actions">
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
