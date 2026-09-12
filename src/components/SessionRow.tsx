import { createElement, useEffect, useReducer, useState } from "react";
import { Check, Folder, GitBranch, GitCompare, GitFork, Image as ImageIcon, PenLine, X } from "lucide-react";
import type { Workspace } from "../stores/workspace";
import { useActivity } from "../stores/activity";
import { useAgentStatus, displayState, type AgentState } from "../stores/agentStatus";
import { useUsage } from "../stores/usage";
import { elapsed, relTime } from "../lib/time";
import { branchOf } from "../lib/branches";
import { fileIcon } from "../lib/fileIcons";
import { PROVIDER_LOGO, PROVIDER_NAME } from "../lib/brand";
import { modelLabel, type ProviderId } from "../lib/models";
import { heuristicState } from "../lib/agentHeuristics";
import { closeSession } from "../actions";
import { gitChanges } from "../api";

type RowState = AgentState | "exited";

// Last line of defence for a "working" row whose CLI is gone: it exited back to the shell, or was
// killed, so its Stop will never arrive. Note this can only catch a DEAD cli, not an idle one —
// claude paints its own cursor, so an idle claude pane still writes to the pty twice a second and
// never looks quiet. A conversation reset is caught by the SessionStart hook instead.
const QUIET_MS = 30_000;

// the part of a thread's cwd below its space's folder, "" if it is the root
function relSub(wsCwd: string, sessCwd?: string): string {
  if (!sessCwd || !wsCwd) return "";
  const a = wsCwd.replace(/[\\/]+$/, "").toLowerCase();
  const b = sessCwd.replace(/[\\/]+$/, "");
  const bl = b.toLowerCase();
  if (bl === a) return "";
  if (bl.startsWith(a + "\\") || bl.startsWith(a + "/")) return b.slice(a.length + 1);
  return b.split(/[\\/]/).pop() || "";
}

/**
 * One thread in the sidebar: agent and time, title, then what it is doing or where it runs, with
 * a live state glyph. Subscribes to its own activity so a chatty pane re-renders only itself.
 */
