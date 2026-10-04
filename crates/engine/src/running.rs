// Which agent CLI runs inside each terminal session, read from the processes under its shell.
// A thread started as Claude can have Claude stopped with Ctrl+C and Codex started by hand in the
// same shell; this is how the sidebar learns that. The command line also names the model and,
// for Claude, the conversation.

use std::collections::HashMap;

use hyprspace_proto::{Agent, SessionId};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// An agent seen running in a terminal, with what its command line says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub agent: Agent,
    pub model: Option<String>,
    pub resume: Option<String>,
}

/// How far below the shell an agent is looked for: a `.cmd` shim runs node through cmd, and node
/// may start the real binary, so a few levels.
const DEPTH: usize = 4;

/// The agent a command line runs, judged by its program and, for one run through node, its script.
pub fn agent_of(cmd: &[String]) -> Option<Agent> {
    cmd.iter().take(2).find_map(|a| {
        let path = a.replace('\\', "/").to_lowercase();
        let name = path.rsplit('/').next().unwrap_or(&path);
        let stem = [".exe", ".cmd", ".js", ".mjs"]
            .iter()
            .fold(name, |n, ext| n.strip_suffix(ext).unwrap_or(n));
        if stem == "claude" || path.contains("@anthropic-ai/claude-code") {
            Some(Agent::Claude)
        } else if stem == "codex" || path.contains("@openai/codex") {
            Some(Agent::Codex)
        } else if stem == "gemini" || path.contains("@google/gemini-cli") {
            Some(Agent::Gemini)
        } else {
            None
        }
    })
}

/// The value after any of `names`, as `--name value` or `--name=value`.
fn flag(cmd: &[String], names: &[&str]) -> Option<String> {
    let mut args = cmd.iter();
    while let Some(a) = args.next() {
        for n in names {
            if a == n {
                return args.next().cloned();
            }
            if let Some(v) = a.strip_prefix(n).and_then(|v| v.strip_prefix('=')) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// What a command line says about the agent it runs.
pub fn read(cmd: &[String]) -> Option<Seen> {
    let agent = agent_of(cmd)?;
    let model = match agent {
        Agent::Claude => flag(cmd, &["--model"]),
        _ => flag(cmd, &["--model", "-m"]),
    };
    let resume = match agent {
        Agent::Claude => flag(cmd, &["--resume", "-r", "--session-id"]),
        _ => None,
    };
    Some(Seen {
        agent,
        model,
        resume,
    })
}

/// A view of the machine's processes, refreshed on each look.
pub struct Processes {
    sys: System,
}

impl Processes {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    /// The agent under each shell, the one nearest to it: an agent's own children (a tool it runs
    /// that happens to start another agent) don't count.
    pub fn agents(&mut self, shells: &[(SessionId, u32)]) -> Vec<(SessionId, Option<Seen>)> {
        // the command line is read once per process, the first time it is seen
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            ProcessRefreshKind::new().with_cmd(UpdateKind::OnlyIfNotSet),
        );
        let procs = self.sys.processes();
        let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
        for (pid, p) in procs {
            if let Some(parent) = p.parent() {
                children.entry(parent).or_default().push(*pid);
            }
        }
        let cmd_of = |pid: &Pid| -> Vec<String> {
            procs.get(pid).map_or_else(Vec::new, |p| {
                let cmd: Vec<String> = p
                    .cmd()
                    .iter()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect();
                if cmd.is_empty() {
                    vec![p.name().to_string_lossy().into_owned()]
                } else {
                    cmd
                }
            })
        };
        shells
            .iter()
            .map(|&(id, shell)| {
                let mut level = vec![Pid::from_u32(shell)];
                for _ in 0..DEPTH {
                    let next: Vec<Pid> = level
                        .iter()
                        .flat_map(|p| children.get(p).into_iter().flatten().copied())
                        .collect();
                    if let Some(seen) = next.iter().find_map(|p| read(&cmd_of(p))) {
                        return (id, Some(seen));
                    }
                    level = next;
                }
                (id, None)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn agents_are_known_by_their_program_or_their_node_script() {
        assert_eq!(
            agent_of(&cmd(r"C:\Users\a\.local\bin\claude.exe --resume x")),
            Some(Agent::Claude)
        );
        assert_eq!(
            agent_of(&cmd(
                "node /usr/lib/node_modules/@anthropic-ai/claude-code/cli.js"
            )),
            Some(Agent::Claude)
        );
        assert_eq!(
            agent_of(&cmd(r"node C:\npm\node_modules\@openai\codex\bin\codex.js")),
            Some(Agent::Codex)
        );
        assert_eq!(
            agent_of(&cmd("/opt/homebrew/bin/gemini")),
            Some(Agent::Gemini)
        );
        assert_eq!(agent_of(&cmd("powershell.exe -NoLogo")), None);
        // a tool claude runs that only mentions claude further along is not claude
        assert_eq!(agent_of(&cmd("git log --grep claude")), None);
    }

    #[test]
    fn the_command_line_names_the_model_and_conversation() {
        let seen = read(&cmd(
            "claude --settings s.json --resume 0b6e1f9c --model claude-opus-5-5",
        ))
        .unwrap();
        assert_eq!(seen.model.as_deref(), Some("claude-opus-5-5"));
        assert_eq!(seen.resume.as_deref(), Some("0b6e1f9c"));
        let seen = read(&cmd("codex -m gpt-5.5 --yolo")).unwrap();
        assert_eq!(
            (seen.agent, seen.model.as_deref()),
            (Agent::Codex, Some("gpt-5.5"))
        );
        let seen = read(&cmd("claude --model=opus")).unwrap();
        assert_eq!((seen.model.as_deref(), seen.resume), (Some("opus"), None));
    }
}
