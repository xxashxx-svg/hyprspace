import { useEffect, useState, type CSSProperties } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { THEMES } from "../themes";
import { useSettings, DEFAULT_FONT, type CursorStyle } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useUpdater } from "../stores/updater";
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

type Tab = "appearance" | "terminal" | "updates" | "about";
const TABS: { id: Tab; label: string }[] = [
  { id: "appearance", label: "Appearance" },
  { id: "terminal", label: "Terminal" },
  { id: "updates", label: "Updates" },
  { id: "about", label: "About" },
];

export function Settings() {
  const close = useUi((s) => s.toggleSettings);
  const [tab, setTab] = useState<Tab>("appearance");

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
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);
  const statusText =
    phase === "checking"
      ? "checking…"
      : phase === "available"
        ? `v${update?.version} available`
        : phase === "downloading"
          ? detail
          : phase === "uptodate"
            ? "you're on the latest version"
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
              {t.label}
            </button>
          ))}
        </nav>

        <div className="settings-main">
          <div className="settings-header">
            <span>{TABS.find((t) => t.id === tab)?.label}</span>
            <button className="modal-x" onClick={close} aria-label="Close">
              ×
            </button>
          </div>

          <div className="settings-content">
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
                  <div className="set-row">
                    <span className="set-key">Family</span>
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
                  </div>
                  <div className="set-row">
                    <span className="set-key">Size</span>
                    <div className="stepper">
                      <button onClick={() => setFontSize(fontSize - 1)}>−</button>
                      <span className="stepper-val">{fontSize}</span>
                      <button onClick={() => setFontSize(fontSize + 1)}>+</button>
                    </div>
                  </div>
                </div>
              </>
            )}

            {tab === "terminal" && (
              <>
                <div className="set-section">
                  <div className="set-label">Cursor</div>
                  <div className="set-row">
                    <span className="set-key">Style</span>
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
                  </div>
                  <div className="set-row">
                    <span className="set-key">Blink</span>
                    <button
                      className={`toggle ${cursorBlink ? "on" : ""}`}
                      onClick={() => setCursorBlink(!cursorBlink)}
                      aria-pressed={cursorBlink}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </div>
                </div>

                <div className="set-section">
                  <div className="set-label">Behavior</div>
                  <div className="set-row">
                    <span className="set-key">Copy on select</span>
                    <button
                      className={`toggle ${copyOnSelect ? "on" : ""}`}
                      onClick={() => setCopyOnSelect(!copyOnSelect)}
                      aria-pressed={copyOnSelect}
                    >
                      <span className="toggle-knob" />
                    </button>
                  </div>
                </div>
              </>
            )}

            {tab === "updates" && (
              <div className="set-section">
                <div className="set-label">Updates</div>
                <div className="set-row">
                  <span className="set-key">Current version</span>
                  <span className="set-val">{version || "…"}</span>
                </div>
                <div className="set-row">
                  <span className="set-key">Status</span>
                  <span className="set-val">{statusText}</span>
                </div>
                <div className="set-row">
                  <span className="set-key" />
                  {phase === "available" ? (
                    <button className="btn primary set-btn" onClick={() => void install()}>
                      Restart &amp; update to {update?.version}
                    </button>
                  ) : (
                    <button
                      className="btn set-btn"
                      onClick={() => void checkNow()}
                      disabled={phase === "checking" || phase === "downloading"}
                    >
                      {phase === "checking" ? "Checking…" : "Check for updates"}
                    </button>
                  )}
                </div>
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
