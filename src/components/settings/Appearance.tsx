import type { CSSProperties, ReactNode } from "react";
import { Monitor, Moon, Sun } from "lucide-react";
import { THEMES, themeById, type Mode, type Scheme } from "../../themes";
import { useSettings, DEFAULT_FONT, UI_FONTS, type UiFont, type DiffColors } from "../../stores/settings";
import { PROVIDER_COLOR } from "../../lib/brand";
import { Row, Group, Toggle } from "./controls";

const FONTS: { label: string; value: string }[] = [
  { label: "JetBrainsMono Nerd Font (bundled)", value: DEFAULT_FONT },
  { label: "Cascadia Code", value: '"Cascadia Code", "Consolas", monospace' },
  { label: "JetBrains Mono", value: '"JetBrains Mono", "Cascadia Code", monospace' },
  { label: "Consolas", value: '"Consolas", monospace' },
  { label: "Courier New", value: '"Courier New", monospace' },
];

const SCHEMES: { id: Scheme; label: string; icon: ReactNode }[] = [
  { id: "system", label: "System", icon: <Monitor size={13} /> },
  { id: "light", label: "Light", icon: <Sun size={13} /> },
  { id: "dark", label: "Dark", icon: <Moon size={13} /> },
];

const MODES: Mode[] = ["light", "dark"];

// The shell in miniature, drawn straight from the live tokens so it is exactly what a theme
// and side give you: the sidebar with three agent threads, two tiled panes, one of them focused.
function ShellPreview() {
  const rows: { p: string; w: number; st: string }[] = [
    { p: "claude", w: 78, st: "ok" },
    { p: "codex", w: 62, st: "busy" },
    { p: "gemini", w: 70, st: "idle" },
  ];
  return (
    <div className="ap">
      <div className="ap-bar">
        <span className="ap-dot" />
        <span className="ap-dot" />
        <span className="ap-tab" />
        <span className="ap-pill" />
      </div>
      <div className="ap-body">
        <div className="ap-rail">
          {rows.map((r, i) => (
            <div key={r.p} className={`ap-row${i === 0 ? " on" : ""}`}>
              <span className="ap-mark" style={{ background: PROVIDER_COLOR[r.p] }} />
              <span className="ap-row-text">
                <span className="ap-line" style={{ width: `${r.w}%` }} />
                <span className="ap-line dim" style={{ width: `${r.w - 24}%` }} />
              </span>
              <span className={`ap-st ${r.st}`} />
            </div>
          ))}
        </div>
        <div className="ap-grid">
          <div className="ap-pane focus">
            <div className="ap-pane-head">
              <span className="ap-mark" style={{ background: PROVIDER_COLOR.claude }} />
              <span className="ap-line" style={{ width: 46 }} />
            </div>
            <pre className="ap-term">
              <span className="g">❯</span> claude --resume{"\n"}
              <span className="d">Reading src/themes.ts</span>{"\n"}
              <span className="g">✓</span> 3 files changed{"\n"}
              <span className="g">❯</span> <span className="ap-caret" />
            </pre>
          </div>
          <div className="ap-pane">
            <div className="ap-pane-head">
              <span className="ap-mark" style={{ background: PROVIDER_COLOR.terminal }} />
              <span className="ap-line" style={{ width: 34 }} />
            </div>
            <pre className="ap-term">
              <span className="g">❯</span> npm run dev{"\n"}
              <span className="b">VITE</span> ready in <span className="y">312 ms</span>{"\n"}
              <span className="d">➜ http://localhost:1420</span>
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}

export function Appearance() {
  const s = useSettings();
  const theme = themeById(s.theme);

  return (
    <>
      <div className="set-section">
        <div className="set-label-row">
          <div className="set-label">Preview</div>
          <div className="seg scheme-seg">
            {SCHEMES.map((sc) => (
              <button key={sc.id} className={`seg-btn${s.colorScheme === sc.id ? " active" : ""}`} onClick={() => s.setColorScheme(sc.id)}>
                {sc.icon}
                {sc.label}
              </button>
            ))}
          </div>
        </div>
        <ShellPreview />
      </div>

      <div className="set-section">
        <div className="set-label">Theme</div>
        <div className="theme-grid">
          {THEMES.map((t) => {
            const on = t.id === theme.id;
            return (
              <div key={t.id} className={`theme-card${on ? " active" : ""}`} onClick={() => s.setTheme(t.id)}>
                <span className="theme-orbs">
                  {MODES.map((m) => {
                    const lit = on && s.mode === m;
                    return (
                      <button
                        key={m}
                        type="button"
                        className={`theme-orb${lit ? " on" : ""}`}
                        title={`${t.name}, ${m}`}
                        style={{ "--o-bg": t.vars[m]["--bg-base"], "--o-accent": t.vars[m]["--accent"] } as CSSProperties}
                        onClick={(e) => {
                          e.stopPropagation();
                          s.setTheme(t.id);
                          s.setColorScheme(m);
                        }}
                      >
                        {lit && <span className="theme-orb-badge">{m === "light" ? <Sun size={10} strokeWidth={2.5} /> : <Moon size={10} strokeWidth={2.5} />}</span>}
                      </button>
                    );
                  })}
                </span>
                <span className="theme-card-text">
                  <span className="theme-card-name">{t.name}</span>
                  <span className="theme-card-blurb">{t.blurb}</span>
                </span>
              </div>
            );
          })}
        </div>
      </div>

      <Group label="Interface">
        <Row label="Font" desc="Everything outside the terminal">
          <select className="set-select" value={s.uiFont} onChange={(e) => s.setUiFont(e.target.value as UiFont)}>
            {(Object.keys(UI_FONTS) as UiFont[]).map((id) => (
              <option key={id} value={id}>
                {UI_FONTS[id].label}
              </option>
            ))}
          </select>
        </Row>
        <Row label="Diff colors" desc="Additions and deletions in diffs and change counts">
          <span className="diff-pick">
            <span className="diff-dot add" />
            <span className="diff-dot del" />
            <select className="set-select" value={s.diffColors} onChange={(e) => s.setDiffColors(e.target.value as DiffColors)}>
              <option value="redgreen">Red and green</option>
              <option value="blueorange">Blue and orange</option>
            </select>
          </span>
        </Row>
        <Row label="Animations" desc="Menus, panels and dialogs ease in. Off, they snap.">
          <Toggle on={s.animations} onChange={s.setAnimations} />
        </Row>
      </Group>

      <Group label="Terminal font">
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
        <div className="set-preview">
          <div className="term-preview" style={{ fontFamily: s.fontFamily, fontSize: s.fontSize, lineHeight: s.lineHeight ?? 1.1 }}>
            <div>
              <span className="g">❯</span> npm run build
            </div>
            <div className="d">vite v7.1.1 building for production...</div>
            <div>
              <span className="g">✓</span> 128 modules transformed
            </div>
            <div>
              <span className="b">dist/index.html</span>
              <span className="d">   0.46 kB</span>
            </div>
            <div>
              <span className="g">✓</span> built in <span className="y">1.24s</span>
            </div>
            <div>
              <span className="g">❯</span> <span className="term-caret" />
            </div>
          </div>
        </div>
      </Group>
    </>
  );
}
