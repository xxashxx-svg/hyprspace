import type { ReactNode } from "react"
import { ChevronRight, GitBranch, LayoutGrid, PanelRight, Plus, Search, Settings } from "lucide-react"
import { AppIcon } from "./primitives"
import claudeIcon from "@/assets/brand/claude.svg"
import openaiIcon from "@/assets/brand/openai.svg"
import grokIcon from "@/assets/brand/grok.svg"

/**
 * The HyprSpace window, rebuilt in DOM: titlebar → sidebar → pane grid.
 * Mirrors the real app chrome so the hero reads as the product, not a generic terminal. Kept in
 * step with the app itself: one flat background split by hairlines, and a sidebar that is one list
 * of spaces, each folding open to its threads.
 */
export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-white/[0.12] bg-[#0a0a0c] shadow-[0_40px_90px_-24px_rgba(0,0,0,0.85),inset_0_1px_0_rgba(255,255,255,0.04)]">
      <Titlebar />
      <div className="flex">
        <Rail />
        <div className="min-w-0 flex-1 p-2">{children}</div>
      </div>
    </div>
  )
}

/** the live usage ring that sits in the real titlebar */
function UsageRing({ pct = 34 }: { pct?: number }) {
  const r = 6.6
  const c = 2 * Math.PI * r
  return (
    <svg viewBox="0 0 16 16" className="size-4 -rotate-90">
      <circle cx="8" cy="8" r={r} fill="none" strokeWidth="2.4" stroke="rgba(255,255,255,0.26)" />
      <circle
        cx="8"
        cy="8"
        r={r}
        fill="none"
        strokeWidth="2.4"
        strokeLinecap="round"
        stroke="#f5f5f5"
        strokeDasharray={`${((c * pct) / 100).toFixed(1)} ${c.toFixed(1)}`}
      />
    </svg>
  )
}

function Titlebar() {
  return (
    <div className="flex h-11 items-center gap-1 border-b border-white/[0.05] bg-[#0a0a0c] px-2.5">
      {/* left: just the mark and the sidebar toggle — no wordmark in the real app */}
      <span className="flex size-7 items-center justify-center">
        <AppIcon className="size-4" />
      </span>
      <span className="flex size-7 items-center justify-center text-zinc-500">
        <PanelRight className="size-4 scale-x-[-1]" />
      </span>

      {/* right: new thread, layout, the usage ring, the dock toggle, then window controls */}
      <div className="ml-auto flex items-center gap-0.5 text-zinc-500">
        <TbIcon>
          <Plus className="size-4" />
        </TbIcon>
        <TbIcon>
          <LayoutGrid className="size-4" />
        </TbIcon>

        <i className="mx-1.5 block h-4 w-px bg-white/[0.1]" />

        <TbIcon>
          <UsageRing />
        </TbIcon>
        <TbIcon>
          <PanelRight className="size-4" />
        </TbIcon>

        <span className="ml-1.5 flex items-center gap-2.5 text-zinc-600">
          <i className="block h-px w-2.5 bg-current" />
          <i className="block size-2 border border-current" />
          <i className="block text-[11px] leading-none">✕</i>
        </span>
      </div>
    </div>
  )
}

function TbIcon({ children }: { children: ReactNode }) {
  return <span className="flex size-7 items-center justify-center">{children}</span>
}

/* a thread row is three lines in the real app: agent + time, the task, then where it is running */
const threads = [
  {
    icon: claudeIcon,
    model: "Opus 5",
    title: "implement the session resume endpoint",
    where: "main",
    time: "now",
    tone: "work" as const,
  },
  {
    icon: openaiIcon,
    model: "gpt-5.6-sol",
    title: "cover the resume path with tests",
    where: "main",
    time: "4m",
    tone: "done" as const,
  },
  {
    icon: grokIcon,
    model: "Grok 4.5",
    title: "document the resume behaviour",
    where: "main",
    time: "1m",
    tone: "wait" as const,
  },
]

const TONE = { work: "bg-amber-400", wait: "bg-blue-400", done: "bg-emerald-400" }

function Rail() {
  return (
    <aside className="hidden w-[240px] shrink-0 flex-col border-r border-white/[0.05] bg-[#0a0a0c] py-2 lg:flex">
      <div className="mx-2 flex items-center gap-2 rounded-lg border border-white/[0.07] bg-white/[0.02] px-2 py-1.5">
        <Search className="size-3.5 text-zinc-500" />
        <span className="text-[12.5px] text-zinc-500">Search threads</span>
        <span className="ml-auto rounded border border-white/[0.08] px-1 font-mono text-[10.5px] text-zinc-600">
          Ctrl K
        </span>
      </div>

      {/* the space you are in: its header, the working tree line, then its threads */}
      <div className="mt-1.5 px-2">
        <div className="flex h-[30px] items-center gap-1 rounded-md px-1">
          <ChevronRight className="size-3 rotate-90 text-zinc-600" />
          <span className="text-[13px] font-semibold text-zinc-100">hyprspace</span>
          <Plus className="ml-auto size-3.5 text-zinc-600" />
        </div>

        <div className="flex items-center gap-1.5 px-2 pt-0.5 pb-1 font-mono text-[10.5px] text-zinc-600">
          <span>3 files</span>
          <span className="text-emerald-400/80">+64</span>
          <span className="text-red-400/80">−12</span>
        </div>

        <div className="grid gap-0.5 pl-3">
          {threads.map((t, i) => (
            <div
              key={t.title}
              className={`grid gap-[3px] rounded-lg px-2 py-1.5 ${i === 0 ? "bg-white/[0.05]" : ""}`}
            >
              <div className="flex items-center gap-1.5">
                <img src={t.icon} alt="" className="size-3 opacity-80" />
                <span className="font-mono text-[10.5px] text-zinc-500">{t.model}</span>
                <span className="ml-auto font-mono text-[10px] text-zinc-600">{t.time}</span>
              </div>
              <div className="truncate text-[12.5px] font-medium text-zinc-200">{t.title}</div>
              <div className="flex items-center gap-1.5 text-[10.5px] text-zinc-600">
                <GitBranch className="size-2.5" />
                <span className="truncate font-mono">{t.where}</span>
                <span className={`ml-auto size-1.5 shrink-0 rounded-full ${TONE[t.tone]}`} />
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* the other spaces, folded shut */}
      {["Agent-Sdk", "website"].map((name) => (
        <div key={name} className="mx-2 flex h-[30px] items-center gap-1 rounded-md px-1">
          <ChevronRight className="size-3 text-zinc-600" />
          <span className="text-[13px] text-zinc-400">{name}</span>
        </div>
      ))}

      <div className="mx-2 mt-1 flex h-[30px] items-center gap-1 rounded-md px-1 text-zinc-600">
        <Plus className="size-3.5" />
        <span className="text-[13px]">Open new thread</span>
      </div>

      <div className="mt-auto flex items-center gap-2 border-t border-white/[0.05] px-4 pt-2.5">
        <Settings className="size-3.5 text-zinc-500" />
        <span className="text-[12.5px] text-zinc-500">Settings</span>
      </div>
    </aside>
  )
}
