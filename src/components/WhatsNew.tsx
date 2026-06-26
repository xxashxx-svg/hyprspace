import { useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { loadState, saveState } from "../api";
import { useNotifications } from "../stores/notifications";
import { changelogFor } from "../lib/changelog";

// After the app updates and relaunches on a new version, post a "What's new" notification listing
// what changed (from the bundled CHANGELOG). Compares the running version to the last one we saw;
// stays quiet on a first-ever launch. Mounted once in App.
export function WhatsNew() {
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const cur = await getVersion().catch(() => "");
      if (!cur || cancelled) return;
      const seen = await loadState("lastSeenVersion").catch(() => null);
      if (seen && seen !== cur) {
        const notes = changelogFor(cur);
        useNotifications.getState().add({
          id: "whatsnew-" + cur,
          title: `What's new in v${cur}`,
          body: notes.length ? notes.map((n) => "• " + n).join("\n") : "Thanks for updating.",
          kind: "update",
        });
      }
      if (seen !== cur) void saveState("lastSeenVersion", cur).catch(() => {});
    })();
    return () => {
      cancelled = true;
    };
  }, []);
  return null;
}
