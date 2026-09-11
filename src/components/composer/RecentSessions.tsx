import { useEffect, useState } from "react";
import { CornerDownLeft } from "lucide-react";
import { agentSessions, type AgentSession } from "../../api";
import { PROVIDER_LOGO, PROVIDER_NAME } from "../../lib/brand";
import type { ProviderId } from "../../lib/models";
import { relTime } from "../../lib/time";

const CAN_RESUME = new Set<string>(["claude", "codex"]);

interface Props {
  provider: ProviderId;
  cwd: string;
  onPick: (s: AgentSession) => void;
}

/** The provider's saved conversations for this folder. Click one to reopen it as a pane. */
export function RecentSessions({ provider, cwd, onPick }: Props) {
  const [list, setList] = useState<AgentSession[]>([]);
  const supported = CAN_RESUME.has(provider) && !!cwd;
  useEffect(() => {
    if (!supported) return;
    let alive = true;
    agentSessions(provider, cwd)
      .then((l) => alive && setList(l))
      .catch(() => alive && setList([]));
    return () => {
      alive = false;
    };
  }, [provider, cwd, supported]);
  if (!supported || list.length === 0) return null;
  const logo = PROVIDER_LOGO[provider];
  return (
    <section className="recent">
      <header className="recent-head">
        {logo && <img src={logo} alt="" width={13} height={13} />}
        <span>Continue a {PROVIDER_NAME[provider]} session</span>
        <span className="recent-count">{list.length}</span>
      </header>
      <div className="recent-list">
        {list.map((s) => (
          <button key={s.id} className="recent-row" title={s.title} onClick={() => onPick(s)}>
            <span className="recent-title">{s.title}</span>
            <span className="recent-time">{relTime(s.modified)}</span>
            <span className="recent-go">
              Resume
              <CornerDownLeft size={11} />
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}
