// The agent CLIs updating with their own updaters (docs/internals/updates.md, "Agent updates").
// HyprSpace runs the CLI's own `update`, or its package manager when the install is proven to be
// that manager's, as a plain child process: no terminal, no stdin, nothing typed. An install it
// can't prove gets the command to run instead. Both updaters cope with a running binary, so no
// session is stopped: a thread picks the new version up when its session next starts.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use hyprspace_proto::agent_update::{How, Install, Outcome};
use hyprspace_proto::{Agent, AgentUpdateCommand, AgentUpdateEvent, Event};
use hyprspace_update::{Avoid, newer};

use crate::providers::cli_version;
use crate::util::{home_dir, no_window, read_json};

const AGENTS: [Agent; 2] = [Agent::Claude, Agent::Codex];
const LIMIT: Duration = Duration::from_secs(15 * 60);
const SLOW: &str =
    "Still updating after 15 minutes, so HyprSpace stopped waiting. It finishes on its own.";
const ASK_NPM: Duration = Duration::from_secs(4);

static RUNNING: Mutex<Vec<Agent>> = Mutex::new(Vec::new());
static AVOID: Mutex<Avoid> = Mutex::new(BTreeMap::new());
/// The newest version npm last named for each CLI, so a check after an update can say whether
/// it's still behind without asking again.
static NEWEST: Mutex<BTreeMap<&'static str, String>> = Mutex::new(BTreeMap::new());

pub fn set_avoid(avoid: Avoid) {
    *AVOID.lock().unwrap_or_else(|e| e.into_inner()) = avoid;
}

pub fn handle(cmd: AgentUpdateCommand, tx: UnboundedSender<Event>) {
    let send = move |e: AgentUpdateEvent| {
        let _ = tx.unbounded_send(Event::AgentUpdate(e));
    };
    match cmd {
        AgentUpdateCommand::Check { latest } => {
            tokio::spawn(async move {
                for agent in AGENTS {
                    if latest && let Some(v) = ask_npm(package(agent)).await {
                        remember(agent, v);
                    }
                    if let Ok(install) = tokio::task::spawn_blocking(move || found(agent)).await {
                        send(AgentUpdateEvent::Found { agent, install });
                    }
                }
            });
        }
        AgentUpdateCommand::Run { agent } => {
            if !claim(agent) {
                send(busy_already(agent));
                return;
            }
            send(AgentUpdateEvent::Running { agent });
            tokio::task::spawn_blocking(move || {
                let from = cli_version(agent.cli());
                let (outcome, slow) = update(agent, from.clone());
                finish(agent, outcome, slow, from, &send);
            });
        }
        AgentUpdateCommand::Revert { agent, version } => {
            if !plain_version(&version) {
                return;
            }
            if !claim(agent) {
                send(busy_already(agent));
                return;
            }
            send(AgentUpdateEvent::Running { agent });
            tokio::task::spawn_blocking(move || {
                let from = cli_version(agent.cli());
                let (outcome, slow) = revert(agent, &version);
                finish(agent, outcome, slow, from, &send);
            });
        }
    }
}

fn busy_already(agent: Agent) -> AgentUpdateEvent {
    AgentUpdateEvent::Done {
        agent,
        outcome: failed(format!("{} is already updating.", agent.name())),
    }
}

/// Reports a run, and for one still going, waits it out holding the lock so no second updater
/// starts beside it, then reports where the version ended up.
fn finish(
    agent: Agent,
    outcome: Outcome,
    slow: Option<std::process::Child>,
    from: Option<String>,
    send: &dyn Fn(AgentUpdateEvent),
) {
    send(AgentUpdateEvent::Done { agent, outcome });
    if let Some(mut child) = slow {
        let _ = child.wait();
        if let (Some(from), Some(to)) = (from, cli_version(agent.cli()))
            && from != to
        {
            send(AgentUpdateEvent::Done {
                agent,
                outcome: Outcome::Updated { from, to },
            });
        }
    }
    release(agent);
    send(AgentUpdateEvent::Found {
        agent,
        install: found(agent),
    });
}

