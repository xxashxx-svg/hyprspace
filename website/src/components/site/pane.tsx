import type { ReactNode } from "react"
import { Columns2, Expand, MoreHorizontal, X } from "lucide-react"
import { cn } from "@/lib/utils"

type Status = "running" | "waiting" | "done"

const dot: Record<Status, string> = {
  running: "bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.7)]",
  waiting: "bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.6)]",
  done: "bg-zinc-500",
}

/**
 * One HyprSpace terminal pane — the app's real pane chrome: agent icon, the pane's
 * given name · its folder, then the split / more / expand / close cluster.
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
      <div className="flex shrink-0 items-center gap-2 border-b border-white/[0.06] px-3 py-2">
        <img src={icon} alt="" className="size-4 shrink-0" />
        <span className="truncate text-[13px] font-semibold text-zinc-100">{name}</span>
        <span className="shrink-0 text-[12px] text-zinc-600">·</span>
        <span className="truncate text-[12px] text-zinc-500">{folder}</span>
        <span className={cn("ml-1 size-1.5 shrink-0 rounded-full", dot[status])} />
        <span className="ml-auto flex shrink-0 items-center gap-2 text-zinc-600">
          <Columns2 className="size-3.5" />
          <MoreHorizontal className="size-3.5" />
          <Expand className="size-3.5" />
          <X className="size-3.5" />
        </span>
      </div>
      <div className="min-w-0 flex-1 overflow-hidden p-3.5">{children}</div>
    </div>
  )
}
