import { useEffect, useState, type ReactNode } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useSettings, type ClaudePermission, type CodexMode } from "../stores/settings";
import { useNotifications } from "../stores/notifications";
import { pickFolder, createProjectDir, gitInit, gitIsRepo } from "../api";
import { projectsBaseDir, DEFAULT_GITIGNORE } from "../lib/projects";
import { claudeCmd, geminiCmd, codexCmd, opencodeCmd, grokCmd, WSL_CMD } from "../actions";
import { isWindows } from "../platform";
import {
  Sparkles,
  Gem,
  Bot,
  SquareCode,
  SquareTerminal,
  Atom,
  Terminal as TerminalIcon,
  Folder,
  Minus,
  Plus,
  GitBranch,
  FileText,
} from "lucide-react";

const SEP = isWindows ? "\\" : "/";
const MAX = 6;

type ProvKey = "claude" | "gemini" | "codex" | "opencode" | "grok" | "wsl" | "terminal";
const PROVIDERS: { key: ProvKey; name: string; icon: ReactNode }[] = [
  { key: "claude", name: "Claude", icon: <Sparkles size={15} /> },
  { key: "gemini", name: "Gemini", icon: <Gem size={15} /> },
  { key: "codex", name: "Codex", icon: <Bot size={15} /> },
  { key: "opencode", name: "OpenCode", icon: <SquareCode size={15} /> },
  { key: "grok", name: "Grok", icon: <Atom size={15} /> },
  ...(isWindows ? [{ key: "wsl" as ProvKey, name: "WSL", icon: <SquareTerminal size={15} /> }] : []),
  { key: "terminal", name: "Terminal", icon: <TerminalIcon size={15} /> },
];

const CLAUDE_MODES: { value: ClaudePermission; label: string }[] = [
  { value: "default", label: "Ask each time" },
  { value: "acceptEdits", label: "Accept edits" },
  { value: "plan", label: "Plan mode" },
  { value: "bypass", label: "Bypass permissions" },
];
const CODEX_MODES: { value: CodexMode; label: string }[] = [
  { value: "default", label: "Suggest (ask)" },
  { value: "auto", label: "Auto (sandboxed)" },
  { value: "bypass", label: "Full access (bypass)" },
];