fn claim(agent: Agent) -> bool {
    let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    if running.contains(&agent) {
        return false;
    }
    running.push(agent);
    true
}

fn release(agent: Agent) {
    RUNNING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|a| *a != agent);
}

fn remember(agent: Agent, version: String) {
    NEWEST
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(agent.cli(), version);
}

fn package(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "@anthropic-ai/claude-code",
        Agent::Codex => "@openai/codex",
    }
}

/// The version npm calls latest. Only the package name goes out.
async fn ask_npm(package: &str) -> Option<String> {
    let url = format!(
        "https://registry.npmjs.org/{}/latest",
        package.replace('/', "%2F")
    );
    let client = reqwest::Client::builder().timeout(ASK_NPM).build().ok()?;
    let body: serde_json::Value = client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    body.get("version")?.as_str().map(str::to_string)
}

/// How `agent` is installed here, and whether npm has something newer.
fn found(agent: Agent) -> Install {
    let version = cli_version(agent.cli());
    let Some(plan) = version.as_ref().and_then(|_| plan(agent)) else {
        return Install {
            version,
            ..Default::default()
        };
    };
    let newest = NEWEST
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(agent.cli())
        .cloned();
    let latest = newest
        .filter(|n| version.as_deref().is_some_and(|v| newer(v, n)))
        // npm only knows Claude's latest channel; stable trails it on purpose
        .filter(|_| agent != Agent::Claude || !claude_on_stable());
    let held = latest.as_ref().is_some_and(|l| {
        AVOID
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(agent.cli())
            .is_some_and(|bad| bad.contains(l))
    });
    Install {
        version,
        how: plan.how,
        command: plan.show,
        latest,
        held,
    }
}

fn claude_on_stable() -> bool {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".claude"));
    read_json(&dir.join("settings.json"))
        .and_then(|s| s.get("autoUpdatesChannel")?.as_str().map(str::to_string))
        .is_some_and(|c| c == "stable")
}

/// What updating an install takes: the command HyprSpace runs (None when it shows `show`
/// instead), with any variables it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Plan {
    how: How,
    run: Option<Vec<OsString>>,
    env: Vec<(String, OsString)>,
    show: String,
}

fn plan(agent: Agent) -> Option<Plan> {
    let path = locate(agent.cli())?;
    let real = std::fs::canonicalize(&path)
        .map(plain)
        .unwrap_or_else(|_| path.clone());
    let npm = npm_prefix_for(agent, &path, &real);
    Some(plan_for(agent, &path, &real, npm.as_deref()))
}

/// `canonicalize` gives `\\?\C:\...` on Windows, and Codex's updater crashes on a `CODEX_HOME`
/// spelled that way.
fn plain(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    }
}

/// The file the harness starts for `cli`: `<cli>.exe` on PATH first, then the shim `cmd`
/// would find through PATHEXT.
fn locate(cli: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
    let names: Vec<String> = if cfg!(windows) {
        let ext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        std::iter::once(format!("{cli}.exe"))
            .chain(
                ext.split(';')
                    .filter(|e| !e.is_empty())
                    .map(|e| format!("{cli}{}", e.to_lowercase())),
            )
            .collect()
    } else {
        vec![cli.to_string()]
    };
    names
        .iter()
        .flat_map(|n| dirs.iter().map(move |d| d.join(n)))
        .find(|p| p.is_file())
}

/// npm's global prefix, asked only when the path looks like npm installed it.
fn npm_prefix_for(agent: Agent, path: &Path, real: &Path) -> Option<PathBuf> {
    let looks = norm(real).contains("/node_modules/")
        || path
            .parent()
            .is_some_and(|d| d.join("node_modules").join(package(agent)).is_dir());
    if !looks {
        return None;
    }
    let mut cmd = shell(&["npm".into(), "prefix".into(), "-g".into()]);
    let out = no_window(&mut cmd).stdin(Stdio::null()).output().ok()?;
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !prefix.is_empty()).then(|| PathBuf::from(prefix))
}

fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

