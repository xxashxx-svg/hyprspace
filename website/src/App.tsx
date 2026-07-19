import {
  Apple,
  BarChart3,
  Bot,
  Command,
  Download,
  FileCode2,
  FolderTree,
  GitBranch,
  Github,
  LayoutGrid,
  RotateCcw,
  ShieldCheck,
  Sparkles,
  Timer,
  Zap,
} from "lucide-react"

import { ClaudePane, CodexPane, GrokPane } from "@/components/site/sessions"
import { AppShell } from "@/components/site/app-shell"
import { Pane } from "@/components/site/pane"
import {
  AppFrame,
  AppIcon,
  ButtonLink,
  Code,
  Eyebrow,
  Point,
  Reveal,
  Section,
  Wrap,
} from "@/components/site/primitives"
import { DOWNLOAD_MAC, DOWNLOAD_WIN, RELEASES, REPO } from "@/site"

import claudeIcon from "@/assets/brand/claude.svg"
import openaiIcon from "@/assets/brand/openai.svg"
import geminiIcon from "@/assets/brand/gemini.svg"
import opencodeIcon from "@/assets/brand/opencode.svg"
import grokIcon from "@/assets/brand/grok.svg"

const providers = [
  { icon: claudeIcon, name: "Claude" },
  { icon: openaiIcon, name: "Codex" },
  { icon: geminiIcon, name: "Gemini" },
  { icon: opencodeIcon, name: "OpenCode" },
  { icon: grokIcon, name: "Grok" },
]

const features = [
  { icon: LayoutGrid, title: "Tile and swap", body: "Drag any pane to rearrange, or drop it into another space." },
  { icon: RotateCcw, title: "Resume any pane", body: "Reopen a pane and the agent picks up the same chat where it left off." },
  { icon: Command, title: "Command palette", body: "Hit Ctrl K to jump to any project, pane, or automation." },
  { icon: GitBranch, title: "Review and ship", body: "See every change in the git dock, then commit, push, and open a PR." },
  { icon: FileCode2, title: "Built-in editor", body: "Open any file from the tree, edit it, and save without leaving." },
  { icon: Zap, title: "Services on startup", body: "Auto run your dev server or any script when you open a folder." },
  { icon: FolderTree, title: "Projects and spaces", body: "Pin a project to a folder, or make a scratch space across repos." },
  { icon: BarChart3, title: "Usage dashboard", body: "Tokens, sessions, and rate limits per provider, straight from local files." },
]

function Nav() {
  return (
    <header className="sticky top-0 z-30 pt-4">
      <Wrap>
        {/* echoes the app titlebar: same radius family, border and inset highlight */}
        <div className="relative mx-auto flex h-[52px] max-w-[980px] items-center justify-between rounded-xl border border-white/[0.09] bg-[#0e0e11]/85 pr-2 pl-3.5 shadow-[0_16px_40px_-20px_rgba(0,0,0,0.9),inset_0_1px_0_rgba(255,255,255,0.045)] backdrop-blur-xl">
          <a
            href="#top"
            className="inline-flex items-center gap-2.5 text-[14.5px] font-semibold tracking-[-0.01em]"
          >
            <AppIcon className="size-[19px]" /> HyprSpace
          </a>

          <nav className="absolute left-1/2 hidden -translate-x-1/2 items-center gap-0.5 md:flex">
            {[
              ["Parallel", "#parallel"],
              ["Automations", "#automations"],
              ["Launcher", "#launcher"],
              ["Features", "#features"],
            ].map(([label, href]) => (
              <a
                key={href}
                href={href}
                className="rounded-md px-3 py-1.5 text-[13.5px] font-medium text-zinc-400 transition-colors hover:bg-white/[0.055] hover:text-zinc-100"
              >
                {label}
              </a>
            ))}
          </nav>

          <div className="flex items-center gap-1.5">
            <a
              href={REPO}
              target="_blank"
              rel="noreferrer noopener"
              aria-label="HyprSpace on GitHub (opens in a new tab)"
              className="flex size-[34px] items-center justify-center rounded-md border border-white/[0.08] bg-white/[0.03] text-zinc-400 transition-colors hover:bg-white/[0.07] hover:text-zinc-100"
            >
              <Github className="size-4" />
            </a>
            <a
              href={DOWNLOAD_WIN}
              className="flex h-[34px] items-center gap-1.5 rounded-md bg-zinc-50 px-3.5 text-[13px] font-semibold text-zinc-950 transition hover:bg-white active:translate-y-px"
            >
              <Download className="size-3.5" />
              Download
            </a>
          </div>
        </div>
      </Wrap>
    </header>
  )
}

