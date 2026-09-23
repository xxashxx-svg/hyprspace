// The intro: what HyprSpace is, which agents it found, how the app fits together, the defaults, and
// a first folder. Shows once for brand-new installs (no spaces, no flag); existing users get the flag
// set silently. Replay from Settings → General. Every claim here should match a real feature.
import { useEffect, useState, type ReactNode } from "react";
import { ArrowLeft, ArrowRight, Check, Copy, FolderOpen, MousePointerClick, RotateCw, ShieldOff } from "lucide-react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useSettings } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useProviders } from "../stores/providers";
import { AGENT_IDS, CLAUDE_PERMISSIONS } from "../lib/models";
import { PROVIDER_LOGO, PROVIDER_NAME } from "../lib/brand";
import { openFolderAsSpace } from "../actions";
import { THEMES, type Scheme } from "../themes";
import { isMac, kbd } from "../platform";
import { Logo } from "./Logo";
import { Blurred } from "./Blurred";
import {
  ComposerDemo,
  FoldersDemo,
  GitDemo,
  PaletteDemo,
  PanesDemo,
  TerminalDemo,
  ThreadsDemo,
  UsageDemo,
} from "./onboarding/demos";

const INSTALL: Record<string, string> = {
  claude: "npm install -g @anthropic-ai/claude-code",
  codex: "npm install -g @openai/codex",
  gemini: "npm install -g @google/gemini-cli",
  opencode: "npm install -g opencode-ai",
  // xAI ships its own installer, no npm one-liner
};

const STEPS = ["Welcome", "Your agents", "The basics", "The tools", "Defaults", "Start"];

function Keys({ k }: { k: string }) {
  // mac: one cap per modifier glyph, then the key ("⌘1-9" → ⌘ 1-9)
  const s = kbd(k);
  const m = /^([⌘⇧⌥]*)(.*)$/.exec(s);
  const caps = isMac && m ? [...m[1], m[2]].filter(Boolean) : s.split(/[+ ]/);
  return (
    <span className="ob-keys">
      {caps.map((c, i) => (
        <kbd key={i}>{c}</kbd>
      ))}
    </span>
  );
}

function CopyCmd({ cmd }: { cmd: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="ob-cmd"
      title="Copy the install command"
      onClick={() => {
        void writeText(cmd);
        setDone(true);
        setTimeout(() => setDone(false), 1200);
      }}
    >
      <code>{cmd}</code>
      {done ? <Check size={12} /> : <Copy size={12} />}
    </button>
  );
}

// A topic list on the left and a working copy of that part of the app on the right. The copy
// remounts when you switch topics, so each one starts fresh.
type Topic = { title: string; sub: string; hint: string; demo: ReactNode };
function Tour({ topics }: { topics: Topic[] }) {
  const [i, setI] = useState(0);
  const t = topics[i];
  return (
    <div className="ob-tour">
      <div className="ob-topics">
        {topics.map((x, k) => (
          <button key={x.title} className={k === i ? "on" : ""} onClick={() => setI(k)}>
            <span className="ob-topic-title">{x.title}</span>
            <span className="ob-topic-sub">{x.sub}</span>
          </button>
        ))}
      </div>
      <div className="ob-stage">
        <div className="ob-stage-demo" key={i}>
          {t.demo}
        </div>
        <div className="ob-stage-hint">
          <MousePointerClick size={13} />
          {t.hint}
        </div>
      </div>
    </div>
  );
}

// a small drawing of the app: the sidebar, and four threads tiled in the grid
function AppSketch() {
  const panes = ["claude", "codex", "gemini", "claude"];
  return (
    <div className="ob-sketch" aria-hidden="true">
      <div className="ob-sketch-rail">
        <span className="ob-sketch-search" />
        {["api-server", "website", "mobile-app"].map((f, i) => (
          <div key={f} className="ob-sketch-folder">
            <span className="ob-sketch-name">{f}</span>
            {i < 2 && <span className="ob-sketch-thread" />}
            {i === 0 && <span className="ob-sketch-thread short" />}
          </div>
        ))}
      </div>
      <div className="ob-sketch-grid">
        {panes.map((p, i) => (
          <div key={i} className="ob-sketch-pane">
            <div className="ob-sketch-head">
              <img src={PROVIDER_LOGO[p]} alt="" />
              <span>{PROVIDER_NAME[p]}</span>
            </div>
            <span className="ob-sketch-line" style={{ width: `${70 - i * 9}%` }} />
            <span className="ob-sketch-line" style={{ width: `${45 + i * 8}%` }} />
            <span className="ob-sketch-line dim" style={{ width: `${55 - i * 4}%` }} />
          </div>
        ))}
      </div>
    </div>
  );
}