fn plan_for(agent: Agent, path: &Path, real: &Path, npm: Option<&Path>) -> Plan {
    let (p, r) = (norm(path), norm(real));
    let both = |s: &str| p.contains(s) || r.contains(s);
    let cli = agent.cli();
    let own = |how: How, env: Vec<(String, OsString)>| Plan {
        how,
        run: Some(vec![path.as_os_str().to_owned(), "update".into()]),
        env,
        show: format!("{cli} update"),
    };
    let manual = |show: String| Plan {
        how: How::Unknown,
        run: None,
        env: Vec::new(),
        show,
    };
    let pkg = package(agent);
    let latest = format!("{pkg}@latest");
    if npm.is_some_and(|pre| npm_owns(pkg, &p, &r, pre)) {
        let run: Vec<OsString> = vec![
            "npm".into(),
            "install".into(),
            "-g".into(),
            latest.clone().into(),
        ];
        return Plan {
            how: How::Npm,
            run: Some(run),
            env: Vec::new(),
            show: format!("npm install -g {latest}"),
        };
    }
    if both("/.bun/") {
        return manual(format!("bun add -g {latest}"));
    }
    if both("/pnpm/") {
        return manual(format!("pnpm add -g {latest}"));
    }
    if both("/mise/") || both("/volta/") || r.contains("/node_modules/") {
        return manual(format!("npm install -g {latest}"));
    }
    if let Some((cask, name)) = homebrew(&r) {
        let mut run: Vec<OsString> = vec!["brew".into(), "upgrade".into()];
        if cask {
            run.push("--cask".into());
        }
        run.push(name.clone().into());
        let show = run
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        return Plan {
            how: How::Homebrew,
            run: Some(run),
            env: Vec::new(),
            show,
        };
    }
    match agent {
        // WinGet's own copy answers `claude update` with success and stays as it was
        Agent::Claude if both("/microsoft/winget/") => {
            manual("winget upgrade Anthropic.ClaudeCode".into())
        }
        Agent::Claude
            if p.ends_with("/.local/bin/claude")
                || p.ends_with("/.local/bin/claude.exe")
                || both("/.local/share/claude/") =>
        {
            own(How::Native, Vec::new())
        }
        Agent::Codex if r.contains("/.codex/packages/standalone/") => {
            let home = codex_home(real).map(|h| ("CODEX_HOME".to_string(), h.into_os_string()));
            own(How::Standalone, home.into_iter().collect())
        }
        Agent::Codex if p.ends_with("/programs/openai/codex/bin/codex.exe") => {
            own(How::Standalone, Vec::new())
        }
        _ => manual(format!("{cli} update")),
    }
}

/// npm owns the install when its global prefix holds both the command and the package.
fn npm_owns(pkg: &str, path: &str, real: &str, prefix: &Path) -> bool {
    let pre = norm(prefix);
    let inside = |s: &str| s.starts_with(&format!("{}/", pre.trim_end_matches('/')));
    let pkg = pkg.to_lowercase();
    (inside(path) || inside(real))
        && (real.contains(&format!("/node_modules/{pkg}/"))
            || prefix.join("node_modules").join(&pkg).is_dir())
}

/// A Homebrew cask or formula from its keg path: `(cask, name)`.
fn homebrew(real: &str) -> Option<(bool, String)> {
    let (cask, rest) = match (real.find("/caskroom/"), real.find("/cellar/")) {
        (Some(i), _) => (true, &real[i + "/caskroom/".len()..]),
        (None, Some(i)) => (false, &real[i + "/cellar/".len()..]),
        _ => return None,
    };
    let name = rest.split('/').next().filter(|n| !n.is_empty())?;
    Some((cask, name.to_string()))
}

/// The `.codex` folder that owns a standalone install, which `codex update` finds it through.
fn codex_home(real: &Path) -> Option<PathBuf> {
    real.ancestors()
        .find(|a| {
            a.file_name().is_some_and(|n| n == "standalone")
                && a.parent()
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == "packages")
        })
        .and_then(|a| a.parent()?.parent())
        .map(Path::to_path_buf)
}