function Hero() {
  return (
    <section className="pt-20 md:pt-24">
      <Wrap className="text-center">
        <span className="inline-flex items-center gap-2.5 rounded-full border border-white/[0.12] bg-white/[0.025] py-1.5 pr-3.5 pl-3 font-mono text-[11.5px] text-zinc-400">
          {providers.map((p) => (
            <img key={p.name} src={p.icon} alt="" className="size-[13px]" />
          ))}
          <span className="h-3 w-px bg-white/[0.12]" />
          {providers.map((p) => p.name).join(" · ")}
        </span>

        <h1 className="mt-7 text-[clamp(40px,6.4vw,72px)] font-medium">
          Run coding agents in parallel.
          <br />
          <span className="text-zinc-500">Automate the rest.</span>
        </h1>

        <p className="mx-auto mt-6 max-w-[56ch] text-[clamp(16px,1.7vw,19px)] leading-relaxed text-zinc-400">
          A desktop workspace for agentic coding. Tile every agent across your projects, then hand
          one a goal and let it run headless and isolated until your tests pass or the work is done.
        </p>

        <div className="mt-9 inline-flex flex-wrap items-center justify-center gap-3.5">
          <ButtonLink href={DOWNLOAD_WIN}>
            <Download className="size-4" /> Download for Windows
          </ButtonLink>
          <ButtonLink href={DOWNLOAD_MAC} variant="line">
            <Apple className="size-4" /> macOS
          </ButtonLink>
          <ButtonLink href="#parallel" variant="ghost" className="group">
            See it run
            <span className="transition group-hover:translate-x-0.5">→</span>
          </ButtonLink>
        </div>

        <p className="mt-5 font-mono text-[12.5px] text-zinc-500">
          Your own CLIs · your own subscriptions · your own keys
        </p>
      </Wrap>

      {/* the money shot: the actual app window, three real agent UIs tiled inside it */}
      <Wrap className="mt-16 max-w-[1300px]">
        <Reveal>
          <AppShell>
            <div className="grid gap-2 lg:grid-cols-3">
              <Pane icon={claudeIcon} name="Claude" folder="acme-app" status="running">
                <ClaudePane />
              </Pane>
              <Pane icon={openaiIcon} name="Codex" folder="acme-app" status="running">
                <CodexPane />
              </Pane>
              <Pane icon={grokIcon} name="Grok" folder="acme-app" status="done">
                <GrokPane />
              </Pane>
            </div>
          </AppShell>
        </Reveal>
      </Wrap>
    </section>
  )
}

function Parallel() {
  return (
    <Section id="parallel">
      <Wrap>
        <Reveal className="max-w-[62ch]">
          <Eyebrow>One window</Eyebrow>
          <h2 className="mt-3.5 text-[clamp(28px,3.8vw,44px)] font-medium">
            Three agents. One feature. No tab juggling.
          </h2>
          <p className="mt-5 text-[16.5px] text-zinc-400">
            Every pane is a real terminal running the agent CLI you already signed into — not a
            reimplementation, not a proxy. Split a feature across providers, or race the same task
            through all of them and keep whichever lands first.
          </p>
        </Reveal>

        <Reveal className="mt-14" delay={80}>
          <div className="grid gap-10 md:grid-cols-3">
            <Point icon={<Bot />} title="Real CLIs, real sessions">
              Each pane spawns <Code>claude</Code>, <Code>codex</Code>, or <Code>gemini</Code> in a
              PTY. Anything the CLI can do, the pane can do.
            </Point>
            <Point icon={<RotateCcw />} title="Resume where you left off">
              Close a pane and reopen it later — the conversation is still there, pinned to the
              folder it started in.
            </Point>
            <Point icon={<LayoutGrid />} title="Drag to rearrange">
              Swap panes around the grid, or drop one into an entirely different space.
            </Point>
          </div>
        </Reveal>
      </Wrap>
    </Section>
  )
}

