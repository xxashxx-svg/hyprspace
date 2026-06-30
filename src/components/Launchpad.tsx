import { useEffect, type ComponentType } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useProjectConfigs, folderKey } from "../stores/projectConfig";
import { useServices, serviceId } from "../stores/services";
import { useLaunchPresets, type LaunchPreset } from "../stores/launchPresets";
import { launchTask, maybeAutostart } from "../lib/startup";
import { pickFolders } from "../api";
import { claudeCmd, geminiCmd, codexCmd, opencodeCmd } from "../actions";
import { isWindows } from "../platform";
import { Logo } from "./Logo";
import { Terminal as TerminalIcon, Play, ScrollText, Layers, Bookmark } from "lucide-react";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import linuxLogo from "../assets/brand/linux.svg";

// the tools you can drop into a space, shown as app-style icons in the dock. the AI providers
// wear their real brand marks (svg); plain terminal keeps a lucide glyph.
type Tool = {
  key: string;
  name: string;
  desc: string;
  iconSrc?: string;
  Icon?: ComponentType<{ size?: number }>;
  cmd: () => string | undefined;
  primary?: boolean;
  winOnly?: boolean;
};
const TOOLS: Tool[] = [
  { key: "claude", name: "Claude", desc: "Anthropic's coding agent", iconSrc: claudeLogo, cmd: () => claudeCmd(), primary: true },
  { key: "gemini", name: "Gemini", desc: "Google's Gemini CLI", iconSrc: geminiLogo, cmd: () => geminiCmd() },
  { key: "codex", name: "Codex", desc: "OpenAI's Codex CLI", iconSrc: openaiLogo, cmd: () => codexCmd() },
  { key: "opencode", name: "OpenCode", desc: "SST's open-source agent", iconSrc: opencodeLogo, cmd: () => opencodeCmd() },
  { key: "wsl", name: "WSL", desc: "Linux shell", iconSrc: linuxLogo, cmd: () => "wsl", winOnly: true },
  { key: "terminal", name: "Terminal", desc: "Plain shell", Icon: TerminalIcon, cmd: () => undefined },
];

interface Props {
  wsId: string;
  name: string;
  kind: string;
  cwd: string;
}

export function Launchpad({ wsId, name, kind, cwd }: Props) {
  const isOpen = kind === "open";
  const presets = useLaunchPresets((s) => s.presets);
  useEffect(() => {
    void useLaunchPresets.getState().load();
  }, []);

  // launch one tool here: open spaces ask for folder(s) first, projects drop straight into cwd
  const launch = async (cmd?: string) => {
    if (isOpen) {
      const folders = await pickFolders();
      folders.forEach((f) => useWorkspaces.getState().addSession(wsId, cmd, f));
    } else {
      useWorkspaces.getState().addSession(wsId, cmd);
    }
    maybeAutostart(wsId);
  };

  // one-click relaunch of a saved mix — same fan-out the launcher does, into a fresh space
  const launchPreset = (p: LaunchPreset) => {
    const ws = useWorkspaces.getState();
    const fname = p.folder.split(/[\\/]/).filter(Boolean).pop() || "workspace";
    const id = ws.addWorkspace(fname, p.folder);
    const cmds: (string | undefined)[] = [];
    for (let i = 0; i < p.agents.claude; i++) cmds.push(claudeCmd(p.claudeMode));
    for (let i = 0; i < p.agents.codex; i++) cmds.push(codexCmd());
    for (let i = 0; i < p.agents.gemini; i++) cmds.push(geminiCmd());
    for (let i = 0; i < (p.agents.opencode ?? 0); i++) cmds.push(opencodeCmd());
    for (let i = 0; i < p.agents.terminal; i++) cmds.push(undefined);
    while (cmds.length < p.count) cmds.push(undefined);
    cmds.forEach((c) => ws.addSession(id, c, p.folder));
    useUi.getState().goSpace();
  };

  const tools = TOOLS.filter((t) => !t.winOnly || isWindows);
  const folderName = cwd.split(/[\\/]/).filter(Boolean).pop();

  return (
    <div className="launchpad">
      <div className="lp-hero">
        <div className="lp-mark">
          <Logo size={30} />
        </div>
        <div className="lp-name">{name}</div>
        <div className="lp-meta">
          {isOpen ? "Open space — launch in any folder" : folderName ? `Project · ${folderName}` : "Project"}
        </div>
      </div>

      <div className="lp-dock">
        {tools.map((t) => (
          <button
            key={t.key}
            className={`lp-app${t.primary ? " primary" : ""}`}
            title={isOpen ? `${t.desc} — you'll pick a folder` : t.desc}
            onClick={() => void launch(t.cmd())}
          >
            <span className="lp-app-ico">
              {t.iconSrc ? (
                <img className="lp-app-img" src={t.iconSrc} width={24} height={24} alt="" />
              ) : t.Icon ? (
                <t.Icon size={24} />
              ) : null}
            </span>
            <span className="lp-app-name">{t.name}</span>
          </button>
        ))}
      </div>

      <div className="lp-more">
        <button className="lp-more-btn" onClick={() => useUi.getState().openLaunch()}>
          <Layers size={14} /> Launch several agents
        </button>
        {presets.length > 0 && <span className="lp-more-sep" />}
        {presets.map((p) => (
          <button
            key={p.id}
            className="lp-preset"
            title={`${p.folder} · ${p.count} terminals`}
            onClick={() => launchPreset(p)}
          >
            <Bookmark size={12} /> {p.name}
            <span className="lp-preset-n">{p.count}</span>
          </button>
        ))}
      </div>

      {!isOpen && cwd && <Services wsId={wsId} folder={cwd} />}
    </div>
  );
}

// configured startup services for the open project, as quick Run chips. background services show a
// live dot + open their logs on click instead of spawning a pane.
function Services({ wsId, folder }: { wsId: string; folder: string }) {
  const cfg = useProjectConfigs((s) => s.configs[folderKey(folder)]);
  const tasks = cfg?.startup ?? [];
  const running = useServices((s) => s.running);
  const known = useServices((s) => s.known);
  if (tasks.length === 0) return null;
  const openLogs = (t: { id: string; name: string }) =>
    useUi.getState().openServiceLogs({ id: serviceId(t.id), name: t.name || "service" });
  return (
    <div className="empty-services">
      <span className="empty-services-label">Services</span>
      <div className="empty-services-row">
        {tasks.map((t) => {
          if (!t.background) {
            return (
              <button key={t.id} className="empty-svc-chip" onClick={() => launchTask(wsId, t)}>
                <Play size={11} />
                {t.name || "service"}
              </button>
            );
          }
          const sid = serviceId(t.id);
          const on = !!running[sid];
          const hasLogs = on || !!known[sid];
          return (
            <div className={`empty-svc-chip bg${on ? " on" : ""}`} key={t.id}>
              <button
                className="empty-svc-main"
                title={on ? "Running in background — view logs" : "Run in background"}
                onClick={() => (on ? openLogs(t) : launchTask(wsId, t))}
              >
                {on ? <span className="svc-dot on" /> : <Play size={11} />}
                {t.name || "service"}
              </button>
              {hasLogs && (
                <button className="empty-svc-logs" title="View logs" onClick={() => openLogs(t)}>
                  <ScrollText size={11} />
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
