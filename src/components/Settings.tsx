import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { THEMES } from "../themes";
import {
  useSettings,
  DEFAULT_FONT,
  type CursorStyle,
  type ClaudePermission,
  type CodexMode,
} from "../stores/settings";
import { useUi } from "../stores/ui";
import { useUpdater } from "../stores/updater";
import { PALETTES } from "../terminal/palettes";
import { useAuth } from "../stores/auth";
import { providerStatus, pickFolder, getHomeDir, type ProviderStatus } from "../api";
import { relTime } from "../lib/time";
import { McpServers } from "./McpServers";
import { SkillsManager } from "./SkillsManager";
import { UsagePanel } from "./UsagePanel";
import { Blurred } from "./Blurred";
import { StartupSettings } from "./StartupSettings";
import { useWorkspaces } from "../stores/workspace";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  User,
  Palette,
  Boxes,
  Gauge,
  SquareTerminal,
  ArrowDownToLine,
  Info,
  X,
  Sparkles,
  Gem,
  Bot,
  SquareCode,
  RefreshCw,
  Plug,
  Check,
  Copy,
  Folder,
  FolderCog,
  LayoutGrid,
  Zap,
  Atom,
  Rocket,
  ArrowLeft,
} from "lucide-react";

const FONTS: { label: string; value: string }[] = [
  { label: "JetBrainsMono Nerd Font (bundled)", value: DEFAULT_FONT },
  { label: "Cascadia Code", value: '"Cascadia Code", "Consolas", monospace' },
  { label: "JetBrains Mono", value: '"JetBrains Mono", "Cascadia Code", monospace' },
  { label: "Consolas", value: '"Consolas", monospace' },
  { label: "Courier New", value: '"Courier New", monospace' },
];

const CURSORS: { label: string; value: CursorStyle }[] = [
  { label: "Bar", value: "bar" },
  { label: "Block", value: "block" },
  { label: "Underline", value: "underline" },
];

type Tab =
  | "account"
  | "appearance"
  | "workspace"
  | "startup"
  | "providers"
  | "usage"
  | "mcp"
  | "skills"
  | "terminal"
  | "updates"
  | "about";

const ICONS: Record<Tab, ReactNode> = {
  account: <User strokeWidth={1.75} />,
  appearance: <Palette strokeWidth={1.75} />,
  workspace: <FolderCog strokeWidth={1.75} />,
  startup: <Rocket strokeWidth={1.75} />,
  providers: <Boxes strokeWidth={1.75} />,
  usage: <Gauge strokeWidth={1.75} />,
  mcp: <Plug strokeWidth={1.75} />,
  skills: <Zap strokeWidth={1.75} />,
  terminal: <SquareTerminal strokeWidth={1.75} />,
  updates: <ArrowDownToLine strokeWidth={1.75} />,
  about: <Info strokeWidth={1.75} />,
};

const TABS: { id: Tab; label: string; desc: string }[] = [
  { id: "account", label: "Account", desc: "Your profile and sign-in" },
  { id: "appearance", label: "Appearance", desc: "Theme, colors and fonts" },
  { id: "workspace", label: "Workspace", desc: "Where projects are created" },
  { id: "startup", label: "Actions", desc: "Project commands you run on demand or on open" },
  { id: "providers", label: "Providers", desc: "How each AI tool launches" },
  { id: "usage", label: "Usage", desc: "Tokens, sessions and limits per provider" },
  { id: "mcp", label: "MCP", desc: "Model Context Protocol servers" },
  { id: "skills", label: "Skills", desc: "Snippets and Claude skills" },
  { id: "terminal", label: "Terminal", desc: "Cursor style and behavior" },
  { id: "updates", label: "Updates", desc: "Version and update checks" },
  { id: "about", label: "About", desc: "About HyprSpace" },
];

// one settings row: label (+ optional description) on the left, control on the right
function Row({ label, desc, children }: { label?: string; desc?: string; children: ReactNode }) {
  return (
    <div className="set-row">
      <div className="set-row-info">
        {label && <div className="set-key">{label}</div>}
        {desc && <div className="set-desc">{desc}</div>}
      </div>
      <div className="set-control">{children}</div>
    </div>
  );
}

