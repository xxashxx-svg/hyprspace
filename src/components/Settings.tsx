import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
  ArrowLeft,
  Bot,
  Check,
  Copy,
  Gauge,
  Info,
  Palette,
  RefreshCw,
  SlidersHorizontal,
  Smartphone,
  SquareTerminal,
  Terminal as TerminalIcon,
  X,
  Zap,
} from "lucide-react";
import { THEMES } from "../themes";
import { useSettings, DEFAULT_FONT, type CursorStyle, type ClaudePermission, type CodexMode } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useUpdater } from "../stores/updater";
import { useAuth } from "../stores/auth";
import { useProviders } from "../stores/providers";
import { useWorkspaces } from "../stores/workspace";
import { PALETTES } from "../terminal/palettes";
import { relTime } from "../lib/time";
import { AGENT_IDS, CLAUDE_PERMISSIONS, CODEX_MODES, EFFORT_LABEL, effortsFor, type ProviderId } from "../lib/models";
import { catalogFor } from "../stores/providers";
import { PROVIDER_LOGO, PROVIDER_NAME, PROVIDER_DESC } from "../lib/brand";
import { SkillsManager } from "./SkillsManager";
import { UsagePanel } from "./UsagePanel";
import { MobileSettings } from "./MobileSettings";
import { Blurred } from "./Blurred";

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

type Tab = "general" | "appearance" | "terminal" | "agents" | "usage" | "skills" | "mobile" | "about";

const TABS: { id: Tab; label: string; desc: string; icon: ReactNode }[] = [
  { id: "general", label: "General", desc: "Account and app behavior", icon: <SlidersHorizontal strokeWidth={1.75} /> },
  { id: "appearance", label: "Appearance", desc: "Theme and fonts", icon: <Palette strokeWidth={1.75} /> },
  { id: "terminal", label: "Terminal", desc: "Colors, cursor, rendering", icon: <SquareTerminal strokeWidth={1.75} /> },
  { id: "agents", label: "Agents", desc: "Default model, effort, and permissions per agent", icon: <Bot strokeWidth={1.75} /> },
  { id: "usage", label: "Usage", desc: "What each agent has used", icon: <Gauge strokeWidth={1.75} /> },
  { id: "skills", label: "Skills", desc: "Reusable instructions for Claude", icon: <Zap strokeWidth={1.75} /> },
  { id: "mobile", label: "Mobile", desc: "Mirror spaces and terminals to your phone", icon: <Smartphone strokeWidth={1.75} /> },
  { id: "about", label: "About", desc: "Version and updates", icon: <Info strokeWidth={1.75} /> },
];

// ---- small building blocks ----
function Row({ label, desc, children }: { label: string; desc?: string; children: ReactNode }) {
  return (
    <div className="set-row">
      <div className="set-row-info">
        <div className="set-key">{label}</div>
        {desc && <div className="set-desc">{desc}</div>}
      </div>
      <div className="set-control">{children}</div>
    </div>
  );
}

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="set-section">
      <div className="set-label">{label}</div>
      <div className="set-group">{children}</div>
    </div>
  );
}

function Toggle({ on, onChange }: { on: boolean; onChange: (b: boolean) => void }) {
  return (
    <button className={`toggle${on ? " on" : ""}`} onClick={() => onChange(!on)} aria-pressed={on}>
      <span className="toggle-knob" />
    </button>
  );
}

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

// The default model for one agent: the catalog, plus a box for any other id.
function ModelSelect({ provider }: { provider: ProviderId }) {
  const value = useSettings((s) => s.agentModel[provider] ?? "");
  const set = useSettings((s) => s.setAgentModel);
  const codexModels = useProviders((p) => p.codexModels);
  const cat = catalogFor(provider, codexModels);
  const known = cat.models.some((m) => m.id === value);
  const [custom, setCustom] = useState(!known);
  return (
    <div className="set-model">
      <select
        className="set-select"
        value={custom ? "__custom" : value}
        onChange={(e) => {
          if (e.target.value === "__custom") {
            setCustom(true);
            return;
          }
          setCustom(false);
          set(provider, e.target.value);
        }}
      >
        {cat.models.map((m) => (
          <option key={m.id || "default"} value={m.id}>
            {m.label}
          </option>
        ))}
        {cat.customModel && <option value="__custom">Other…</option>}
      </select>
      {custom && (
        <input
          className="set-input"
          autoFocus
          placeholder={cat.modelHint ?? "model id"}
          defaultValue={known ? "" : value}
          onBlur={(e) => set(provider, e.currentTarget.value.trim())}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
      )}
    </div>
  );
}

