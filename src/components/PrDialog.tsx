import { useEffect, useState } from "react";
import { useGit } from "../stores/git";

// "Create pull request" modal — pre-filled from the repo (title + body from the commits, base = the
// default branch) so you review and edit everything before it's opened, instead of a blind one-click.
export function PrDialog() {
  const open = useGit((s) => s.prOpen);
  const defaults = useGit((s) => s.prDefaults);
  const busy = useGit((s) => s.prBusy);
  const create = useGit((s) => s.createPr);
  const close = useGit((s) => s.closePr);

  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [base, setBase] = useState("");
  const [draft, setDraft] = useState(false);

  // seed the fields once the repo defaults arrive
  useEffect(() => {
    if (!defaults) return;
    setTitle(defaults.title);
    setBody(defaults.body);
    setBase(defaults.base);
    setDraft(false);
  }, [defaults]);

  if (!open) return null;

  const loading = !defaults;
  const onDefault = !!defaults?.onDefault;
  const sameAsHead = !!defaults && base === defaults.head;
  const blocked = loading || onDefault || sameAsHead || !title.trim() || !base.trim();
  const submit = () => {
    if (blocked || busy) return;
    void create({ title: title.trim(), body, base: base.trim(), draft, push: !defaults?.pushed });
  };

  return (
    <div className="cd-overlay" onMouseDown={close}>
      <div className="cd cd-pr" onMouseDown={(e) => e.stopPropagation()}>
        <div className="cd-head">
          <span className="cd-title">Create pull request</span>
          {defaults && (
            <span className="cd-count">
              <code>{defaults.head}</code> → <code>{base || defaults.base}</code>
            </span>
          )}
        </div>

        {loading ? (
          <div className="cd-pr-loading">Reading the repo…</div>
        ) : onDefault ? (
          <div className="cd-pr-warn">
            You're on <code>{defaults.head}</code> — that's the base branch. Switch to a feature branch
            to open a PR.
          </div>
        ) : (
          <>
            <label className="cd-field">
              <span>Title</span>
              <input
                className="svc-in"
                autoFocus
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") close();
                }}
              />
            </label>
            <label className="cd-field">
              <span>Description</span>
              <textarea
                className="cd-msg cd-pr-body"
                rows={6}
                placeholder="Describe the change… (markdown supported)"
                value={body}
                onChange={(e) => setBody(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") close();
                  if ((e.metaKey || e.ctrlKey) && e.key === "Enter") submit();
                }}
              />
            </label>
            <div className="cd-pr-row">
              <label className="cd-field">
                <span>Base branch</span>
                <select className="set-select" value={base} onChange={(e) => setBase(e.target.value)}>
                  {(defaults.branches.length ? defaults.branches : [defaults.base]).map((b) => (
                    <option key={b} value={b}>
                      {b}
                    </option>
                  ))}
                </select>
              </label>
              <label className="cd-check">
                <input type="checkbox" checked={draft} onChange={(e) => setDraft(e.target.checked)} />
                Open as draft
              </label>
            </div>
            {sameAsHead && <div className="cd-pr-warn">Base must be different from your branch.</div>}
            {!defaults.pushed && (
              <div className="cd-hint">Your branch isn't pushed yet — it'll be pushed to origin first.</div>
            )}
          </>
        )}

        <div className="cd-foot">
          <span className="cd-hint">{draft ? "Opens a draft PR" : "Opens a PR on GitHub"} · needs the gh CLI</span>
          <div className="cd-actions">
            <button className="btn" onClick={close} disabled={busy}>
              Cancel
            </button>
            <button className="btn primary" onClick={submit} disabled={busy || blocked}>
              {busy ? "Creating…" : draft ? "Create draft" : "Create PR"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
