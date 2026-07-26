import type { ReactNode } from "react"
import {
  Bell,
  Folder,
  GitBranch,
  Layers,
  LayoutGrid,
  PanelRight,
  Plus,
  Repeat2,
  Search,
  Settings,
  Zap,
} from "lucide-react"
import { AppIcon } from "./primitives"
import claudeIcon from "@/assets/brand/claude.svg"
import openaiIcon from "@/assets/brand/openai.svg"
import grokIcon from "@/assets/brand/grok.svg"

/**
 * The HyprSpace window, rebuilt in DOM: titlebar → left rail → pane grid.
 * Mirrors the real app chrome so the hero reads as the product, not a generic terminal.
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

/** the live Claude usage ring that sits in the real titlebar */
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
    <div className="flex h-11 items-center gap-1 border-b border-white/[0.07] bg-[#0e0e11] px-2.5">
      {/* left: just the mark and the sidebar toggle — no wordmark in the real app */}
      <span className="flex size-7 items-center justify-center">
        <AppIcon className="size-4" />
      </span>
      <span className="flex size-7 items-center justify-center text-zinc-500">
        <PanelRight className="size-4 scale-x-[-1]" />
      </span>

      {/* right: icon-only actions, then the usage ring, then window controls */}
      <div className="ml-auto flex items-center gap-0.5 text-zinc-500">
        <TbIcon>
          <Plus className="size-4" />
        </TbIcon>
        <TbIcon>
          <GitBranch className="size-4" />
        </TbIcon>
        <TbIcon>
          <Zap className="size-4" />
        </TbIcon>
        <TbIcon>
          <LayoutGrid className="size-4" />
        </TbIcon>

        <i className="mx-1.5 block h-4 w-px bg-white/[0.1]" />

        <TbIcon>
          <UsageRing />
        </TbIcon>
        <TbIcon>
          <Bell className="size-4" />
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

/* the rail's agent rows are two lines in the real app: name + time, then what it's doing */
const agents = [
  { icon: claudeIcon, name: "Claude", act: "Edit src/pty.rs", dot: "bg-amber-400", time: "now" },
  { icon: openaiIcon, name: "Codex", act: "Ran cargo test resume", dot: "bg-amber-400", time: "now" },
  { icon: grokIcon, name: "Grok", act: "Turn completed in 9.1s", dot: "bg-emerald-400", time: "1m" },
]

function Rail() {
  return (
    <aside className="hidden w-[228px] shrink-0 flex-col border-r border-white/[0.07] bg-[#0c0c0e] py-2.5 lg:flex">
      <div className="mx-2 flex items-center gap-2 rounded-md border border-white/[0.07] px-2 py-1.5">
        <Search className="size-3.5 text-zinc-500" />
        <span className="text-[12.5px] text-zinc-500">Search</span>
        <span className="ml-auto rounded border border-white/[0.08] px-1 font-mono text-[10.5px] text-zinc-600">
          Ctrl K
        </span>
      </div>

      <div className="mx-2 mt-1.5 flex items-center gap-2 rounded-md px-2 py-1.5">
        <Repeat2 className="size-4 text-zinc-400" />
        <span className="text-[13px] text-zinc-300">Automations</span>
        <span className="ml-auto text-[11px] text-zinc-600">3</span>
      </div>

      <RailHeading>Projects</RailHeading>

      {/* the active project, expanded: branch tier, then its agents */}
      <div className="mx-2 mt-1 flex items-center gap-2 rounded-md bg-white/[0.05] px-2 py-1.5">
        <Folder className="size-3.5 text-zinc-400" />
        <span className="text-[12.5px] font-medium text-zinc-100">acme-app</span>
        <span className="ml-auto text-[11px] text-zinc-500">3</span>
      </div>

      <div className="mt-0.5 flex items-center gap-1.5 py-1 pr-2 pl-6">
        <GitBranch className="size-3 shrink-0 text-zinc-600" />
        {/* the space's own branch reads brighter than its worktrees, with a small dot */}
        <span className="text-[11.5px] text-zinc-400">main</span>
        <span className="size-1 shrink-0 rounded-full bg-zinc-400" />
        <span className="ml-auto text-[11px] text-zinc-600">3</span>
      </div>

      <div className="grid gap-0.5 px-2">
        {agents.map((a) => (
          <div key={a.name} className="flex items-start gap-2 rounded-md py-1 pr-2 pl-6">
            <span className="relative mt-[3px] flex size-3.5 shrink-0 items-center justify-center">
              <img src={a.icon} alt="" className="size-3.5 opacity-80" />
              <span
                className={`absolute -right-0.5 -bottom-0.5 size-1.5 rounded-full ring-2 ring-[#0c0c0e] ${a.dot}`}
              />
            </span>
            <span className="flex min-w-0 flex-col">
              <span className="text-[12.5px] leading-tight text-zinc-200">{a.name}</span>
              <span className="truncate text-[10.5px] leading-tight text-zinc-600">{a.act}</span>
            </span>
            <span className="mt-px ml-auto shrink-0 text-[11px] text-zinc-600">{a.time}</span>
          </div>
        ))}
      </div>

      <RailHeading>Open spaces</RailHeading>
      <div className="mx-2 mt-1 flex items-center gap-2 rounded-md px-2 py-1.5">
        <Layers className="size-3.5 text-zinc-500" />
        <span className="text-[12.5px] text-zinc-400">scratch</span>
        <span className="ml-auto text-[11px] text-zinc-600">2</span>
      </div>

      <div className="mt-auto flex items-center gap-2 border-t border-white/[0.07] px-4 pt-2.5">
        <Settings className="size-3.5 text-zinc-500" />
        <span className="text-[12.5px] text-zinc-500">Settings</span>
      </div>
    </aside>
  )
}

function RailHeading({ children }: { children: ReactNode }) {
  return (
    <div className="mt-3.5 flex items-center px-4">
      <span className="text-[10.5px] font-medium tracking-[0.14em] text-zinc-600 uppercase">
        {children}
      </span>
      <Plus className="ml-auto size-3 text-zinc-600" />
    </div>
  )
}
