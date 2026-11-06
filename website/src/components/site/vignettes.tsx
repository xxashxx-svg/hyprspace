import type { ReactNode } from "react"
import { ArrowUp, ChevronDown, ChevronRight, Hand, Paperclip, Sparkles } from "lucide-react"
import { cn } from "@/lib/utils"
import { Mark, UsageRing, useSeconds } from "./app/bits"
import { CommitBox, GitFiles } from "./app/dock"
import { ROWS, ThreadRow } from "./app/sidebar"

function Panel({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cn("app overflow-hidden rounded-[10px] border border-white/[0.08]", className)}>{children}</div>
}

export function ThreadsVignette() {
  const secs = useSeconds()
  return (
    <Panel className="flex flex-col gap-0.5 p-1.5">
      {ROWS.slice(1, 4).map((row) => (
        <ThreadRow key={row.title} row={row} secs={secs} />
      ))}
    </Panel>
  )
}

function Limit({ name, when, left }: { name: string; when: string; left: number }) {
  return (
    <div className="flex items-center gap-3 px-3.5 py-2.5">
      <UsageRing pct={100 - left} color="#d97757" size={22} />
      <div className="flex min-w-0 flex-1 flex-col gap-px">
        <span className="truncate text-[13px] font-medium">{name}</span>
        <span className="truncate text-[11.5px] text-(--t3)">{when}</span>
      </div>
      <div className="flex flex-col items-end">
        <span className="text-[15px] font-semibold">{left}%</span>
        <span className="text-[10.5px] text-(--t3)">left</span>
      </div>
    </div>
  )
}

export function UsageVignette() {
  return (
    <Panel className="bg-(--s2)">
      <div className="flex h-[38px] items-center gap-0.5 pr-2.5 pl-1.5">
        <span className="flex items-center gap-1.5 rounded-md bg-white/[0.08] px-2.5 py-1.5 text-[12.5px] font-medium">
          <Mark agent="claude" />
          Claude
        </span>
        <span className="flex items-center gap-1.5 px-2.5 py-1.5 text-[12.5px] font-medium text-(--t3)">
          <span className="opacity-60">
            <Mark agent="codex" />
          </span>
          Codex
        </span>
        <span className="flex-1" />
        <span className="rounded-full bg-white/[0.06] px-[7px] py-px text-[10.5px] font-medium text-(--t2)">Max</span>
      </div>
      <Limit name="Session · 5h" when="Resets 6:40 PM · in 2h 41m" left={66} />
      <Limit name="This week" when="Resets Mon 9:00 AM · in 4d" left={46} />
    </Panel>
  )
}

export function PaletteVignette() {
  return (
    <Panel className="bg-(--s2) p-1.5 text-[12.5px]">
      <div className="flex items-center gap-1 px-2 pt-1 pb-2 text-(--t2)">
        resume
        <i className="hs-blink block h-3.5 w-px bg-(--t2)" />
      </div>
      <div className="pt-1.5">
        {[
          ["Keep a conversation through a restart", "hyprspace"],
          ["crates/engine/src/terminal.rs", "file"],
          ["Resume the last thread", "action"],
        ].map(([label, hint], i) => (
          <div key={label} className={cn("flex h-7 items-center gap-2 rounded-md px-2", i === 0 && "bg-white/[0.08]")}>
            <span className="min-w-0 flex-1 truncate">{label}</span>
            <span className="shrink-0 text-[11px] text-(--t3)">{hint}</span>
          </div>
        ))}
      </div>
    </Panel>
  )
}

export function GitVignette() {
  return (
    <Panel>
      <GitFiles />
      <CommitBox />
    </Panel>
  )
}

function MenuItem({ children, on, sub }: { children: ReactNode; on?: boolean; sub?: boolean }) {
  return (
    <div className={cn("flex h-7 items-center gap-2 rounded-md px-2 text-[12.5px]", on && "bg-white/[0.08]")}>
      <span className="flex-1 truncate">{children}</span>
      {sub && <ChevronRight className="size-3 text-(--t3)" />}
    </div>
  )
}

const menu = "rounded-[12px] border border-white/[0.1] bg-(--s2) p-[5px] shadow-[0_12px_32px_-8px_rgba(0,0,0,0.7)]"

