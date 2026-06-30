import { useEffect, useState } from "react";
import { useGit, gitCwd } from "../stores/git";

// a few starter .gitignore templates the dialog can drop in (None = skip)
const GITIGNORES: Record<string, string> = {
  None: "",
  Generic: "node_modules/\ndist/\nbuild/\ntarget/\n__pycache__/\n.venv/\n*.log\n.env\n.env.local\n.DS_Store\n",
  Node: "node_modules/\ndist/\nbuild/\ncoverage/\n.env\n.env.local\nnpm-debug.log*\n.DS_Store\n",
  Python: "__pycache__/\n*.py[cod]\n.venv/\nvenv/\n*.egg-info/\ndist/\nbuild/\n.pytest_cache/\n.env\n.DS_Store\n",
  Rust: "/target\n**/*.rs.bk\nCargo.lock\n.env\n.DS_Store\n",
};

// "Initialize repository" modal — set up the folder as a repo with the details you choose, and
// optionally create it on GitHub + push. Replaces the old one-click bare `git init`.
export function InitRepoDialog() {
  const open = useGit((s) => s.initOpen);
  const busy = useGit((s) => s.initBusy);
  const run = useGit((s) => s.runInitRepo);
  const close = useGit((s) => s.closeInitRepo);

  const [name, setName] = useState("");
  const [branch, setBranch] = useState("main");
  const [ignoreTpl, setIgnoreTpl] = useState("Generic");
  const [readme, setReadme] = useState(false);
  const [commit, setCommit] = useState(true);
  const [commitMsg, setCommitMsg] = useState("Initial commit");
  const [github, setGithub] = useState(false);
  const [priv, setPriv] = useState(true);
  const [description, setDescription] = useState("");

  // seed from the active folder each time it opens
  useEffect(() => {
    if (!open) return;
    setName(gitCwd().split(/[\\/]/).filter(Boolean).pop() || "");
    setBranch("main");
    setIgnoreTpl("Generic");
    setReadme(false);
    setCommit(true);
    setCommitMsg("Initial commit");
    setGithub(false);
    setPriv(true);
    setDescription("");
  }, [open]);

  if (!open) return null;

  const willCommit = commit || github; // gh push needs a commit
  const submit = () => {
    if (busy || !name.trim()) return;
    void run({
      name: name.trim(),
      branch: branch.trim() || "main",
      gitignore: GITIGNORES[ignoreTpl] ?? "",
      readme,
      commit: willCommit,
      commitMsg: commitMsg.trim() || "Initial commit",
      github,
      private: priv,
      description: description.trim(),
    });
  };

  return (
    <div className="cd-overlay" onMouseDown={close}>
      <div className="cd cd-pr" onMouseDown={(e) => e.stopPropagation()}>
        <div className="cd-head">
          <span className="cd-title">Initialize repository</span>
          <span className="cd-count">{name || "this folder"}</span>
        </div>

        <div className="cd-pr-row">
          <label className="cd-field">
            <span>Repository name</span>
            <input className="svc-in" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="cd-field cd-field-sm">
            <span>Default branch</span>
            <input className="svc-in" value={branch} onChange={(e) => setBranch(e.target.value)} />
          </label>
        </div>

        <div className="cd-pr-row">
          <label className="cd-field">
            <span>.gitignore</span>
            <select className="set-select" value={ignoreTpl} onChange={(e) => setIgnoreTpl(e.target.value)}>
              {Object.keys(GITIGNORES).map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label className="cd-check">
            <input type="checkbox" checked={readme} onChange={(e) => setReadme(e.target.checked)} />
            Add README
          </label>
        </div>

        <label className="cd-inline">
          <input type="checkbox" checked={willCommit} disabled={github} onChange={(e) => setCommit(e.target.checked)} />
          <span className="cd-inline-label">Initial commit</span>
          <input
            className="svc-in cd-grow"
            value={commitMsg}
            disabled={!willCommit}
            onChange={(e) => setCommitMsg(e.target.value)}
          />
        </label>

        <label className="cd-inline">
          <input type="checkbox" checked={github} onChange={(e) => setGithub(e.target.checked)} />
          <span className="cd-inline-label">Create on GitHub &amp; push</span>
          <span className="cd-hint">needs gh CLI</span>
        </label>
        {github && (
          <div className="cd-pr-row cd-indent">
            <label className="cd-field cd-field-sm">
              <span>Visibility</span>
              <select
                className="set-select"
                value={priv ? "private" : "public"}
                onChange={(e) => setPriv(e.target.value === "private")}
              >
                <option value="private">Private</option>
                <option value="public">Public</option>
              </select>
            </label>
            <label className="cd-field">
              <span>Description</span>
              <input
                className="svc-in"
                placeholder="optional"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </label>
          </div>
        )}

        <div className="cd-foot">
          <span className="cd-hint">
            {github ? "Creates a GitHub repo + pushes" : "Local repo"}
            {willCommit ? " · first commit" : ""}
          </span>
          <div className="cd-actions">
            <button className="btn" onClick={close} disabled={busy}>
              Cancel
            </button>
            <button className="btn primary" onClick={submit} disabled={busy || !name.trim()}>
              {busy ? "Working…" : github ? "Create & push" : "Initialize"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
