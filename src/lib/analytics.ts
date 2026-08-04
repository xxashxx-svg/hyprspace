// Anonymous product analytics (PostHog).
//
// WHAT THIS SENDS, exhaustively: one event per launch ("app_opened") carrying a random install id,
// the app version, and the OS. That is the entire payload. Keep it that way — every extra property
// is something a reader of this repo has to be talked out of worrying about, and install counts and
// retention are already answered by this one event.
//
// WHAT IT NEVER SENDS: your prompts, terminal output, file paths, project or folder names, repo
// names, commands, or anything from your account. There is no way to trace an install id back to a
// person — it's a random uuid generated on this machine and never joined to a sign-in.
//
// Why a 40-line client instead of posthog-js: that SDK is built around autocapture and session
// recording, both of which would hoover up DOM content from an app whose whole screen is terminals
// full of secrets. Disabling half a large dependency is a worse privacy story than an explicit
// client you can read in one sitting. This posts to PostHog's documented capture endpoint, so the
// dashboards, retention and DAU/MAU all work the same.
//
// Off unless VITE_POSTHOG_KEY is set, off in dev builds (so our own testing doesn't pollute the
// numbers), and off whenever the user turns it off in Settings.
import { getVersion } from "@tauri-apps/api/app";
import { loadState, saveState } from "../api";
import { useSettings } from "../stores/settings";
import { isMac, isWindows } from "../platform";

const KEY = import.meta.env.VITE_POSTHOG_KEY ?? "";
const HOST = import.meta.env.VITE_POSTHOG_HOST ?? "https://us.i.posthog.com";

/** configured at build time AND a release build — dev runs are never counted */
export const analyticsReady = Boolean(KEY) && import.meta.env.PROD;

const os = isWindows ? "windows" : isMac ? "macos" : "linux";

let installId: string | null = null;
let version = "";

// A random id for this install. Persisted so the same machine counts as one returning user rather
// than a new one every launch — that's what makes retention mean anything.
async function id(): Promise<string> {
  if (installId) return installId;
  const saved = await loadState("analytics-id").catch(() => null);
  if (saved) {
    installId = saved;
    return saved;
  }
  installId = crypto.randomUUID();
  await saveState("analytics-id", installId).catch(() => {});
  return installId;
}

/** Fire and forget. Never throws, never blocks the caller, never retries — a dropped event is
 *  strictly better than analytics interfering with the app. */
export async function track(event: string, props?: Record<string, string | number | boolean>) {
  if (!analyticsReady) return;
  if (!useSettings.getState().analytics) return;
  try {
    const distinct_id = await id();
    if (!version) version = await getVersion().catch(() => "");
    await fetch(`${HOST}/capture/`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        api_key: KEY,
        event,
        distinct_id,
        properties: { ...props, version, os },
      }),
    });
  } catch {
    /* offline, blocked, or the user's firewall said no — none of that is our problem */
  }
}