fn update(agent: Agent, from: Option<String>) -> (Outcome, Option<std::process::Child>) {
    let Some(from) = from else {
        return (failed(format!("{} isn't installed.", agent.name())), None);
    };
    let Some(plan) = plan(agent) else {
        return (
            failed(format!("Couldn't find {} on PATH.", agent.cli())),
            None,
        );
    };
    if let Some(why) = npm_in_use(agent, &plan) {
        return (failed(why), None);
    }
    let Some(run) = &plan.run else {
        return (failed(format!("Run `{}` yourself.", plan.show)), None);
    };
    match execute(run, &plan.env) {
        Ran::Ok => {}
        Ran::Failed(why) => return (failed(why), None),
        Ran::Waiting(child) => return (failed(SLOW.into()), Some(child)),
    }
    let outcome = match cli_version(agent.cli()) {
        Some(to) if to != from => Outcome::Updated { from, to },
        Some(version) => Outcome::Current { version },
        None => failed(format!(
            "The update finished, but `{} --version` no longer answers.",
            agent.cli()
        )),
    };
    (outcome, None)
}

/// On Windows npm can't replace files a running process holds, and stops halfway, so an npm
/// install waits until nothing runs from its package folder.
fn npm_in_use(agent: Agent, plan: &Plan) -> Option<String> {
    if !cfg!(windows) || plan.how != How::Npm {
        return None;
    }
    let shim = locate(agent.cli())?;
    let scope = package(agent).split('/').next()?;
    let dir = norm(&shim.parent()?.join("node_modules").join(scope));
    let mut sys = sysinfo::System::new();
    sys.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::All,
        sysinfo::ProcessRefreshKind::new().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
    );
    sys.processes()
        .values()
        .filter_map(|p| p.exe())
        .any(|exe| norm(exe).starts_with(&dir))
        .then(|| {
            format!(
                "{} is running from its npm install, and npm can't replace it while it runs. Close its threads, then update.",
                agent.name()
            )
        })
}

fn revert(agent: Agent, version: &str) -> (Outcome, Option<std::process::Child>) {
    let Some(plan) = plan(agent).filter(|p| agent == Agent::Claude && p.how == How::Native) else {
        return (
            failed("Only Claude's own install can go back to an older version.".into()),
            None,
        );
    };
    let Some(mut run) = plan.run else {
        return (failed("Couldn't find claude on PATH.".into()), None);
    };
    run.truncate(1);
    run.extend([OsString::from("install"), OsString::from(version)]);
    match execute(&run, &[]) {
        Ran::Ok => {}
        Ran::Failed(why) => return (failed(why), None),
        Ran::Waiting(child) => return (failed(SLOW.into()), Some(child)),
    }
    let outcome = match cli_version(agent.cli()) {
        Some(to) if to == version => Outcome::Reverted { to },
        Some(to) => failed(format!("Claude is on {to}, not {version}.")),
        None => failed("`claude --version` no longer answers.".into()),
    };
    (outcome, None)
}

fn failed(why: String) -> Outcome {
    Outcome::Failed { why }
}

/// A version to pass on a command line: digits, dots and a pre-release tag, nothing else.
fn plain_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 40
        && v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
}

/// A command as a child process, through `cmd /C` on Windows for a package manager's `.cmd`
/// shim. The arguments are HyprSpace's own, never user text.
fn shell(argv: &[OsString]) -> Command {
    let program = &argv[0];
    let is_path = Path::new(program).components().count() > 1;
    if cfg!(windows) && !is_path {
        let mut c = Command::new("cmd");
        c.arg("/D").arg("/C").args(argv);
        c
    } else {
        let mut c = Command::new(program);
        c.args(&argv[1..]);
        c
    }
}

/// Runs an updater to the end, or for at most five minutes. Ok when it exits 0; otherwise the
/// last line it printed, which is where updaters put the reason.
/// How an updater run ended: done, failed with its last line, or still going when HyprSpace
/// stopped waiting.
enum Ran {
    Ok,
    Failed(String),
    Waiting(std::process::Child),
}

