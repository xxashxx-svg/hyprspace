import { CircleAlert, FolderOpen, GitBranch } from "lucide-react";
import type { RepoRef } from "../../lib/repoUrl";

interface Props {
  repo: RepoRef;
  parent: string;
  name: string;
  busy: boolean;
  error: string | null;
  onParent: () => void;
  onName: (name: string) => void;
  onClone: () => void;
}

const GITHUB_MARK =
  "M12 .5C5.65.5.5 5.65.5 12c0 5.08 3.29 9.39 7.86 10.91.58.1.79-.25.79-.56v-2.17c-3.2.7-3.87-1.37-3.87-1.37-.52-1.33-1.28-1.68-1.28-1.68-1.04-.71.08-.7.08-.7 1.15.08 1.76 1.19 1.76 1.19 1.03 1.76 2.7 1.25 3.36.96.1-.75.4-1.25.73-1.54-2.55-.29-5.24-1.28-5.24-5.69 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.18 1.18a11 11 0 0 1 5.79 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.42-2.7 5.39-5.26 5.68.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5z";

/** Shown under the composer when the text starts with a repository link: where to clone, and go. */
export function ClonePanel({ repo, parent, name, busy, error, onParent, onName, onClone }: Props) {
  const host = /^(?:https?:\/\/|ssh:\/\/)?(?:git@)?([^/:]+)/i.exec(repo.url)?.[1]?.toLowerCase() ?? "";
  const shown = repo.url.replace(/^(?:https?:\/\/|ssh:\/\/)?(?:git@)?/i, "").replace(/\.git\/?$/i, "");
  const sep = parent.includes("\\") ? "\\" : "/";
  const ready = !busy && !!parent && !!name.trim();

  return (
    <div className={`clone${busy ? " busy" : ""}`}>
      <div className="clone-progress" />
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
        <button className="clone-go" onClick={onClone} disabled={!ready}>
          {busy ? "Cloning" : "Clone"}
          {!busy && <kbd>Enter</kbd>}
        </button>
      </div>

      <div className="clone-dest">
        <span className="clone-label">Into</span>
        <span className="clone-path">
          <button className="clone-parent" title={parent ? `${parent}. Click to change.` : "Pick a folder"} onClick={onParent} disabled={busy}>
            <FolderOpen size={13} />
            <span className="clone-parent-text">{parent || "Pick a folder"}</span>
          </button>
          <span className="clone-sep">{sep}</span>
          <input
            className="clone-name"
            value={name}
            spellCheck={false}
            disabled={busy}
            size={Math.max(6, name.length + 1)}
            onChange={(e) => onName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") onClone();
            }}
          />
        </span>
      </div>

      {error ? (
        <div className="clone-error">
          <CircleAlert size={13} />
          <span>{error}</span>
        </div>
      ) : (
        <div className="clone-note">{busy ? "Fetching the repository. This can take a moment on a large one." : "Opens as a new thread when it finishes."}</div>
      )}
    </div>
  );
}
