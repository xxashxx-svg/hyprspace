import { Fragment, useEffect, type CSSProperties, type ReactNode } from "react";
import {
  ArrowLeft,
  Bot,
  Check,
  Gauge,
  Palette,
  SlidersHorizontal,
  Smartphone,
  SquareTerminal,
  X,
  Zap,
} from "lucide-react";
import { useSettings, type CursorStyle } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { PALETTES } from "../terminal/palettes";
import { SkillsManager } from "./SkillsManager";
import { UsagePanel } from "./UsagePanel";
import { MobileSettings } from "./MobileSettings";
import { Row, Group, Toggle } from "./settings/controls";
import { General } from "./settings/General";
import { useVersion } from "../lib/version";
import { useUpdater } from "../stores/updater";
import { Appearance } from "./settings/Appearance";
import { Defaults } from "./settings/Defaults";

const CURSORS: { label: string; value: CursorStyle }[] = [
  { label: "Bar", value: "bar" },
  { label: "Block", value: "block" },
  { label: "Underline", value: "underline" },
];

type Tab = "general" | "appearance" | "terminal" | "agents" | "usage" | "skills" | "mobile";

const TABS: { id: Tab; group: string; label: string; desc: string; icon: ReactNode }[] = [
  { id: "general", group: "App", label: "General", desc: "Updates, behavior and privacy", icon: <SlidersHorizontal strokeWidth={1.75} /> },
  { id: "appearance", group: "App", label: "Appearance", desc: "Theme and fonts", icon: <Palette strokeWidth={1.75} /> },
  { id: "terminal", group: "App", label: "Terminal", desc: "Colors, cursor, rendering", icon: <SquareTerminal strokeWidth={1.75} /> },
  { id: "agents", group: "Agents", label: "Defaults", desc: "What each agent starts with: model, effort and permissions", icon: <Bot strokeWidth={1.75} /> },
  { id: "usage", group: "Agents", label: "Usage", desc: "What each agent has used", icon: <Gauge strokeWidth={1.75} /> },
  { id: "skills", group: "Agents", label: "Skills", desc: "Reusable instructions for Claude", icon: <Zap strokeWidth={1.75} /> },
  { id: "mobile", group: "Devices", label: "Mobile", desc: "Mirror spaces and terminals to your phone", icon: <Smartphone strokeWidth={1.75} /> },
];

/** The in-app settings screen. */
export function Settings() {
  const close = useUi((s) => s.closeSettings);
  const rawTab = useUi((s) => s.settingsTab);
  const setTab = useUi((s) => s.setSettingsTab);
  const tab: Tab = TABS.some((t) => t.id === rawTab) ? (rawTab as Tab) : "general";

  const s = useSettings();

  const workspaces = useWorkspaces((w) => w.workspaces);
  const activeId = useWorkspaces((w) => w.activeId);
  const focusedSessionId = useWorkspaces((w) => w.focusedSessionId);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [close]);

  const active = TABS.find((t) => t.id === tab) ?? TABS[0];
  const version = useVersion();
  const phase = useUpdater((u) => u.phase);
  const ready = phase === "available";
  const skillsCwd = (() => {
    const w = workspaces.find((x) => x.id === activeId);
    return w?.sessions.find((x) => x.id === focusedSessionId)?.cwd || w?.cwd || "";
  })();

  return (
    <div className="settings-screen">
      <nav className="settings-nav">
        <div className="settings-brand">Settings</div>
        {TABS.map((t, i) => (
          <Fragment key={t.id}>
            {(i === 0 || TABS[i - 1].group !== t.group) && <div className="settings-group">{t.group}</div>}
            <button className={`settings-nav-item${tab === t.id ? " active" : ""}`} onClick={() => setTab(t.id)}>
              {t.icon}
              <span className="settings-nav-label">{t.label}</span>
              {t.id === "general" && ready && <i className="settings-nav-dot" title="An update is ready" />}
            </button>
          </Fragment>
        ))}
        <div className="settings-nav-bottom">
          <button className="settings-back" onClick={close}>
            <ArrowLeft size={15} strokeWidth={2} />
            Back to app
            <kbd>Esc</kbd>
          </button>
          <button className={`settings-foot${ready ? " ready" : ""}`} onClick={() => setTab("general")} title="Updates">
            <span className="settings-foot-ver">HyprSpace {version && `v${version}`}</span>
            <span className="settings-foot-state">{ready ? "Update ready" : phase === "uptodate" ? "Up to date" : ""}</span>
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
            {tab === "general" && <General />}

            {tab === "appearance" && <Appearance />}

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

            {tab === "agents" && <Defaults />}

            {tab === "usage" && <UsagePanel />}
            {tab === "skills" && <SkillsManager cwd={skillsCwd} />}
            {tab === "mobile" && <MobileSettings />}

          </div>
        </div>
      </div>
    </div>
  );
}
