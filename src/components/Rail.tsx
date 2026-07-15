import { useEffect, useLayoutEffect, useReducer, useRef, useState } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { useWorkspaces, type Workspace } from "../stores/workspace";
import { useUi } from "../stores/ui";
import { useLoops } from "../stores/loops";
import { useActivity } from "../stores/activity";
import { relTime } from "../lib/time";
import { kbd } from "../platform";
import { revealPath } from "../api";
import claudeLogo from "../assets/brand/claude.svg";
import geminiLogo from "../assets/brand/gemini.svg";
import openaiLogo from "../assets/brand/openai.svg";
import opencodeLogo from "../assets/brand/opencode.svg";
import grokLogo from "../assets/brand/grok.svg";
import {
  GripVertical,
  Folder,
  FolderOpen,
  LayoutGrid,
  Settings as SettingsIcon,
  Plus,
  X,
  Search,
  ChevronLeft,
  ChevronRight,
  Repeat,
  Copy,
  Server,
  Pencil,
} from "lucide-react";

// provider brand marks shown on agent session rows (Codex = OpenAI), like Orca surfaces the agent
const SESS_LOGO: Record<string, string> = {
  claude: claudeLogo,
  codex: openaiLogo,
  gemini: geminiLogo,
  opencode: opencodeLogo,
  grok: grokLogo,
};

const wsAt = (x: number, y: number): string | null => {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".rail-item")?.dataset.wsid ?? null;
};

// the part of a session's cwd below its project folder (e.g. "spookeypumpkin27"), "" if it's the root
function relSub(wsCwd: string, sessCwd?: string): string {
  if (!sessCwd || !wsCwd) return "";
  const a = wsCwd.replace(/[\\/]+$/, "").toLowerCase();
  const b = sessCwd.replace(/[\\/]+$/, "");
  const bl = b.toLowerCase();
  if (bl === a) return "";
  if (bl.startsWith(a + "\\") || bl.startsWith(a + "/")) return b.slice(a.length + 1);
  return b.split(/[\\/]/).pop() || ""; // not under the project — just its name
}

