import { BarChart3, MessageSquare, MessagesSquare, ShieldCheck, SquareTerminal } from "lucide-react"

import { AppWindow } from "@/components/site/app/window"
import { TerminalThread } from "@/components/site/app/terminal"
import { Nav } from "@/components/site/nav"
import { DownloadCta, Install, ReleasePill } from "@/components/site/showcase"
import {
  ComposerVignette,
  EditorVignette,
  GitVignette,
  PaletteVignette,
  PhoneVignette,
  ThreadsVignette,
  UsageVignette,
  WorktreeVignette,
} from "@/components/site/vignettes"
import {
  AppIcon,
  Eyebrow,
  Point,
  Reveal,
  Section,
  Wrap,
} from "@/components/site/primitives"
import { DOWNLOAD_ANDROID, RELEASES, REPO } from "@/site"


const bento = [
  {
    span: "lg:col-span-3",
    visual: <ThreadsVignette />,
    title: "Every thread in one list",
    body: "Two lines per thread: which ones are working, which wait on you and which are done. Settle or snooze one, and Ctrl+Z brings it back.",
  },
  {
    span: "lg:col-span-3",
    visual: <UsageVignette />,
    title: "Your limits in the title bar",
    body: "Session and weekly limits for Claude and Codex, read from your own account.",
  },
  {
    span: "lg:col-span-2",
    visual: <PaletteVignette />,
    title: "Command palette",
    body: "Jump to any thread, file or action with Ctrl K.",
  },
  {
    span: "lg:col-span-2",
    visual: <GitVignette />,
    title: "Review and commit",
    body: "Tick the files, write the message, then commit and push from the dock.",
  },
  {
    span: "lg:col-span-2",
    visual: <WorktreeVignette />,
    title: "A worktree per thread",
    body: "Start a thread on its own branch, then merge it, open a pull request or remove it.",
  },
  {
    span: "lg:col-span-2",
    visual: <PhoneVignette />,
    title: "Your phone",
    body: "Pair the Android app to follow threads, answer approvals and start new ones.",
    link: ["Download the Android app", DOWNLOAD_ANDROID],
  },
  {
    span: "lg:col-span-2",
    visual: <EditorVignette />,
    title: "Files open in the app",
    body: "Text files open ready to edit, and Ctrl+S saves. Images and videos open too, and HTML opens in your browser.",
  },
  {
    span: "lg:col-span-2",
    visual: <ComposerVignette />,
    title: "Start from the composer",
    body: "Pick the agent, model and effort. Type / for commands and skills, @ for files and threads.",
  },
]

function Hero() {
  return (
    <section className="pt-20 md:pt-24">
      <Wrap className="text-center">
        <ReleasePill />

        <h1 className="mt-7 text-[clamp(40px,6.4vw,72px)] font-medium">
          Multiple agents.
          <br />
          <span className="text-zinc-500">One screen.</span>
        </h1>

        <p className="mx-auto mt-6 max-w-[56ch] text-[clamp(16px,1.7vw,19px)] leading-relaxed text-zinc-400">
          A desktop workspace for Claude Code and Codex. Your own CLIs, your own subscriptions.
        </p>

        <div className="mt-9">
          <DownloadCta />
        </div>

      </Wrap>

      <Wrap className="mt-16 max-w-[1300px]">
        <Reveal>
          <AppWindow />
        </Reveal>
      </Wrap>
    </section>
  )
}

