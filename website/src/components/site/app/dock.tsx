import type { ReactNode } from "react"
import { ArrowUp, Check, FileCode, FolderTree, GitBranch, MessageSquare, X } from "lucide-react"
import { cn } from "@/lib/utils"
import { Counts } from "./bits"

const FILES: { name: string; dir: string; added: number; removed: number; status: string; ticked: boolean }[] = [
  { name: "terminal.rs", dir: "crates/engine/src", added: 18, removed: 4, status: "M", ticked: true },
  { name: "hooks.rs", dir: "crates/engine/src", added: 6, removed: 2, status: "M", ticked: true },
  { name: "resume.rs", dir: "crates/engine/tests", added: 40, removed: 6, status: "A", ticked: false },
]

export function Tick({ on, some }: { on?: boolean; some?: boolean }) {
  return (
    <span
      className={cn(
        "flex size-[15px] shrink-0 items-center justify-center rounded-[4px] border",
        on || some ? "border-(--accent) bg-(--accent)" : "border-[rgb(116_116_116/0.55)]",
      )}
    >
      {on && <Check className="size-[11px] text-(--on-accent)" strokeWidth={3} />}
      {some && <i className="block h-[1.5px] w-[7px] bg-(--on-accent)" />}
    </span>
  )
}

export function GitFiles() {
  return (
    <div className="px-1.5 py-1">
      {FILES.map((f) => (
        <div key={f.name} className="flex h-7 items-center gap-2 rounded-md px-1.5">
          <Tick on={f.ticked} />
          <span className="flex min-w-0 flex-1 items-center gap-[7px]">
            <FileCode className="size-3.5 shrink-0 text-[#f2ad73]" />
            <span className="truncate text-[12.5px]">{f.name}</span>
            <span className="min-w-0 flex-1 truncate text-[11px] text-(--t3)">{f.dir}</span>
          </span>
          <Counts added={f.added} removed={f.removed} className="gap-[5px] text-[10.5px]" />
          <span
            className={cn(
              "w-3 text-center font-app-mono text-[10.5px] font-semibold",
              f.status === "A" ? "text-(--ok)" : "text-(--busy)",
            )}
          >
            {f.status}
          </span>
        </div>
      ))}
    </div>
  )
}

export function CommitBox() {
  return (
    <div className="flex flex-col gap-2 p-3">
      <div className="flex flex-col rounded-[10px] bg-white/[0.04] text-[12.5px]">
        <div className="px-2.5 pt-2 pb-1.5 font-medium">Resume a conversation after a restart</div>
        <div className="min-h-12 px-2.5 pb-2 text-(--t3)">Description</div>
      </div>
      <div className="flex gap-1.5">
        <span className="flex h-[26px] flex-1 items-center justify-center rounded-md bg-(--accent) text-[12px] font-medium text-(--on-accent)">
          Commit to resume-fix
        </span>
        <span className="flex h-[26px] flex-1 items-center justify-center rounded-lg bg-white/[0.06] text-[12px] font-medium">
          Commit and push
        </span>
      </div>
      <div className="text-[11px] text-(--t3)">2 of 3 files ticked</div>
    </div>
  )
}

function Segment({ on, children }: { on?: boolean; children: ReactNode }) {
  return (
    <span
      className={cn(
        "flex h-[26px] min-w-0 items-center gap-1.5 rounded-md px-2.5 text-[12.5px] font-medium",
        on ? "bg-white/[0.1] text-(--t1)" : "text-(--t3)",
      )}
    >
      {children}
    </span>
  )
}

export function Dock() {
  return (
    <aside className="hidden w-[320px] shrink-0 flex-col border-l border-(--b0) xl:flex">
      <div className="flex h-[42px] shrink-0 items-center gap-1 pr-1.5 pl-2">
        <div className="flex min-w-0 items-center gap-0.5 rounded-[9px] bg-white/[0.045] p-[3px]">
          <Segment>
            <FolderTree className="size-[13px] shrink-0" />
            Files
          </Segment>
          <Segment on>
            <GitBranch className="size-[13px] shrink-0" />
            Git
            <span className="font-app-mono text-[11px] text-(--t2)">3</span>
          </Segment>
          <Segment>
            <MessageSquare className="size-[13px] shrink-0" />
            <span className="truncate">Why did the test hang</span>
          </Segment>
        </div>
        <span className="flex-1" />
        <span className="flex size-[26px] shrink-0 items-center justify-center text-(--t3)">
          <X className="size-3.5" />
        </span>
      </div>

      <div className="flex h-9 shrink-0 items-center gap-[7px] px-3">
        <GitBranch className="size-[13px] text-(--t3)" />
        <span className="flex-1 truncate font-app-mono text-[12px]">resume-fix</span>
        <span className="flex h-6 items-center gap-1 rounded-md bg-white/[0.07] px-[9px] text-[11.5px] font-medium">
          <ArrowUp className="size-3" />
          Push 2
        </span>
      </div>

      <div className="flex h-8 shrink-0 items-center px-3">
        <span className="flex items-center gap-2 text-[12px] text-(--t2)">
          <Tick some />3 changed files
        </span>
        <span className="flex-1" />
        <Counts added={64} removed={12} className="text-[11px]" />
      </div>

      <div className="flex-1">
        <GitFiles />
      </div>
      <CommitBox />
    </aside>
  )
}
