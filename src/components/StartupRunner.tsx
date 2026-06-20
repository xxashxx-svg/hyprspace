import { useEffect } from "react";
import { useWorkspaces } from "../stores/workspace";
import { useProjectConfigs } from "../stores/projectConfig";
import { maybeAutostart } from "../lib/startup";

// Loads the folder configs and autostarts a project's services the first time its folder opens this
// session. Renders nothing.
export function StartupRunner() {
  const activeId = useWorkspaces((s) => s.activeId);
  const hydrated = useWorkspaces((s) => s.hydrated);
  const cfgLoaded = useProjectConfigs((s) => s.loaded);

  useEffect(() => {
    void useProjectConfigs.getState().load();
  }, []);

  useEffect(() => {
    if (hydrated && cfgLoaded && activeId) maybeAutostart(activeId);
  }, [activeId, hydrated, cfgLoaded]);

  return null;
}