// one session row — subscribes to just ITS activity primitives, so a chatty pane re-renders only
// this row (not the whole rail), and owns the busy→ready decay timer + relTime refresh itself
function SessionRow({
  sess,
  active,
  sub,
  onFocus,
}: {
  sess: Workspace["sessions"][number];
  active: boolean;
  sub: string;
  onFocus: () => void;
}) {
  const isExited = useActivity((s) => !!s.exited[sess.id]);
  const lastOut = useActivity((s) => s.lastOut[sess.id]);
  const [, tick] = useReducer((x: number) => x + 1, 0);

  // re-render right after the 1.5s busy window lapses so the dot decays busy → ready
  useEffect(() => {
    if (!lastOut) return;
    const left = lastOut + 1500 - Date.now();
    if (left <= 0) return;
    const t = setTimeout(tick, left + 50);
    return () => clearTimeout(t);
  }, [lastOut]);

  // keep the relTime label fresh — a slow tick, and only while the app is actually visible
  const hasOut = !!lastOut;
  useEffect(() => {
    if (!hasOut) return;
    let iv: ReturnType<typeof setInterval> | undefined;
    const arm = () => {
      if (document.visibilityState === "visible") {
        if (!iv) iv = setInterval(tick, 30_000);
      } else if (iv) {
        clearInterval(iv);
        iv = undefined;
      }
    };
    arm();
    const onVis = () => {
      arm();
      if (document.visibilityState === "visible") tick(); // catch up after being hidden
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      if (iv) clearInterval(iv);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, [hasOut]);

  const dot = isExited ? "exited" : lastOut && Date.now() - lastOut < 1500 ? "busy" : "ready";
  return (
    <button className={`rail-session${active ? " active" : ""}`} title={sess.cwd || sess.title} onClick={onFocus}>
      <span className="rail-sess-ico">
        {SESS_LOGO[sess.provider] && <img className="rail-sess-logo" src={SESS_LOGO[sess.provider]} alt="" />}
        <span className={`rail-sess-dot s-${dot}${SESS_LOGO[sess.provider] ? " badge" : ""}`} />
      </span>
      <span className="rail-sess-name">{sess.title}</span>
      {sub && (
        <span className="rail-sess-sub" title={sess.cwd}>
          {sub}
        </span>
      )}
      {lastOut ? <span className="rail-sess-time">{relTime(lastOut)}</span> : null}
    </button>
  );
}

export function Rail() {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const setActive = useWorkspaces((s) => s.setActive);
  const addOpenSpace = useWorkspaces((s) => s.addOpenSpace);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const renameWorkspace = useWorkspaces((s) => s.renameWorkspace);
  const reorderWorkspaces = useWorkspaces((s) => s.reorderWorkspaces);
  const collapsed = useUi((s) => s.railCollapsed);
  const toggleRail = useUi((s) => s.toggleRail);
  const paneDragging = useUi((s) => s.paneDragging);
  const paneDragOverWs = useUi((s) => s.paneDragOverWs);
  const view = useUi((s) => s.view);
  const goSpace = useUi((s) => s.goSpace);
  // loops live entirely on the Loops page now — the rail only surfaces a count on the nav item:
  // how many are running (highlighted) or, when idle, how many exist (muted). primitive selectors
  // so a run's streamed output never re-renders the rail
  const activeLoops = useLoops((s) => {
    let n = 0;
    for (const r of Object.values(s.runs)) if (r.status === "running" || r.status === "paused") n++;
    return n;
  });
  const loopCount = useLoops((s) => Object.keys(s.loops).length);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);

  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [filter, setFilter] = useState(""); // live sidebar filter (Orca-style Search box)
  const drag = useRef<{ id: string; sx: number; sy: number; active: boolean; over: string | null } | null>(null);
  // drag feedback is toggled imperatively so a pointermove never re-renders (and flickers) the whole
  // rail — dropping is the only thing that hits React state. re-applied after any unrelated re-render.
  const applyDrag = () => {
    for (const el of document.querySelectorAll(".rail-item.dragging, .rail-item.drop-over"))
      el.classList.remove("dragging", "drop-over");
    const d = drag.current;
    if (!d?.active) return;
    document.querySelector(`.rail-item[data-wsid="${d.id}"]`)?.classList.add("dragging");
    if (d.over) document.querySelector(`.rail-item[data-wsid="${d.over}"]`)?.classList.add("drop-over");
  };
  // a re-render mid-drag (a store update, a workspace change) would wipe the classes React
  // doesn't know about — re-apply here, before paint, so it never flickers.
  useLayoutEffect(() => {
    if (drag.current?.active) applyDrag();
  });
  // a wrap's sub-content (sessions / file tree) is mounted once it's first expanded and kept mounted
  // after — so collapsing/expanding the whole sidebar never remounts (and refetches) it. the .rail-sub
  // grid-rows reveal hides it when not open. readyTrees = projects whose file listing has loaded, so
  // we only reveal once the height is known (no Loading→pop).
  const [mountedSubs, setMountedSubs] = useState<Set<string>>(new Set());

  // smooth list add/remove/reorder (the two rail lists), T3-style. the per-item expand reveal is CSS
  // (.rail-sub grid-rows) so it tolerates the async file tree and doesn't fight the width transition.
  const [projRef] = useAutoAnimate();
  const [spaceRef] = useAutoAnimate();

  // filter matches a space by name or any of its session titles
  const q = filter.trim().toLowerCase();
  const matchesFilter = (w: Workspace) =>
    !q || w.name.toLowerCase().includes(q) || w.sessions.some((s) => s.title.toLowerCase().includes(q));
  const projects = workspaces.filter((w) => w.kind !== "open").filter(matchesFilter);
  const openSpaces = workspaces.filter((w) => w.kind === "open").filter(matchesFilter);

  // expand/collapse a wrap; on first expand, mount its sub-content (kept mounted after).
  // files live in the dock's Files panel now — the sidebar only expands to its sessions.
  const expandSub = (w: Workspace) => {
    const willOpen = !expanded.has(w.id);
    toggleExpand(w.id);
    if (willOpen) setMountedSubs((p) => (p.has(w.id) ? p : new Set(p).add(w.id)));
  };

  // auto-expand the active OPEN SPACE so its session list shows. projects aren't auto-expanded —
  // their chevron opens a file tree, and dumping that on every click is annoying (toggle it yourself).
  // (dot decay / relTime refresh live in each SessionRow now — no rail-wide tick.)
  useEffect(() => {
    if (!activeId) return;
    const ws = useWorkspaces.getState().workspaces.find((w) => w.id === activeId);
    if (ws?.kind === "open") {
      setExpanded((p) => (p.has(activeId) ? p : new Set(p).add(activeId)));
      setMountedSubs((p) => (p.has(activeId) ? p : new Set(p).add(activeId)));
    }
  }, [activeId]);

  const toggleExpand = (id: string) =>
    setExpanded((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const focusSession = (wid: string, sid: string) => {
    setActive(wid);
    setFocused(sid);
    goSpace();
  };

  const item = (w: Workspace) => {
    const isExpanded = expanded.has(w.id);
    const hasSessions = w.sessions.length > 0;
    const canExpand = hasSessions; // sessions only — files belong to the dock's Files panel
    return (
      <div key={w.id} className="rail-item-wrap" data-wsid={w.id}>
        <div
          data-wsid={w.id}
          className={`rail-item ${w.id === activeId && view === "space" ? "active" : ""}${
            paneDragging && w.id !== activeId ? " pane-droppable" : ""
          }${paneDragOverWs === w.id ? " pane-drop-over" : ""}`}
          title={w.cwd || w.name}
          onClick={() => {
            setActive(w.id);
            goSpace();
          }}
          onDoubleClick={() => setEditing(w.id)}
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setMenu({ x: e.clientX, y: e.clientY, id: w.id });
          }}
        >
          {canExpand ? (
            <button
              className={`rail-twist${isExpanded ? " open" : ""}`}
              title={isExpanded ? "Collapse" : "Expand"}
              onClick={(e) => {
                e.stopPropagation();
                expandSub(w);
              }}
            >
              <ChevronRight size={13} />
            </button>
          ) : (
            <span className="rail-twist-spacer" />
          )}
          <span
            className="rail-grip"
            title="Drag to reorder"
            onClick={(e) => e.stopPropagation()}
            onPointerDown={(e) => {
              e.stopPropagation();
              e.currentTarget.setPointerCapture?.(e.pointerId);
              drag.current = { id: w.id, sx: e.clientX, sy: e.clientY, active: false, over: null };
            }}
            onPointerMove={(e) => {
              const d = drag.current;
              if (!d) return;
              if (!d.active) {
                if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 4) return;
                d.active = true;
                applyDrag(); // mark the picked-up row
              }
              const t = wsAt(e.clientX, e.clientY);
              const over = t && t !== d.id ? t : null;
              if (over !== d.over) {
                d.over = over;
                applyDrag(); // move the drop indicator — no setState, no re-render
              }
            }}
            onPointerUp={(e) => {
              const d = drag.current;
              drag.current = null;
              e.currentTarget.releasePointerCapture?.(e.pointerId);
              applyDrag(); // clears the drag classes
              if (d?.active && d.over && d.over !== d.id) reorderWorkspaces(d.id, d.over);
            }}
          >
            <GripVertical size={13} aria-hidden="true" />
          </span>
          {w.kind === "open" ? (
            <LayoutGrid className="rail-ico" size={14} style={{ color: w.color }} />
          ) : (
            <Folder className="rail-ico" size={14} style={{ color: w.color }} />
          )}
          {editing === w.id ? (
            <input
              className="rail-rename"
              autoFocus
              defaultValue={w.name}
              onClick={(e) => e.stopPropagation()}
              onBlur={(e) => {
                const v = e.currentTarget.value.trim();
                if (v) renameWorkspace(w.id, v);
                setEditing(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
                if (e.key === "Escape") setEditing(null);
              }}
            />
          ) : (
            <span className="rail-name">{w.name}</span>
          )}
          {hasSessions && <span className="rail-count">{w.sessions.length}</span>}
          <button
            className="rail-clear"
            title={`Clear "${w.name}" (closes its sessions — your files stay)`}
            onClick={(e) => {
              e.stopPropagation();
              removeWorkspace(w.id);
            }}
          >
            <X size={12} />
          </button>
        </div>
        {mountedSubs.has(w.id) && (
          <div className={`rail-sub${isExpanded ? " open" : ""}`}>
            <div className="rail-sub-inner">
              {hasSessions && (
                <div className="rail-sessions">
                  {w.sessions.map((s) => (
                    <SessionRow
                      key={s.id}
                      sess={s}
                      active={view === "space" && w.id === activeId && focusedSessionId === s.id}
                      sub={relSub(w.cwd, s.cwd)} // subfolder it's running in, if any
                      onFocus={() => focusSession(w.id, s.id)}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    );
  };

  return (
    <div className={`rail${collapsed ? " collapsed" : ""}`}>
      <button
        className="rail-edge"
        title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
        onClick={toggleRail}
      >
        {collapsed ? <ChevronRight size={15} /> : <ChevronLeft size={15} />}
      </button>
      <div className="rail-scroll">
      {collapsed ? (
        <button className="rail-search" onClick={() => useUi.getState().setPalette(true)}>
          <Search size={15} />
        </button>
      ) : (
        <div className="rail-search-box">
          <Search size={14} />
          <input
            className="rail-search-input"
            placeholder="Search"
            value={filter}
            spellCheck={false}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setFilter("");
            }}
          />
          {filter ? (
            <button className="rail-search-clear" title="Clear" onClick={() => setFilter("")}>
              <X size={12} />
            </button>
          ) : (
            <button
              className="rail-search-kbd"
              title="Search everything (command palette)"
              onClick={() => useUi.getState().setPalette(true)}
            >
              {kbd("Ctrl K")}
            </button>
          )}
        </div>
      )}
      <button
        className={`rail-nav${view === "loops" ? " active" : ""}`}
        title="Automations"
        onClick={() => useUi.getState().goLoops()}
      >
        <Repeat size={15} />
        <span className="rail-nav-label">Automations</span>
        {activeLoops > 0 ? (
          <span className="rail-nav-badge" title={`${activeLoops} running`}>{activeLoops}</span>
        ) : loopCount > 0 ? (
          <span className="rail-nav-badge muted" title={`${loopCount} automation${loopCount > 1 ? "s" : ""}`}>
            {loopCount}
          </span>
        ) : null}
      </button>

      <div className="rail-header">
        <span className="rail-title">Projects</span>
        <button
          className="rail-add"
          title="New project"
          onClick={() => useUi.getState().openNewProject()}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="rail-list" ref={projRef}>{projects.map(item)}</div>

      <div className="rail-header">
        <span className="rail-title">Open spaces</span>
        <button
          className="rail-add"
          title="New open space"
          onClick={() => {
            addOpenSpace();
            goSpace();
          }}
        >
          <Plus size={15} />
        </button>
      </div>
      <div className="rail-list" ref={spaceRef}>
        {openSpaces.map(item)}
        {openSpaces.length === 0 && <div className="rail-empty">launch sessions in any folder</div>}
      </div>
      </div>

      <div className="rail-foot">
        <button
          className="rail-settings"
          title="Settings"
          onClick={() => useUi.getState().openSettings()}
        >
          <SettingsIcon size={16} strokeWidth={1.75} />
          <span className="rail-settings-label">Settings</span>
        </button>
      </div>

      {menu &&
        (() => {
          const w = workspaces.find((x) => x.id === menu.id);
          const cwd = w?.cwd ?? "";
          return (
            <>
              <div
                className="ctx-backdrop"
                onClick={() => setMenu(null)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  setMenu(null);
                }}
              />
              <div className="ctx-menu" style={{ left: menu.x, top: menu.y }}>
                {cwd && (
                  <>
                    <button
                      className="ctx-item"
                      onClick={() => {
                        void revealPath(cwd).catch(() => {});
                        setMenu(null);
                      }}
                    >
                      <FolderOpen size={14} />
                      <span>Open folder</span>
                    </button>
                    <button
                      className="ctx-item"
                      onClick={() => {
                        void navigator.clipboard.writeText(cwd).catch(() => {});
                        setMenu(null);
                      }}
                    >
                      <Copy size={14} />
                      <span>Copy path</span>
                    </button>
                    <div className="ctx-sep" />
                    <button
                      className="ctx-item"
                      onClick={() => {
                        if (w) useUi.getState().openServices({ folder: cwd, wsId: w.id, name: w.name });
                        setMenu(null);
                      }}
                    >
                      <Server size={14} />
                      <span>Services</span>
                    </button>
                  </>
                )}
                <button
                  className="ctx-item"
                  onClick={() => {
                    setEditing(menu.id);
                    setMenu(null);
                  }}
                >
                  <Pencil size={14} />
                  <span>Rename</span>
                </button>
              </div>
            </>
          );
        })()}
    </div>
  );
}
