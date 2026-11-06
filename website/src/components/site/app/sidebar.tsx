import type { ReactNode } from "react"
import {
  ChevronRight,
  CircleCheck,
  CircleDashed,
  CircleHelp,
  Gauge,
  ListFilter,
  RefreshCw,
  Search,
  Settings,
  SquarePen,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { AgentMark, Tag, type Agent } from "./bits"

export function stopwatch(secs: number) {
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${String(secs % 60).padStart(2, "0")}s`
  return `${Math.floor(secs / 3600)}h ${String(Math.floor(secs / 60) % 60).padStart(2, "0")}m`
}

type Row = {
  space: string
  title: string
  agent: Agent
  state: "working" | "waiting" | "done" | "idle"
  since?: number
  ago?: string
  selected?: boolean
}

export const ROWS: Row[] = [
  {
    space: "hyprspace",
    title: "Keep a conversation through a restart",
    agent: "claude",
    state: "working",
    since: 187,
    selected: true,
  },
  {
    space: "website",
    title: "Rewrite the hero for the new app",
    agent: "codex",
    state: "waiting",
  },
  {
    space: "Agent-Sdk",
    title: "Stream tool results to the phone",
    agent: "claude",
    state: "working",
    since: 64,
  },
  {
    space: "hyprspace",
    title: "Cache the dock view",
    agent: "claude",
    state: "done",
  },
  {
    space: "website",
    title: "Fix the install script on macOS",
    agent: "codex",
    state: "idle",
    ago: "1h",
  },
  {
    space: "Agent-Sdk",
    title: "Draft the 0.3 release notes",
    agent: "claude",
    state: "idle",
    ago: "3h",
  },
]

function Said({ icon, tint, children }: { icon: ReactNode; tint: string; children: ReactNode }) {
  return (
    <span className={cn("flex shrink-0 items-center gap-1 text-[11px] font-medium", tint)}>
      {icon}
      {children}
    </span>
  )
}

function Slot({ row, secs }: { row: Row; secs: number }) {
  switch (row.state) {
    case "working":
      return (
        <Said icon={<CircleDashed className="size-3" />} tint="text-(--busy)">
          <span className="font-app-mono text-[10.5px]">{stopwatch((row.since ?? 0) + secs)}</span>
        </Said>
      )
    case "waiting":
      return (
        <Said icon={<CircleHelp className="size-3" />} tint="text-(--wait)">
          Needs you
        </Said>
      )
    case "done":
      return (
        <Said icon={<CircleCheck className="size-3" />} tint="text-(--ok)">
          Done
        </Said>
      )
    default:
      return <span className="shrink-0 text-[12px] text-(--t2)">{row.ago}</span>
  }
}

export function ThreadRow({ row, secs }: { row: Row; secs: number }) {
  const working = row.state === "working"
  const recede = working && !row.selected
  const quiet = row.state === "idle" && !row.selected
  return (
    <div
      className={cn(
        "relative flex flex-col gap-0.5 overflow-hidden rounded-lg px-[9px] pt-[7px] pb-2",
        row.selected && "bg-(--s3)",
        recede && "opacity-70",
      )}
    >
      {working && (
        <span className="hs-sheen pointer-events-none absolute inset-y-0 flex w-[60%]">
          <i className="flex-1 bg-gradient-to-r from-transparent to-[rgb(245_158_11/0.035)]" />
          <i className="flex-1 bg-gradient-to-r from-[rgb(245_158_11/0.035)] to-transparent" />
        </span>
      )}
      <div className="flex h-5 min-w-0 items-center gap-1.5">
        <Tag space={row.space} />
        <span className="min-w-0 flex-1 truncate text-[11.5px] text-(--t3)">{row.space}</span>
        <Slot row={row} secs={secs} />
      </div>
      <div className="flex min-w-0 items-center gap-1.5">
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[13.5px] text-(--t1)",
            quiet ? "font-normal opacity-75" : "font-medium",
          )}
        >
          {row.title}
        </span>
        <AgentMark agent={row.agent} working={working} />
      </div>
    </div>
  )
}

function Shelf({ label, count }: { label: string; count: number }) {
  return (
    <div className="flex h-[26px] items-center gap-2 px-2 text-[11.5px] font-medium text-(--t3)">
      {label}
      <span className="font-app-mono text-[10.5px]">{count}</span>
      <i className="block h-px flex-1 bg-white/[0.08]" />
      <ChevronRight className="size-3 rotate-90" />
    </div>
  )
}

function Foot({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <span className="flex h-[30px] items-center gap-2 rounded-lg px-[9px] text-[12.5px] text-(--t2)">
      <span className="text-(--t3)">{icon}</span>
      {children}
    </span>
  )
}

export function Sidebar({ secs }: { secs: number }) {
  return (
    <aside className="hidden w-[272px] shrink-0 flex-col border-r border-(--b0) bg-(--s1) lg:flex">
      <div className="mb-1 flex items-center gap-0.5 px-2 py-2.5">
        <div className="flex h-8 min-w-0 flex-1 items-center gap-2 pr-1.5 pl-2.5">
          <Search className="size-3.5 shrink-0 text-(--t3)" />
          <span className="flex-1 text-[13px] text-(--t3)">Search</span>
          <span className="flex h-[18px] items-center rounded-[4px] bg-white/[0.08] px-[5px] text-[10.5px] font-medium text-(--t2)">
            Ctrl K
          </span>
        </div>
        <span className="flex size-7 shrink-0 items-center justify-center text-(--t3)">
          <ListFilter className="size-3.5" />
        </span>
        <span className="flex size-7 shrink-0 items-center justify-center text-(--t2)">
          <SquarePen className="size-[15px]" />
        </span>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-0.5 px-2 pt-0.5">
        {ROWS.map((row) => (
          <ThreadRow key={row.title} row={row} secs={secs} />
        ))}
        <div className="flex flex-col gap-0.5 pt-2.5">
          <Shelf label="Snoozed" count={2} />
          <Shelf label="Settled" count={14} />
        </div>
      </div>

      <div className="flex items-center gap-0.5 px-2 pt-[7px] pb-2">
        <Foot icon={<Settings className="size-[15px]" />}>Settings</Foot>
        <Foot icon={<Gauge className="size-[15px]" />}>Usage</Foot>
        <span className="flex-1" />
        <span className="flex size-7 items-center justify-center text-(--t3)">
          <RefreshCw className="size-[15px]" />
        </span>
      </div>
    </aside>
  )
}
