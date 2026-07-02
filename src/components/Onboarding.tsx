// First-run wizard: welcome → provider check → defaults → first workspace. Shows once for brand-new
// installs (no spaces, no flag); existing users get the flag set silently. Replay from Settings → About.
import { useEffect, useState } from "react";
import { useSettings, type ClaudePermission } from "../stores/settings";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { providerStatus, pickFolder, type ProviderStatus } from "../api";
import { claudeCmd } from "../actions";
import { THEMES } from "../themes";
import { kbd } from "../platform";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Logo } from "./Logo";
import { Blurred } from "./Blurred";
import { Check, Copy, RotateCw, FolderOpen, LayoutGrid, ArrowRight } from "lucide-react";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";

const PROVIDERS = [
  { id: "claude", label: "Claude Code", logo: claudeLogo, install: "npm install -g @anthropic-ai/claude-code" },
  { id: "codex", label: "Codex", logo: openaiLogo, install: "npm install -g @openai/codex" },
  { id: "gemini", label: "Gemini", logo: geminiLogo, install: "npm install -g @google/gemini-cli" },
  { id: "opencode", label: "OpenCode", logo: opencodeLogo, install: "npm install -g opencode-ai" },
  { id: "grok", label: "Grok", logo: grokLogo, install: "" }, // xAI ships its own installer, no npm one-liner
];

type Probe = ProviderStatus | "probing";