export function SessionRow({
  ws,
  sess,
  active,
  onFocus,
}: {
  ws: Workspace;
  sess: Workspace["sessions"][number];
  active: boolean;
  onFocus: () => void;
}) {
  const isExited = useActivity((s) => !!s.exited[sess.id]);
  const lastOut = useActivity((s) => s.lastOut[sess.id]);
  const liveModel = useUsage((s) => s.byPane[sess.id]?.model);
  const agent = useAgentStatus((s) => s.byPane[sess.id]);
  const [, tick] = useReducer((x: number) => x + 1, 0);

  const now = Date.now();
  // Claude reports through hooks. Everything else is judged from the terminal output.
  let state: RowState;
  let activity: string | undefined;
  if (sess.draft) {
    state = "idle";
    activity = "Not started";
  } else if (agent) {
    state = isExited ? "exited" : displayState(agent, now);
    activity = agent.activity;
    // Don't take "working" on trust when the pty has gone silent — see QUIET_MS. Measured from
    // whichever is later, the last output or the moment the state was set, so a turn that has only
    // just started is never cut off early. It re-arms on its own: new output makes it working again.
    if (state === "working" && now - Math.max(lastOut ?? 0, agent.since) > QUIET_MS) {
      state = "idle";
      activity = undefined;
    }
  } else {
    const h = heuristicState(sess.id, lastOut, isExited, now);
    state = h.state;
    activity = h.activity;
  }

  // Only a pane reporting through claude's hooks knows when it started working, so only those can
  // count up. Everything else keeps the relative time, which is all we honestly have for them.
  const runningSince = state === "working" && agent ? agent.since : undefined;

  // re-render once the busy window lapses so the state can settle to idle or waiting
  useEffect(() => {
    if (!lastOut) return;
    const left = lastOut + 2600 - Date.now();
    if (left <= 0) return;
    const t = setTimeout(tick, left + 50);
    return () => clearTimeout(t);
  }, [lastOut]);

  // the counter has to move every second, but only while this row is counting and the window is
  // on screen — a hidden window ticking once a second per working thread is pure waste
  const counting = runningSince !== undefined;
  useEffect(() => {
    if (!counting) return;
    let iv: ReturnType<typeof setInterval> | undefined;
    const arm = () => {
      if (document.visibilityState === "visible") {
        if (!iv) iv = setInterval(tick, 1000);
      } else if (iv) {
        clearInterval(iv);
        iv = undefined;
      }
    };
    arm();
    const onVis = () => {
      arm();
      if (document.visibilityState === "visible") tick();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      if (iv) clearInterval(iv);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, [counting]);

  // keep the relative time fresh, only while the window is visible
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
      if (document.visibilityState === "visible") tick();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => {
      if (iv) clearInterval(iv);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, [hasOut]);

  const isDoc = !!(sess.image || sess.file || sess.media || sess.diff);
  const docIcon = sess.image ? ImageIcon : sess.diff ? GitCompare : fileIcon(sess.title || sess.file || sess.media || "");
  const logo = PROVIDER_LOGO[sess.provider];
  const cwd = sess.cwd || ws.cwd;
  // A branch when the folder is a repo, otherwise just where the thread runs. The icon has to say
  // which of the two it is: the folder-name fallback under a branch icon read as a branch named
  // after the project, on folders that are not repos at all.
  const branch = cwd ? branchOf(cwd) : undefined;
  const where = [branch, relSub(ws.cwd, sess.cwd)].filter(Boolean).join("/") || (cwd ? cwd.split(/[\\/]/).filter(Boolean).pop() : "no folder");
  const label = sess.draft
    ? "Composer"
    : liveModel || (sess.model ? modelLabel(sess.provider as ProviderId, sess.model) : PROVIDER_NAME[sess.provider] || sess.provider);

  if (isDoc) {
    return (
      <button
        className={`sess-row doc${active ? " active" : ""}`}
        data-sid={sess.id}
        data-wsid={ws.id}
        title={sess.image || sess.file || sess.media || sess.diff}
        onClick={onFocus}
      >
        {createElement(docIcon, { size: 13, className: "sess-doc-ico" })}
        <span className="sess-name">{sess.title}</span>
        <span
          className="sess-close"
          role="button"
          title="Close"
          onClick={(e) => {
            e.stopPropagation();
            void closeSession(ws.id, sess.id);
          }}
        >
          <X size={12} />
        </span>
      </button>
    );
  }
  return (
    <button
      className={`sess-row st-${state}${active ? " active" : ""}`}
      data-sid={sess.id}
      data-wsid={ws.id}
      title={sess.cwd || sess.title}
      onClick={onFocus}
    >
      <span className="sess-top">
        <span className="sess-mark">
          {sess.draft ? <PenLine size={12} /> : logo ? <img src={logo} alt="" /> : <span className="sess-mark-term" />}
        </span>
        <span className="sess-prov">{label}</span>
        {runningSince !== undefined ? (
          <span className="sess-elapsed">{elapsed(runningSince, now)}</span>
        ) : lastOut ? (
          <span className="sess-time">{relTime(lastOut)}</span>
        ) : null}
      </span>
      <span className="sess-name">{sess.title}</span>
      <span className="sess-foot">
        {activity ? (
          <span className="sess-act" title={activity}>
            {activity}
          </span>
        ) : (
          <span className="sess-where" title={cwd}>
            {branch ? <GitBranch size={11} /> : <Folder size={11} />}
            <span>{where}</span>
          </span>
        )}
        <span className="sess-state" title={state}>
          {state === "done" && <Check size={9} strokeWidth={3} />}
        </span>
      </span>
      {agent && agent.subs.length > 0 && (
        <span className="sess-subs">
          {agent.subs.map((sub) => (
            <span key={sub.id} className={`sess-sub ${sub.state}`} title={sub.label}>
              <span className="sess-sub-mark">{logo ? <img src={logo} alt="" /> : <GitFork size={9} />}</span>
              <span className="sess-sub-label">{sub.label}</span>
              <span className="sess-sub-time">{relTime(sub.startedAt)}</span>
            </span>
          ))}
        </span>
      )}
      <span
        className="sess-close"
        role="button"
        title="Close"
        onClick={(e) => {
          e.stopPropagation();
          void closeSession(ws.id, sess.id);
        }}
      >
        <X size={12} />
      </span>
    </button>
  );
}

/**
 * "3 files · +12 −4" for a folder.
 *
 * It reads once as soon as it knows the folder, even for a space that is folded shut. That is what
 * keeps the fold smooth: git takes a moment to answer, so a summary that only started loading when
 * the space opened would land after the fold had already settled and shove the threads down a
 * second time. Polling, which is the expensive part, still only runs while the space is open.
 */
export function useDiffSummary(cwd: string, active = true) {
  const [sum, setSum] = useState<{ files: number; added: number; removed: number } | null>(null);
  useEffect(() => {
    if (!cwd) return;
    let alive = true;
    const tick = () => {
      if (document.hidden) return;
      gitChanges(cwd)
        .then((files) => {
          if (!alive) return;
          if (!files.length) return setSum(null);
          setSum({
            files: files.length,
            added: files.reduce((n, f) => n + f.added, 0),
            removed: files.reduce((n, f) => n + f.removed, 0),
          });
        })
        .catch(() => alive && setSum(null));
    };
    tick();
    const id = active ? setInterval(tick, 20_000) : undefined;
    return () => {
      alive = false;
      if (id) clearInterval(id);
    };
  }, [cwd, active]);
  return cwd ? sum : null;
}
