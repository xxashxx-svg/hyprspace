// The agent CLIs' side of updating: what each install says, the updates running now and how the
// last ones ended. The engine runs the updaters (engine/src/agent_update.rs).

use hyprspace_proto::Agent;
use hyprspace_proto::AgentUpdateEvent;
use hyprspace_proto::agent_update::{Install, Outcome};

#[derive(Default)]
pub struct Agents {
    found: Vec<(Agent, Install)>,
    pub(crate) running: Vec<Agent>,
    ended: Vec<(Agent, Outcome)>,
    /// Agents whose result goes on the corner card, because the card or the sidebar started them.
    pub(crate) announce: Vec<Agent>,
    /// The "is out" card was closed for these versions; it comes back for the next one.
    dismissed: Vec<String>,
    /// Went back to an older version on purpose, so nothing offers the newer one again until the
    /// next launch, and "Restart and update" leaves them be.
    reverted: Vec<Agent>,
}

impl Agents {
    pub fn install(&self, agent: Agent) -> Option<&Install> {
        self.found.iter().find(|(a, _)| *a == agent).map(|(_, i)| i)
    }

    pub fn outcome(&self, agent: Agent) -> Option<&Outcome> {
        self.ended.iter().find(|(a, _)| *a == agent).map(|(_, o)| o)
    }

    /// Agents with a newer version HyprSpace can install, and that version.
    pub fn behind(&self) -> Vec<(Agent, String)> {
        self.found
            .iter()
            .filter(|(a, i)| {
                i.behind() && i.runs() && !self.running.contains(a) && !self.reverted.contains(a)
            })
            .filter_map(|(a, i)| Some((*a, i.latest.clone()?)))
            .collect()
    }

    /// [`Agents::behind`], unless its card was closed for exactly these versions.
    pub fn offered(&self) -> Vec<(Agent, String)> {
        let behind = self.behind();
        let all = behind
            .iter()
            .all(|(a, v)| self.dismissed.contains(&key(*a, v)));
        if all { Vec::new() } else { behind }
    }

    pub fn dismiss(&mut self) {
        let keys: Vec<String> = self.behind().iter().map(|(a, v)| key(*a, v)).collect();
        self.dismissed.extend(keys);
    }

    pub fn start(&mut self, agent: Agent, announce: bool) {
        self.ended.retain(|(a, _)| *a != agent);
        self.announce.retain(|a| *a != agent);
        if announce {
            self.announce.push(agent);
        }
        if !self.running.contains(&agent) {
            self.running.push(agent);
        }
    }

    pub fn event(&mut self, e: AgentUpdateEvent) {
        match e {
            AgentUpdateEvent::Found { agent, install } => {
                self.found.retain(|(a, _)| *a != agent);
                self.found.push((agent, install));
            }
            AgentUpdateEvent::Running { agent } => {
                if !self.running.contains(&agent) {
                    self.running.push(agent);
                }
            }
            AgentUpdateEvent::Done { agent, outcome } => {
                self.running.retain(|a| *a != agent);
                match outcome {
                    Outcome::Reverted { .. } => self.reverted.push(agent),
                    Outcome::Updated { .. } => self.reverted.retain(|a| *a != agent),
                    _ => {}
                }
                self.ended.retain(|(a, _)| *a != agent);
                self.ended.push((agent, outcome));
            }
        }
    }

    /// The results the corner card reports, until it's closed.
    pub fn announced(&self) -> Vec<(Agent, Outcome)> {
        self.ended
            .iter()
            .filter(|(a, _)| self.announce.contains(a) && !self.running.contains(a))
            .cloned()
            .collect()
    }

    pub fn close_announced(&mut self) {
        self.announce.retain(|a| self.running.contains(a));
    }
}

fn key(agent: Agent, version: &str) -> String {
    format!("{}@{version}", agent.cli())
}

/// "Claude 2.1.316", "Claude 2.1.316 and Codex 0.166.0".
pub fn versions(list: &[(Agent, String)]) -> String {
    and(list
        .iter()
        .map(|(a, v)| format!("{} {v}", a.name()))
        .collect())
}

