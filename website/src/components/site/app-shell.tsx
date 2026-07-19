import type { ReactNode } from "react"
import {
  Bell,
  ChevronDown,
  FolderOpen,
  GitBranch,
  LayoutGrid,
  PanelRight,
  Plus,
  Repeat2,
  Search,
  Settings,
} from "lucide-react"
import { AppIcon } from "./primitives"

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

function Titlebar() {
  return (
    <div className="flex h-11 items-center gap-2.5 border-b border-white/[0.07] bg-[#0e0e11] px-3.5">
      <AppIcon className="size-4" />
      <span className="text-[13.5px] font-semibold text-zinc-100">HyprSpace</span>

      <div className="ml-auto flex items-center gap-1.5">
        <TitlebarBtn icon={<Plus className="size-3.5" />} label="New" />
        <TitlebarBtn icon={<FolderOpen className="size-3.5" />} label="Open" caret />
        <TitlebarBtn icon={<GitBranch className="size-3.5" />} label="Commit & push" caret />
        <span className="mx-0.5 flex items-center gap-2 text-zinc-600">
          <LayoutGrid className="size-4" />
          <Bell className="size-4" />
          <PanelRight className="size-4" />
        </span>
        <span className="ml-1 flex items-center gap-2 text-zinc-600">
          <i className="block h-px w-2.5 bg-current" />
          <i className="block size-2 border border-current" />
          <i className="block text-[11px] leading-none">✕</i>
        </span>
      </div>
    </div>
  )
}

function TitlebarBtn({
  icon,
  label,
  caret,
}: {
  icon: ReactNode
  label: string
  caret?: boolean
}) {
  return (
    <span className="flex items-center gap-1.5 rounded-md border border-white/[0.08] bg-white/[0.03] px-2.5 py-1 text-[12.5px] text-zinc-300">
      <span className="text-zinc-400">{icon}</span>
      {label}
      {caret && <ChevronDown className="size-3 text-zinc-500" />}
    </span>
  )
}

const panes = [
  { name: "Claude", dot: "bg-emerald-400" },
  { name: "Codex", dot: "bg-emerald-400" },
  { name: "Grok", dot: "bg-zinc-500" },
]

function Rail() {
  return (
    <aside className="hidden w-[208px] shrink-0 flex-col border-r border-white/[0.07] bg-[#0c0c0e] py-2.5 lg:flex">
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
      </div>

      <RailHeading>Projects</RailHeading>
      <RailHeading>Open spaces</RailHeading>

      {/* the active space, expanded to its three panes */}
      <div className="mx-2 mt-1 flex items-center gap-2 rounded-md bg-white/[0.05] px-2 py-1.5">
        <LayoutGrid className="size-3.5 text-zinc-400" />
        <span className="text-[12.5px] font-medium text-zinc-100">acme-app</span>
        <span className="ml-auto rounded bg-white/[0.07] px-1.5 text-[11px] text-zinc-400">3</span>
      </div>

      <div className="mt-0.5 grid gap-0.5 px-2">
        {panes.map((p) => (
          <div key={p.name} className="flex items-center gap-2 rounded-md py-1 pl-4 pr-2">
            <span className={`size-1.5 shrink-0 rounded-full ${p.dot}`} />
            <span className="text-[12.5px] text-zinc-400">{p.name}</span>
            <span className="ml-auto text-[11px] text-zinc-600">now</span>
          </div>
        ))}
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
