import { useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { useWorkspaces, type Workspace } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useAuth } from "../stores/auth";
import { pickFolder } from "../api";
import { kbd } from "../platform";
import { Folder, FolderPlus, Layers, LayoutGrid } from "lucide-react";
import { Blurred } from "./Blurred";
import { useAgentStatus, displayState } from "../stores/agentStatus";
import { relTime } from "../lib/time";

export function HomePage() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const addWorkspace = useWorkspaces((s) => s.addWorkspace);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const setActive = useWorkspaces((s) => s.setActive);
  const goSpace = useUi((s) => s.goSpace);
  const openNewProject = useUi((s) => s.openNewProject);
  const user = useAuth((s) => s.user);
  const byPane = useAgentStatus((s) => s.byPane);

  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  // no name is fine — "Good morning, there" reads like a bug, so just drop the comma
  const name =
    ((user?.user_metadata?.full_name as string) || "").trim().split(" ")[0] ||
    user?.email?.split("@")[0] ||
    "";
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

  // "Continue" = the spaces you were last in that still have panes open. Ranked by when you last
  // entered them, falling back to the freshest agent activity for spaces predating that stamp.
  const cont = useMemo(() => {
    const when = (w: Workspace) =>
      Math.max(
        w.lastOpenedAt ?? 0,
        ...w.sessions.map((x) => byPane[x.id]?.since ?? 0),
        0,
      );
    return workspaces
      .filter((w) => w.sessions.length > 0 && when(w) > 0)
      .sort((a, b) => when(b) - when(a))
      .slice(0, 3);
  }, [workspaces, byPane]);

  const contIds = useMemo(() => new Set(cont.map((w) => w.id)), [cont]);
  const rest = useMemo(() => workspaces.filter((w) => !contIds.has(w.id)), [workspaces, contIds]);

  /** the most interesting state across a space's panes — waiting beats working beats done */
  const stateOf = (w: Workspace) => {
    const now = Date.now();
    const states = w.sessions.map((x) => displayState(byPane[x.id], now));
    if (states.includes("waiting")) return "await";
    if (states.includes("working")) return "busy";
    return "ok";
  };

  /** one line of what's happening in there, or a plain description when nothing is reporting */
  const activityOf = (w: Workspace) => {
    const now = Date.now();
    const waiting = w.sessions.find((x) => displayState(byPane[x.id], now) === "waiting");
    const working = w.sessions.find((x) => displayState(byPane[x.id], now) === "working");
    const pick = waiting ?? working;
    const line = pick && byPane[pick.id]?.activity;
    if (line) return line;
    const n = w.sessions.length;
    return `${w.kind === "open" ? "Open space" : "Project"} · ${n} session${n === 1 ? "" : "s"}`;
  };

  return (
    <div className="home">
      <div className="home-scroll">
      <div className="home-inner">
        <header className="home-head">
          <div className="home-greeting">{name ? `${greet}, ${name}` : greet}</div>
        </header>

        <div className="home-row">
          <button className="home-btn primary" onClick={() => useUi.getState().openLaunch()}>
            <LayoutGrid size={14} />
            Launch agents
          </button>
          <span className="home-row-div" />
          <button className="home-btn" onClick={openNewProject}>
            <FolderPlus size={14} />
            New project
          </button>
          <button className="home-btn" onClick={() => void openProject()}>
            <Folder size={14} />
            Open a project
          </button>
          <button className="home-btn" onClick={newOpen}>
            <Layers size={14} />
            New open space
          </button>
        </div>

        {cont.length > 0 && (
          <section className="home-section">
            <div className="home-section-title">Continue</div>
            <div className="home-cont">
              {cont.map((w) => (
                <button key={w.id} className="home-cc" onClick={() => enter(w.id)}>
                  <span className="home-cc-top">
                    {w.kind === "open" ? <Folder size={14} /> : <Folder size={14} />}
                    <span className="home-cc-name">{w.name}</span>
                    <span className={`home-dot ${stateOf(w)}`} />
                  </span>
                  <span className="home-cc-act">{activityOf(w)}</span>
                  <span className="home-cc-foot">
                    <span>
                      {w.sessions.length} agent{w.sessions.length === 1 ? "" : "s"}
                    </span>
                    {w.lastOpenedAt && (
                      <>
                        <span>·</span>
                        <span>{relTime(w.lastOpenedAt)}</span>
                      </>
                    )}
                  </span>
                </button>
              ))}
            </div>
          </section>
        )}

        <section className="home-section">
          <div className="home-section-title">
            All spaces <span className="home-count">{workspaces.length}</span>
          </div>
          {workspaces.length === 0 ? (
            <div className="home-empty">
              No spaces yet — open a project or create an open space to start.
            </div>
          ) : (
            <div className="home-list">
              {rest.map((w) => (
                <button key={w.id} className="home-r" onClick={() => enter(w.id)}>
                  {w.kind === "open" ? (
                    <Layers size={14} className="home-r-ico" />
                  ) : (
                    <Folder size={14} className="home-r-ico" />
                  )}
                  <span className="home-r-name">{w.name}</span>
                  <span className="home-r-kind">{w.kind === "open" ? "Open space" : "Project"}</span>
                  <span className="home-r-right">
                    {w.sessions.length > 0 && (
                      <span>
                        {w.sessions.length} session{w.sessions.length === 1 ? "" : "s"}
                      </span>
                    )}
                    {w.lastOpenedAt && <span>{relTime(w.lastOpenedAt)}</span>}
                    <span className={`home-dot ${w.sessions.length ? stateOf(w) : "off"}`} />
                  </span>
                </button>
              ))}
            </div>
          )}
        </section>

        <div className="home-tips">
          <span className="home-tip">
            <kbd>{kbd("Ctrl K")}</kbd> Search &amp; commands
          </span>
          <span className="home-tip">
            <kbd>{kbd("Ctrl ⇧ T")}</kbd> New terminal
          </span>
          <span className="home-tip">
            <kbd>{kbd("Ctrl ⇧ G")}</kbd> Review dock
          </span>
        </div>
      </div>
      </div>
      <footer className="home-foot">
        <span>HyprSpace{version ? ` v${version}` : ""}</span>
        {user?.email && (
          <span className="home-foot-acct">
            <Blurred text={user.email} />
          </span>
        )}
      </footer>
    </div>
  );
}
