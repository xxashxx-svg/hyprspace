import { useEffect, useState } from "react";
import { useSkills } from "../stores/skills";
import { listSkills, skillRead, skillWrite, skillDelete, type SkillItem } from "../api";
import { confirmDialog } from "../stores/confirm";
import { useNotifications } from "../stores/notifications";
import { Plus, Pencil, Trash2, ArrowLeft, Bookmark, Sparkles, SquareSlash } from "lucide-react";

const SKILL_TEMPLATE = `---
description: One line on what this skill does and when Claude should use it
---

# Skill

Write the instructions for Claude here.
`;
const slug = (s: string) => s.trim().replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");

type SkillDraft = {
  origScope: string | null;
  origName: string | null;
  scope: "user" | "project";
  name: string;
  content: string;
  kind: "skill" | "command";
};
type SnipDraft = { id: string | null; name: string; body: string };

// Roomy authoring home for snippets + Claude skills (the dock handles drag-to-insert).
export function SkillsManager({ cwd }: { cwd: string }) {
  const snippets = useSkills((s) => s.snippets);
  const [discovered, setDiscovered] = useState<SkillItem[]>([]);
  const [snip, setSnip] = useState<SnipDraft | null>(null);
  const [skill, setSkill] = useState<SkillDraft | null>(null);
  const [busy, setBusy] = useState(false);
  const projName = cwd.split(/[\\/]/).filter(Boolean).pop() || "";

  const refresh = () =>
    listSkills(cwd)
      .then(setDiscovered)
      .catch(() => setDiscovered([]));
  useEffect(() => {
    void useSkills.getState().load();
  }, []);
  useEffect(() => {
    void refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [cwd]);

  const saveSnip = () => {
    if (!snip || !snip.name.trim() || !snip.body.trim()) return;
    if (snip.id) useSkills.getState().updateSnippet(snip.id, snip.name, snip.body);
    else useSkills.getState().addSnippet(snip.name, snip.body);
    setSnip(null);
  };

  const editSkill = async (it: SkillItem) => {
    const content = await skillRead(it.scope, cwd, it.name, it.kind).catch(() => SKILL_TEMPLATE);
    setSkill({
      origScope: it.scope,
      origName: it.name,
      scope: it.scope,
      name: it.name,
      content,
      kind: it.kind,
    });
  };
  const saveSkill = async () => {
    if (!skill || !skill.name.trim()) return;
    const f = skill;
    setBusy(true);
    try {
      await skillWrite(f.scope, cwd, f.name, f.content, f.kind);
      if (f.origName && (f.origName !== f.name.trim() || f.origScope !== f.scope)) {
        await skillDelete(f.origScope ?? "user", cwd, f.origName, f.kind).catch(() => {});
      }
      await refresh();
      setSkill(null);
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't save skill", body: String(e) });
    } finally {
      setBusy(false);
    }
  };
  const delSkill = async (it: SkillItem) => {
    const ok = await confirmDialog({
      title: "Delete skill",
      message: `Delete "${it.command}"? This removes its SKILL.md folder.`,
      confirmLabel: "Delete",
      cancelLabel: "Cancel",
      danger: true,
    });
    if (!ok) return;
    try {
      await skillDelete(it.scope, cwd, it.name, it.kind);
      await refresh();
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't delete skill", body: String(e) });
    }
  };

  // ---- skill editor (full view) ----
  if (skill) {
    const f = skill;
    const set = (p: Partial<SkillDraft>) => setSkill({ ...f, ...p });
    return (
      <div className="skm">
        <button className="skm-back" onClick={() => setSkill(null)}>
          <ArrowLeft size={15} /> Back to skills
        </button>
        <div className="set-section">
          <div className="set-label">{f.origName ? "Edit skill" : "New skill"}</div>
          <div className="set-group">
            <div className="set-row">
              <div className="set-row-info">
                <div className="set-key">Name</div>
                <div className="set-desc">Becomes /{slug(f.name) || "name"}</div>
              </div>
              <div className="set-control">
                <input
                  className="np-input skm-name"
                  autoFocus
                  placeholder="my-skill"
                  value={f.name}
                  onChange={(e) => set({ name: e.target.value })}
                />
              </div>
            </div>
            <div className="set-row">
              <div className="set-row-info">
                <div className="set-key">Scope</div>
                <div className="set-desc">Where the SKILL.md is written</div>
              </div>
              <div className="set-control">
                <div className="np-seg">
                  <button className={f.scope === "user" ? "active" : ""} onClick={() => set({ scope: "user" })}>
                    User
                  </button>
                  <button
                    className={f.scope === "project" ? "active" : ""}
                    disabled={!cwd}
                    onClick={() => cwd && set({ scope: "project" })}
                  >
                    Project
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
        <div className="set-section">
          <div className="set-label">SKILL.md</div>
          <textarea
            className="skm-editor"
            spellCheck={false}
            value={f.content}
            onChange={(e) => set({ content: e.target.value })}
          />
        </div>
        <div className="skm-foot">
          <button className="btn" onClick={() => setSkill(null)} disabled={busy}>
            Cancel
          </button>
          <button className="btn primary" onClick={() => void saveSkill()} disabled={busy || !f.name.trim()}>
            {busy ? "Saving…" : "Save skill"}
          </button>
        </div>
      </div>
    );
  }

  // ---- list view ----
  return (
    <div className="skm">
      <div className="set-section">
        <div className="skm-head">
          <span className="set-label">Snippets</span>
          <button className="skills-add" title="New snippet" onClick={() => setSnip({ id: null, name: "", body: "" })}>
            <Plus size={14} />
          </button>
        </div>

        {snip && (
          <div className="skills-form" style={{ marginBottom: 8 }}>
            <input
              className="np-input"
              placeholder="name"
              autoFocus
              value={snip.name}
              onChange={(e) => setSnip({ ...snip, name: e.target.value })}
            />
            <textarea
              className="mcp-area"
              rows={4}
              placeholder="text to insert — a prompt, a command…"
              value={snip.body}
              onChange={(e) => setSnip({ ...snip, body: e.target.value })}
            />
            <div className="skills-form-foot">
              <button className="btn" onClick={() => setSnip(null)}>
                Cancel
              </button>
              <button className="btn primary" onClick={saveSnip} disabled={!snip.name.trim() || !snip.body.trim()}>
                Save
              </button>
            </div>
          </div>
        )}

        <div className="skm-rows">
          {snippets.length === 0 && !snip && <div className="skm-empty">No snippets yet.</div>}
          {snippets.map((s) => (
            <div className="mcp-row" key={s.id}>
              <span className="mcp-ico">
                <Bookmark size={15} />
              </span>
              <span className="mcp-info">
                <span className="mcp-name">{s.name}</span>
                <span className="mcp-sum">{s.body}</span>
              </span>
              <button
                className="mcp-btn"
                title="Edit"
                onClick={() => setSnip({ id: s.id, name: s.name, body: s.body })}
              >
                <Pencil size={14} />
              </button>
              <button
                className="mcp-btn danger"
                title="Delete"
                onClick={() => useSkills.getState().removeSnippet(s.id)}
              >
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
      </div>

      <div className="set-section">
        <div className="skm-head">
          <span className="set-label">Claude skills</span>
          <button
            className="skills-add"
            title="New skill"
            onClick={() =>
              setSkill({
                origScope: null,
                origName: null,
                scope: "user",
                name: "",
                content: SKILL_TEMPLATE,
                kind: "skill",
              })
            }
          >
            <Plus size={14} />
          </button>
        </div>
        <div className="skm-context">
          {projName ? (
            <>
              Your user skills, plus project skills in <strong>{projName}</strong>.
            </>
          ) : (
            "Your user skills. Open a project to manage its project skills here too."
          )}
        </div>
        <div className="skm-rows">
          {discovered.length === 0 && <div className="skm-empty">None yet — create one with the + above.</div>}
          {discovered.map((it) => (
            <div className="mcp-row" key={`${it.scope}-${it.command}`}>
              <span className="mcp-ico">
                {it.kind === "command" ? <SquareSlash size={15} /> : <Sparkles size={15} />}
              </span>
              <span className="mcp-info">
                <span className="mcp-name">
                  {it.command}
                  <span className="mcp-kind">{it.scope}</span>
                </span>
                <span className="mcp-sum">{it.description || "No description"}</span>
              </span>
              <button className="mcp-btn" title="Edit" onClick={() => void editSkill(it)}>
                <Pencil size={14} />
              </button>
              <button className="mcp-btn danger" title="Delete" onClick={() => void delSkill(it)}>
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
