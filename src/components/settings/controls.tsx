import type { ReactNode } from "react";

// The building blocks every settings tab is made of: a labeled row, a card of rows, a switch.
export function Row({ icon, label, desc, children }: { icon?: ReactNode; label: string; desc?: ReactNode; children: ReactNode }) {
  return (
    <div className="set-row">
      {icon && <span className="set-row-ico">{icon}</span>}
      <div className="set-row-info">
        <div className="set-key">{label}</div>
        {desc && <div className="set-desc">{desc}</div>}
      </div>
      <div className="set-control">{children}</div>
    </div>
  );
}

export function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="set-section">
      <div className="set-label">{label}</div>
      <div className="set-group">{children}</div>
    </div>
  );
}

export function Toggle({ on, onChange }: { on: boolean; onChange: (b: boolean) => void }) {
  return (
    <button className={`toggle${on ? " on" : ""}`} onClick={() => onChange(!on)} aria-pressed={on}>
      <span className="toggle-knob" />
    </button>
  );
}
