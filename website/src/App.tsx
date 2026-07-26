import {
  Apple,
  BarChart3,
  Download,
  Github,
  ShieldCheck,
} from "lucide-react"

import { ClaudePane, CodexPane, GrokPane } from "@/components/site/sessions"
import { AppShell } from "@/components/site/app-shell"
import { Nav, usePlatform } from "@/components/site/nav"
import { Install, ReleasePill } from "@/components/site/showcase"
import {
  DiffVignette,
  EditorVignette,
  PaletteVignette,
  StartupVignette,
  SwapVignette,
  TreeVignette,
  UsageVignette,
} from "@/components/site/vignettes"
import { Pane } from "@/components/site/pane"
import {
  AppIcon,
  ButtonLink,
  Eyebrow,
  Point,
  Reveal,
  Section,
  Wrap,
} from "@/components/site/primitives"
import { DOWNLOAD_MAC, DOWNLOAD_WIN, RELEASES, REPO } from "@/site"

import claudeIcon from "@/assets/brand/claude.svg"
import openaiIcon from "@/assets/brand/openai.svg"
import opencodeIcon from "@/assets/brand/opencode.svg"
import grokIcon from "@/assets/brand/grok.svg"

const providers = [
  { icon: claudeIcon, name: "Claude" },
  { icon: openaiIcon, name: "Codex" },
  { icon: opencodeIcon, name: "OpenCode" },
  { icon: grokIcon, name: "Grok" },
]

const bento = [
  {
    span: "lg:col-span-3",
    visual: <TreeVignette />,
    title: "Live agent tree",
    body: "Every agent, what it's doing, and the sub-agents it spawned.",
  },
  {
    span: "lg:col-span-3",
    visual: <UsageVignette />,
    title: "Usage in the titlebar",
    body: "Session and weekly limits, updated as the agents work.",
  },
  {
    span: "lg:col-span-2",
    visual: <PaletteVignette />,
    title: "Command palette",
    body: "Jump to any project, pane, or automation.",
  },
  {
    span: "lg:col-span-2",
    visual: <DiffVignette />,
    title: "Review and ship",
    body: "Review every change, then commit, push, PR.",
  },
  {
    span: "lg:col-span-2",
    visual: <SwapVignette />,
    title: "Tile and swap",
    body: "Drag to rearrange, or drop into another space.",
  },
  {
    span: "lg:col-span-3",
    visual: <EditorVignette />,
    title: "Built-in editor",
    body: "Open, edit and save without leaving the window.",
  },
  {
    span: "lg:col-span-3",
    visual: <StartupVignette />,
    title: "Startup actions",
    body: "Auto-run your dev server when you open a folder.",
  },
]
function Hero() {
  const dl = usePlatform()
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
          Mission control for Claude, Codex, Grok and OpenCode. Your own CLIs, your own subscriptions.
        </p>

        <div className="mt-9 inline-flex flex-wrap items-center justify-center gap-3.5">
          <ButtonLink href={dl.href}>
            <dl.Icon className="size-4" /> {dl.label}
          </ButtonLink>
          <ButtonLink href={dl.other.href} variant="line">
            <dl.other.Icon className="size-4" /> {dl.other.label}
          </ButtonLink>
        </div>

        <div className="mt-9 flex flex-wrap items-center justify-center gap-2">
          {providers.map((p) => (
            <span
              key={p.name}
              className="inline-flex items-center gap-2 rounded-lg border border-white/[0.08] bg-white/[0.02] px-3 py-1.5 font-mono text-[12px] text-zinc-400"
            >
              <img src={p.icon} alt="" className="size-[15px]" />
              {p.name.toLowerCase()}
            </span>
          ))}
        </div>
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

function InstallBand() {
  return (
    <Section id="install" className="pt-4">
      <Wrap className="max-w-[760px]">
        <Reveal className="text-center">
          <Eyebrow>Install</Eyebrow>
          <h2 className="mt-3.5 text-[clamp(24px,3vw,32px)] font-medium">One command. No account.</h2>
        </Reveal>
        <Reveal className="mt-8" delay={80}>
          <Install />
        </Reveal>
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
              Every pane runs the CLI you already signed into. Nothing stored, nothing resold.
            </p>
            <div className="mt-6">
              <Point icon={<ShieldCheck />} title="Nothing resold">
                Exactly what your plan gives you. A workspace, not a reseller.
              </Point>
              <Point icon={<BarChart3 />} title="Usage at a glance">
                Limits in the titlebar, read locally. Nothing of yours leaves the machine.
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

        {/* bento: every card carries a live miniature of the thing it describes, not an icon */}
        <Reveal className="mt-14" delay={80}>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-6">
            {bento.map(({ span, visual, title, body }) => (
              <div
                key={title}
                className={`flex flex-col overflow-hidden rounded-[13px] border border-white/[0.08] bg-[#0c0c0e] ${span}`}
              >
                <div className="border-b border-white/[0.08] bg-[#131317] p-3.5">{visual}</div>
                <div className="px-4 pt-3.5 pb-4">
                  <h3 className="text-[15px] font-semibold">{title}</h3>
                  <p className="mt-1.5 text-[13px] leading-[1.55] text-zinc-400">{body}</p>
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
        <Workspace />
        <Closing />
      </main>

      <Footer />
    </>
  )
}
