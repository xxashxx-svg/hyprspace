import { lazy, Suspense, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Titlebar } from "./components/Titlebar";
import { Rail } from "./components/Rail";
import { PaneGrid } from "./components/PaneGrid";
import { useWorkspaces } from "./stores/workspace";
import { useUi } from "./stores/ui";
import { useSettings } from "./stores/settings";
import { useProviders } from "./stores/providers";
import { initSettingsSync } from "./stores/settingsSync";
import { initBridge } from "./stores/bridge";
import { initMobileBridge } from "./mobileBridge";
import { useGit } from "./stores/git";
import { usePreview } from "./stores/preview";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { SignInScreen } from "./components/AuthGate";
import { track } from "./lib/analytics";
import { Updater } from "./components/Updater";
import { Hotkeys } from "./components/Hotkeys";
import { HomePage } from "./components/HomePage";
import { Dock } from "./components/dock/Dock";
import { WhatsNew } from "./components/WhatsNew";
import { isMac } from "./platform";
import { applyTheme } from "./themes";
import { useSessionNamer } from "./ai/autoNameSession";
import { saveState, loadState, backupState, writePty } from "./api";
import "./styles/tokens.css";
import "./App.css";

// pages/dialogs that only appear behind a condition are code-split — their chunks load on
// first open, not at startup. always-mounted stuff (Titlebar/Rail/PaneGrid/HomePage) stays static.
const Settings = lazy(() => import("./components/Settings").then((m) => ({ default: m.Settings })));
const LaunchWorkspace = lazy(() =>
  import("./components/LaunchWorkspace").then((m) => ({ default: m.LaunchWorkspace })),
);
const NewProjectDialog = lazy(() =>
  import("./components/NewProjectDialog").then((m) => ({ default: m.NewProjectDialog })),
);
const Onboarding = lazy(() => import("./components/Onboarding").then((m) => ({ default: m.Onboarding })));
const CommandPalette = lazy(() =>
  import("./components/CommandPalette").then((m) => ({ default: m.CommandPalette })),
);
const CommitDialog = lazy(() => import("./components/CommitDialog").then((m) => ({ default: m.CommitDialog })));
const PrDialog = lazy(() => import("./components/PrDialog").then((m) => ({ default: m.PrDialog })));
const InitRepoDialog = lazy(() =>
  import("./components/InitRepoDialog").then((m) => ({ default: m.InitRepoDialog })),
);
const PreviewPanel = lazy(() => import("./components/PreviewPanel").then((m) => ({ default: m.PreviewPanel })));

const win = getCurrentWindow();

// @tauri-apps/api declares this union but doesn't export it, so mirror it here
type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

// frameless window → we draw our own edge/corner resize grips.
// NOTE: no top-edge ("n") grip — it would steal the titlebar's top pixels and block grab-to-move.
// Height still resizes from the bottom edge and the top corners.
const RESIZE: Array<[string, ResizeDirection]> = [
  ["s", "South"],
  ["e", "East"],
  ["w", "West"],
  ["ne", "NorthEast"],
  ["nw", "NorthWest"],
  ["se", "SouthEast"],
  ["sw", "SouthWest"],
];

// boot exactly once per session — even across HMR remounts / concurrent effects (no double-seed)
let booted = false;

