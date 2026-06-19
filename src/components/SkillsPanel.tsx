import { useEffect, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { useUi } from "../stores/ui";
import { useWorkspaces } from "../stores/workspace";
import { useSkills } from "../stores/skills";
import {
  listSkills,
  writePty,
  skillRead,
  skillWrite,
  skillDelete,
  type SkillItem,
} from "../api";
import { confirmDialog } from "../stores/confirm";
import { useNotifications } from "../stores/notifications";
import { Plus, Sparkles, SquareSlash, Pencil, Trash2, Bookmark, ArrowLeft } from "lucide-react";

const SKILL_TEMPLATE = `---
description: One line on what this skill does and when Claude should use it
---

# Skill

Write the instructions for Claude here.
`;

// mirror the Rust folder-name sanitizer, for the "/name" preview
const slug = (s: string) => s.trim().replace(/[^a-zA-Z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");

function paneAt(x: number, y: number): string | null {
  const el = document.elementFromPoint(x, y) as HTMLElement | null;
  return el?.closest<HTMLElement>(".pane-cell")?.dataset.sid ?? null;
}
function inject(sid: string, text: string) {
  const wrapped = text.includes("\n") ? `\x1b[200~${text}\x1b[201~` : text;
  void writePty(sid, new TextEncoder().encode(wrapped));
  useWorkspaces.getState().setFocused(sid);
}
function providerOf(sid: string): string | undefined {
  for (const w of useWorkspaces.getState().workspaces) {
    const s = w.sessions.find((x) => x.id === sid);
    if (s) return s.provider;
  }
  return undefined;
}
function focusedSid(): string | undefined {
  const ws = useWorkspaces.getState();
  const active = ws.workspaces.find((w) => w.id === ws.activeId);
  const fid = ws.focusedSessionId;
  return fid && active?.sessions.some((s) => s.id === fid) ? fid : undefined;
}
// a Claude "/command" only works inside Claude; in Codex/Gemini/shell we paste the skill's
// instructions (its SKILL.md body) so it still does something useful there.
type DragItem = { kind: "snippet" | "skill"; command?: string; body: string };
function resolveText(p: DragItem, provider?: string): string {
  if (p.kind === "skill" && provider !== "claude") return p.body || (p.command ?? "");
  return p.command ? p.command + " " : p.body;
}

type SkillDraft = {
  origScope: string | null;
  origName: string | null;
  scope: "user" | "project";
  name: string;
  content: string;
  kind: "skill" | "command";
};

export function SkillsPanel({ cwd }: { cwd: string }) {
  const snippets = useSkills((s) => s.snippets);
  const [discovered, setDiscovered] = useState<SkillItem[]>([]);
  const [form, setForm] = useState<{ id: string | null; name: string; body: string } | null>(null);
  const [skillForm, setSkillForm] = useState<SkillDraft | null>(null);
  const [busy, setBusy] = useState(false);

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

  // drag a chip onto a pane to insert; plain click inserts into the focused pane
  const drag = useRef<{ p: DragItem; sx: number; sy: number; active: boolean } | null>(null);
  const onDown = (e: RPointerEvent, p: DragItem) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture?.(e.pointerId);
    drag.current = { p, sx: e.clientX, sy: e.clientY, active: false };
  };
  const onMove = (e: RPointerEvent) => {
    const d = drag.current;
    if (!d) return;
    if (!d.active) {
      if (Math.hypot(e.clientX - d.sx, e.clientY - d.sy) < 5) return;
      d.active = true;
      document.body.classList.add("skill-dragging");
    }
    useUi.getState().setSkillDrop(paneAt(e.clientX, e.clientY));
  };
  const onUp = (e: RPointerEvent) => {
    const d = drag.current;
    drag.current = null;
    e.currentTarget.releasePointerCapture?.(e.pointerId);
    document.body.classList.remove("skill-dragging");
    useUi.getState().setSkillDrop(null);
    if (!d) return;
    const sid = d.active ? paneAt(e.clientX, e.clientY) : focusedSid();
    if (sid) inject(sid, resolveText(d.p, providerOf(sid)));
  };
  const dragProps = (p: DragItem) => ({
    onPointerDown: (e: RPointerEvent) => onDown(e, p),
    onPointerMove: onMove,
    onPointerUp: onUp,
  });

  // ---- snippets ----
  const saveSnippet = () => {
    if (!form || !form.name.trim() || !form.body.trim()) return;
    if (form.id) useSkills.getState().updateSnippet(form.id, form.name, form.body);
    else useSkills.getState().addSnippet(form.name, form.body);
    setForm(null);
  };

  // ---- claude skills ----
  const editSkill = async (it: SkillItem) => {
    const content = await skillRead(it.scope, cwd, it.name, it.kind).catch(() => SKILL_TEMPLATE);
    setSkillForm({
      origScope: it.scope,
      origName: it.name,
      scope: it.scope,
      name: it.name,
      content,
      kind: it.kind,
    });
  };
  const saveSkill = async () => {
    if (!skillForm || !skillForm.name.trim()) return;
    const f = skillForm;
    setBusy(true);
    try {
      await skillWrite(f.scope, cwd, f.name, f.content, f.kind);
      if (f.origName && (f.origName !== f.name.trim() || f.origScope !== f.scope)) {
        await skillDelete(f.origScope ?? "user", cwd, f.origName, f.kind).catch(() => {});
      }
      await refresh();
      setSkillForm(null);
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't save skill", body: String(e) });
    } finally {
      setBusy(false);
    }
  };
  const deleteSkill = async (it: SkillItem) => {
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

  // ---- skill editor view ----
  if (skillForm) {
    const f = skillForm;
    const set = (p: Partial<SkillDraft>) => setSkillForm({ ...f, ...p });
    return (
      <div className="dock-body skills">
        <div className="mcp-edit-head">
          <button className="mcp-back" onClick={() => setSkillForm(null)}>
            <ArrowLeft size={15} />
          </button>
          <span className="mcp-edit-title">{f.origName ? "Edit skill" : "New skill"}</span>
        </div>
        <div className="mcp-field">
          <span className="mcp-flabel">
            Name <span className="mcp-hint">→ /{slug(f.name) || "name"}</span>
          </span>
          <input
            className="np-input"
            autoFocus
            placeholder="my-skill"
            value={f.name}
            onChange={(e) => set({ name: e.target.value })}
          />
        </div>
        <div className="mcp-field">
          <span className="mcp-flabel">Scope</span>
          <div className="np-seg">
            <button className={f.scope === "user" ? "active" : ""} onClick={() => set({ scope: "user" })}>
              User (~/.claude)
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
        <div className="mcp-field">
          <span className="mcp-flabel">SKILL.md</span>
          <textarea
            className="mcp-area"
            rows={12}
            value={f.content}
            onChange={(e) => set({ content: e.target.value })}
          />
        </div>
        <div className="mcp-edit-foot">
          <button className="btn" onClick={() => setSkillForm(null)} disabled={busy}>
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
    <div className="dock-body skills">
      <div className="skills-hint">
        Drag onto a terminal to insert · click for the focused pane.{" "}
        <button className="skills-manage" onClick={() => useUi.getState().openSettings("skills")}>
          Manage in Settings
        </button>
      </div>

      <div className="skills-sec">
        <div className="skills-sec-head">
          <span>Snippets</span>
          <button
            className="skills-add"
            title="New snippet"
            onClick={() => setForm({ id: null, name: "", body: "" })}
          >
            <Plus size={14} />
          </button>
        </div>

        {form && (
          <div className="skills-form">
            <input
              className="np-input"
              placeholder="name"
              autoFocus
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <textarea
              className="mcp-area"
              rows={3}
              placeholder="text to insert — a prompt, a command…"
              value={form.body}
              onChange={(e) => setForm({ ...form, body: e.target.value })}
            />
            <div className="skills-form-foot">
              <button className="btn" onClick={() => setForm(null)}>
                Cancel
              </button>
              <button
                className="btn primary"
                onClick={saveSnippet}
                disabled={!form.name.trim() || !form.body.trim()}
              >
                Save
              </button>
            </div>
          </div>
        )}

        {snippets.length === 0 && !form && <div className="skills-empty">No snippets yet.</div>}
        {snippets.map((s) => (
          <div className="skill-chip" key={s.id} {...dragProps({ kind: "snippet", body: s.body })} title={s.body}>
            <span className="skill-chip-ico">
              <Bookmark size={14} />
            </span>
            <span className="skill-chip-body">
              <span className="skill-chip-name">{s.name}</span>
              <span className="skill-chip-desc">{s.body}</span>
            </span>
            <span className="skill-chip-acts">
              <button
                className="skill-act"
                title="Edit"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={() => setForm({ id: s.id, name: s.name, body: s.body })}
              >
                <Pencil size={12} />
              </button>
              <button
                className="skill-act"
                title="Delete"
                onPointerDown={(e) => e.stopPropagation()}
                onClick={() => useSkills.getState().removeSnippet(s.id)}
              >
                <Trash2 size={12} />
              </button>
            </span>
          </div>
        ))}
      </div>

      <div className="skills-sec">
        <div className="skills-sec-head">
          <span>Claude skills</span>
          <button
            className="skills-add"
            title="New skill"
            onClick={() =>
              setSkillForm({
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
        {discovered.length === 0 ? (
          <div className="skills-empty">None yet — create one with the + above.</div>
        ) : (
          discovered.map((it) => (
            <div
              className="skill-chip"
              key={`${it.scope}-${it.command}`}
              {...dragProps({ kind: "skill", command: it.command, body: it.body })}
              title={it.description || it.command}
            >
              <span className="skill-chip-ico">
                {it.kind === "command" ? <SquareSlash size={14} /> : <Sparkles size={14} />}
              </span>
              <span className="skill-chip-body">
                <span className="skill-chip-name">{it.command}</span>
                {it.description && <span className="skill-chip-desc">{it.description}</span>}
              </span>
              <span className="skill-chip-acts">
                <button
                  className="skill-act"
                  title="Edit"
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={() => void editSkill(it)}
                >
                  <Pencil size={12} />
                </button>
                <button
                  className="skill-act"
                  title="Delete"
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={() => void deleteSkill(it)}
                >
                  <Trash2 size={12} />
                </button>
              </span>
              <span className="skill-chip-scope">{it.scope}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
