import { useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAuth } from "../stores/auth";
import { useEntitlement } from "../stores/entitlement";
import { Logo } from "./Logo";

// Where "Subscribe" sends the user. Points at the site's root rather than /subscribe: that path
// doesn't exist yet (it 404s), and the root at least lands them somewhere real. Swap this for the
// Polar checkout link when going paid.
const SUBSCRIBE_URL = "https://hyprspace.dev";

// Shown only when the backend says paid + this account isn't entitled. Reuses the auth-gate styles
// so it feels like the sign-in screen.
export function Paywall() {
  const reason = useEntitlement((s) => s.reason);
  const tier = useEntitlement((s) => s.tier);
  const check = useEntitlement((s) => s.check);
  const signOut = useAuth((s) => s.signOut);
  const user = useAuth((s) => s.user);
  const [busy, setBusy] = useState(false);

  const recheck = async () => {
    setBusy(true);
    await check();
    setBusy(false);
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
        <div className="auth-title">Subscription required</div>
        <div className="auth-subtitle">
          {reason || "HyprSpace now needs an active subscription to continue."}
          {tier ? ` (${tier})` : ""}
        </div>

        <div className="auth-form">
          <button className="auth-submit" onClick={() => void openUrl(SUBSCRIBE_URL)}>
            Subscribe
          </button>
          <button className="auth-google" disabled={busy} onClick={() => void recheck()}>
            {busy ? "Checking…" : "I've subscribed — re-check"}
          </button>
        </div>

        <button className="auth-switch" onClick={() => void signOut()}>
          {user?.email ? `Sign out (${user.email})` : "Sign out"}
        </button>
      </div>
    </div>
  );
}
