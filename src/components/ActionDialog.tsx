import { useEffect, useState } from "react";
import { useActionEditor } from "../stores/actionEditor";
import { useProjectConfigs, folderKey, type Action } from "../stores/projectConfig";
import { runAction } from "../lib/startup";
import { Play } from "lucide-react";

// build a "Ctrl+Alt+Shift+K"-style label from a keydown, ignoring bare modifier presses
function comboFromEvent(e: React.KeyboardEvent): string | null {
  const k = e.key;
  if (k === "Control" || k === "Alt" || k === "Shift" || k === "Meta") return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Meta");
  parts.push(k.length === 1 ? k.toUpperCase() : k);
  return parts.join("+");
}

const blank = (): Partial<Action> => ({
  name: "",
  command: "",
  keybinding: "",
  previewUrl: "",
  openPreview: false,
  runOnOpen: false,
  runOnWorktree: false,
  background: false,
});

// Add/Edit an Action — a project-scoped command you run from the top bar, palette, or a keybinding.
export function ActionDialog() {
  const open = useActionEditor((s) => s.open);
  const folder = useActionEditor((s) => s.folder);
  const wsId = useActionEditor((s) => s.wsId);
  const editing = useActionEditor((s) => s.editing);
  const close = useActionEditor((s) => s.close);

  const [f, setF] = useState<Partial<Action>>(blank());
  useEffect(() => {
    if (open) setF(editing ? { ...editing } : blank());
  }, [open, editing]);

  if (!open) return null;

  const set = (patch: Partial<Action>) => setF((x) => ({ ...x, ...patch }));
  const canSave = !!f.name?.trim() && !!f.command?.trim();

  const persist = (): Action => {
    const action: Action = {
      id: editing?.id ?? crypto.randomUUID(),
      name: f.name!.trim(),
      command: f.command!.trim(),
      folder: f.folder?.trim() || undefined,
      keybinding: f.keybinding?.trim() || undefined,
      previewUrl: f.previewUrl?.trim() || undefined,
      openPreview: !!f.openPreview,
      runOnOpen: !!f.runOnOpen,
      runOnWorktree: !!f.runOnWorktree,
      background: !!f.background,
    };
    const cur = useProjectConfigs.getState().getConfig(folder).startup;
    const next = editing ? cur.map((a) => (a.id === action.id ? action : a)) : [...cur, action];
    useProjectConfigs.getState().setConfig(folder, { startup: next });
    return action;
  };

  const save = () => {
    if (!canSave) return;
    persist();
    close();
  };
  const saveAndRun = () => {
    if (!canSave) return;
    const a = persist();
    if (wsId) runAction(wsId, a);
    close();
  };

  return (
    <div className="cd-overlay" onMouseDown={close}>
      <div className="cd cd-pr" onMouseDown={(e) => e.stopPropagation()}>
        <div className="cd-head">
          <span className="cd-title">{editing ? "Edit action" : "Add action"}</span>
          <span className="cd-count">{folderKey(folder).split(/[\\/]/).pop() || "project"}</span>
        </div>

        <label className="cd-field">
          <span>Name</span>
          <div className="ad-name-row">
            {wsId && (
              <button className="ad-run" title="Save & run" disabled={!canSave} onClick={saveAndRun}>
                <Play size={14} />
              </button>
            )}
            <input
              className="svc-in"
              autoFocus
              placeholder="e.g. Dev server"
              value={f.name ?? ""}
              onChange={(e) => set({ name: e.target.value })}
            />
          </div>
        </label>

        <label className="cd-field">
          <span>Keybinding</span>
          <input
            className="svc-in ad-keys"
            readOnly
            placeholder="Press a shortcut"
            value={f.keybinding ?? ""}
            onKeyDown={(e) => {
              e.preventDefault();
              if (e.key === "Backspace" || e.key === "Delete") return set({ keybinding: "" });
              const c = comboFromEvent(e);
              if (c) set({ keybinding: c });
            }}
          />
          <span className="cd-hint">Click, then press a shortcut. Backspace clears it.</span>
        </label>

        <label className="cd-field">
          <span>Command</span>
          <textarea
            className="cd-msg cd-pr-body"
            rows={3}
            placeholder="e.g. npm run dev"
            value={f.command ?? ""}
            onChange={(e) => set({ command: e.target.value })}
          />
        </label>

        <div className="cd-pr-row">
          <label className="cd-field">
            <span>Preview URL (optional)</span>
            <input
              className="svc-in"
              placeholder="http://localhost:5173"
              value={f.previewUrl ?? ""}
              onChange={(e) => set({ previewUrl: e.target.value })}
            />
          </label>
          <label className="cd-field cd-field-sm">
            <span>Subfolder</span>
            <input
              className="svc-in"
              placeholder="(root)"
              value={f.folder ?? ""}
              onChange={(e) => set({ folder: e.target.value })}
            />
          </label>
        </div>

        <label className="cd-inline">
          <input type="checkbox" checked={!!f.openPreview} onChange={(e) => set({ openPreview: e.target.checked })} />
          <span className="cd-inline-label">Open the preview when this action runs</span>
        </label>
        <label className="cd-inline">
          <input type="checkbox" checked={!!f.background} onChange={(e) => set({ background: e.target.checked })} />
          <span className="cd-inline-label">Run in the background (headless — watch its logs)</span>
        </label>
        <label className="cd-inline">
          <input type="checkbox" checked={!!f.runOnOpen} onChange={(e) => set({ runOnOpen: e.target.checked })} />
          <span className="cd-inline-label">Run automatically when the project opens</span>
        </label>
        <label className="cd-inline">
          <input type="checkbox" checked={!!f.runOnWorktree} onChange={(e) => set({ runOnWorktree: e.target.checked })} />
          <span className="cd-inline-label">Run automatically on worktree creation</span>
        </label>

        <div className="cd-foot">
          <span className="cd-hint">Runs from the top bar, ⌘K, or its keybinding</span>
          <div className="cd-actions">
            <button className="btn" onClick={close}>Cancel</button>
            <button className="btn primary" onClick={save} disabled={!canSave}>Save action</button>
          </div>
        </div>
      </div>
    </div>
  );
}
