import { useEffect, useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { licenseStatus, activateLicense } from "../api";
import { Logo } from "./Logo";

type Phase = "checking" | "locked" | "ok";

// Hard gate: nothing below mounts (no workspaces, no PTYs) until a valid key is stored.
export function LicenseGate({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>("checking");
  const [key, setKey] = useState("");
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    licenseStatus()
      .then((info) => setPhase(info ? "ok" : "locked"))
      .catch(() => setPhase("locked")); // can't read it → treat as locked, don't fail open
  }, []);

  const activate = async () => {
    const k = key.trim();
    if (!k || busy) return;
    setBusy(true);
    setErr("");
    try {
      await activateLicense(k);
      setPhase("ok");
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (phase === "ok") return <>{children}</>;
  // brief blank while we check the stored key — no gate flash on a licensed launch
  if (phase === "checking") return <div className="license-boot" />;

  return (
    <div className="license-gate">
      {/* frameless: no titlebar here yet, so this strip lets you move/quit the window */}
      <div className="license-bar" data-tauri-drag-region>
        <button
          className="license-quit"
          title="Quit"
          onClick={() => void getCurrentWindow().close()}
        >
          ×
        </button>
      </div>
      <div className="license-card">
        <div className="license-logo">
          <Logo size={34} />
        </div>
        <div className="license-title">HyprSpace</div>
        <div className="license-sub">Enter your license key to activate</div>
        <input
          className="license-input"
          placeholder="HSPACE-…"
          value={key}
          spellCheck={false}
          autoFocus
          autoCapitalize="off"
          autoCorrect="off"
          onChange={(e) => {
            setKey(e.target.value);
            setErr("");
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") void activate();
          }}
        />
        {err && <div className="license-err">{err}</div>}
        <button
          className="license-btn"
          disabled={busy || !key.trim()}
          onClick={() => void activate()}
        >
          {busy ? "Activating…" : "Activate"}
        </button>
      </div>
    </div>
  );
}
