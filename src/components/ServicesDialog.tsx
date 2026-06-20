import { useEffect } from "react";
import { useUi } from "../stores/ui";
import { ServicesEditor } from "./ServicesEditor";
import { X } from "lucide-react";

// The Services config modal, opened from a project's right-click menu. Configures that folder.
export function ServicesDialog() {
  const target = useUi((s) => s.servicesFor);
  const close = useUi((s) => s.closeServices);

  useEffect(() => {
    if (!target) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [target, close]);

  if (!target) return null;

  return (
    <div className="svc-overlay" onMouseDown={close}>
      <div className="svc-dialog" onMouseDown={(e) => e.stopPropagation()}>
        <div className="svc-dialog-head">
          <span className="svc-dialog-path" title={target.folder}>
            {target.folder}
          </span>
          <button className="svc-dialog-x" onClick={close} title="Close (Esc)">
            <X size={15} />
          </button>
        </div>
        <div className="svc-dialog-body">
          <ServicesEditor folder={target.folder} name={`Services · ${target.name}`} wsId={target.wsId} />
        </div>
      </div>
    </div>
  );
}
