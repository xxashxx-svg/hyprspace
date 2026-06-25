import { Repeat } from "lucide-react";
import { LoopsManager } from "./LoopsManager";

// The dedicated full-page Loops / Automations dashboard (rail → Loops). It reuses the same
// LoopsManager panel that the rest of the app drives, just framed as a first-class page.
export function LoopsPage() {
  return (
    <div className="loops-page">
      <div className="loops-page-inner">
        <div className="loops-page-head">
          <Repeat size={20} strokeWidth={1.75} />
          <div>
            <h1>Loops &amp; Automations</h1>
            <p>Agents that run on a schedule, on an interval, or until the job's done.</p>
          </div>
        </div>
        <LoopsManager />
      </div>
    </div>
  );
}