function Welcome() {
  return (
    <>
      <h1>Run your coding agents side by side</h1>
      <p className="ob-lead">
        HyprSpace is one window for the AI coding tools you already use. Each thread is a real terminal
        running Claude Code, Codex, Gemini, OpenCode or Grok in a folder you choose.
      </p>
      <AppSketch />
      <p className="ob-aside">
        There's no new account. Agents use the logins your CLIs already have. HyprSpace types into the CLI, and your
        prompts go from the CLI straight to its provider. It all runs on this computer.
      </p>
    </>
  );
}

function Agents() {
  const status = useProviders((p) => p.status);
  const checking = useProviders((p) => p.checking);
  const checkedAt = useProviders((p) => p.checkedAt);
  const refresh = useProviders((p) => p.refresh);
  useEffect(() => {
    if (checkedAt == null) void refresh();
  }, [checkedAt, refresh]);

  const ready = AGENT_IDS.filter((id) => status[id]?.installed).length;
  return (
    <>
      <h1>Check your agents</h1>
      <p className="ob-lead">
        HyprSpace has no AI of its own. It starts these command-line tools for you, signed in as you. You need
        at least one installed and signed in.
      </p>
      <div className="ob-agents">
        {AGENT_IDS.map((id) => {
          const s = status[id];
          const tone = !s ? "wait" : !s.installed ? "off" : s.account || s.plan ? "ok" : "warn";
          return (
            <div key={id} className={`ob-agent ${tone}`}>
              <img src={PROVIDER_LOGO[id]} alt="" />
              <div className="ob-agent-info">
                <span className="ob-agent-name">{PROVIDER_NAME[id]}</span>
                {!s ? (
                  <span className="ob-agent-sub">Checking</span>
                ) : s.installed ? (
                  <span className="ob-agent-sub">
                    {s.version ? `v${s.version.replace(/^v/, "")}` : "Installed"}
                    {s.account ? (
                      <>
                        {" · "}
                        <Blurred text={s.account} />
                      </>
                    ) : s.plan ? (
                      ` · ${s.plan}`
                    ) : (
                      " · Run it once in a terminal to sign in"
                    )}
                  </span>
                ) : INSTALL[id] ? (
                  <CopyCmd cmd={INSTALL[id]} />
                ) : (
                  <span className="ob-agent-sub">Not found. Install it from xAI's docs.</span>
                )}
              </div>
              <span className="ob-agent-state">{tone === "ok" ? "Ready" : tone === "warn" ? "Sign in" : tone === "off" ? "Not installed" : ""}</span>
            </div>
          );
        })}
      </div>
      <div className="ob-row">
        <span className="ob-note">
          {checking && checkedAt == null ? "Checking this machine" : ready ? `${ready} of ${AGENT_IDS.length} installed` : "None found. Install one, then check again."}
        </span>
        <button className="btn ob-btn" onClick={() => void refresh()} disabled={checking}>
          <RotateCw size={13} className={checking ? "ob-spin" : undefined} />
          Check again
        </button>
      </div>
    </>
  );
}

function Basics() {
  return (
    <>
      <h1>Folders, threads and panes</h1>
      <p className="ob-lead">Four ideas cover most of the app. Each one on the right works, so try it.</p>
      <Tour
        topics={[
          {
            title: "Folders",
            sub: "The sidebar lists them",
            hint: "Click a folder to open or fold it. The dot on a thread shows if the agent is working or waiting on you.",
            demo: <FoldersDemo />,
          },
          {
            title: "Threads",
            sub: "One agent, one task",
            hint: "Press the square button and pick a folder. The + in the titlebar skips the question and uses the folder you're in.",
            demo: <ThreadsDemo />,
          },
          {
            title: "The composer",
            sub: "Where every thread starts",
            hint: "Pick an agent and effort, type a task, press Enter. Past conversations in the folder show under the box.",
            demo: <ComposerDemo />,
          },
          {
            title: "Panes",
            sub: "Threads tile into a grid",
            hint: "Drag a header onto another pane to swap them. Double-click a header to maximize, again to restore.",
            demo: <PanesDemo />,
          },
        ]}
      />
    </>
  );
}