export function NewProjectDialog() {
  const open = useUi((s) => s.newProjectOpen);
  const close = useUi((s) => s.closeNewProject);

  const [name, setName] = useState("");
  const [mode, setMode] = useState<"existing" | "new">("existing");
  const [baseDir, setBaseDir] = useState(""); // existing → the folder; new → the parent
  const [folderName, setFolderName] = useState("");
  const [counts, setCounts] = useState<Record<ProvKey, number>>({
    claude: 1,
    gemini: 0,
    codex: 0,
    opencode: 0,
    grok: 0,
    wsl: 0,
    terminal: 0,
  });
  const [claudeMode, setClaudeMode] = useState<ClaudePermission>("acceptEdits");
  const [codexMode, setCodexMode] = useState<CodexMode>("auto");
  const [initGit, setInitGit] = useState(true);
  const [addReadme, setAddReadme] = useState(false);
  const [addGitignore, setAddGitignore] = useState(false);
  const [alreadyRepo, setAlreadyRepo] = useState(false);
  const [busy, setBusy] = useState(false);

  // fresh state every time it opens
  useEffect(() => {
    if (!open) return;
    setName("");
    setMode("existing");
    setBaseDir("");
    setFolderName("");
    setCounts({ claude: 1, gemini: 0, codex: 0, opencode: 0, grok: 0, wsl: 0, terminal: 0 });
    setClaudeMode(useSettings.getState().claudePermission);
    setCodexMode(useSettings.getState().codexMode);
    setInitGit(true);
    setAddReadme(false);
    setAddGitignore(false);
    setAlreadyRepo(false);
    setBusy(false);
  }, [open]);

  if (!open) return null;

  const folder =
    mode === "new"
      ? baseDir && folderName.trim()
        ? `${baseDir}${SEP}${folderName.trim()}`
        : ""
      : baseDir;
  const total = PROVIDERS.reduce((n, p) => n + counts[p.key], 0);
  const canCreate = !!name.trim() && !!folder && !busy;

  const switchMode = (m: "existing" | "new") => {
    if (m === mode) return;
    setMode(m);
    setBaseDir("");
    setAlreadyRepo(false);
    setInitGit(true);
    // a fresh folder gets starter files by default; an existing one is left untouched
    setAddReadme(m === "new");
    setAddGitignore(m === "new");
    // prefill the parent with the configured projects folder so a new folder lands there by default
    if (m === "new") void projectsBaseDir().then((b) => b && setBaseDir((cur) => cur || b));
  };

  const browse = async () => {
    const f = await pickFolder();
    if (!f) return;
    const base = f.split(/[\\/]/).filter(Boolean).pop() || "";
    setBaseDir(f);
    if (mode === "existing") {
      if (!name.trim()) setName(base);
      // don't offer a redundant "init git" when the folder is already a repo
      gitIsRepo(f)
        .then((r) => {
          setAlreadyRepo(r);
          if (r) setInitGit(false);
        })
        .catch(() => {});
    }
  };

  const bump = (k: ProvKey, d: number) =>
    setCounts((c) => ({ ...c, [k]: Math.max(0, Math.min(MAX, c[k] + d)) }));

  const create = async () => {
    if (!canCreate) return;
    setBusy(true);
    try {
      await createProjectDir(
        folder,
        addReadme ? `# ${name.trim()}\n` : null,
        addGitignore ? DEFAULT_GITIGNORE : null,
      );
      const id = useWorkspaces.getState().addWorkspace(name.trim(), folder);
      if (initGit && !alreadyRepo) await gitInit(folder).catch(() => {});
      // launch the requested agents/terminals into the new project
      const launches: (string | undefined)[] = [];
      for (let i = 0; i < counts.claude; i++) launches.push(claudeCmd(claudeMode));
      for (let i = 0; i < counts.gemini; i++) launches.push(geminiCmd());
      for (let i = 0; i < counts.codex; i++) launches.push(codexCmd(codexMode));
      for (let i = 0; i < counts.opencode; i++) launches.push(opencodeCmd());
      for (let i = 0; i < counts.grok; i++) launches.push(grokCmd());
      for (let i = 0; i < counts.wsl; i++) launches.push(WSL_CMD);
      for (let i = 0; i < counts.terminal; i++) launches.push(undefined);
      launches.forEach((cmd) => useWorkspaces.getState().addSession(id, cmd, folder));
      useUi.getState().goSpace();
      close();
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't create project", body: String(e) });
      setBusy(false);
    }
  };

  return (
    <div className="np-overlay" onMouseDown={() => !busy && close()}>
      <div
        className="np"
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape" && !busy) close();
          if ((e.metaKey || e.ctrlKey) && e.key === "Enter") void create();
        }}
      >
        <div className="np-head">
          <span className="np-title">New project</span>
          <span className="np-sub">Set up a folder with agents and git, ready to go.</span>
        </div>

        <div className="np-body">
          <label className="np-field">
            <span className="np-label">Project name</span>
            <input
              className="np-input"
              autoFocus
              placeholder="my-project"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </label>

          <div className="np-field">
            <span className="np-label">Location</span>
            <div className="np-seg">
              <button
                className={mode === "existing" ? "active" : ""}
                onClick={() => switchMode("existing")}
              >
                Existing folder
              </button>
              <button className={mode === "new" ? "active" : ""} onClick={() => switchMode("new")}>
                New folder
              </button>
            </div>
            <div className="np-browse">
              <button className="btn" onClick={() => void browse()}>
                <Folder size={14} />
                {mode === "new" ? "Choose location…" : "Choose folder…"}
              </button>
              {mode === "new" && (
                <input
                  className="np-input np-foldername"
                  placeholder="folder-name"
                  value={folderName}
                  onChange={(e) => {
                    const v = e.target.value;
                    setName((n) => (!n.trim() || n === folderName ? v : n));
                    setFolderName(v);
                  }}
                />
              )}
            </div>
            <div className={`np-path${folder ? "" : " np-path-empty"}`} title={folder || undefined}>
              {folder || "No folder chosen yet"}
            </div>
          </div>

          <div className="np-field">
            <span className="np-label">
              Agents &amp; terminals
              {total > 0 && <span className="np-badge">{total}</span>}
            </span>
            <div className="np-agents">
              {PROVIDERS.map((p) => (
                <div className="np-agent" key={p.key}>
                  <span className="np-agent-ico">{p.icon}</span>
                  <span className="np-agent-name">{p.name}</span>
                  <div className="np-step">
                    <button
                      onClick={() => bump(p.key, -1)}
                      disabled={counts[p.key] === 0}
                      aria-label={`fewer ${p.name}`}
                    >
                      <Minus size={13} />
                    </button>
                    <span>{counts[p.key]}</span>
                    <button
                      onClick={() => bump(p.key, 1)}
                      disabled={counts[p.key] >= MAX}
                      aria-label={`more ${p.name}`}
                    >
                      <Plus size={13} />
                    </button>
                  </div>
                </div>
              ))}
            </div>
            {counts.claude > 0 && (
              <label className="np-claude-mode">
                <span>Claude permission mode</span>
                <select
                  value={claudeMode}
                  onChange={(e) => setClaudeMode(e.target.value as ClaudePermission)}
                >
                  {CLAUDE_MODES.map((m) => (
                    <option key={m.value} value={m.value}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </label>
            )}
            {counts.codex > 0 && (
              <label className="np-claude-mode">
                <span>Codex approval mode</span>
                <select
                  value={codexMode}
                  onChange={(e) => setCodexMode(e.target.value as CodexMode)}
                >
                  {CODEX_MODES.map((m) => (
                    <option key={m.value} value={m.value}>
                      {m.label}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>

          <div className="np-field">
            <span className="np-label">Setup</span>
            <div className="np-opts">
              <label className={`np-opt${alreadyRepo ? " np-opt-disabled" : ""}`}>
                <input
                  type="checkbox"
                  checked={alreadyRepo ? true : initGit}
                  disabled={alreadyRepo}
                  onChange={(e) => setInitGit(e.target.checked)}
                />
                <GitBranch size={14} />
                <span>{alreadyRepo ? "Already a git repository" : "Initialize a git repository"}</span>
              </label>
              <label className="np-opt">
                <input
                  type="checkbox"
                  checked={addReadme}
                  onChange={(e) => setAddReadme(e.target.checked)}
                />
                <FileText size={14} />
                <span>Add a README.md</span>
              </label>
              <label className="np-opt">
                <input
                  type="checkbox"
                  checked={addGitignore}
                  onChange={(e) => setAddGitignore(e.target.checked)}
                />
                <FileText size={14} />
                <span>Add a .gitignore</span>
              </label>
            </div>
          </div>
        </div>

        <div className="np-foot">
          <span className="np-foot-hint">
            {total > 0
              ? `Creates the project and launches ${total} ${total === 1 ? "pane" : "panes"}.`
              : "Creates an empty project."}
          </span>
          <div className="np-actions">
            <button className="btn" onClick={close} disabled={busy}>
              Cancel
            </button>
            <button className="btn primary" onClick={() => void create()} disabled={!canCreate}>
              {busy ? "Creating…" : "Create project"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