// a labeled card that groups related rows, split by dividers — the Cursor look
function Group({ label, pad, children }: { label: string; pad?: boolean; children: ReactNode }) {
  return (
    <div className="set-section">
      <div className="set-label">{label}</div>
      <div className={`set-group${pad ? " pad" : ""}`}>{children}</div>
    </div>
  );
}

// the signed-in line under a provider's name — account + plan, or why it's unavailable
function statusLine(s: ProviderStatus | "loading"): { cls: string; node: ReactNode } {
  if (s === "loading") return { cls: "muted", node: "Checking…" };
  if (!s.installed) {
    const name = s.id ? s.id.charAt(0).toUpperCase() + s.id.slice(1) : "This";
    return {
      cls: "err",
      node: (
        <>
          {name} CLI (<code className="provider-cli">{s.id || "?"}</code>) is not installed or not on
          PATH.
        </>
      ),
    };
  }
  if (s.account)
    return {
      cls: "ok",
      node: (
        <>
          Authenticated as{" "}
          <span className="provider-acct">
            <Blurred text={s.account} />
          </span>
          {s.plan ? ` · ${s.plan}` : ""}
        </>
      ),
    };
  return { cls: "warn", node: s.detail || "Signed in" };
}

// ---- account-tab helpers ----
function fmtDate(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return isNaN(d.getTime())
    ? "—"
    : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}
function fmtDateTime(iso?: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return isNaN(d.getTime())
    ? "—"
    : d.toLocaleString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        hour: "numeric",
        minute: "2-digit",
      });
}
// copy-to-clipboard button with a brief check tick
function CopyBtn({ value }: { value: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="acct-copy"
      title="Copy"
      onClick={() => {
        void writeText(value);
        setDone(true);
        setTimeout(() => setDone(false), 1200);
      }}
    >
      {done ? <Check size={13} /> : <Copy size={13} />}
    </button>
  );
}

// a provider card — icon + name + version, the signed-in line, then its launch options
function Provider({
  icon,
  name,
  status,
  children,
}: {
  icon: ReactNode;
  name: string;
  status?: ProviderStatus | "loading";
  children?: ReactNode;
}) {
  const line = status ? statusLine(status) : null;
  return (
    <div className="set-section">
      <div className="provider-head">
        <span className="provider-ico">{icon}</span>
        <span className="provider-name">{name}</span>
        {status && status !== "loading" && status.version && (
          <span className="provider-ver">v{status.version}</span>
        )}
      </div>
      {line && (
        <div className={`provider-status ${line.cls}`}>
          <span className="provider-status-dot" />
          <span className="provider-status-text">{line.node}</span>
        </div>
      )}
      {children && <div className="set-group">{children}</div>}
    </div>
  );
}

