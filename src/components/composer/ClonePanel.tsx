import { CircleAlert, FolderOpen, GitBranch } from "lucide-react";
import type { RepoRef } from "../../lib/repoUrl";

export type CloneInto = "new" | "here";
export type CloneOpen = "new" | "here";

interface Props {
  repo: RepoRef;
  parent: string;
  name: string;
  into: CloneInto;
  open: CloneOpen;
  /** whether `parent` has nothing in it; null while unknown. Only an empty folder can take a clone */
  parentEmpty: boolean | null;
  /** false when there's no thread here to open it in */
  canOpenHere: boolean;
  agent: string;
  busy: boolean;
  /** git's latest progress line while it runs */
  line: string | null;
  error: string | null;
  onParent: () => void;
  onName: (name: string) => void;
  onInto: (v: CloneInto) => void;
  onOpen: (v: CloneOpen) => void;
  onClone: () => void;
}

const GITHUB_MARK =
  "M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.1.79-.25.79-.56v-2.17c-3.2.7-3.87-1.37-3.87-1.37-.52-1.33-1.28-1.68-1.28-1.68-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.19 1.76 1.19 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.73-1.54-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11 11 0 0 1 5.79 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.42-2.7 5.39-5.26 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5z";

// git's progress counters, in the words someone waiting on them would use
const PHASE: Record<string, string> = {
  "Enumerating objects": "Listing files on the server",
  "Counting objects": "Counting files on the server",
  "Compressing objects": "Packing on the server",
  "Receiving objects": "Downloading",
  "Resolving deltas": "Unpacking",
  "Updating files": "Writing files",
};

/** "Receiving objects:  62% (620/1000), 1.20 MiB | 2.30 MiB/s" → Downloading, 62, 1.20 MiB */
function readLine(line: string | null): { label: string; pct: number | null; size?: string } {
  if (!line) return { label: "Connecting", pct: null };
  const m = /^(?:remote:\s*)?([A-Za-z ]+?):\s+(\d+)%(?:[^,]*,\s*([\d.]+\s[KMG]iB))?/.exec(line);
  if (!m) return { label: /^Cloning into/.test(line) ? "Connecting" : "Working", pct: null };
  return { label: PHASE[m[1]] ?? m[1], pct: Number(m[2]), size: m[3] };
}

const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;

/** Shown under the composer when the text starts with a repository link: where it goes, where it opens. */
export function ClonePanel(p: Props) {
  const { repo, parent, name, into, open, parentEmpty, canOpenHere, agent, busy, line, error } = p;
  const host = /^(?:https?:\/\/|ssh:\/\/)?(?:git@)?([^/:]+)/i.exec(repo.url)?.[1]?.toLowerCase() ?? "";
  const shown = repo.url.replace(/^(?:https?:\/\/|ssh:\/\/)?(?:git@)?/i, "").replace(/\.git\/?$/i, "");
  const sep = parent.includes("\\") ? "\\" : "/";
  const folder = into === "here" ? baseName(parent) : name.trim();
  const ready = !busy && !!parent && (into === "here" ? parentEmpty === true : !!name.trim());
  const prog = readLine(line);

  return (
    <div className={`clone${busy ? " busy" : ""}`}>
      <div className="clone-head">
        <span className="clone-mark">
          {host === "github.com" ? (
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden>
              <path d={GITHUB_MARK} />
            </svg>
          ) : (
            <GitBranch size={15} />
          )}
        </span>
        <span className="clone-title">
          <b>{repo.label}</b>
          <span className="clone-url" title={repo.url}>
            {shown}
          </span>
        </span>
        <button className="clone-go" onClick={p.onClone} disabled={!ready}>
          {busy ? "Cloning" : "Clone"}
          {!busy && <kbd>Enter</kbd>}
        </button>
      </div>

      <div className="clone-rows">
        <span className="clone-label">Folder</span>
        <div className="clone-choice">
          <div className="clone-seg">
            <button className={into === "new" ? "on" : ""} disabled={busy} onClick={() => p.onInto("new")}>
              New folder inside
            </button>
            <button
              className={into === "here" ? "on" : ""}
              disabled={busy || parentEmpty === false}
              title={parentEmpty === false ? `${baseName(parent)} already has files in it` : undefined}
              onClick={() => p.onInto("here")}
            >
              Straight into {baseName(parent) || "this folder"}
              {parentEmpty === false && <em>not empty</em>}
            </button>
            {/* not a mode, just a shortcut to the picker: the two choices above then apply to it */}
            <button className="clone-other" disabled={busy} onClick={p.onParent}>
              <FolderOpen size={13} />
              Other folder
            </button>
          </div>
          <span className="clone-path">
            <button className="clone-parent" title={parent ? `${parent}. Click to change.` : "Pick a folder"} onClick={p.onParent} disabled={busy}>
              <FolderOpen size={13} />
              <span className="clone-parent-text">{parent || "Pick a folder"}</span>
            </button>
            {into === "new" && (
              <>
                <span className="clone-sep">{sep}</span>
                <input
                  className="clone-name"
                  value={name}
                  spellCheck={false}
                  disabled={busy}
                  size={Math.max(6, name.length + 1)}
                  onChange={(e) => p.onName(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") p.onClone();
                  }}
                />
              </>
            )}
          </span>
        </div>

        <span className="clone-label">Open in</span>
        <div className="clone-choice">
          <div className="clone-seg">
            <button className={open === "new" ? "on" : ""} disabled={busy} onClick={() => p.onOpen("new")}>
              A new thread
            </button>
            <button className={open === "here" ? "on" : ""} disabled={busy || !canOpenHere} onClick={() => p.onOpen("here")}>
              This thread
            </button>
          </div>
          <span className="clone-says">
            {open === "here" ? `${agent} starts right here, working in ${folder || "the clone"}.` : `Adds ${folder || "the clone"} to the sidebar as its own thread.`}
          </span>
        </div>
      </div>

      {error ? (
        <div className="clone-error">
          <CircleAlert size={13} />
          <span>{error}</span>
        </div>
      ) : busy ? (
        <div className="clone-status">
          <div className={`clone-bar${prog.pct == null ? " sweep" : ""}`}>
            <i style={prog.pct == null ? undefined : { width: `${prog.pct}%` }} />
          </div>
          <span>
            {prog.label}
            {prog.pct != null && ` ${prog.pct}%`}
            {prog.size && ` · ${prog.size}`}
          </span>
        </div>
      ) : null}
    </div>
  );
}