/// Runs an updater to the end, waiting at most fifteen minutes. A slow one is never killed:
/// stopping `claude update` while it copies the new `claude.exe` into place would leave no
/// working Claude, so it's left to finish on its own.
fn execute(argv: &[OsString], env: &[(String, OsString)]) -> Ran {
    let mut cmd = shell(argv);
    // Codex's installer runs `tar`; Git's GNU tar ahead of Windows' own reads `C:` as a host
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        let mut paths = vec![PathBuf::from(root).join("System32")];
        paths.extend(
            std::env::var_os("PATH").map_or_else(Vec::new, |p| std::env::split_paths(&p).collect()),
        );
        if let Ok(joined) = std::env::join_paths(paths) {
            cmd.env("PATH", joined);
        }
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let spawned = no_window(&mut cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => return Ran::Failed(format!("Couldn't start the updater: {e}")),
    };
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if started.elapsed() < LIMIT => std::thread::sleep(Duration::from_millis(200)),
            Ok(None) => return Ran::Waiting(child),
            Err(e) => return Ran::Failed(format!("Lost track of the updater: {e}")),
        }
    };
    if status.success() {
        return Ran::Ok;
    }
    let text = |b: &Arc<Mutex<Vec<u8>>>| {
        String::from_utf8_lossy(&b.lock().unwrap_or_else(|e| e.into_inner())).to_string()
    };
    Ran::Failed(
        last_line(&text(&err))
            .or_else(|| last_line(&text(&out)))
            .unwrap_or_else(|| format!("The updater stopped with {status}.")),
    )
}

/// Reads a pipe to its end on its own thread, so a chatty updater never blocks on a full pipe.
fn drain(pipe: Option<impl Read + Send + 'static>) -> Arc<Mutex<Vec<u8>>> {
    let buf = Arc::new(Mutex::new(Vec::new()));
    if let Some(mut pipe) = pipe {
        let keep = buf.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut b = keep.lock().unwrap_or_else(|e| e.into_inner());
                b.extend_from_slice(&chunk[..n]);
                let over = b.len().saturating_sub(64 * 1024);
                b.drain(..over);
            }
        });
    }
    buf
}

