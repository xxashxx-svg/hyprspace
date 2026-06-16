import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { THEMES } from "../themes";
import { useSettings, DEFAULT_FONT, type CursorStyle } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useUpdater } from "../stores/updater";
import { useAuth } from "../stores/auth";
import { Logo } from "./Logo";

const FONTS: { label: string; value: string }[] = [
  { label: "Cascadia Code", value: DEFAULT_FONT },
  { label: "JetBrains Mono", value: '"JetBrains Mono", "Cascadia Code", monospace' },
  { label: "Consolas", value: '"Consolas", monospace' },
  { label: "Courier New", value: '"Courier New", monospace' },
];

const CURSORS: { label: string; value: CursorStyle }[] = [
  { label: "Bar", value: "bar" },
  { label: "Block", value: "block" },
  { label: "Underline", value: "underline" },
];

type Tab = "account" | "appearance" | "terminal" | "updates" | "about";

const ICONS: Record<Tab, ReactNode> = {
  account: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="12" cy="8" r="3.5" />
      <path d="M5 20c0-3.6 3.1-5.5 7-5.5s7 1.9 7 5.5" />
    </svg>
  ),
  appearance: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 3a9 9 0 0 0 0 18z" fill="currentColor" stroke="none" />
    </svg>
  ),
  terminal: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <rect x="3" y="4.5" width="18" height="15" rx="2" />
      <path d="M7 9.5l3 2.5-3 2.5M12.5 15h4.5" />
    </svg>
  ),
  updates: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <path d="M12 3.5v11M7.5 10l4.5 4.5 4.5-4.5M5 20.5h14" />
    </svg>
  ),
  about: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6">
      <circle cx="12" cy="12" r="9" />
      <path d="M12 11v5M12 7.5h.01" />
    </svg>
  ),
};

const TABS: { id: Tab; label: string; desc: string }[] = [
  { id: "account", label: "Account", desc: "Your profile and sign-in" },
  { id: "appearance", label: "Appearance", desc: "Theme, colors and fonts" },
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

export function Settings() {
  const close = useUi((s) => s.toggleSettings);
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

  const phase = useUpdater((s) => s.phase);
  const detail = useUpdater((s) => s.detail);
  const update = useUpdater((s) => s.update);
  const checkNow = useUpdater((s) => s.checkNow);
  const install = useUpdater((s) => s.install);

  const authUser = useAuth((s) => s.user);
  const signingIn = useAuth((s) => s.signingIn);
  const signInGoogle = useAuth((s) => s.signInWithGoogle);
  const signOut = useAuth((s) => s.signOut);

  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);
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

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [close]);

  const active = TABS.find((t) => t.id === tab) ?? TABS[0];
  const avatar =
    typeof authUser?.user_metadata?.avatar_url === "string" ? authUser.user_metadata.avatar_url : null;

  return (
    <div className="settings-overlay" onMouseDown={close}>
      <div className="settings-win" onMouseDown={(e) => e.stopPropagation()}>
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
        </nav>

        <div className="settings-main">
          <div className="settings-header">
            <div>
              <div className="settings-header-title">{active.label}</div>
              <div className="settings-header-desc">{active.desc}</div>
            </div>
            <button className="modal-x" onClick={close} aria-label="Close">
              ×
            </button>
          </div>

          <div className="settings-content">
            {tab === "account" && (
              <div className="set-section">
                <div className="set-label">{authUser ? "Profile" : "Account"}</div>
                {authUser ? (
                  <div className="acct-card">
                    {avatar ? (
                      <img className="acct-avatar" src={avatar} alt="" referrerPolicy="no-referrer" />
                    ) : (
                      <div className="acct-avatar acct-avatar-fallback">
                        {(authUser.email ?? "?").trim()[0]?.toUpperCase()}
                      </div>
                    )}
                    <div className="acct-meta">
                      <div className="acct-name">
                        {(authUser.user_metadata?.full_name as string) ?? authUser.email}
                      </div>
                      <div className="acct-email">{authUser.email}</div>
                    </div>
                    <button className="btn" onClick={() => void signOut()}>
                      Sign out
                    </button>
                  </div>
                ) : (
                  <Row label="Status" desc="You're not signed in.">
                    <button
                      className="btn primary"
                      disabled={signingIn}
                      onClick={() => void signInGoogle()}
                    >
                      {signingIn ? "Waiting…" : "Sign in with Google"}
                    </button>
                  </Row>
                )}
              </div>
            )}

            {tab === "appearance" && (
              <>
                <div className="set-section">
                  <div className="set-label">Theme</div>
                  <div className="theme-grid">
                    {THEMES.map((t) => (
                      <button
                        key={t.id}
                        className={`theme-swatch ${t.id === theme ? "active" : ""}`}
                        onClick={() => setTheme(t.id)}
                        style={
                          {
                            "--sw-bg": t.vars["--bg-base"],
                            "--sw-surface": t.vars["--surface-2"],
                            "--sw-accent": t.vars["--accent"],
                            "--sw-text": t.vars["--text-2"],
                          } as CSSProperties
                        }
                      >
                        <div className="sw-preview">
                          <div className="sw-bar accent" />
                          <div className="sw-bar" />
                          <div className="sw-bar short" />
                        </div>
                        <span className="sw-name">{t.name}</span>
                      </button>
                    ))}
                  </div>
                </div>

                <div className="set-section">
                  <div className="set-label">Font</div>
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
                </div>
              </>
            )}

            {tab === "terminal" && (
              <>
                <div className="set-section">
                  <div className="set-label">Cursor</div>
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
                </div>

                <div className="set-section">
                  <div className="set-label">Behavior</div>
                  <Row label="Copy on select" desc="Copy text to the clipboard as soon as you select it">
                    <button
                      className={`toggle ${copyOnSelect ? "on" : ""}`}
                      onClick={() => setCopyOnSelect(!copyOnSelect)}
                      aria-pressed={copyOnSelect}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </Row>
                </div>
              </>
            )}

            {tab === "updates" && (
              <div className="set-section">
                <div className="set-label">Updates</div>
                <Row label="Current version">
                  <span className="set-val">{version || "…"}</span>
                </Row>
                <Row label="Status" desc="Updates install on restart">
                  <span className="set-val">{statusText}</span>
                </Row>
                <Row>
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
                      {phase === "checking" ? "Checking…" : "Check for updates"}
                    </button>
                  )}
                </Row>
              </div>
            )}

            {tab === "about" && (
              <div className="settings-about">
                <div className="about-logo">
                  <Logo size={42} />
                </div>
                <div className="about-name">HyprSpace</div>
                <div className="about-ver">Version {version || "…"}</div>
                <div className="about-blurb">
                  A multi-terminal AI workspace — tile Claude Code and shells across projects, with
                  isolated agents, a command palette, and live review.
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