function Automations() {
  return (
    <Section id="automations">
      <Wrap>
        <div className="grid items-center gap-16 lg:grid-cols-[0.92fr_1.08fr]">
          <Reveal>
            <Eyebrow>Automations</Eyebrow>
            <h2 className="mt-3.5 max-w-[16ch] text-[clamp(28px,3.6vw,42px)] font-medium">
              Agents that don&apos;t stop at one turn.
            </h2>
            <p className="mt-4.5 max-w-[46ch] text-[16.5px] text-zinc-400">
              Give an automation a goal and a stop rule. It runs an agent headless, over and over,
              until your tests pass, until a task list is empty, or on a real cron schedule. Then it
              notifies you with the diff to review.
            </p>
            <div className="mt-6">
              <Point icon={<Timer />} title="Run it your way">
                Until done, every N minutes, once, or on a full cron expression like{" "}
                <Code>*/30 9-18 * * 1-5</Code>. Keep context across passes or start each fresh.
              </Point>
              <Point icon={<ShieldCheck />} title="It can't run forever">
                A hard <Code>max iterations</Code> cap is mandatory, with optional stops on a passing
                check, a sentinel, a time budget, or no progress.
              </Point>
              <Point icon={<GitBranch />} title="Isolated, then reviewed">
                Automations run on a throwaway git worktree, never your working tree. Every run is
                kept with what it changed, what it cost, and why it stopped.
              </Point>
            </div>
          </Reveal>

          <Reveal delay={80}>
            <AppFrame title="Automations">
              <div className="px-5 pt-4.5 pb-4">
                <div className="mb-3 flex items-center gap-2.5">
                  <span className="size-2 animate-pulse rounded-full bg-emerald-400 shadow-[0_0_10px_rgba(52,211,153,0.7)]" />
                  <span className="text-[14px] font-semibold">Nightly dependency updates</span>
                  <span className="ml-auto rounded-full border border-emerald-400/25 bg-emerald-400/10 px-2.5 py-0.5 text-[11.5px] text-emerald-400">
                    Running
                  </span>
                </div>

                <div className="mb-3.5 flex flex-wrap gap-1.5">
                  {["0 3 * * *", "worktree", "max 30", "until: npm test"].map((c, i) => (
                    <span
                      key={c}
                      className={
                        i === 0
                          ? "rounded-full border border-white/[0.12] bg-white/5 px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-100"
                          : "rounded-full border border-white/[0.07] px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-500"
                      }
                    >
                      {c}
                    </span>
                  ))}
                </div>

                <div className="grid gap-1">
                  {[
                    ["Bash: npm outdated", "ok"],
                    ["Edit package.json", "ok"],
                    ["Bash: npm test", "running"],
                  ].map(([line, state]) => (
                    <div
                      key={line}
                      className="flex items-center gap-2 rounded-md border border-white/[0.07] bg-zinc-900 px-2.5 py-1.5 font-mono text-[11.5px] text-zinc-400"
                    >
                      <b className="font-normal text-zinc-600">→</b>
                      {line}
                      <i
                        className={`ml-auto text-[10px] not-italic ${state === "ok" ? "text-emerald-400" : "text-zinc-600"}`}
                      >
                        {state}
                      </i>
                    </div>
                  ))}
                </div>

                <div className="mt-3.5 h-1 overflow-hidden rounded-full bg-zinc-800">
                  <span className="block h-full w-[23%] rounded-full bg-zinc-100" />
                </div>
                <p className="mt-2 font-mono text-[10.5px] text-zinc-500">
                  iteration 7 / 30 · 84k tokens · $0.41
                </p>

                <p className="mt-4 mb-2 font-mono text-[10px] tracking-[0.16em] text-zinc-500 uppercase">
                  past runs
                </p>
                <div className="grid gap-1">
                  {[
                    ["yesterday", "4 files · +120 −38"],
                    ["2 days ago", "2 files · +34 −6"],
                  ].map(([when, what]) => (
                    <div
                      key={when}
                      className="flex items-center gap-2.5 rounded-md border border-white/[0.07] bg-zinc-900 px-2.5 py-[7px] text-[11.5px] text-zinc-400"
                    >
                      <i className="size-1.5 shrink-0 rounded-full bg-emerald-400" />
                      <span className="min-w-[72px] text-zinc-500">{when}</span>
                      <em className="font-mono text-[10.5px] text-emerald-400 not-italic">{what}</em>
                      <span className="ml-auto font-mono text-[10.5px] text-zinc-500">
                        check passed
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </AppFrame>
          </Reveal>
        </div>
      </Wrap>
    </Section>
  )
}

function Launcher() {
  return (
    <Section id="launcher">
      <Wrap>
        <div className="grid items-center gap-16 lg:grid-cols-[1.08fr_0.92fr]">
          <Reveal className="lg:order-2">
            <Eyebrow>Multi-agent launcher</Eyebrow>
            <h2 className="mt-3.5 max-w-[16ch] text-[clamp(28px,3.6vw,42px)] font-medium">
              Fan out a whole team in one click.
            </h2>
            <p className="mt-4.5 max-w-[46ch] text-[16.5px] text-zinc-400">
              Pick a folder, a grid size, and an agent mix. All Claude, one of each, or split evenly
              across five providers. HyprSpace tiles them all at once, each with its own name.
            </p>
            <div className="mt-6">
              <Point icon={<LayoutGrid />} title="One folder, many minds">
                Split the work across a crew, or watch different models take the same task.
              </Point>
              <Point icon={<Sparkles />} title="Saved presets">
                Bottle the exact folder, grid, and mix, then relaunch the whole setup in one click.
              </Point>
            </div>
          </Reveal>

          <Reveal className="lg:order-1" delay={80}>
            <AppFrame title="Launch a workspace">
              <div className="grid gap-5 p-5 sm:grid-cols-2">
                <div>
                  <p className="mb-2.5 font-mono text-[10px] tracking-[0.16em] text-zinc-500 uppercase">
                    layout
                  </p>
                  <div className="mb-3.5 flex flex-wrap gap-1.5">
                    {["2", "4", "6", "8", "12"].map((n) => (
                      <span
                        key={n}
                        className={
                          n === "6"
                            ? "rounded-full border border-white/[0.12] bg-white/5 px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-100"
                            : "rounded-full border border-white/[0.07] px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-500"
                        }
                      >
                        {n}
                      </span>
                    ))}
                  </div>
                  <div className="grid grid-cols-3 gap-1.5">
                    {[claudeIcon, openaiIcon, geminiIcon, claudeIcon, opencodeIcon, grokIcon].map(
                      (ic, i) => (
                        <span
                          key={i}
                          className="grid aspect-[16/11] place-items-center rounded-md border border-white/[0.07] bg-zinc-900"
                        >
                          <img src={ic} alt="" className="size-[17px]" />
                        </span>
                      ),
                    )}
                  </div>
                </div>

                <div>
                  <p className="mb-2.5 font-mono text-[10px] tracking-[0.16em] text-zinc-500 uppercase">
                    agents
                  </p>
                  <div className="mb-3.5 flex flex-wrap gap-1.5">
                    {["All Claude", "One of each", "Split evenly"].map((m) => (
                      <span
                        key={m}
                        className={
                          m === "One of each"
                            ? "rounded-full border border-white/[0.12] bg-white/5 px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-100"
                            : "rounded-full border border-white/[0.07] px-2.5 py-0.5 font-mono text-[10.5px] text-zinc-500"
                        }
                      >
                        {m}
                      </span>
                    ))}
                  </div>
                  <div className="grid gap-1">
                    {[
                      [claudeIcon, "Claude", "2"],
                      [openaiIcon, "Codex", "1"],
                      [geminiIcon, "Gemini", "1"],
                      [opencodeIcon, "OpenCode", "1"],
                      [grokIcon, "Grok", "1"],
                    ].map(([ic, name, n]) => (
                      <div
                        key={name as string}
                        className="flex items-center gap-2.5 rounded-md border border-white/[0.07] bg-zinc-900 px-2.5 py-1.5"
                      >
                        <img src={ic as string} alt="" className="size-3.5" />
                        <span className="text-[12.5px] font-semibold">{name as string}</span>
                        <span className="ml-auto rounded bg-zinc-800 px-2 font-mono text-[11.5px] text-zinc-400">
                          {n as string}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-3 border-t border-white/[0.07] px-5 py-3.5">
                <span className="font-mono text-[11px] text-zinc-500">
                  opens <b className="font-medium text-zinc-400">demo</b> with 6 agents
                </span>
                <span className="ml-auto rounded-md bg-zinc-50 px-4 py-1.5 text-[12.5px] font-semibold text-zinc-950">
                  Launch 6
                </span>
              </div>
            </AppFrame>
          </Reveal>
        </div>
      </Wrap>
    </Section>
  )
}

function Subscription() {
  return (
    <Section id="subscription">
      <Wrap>
        <div className="grid items-center gap-16 lg:grid-cols-[0.92fr_1.08fr]">
          <Reveal>
            <Eyebrow>Your subscription</Eyebrow>
            <h2 className="mt-3.5 max-w-[16ch] text-[clamp(28px,3.6vw,42px)] font-medium">
              Your CLI. Your quota. No middleman.
            </h2>
            <p className="mt-4.5 max-w-[46ch] text-[16.5px] text-zinc-400">
              Every pane runs the agent CLI you already signed into. HyprSpace never stores your API
              keys and never resells your usage. Log in once, then run them all at once.
            </p>
            <div className="mt-6">
              <Point icon={<ShieldCheck />} title="Nothing resold">
                You get exactly what your plan gives you. It&apos;s a workspace, not a reseller.
              </Point>
              <Point icon={<BarChart3 />} title="Usage at a glance">
                Tokens, sessions, and rate limits per provider, read entirely from each tool&apos;s
                local files. No network calls, no tokens spent.
              </Point>
            </div>
          </Reveal>

          <Reveal delay={80}>
            <div className="overflow-hidden rounded-2xl border border-white/[0.12] bg-[#0c0c0e] shadow-[0_24px_56px_-28px_rgba(0,0,0,0.7),inset_0_1px_0_rgba(255,255,255,0.03)]">
              <div className="flex items-center gap-[7px] border-b border-white/[0.07] px-3.5 py-2.5">
                <span className="size-2.5 rounded-full bg-zinc-700" />
                <span className="size-2.5 rounded-full bg-zinc-700" />
                <span className="size-2.5 rounded-full bg-zinc-700" />
                <span className="ml-1.5 font-mono text-[11px] text-zinc-500">~/projects</span>
              </div>
              <pre className="overflow-x-auto p-4.5 font-mono text-[13px] leading-loose text-zinc-400">
                <span className="text-zinc-500"># sign in to the agents you already pay for</span>
                {"\n"}
                <span className="text-zinc-100">$</span>{" "}
                <span className="text-zinc-100">claude</span>
                {"      "}
                <span className="text-zinc-500"># Claude Code</span>
                {"\n"}
                <span className="text-zinc-100">$</span>{" "}
                <span className="text-zinc-100">gemini</span>
                {"      "}
                <span className="text-zinc-500"># Gemini CLI</span>
                {"\n"}
                <span className="text-zinc-100">$</span>{" "}
                <span className="text-zinc-100">codex login</span>
                {" "}
                <span className="text-zinc-500"># OpenAI Codex</span>
                {"\n\n"}
                <span className="text-zinc-500">
                  # then open HyprSpace and tile them. that&apos;s it.
                </span>
              </pre>
            </div>
          </Reveal>
        </div>
      </Wrap>
    </Section>
  )
}

function Workspace() {
  return (
    <Section id="features">
      <Wrap>
        <Reveal className="text-center">
          <Eyebrow>The workspace</Eyebrow>
          <h2 className="mt-3.5 text-[clamp(26px,3.4vw,38px)] font-medium">
            Everything around the agents, in one window.
          </h2>
        </Reveal>

        <Reveal className="mt-14" delay={80}>
          <div className="grid grid-cols-1 gap-x-9 gap-y-10 sm:grid-cols-2 lg:grid-cols-4">
            {features.map(({ icon: Icon, title, body }) => (
              <div key={title}>
                <Icon className="size-5 text-zinc-100" />
                <h3 className="mt-4 text-[15.5px] font-semibold">{title}</h3>
                <p className="mt-1.5 text-[14px] text-zinc-400">{body}</p>
              </div>
            ))}
          </div>
        </Reveal>
      </Wrap>
    </Section>
  )
}

function Closing() {
  return (
    <Section className="text-center">
      <Wrap>
        <Reveal>
          <h2 className="text-[clamp(32px,5vw,56px)] font-medium">
            Stop babysitting one chat at a time.
          </h2>
          <p className="mx-auto mt-5 max-w-[44ch] text-[17px] text-zinc-400">
            Run a fleet of agents in parallel, automate them until the work is done, and keep every
            key your own.
          </p>
          <div className="mt-8 inline-flex flex-wrap items-center justify-center gap-3.5">
            <ButtonLink href={DOWNLOAD_WIN}>
              <Download className="size-4" /> Download for Windows
            </ButtonLink>
            <ButtonLink href={DOWNLOAD_MAC} variant="line">
              <Apple className="size-4" /> Download for macOS
            </ButtonLink>
            <ButtonLink href={REPO} variant="ghost" external>
              <Github className="size-4" /> Source
            </ButtonLink>
          </div>
          <p className="mt-5 font-mono text-[12px] text-zinc-500">
            Windows 10 &amp; 11 · macOS (Apple Silicon) · free during the beta · Linux soon
          </p>
        </Reveal>
      </Wrap>
    </Section>
  )
}

function Footer() {
  return (
    <footer className="border-t border-white/[0.07] pt-10 pb-14">
      <Wrap>
        <div className="flex flex-wrap items-start gap-6">
          <div>
            <div className="flex items-center gap-2.5 font-semibold">
              <AppIcon className="size-[19px]" /> HyprSpace
            </div>
            <p className="mt-3 max-w-[34ch] text-[13.5px] text-zinc-500">
              A desktop workspace for agentic coding. Runs the agents you already pay for.
            </p>
          </div>
          <div className="ml-auto flex gap-5">
            {[
              ["Parallel", "#parallel"],
              ["Automations", "#automations"],
              ["Launcher", "#launcher"],
              ["Source", REPO],
              ["Releases", RELEASES],
            ].map(([label, href]) => (
              <a
                key={label}
                href={href}
                {...(href.startsWith("http")
                  ? { target: "_blank", rel: "noreferrer noopener" }
                  : {})}
                className="text-[14px] text-zinc-400 hover:text-zinc-100"
              >
                {label}
              </a>
            ))}
          </div>
        </div>
        <p className="mt-7 font-mono text-[12.5px] text-zinc-500">
          © 2026 HyprSpace · MIT licensed · Runs your own CLI.
        </p>
      </Wrap>
    </footer>
  )
}

export default function App() {
  return (
    <>
      <a
        href="#top"
        className="fixed top-[-60px] left-3.5 z-50 rounded-md bg-zinc-50 px-3.5 py-2.5 font-semibold text-zinc-950 transition-[top] focus:top-3.5"
      >
        Skip to content
      </a>

      <Nav />

      <main id="top">
        <Hero />
        <Parallel />
        <Automations />
        <Launcher />
        <Subscription />
        <Workspace />
        <Closing />
      </main>

      <Footer />
    </>
  )
}