/// The last line that says something, without terminal colors or a crash's backtrace frames.
fn last_line(text: &str) -> Option<String> {
    let text = text.split("stack backtrace:").next().unwrap_or(text);
    text.lines()
        .map(strip_ansi)
        .map(|l| l.trim().to_string())
        .rfind(|l| {
            !l.is_empty()
                && l.chars().any(char::is_alphanumeric)
                && !l.starts_with("at ")
                && !l.starts_with("note: run with `RUST_BACKTRACE")
        })
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_at(agent: Agent, path: &str, real: &str) -> Plan {
        plan_for(agent, Path::new(path), Path::new(real), None)
    }

    #[test]
    fn native_and_standalone_installs_run_their_own_updater() {
        let p = plan_at(
            Agent::Claude,
            r"C:\Users\a\.local\bin\claude.exe",
            r"C:\Users\a\.local\bin\claude.exe",
        );
        assert_eq!(p.how, How::Native);
        assert_eq!(p.run.unwrap()[1], "update");
        let p = plan_at(
            Agent::Claude,
            "/Users/a/.local/bin/claude",
            "/Users/a/.local/share/claude/versions/2.1.314",
        );
        assert_eq!(p.how, How::Native);

        let p = plan_at(
            Agent::Codex,
            "C:/Users/a/AppData/Local/Programs/OpenAI/Codex/bin/codex.exe",
            "C:/Users/a/.codex/packages/standalone/releases/0.164.0-x86_64-pc-windows-msvc/bin/codex.exe",
        );
        assert_eq!(p.how, How::Standalone);
        assert_eq!(p.env[0].0, "CODEX_HOME");
        assert_eq!(
            PathBuf::from(&p.env[0].1),
            PathBuf::from("C:/Users/a/.codex")
        );
    }

    #[test]
    fn installs_it_cant_prove_show_the_command_instead() {
        let p = plan_at(
            Agent::Claude,
            r"C:\Users\a\AppData\Local\Microsoft\WinGet\Links\claude.exe",
            r"C:\Users\a\AppData\Local\Microsoft\WinGet\Packages\Anthropic.ClaudeCode\claude.exe",
        );
        assert_eq!((p.how, p.run), (How::Unknown, None));
        assert_eq!(p.show, "winget upgrade Anthropic.ClaudeCode");

        let p = plan_at(
            Agent::Codex,
            "/home/a/project/node_modules/.bin/codex",
            "/home/a/project/node_modules/@openai/codex/bin/codex.js",
        );
        assert_eq!((p.how, p.run), (How::Unknown, None));

        let p = plan_at(
            Agent::Codex,
            "/home/a/.bun/bin/codex",
            "/home/a/.bun/bin/codex",
        );
        assert_eq!(p.show, "bun add -g @openai/codex@latest");

        assert_eq!(
            plan_at(Agent::Claude, "/opt/x/claude", "/opt/x/claude").run,
            None
        );
    }

    #[test]
    fn npm_only_counts_inside_its_own_prefix() {
        let p = plan_for(
            Agent::Codex,
            Path::new("/usr/local/bin/codex"),
            Path::new("/usr/local/lib/node_modules/@openai/codex/bin/codex.js"),
            Some(Path::new("/usr/local")),
        );
        assert_eq!(p.how, How::Npm);
        let run: Vec<String> = p
            .run
            .unwrap()
            .iter()
            .map(|a| a.to_string_lossy().into())
            .collect();
        assert_eq!(run, ["npm", "install", "-g", "@openai/codex@latest"]);
        let p = plan_for(
            Agent::Codex,
            Path::new("/home/a/app/node_modules/.bin/codex"),
            Path::new("/home/a/app/node_modules/@openai/codex/bin/codex.js"),
            Some(Path::new("/usr/local")),
        );
        assert_eq!(p.run, None);
    }

    #[test]
    fn homebrew_upgrades_its_own_cask_or_formula() {
        let p = plan_at(
            Agent::Codex,
            "/opt/homebrew/bin/codex",
            "/opt/homebrew/Caskroom/codex/0.164.0/codex-aarch64-apple-darwin",
        );
        assert_eq!(p.how, How::Homebrew);
        assert_eq!(p.show, "brew upgrade --cask codex");
        let p = plan_at(
            Agent::Claude,
            "/opt/homebrew/bin/claude",
            "/opt/homebrew/Cellar/claude-code/2.1.314/bin/claude",
        );
        assert_eq!(p.show, "brew upgrade claude-code");
    }

    #[cfg(windows)]
    #[test]
    fn an_updater_ends_ok_or_with_its_last_line() {
        let args = |a: &[&str]| a.iter().map(OsString::from).collect::<Vec<_>>();
        assert!(matches!(execute(&args(&["exit", "0"]), &[]), Ran::Ok));
        match execute(
            &args(&["echo", "disk", "full", "1>&2", "&", "exit", "3"]),
            &[],
        ) {
            Ran::Failed(why) => assert_eq!(why, "disk full"),
            _ => panic!("expected a failure"),
        }
    }

    #[test]
    fn only_plain_versions_reach_a_command_line() {
        assert!(plain_version("2.1.314"));
        assert!(plain_version("0.166.0-alpha.2"));
        assert!(!plain_version("latest"));
        assert!(!plain_version("2.1.314 & calc"));
        assert!(!plain_version(""));
    }

    #[test]
    fn failures_report_the_last_line_that_says_something() {
        let out = "Checking...\n\u{1b}[31mError: EACCES: permission denied\u{1b}[0m\n\n───\n";
        assert_eq!(
            last_line(out).as_deref(),
            Some("Error: EACCES: permission denied")
        );
        assert_eq!(last_line("\n \n"), None);
        let crash = "thread 'main' panicked at src/update.rs:9:5:\nno home\nnote: run with `RUST_BACKTRACE=1` to display a backtrace\nstack backtrace:\n   0: std::panicking\n  11: RtlUserThreadStart\n";
        assert_eq!(last_line(crash).as_deref(), Some("no home"));
        assert_eq!(
            plain(PathBuf::from(r"\\?\C:\Users\a\.codex")),
            PathBuf::from(r"C:\Users\a\.codex")
        );
        assert_eq!(
            plain(PathBuf::from(r"\\?\UNC\host\share")),
            PathBuf::from(r"\\?\UNC\host\share")
        );
    }
}