export function WorktreeVignette() {
  return (
    <div className="app relative h-[172px] overflow-hidden rounded-[10px] border border-white/[0.08] p-3">
      <div className={cn(menu, "w-[170px]")}>
        <MenuItem>Settle thread</MenuItem>
        <MenuItem sub>Snooze</MenuItem>
        <i className="mx-1.5 my-1 block h-px bg-white/[0.06]" />
        <MenuItem>Rename thread</MenuItem>
        <MenuItem on sub>
          Worktree
        </MenuItem>
      </div>
      <div className={cn(menu, "absolute top-[72px] left-[150px] w-[170px]")}>
        <MenuItem>Merge into main</MenuItem>
        <MenuItem on>Create pull request</MenuItem>
        <MenuItem>Remove worktree</MenuItem>
      </div>
    </div>
  )
}

export function PhoneVignette() {
  return (
    <div className="app flex h-[172px] justify-center overflow-hidden rounded-[10px] border border-white/[0.08] pt-4">
      <div className="w-[176px] rounded-t-[22px] border-x border-t border-white/[0.14] bg-[#101010] px-2.5 pt-3">
        <div className="mx-auto mb-3 h-1 w-10 rounded-full bg-white/[0.14]" />
        <div className="rounded-xl border border-(--b1) bg-white/[0.035] p-2.5">
          <div className="flex items-center gap-1.5 text-[10.5px] font-medium text-(--wait)">
            <Hand className="size-[11px]" />
            Needs your OK
          </div>
          <div className="mt-1 truncate text-[12px] font-medium">Run bun run build</div>
          <div className="mt-2.5 flex gap-1.5">
            <span className="flex h-6 flex-1 items-center justify-center rounded-md bg-white/[0.06] text-[11px]">
              Deny
            </span>
            <span className="flex h-6 flex-1 items-center justify-center rounded-md bg-(--accent) text-[11px] text-(--on-accent)">
              Allow
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}

export function EditorVignette() {
  const k = "text-[#c8aefa]"
  const f = "text-[#86c3ff]"
  const s = "text-[#90d192]"
  return (
    <Panel>
      <div className="flex h-8 items-center gap-2 px-3 text-[12px]">
        <span className="truncate">crates/engine/src/hooks.rs</span>
        <i className="block size-1.5 rounded-full bg-(--t2)" />
        <span className="flex-1" />
        <span className="text-[11px] text-(--t3)">Ctrl+S</span>
      </div>
      <pre className="px-3 py-2.5 font-app-mono text-[11px] leading-[1.85] text-[#e4e4e4]">
        <span className={k}>fn</span> <span className={f}>post</span>(port: u16, body: &amp;str) {"{\n"}
        {"    "}
        <span className={k}>let</span> addr = <span className={f}>local</span>(port);{"\n"}
        {"    "}
        <span className={f}>send</span>(addr, <span className={s}>"application/json"</span>, body){"\n"}
        {"}"}
      </pre>
    </Panel>
  )
}

export function ComposerVignette() {
  const chip = "flex h-[26px] shrink-0 items-center gap-1.5 rounded-lg px-2 text-[12px] whitespace-nowrap"
  return (
    <Panel className="p-3">
      <div className="mb-3 text-center text-[17px] font-medium">What should we build in hyprspace?</div>
      <div className="rounded-[18px] bg-white/[0.045]">
        <div className="min-h-[52px] px-4 pt-3 text-[13px] leading-[21px] text-(--t2)">
          Add a reconnect test for the phone bridge
          <i className="hs-blink ml-px inline-block h-3.5 w-px translate-y-0.5 bg-(--t2)" />
        </div>
        <div className="flex items-center gap-0.5 px-1.5 pb-1.5 text-(--t3)">
          <span className="flex size-7 items-center justify-center">
            <Paperclip className="size-3.5" />
          </span>
          <span className={cn(chip, "bg-white/[0.1] text-(--t1)")}>
            <Sparkles className="size-3" />
            Structured
          </span>
          <span className="flex-1" />
          <span className={cn(chip, "text-(--t1)")}>
            <Mark agent="claude" />
            Opus 5.5
            <ChevronDown className="size-3 text-(--t3)" />
          </span>
          <span className="ml-1 flex size-7 items-center justify-center rounded-full bg-(--t1)">
            <ArrowUp className="size-[15px] text-(--bg)" />
          </span>
        </div>
      </div>
    </Panel>
  )
}