/// "Claude", "Claude and Codex".
pub fn names(list: &[Agent]) -> String {
    and(list.iter().map(|a| a.name().to_string()).collect())
}

fn and(mut items: Vec<String>) -> String {
    match items.len() {
        0 | 1 => items.pop().unwrap_or_default(),
        _ => {
            let last = items.pop().unwrap_or_default();
            format!("{} and {last}", items.join(", "))
        }
    }
}

/// One update's result as a sentence.
pub fn said(agent: Agent, outcome: &Outcome) -> String {
    let name = agent.name();
    match outcome {
        Outcome::Updated { to, .. } => {
            format!("{name} updated to {to}. Open threads switch when they next start.")
        }
        Outcome::Current { version } => {
            format!("{name} {version} is already the newest on its channel.")
        }
        Outcome::Reverted { to } => format!("{name} is back on {to}."),
        Outcome::Failed { why } => format!("{name} didn't update: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyprspace_proto::agent_update::How;

    fn found(agent: Agent, version: &str, latest: Option<&str>, how: How) -> AgentUpdateEvent {
        AgentUpdateEvent::Found {
            agent,
            install: Install {
                version: Some(version.into()),
                how,
                command: format!("{} update", agent.cli()),
                latest: latest.map(Into::into),
                held: false,
            },
        }
    }

    #[test]
    fn only_installs_it_can_run_are_offered_and_a_closed_card_waits_for_the_next_version() {
        let mut a = Agents::default();
        a.event(found(
            Agent::Claude,
            "2.1.314",
            Some("2.1.316"),
            How::Native,
        ));
        a.event(found(
            Agent::Codex,
            "0.164.0",
            Some("0.166.0"),
            How::Unknown,
        ));
        assert_eq!(a.offered(), [(Agent::Claude, "2.1.316".to_string())]);
        a.dismiss();
        assert!(a.offered().is_empty());
        a.event(found(
            Agent::Claude,
            "2.1.314",
            Some("2.1.317"),
            How::Native,
        ));
        assert_eq!(versions(&a.offered()), "Claude 2.1.317");
    }

    #[test]
    fn a_result_shows_on_the_card_only_when_the_card_started_it() {
        let mut a = Agents::default();
        a.start(Agent::Claude, true);
        a.start(Agent::Codex, false);
        assert!(a.behind().is_empty());
        for agent in [Agent::Claude, Agent::Codex] {
            a.event(AgentUpdateEvent::Done {
                agent,
                outcome: Outcome::Current {
                    version: "1.0.0".into(),
                },
            });
        }
        assert_eq!(a.announced().len(), 1);
        a.close_announced();
        assert!(a.announced().is_empty());
        assert!(a.outcome(Agent::Codex).is_some());
    }

    #[test]
    fn going_back_keeps_the_newer_version_from_being_offered_again() {
        let mut a = Agents::default();
        a.event(found(
            Agent::Claude,
            "2.1.314",
            Some("2.1.316"),
            How::Native,
        ));
        a.event(AgentUpdateEvent::Done {
            agent: Agent::Claude,
            outcome: Outcome::Reverted {
                to: "2.1.314".into(),
            },
        });
        a.event(found(
            Agent::Claude,
            "2.1.314",
            Some("2.1.316"),
            How::Native,
        ));
        assert!(a.behind().is_empty());
        assert!(a.install(Agent::Claude).is_some_and(|i| i.behind()));
    }

    #[test]
    fn lists_read_as_english() {
        assert_eq!(names(&[Agent::Claude]), "Claude");
        assert_eq!(names(&[Agent::Claude, Agent::Codex]), "Claude and Codex");
        assert_eq!(
            versions(&[
                (Agent::Claude, "2.1.316".into()),
                (Agent::Codex, "0.166.0".into())
            ]),
            "Claude 2.1.316 and Codex 0.166.0"
        );
    }
}