function Tools() {
  return (
    <>
      <h1>The tools around them</h1>
      <p className="ob-lead">Everything else is a shortcut away. These work too.</p>
      <Tour
        topics={[
          {
            title: "Command palette",
            sub: kbd("Ctrl K"),
            hint: "Type to filter commands and threads. Two letters or more also searches the text in every terminal.",
            demo: <PaletteDemo />,
          },
          {
            title: "Terminals",
            sub: "Real ones, with extras",
            hint: `Hold ${kbd("Ctrl")} and click a file path to open it. Hover [Image #1] to see what you pasted.`,
            demo: <TerminalDemo />,
          },
          {
            title: "Files and git",
            sub: kbd("Ctrl+Shift+G"),
            hint: "Tick files, write a summary, commit. Push shows up once you're ahead.",
            demo: <GitDemo />,
          },
          {
            title: "Usage",
            sub: "In the titlebar",
            hint: "How much of each plan's limits is left, and when they reset.",
            demo: <UsageDemo />,
          },
        ]}
      />
      <p className="ob-aside">
        Also in Settings: Mobile mirrors your threads to the Android app over wifi, and Skills keeps reusable
        instructions for Claude.
      </p>
    </>
  );
}

const SCHEMES: { v: Scheme; label: string }[] = [
  { v: "dark", label: "Dark" },
  { v: "light", label: "Light" },
  { v: "system", label: "Match the system" },
];

function Defaults() {
  const perm = useSettings((s) => s.claudePermission);
  const theme = useSettings((s) => s.theme);
  const scheme = useSettings((s) => s.colorScheme);
  return (
    <>
      <h1>Set your defaults</h1>
      <p className="ob-lead">
        How much Claude may do without asking. Codex and Gemini have their own modes in Settings, Defaults. You can
        change all of it later.
      </p>
      <div className="ob-perms">
        {CLAUDE_PERMISSIONS.map((p) => (
          <button
            key={p.value}
            className={`ob-perm${perm === p.value ? " on" : ""}${p.risky ? " risky" : ""}`}
            onClick={() => useSettings.getState().setClaudePermission(p.value)}
          >
            <span className="ob-perm-label">
              {p.risky && <ShieldOff size={13} />}
              {p.label}
              {p.value === "acceptEdits" && <em>Recommended</em>}
            </span>
            <span className="ob-perm-says">{p.says}</span>
          </button>
        ))}
      </div>
      <div className="ob-sub-h">Look</div>
      <div className="ob-seg">
        {SCHEMES.map((s) => (
          <button key={s.v} className={scheme === s.v ? "on" : ""} onClick={() => useSettings.getState().setColorScheme(s.v)}>
            {s.label}
          </button>
        ))}
      </div>
      <div className="ob-themes">
        {THEMES.map((t) => (
          <button key={t.id} className={`ob-theme${theme === t.id ? " on" : ""}`} title={t.blurb} onClick={() => useSettings.getState().setTheme(t.id)}>
            <span className="ob-theme-swatch" style={{ background: t.vars.dark["--accent"] }} />
            {t.name}
          </button>
        ))}
      </div>
    </>
  );
}

const SHORTCUTS: [string, string][] = [
  ["Ctrl K", "Command palette (outside a terminal)"],
  ["Ctrl+Shift+N", "New thread in this folder"],
  ["Ctrl+Shift+T", "New plain terminal"],
  ["Ctrl+Shift+G", "Files and git"],
  ["Ctrl+Shift+M", "Maximize the pane"],
  ["Ctrl+Shift+W", "Close the pane"],
  ["Ctrl 1-9", "Jump to a folder"],
  ["Ctrl Tab", "Next folder"],
];

