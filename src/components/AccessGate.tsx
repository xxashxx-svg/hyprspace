import { useState, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Logo } from "./Logo";

// ── PRIVATE-BETA GATE (temporary) ────────────────────────────────────────────────────────────
// Blocks the SHIPPED build behind an access code so people who find the public GitHub release
// can't use it before launch. The build stores only the SHA-256 (hex) of the code — never the code
// — so it can't be read out of the binary. Set ACCESS_HASH to enable; "" = off (today's behavior).
// Only enforced in a production build (`import.meta.env.PROD`), so your `tauri dev` is never gated.
// Remove this gate (and the wrapper in main.tsx) when you go public.
const ACCESS_HASH = "ff46ac2326bf85c400dae772fcf4ea55f7585f187c6e9bf6222a270c8523c1d9"; // hex sha256 of the access code; "" to disable
const UNLOCK_KEY = "hs-access";

async function sha256Hex(s: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(s.trim()));
  return Array.from(new Uint8Array(buf))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function AccessGate({ children }: { children: ReactNode }) {
  const enforced = import.meta.env.PROD && ACCESS_HASH.length > 0;
  const [unlocked, setUnlocked] = useState(
    () => !enforced || localStorage.getItem(UNLOCK_KEY) === ACCESS_HASH,
  );
  const [code, setCode] = useState("");
  const [err, setErr] = useState(false);
  const [busy, setBusy] = useState(false);

  if (unlocked) return <>{children}</>;

  const submit = async () => {
    if (!code || busy) return;
    setBusy(true);
    try {
      if ((await sha256Hex(code)) === ACCESS_HASH) {
        localStorage.setItem(UNLOCK_KEY, ACCESS_HASH); // remember on this machine
        setUnlocked(true);
        return;
      }
    } catch {
      /* fall through to the error state */
    }
    setBusy(false);
    setErr(true);
    setCode("");
  };

  return (
    <div className="auth-gate">
      <div className="auth-bar" data-tauri-drag-region>
        <button className="auth-quit" title="Quit" onClick={() => void getCurrentWindow().close()}>
          ×
        </button>
      </div>
      <div className="auth-card">
        <div className="auth-logo">
          <Logo size={30} />
        </div>
        <div className="auth-title">HyprSpace</div>
        <div className="auth-subtitle">Private beta — enter your access code</div>
        <div className="auth-form">
          <input
            className="auth-input"
            type="password"
            placeholder="Access code"
            autoFocus
            value={code}
            onChange={(e) => {
              setCode(e.target.value);
              if (err) setErr(false);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submit();
            }}
          />
          {err && <div className="auth-msg err">Wrong code — try again.</div>}
          <button className="auth-submit" disabled={busy || !code} onClick={() => void submit()}>
            {busy ? "Checking…" : "Unlock"}
          </button>
        </div>
      </div>
    </div>
  );
}
