import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Titlebar } from "./components/Titlebar";
import { Rail } from "./components/Rail";
import { PaneGrid } from "./components/PaneGrid";
import { useWorkspaces } from "./stores/workspace";
import { useProjectConfigs } from "./stores/projectConfig";
import { taskFromFile } from "./lib/startup";
import { useUi } from "./stores/ui";
import { useSettings } from "./stores/settings";
import { initSettingsSync } from "./stores/settingsSync";
import { Settings } from "./components/Settings";
import { CommitDialog } from "./components/CommitDialog";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { NewProjectDialog } from "./components/NewProjectDialog";
import { LaunchWorkspace } from "./components/LaunchWorkspace";
import { ServicesDialog } from "./components/ServicesDialog";
import { ServiceLogs } from "./components/ServiceLogs";
import { Updater } from "./components/Updater";
import { CommandPalette } from "./components/CommandPalette";
import { Hotkeys } from "./components/Hotkeys";
import { ReviewDock } from "./components/ReviewDock";
import { HomePage } from "./components/HomePage";
import { LoopsPage } from "./components/LoopsPage";
import { StartupRunner } from "./components/StartupRunner";
import { LoopRunner } from "./components/LoopRunner";
import { isMac } from "./platform";
import { applyTheme } from "./themes";
import { saveState, loadState, backupState, writePty } from "./api";
import "./styles/tokens.css";
import "./App.css";

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

  // ---- hydrate on launch, carefully: a read hiccup must never clobber saved data ----
  useEffect(() => {
    if (booted) return;
    booted = true;
    let cancelled = false;
    (async () => {
      const seedHome = async (enableSaving: boolean) => {
        if (useWorkspaces.getState().workspaces.length === 0) {
          const home = await invoke<string>("get_home_dir").catch(() => "");
          if (cancelled) return;
          useWorkspaces.getState().addWorkspace("Home", home || "");
        }
        if (enableSaving) useWorkspaces.getState().markHydrated();
      };

      let raw: string | null;
      try {
        raw = await loadState("workspaces");
      } catch (e) {
        console.error("workspaces load failed; running without persistence this session:", e);
        await seedHome(false);
        return;
      }
      if (cancelled) return;

      if (raw != null) {
        try {
          const parsed = JSON.parse(raw);
          if (parsed && Array.isArray(parsed.workspaces) && parsed.workspaces.length) {
            useWorkspaces
              .getState()
              .hydrate(parsed.workspaces, parsed.activeId ?? parsed.workspaces[0].id);
            return;
          }
        } catch {
          await backupState("workspaces").catch(() => {});
        }
      }
      await seedHome(true);
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
      const blob = JSON.stringify({ workspaces, activeId });
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

  // kill the default WebView2 right-click menu (Back / Refresh / Save as / Print / Inspect)
  useEffect(() => {
    const onCtx = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", onCtx);
    return () => document.removeEventListener("contextmenu", onCtx);
  }, []);

  // drop files onto a terminal pane → insert their (quoted) paths; show a drop overlay on that pane
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let lastSid: string | null = null;
    const setDrop = (sid: string | null) => {
      if (sid === lastSid) return;
      lastSid = sid;
      useUi.getState().setFileDrop(sid);
    };
    const sidAt = (px: number, py: number): string | null => {
      const dpr = window.devicePixelRatio || 1;
      const el = document.elementFromPoint(px / dpr, py / dpr) as HTMLElement | null;
      return el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
    };
    // the services config dropzone (drop a .bat/script/.exe to add it as a startup task)
    const svcDropAt = (px: number, py: number): HTMLElement | null => {
      const dpr = window.devicePixelRatio || 1;
      const el = document.elementFromPoint(px / dpr, py / dpr) as HTMLElement | null;
      return el?.closest<HTMLElement>(".svc-drop") ?? null;
    };
    const highlightSvc = (el: HTMLElement | null) => {
      document.querySelectorAll(".svc-drop.over").forEach((e) => e.classList.remove("over"));
      el?.classList.add("over");
    };
    win
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "over") {
          const svc = svcDropAt(p.position.x, p.position.y);
          highlightSvc(svc);
          setDrop(svc ? null : sidAt(p.position.x, p.position.y));
        } else if (p.type === "drop") {
          const svc = svcDropAt(p.position.x, p.position.y);
          highlightSvc(null);
          setDrop(null);
          if (svc && p.paths.length) {
            const folder = svc.dataset.folder ?? "";
            if (folder) {
              const cur = useProjectConfigs.getState().getConfig(folder).startup;
              useProjectConfigs
                .getState()
                .setConfig(folder, { startup: [...cur, ...p.paths.map((path) => taskFromFile(path))] });
            }
          } else {
            const sid = sidAt(p.position.x, p.position.y);
            if (sid && p.paths.length) {
              const text = p.paths.map((path) => (/\s/.test(path) ? `"${path}"` : path)).join(" ");
              void writePty(sid, new TextEncoder().encode(text));
            }
          }
        } else if (p.type === "leave") {
          highlightSvc(null);
          setDrop(null);
        }
      })
      .then((un) => {
        unlisten = un;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
      setDrop(null);
    };
  }, []);

  return (
    <div className="app-shell">
      <Hotkeys />
      <Titlebar />
      <div className="app-body">
        <Rail />
        {view === "home" && <HomePage />}
        {view === "loops" && <LoopsPage />}
        {/* kept mounted (PTYs stay alive) but hidden unless we're in a space */}
        <div className="workspace-view" style={{ display: view === "space" ? "flex" : "none" }}>
          <PaneGrid />
          <ReviewDock />
        </div>
      </div>
      <Updater />
      <StartupRunner />
      <LoopRunner />
      <CommandPalette />
      {settingsOpen && <Settings />}
      <CommitDialog />
      <ConfirmDialog />
      <NewProjectDialog />
      <LaunchWorkspace />
      <ServicesDialog />
      <ServiceLogs />
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