export default function App() {
  const view = useUi((s) => s.view);
  const settingsOpen = useUi((s) => s.settingsOpen);
  const signInOpen = useUi((s) => s.signInOpen);
  // open flags for the lazy dialogs — hoisted here so their chunks only load on first open
  const paletteOpen = useUi((s) => s.paletteOpen);
  const newProjectOpen = useUi((s) => s.newProjectOpen);
  const onboardingOpen = useUi((s) => s.onboardingOpen);
  const onboarded = useSettings((s) => s.onboarded);
  const settingsHydrated = useSettings((s) => s.hydrated);
  const commitOpen = useGit((s) => s.dialogOpen);
  const prOpen = useGit((s) => s.prOpen);
  const initRepoOpen = useGit((s) => s.initOpen);
  const previewOpen = usePreview((s) => s.open);
  useSessionNamer(); // periodically task-names agent panes via Codex (single-flight, kill-switchable)

  // ---- hydrate on launch, carefully: a read hiccup must never clobber saved data ----
  useEffect(() => {
    if (booted) return;
    booted = true;
    // one anonymous ping per launch — this is the whole of the "how many people use this" signal
    void track("app_opened");
    void useProviders.getState().refresh();
    let cancelled = false;
    (async () => {
      // we no longer seed a default "Home" workspace — the sidebar starts empty and the user adds
      // their own projects / open spaces. drop an unused "Home" left over from an older build too.
      type SavedWs = { id: string; name: string; kind: string; sessions: unknown[] };
      const isSeedHome = (w: SavedWs) =>
        w.name === "Home" && w.kind !== "open" && Array.isArray(w.sessions) && w.sessions.length === 0;
      const finishBoot = (enableSaving: boolean) => {
        if (enableSaving) useWorkspaces.getState().markHydrated();
      };

      let raw: string | null;
      try {
        raw = await loadState("workspaces");
      } catch (e) {
        console.error("workspaces load failed; running without persistence this session:", e);
        finishBoot(false);
        return;
      }
      if (cancelled) return;

      if (raw != null) {
        try {
          const parsed = JSON.parse(raw);
          if (parsed && Array.isArray(parsed.workspaces)) {
            const ws = parsed.workspaces.filter((w: SavedWs) => !isSeedHome(w));
            if (ws.length) {
              const activeId = ws.some((w: SavedWs) => w.id === parsed.activeId) ? parsed.activeId : ws[0].id;
              useWorkspaces.getState().hydrate(ws, activeId);
              return;
            }
          }
        } catch {
          await backupState("workspaces").catch(() => {});
        }
      }
      finishBoot(true);
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // ---- persist on change (debounced), skip no-op writes, flush before the window goes away ----
  useEffect(() => {
    let pending = false;
    let t: ReturnType<typeof setTimeout> | undefined;
    let lastSig = "";

    const doSave = () => {
      pending = false;
      const { workspaces, activeId } = useWorkspaces.getState();
      // image tabs are viewer-only and their file is usually a temp clip that gets swept later —
      // persisting them means reopening the app to tabs that can only say "couldn't open image".
      // they're one ctrl+click to get back, so drop them from the saved layout. ephemeral panes
      // (automation runs) are dropped too — saving one would relaunch its agent on next start.
      const saved = workspaces.map((w) =>
        w.sessions.some((s) => s.image || s.ephemeral)
          ? { ...w, sessions: w.sessions.filter((s) => !s.image && !s.ephemeral) }
          : w,
      );
      const blob = JSON.stringify({ workspaces: saved, activeId });
      if (blob === lastSig) return;
      lastSig = blob;
      void saveState("workspaces", blob).catch((e) => console.error("workspaces save failed:", e));
    };
    const flush = () => {
      clearTimeout(t);
      if (pending) doSave();
    };

    const unsub = useWorkspaces.subscribe((s) => {
      if (!s.hydrated) return;
      pending = true;
      clearTimeout(t);
      t = setTimeout(doSave, 300);
    });

    const onHide = () => {
      if (document.visibilityState === "hidden") flush();
    };
    window.addEventListener("blur", flush);
    window.addEventListener("pagehide", flush);
    document.addEventListener("visibilitychange", onHide);

    return () => {
      flush();
      window.removeEventListener("blur", flush);
      window.removeEventListener("pagehide", flush);
      document.removeEventListener("visibilitychange", onHide);
      unsub();
    };
  }, []);

  // ---- settings: restore + apply theme on launch, then keep both windows in sync ----
  useEffect(() => {
    let cancelled = false;
    let dispose: (() => void) | undefined;
    loadState("settings")
      .catch(() => null)
      .then((raw) => {
        if (cancelled) return;
        if (raw) {
          try {
            useSettings.getState().hydrate(JSON.parse(raw));
          } catch {
            applyTheme(useSettings.getState().theme);
            useSettings.getState().markHydrated();
          }
        } else {
          applyTheme(useSettings.getState().theme);
          useSettings.getState().markHydrated();
        }
        // owns save-on-change + cross-window broadcast (the settings window does the same)
        dispose = initSettingsSync();
      });
    return () => {
      cancelled = true;
      dispose?.();
    };
  }, []);

  // ---- mobile bridge: LAN server for the phone app, plus the state mirror it reads ----
  useEffect(() => {
    let cancelled = false;
    let disposers: (() => void)[] = [];
    void initBridge().then((stop) => {
      if (cancelled) return void stop();
      disposers = [stop, initMobileBridge()];
    });
    return () => {
      cancelled = true;
      disposers.forEach((d) => d());
    };
  }, []);

  // kill the default WebView2 right-click menu (Back / Refresh / Save as / Print / Inspect)
  useEffect(() => {
    const onCtx = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", onCtx);
    return () => document.removeEventListener("contextmenu", onCtx);
  }, []);

  // Drop files onto a terminal to insert their quoted paths, or onto a composer to attach them.
  // A composer has no PTY to write to, so it gets the paths as a DOM event on its own element —
  // that scopes the drop to the composer actually under the cursor without a registry.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let lastSid: string | null = null;
    let lastComposer: HTMLElement | null = null;
    const setDrop = (sid: string | null) => {
      if (sid === lastSid) return;
      lastSid = sid;
      useUi.getState().setFileDrop(sid);
    };
    const setComposer = (el: HTMLElement | null) => {
      if (el === lastComposer) return;
      lastComposer?.classList.remove("drop-over");
      el?.classList.add("drop-over");
      lastComposer = el;
    };
    const clear = () => {
      setDrop(null);
      setComposer(null);
    };
    const elAt = (px: number, py: number): HTMLElement | null => {
      const dpr = window.devicePixelRatio || 1;
      return document.elementFromPoint(px / dpr, py / dpr) as HTMLElement | null;
    };
    const targetAt = (px: number, py: number) => {
      const el = elAt(px, py);
      const composer = el?.closest<HTMLElement>(".composer-pane") ?? null;
      if (composer) return { composer, sid: null };
      const sid = el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
      if (!sid) return { composer: null, sid: null };
      // a viewer pane has no PTY — dropping on it would write to a dead session id
      const sess = useWorkspaces
        .getState()
        .workspaces.flatMap((w) => w.sessions)
        .find((s) => s.id === sid);
      return { composer: null, sid: sess?.image || sess?.media || sess?.diff ? null : sid };
    };
    win
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "over") {
          const t = targetAt(p.position.x, p.position.y);
          setDrop(t.sid);
          setComposer(t.composer);
        } else if (p.type === "drop") {
          const t = targetAt(p.position.x, p.position.y);
          clear();
          if (!p.paths.length) return;
          if (t.composer) {
            t.composer.dispatchEvent(new CustomEvent("hyprspace-files", { detail: p.paths }));
          } else if (t.sid) {
            const text = p.paths.map((path) => (/\s/.test(path) ? `"${path}"` : path)).join(" ");
            void writePty(t.sid, new TextEncoder().encode(text));
          }
        } else if (p.type === "leave") {
          clear();
        }
      })
      .then((un) => {
        unlisten = un;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
      clear();
    };
  }, []);

  return (
    <div className="app-shell">
      <Hotkeys />
      <Titlebar />
      <div className="app-body">
        <Rail />
        {view === "home" && <HomePage />}
        <Suspense fallback={null}>{view === "launch" && <LaunchWorkspace />}</Suspense>
        {/* kept mounted (PTYs stay alive) but hidden unless we're in a space */}
        <div className="workspace-view" style={{ display: view === "space" ? "flex" : "none" }}>
          <PaneGrid />
          <Dock />
        </div>
      </div>
      <Updater />
      <WhatsNew />
      <Suspense fallback={null}>
        {/* onboarding also mounts while undecided (!onboarded) — its own effect makes the
            new-install-vs-existing-user call, then either opens the wizard or flags + unmounts */}
        {settingsHydrated && (onboardingOpen || !onboarded) && <Onboarding />}
        {paletteOpen && <CommandPalette />}
        {settingsOpen && <Settings />}
        {signInOpen && <SignInScreen />}
        {previewOpen && <PreviewPanel />}
        {commitOpen && <CommitDialog />}
        {prOpen && <PrDialog />}
        {initRepoOpen && <InitRepoDialog />}
        {newProjectOpen && <NewProjectDialog />}
      </Suspense>
      <ConfirmDialog />
      {/* custom edge/corner resize grips — macOS keeps native decorations, so skip them there */}
      {!isMac &&
        RESIZE.map(([k, dir]) => (
          <div
            key={k}
            className={`rh rh-${k}`}
            onMouseDown={(e) => {
              e.preventDefault();
              void win.startResizeDragging(dir);
            }}
          />
        ))}
    </div>
  );
}