function InstallBand() {
  return (
    <Section id="install" className="pt-4">
      <Wrap>
        {/* Command on the left, copy on the right — the mirror of Subscription below, so the two
            splits alternate instead of reading as the same block twice. On mobile the heading comes
            first; the command is the payload, not the introduction. */}
        {/* minmax(0,…) is load-bearing: a grid item's min-width is `auto`, so the command block
            would otherwise refuse to shrink below the min-content width of the longest install
            command and swallow the row. */}
        <div className="grid items-center gap-10 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)] lg:gap-14">
          <Reveal className="order-2 min-w-0 lg:order-1">
            <Install />
          </Reveal>
          <Reveal className="order-1 lg:order-2" delay={80}>
            {/* no eyebrow: "One command. No account." already says what this section is, and the
                #install anchor does the navigational work a label would. */}
            <h2 className="text-[clamp(26px,3.2vw,36px)] leading-[1.15] font-medium">
              One command. No account.
            </h2>
            <p className="mt-4 max-w-[44ch] text-[16px] leading-relaxed text-zinc-400">
              Paste one line into a terminal and the signed installer does the rest. There is no
              account to make. You sign into the agent CLIs you already pay for, and HyprSpace runs
              those.
            </p>
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
        <div className="grid grid-cols-[minmax(0,1fr)] items-center gap-16 lg:grid-cols-[0.92fr_1.08fr]">
          <Reveal>
            <Eyebrow>Your subscription</Eyebrow>
            <h2 className="mt-3.5 max-w-[16ch] text-[clamp(28px,3.6vw,42px)] font-medium">
              Your CLI. Your quota. No middleman.
            </h2>
            <p className="mt-4.5 max-w-[46ch] text-[16.5px] text-zinc-400">
              Every thread runs the CLI you already signed into. Nothing stored, nothing resold.
            </p>
            <div className="mt-6">
              <Point icon={<ShieldCheck />} title="Nothing resold">
                Exactly what your plan gives you. A workspace, not a reseller.
              </Point>
              <Point icon={<BarChart3 />} title="Usage at a glance">
                Limits in the titlebar, read from your own account. Only ever your provider, never
                us.
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
                <span className="text-zinc-100">codex login</span>
                {" "}
                <span className="text-zinc-500"># OpenAI Codex</span>
                {"\n\n"}
                <span className="text-zinc-500">
                  # then open HyprSpace and start a thread. that&apos;s it.
                </span>
              </pre>
            </div>
          </Reveal>
        </div>
      </Wrap>
    </Section>
  )
}

function TwoWays() {
  return (
    <Section id="threads">
      <Wrap>
        <div className="grid items-center gap-16 lg:grid-cols-[1.08fr_0.92fr]">
          <Reveal className="order-2 min-w-0 lg:order-1">
            <TerminalThread />
          </Reveal>
          <Reveal className="order-1 lg:order-2" delay={80}>
            <Eyebrow>Two ways to run</Eyebrow>
            <h2 className="mt-3.5 max-w-[16ch] text-[clamp(28px,3.6vw,42px)] font-medium">
              A chat, or the real CLI.
            </h2>
            <p className="mt-4.5 max-w-[46ch] text-[16.5px] text-zinc-400">
              Each thread runs one way or the other. Pick it in the composer.
            </p>
            <div className="mt-6">
              <Point icon={<MessagesSquare />} title="Structured">
                HyprSpace drives the CLI over its machine protocol and draws the run as a chat. Tool
                calls fold into one line, and each run ends with the files it changed and an undo.
              </Point>
              <Point icon={<SquareTerminal />} title="Terminal">
                The CLI itself in a real terminal, with its own screen and shortcuts. The sidebar still
                follows what it is doing.
              </Point>
              <Point icon={<MessageSquare />} title="Side chats">
                Ask something with /btw while a run keeps going. The answer waits in the dock, and Bring
                back hands it to the thread&apos;s next message.
              </Point>
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

        {/* bento: every card carries a live miniature of the thing it describes, not an icon */}
        <Reveal className="mt-14" delay={80}>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-6">
            {bento.map(({ span, visual, title, body, link }) => (
              <div
                key={title}
                className={`flex flex-col overflow-hidden rounded-[13px] border border-white/[0.08] bg-[#0c0c0e] ${span}`}
              >
                <div className="border-b border-white/[0.08] bg-[#131317] p-3.5">{visual}</div>
                <div className="px-4 pt-3.5 pb-4">
                  <h3 className="text-[15px] font-semibold">{title}</h3>
                  <p className="mt-1.5 text-[13px] leading-[1.55] text-zinc-400">{body}</p>
                  {link && (
                    <a
                      href={link[1]}
                      className="mt-2.5 inline-flex items-center gap-1.5 text-[13px] font-medium text-zinc-100 underline-offset-[3px] hover:underline"
                    >
                      {link[0]} →
                    </a>
                  )}
                </div>
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
          <div className="mt-8">
            <DownloadCta />
          </div>
          <p className="mt-4 font-mono text-[11.5px] text-zinc-600">free during the beta</p>
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
              ["Source", REPO],
              ["Releases", RELEASES],
              ["Android app", DOWNLOAD_ANDROID],
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
      <div className="site-bg" aria-hidden />
      <div className="site-glow" aria-hidden />

      <a
        href="#top"
        className="fixed top-[-60px] left-3.5 z-50 rounded-md bg-zinc-50 px-3.5 py-2.5 font-semibold text-zinc-950 transition-[top] focus:top-3.5"
      >
        Skip to content
      </a>

      <Nav />

      <main id="top">
        <Hero />
        <InstallBand />
        <Subscription />
        <TwoWays />
        <Workspace />
        <Closing />
      </main>

      <Footer />
    </>
  )
}
