import { BellOff, Download, PlayCircle, Radio, RefreshCw, Tag } from "lucide-react";
import { useSettings } from "../../stores/settings";
import { useUi } from "../../stores/ui";
import { useUpdater, type UpdatePhase } from "../../stores/updater";
import { useVersion } from "../../lib/version";
import { Logo } from "../Logo";
import { Row, Group, Toggle } from "./controls";

// Settings → General: the app itself (version, updates, the intro), how it behaves, and the one
// thing it sends. Keep the ping copy in step with lib/analytics.ts (CLAUDE.md rule 8).

const TONE: Record<UpdatePhase, string> = {
  idle: "",
  checking: "busy",
  downloading: "busy",
  available: "new",
  uptodate: "ok",
  error: "err",
};

function AppCard() {
  const phase = useUpdater((u) => u.phase);
  const detail = useUpdater((u) => u.detail);
  const pct = useUpdater((u) => u.pct);
  const update = useUpdater((u) => u.update);
  const checkNow = useUpdater((u) => u.checkNow);
  const install = useUpdater((u) => u.install);

  const version = useVersion();

  const status =
    phase === "checking"
      ? "Checking for updates"
      : phase === "available"
        ? `Version ${update?.version} is ready to install`
        : phase === "downloading" || phase === "error"
          ? detail
          : phase === "uptodate"
            ? "You're on the latest version"
            : "Checks for updates on launch";
  const busy = phase === "checking" || phase === "downloading";

  return (
    <div className="gn-app">
      <div className="gn-app-main">
        <span className="gn-logo">
          <Logo size={26} />
        </span>
        <div className="gn-app-info">
          <div className="gn-app-name">
            HyprSpace <span className="gn-ver">{version ? `v${version}` : ""}</span>
          </div>
          <div className={`gn-status ${TONE[phase]}`}>
            <i />
            {status}
          </div>
        </div>
        {phase === "available" ? (
          <button className="btn primary gn-btn" onClick={() => void install()}>
            <Download size={14} />
            Restart and update
          </button>
        ) : (
          <button className="btn gn-btn" onClick={() => void checkNow()} disabled={busy}>
            <RefreshCw size={14} className={busy ? "gn-spin" : undefined} />
            {phase === "checking" ? "Checking" : "Check for updates"}
          </button>
        )}
      </div>
      {phase === "downloading" && (
        <div className={`gn-progress${pct < 0 ? " indet" : ""}`}>
          <span style={{ width: pct < 0 ? undefined : `${pct}%` }} />
        </div>
      )}
      <div className="gn-app-foot">
        <span>Runs the Claude, Codex, Gemini, OpenCode and Grok CLIs you already have, side by side, on your own machine.</span>
        <button className="gn-link" onClick={() => useUi.getState().openOnboarding()}>
          <PlayCircle size={13} />
          Show the intro
        </button>
      </div>
    </div>
  );
}

export function General() {
  const s = useSettings();
  const hidden = s.dismissedConfirms.length;
  return (
    <>
      <AppCard />

      <Group label="Behavior">
        <Row icon={<Tag />} label="Name panes after their task" desc="Codex writes a short title from the first prompt. Off, panes are named after their folder.">
          <Toggle on={s.autoNameAgents} onChange={s.setAutoNameAgents} />
        </Row>
        <Row icon={<BellOff />} label="Hidden confirmations" desc="Bring back the dialogs you dismissed with 'don't ask again'.">
          <button className="btn" disabled={!hidden} onClick={() => s.resetDismissedConfirms()}>
            {hidden ? `Show ${hidden} again` : "None hidden"}
          </button>
        </Row>
      </Group>

      <Group label="Privacy">
        <Row
          icon={<Radio />}
          label="Anonymous launch ping"
          desc="One ping per launch with a random install id, the version, and the OS. Never prompts, output, paths, or project names."
        >
          <Toggle on={s.analytics} onChange={s.setAnalytics} />
        </Row>
      </Group>
    </>
  );
}
