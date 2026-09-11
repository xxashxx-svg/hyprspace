import { useWorkspaces } from "../stores/workspace";
import { ComposerPane } from "./composer/ComposerPane";

/** The home page: the composer for the active thread. With nothing open it asks for a folder. */
export function HomePage() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const target = workspaces.find((w) => w.id === activeId) ?? workspaces[0];
  return (
    <div className="home">
      <ComposerPane wsId={target?.id} spacePicker />
    </div>
  );
}
