// The automation editor: what it should do, where, when, and when to give up. Everything else is
// a default or lives under Advanced — the old form asked twelve questions to express one sentence.
import { useState } from "react";
import { ChevronRight, Folder, Trash2 } from "lucide-react";
import { useLoops, type LoopDef } from "../stores/loops";
import { pickFolder } from "../api";
import { useConfirm } from "../stores/confirm";

const PERMS = [
  { id: "acceptEdits", label: "Accept edits" },
  { id: "plan", label: "Plan only" },
  { id: "bypass", label: "Bypass" },
];

/** the four schedule shapes, mapped onto the stored mode + schedule fields */
type When = "manual" | "interval" | "daily" | "cron";

function whenOf(def: LoopDef): When {
  if (def.mode === "interval") return "interval";
  if (def.mode === "cron") return def.schedule?.cron ? "cron" : "daily";
  return "manual";
}

export function AutomationEditor({ id, onClose }: { id: string; onClose: () => void }) {
  const def = useLoops((s) => s.loops[id]);
  const [advanced, setAdvanced] = useState(false);
  if (!def) return null;

  const save = (patch: Partial<LoopDef>) => useLoops.getState().upsert({ ...def, ...patch });
  const when = whenOf(def);

  const setWhen = (w: When) => {
    if (w === "manual") return save({ mode: "manual" });
    if (w === "interval") return save({ mode: "interval", intervalSec: def.intervalSec || 3600 });
    if (w === "daily") return save({ mode: "cron", schedule: { dailyAt: def.schedule?.dailyAt || "03:00" } });
    save({ mode: "cron", schedule: { cron: def.schedule?.cron || "0 3 * * *" } });
  };

  const chooseFolder = async () => {
    const f = await pickFolder();
    if (f) save({ folder: f });
  };

  const del = async () => {
    const ok = await useConfirm.getState().open({
      title: "Delete automation",
      message: `“${def.name || "Untitled"}” and its run history will be removed.`,
      confirmLabel: "Delete",
      danger: true,
    });
    if (ok) {
      useLoops.getState().remove(id);
      onClose();
    }
  };

  return (
    <div className="ae">
      <input
        className="ae-name"
        value={def.name}
        placeholder="Name this automation"
        onChange={(e) => save({ name: e.target.value })}
      />

      <label className="ae-field">
        <span className="ae-lab">What should it do?</span>
        <textarea
          className="ae-input ae-ta"
          value={def.prompt}
          placeholder="Check every dependency for updates, apply the safe ones, run the tests, and stop if anything fails."
          onChange={(e) => save({ prompt: e.target.value })}
        />
      </label>

      <div className="ae-two">
        <label className="ae-field">
          <span className="ae-lab">Where</span>
          <button className="ae-input ae-pick" onClick={() => void chooseFolder()} title={def.folder}>
            <Folder size={13} />
            <span className="ae-pick-txt">{def.folder.split(/[\\/]/).filter(Boolean).pop() || "Pick a folder…"}</span>
          </button>
        </label>

        <label className="ae-field">
          <span className="ae-lab">When</span>
          <select className="ae-input" value={when} onChange={(e) => setWhen(e.target.value as When)}>
            <option value="manual">Only when I run it</option>
            <option value="interval">Every…</option>
            <option value="daily">Daily at…</option>
            <option value="cron">On a cron</option>
          </select>
        </label>
      </div>

      {when === "interval" && (
        <label className="ae-field">
          <span className="ae-lab">Every</span>
          <div className="ae-row">
            <input
              className="ae-input ae-num"
              type="number"
              min={1}
              value={Math.round((def.intervalSec || 3600) / 60)}
              onChange={(e) => save({ intervalSec: Math.max(1, Number(e.target.value)) * 60 })}
            />
            <span className="ae-unit">minutes</span>
          </div>
        </label>
      )}
      {when === "daily" && (
        <label className="ae-field">
          <span className="ae-lab">At</span>
          <input
            className="ae-input ae-num"
            type="time"
            value={def.schedule?.dailyAt || "03:00"}
            onChange={(e) => save({ schedule: { dailyAt: e.target.value } })}
          />
        </label>
      )}
      {when === "cron" && (
        <label className="ae-field">
          <span className="ae-lab">Cron</span>
          <input
            className="ae-input ae-mono"
            value={def.schedule?.cron || ""}
            placeholder="0 3 * * 1-5"
            onChange={(e) => save({ schedule: { cron: e.target.value } })}
          />
        </label>
      )}
      {when !== "manual" && (
        <label className="ae-field ae-check">
          <input
            type="checkbox"
            checked={def.enabled}
            onChange={(e) => save({ enabled: e.target.checked })}
          />
          <span>Arm automatically when HyprSpace opens</span>
        </label>
      )}

      <div className="ae-two">
        <label className="ae-field">
          <span className="ae-lab">Give up after</span>
          <div className="ae-row">
            <input
              className="ae-input ae-num"
              type="number"
              min={1}
              value={def.stop.timeBudgetMin || 60}
              onChange={(e) =>
                save({ stop: { ...def.stop, timeBudgetMin: Math.max(1, Number(e.target.value)) } })
              }
            />
            <span className="ae-unit">minutes</span>
          </div>
        </label>
        <label className="ae-field ae-check">
          <input
            type="checkbox"
            checked={def.worktree}
            onChange={(e) => save({ worktree: e.target.checked })}
          />
          <span>Run in an isolated worktree</span>
        </label>
      </div>

      <button className="ae-adv" onClick={() => setAdvanced((v) => !v)}>
        <ChevronRight size={13} className={advanced ? "ae-adv-open" : ""} />
        Advanced
      </button>
      {advanced && (
        <div className="ae-two">
          <label className="ae-field">
            <span className="ae-lab">Permission</span>
            <select
              className="ae-input"
              value={def.permissionMode || "acceptEdits"}
              onChange={(e) => save({ permissionMode: e.target.value })}
            >
              {PERMS.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.label}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      <div className="ae-foot">
        <button className="ae-del" onClick={() => void del()}>
          <Trash2 size={13} /> Delete
        </button>
        <button className="ae-done" onClick={onClose}>
          Done
        </button>
      </div>
    </div>
  );
}
