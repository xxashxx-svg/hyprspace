import { useEffect, useReducer, useRef, useState } from "react";
import type { MouseEvent as RMouseEvent, PointerEvent as RPointerEvent } from "react";
import { useAutoAnimate } from "@formkit/auto-animate/react";
import { Archive, ArchiveRestore, ChevronRight, Copy, FolderOpen, FolderPlus, Pencil, Plus, Search, Settings as SettingsIcon, SquarePen, Trash2, X } from "lucide-react";
import { useUi } from "../stores/ui";
import { useSettings } from "../stores/settings";
import { useWorkspaces } from "../stores/workspace";
import { kbd } from "../platform";
import { pickFolder, revealPath } from "../api";
import { newSession } from "../actions";
import { onBranchResolved } from "../lib/branches";
import { SessionRow, useDiffSummary } from "./SessionRow";

/**
 * The working tree line under an open space: file count and line deltas. A clean tree says nothing
 * rather than repeating "clean" down the whole sidebar — the absence of the line is the message.
 */
function SpaceSummary({ cwd, active }: { cwd: string; active: boolean }) {
  const diff = useDiffSummary(cwd, active);
  if (!cwd) return <div className="space-sum">No folder yet</div>;
  if (!diff) return null;
  return (
    <div className="space-sum">
      <span>
        {diff.files} {diff.files === 1 ? "file" : "files"}
      </span>
      <span className="space-sum-add">+{diff.added}</span>
      <span className="space-sum-del">−{diff.removed}</span>
    </div>
  );
}

/**
 * The sidebar: search, then every space as a section that folds open to show its threads, and
 * settings at the bottom. The active space is open by default. Threads drag to reorder, and
 * dropping one on another space (its header or its rows) moves it there. The right edge resizes.
 */