function Start({ finish }: { finish: () => void }) {
  const pick = async () => {
    const before = useWorkspaces.getState().workspaces.length;
    await openFolderAsSpace();
    if (useWorkspaces.getState().workspaces.length === before) return; // picker cancelled
    finish();
    useUi.getState().goSpace();
  };
  return (
    <>
      <h1>Pick a folder to start</h1>
      <p className="ob-lead">It opens in the sidebar with a composer waiting. Type a task and press Enter.</p>
      <button className="ob-start" onClick={() => void pick()}>
        <span className="ob-start-ico">
          <FolderOpen size={20} />
        </span>
        <span className="ob-start-text">
          <span className="ob-start-title">Choose a folder</span>
          <span className="ob-start-sub">A project you want an agent to work on</span>
        </span>
        <ArrowRight size={16} />
      </button>
      <div className="ob-sub-h">Shortcuts worth learning</div>
      <div className="ob-shortcuts">
        {SHORTCUTS.map(([k, what]) => (
          <div key={k} className="ob-shortcut">
            <Keys k={k} />
            <span>{what}</span>
          </div>
        ))}
      </div>
    </>
  );
}

export function Onboarding() {
  const open = useUi((s) => s.onboardingOpen);
  const hydrated = useSettings((s) => s.hydrated);
  const onboarded = useSettings((s) => s.onboarded);
  const wsHydrated = useWorkspaces((s) => s.hydrated);
  const wsCount = useWorkspaces((s) => s.workspaces.length);
  const [step, setStep] = useState(0);
  const [leaving, setLeaving] = useState(false);

  // decide once both stores have loaded: brand-new install → intro; existing user → flag silently
  useEffect(() => {
    if (!hydrated || !wsHydrated || onboarded) return;
    if (wsCount > 0) useSettings.getState().setOnboarded(true);
    else useUi.getState().openOnboarding();
  }, [hydrated, wsHydrated, onboarded, wsCount]);

  // a replay starts from the top. setting state while rendering (not in an effect) is React's
  // pattern for resetting on a prop change
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setStep(0);
      setLeaving(false);
    }
  }

  if (!open) return null;

  const finish = () => {
    useSettings.getState().setOnboarded(true);
    useUi.getState().closeOnboarding();
  };
  // leaving on purpose plays the exit: the card sinks away, then the backdrop closes in to a point
  // and the app shows through. skipped when motion is off (the setting or the OS)
  const leave = () => {
    const still =
      document.documentElement.classList.contains("reduce-motion") || matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (still) return finish();
    setLeaving(true);
    setTimeout(finish, 720);
  };
  const last = step === STEPS.length - 1;

  return (
    <div className={`ob-overlay${leaving ? " leaving" : ""}`}>
      <div className="ob-frame">
        <nav className="ob-nav">
          <div className="ob-brand">
            <Logo size={20} />
            HyprSpace
          </div>
          <ol className="ob-steps">
            {STEPS.map((s, i) => (
              <li key={s}>
                <button className={i === step ? "on" : i < step ? "done" : ""} onClick={() => setStep(i)}>
                  <span className="ob-step-n">{i < step ? <Check size={11} strokeWidth={3} /> : i + 1}</span>
                  {s}
                </button>
              </li>
            ))}
          </ol>
          <button className="ob-skip" onClick={leave}>
            Skip the intro
          </button>
        </nav>

        <div className="ob-main">
          <div className="ob-body" key={step}>
            <div className="ob-eyebrow">
              Step {step + 1} of {STEPS.length}
            </div>
            {step === 0 && <Welcome />}
            {step === 1 && <Agents />}
            {step === 2 && <Basics />}
            {step === 3 && <Tools />}
            {step === 4 && <Defaults />}
            {step === 5 && <Start finish={finish} />}
          </div>
          <div className="ob-foot">
            {step > 0 && (
              <button className="btn ob-btn" onClick={() => setStep(step - 1)}>
                <ArrowLeft size={13} />
                Back
              </button>
            )}
            <span className="ob-foot-gap" />
            {last ? (
              <button className="btn ob-btn" onClick={leave}>
                I'll look around first
              </button>
            ) : (
              <button className="btn primary ob-btn" onClick={() => setStep(step + 1)}>
                {step === 0 ? "Get started" : "Next"}
                <ArrowRight size={13} />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
