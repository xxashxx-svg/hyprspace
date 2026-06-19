import { useEffect, useState, type CSSProperties } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useWorkspaces } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useAuth } from "../stores/auth";
import { pickFolder } from "../api";
import { Folder, FolderPlus, LayoutGrid, ChevronRight } from "lucide-react";
import { ChatPanel } from "./ChatPanel";

export function HomePage() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const addWorkspace = useWorkspaces((s) => s.addWorkspace);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const setActive = useWorkspaces((s) => s.setActive);
  const goSpace = useUi((s) => s.goSpace);
  const openNewProject = useUi((s) => s.openNewProject);
  const user = useAuth((s) => s.user);

  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  const name =
    ((user?.user_metadata?.full_name as string) || "").trim().split(" ")[0] ||
    user?.email?.split("@")[0] ||
    "there";
  const hour = new Date().getHours();
  const greet =
    hour < 5 ? "Working late" : hour < 12 ? "Good morning" : hour < 18 ? "Good afternoon" : "Good evening";

  const openProject = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    const n = folder.split(/[\\/]/).filter(Boolean).pop() || "Project";
    addWorkspace(n, folder); // sets it active
    goSpace();
  };
  const newOpen = () => {
    addOpenSpace();
    goSpace();
  };
  const enter = (id: string) => {
    setActive(id);
    goSpace();
  };

  return (
    <div className="home">
      <div className="home-scroll">
      <div className="home-inner">
        <header className="home-head">
          <div className="home-greeting">
            {greet}, {name}
          </div>
          <div className="home-sub">Pick up where you left off, or start something new.</div>
        </header>

        <div className="home-actions">
          <button className="home-action" onClick={openNewProject}>
            <span className="home-action-ico">
              <FolderPlus size={18} />
            </span>
            <span className="home-action-body">
              <span className="home-action-title">New project</span>
              <span className="home-action-sub">Folder, agents &amp; git in one step</span>
            </span>
            <ChevronRight size={16} className="home-action-arrow" />
          </button>
          <button className="home-action" onClick={() => void openProject()}>
            <span className="home-action-ico">
              <Folder size={18} />
            </span>
            <span className="home-action-body">
              <span className="home-action-title">Open a project</span>
              <span className="home-action-sub">Open a folder as a project</span>
            </span>
            <ChevronRight size={16} className="home-action-arrow" />
          </button>
          <button className="home-action" onClick={newOpen}>
            <span className="home-action-ico">
              <LayoutGrid size={18} />
            </span>
            <span className="home-action-body">
              <span className="home-action-title">New open space</span>
              <span className="home-action-sub">A scratch space for any folders</span>
            </span>
            <ChevronRight size={16} className="home-action-arrow" />
          </button>
        </div>

        <section className="home-section">
          <div className="home-section-title">Your spaces</div>
          {workspaces.length === 0 ? (
            <div className="home-empty">
              No spaces yet — open a project or create an open space to start.
            </div>
          ) : (
            <div className="home-grid">
              {workspaces.map((w) => (
                <button
                  key={w.id}
                  className="home-card"
                  onClick={() => enter(w.id)}
                  style={{ "--ws": w.color } as CSSProperties}
                >
                  <span className="home-card-ico">
                    {w.kind === "open" ? <LayoutGrid size={16} /> : <Folder size={16} />}
                  </span>
                  <span className="home-card-body">
                    <span className="home-card-name">{w.name}</span>
                    <span className="home-card-meta">
                      {w.kind === "open" ? "Open space" : "Project"}
                      {w.sessions.length > 0 &&
                        ` · ${w.sessions.length} session${w.sessions.length > 1 ? "s" : ""}`}
                    </span>
                  </span>
                  {w.sessions.length > 0 && <span className="home-card-dot" title="Has active sessions" />}
                </button>
              ))}
            </div>
          )}
        </section>

        <div className="home-tips">
          <span className="home-tip">
            <kbd>Ctrl K</kbd> Search &amp; commands
          </span>
          <span className="home-tip">
            <kbd>Ctrl ⇧ T</kbd> New terminal
          </span>
          <span className="home-tip">
            <kbd>Ctrl ⇧ G</kbd> Review dock
          </span>
        </div>
      </div>
      </div>
      <ChatPanel />
      <footer className="home-foot">
        <span>HyprSpace{version ? ` v${version}` : ""}</span>
        {user?.email && <span className="home-foot-acct">{user.email}</span>}
      </footer>
    </div>
  );
}
