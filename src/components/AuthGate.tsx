import { useEffect, useState, type ChangeEvent, type ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAuth } from "../stores/auth";
import { supabaseReady } from "../lib/supabase";
import { applyTheme } from "../themes";
import { loadState } from "../api";
import { Logo } from "./Logo";
import { EntitlementGate } from "./EntitlementGate";

function GoogleMark() {
  return (
    <svg width="16" height="16" viewBox="0 0 18 18" aria-hidden="true">
      <path fill="#4285F4" d="M17.64 9.2c0-.64-.06-1.25-.16-1.84H9v3.48h4.84a4.14 4.14 0 0 1-1.8 2.72v2.26h2.92c1.7-1.57 2.68-3.88 2.68-6.62z" />
      <path fill="#34A853" d="M9 18c2.43 0 4.47-.8 5.96-2.18l-2.92-2.26c-.8.54-1.84.86-3.04.86-2.34 0-4.32-1.58-5.03-3.7H.96v2.33A9 9 0 0 0 9 18z" />
      <path fill="#FBBC05" d="M3.97 10.72a5.4 5.4 0 0 1 0-3.44V4.95H.96a9 9 0 0 0 0 8.1l3.01-2.33z" />
      <path fill="#EA4335" d="M9 3.58c1.32 0 2.5.46 3.44 1.35l2.58-2.58A9 9 0 0 0 .96 4.95l3 2.33C4.68 5.16 6.66 3.58 9 3.58z" />
    </svg>
  );
}

// Hard account gate: nothing below mounts (no workspaces, no PTYs) until you're signed in.
// Email/password (with an emailed code to verify the address) or Google. No key fallback.
export function AuthGate({ children }: { children: ReactNode }) {
  const ready = useAuth((s) => s.ready);
  const session = useAuth((s) => s.session);
  const signingIn = useAuth((s) => s.signingIn);
  const error = useAuth((s) => s.error);
  const notice = useAuth((s) => s.notice);
  const pendingEmail = useAuth((s) => s.pendingEmail);
  const init = useAuth((s) => s.init);
  const signInGoogle = useAuth((s) => s.signInWithGoogle);
  const signInEmail = useAuth((s) => s.signInWithEmail);
  const signUpEmail = useAuth((s) => s.signUpWithEmail);
  const verifyCode = useAuth((s) => s.verifyCode);
  const resendCode = useAuth((s) => s.resendCode);
  const cancelVerify = useAuth((s) => s.cancelVerify);
  const clearMessages = useAuth((s) => s.clearMessages);

  const [mode, setMode] = useState<"signin" | "signup">("signin");
  const [email, setEmail] = useState("");
  const [pw, setPw] = useState("");
  const [code, setCode] = useState("");

  useEffect(() => {
    void init();
    // the gate renders before <App>, so apply the saved theme here or it'd show stale colors
    loadState("settings")
      .then((raw) => {
        if (!raw) return;
        try {
          const s = JSON.parse(raw);
          if (s?.theme) applyTheme(s.theme);
        } catch {
          /* ignore */
        }
      })
      .catch(() => {});
  }, [init]);

  // signed in → run the (currently dormant) subscription gate, then the app
  if (session) return <EntitlementGate>{children}</EntitlementGate>;
  if (!ready) return <div className="license-boot" />; // brief blank while we restore the session

  const onField = (set: (v: string) => void) => (e: ChangeEvent<HTMLInputElement>) => {
    set(e.target.value);
    if (error || notice) clearMessages();
  };

  // ---- verify step: enter the 6-digit code emailed after signup ----
  if (pendingEmail) {
    const submitCode = () => {
      if (code.length < 6 || signingIn) return;
      void verifyCode(code);
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
          <div className="auth-title">Verify your email</div>
          <div className="auth-subtitle">Enter the 6-digit code sent to {pendingEmail}</div>

          <div className="auth-form">
            <input
              className="auth-input auth-code"
              inputMode="numeric"
              maxLength={6}
              placeholder="••••••"
              autoFocus
              value={code}
              onChange={(e) => {
                setCode(e.target.value.replace(/\D/g, "").slice(0, 6));
                if (error || notice) clearMessages();
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitCode();
              }}
            />
            {error && <div className="auth-msg err">{error}</div>}
            {notice && <div className="auth-msg ok">{notice}</div>}
            <button className="auth-submit" disabled={signingIn || code.length < 6} onClick={submitCode}>
              {signingIn ? "Verifying…" : "Verify"}
            </button>
          </div>

          <div className="auth-verify-actions">
            <button className="auth-switch" onClick={() => void resendCode()}>
              Resend code
            </button>
            <span className="auth-dot">·</span>
            <button
              className="auth-switch"
              onClick={() => {
                cancelVerify();
                setCode("");
              }}
            >
              Back
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ---- sign in / sign up ----
  const submit = () => {
    if (!email.trim() || !pw || signingIn) return;
    if (mode === "signin") void signInEmail(email.trim(), pw);
    else void signUpEmail(email.trim(), pw);
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
        <div className="auth-subtitle">
          {mode === "signin" ? "Sign in to continue" : "Create your account"}
        </div>

        <div className="auth-form">
          <input
            className="auth-input"
            type="email"
            placeholder="Email"
            autoComplete="email"
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            autoFocus
            value={email}
            onChange={onField(setEmail)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
            }}
          />
          <input
            className="auth-input"
            type="password"
            placeholder="Password"
            autoComplete={mode === "signin" ? "current-password" : "new-password"}
            value={pw}
            onChange={onField(setPw)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
            }}
          />
          {error && <div className="auth-msg err">{error}</div>}
          {notice && <div className="auth-msg ok">{notice}</div>}
          <button
            className="auth-submit"
            disabled={signingIn || !email.trim() || !pw}
            onClick={submit}
          >
            {signingIn ? "Working…" : mode === "signin" ? "Sign in" : "Create account"}
          </button>
        </div>

        <div className="auth-divider">
          <span>or</span>
        </div>

        <button
          className="auth-google"
          disabled={signingIn || !supabaseReady}
          onClick={() => void signInGoogle()}
        >
          <GoogleMark />
          Continue with Google
        </button>

        <button
          className="auth-switch"
          onClick={() => {
            setMode((m) => (m === "signin" ? "signup" : "signin"));
            clearMessages();
          }}
        >
          {mode === "signin" ? "New here? Create an account" : "Already have an account? Sign in"}
        </button>
      </div>
    </div>
  );
}