function EffortSelect({ provider }: { provider: ProviderId }) {
  const value = useSettings((s) => s.agentEffort[provider] ?? "");
  const model = useSettings((s) => s.agentModel[provider] ?? "");
  const set = useSettings((s) => s.setAgentEffort);
  const codexModels = useProviders((p) => p.codexModels);
  const efforts = effortsFor(provider, model, catalogFor(provider, codexModels));
  if (!efforts.length) return null;
  return (
    <select className="set-select" value={value} onChange={(e) => set(provider, e.target.value)}>
      <option value="">Default</option>
      {efforts.map((lvl) => (
        <option key={lvl} value={lvl}>
          {EFFORT_LABEL[lvl] ?? lvl}
        </option>
      ))}
    </select>
  );
}

function fmtDate(iso?: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  return isNaN(d.getTime()) ? "" : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

/** The in-app settings screen. */
export function Settings() {
  const close = useUi((s) => s.closeSettings);
  const rawTab = useUi((s) => s.settingsTab);
  const setTab = useUi((s) => s.setSettingsTab);
  const tab: Tab = TABS.some((t) => t.id === rawTab) ? (rawTab as Tab) : "general";

  const s = useSettings();
  const providers = useProviders((p) => p.status);
  const codexModels = useProviders((p) => p.codexModels);
  const checking = useProviders((p) => p.checking);
  const checkedAt = useProviders((p) => p.checkedAt);
  const refreshProviders = useProviders((p) => p.refresh);
  useEffect(() => {
    if (tab === "agents" && checkedAt == null) void refreshProviders();
  }, [tab, checkedAt, refreshProviders]);

  const phase = useUpdater((u) => u.phase);
  const detail = useUpdater((u) => u.detail);
  const update = useUpdater((u) => u.update);
  const checkNow = useUpdater((u) => u.checkNow);
  const install = useUpdater((u) => u.install);

  const authUser = useAuth((a) => a.user);
  const signingIn = useAuth((a) => a.signingIn);
  const signOut = useAuth((a) => a.signOut);
  const workspaces = useWorkspaces((w) => w.workspaces);
  const activeId = useWorkspaces((w) => w.activeId);
  const focusedSessionId = useWorkspaces((w) => w.focusedSessionId);

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

  const active = TABS.find((t) => t.id === tab) ?? TABS[0];
  const fullName = ((authUser?.user_metadata?.full_name as string) || authUser?.email || "").trim();
  const avatar = typeof authUser?.user_metadata?.avatar_url === "string" ? authUser.user_metadata.avatar_url : null;
  const initial = (fullName || "?")[0]?.toUpperCase() ?? "?";
  const skillsCwd = (() => {
    const w = workspaces.find((x) => x.id === activeId);
    return w?.sessions.find((x) => x.id === focusedSessionId)?.cwd || w?.cwd || "";
  })();
  const updateText =
    phase === "checking"
      ? "Checking"
      : phase === "available"
        ? `Version ${update?.version} is ready to install`
        : phase === "downloading"
          ? detail
          : phase === "uptodate"
            ? "You are on the latest version"
            : phase === "error"
              ? detail
              : "Not checked yet";

  return (
    <div className="settings-screen">
      <nav className="settings-nav">
        <div className="settings-brand">Settings</div>
        {TABS.map((t) => (
          <button key={t.id} className={`settings-nav-item${tab === t.id ? " active" : ""}`} onClick={() => setTab(t.id)}>
            {t.icon}
            {t.label}
          </button>
        ))}
        <div className="settings-nav-bottom">
          {authUser && (
            <button className="settings-acct" onClick={() => setTab("general")} title="Account">
              {avatar ? <img src={avatar} alt="" referrerPolicy="no-referrer" /> : <span className="settings-acct-ava">{initial}</span>}
              <span className="settings-acct-name">{fullName.split("@")[0] || "Account"}</span>
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
            {tab === "general" && (
              <>
                <Group label="Account">
                  {authUser ? (
                    <>
                      <div className="acct-row">
                        {avatar ? <img className="acct-avatar" src={avatar} alt="" referrerPolicy="no-referrer" /> : <div className="acct-avatar acct-avatar-fallback">{initial}</div>}
                        <div className="acct-meta">
                          <div className="acct-name">{fullName || authUser.email}</div>
                          <div className="acct-email">
                            <Blurred text={authUser.email ?? ""} />
                          </div>
                        </div>
                        <button className="btn" onClick={() => void signOut()}>
                          Sign out
                        </button>
                      </div>
                      <Row label="Member since">
                        <span className="set-val">{fmtDate(authUser.created_at)}</span>
                      </Row>
                      <Row label="Account id">
                        <span className="set-val set-val-copy">
                          <code>{authUser.id}</code>
                          <CopyBtn value={authUser.id} />
                        </span>
                      </Row>
                    </>
                  ) : (
                    <Row label="Not signed in" desc="HyprSpace runs the CLIs already on this machine. An account only carries these settings between devices.">
                      <button className="btn primary" disabled={signingIn} onClick={() => useUi.getState().openSignIn()}>
                        {signingIn ? "Waiting" : "Sign in"}
                      </button>
                    </Row>
                  )}
                </Group>

                <Group label="Behavior">
                  <Row label="Name panes after their task" desc="Codex writes a short title from the first prompt. Off, panes are named after their folder.">
                    <Toggle on={s.autoNameAgents} onChange={s.setAutoNameAgents} />
                  </Row>
                  <Row label="Anonymous launch ping" desc="One ping per launch with a random install id, the version, and the OS. Never prompts, output, paths, or project names.">
                    <Toggle on={s.analytics} onChange={s.setAnalytics} />
                  </Row>
                  <Row label="Hidden confirmations" desc="Bring back the dialogs you dismissed with 'don't ask again'.">
                    <button className="btn" disabled={s.dismissedConfirms.length === 0} onClick={() => s.resetDismissedConfirms()}>
                      {s.dismissedConfirms.length ? "Show them again" : "None hidden"}
                    </button>
                  </Row>
                </Group>
              </>
            )}

            {tab === "appearance" && (
              <>
                <div className="set-section">
                  <div className="set-label">Theme</div>
                  <div className="theme-grid">
                    {THEMES.map((t) => {
                      const on = t.id === s.theme;
                      return (
                        <button
                          key={t.id}
                          type="button"
                          className={`theme-card${on ? " active" : ""}`}
                          onClick={() => s.setTheme(t.id)}
                          aria-pressed={on}
                          style={{ "--sw-accent": t.vars["--accent"], "--sw-on": t.vars["--on-accent"] } as CSSProperties}
                        >
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
                            <span className="theme-card-name">{t.name}</span>
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
                  <Row label="Family" desc="Used in every terminal">
                    <select className="set-select" value={s.fontFamily} onChange={(e) => s.setFontFamily(e.target.value)}>
                      {FONTS.map((f) => (
                        <option key={f.label} value={f.value}>
                          {f.label}
                        </option>
                      ))}
                    </select>
                  </Row>
                  <Row label="Size" desc="In pixels">
                    <div className="stepper">
                      <button onClick={() => s.setFontSize(s.fontSize - 1)}>−</button>
                      <span className="stepper-val">{s.fontSize}</span>
                      <button onClick={() => s.setFontSize(s.fontSize + 1)}>+</button>
                    </div>
                  </Row>
                  <Row label="Line height" desc="Lower is tighter, higher is airier">
                    <div className="stepper">
                      <button onClick={() => s.setLineHeight((s.lineHeight ?? 1.1) - 0.05)}>−</button>
                      <span className="stepper-val">{(s.lineHeight ?? 1.1).toFixed(2)}</span>
                      <button onClick={() => s.setLineHeight((s.lineHeight ?? 1.1) + 0.05)}>+</button>
                    </div>
                  </Row>
                </Group>
              </>
            )}

            {tab === "terminal" && (
              <>
                <div className="set-section">
                  <div className="set-label">Colors</div>
                  <div className="term-theme-grid">
                    {PALETTES.map((p) => {
                      const t = p.theme;
                      const on = p.id === s.terminalTheme;
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
                          className={`term-theme-card${on ? " active" : ""}`}
                          onClick={() => s.setTerminalTheme(p.id)}
                          aria-pressed={on}
                          style={{ "--tc-ring": caret } as CSSProperties}
                        >
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
                </div>

                <Group label="Cursor">
                  <Row label="Shape">
                    <div className="seg">
                      {CURSORS.map((c) => (
                        <button key={c.value} className={`seg-btn${s.cursorStyle === c.value ? " active" : ""}`} onClick={() => s.setCursorStyle(c.value)}>
                          {c.label}
                        </button>
                      ))}
                    </div>
                  </Row>
                  <Row label="Blink">
                    <Toggle on={s.cursorBlink} onChange={s.setCursorBlink} />
                  </Row>
                </Group>

                <Group label="Behavior">
                  <Row label="Copy on select" desc="Selected text goes to the clipboard right away">
                    <Toggle on={s.copyOnSelect} onChange={s.setCopyOnSelect} />
                  </Row>
                  <Row label="GPU rendering" desc="Draws block art (logos, progress bars) without gaps. Turn off for ClearType text instead.">
                    <Toggle on={s.gpuRender} onChange={s.setGpuRender} />
                  </Row>
                </Group>
              </>
            )}

            {tab === "agents" && (
              <>
                <div className="set-bar">
                  <span>{checkedAt ? `Checked ${relTime(checkedAt)} ago` : "Checking"}</span>
                  <button className="btn" disabled={checking} onClick={() => void refreshProviders()}>
                    <RefreshCw size={13} className={checking ? "spin" : ""} />
                    Check again
                  </button>
                </div>
                {AGENT_IDS.map((id) => {
                  const st = providers[id];
                  return (
                    <div className="set-section" key={id}>
                      <div className="agent-head">
                        <img className="agent-mark" src={PROVIDER_LOGO[id]} alt="" />
                        <span className="agent-name">{PROVIDER_NAME[id]}</span>
                        <span className="agent-desc">{PROVIDER_DESC[id]}</span>
                        {st?.version && <span className="agent-ver">v{st.version}</span>}
                      </div>
                      <div className={`agent-status ${!st ? "muted" : st.installed ? (st.account ? "ok" : "warn") : "err"}`}>
                        <span className="agent-status-dot" />
                        {!st
                          ? "Checking"
                          : !st.installed
                            ? `Not installed. The ${id} command is not on PATH.`
                            : st.account
                              ? (
                                  <>
                                    Signed in as <Blurred text={st.account} />
                                    {st.plan ? `, ${st.plan}` : ""}
                                  </>
                                )
                              : st.detail || "Installed"}
                      </div>
                      <div className="set-group">
                        <Row label="Model" desc="Used for new sessions. The composer can change it per session.">
                          <ModelSelect provider={id} />
                        </Row>
                        {effortsFor(id, s.agentModel[id] ?? "", catalogFor(id, codexModels)).length > 0 && (
                          <Row label="Effort" desc="How hard the model thinks before it answers">
                            <EffortSelect provider={id} />
                          </Row>
                        )}
                        {id === "claude" && (
                          <Row label="Permissions" desc="What Claude may do without asking">
                            <select className="set-select" value={s.claudePermission} onChange={(e) => s.setClaudePermission(e.target.value as ClaudePermission)}>
                              {CLAUDE_PERMISSIONS.map((m) => (
                                <option key={m.value} value={m.value}>
                                  {m.label}
                                </option>
                              ))}
                            </select>
                          </Row>
                        )}
                        {id === "gemini" && (
                          <Row label="YOLO mode" desc="Run actions without asking">
                            <Toggle on={s.geminiYolo} onChange={s.setGeminiYolo} />
                          </Row>
                        )}
                        {id === "codex" && (
                          <Row label="Approvals" desc="What Codex may do without asking">
                            <select className="set-select" value={s.codexMode} onChange={(e) => s.setCodexMode(e.target.value as CodexMode)}>
                              {CODEX_MODES.map((m) => (
                                <option key={m.value} value={m.value}>
                                  {m.label}
                                </option>
                              ))}
                            </select>
                          </Row>
                        )}
                      </div>
                    </div>
                  );
                })}
                <div className="set-hint">
                  <TerminalIcon size={12} /> Terminal and WSL open a plain shell and have no options.
                </div>
              </>
            )}

            {tab === "usage" && <UsagePanel />}
            {tab === "skills" && <SkillsManager cwd={skillsCwd} />}
            {tab === "mobile" && <MobileSettings />}

            {tab === "about" && (
              <>
                <Group label="HyprSpace">
                  <Row label="Version">
                    <span className="set-val">{version || "…"}</span>
                  </Row>
                  <Row label="Updates" desc={updateText}>
                    {phase === "available" ? (
                      <button className="btn primary" onClick={() => void install()}>
                        Restart and update
                      </button>
                    ) : (
                      <button className="btn" onClick={() => void checkNow()} disabled={phase === "checking" || phase === "downloading"}>
                        {phase === "checking" ? "Checking" : "Check now"}
                      </button>
                    )}
                  </Row>
                  <Row label="Intro" desc="The first-run walkthrough">
                    <button className="btn" onClick={() => useUi.getState().openOnboarding()}>
                      Show again
                    </button>
                  </Row>
                </Group>
                <p className="set-blurb">
                  A workspace for terminal agents. Runs the Claude, Codex, Gemini, OpenCode, and Grok CLIs you already have, side by side, on your own machine.
                </p>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