function CopyCmd({ cmd }: { cmd: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="ob-cmd"
      title="Copy install command"
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

function StepWelcome({ onNext, onSkip }: { onNext: () => void; onSkip: () => void }) {
  return (
    <div className="ob-step">
      <div className="ob-hero">
        <Logo size={44} />
        <h1>Welcome to HyprSpace</h1>
        <p className="ob-tagline">Your multi-agent AI workspace.</p>
      </div>
      <div className="ob-points">
        <div className="ob-point">
          <span className="ob-point-title">Tile agents across projects</span>
          <span>Run Claude Code, Codex, Gemini and more side by side. Every pane is a real terminal.</span>
        </div>
        <div className="ob-point">
          <span className="ob-point-title">Your logins, your machine</span>
          <span>Agents run on the CLIs you're already signed into. Nothing is proxied, no keys leave your computer.</span>
        </div>
        <div className="ob-point">
          <span className="ob-point-title">Automate the boring runs</span>
          <span>Schedule agents with Automations: nightly dep updates, fix-until-green, continuous review.</span>
        </div>
      </div>
      <div className="ob-actions">
        <button className="btn" onClick={onSkip}>
          Skip intro
        </button>
        <button className="btn primary" onClick={onNext}>
          Get started <ArrowRight size={13} />
        </button>
      </div>
    </div>
  );
}

function StepProviders({ onNext }: { onNext: () => void }) {
  const [probes, setProbes] = useState<Record<string, Probe>>(
    Object.fromEntries(PROVIDERS.map((p) => [p.id, "probing"])),
  );
  const probe = () => {
    setProbes(Object.fromEntries(PROVIDERS.map((p) => [p.id, "probing"])));
    for (const p of PROVIDERS) {
      providerStatus(p.id)
        .then((s) => setProbes((c) => ({ ...c, [p.id]: s })))
        .catch(() =>
          setProbes((c) => ({
            ...c,
            [p.id]: { id: p.id, installed: false, version: null, account: null, plan: null, detail: null },
          })),
        );
    }
  };
  useEffect(probe, []);

  const done = Object.values(probes).filter((p) => p !== "probing");
  const found = done.filter((p) => (p as ProviderStatus).installed).length;
  const probing = done.length < PROVIDERS.length;

  return (
    <div className="ob-step">
      <h2>Your agents</h2>
      <p className="ob-sub">
        HyprSpace launches the AI CLIs installed on this machine. Here's what it found. You need at
        least one to run agents.
      </p>
      <div className="ob-providers">
        {PROVIDERS.map((p) => {
          const s = probes[p.id];
          return (
            <div key={p.id} className="ob-provider">
              <img src={p.logo} alt="" />
              <div className="ob-provider-info">
                <span className="ob-provider-name">{p.label}</span>
                {s === "probing" ? (
                  <span className="ob-provider-sub dim">checking…</span>
                ) : s.installed ? (
                  <span className="ob-provider-sub">
                    {s.version ? `v${s.version.replace(/^v/, "")}` : "installed"}
                    {s.account ? (
                      <>
                        {" · "}
                        <Blurred text={s.account} />
                      </>
                    ) : s.plan ? (
                      ` · ${s.plan}`
                    ) : (
                      " · run it once in a terminal to sign in"
                    )}
                  </span>
                ) : p.install ? (
                  <CopyCmd cmd={p.install} />
                ) : (
                  <span className="ob-provider-sub dim">not found, grab it from xAI's docs</span>
                )}
              </div>
              <span
                className={`ob-provider-dot ${
                  s === "probing" ? "wait" : s.installed ? (s.account || s.plan ? "ok" : "warn") : "off"
                }`}
              />
            </div>
          );
        })}
      </div>
      <div className="ob-actions">
        <span className="ob-foot-note">
          {probing ? "Checking…" : found ? `${found} of ${PROVIDERS.length} ready` : "None found yet. Install one, then hit refresh"}
        </span>
        <button className="btn" onClick={probe} disabled={probing}>
          <RotateCw size={13} /> Refresh
        </button>
        <button className="btn primary" onClick={onNext}>
          Continue <ArrowRight size={13} />
        </button>
      </div>
    </div>
  );
}

const PERMS: { v: ClaudePermission; label: string; desc: string }[] = [
  { v: "plan", label: "Plan", desc: "read-only, proposes changes but never touches files" },
  { v: "acceptEdits", label: "Accept edits", desc: "edits files freely, asks before commands" },
  { v: "bypass", label: "Bypass", desc: "never asks for anything, use with care" },
  { v: "default", label: "Ask", desc: "confirms every action" },
];

function StepDefaults({ onNext }: { onNext: () => void }) {
  const perm = useSettings((s) => s.claudePermission);
  const theme = useSettings((s) => s.theme);
  return (
    <div className="ob-step">
      <h2>How agents behave</h2>
      <p className="ob-sub">How much freedom should Claude get by default? You can change this anytime.</p>
      <div className="ob-perms">
        {PERMS.map((p) => (
          <button
            key={p.v}
            className={`ob-perm${perm === p.v ? " on" : ""}`}
            onClick={() => useSettings.getState().setClaudePermission(p.v)}
          >
            <span className="ob-perm-label">
              {p.label}
              {p.v === "acceptEdits" && <em>recommended</em>}
            </span>
            <span className="ob-perm-desc">{p.desc}</span>
          </button>
        ))}
      </div>
      <p className="ob-sub" style={{ marginTop: 18 }}>
        Pick an accent. Everything else stays calm and neutral.
      </p>
      <div className="ob-themes">
        {THEMES.map((t) => (
          <button
            key={t.id}
            className={`ob-theme${theme === t.id ? " on" : ""}`}
            title={t.blurb}
            onClick={() => useSettings.getState().setTheme(t.id)}
          >
            <span className="ob-theme-swatch" style={{ background: t.vars["--accent"] }} />
            {t.name}
          </button>
        ))}
      </div>
      <div className="ob-actions">
        <button className="btn primary" onClick={onNext}>
          Continue <ArrowRight size={13} />
        </button>
      </div>
    </div>
  );
}

function StepLaunch({ finish }: { finish: () => void }) {
  const openProject = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    const name = folder.split(/[\\/]/).filter(Boolean).pop() || "Project";
    const id = useWorkspaces.getState().addWorkspace(name, folder);
    useWorkspaces.getState().addSession(id, claudeCmd()); // land in a working agent, not an empty grid
    finish();
    useUi.getState().goSpace();
  };
  const openSpace = () => {
    useWorkspaces.getState().addOpenSpace();
    finish();
    useUi.getState().goSpace();
  };
  return (
    <div className="ob-step">
      <h2>Open your first space</h2>
      <p className="ob-sub">A project pins agents to one folder; an open space mixes folders freely.</p>
      <div className="ob-launch">
        <button className="ob-launch-card" onClick={() => void openProject()}>
          <FolderOpen size={20} strokeWidth={1.75} />
          <span className="ob-launch-title">Open a project folder</span>
          <span className="ob-launch-desc">picks a folder and starts a Claude agent in it</span>
        </button>
        <button className="ob-launch-card" onClick={openSpace}>
          <LayoutGrid size={20} strokeWidth={1.75} />
          <span className="ob-launch-title">Start an open space</span>
          <span className="ob-launch-desc">a scratch grid where every pane can live in a different folder</span>
        </button>
      </div>
      <div className="ob-tips">
        <span>
          <kbd>{kbd("Ctrl K")}</kbd> command palette
        </span>
        <span>
          <kbd>{kbd("Ctrl ⇧ G")}</kbd> review dock
        </span>
        <span>
          <kbd>{kbd("Ctrl 1-9")}</kbd> switch spaces
        </span>
      </div>
      <div className="ob-actions">
        <button className="btn" onClick={finish}>
          I'll explore on my own
        </button>
      </div>
    </div>
  );
}

export function Onboarding() {
  const open = useUi((s) => s.onboardingOpen);
  const hydrated = useSettings((s) => s.hydrated);
  const onboarded = useSettings((s) => s.onboarded);
  const wsHydrated = useWorkspaces((s) => s.hydrated);
  const wsCount = useWorkspaces((s) => s.workspaces.length);
  const [step, setStep] = useState(0);

  // decide once both stores have loaded: brand-new install → wizard; existing user → flag silently
  useEffect(() => {
    if (!hydrated || !wsHydrated || onboarded) return;
    if (wsCount > 0) useSettings.getState().setOnboarded(true);
    else useUi.getState().openOnboarding();
  }, [hydrated, wsHydrated, onboarded, wsCount]);

  useEffect(() => {
    if (open) setStep(0); // replay starts from the top
  }, [open]);

  if (!open) return null;

  const finish = () => {
    useSettings.getState().setOnboarded(true);
    useUi.getState().closeOnboarding();
  };

  return (
    <div className="ob-overlay">
      <div className="ob-frame">
        {step === 0 && <StepWelcome onNext={() => setStep(1)} onSkip={finish} />}
        {step === 1 && <StepProviders onNext={() => setStep(2)} />}
        {step === 2 && <StepDefaults onNext={() => setStep(3)} />}
        {step === 3 && <StepLaunch finish={finish} />}
        <div className="ob-dots">
          {[0, 1, 2, 3].map((i) => (
            <button
              key={i}
              className={`ob-dot${i === step ? " on" : ""}`}
              aria-label={`step ${i + 1}`}
              onClick={() => setStep(i)}
            />
          ))}
        </div>
        {step > 0 && (
          <button className="ob-skip" onClick={finish}>
            Skip
          </button>
        )}
      </div>
    </div>
  );
}