export function Rail() {
  const collapsed = useUi((s) => s.railCollapsed);
  const view = useUi((s) => s.view);
  const workspaces = useWorkspaces((s) => s.workspaces);
  const activeId = useWorkspaces((s) => s.activeId);
  const focusedSessionId = useWorkspaces((s) => s.focusedSessionId);
  const setActive = useWorkspaces((s) => s.setActive);
  const setFocused = useWorkspaces((s) => s.setFocused);
  const addWorkspace = useWorkspaces((s) => s.addWorkspace);
  const removeWorkspace = useWorkspaces((s) => s.removeWorkspace);
  const renameWorkspace = useWorkspaces((s) => s.renameWorkspace);
  const setArchived = useWorkspaces((s) => s.setArchived);
  const reorderSessions = useWorkspaces((s) => s.reorderSessions);
  const reorderWorkspaces = useWorkspaces((s) => s.reorderWorkspaces);
  const moveSessionToWorkspace = useWorkspaces((s) => s.moveSessionToWorkspace);
  const paneDragging = useUi((s) => s.paneDragging);
  const paneDragOverWs = useUi((s) => s.paneDragOverWs);
  const width = useSettings((s) => s.railWidth);

  const [filter, setFilter] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  // spaces opened or folded by hand (true = open), remembered with the space that was active at
  // the time so the active space unfolds again whenever it changes
  const [folds, setFolds] = useState<{ at: string | null; ids: Map<string, boolean> }>({ at: null, ids: new Map() });
  const byHand = folds.at === activeId ? folds.ids : new Map([...folds.ids].filter(([id]) => id !== activeId));
  // every space that has been active stays open until folded by hand, so switching spaces never
  // snaps the one you just left shut under the pointer
  const [seen, setSeen] = useState<Set<string>>(() => new Set(activeId ? [activeId] : []));
  if (activeId && !seen.has(activeId)) setSeen(new Set(seen).add(activeId));
  const [listRef] = useAutoAnimate();

  const [, bump] = useReducer((x: number) => x + 1, 0);
  useEffect(() => onBranchResolved(bump), []);

  const q = filter.trim().toLowerCase();
  const shown = workspaces
    .map((w) => ({ w, threads: w.sessions.filter((s) => !q || s.title.toLowerCase().includes(q)) }))
    .filter(({ w, threads }) => !q || threads.length > 0 || w.name.toLowerCase().includes(q));
  const live = shown.filter(({ w }) => !w.archived);
  const archived = shown.filter(({ w }) => w.archived);
  // the archived group stays folded unless opened, or unless the space you are in lives there
  const [archOpen, setArchOpen] = useState(false);
  const archShown = archOpen || archived.some(({ w }) => w.id === activeId);

  const open = (id: string) => {
    setActive(id);
    if (view !== "home") useUi.getState().goSpace();
  };
  const toggleFold = (id: string, open: boolean) => {
    const ids = new Map(byHand);
    ids.set(id, !open);
    setFolds({ at: activeId, ids });
  };
  const focus = (wid: string, sid: string) => {
    if (wid !== activeId) setActive(wid);
    setFocused(sid);
    useUi.getState().goSpace();
  };
  const openFolder = async () => {
    const folder = await pickFolder();
    if (!folder) return;
    addWorkspace(folder.split(/[\\/]/).filter(Boolean).pop() || "Project", folder);
    if (view !== "home") useUi.getState().goSpace();
  };

  // Dragging, done imperatively so a pointermove never re-renders the list. A thread drags to
  // reorder within its space or onto another space; a space header drags to reorder the spaces.
  // A drag starts after a few pixels of travel; the click that would follow a drop is swallowed.
  const drag = useRef<{
    kind: "thread" | "space";
    sid: string;
    wsId: string;
    el: HTMLElement;
    sx: number;
    sy: number;
    on: boolean;
    over: HTMLElement | null;
    after: boolean; // space drags: land below `over` instead of above it
  } | null>(null);
  // the live space headers in sidebar order, minus the one being dragged
  const spaceHeads = (self: HTMLElement) =>
    Array.from(document.querySelectorAll<HTMLElement>(".space-list .space-row:not(.archive-row)")).filter((r) => r !== self);
  const targetAt = (x: number, y: number, kind: "thread" | "space") =>
    (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>(
      kind === "thread" ? ".sess-row, .space-row" : ".space-row:not(.archive-row)",
    ) ?? null;
  const onPointerDown = (e: RPointerEvent) => {
    if (e.button !== 0) return;
    const t = e.target as HTMLElement;
    if (t.closest(".sess-close")) return;
    const row = t.closest<HTMLElement>(".sess-row");
    if (row?.dataset.sid && row.dataset.wsid) {
      drag.current = { kind: "thread", sid: row.dataset.sid, wsId: row.dataset.wsid, el: row, sx: e.clientX, sy: e.clientY, on: false, over: null, after: false };
      return;
    }
    const head = t.closest<HTMLElement>(".space-row");
    if (!head || head.classList.contains("archive-row") || !head.dataset.wsid || t.closest("button")) return;
    drag.current = { kind: "space", sid: "", wsId: head.dataset.wsid, el: head, sx: e.clientX, sy: e.clientY, on: false, over: null, after: false };
  };
  const onPointerMove = (e: RPointerEvent) => {
    const d = drag.current;
    if (!d) return;
    if (!d.on) {
      if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 5) return;
      d.on = true;
      d.el.classList.add("dragging");
      d.el.closest(".space-list")?.classList.add("dragging");
      d.el.setPointerCapture?.(e.pointerId);
    }
    const over = targetAt(e.clientX, e.clientY, d.kind);
    let next = over && over !== d.el ? over : null;
    let after = false;
    if (d.kind === "space") {
      if (next) {
        // upper half lands above the header, lower half below it
        const r = next.getBoundingClientRect();
        after = e.clientY > r.top + r.height / 2;
      } else {
        // past the last header: land at the end of the list
        const heads = spaceHeads(d.el);
        const last = heads[heads.length - 1];
        if (last && e.clientY > last.getBoundingClientRect().bottom) {
          next = last;
          after = true;
        }
      }
    }
    if (next !== d.over || after !== d.after) {
      d.over?.classList.remove("drop-over", "drop-here", "drop-after");
      next?.classList.add(d.kind === "space" ? (after ? "drop-after" : "drop-here") : "drop-over");
      d.over = next;
      d.after = after;
    }
  };
  const endDrag = (drop: boolean) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    d.el.classList.remove("dragging");
    d.over?.classList.remove("drop-over", "drop-here", "drop-after");
    d.el.closest(".space-list")?.classList.remove("dragging");
    if (!d.on) return;
    d.el.dataset.justDragged = "1";
    const tWs = d.over?.dataset.wsid;
    const tSid = d.over?.dataset.sid;
    if (!drop || !tWs || !d.over) return;
    if (d.kind === "space") {
      if (!d.after) {
        if (tWs !== d.wsId) reorderWorkspaces(d.wsId, tWs);
        return;
      }
      // below a header means in front of the next one, or last when there is none
      const heads = spaceHeads(d.el);
      const following = heads[heads.indexOf(d.over) + 1];
      reorderWorkspaces(d.wsId, following?.dataset.wsid ?? null);
    } else if (tWs !== d.wsId) moveSessionToWorkspace(d.wsId, d.sid, tWs);
    else if (tSid) reorderSessions(d.wsId, d.sid, tSid);
  };
  const onClickCapture = (e: RMouseEvent) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>(".sess-row, .space-row");
    if (el?.dataset.justDragged) {
      delete el.dataset.justDragged;
      e.stopPropagation();
      e.preventDefault();
    }
  };

  const resize = useRef<{ x: number; w: number } | null>(null);
  const onResizeDown = (e: RPointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    resize.current = { x: e.clientX, w: width };
  };
  const onResizeMove = (e: RPointerEvent<HTMLDivElement>) => {
    if (resize.current) useSettings.getState().setRailWidth(resize.current.w + (e.clientX - resize.current.x));
  };
  const onResizeUp = (e: RPointerEvent<HTMLDivElement>) => {
    resize.current = null;
    e.currentTarget.releasePointerCapture(e.pointerId);
  };

  const menuWs = menu ? workspaces.find((w) => w.id === menu.id) : undefined;

  return (
    <div className={`rail${collapsed ? " hidden" : ""}`} style={{ "--rail-w": `${width}px` } as React.CSSProperties}>
      {/* Starting something is what the sidebar gets used for most, so it leads. New thread opens a
          composer in the space you're in. The folder button beside it adds a whole new space: that's
          what the old "Open new thread" row at the bottom actually did, under the wrong name. */}
      <div className="rail-top">
        <button className="rail-new" title={`New thread (${kbd("Ctrl+Shift+N")})`} onClick={newSession}>
          <SquarePen size={14} />
          <span className="rail-new-label">New thread</span>
          {/* the shortcut only fits once the rail is wide enough; at its 200px minimum it would
              crowd the label, so it waits for the room rather than getting clipped */}
          {width >= 260 && <span className="rail-new-kbd">{kbd("Ctrl Shift N")}</span>}
        </button>
        <button className="rail-folder" title="Open a folder as a new space" onClick={() => void openFolder()}>
          <FolderPlus size={15} />
        </button>
      </div>

      <div className="rail-search-box">
        <Search size={14} />
        <input
          className="rail-search-input"
          placeholder="Search threads"
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
          <button className="rail-search-kbd" title="Search everything (command palette)" onClick={() => useUi.getState().setPalette(true)}>
            {kbd("Ctrl K")}
          </button>
        )}
      </div>

      <div className="rail-scroll">
        <div
          className="space-list"
          ref={listRef}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={() => endDrag(true)}
          onPointerCancel={() => endDrag(false)}
          onClickCapture={onClickCapture}
        >
          {live.map(({ w, threads }) => {
            const isActive = w.id === activeId;
            // a search opens every space with a hit; otherwise by hand, else any space with
            // threads or one that has been active
            const isOpen = q ? threads.length > 0 : (byHand.get(w.id) ?? (threads.length > 0 || seen.has(w.id)));
            return (
              <section key={w.id} className={`space${isActive ? " active" : ""}${isOpen ? " open" : ""}${threads.length === 0 ? " empty" : ""}`}>
                {editing === w.id ? (
                  <input
                    className="space-rename"
                    autoFocus
                    defaultValue={w.name}
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
                  <div
                    className={`space-row${paneDragging && !isActive ? " droppable" : ""}${paneDragOverWs === w.id ? " drop-over" : ""}`}
                    data-wsid={w.id}
                    title={w.cwd || "Asks for a folder"}
                    onClick={() => open(w.id)}
                    onContextMenu={(e) => {
                      e.preventDefault();
                      setMenu({ x: e.clientX, y: e.clientY, id: w.id });
                    }}
                  >
                    <button
                      className={`space-twist${isOpen ? " open" : ""}`}
                      title={isOpen ? "Fold" : "Unfold"}
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleFold(w.id, isOpen);
                      }}
                    >
                      <ChevronRight size={12} />
                    </button>
                    <span className="space-name">{w.name}</span>
                    <button
                      className="space-add"
                      title="New thread here"
                      onClick={(e) => {
                        e.stopPropagation();
                        setActive(w.id);
                        newSession();
                      }}
                    >
                      <Plus size={13} />
                    </button>
                    <button
                      className="space-add"
                      title="Archive this space"
                      onClick={(e) => {
                        e.stopPropagation();
                        setArchived(w.id, true);
                      }}
                    >
                      <Archive size={13} />
                    </button>
                  </div>
                )}
                {/* always mounted so the fold can animate; the summary only polls while open */}
                <div className="space-fold">
                  <div className="space-fold-in">
                    <div
                      className="space-body"
                      // the status line and the empty note belong to the space too: a click there
                      // opens it, while thread rows keep handling their own clicks
                      onClick={(e) => {
                        if (!(e.target as HTMLElement).closest(".sess-row")) open(w.id);
                      }}
                      onContextMenu={(e) => {
                        if ((e.target as HTMLElement).closest(".sess-row")) return;
                        e.preventDefault();
                        setMenu({ x: e.clientX, y: e.clientY, id: w.id });
                      }}
                    >
                      {/* mounted whether the space is folded or not, so its height is part of the
                          fold from the first frame. Gated on `isOpen` it loaded late and pushed the
                          threads down once the fold had already finished. */}
                      <SpaceSummary cwd={w.cwd} active={isOpen} />
                      {threads.map((s) => (
                        <SessionRow
                          key={s.id}
                          ws={w}
                          sess={s}
                          active={view === "space" && isActive && focusedSessionId === s.id}
                          onFocus={() => focus(w.id, s.id)}
                        />
                      ))}
                      {threads.length === 0 && <div className="space-empty">No threads yet</div>}
                    </div>
                  </div>
                </div>
              </section>
            );
          })}
          {live.length === 0 && archived.length === 0 && <div className="rail-empty">{q ? "Nothing matches." : "No threads yet."}</div>}
          {archived.length > 0 && (
            <section className={`space archive${archShown ? " open" : ""}`}>
              <div className="space-row archive-row" onClick={() => setArchOpen(!archShown)}>
                <button className={`space-twist${archShown ? " open" : ""}`} title={archShown ? "Fold" : "Unfold"}>
                  <ChevronRight size={12} />
                </button>
                <Archive size={13} className="archive-ico" />
                <span className="space-name">Archived</span>
                <span className="space-count">{archived.length}</span>
              </div>
              <div className="space-fold">
                <div className="space-fold-in">
                  <div className="space-body">
                    {archived.map(({ w }) => (
                      <div
                        key={w.id}
                        className={`arch-row${w.id === activeId ? " active" : ""}${paneDragOverWs === w.id ? " drop-over" : ""}`}
                        data-wsid={w.id}
                        title={w.cwd}
                        onClick={() => open(w.id)}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          setMenu({ x: e.clientX, y: e.clientY, id: w.id });
                        }}
                      >
                        <span className="arch-name">{w.name}</span>
                        {w.sessions.length > 0 && (
                          <span className="arch-count">
                            {w.sessions.length} {w.sessions.length === 1 ? "thread" : "threads"}
                          </span>
                        )}
                        <button
                          className="arch-restore"
                          onClick={(e) => {
                            e.stopPropagation();
                            setArchived(w.id, false);
                          }}
                        >
                          <ArchiveRestore size={11} />
                          Restore
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </section>
          )}
        </div>
      </div>

      <div className="rail-foot">
        <button className="rail-settings" title="Settings" onClick={() => useUi.getState().openSettings()}>
          <SettingsIcon size={16} strokeWidth={1.75} />
          <span className="rail-settings-label">Settings</span>
        </button>
      </div>
      <div className="rail-resize" onPointerDown={onResizeDown} onPointerMove={onResizeMove} onPointerUp={onResizeUp} />

      {menu && menuWs && (
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
            <div className="ctx-head">{menuWs.name}</div>
            <button
              className="ctx-item"
              onClick={() => {
                setMenu(null);
                setActive(menuWs.id);
                newSession();
              }}
            >
              <Plus size={14} />
              <span>New thread</span>
            </button>
            {menuWs.cwd && (
              <>
                <button
                  className="ctx-item"
                  onClick={() => {
                    void revealPath(menuWs.cwd).catch(() => {});
                    setMenu(null);
                  }}
                >
                  <FolderOpen size={14} />
                  <span>Open folder</span>
                </button>
                <button
                  className="ctx-item"
                  onClick={() => {
                    void navigator.clipboard.writeText(menuWs.cwd).catch(() => {});
                    setMenu(null);
                  }}
                >
                  <Copy size={14} />
                  <span>Copy path</span>
                </button>
              </>
            )}
            <div className="ctx-sep" />
            <button
              className="ctx-item"
              onClick={() => {
                setMenu(null);
                setEditing(menuWs.id);
              }}
            >
              <Pencil size={14} />
              <span>Rename</span>
            </button>
            <button
              className="ctx-item"
              onClick={() => {
                setMenu(null);
                setArchived(menuWs.id, !menuWs.archived);
              }}
            >
              {menuWs.archived ? <ArchiveRestore size={14} /> : <Archive size={14} />}
              <span>{menuWs.archived ? "Restore" : "Archive"}</span>
            </button>
            <button
              className="ctx-item danger"
              onClick={() => {
                setMenu(null);
                removeWorkspace(menuWs.id);
              }}
            >
              <Trash2 size={14} />
              <span>Delete thread</span>
            </button>
          </div>
        </>
      )}
    </div>
  );
}
