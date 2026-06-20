import { useWorkspaces } from "../stores/workspace";
import { ServicesEditor } from "./ServicesEditor";

// The active project's services, in the dock. Config is keyed by the project's folder.
export function ServicesPanel() {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === s.activeId) ?? null);
  if (!ws || ws.kind === "open" || !ws.cwd) {
    return (
      <div className="svc-empty">
        Services are per-folder — open a project (a folder) to configure its startup tasks, env, and
        shell. You can also set them up in Settings → Startup.
      </div>
    );
  }
  return <ServicesEditor folder={ws.cwd} name={`Services · ${ws.name}`} wsId={ws.id} />;
}