// In-app settings screen — rendered inside the main window (no separate OS window).
export function Settings() {
  const close = useUi((s) => s.closeSettings);
  const tab = useUi((s) => s.settingsTab) as Tab;
  const setTab = useUi((s) => s.setSettingsTab);

  const theme = useSettings((s) => s.theme);
  const setTheme = useSettings((s) => s.setTheme);
  const fontSize = useSettings((s) => s.fontSize);
  const setFontSize = useSettings((s) => s.setFontSize);
  const fontFamily = useSettings((s) => s.fontFamily);
  const setFontFamily = useSettings((s) => s.setFontFamily);
  const cursorStyle = useSettings((s) => s.cursorStyle);
  const setCursorStyle = useSettings((s) => s.setCursorStyle);
  const cursorBlink = useSettings((s) => s.cursorBlink);
  const setCursorBlink = useSettings((s) => s.setCursorBlink);
  const copyOnSelect = useSettings((s) => s.copyOnSelect);
  const setCopyOnSelect = useSettings((s) => s.setCopyOnSelect);
  const lineHeight = useSettings((s) => s.lineHeight);
  const setLineHeight = useSettings((s) => s.setLineHeight);
  const terminalTheme = useSettings((s) => s.terminalTheme);
  const setTerminalTheme = useSettings((s) => s.setTerminalTheme);
  const gpuRender = useSettings((s) => s.gpuRender);
  const setGpuRender = useSettings((s) => s.setGpuRender);
  const autoNameAgents = useSettings((s) => s.autoNameAgents);
  const setAutoNameAgents = useSettings((s) => s.setAutoNameAgents);
  const dismissedConfirms = useSettings((s) => s.dismissedConfirms);
  const claudePermission = useSettings((s) => s.claudePermission);
  const setClaudePermission = useSettings((s) => s.setClaudePermission);
  const geminiYolo = useSettings((s) => s.geminiYolo);
  const setGeminiYolo = useSettings((s) => s.setGeminiYolo);
  const codexMode = useSettings((s) => s.codexMode);
  const setCodexMode = useSettings((s) => s.setCodexMode);
  const projectsDir = useSettings((s) => s.projectsDir);
  const setProjectsDir = useSettings((s) => s.setProjectsDir);

  // resolve what the projects folder actually is right now, for the Workspace tab
  const [defaultProjectsDir, setDefaultProjectsDir] = useState("~/Documents/HyprSpace");
  useEffect(() => {
    getHomeDir()
      .then((h) => {
        if (!h) return;
        const sep = h.includes("\\") ? "\\" : "/";
        setDefaultProjectsDir([h, "Documents", "HyprSpace"].join(sep));
      })
      .catch(() => {});
  }, []);
  const chooseProjectsDir = async () => {
    const f = await pickFolder();
    if (f) setProjectsDir(f);
  };

  // live provider status (CLI version + signed-in account/plan) for the Providers tab
  const [statuses, setStatuses] = useState<Record<string, ProviderStatus | "loading">>({});
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const refreshStatuses = () => {
    const ids = ["claude", "gemini", "codex", "opencode", "grok"];
    setStatuses(Object.fromEntries(ids.map((id) => [id, "loading" as const])));
    setCheckedAt(Date.now());
    ids.forEach((id) =>
      providerStatus(id)
        .then((st) => setStatuses((p) => ({ ...p, [id]: st })))
        .catch(() =>
          setStatuses((p) => ({
            ...p,
            [id]: {
              id,
              installed: false,
              version: null,
              account: null,
              plan: null,
              detail: "Couldn't check",
            },
          })),
        ),
    );
  };
  useEffect(() => {
    if (tab === "providers" && checkedAt == null) refreshStatuses();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const phase = useUpdater((s) => s.phase);
  const detail = useUpdater((s) => s.detail);
  const update = useUpdater((s) => s.update);
  const checkNow = useUpdater((s) => s.checkNow);
  const install = useUpdater((s) => s.install);

  const authUser = useAuth((s) => s.user);
  const signingIn = useAuth((s) => s.signingIn);
  const signInGoogle = useAuth((s) => s.signInWithGoogle);
  const signOut = useAuth((s) => s.signOut);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);

  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [close]);

  const statusText =
    phase === "checking"
      ? "Checking…"
      : phase === "available"
        ? `v${update?.version} available`
        : phase === "downloading"
          ? detail
          : phase === "uptodate"
            ? "You're on the latest version"
            : phase === "error"
              ? detail
              : "—";

  const active = TABS.find((t) => t.id === tab) ?? TABS[0];
  const avatar =
    typeof authUser?.user_metadata?.avatar_url === "string" ? authUser.user_metadata.avatar_url : null;
  const fullName = ((authUser?.user_metadata?.full_name as string) || authUser?.email || "").trim();
  const initial = (fullName || "?")[0]?.toUpperCase() ?? "?";
  const projectCount = workspaces.filter((w) => w.kind !== "open").length;
  const openCount = workspaces.filter((w) => w.kind === "open").length;
  const sessionCount = workspaces.reduce((n, w) => n + w.sessions.length, 0);
  const acctProvider = authUser?.app_metadata?.provider;
  const providerLabel = acctProvider
    ? acctProvider.charAt(0).toUpperCase() + acctProvider.slice(1)
    : "—";
  const skillsCwd = (() => {
    const w = workspaces.find((x) => x.id === activeId);
    return w?.sessions.find((s) => s.id === focusedSessionId)?.cwd || w?.cwd || "";
  })();

  return (
    <div className="settings-screen">
      <nav className="settings-nav">
        <div className="settings-brand">Settings</div>
        {TABS.map((t) => (
          <button
            key={t.id}
            className={`settings-nav-item${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {ICONS[t.id]}
            {t.label}
          </button>
        ))}
        <div className="settings-nav-bottom">
          {authUser && (
            <button
              className={`settings-acct${tab === "account" ? " active" : ""}`}
              onClick={() => setTab("account")}
              title="Account"
            >
              {avatar ? (
                <img src={avatar} alt="" referrerPolicy="no-referrer" />
              ) : (
                <span className="settings-acct-ava">{initial}</span>
              )}
              <div className="settings-acct-meta">
                {/* never the raw email here — it'd leak on streams */}
                <div className="settings-acct-name">
                  {((authUser.user_metadata?.full_name as string) || "").trim() ||
                    authUser.email?.split("@")[0] ||
                    "Account"}
                </div>
              </div>
            </button>
          )}
          <button className="settings-nav-item settings-back" onClick={close} title="Back (Esc)">
            <ArrowLeft strokeWidth={1.75} />
            Back
          </button>
        </div>
      </nav>

      <div className="settings-main">
        <div className="settings-header">
          <div>
            <div className="settings-header-title">{active.label}</div>
            <div className="settings-header-desc">{active.desc}</div>
          </div>
          <button className="settings-close" onClick={close} aria-label="Close settings" title="Close (Esc)">
            <X size={18} />
          </button>
        </div>

        <div className="settings-content">
          <div className="settings-page">
            {tab === "account" &&
              (authUser ? (
                <>
                  <Group label="Profile">
                    <div className="acct-row">
                      {avatar ? (
                        <img className="acct-avatar" src={avatar} alt="" referrerPolicy="no-referrer" />
                      ) : (
                        <div className="acct-avatar acct-avatar-fallback">{initial}</div>
                      )}
                      <div className="acct-meta">
                        <div className="acct-name">{fullName || authUser.email}</div>
                        <div className="acct-email">{authUser.email}</div>
                      </div>
                      <button className="btn" onClick={() => void signOut()}>
                        Sign out
                      </button>
                    </div>
                  </Group>

                  <Group label="Account">
                    <Row label="Signed in with">
                      <span className="acct-val">{providerLabel}</span>
                    </Row>
                    <Row label="Email">
                      <span className="acct-val acct-val-copy">
                        {authUser.email}
                        {authUser.email_confirmed_at && (
                          <Check size={13} className="acct-verified" />
                        )}
                        <CopyBtn value={authUser.email ?? ""} />
                      </span>
                    </Row>
                    <Row label="Member since">
                      <span className="acct-val">{fmtDate(authUser.created_at)}</span>
                    </Row>
                    <Row label="Last sign-in">
                      <span className="acct-val">{fmtDateTime(authUser.last_sign_in_at)}</span>
                    </Row>
                    <Row label="Account ID">
                      <span className="acct-val acct-val-copy">
                        <code className="acct-id">{authUser.id}</code>
                        <CopyBtn value={authUser.id} />
                      </span>
                    </Row>
                  </Group>

                  <div className="set-section">
                    <div className="set-label">Workspace</div>
                    <div className="acct-stats">
                      <div className="acct-stat">
                        <span className="acct-stat-ico">
                          <Folder size={16} />
                        </span>
                        <span className="acct-stat-body">
                          <span className="acct-stat-n">{projectCount}</span>
                          <span className="acct-stat-l">Projects</span>
                        </span>
                      </div>
                      <div className="acct-stat">
                        <span className="acct-stat-ico">
                          <LayoutGrid size={16} />
                        </span>
                        <span className="acct-stat-body">
                          <span className="acct-stat-n">{openCount}</span>
                          <span className="acct-stat-l">Open spaces</span>
                        </span>
                      </div>
                      <div className="acct-stat">
                        <span className="acct-stat-ico">
                          <SquareTerminal size={16} />
                        </span>
                        <span className="acct-stat-body">
                          <span className="acct-stat-n">{sessionCount}</span>
                          <span className="acct-stat-l">Sessions</span>
                        </span>
                      </div>
                    </div>
                  </div>
                </>
              ) : (
                <Group label="Account">
                  <Row label="Status" desc="You're not signed in.">
                    <button
                      className="btn primary"
                      disabled={signingIn}
                      onClick={() => void signInGoogle()}
                    >
                      {signingIn ? "Waiting…" : "Sign in with Google"}
                    </button>
                  </Row>
                </Group>
              ))}

            {tab === "appearance" && (
              <>
                <div className="set-section">
                  <div className="set-label">Theme</div>
                  <div className="theme-grid">
                    {THEMES.map((t) => {
                      const active = t.id === theme;
                      return (
                        <button
                          key={t.id}
                          type="button"
                          className={`theme-card ${active ? "active" : ""}`}
                          onClick={() => setTheme(t.id)}
                          aria-pressed={active}
                          style={
                            {
                              "--sw-accent": t.vars["--accent"],
                              "--sw-on": t.vars["--on-accent"],
                            } as CSSProperties
                          }
                        >
                          {/* tiny mockup of the app so you see the accent in context */}
                          <div className="theme-card-frame">
                            <div className="tcp-bar">
                              <span className="tcp-dot" />
                              <span className="tcp-dot" />
                              <span className="tcp-tab" />
                              <span className="tcp-pill" />
                            </div>
                            <div className="tcp-body">
                              <div className="tcp-rail">
                                <span className="tcp-nav on" />
                                <span className="tcp-nav" />
                                <span className="tcp-nav" />
                                <span className="tcp-nav" />
                              </div>
                              <div className="tcp-main">
                                <span className="tcp-line lg" />
                                <span className="tcp-line" />
                                <span className="tcp-line sm" />
                                <span className="tcp-btn" />
                              </div>
                            </div>
                          </div>
                          <div className="theme-card-meta">
                            <span className="theme-card-dot" />
                            <span className="theme-card-text">
                              <span className="theme-card-name">{t.name}</span>
                            </span>
                            <span className="theme-card-check">
                              <Check size={12} strokeWidth={3} />
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </div>

                <Group label="Font">
                  <Row label="Family" desc="Used across all terminals">
                    <select
                      className="set-select"
                      value={fontFamily}
                      onChange={(e) => setFontFamily(e.target.value)}
                    >
                      {FONTS.map((f) => (
                        <option key={f.label} value={f.value}>
                          {f.label}
                        </option>
                      ))}
                    </select>
                  </Row>
                  <Row label="Size" desc="Terminal text size, in pixels">
                    <div className="stepper">
                      <button onClick={() => setFontSize(fontSize - 1)}>−</button>
                      <span className="stepper-val">{fontSize}</span>
                      <button onClick={() => setFontSize(fontSize + 1)}>+</button>
                    </div>
                  </Row>
                  <Row label="Line height" desc="Row spacing, lower is tighter and higher is airier (applies live)">
                    <div className="stepper">
                      <button onClick={() => setLineHeight((lineHeight ?? 1.1) - 0.05)}>−</button>
                      <span className="stepper-val">{(lineHeight ?? 1.1).toFixed(2)}</span>
                      <button onClick={() => setLineHeight((lineHeight ?? 1.1) + 0.05)}>+</button>
                    </div>
                  </Row>
                </Group>
              </>
            )}

            {tab === "workspace" && (
              <Group label="Projects">
                <Row
                  label="Projects folder"
                  desc="Where new projects are created, including ones agents make for you"
                >
                  <div className="set-path">
                    <code className="set-path-val" title={projectsDir || defaultProjectsDir}>
                      {projectsDir || defaultProjectsDir}
                    </code>
                    <button className="btn" onClick={() => void chooseProjectsDir()}>
                      Change…
                    </button>
                    {projectsDir ? (
                      <button className="btn" onClick={() => setProjectsDir("")}>
                        Reset
                      </button>
                    ) : (
                      <span className="set-path-tag">Default</span>
                    )}
                  </div>
                </Row>
                <Row
                  label="Confirmation dialogs"
                  desc="Bring back any dialogs you've hidden with “don't ask me again”"
                >
                  <button
                    className="btn"
                    onClick={() => useSettings.getState().resetDismissedConfirms()}
                    disabled={dismissedConfirms.length === 0}
                  >
                    {dismissedConfirms.length ? "Restore" : "None hidden"}
                  </button>
                </Row>
              </Group>
            )}

            {tab === "startup" && <StartupSettings />}

            {tab === "providers" && (
              <>
                <div className="providers-bar">
                  <span className="providers-checked">
                    {checkedAt ? `Checked ${relTime(checkedAt)}` : "Checking…"}
                  </span>
                  <button
                    className="providers-refresh"
                    title="Re-check providers"
                    onClick={refreshStatuses}
                  >
                    <RefreshCw size={14} />
                  </button>
                </div>
                <Provider icon={<Sparkles size={16} />} name="Claude" status={statuses.claude}>
                  <Row label="Permission mode" desc="How Claude handles approvals when you launch it">
                    <select
                      className="set-select"
                      value={claudePermission}
                      onChange={(e) => setClaudePermission(e.target.value as ClaudePermission)}
                    >
                      <option value="default">Ask each time</option>
                      <option value="acceptEdits">Accept edits</option>
                      <option value="plan">Plan mode</option>
                      <option value="bypass">Bypass permissions</option>
                    </select>
                  </Row>
                </Provider>

                <Provider icon={<Gem size={16} />} name="Gemini" status={statuses.gemini}>
                  <Row label="Auto-approve (YOLO)" desc="Run actions without asking for confirmation">
                    <button
                      className={`toggle ${geminiYolo ? "on" : ""}`}
                      onClick={() => setGeminiYolo(!geminiYolo)}
                      aria-pressed={geminiYolo}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                </Provider>

                <Provider icon={<Bot size={16} />} name="Codex" status={statuses.codex}>
                  <Row
                    label="Approval mode"
                    desc="How Codex handles approvals and sandboxing when you launch it"
                  >
                    <select
                      className="set-select"
                      value={codexMode}
                      onChange={(e) => setCodexMode(e.target.value as CodexMode)}
                    >
                      <option value="default">Suggest (ask)</option>
                      <option value="auto">Auto (sandboxed)</option>
                      <option value="bypass">Full access (bypass)</option>
                    </select>
                  </Row>
                </Provider>

                <Provider icon={<SquareCode size={16} />} name="OpenCode" status={statuses.opencode}>
                  <Row label="Model & providers" desc="OpenCode is bring-your-own-model. Manage them with `opencode auth`">
                    <span className="set-hint">configured in OpenCode</span>
                  </Row>
                </Provider>

                <Provider icon={<Atom size={16} />} name="Grok" status={statuses.grok}>
                  <Row label="Sign in & model" desc="Grok uses your `grok` login (or XAI_API_KEY) and its own default model">
                    <span className="set-hint">configured in Grok</span>
                  </Row>
                </Provider>

                <div className="set-hint">Terminal and WSL open plain shells, no agent options.</div>
              </>
            )}

            {tab === "usage" && <UsagePanel />}

            {tab === "mcp" && <McpServers />}

            {tab === "skills" && <SkillsManager cwd={skillsCwd} />}

            {tab === "terminal" && (
              <>
                <div className="set-section">
                  <div className="set-label">Color theme</div>
                  <div className="term-theme-grid">
                    {PALETTES.map((p) => {
                      const t = p.theme;
                      const active = p.id === terminalTheme;
                      // adaptive card: bg/fg/caret ride the app theme vars; its ANSI preview uses
                      // the fixed T3 muted set (same colors termTheme() hands xterm on adaptive)
                      const bg = t?.background ?? "var(--bg-terminal)";
                      const fg = t?.foreground ?? "var(--term-fg)";
                      const caret = t?.cursor ?? "var(--term-cursor)";
                      const gray = t?.brightBlack ?? "rgb(110, 120, 136)";
                      const c = {
                        red: t?.red ?? "rgb(255, 122, 142)",
                        green: t?.green ?? "rgb(134, 231, 149)",
                        yellow: t?.yellow ?? "rgb(244, 205, 114)",
                        blue: t?.blue ?? "rgb(137, 190, 255)",
                        magenta: t?.magenta ?? "rgb(208, 176, 255)",
                        cyan: t?.cyan ?? "rgb(124, 232, 237)",
                      };
                      return (
                        <button
                          key={p.id}
                          type="button"
                          className={`term-theme-card ${active ? "active" : ""}`}
                          onClick={() => setTerminalTheme(p.id)}
                          aria-pressed={active}
                          style={{ "--tc-ring": caret } as CSSProperties}
                        >
                          {/* mini terminal: a real prompt drawn in the palette's own colors */}
                          <div className="ttc-shot" style={{ background: bg, color: fg }}>
                            <div className="ttc-line">
                              <span style={{ color: c.green }}>❯ </span>
                              <span style={{ color: c.magenta }}>git </span>
                              commit <span style={{ color: c.cyan }}>-m </span>
                              <span style={{ color: c.yellow }}>"ship it"</span>
                              <span className="ttc-caret" style={{ background: caret }} />
                            </div>
                            <div className="ttc-line" style={{ color: gray }}>
                              <span style={{ color: c.green }}>✓ </span>
                              <span style={{ color: c.blue }}>main </span>
                              a1f39c · 2 files
                            </div>
                          </div>
                          <div className="ttc-meta">
                            <span className="ttc-name">{p.label}</span>
                            <span className="ttc-check">
                              <Check size={12} strokeWidth={3} />
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                  <div className="set-hint">Applies live to every terminal.</div>
                </div>

                <Group label="Cursor">
                  <Row label="Style" desc="Shape of the terminal cursor">
                    <div className="seg">
                      {CURSORS.map((c) => (
                        <button
                          key={c.value}
                          className={`seg-btn ${cursorStyle === c.value ? "active" : ""}`}
                          onClick={() => setCursorStyle(c.value)}
                        >
                          {c.label}
                        </button>
                      ))}
                    </div>
                  </Row>
                  <Row label="Blink" desc="Blink the cursor when a pane is focused">
                    <button
                      className={`toggle ${cursorBlink ? "on" : ""}`}
                      onClick={() => setCursorBlink(!cursorBlink)}
                      aria-pressed={cursorBlink}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                  <Row
                    label="GPU rendering"
                    desc="WebGL draws block art (logos, progress bars, box-drawing) seamlessly, like Alacritty. Off = the DOM renderer with ClearType text, which can leave row gaps in block art. Applies immediately."
                  >
                    <button
                      className={`toggle ${gpuRender ? "on" : ""}`}
                      onClick={() => setGpuRender(!gpuRender)}
                      aria-pressed={gpuRender}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                </Group>

                <Group label="Behavior">
                  <Row label="Copy on select" desc="Copy text to the clipboard as soon as you select it">
                    <button
                      className={`toggle ${copyOnSelect ? "on" : ""}`}
                      onClick={() => setCopyOnSelect(!copyOnSelect)}
                      aria-pressed={copyOnSelect}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                  <Row
                    label="Auto-name agents"
                    desc="Name agent panes after their task, using Codex (your free codex login). Falls back to the folder name."
                  >
                    <button
                      className={`toggle ${autoNameAgents ? "on" : ""}`}
                      onClick={() => setAutoNameAgents(!autoNameAgents)}
                      aria-pressed={autoNameAgents}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                </Group>
              </>
            )}

            {tab === "updates" && (
              <Group label="Updates">
                <Row label="Current version">
                  <span className="set-val">{version || "…"}</span>
                </Row>
                <Row label="Status" desc="Updates install on restart">
                  <span className="set-val">{statusText}</span>
                </Row>
                <Row label="Check for updates" desc="HyprSpace updates itself in the background">
                  {phase === "available" ? (
                    <button className="btn primary" onClick={() => void install()}>
                      Restart &amp; update to {update?.version}
                    </button>
                  ) : (
                    <button
                      className="btn"
                      onClick={() => void checkNow()}
                      disabled={phase === "checking" || phase === "downloading"}
                    >
                      {phase === "checking" ? "Checking…" : "Check now"}
                    </button>
                  )}
                </Row>
              </Group>
            )}

            {tab === "about" && (
              <div className="settings-about">
                <div className="about-name">HyprSpace</div>
                <div className="about-ver">Version {version || "…"}</div>
                <div className="about-blurb">
                  A multi-terminal AI workspace. Tile Claude Code and shells across projects, with
                  isolated agents, a command palette, and live review.
                </div>
                <button
                  className="btn"
                  style={{ marginTop: 14 }}
                  onClick={() => useUi.getState().openOnboarding()}
                >
                  Replay the intro
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
