import type { ReactNode } from "react"
import { Plus, X } from "lucide-react"
import { cn } from "@/lib/utils"

type Status = "running" | "waiting" | "done"

const dot: Record<Status, string> = {
  running: "bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.6)]",
  waiting: "bg-blue-400 shadow-[0_0_8px_rgba(96,165,250,0.6)]",
  done: "bg-emerald-400",
}

/**
 * One HyprSpace pane. Every pane is a tab strip now — the active tab, a Chrome-style new-tab
 * button, and the folder on the right when it differs from the tab's name.
 */
export function Pane({
  icon,
  name,
  folder,
  status = "running",
  className,
  children,
}: {
  icon: string
  name: string
  folder: string
  status?: Status
  className?: string
  children: ReactNode
}) {
  return (
    <div
      className={cn(
        "flex min-w-0 flex-col overflow-hidden rounded-lg border border-white/[0.08] bg-[#0c0c0e]",
        className,
      )}
    >
      <div className="flex h-9 shrink-0 items-center gap-1 border-b border-white/[0.06] px-1.5">
        {/* the active tab */}
        <span className="flex min-w-0 items-center gap-2 rounded-md bg-white/[0.06] px-2.5 py-1">
          <img src={icon} alt="" className="size-3.5 shrink-0" />
          <span className="truncate text-[12.5px] text-zinc-100">{name}</span>
          <span className={cn("size-1.5 shrink-0 rounded-full", dot[status])} />
          <X className="size-3 shrink-0 text-zinc-600" />
        </span>

        <span className="flex size-6 shrink-0 items-center justify-center text-zinc-600">
          <Plus className="size-3.5" />
        </span>

        <span className="ml-auto shrink-0 truncate pr-1 text-[11px] text-zinc-600">{folder}</span>
      </div>
      <div className="min-w-0 flex-1 overflow-hidden p-3.5">{children}</div>
    </div>
  )
}
