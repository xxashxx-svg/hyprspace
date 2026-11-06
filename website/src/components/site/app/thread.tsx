import type { ReactNode } from "react"
import { ChevronRight, FileDiff, FilePenLine, Folder, GitBranch, Paperclip, RotateCcw } from "lucide-react"
import { cn } from "@/lib/utils"
import { Counts, Keycap, Mark, UsageRing } from "./bits"

function Prompt({ children }: { children: ReactNode }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[80%] rounded-[18px] bg-white/[0.06] px-4 py-2.5 text-[14px] leading-[1.6] text-(--t1)">
        {children}
      </div>
    </div>
  )
}

function Fold({ label, open, right }: { label: string; open?: boolean; right?: ReactNode }) {
  return (
    <div className="flex min-w-0 items-center gap-2 text-[13px] text-(--t3)">
      <span className="flex size-[18px] shrink-0 items-center justify-center">
        <ChevronRight className={cn("size-3", open && "rotate-90")} />
      </span>
      <span className="min-w-0 truncate">{label}</span>
      {right}
    </div>
  )
}

function Reply({ children }: { children: ReactNode }) {
  return <p className="text-[14px] leading-[1.65] text-(--t1)">{children}</p>
}

function Code({ children }: { children: ReactNode }) {
  return (
    <code className="rounded-[5px] bg-white/[0.06] px-[5px] py-px font-app-mono text-[12.5px] text-(--t1)">
      {children}
    </code>
  )
}

const FILES: [string, string, number, number][] = [
  ["crates/engine/src/", "terminal.rs", 18, 4],
  ["crates/engine/src/", "hooks.rs", 6, 2],
  ["crates/engine/tests/", "resume.rs", 40, 6],
]

function Changes() {
  return (
    <div className="overflow-hidden rounded-xl border border-(--b1) bg-white/[0.025]">
      <div className="flex items-center gap-3 px-3.5 py-3">
        <span className="flex size-[34px] shrink-0 items-center justify-center rounded-lg bg-white/[0.06]">
          <FileDiff className="size-4 text-(--t1)" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="text-[13.5px] font-medium">Edited 3 files</span>
          <Counts added={64} removed={12} className="text-[12px]" />
        </div>
        <span className="hidden h-7 items-center gap-1.5 rounded-[7px] px-2.5 text-[12.5px] sm:flex">
          Undo
          <RotateCcw className="size-3 text-(--t2)" />
        </span>
        <span className="flex h-7 items-center rounded-lg bg-white/[0.06] px-3 text-[12.5px] font-medium">
          View changes
        </span>
      </div>
      {FILES.map(([dir, name, a, r]) => (
        <div key={name} className="flex items-center gap-3 border-t border-(--b0) px-3.5 py-[7px] text-[13px]">
          <span className="min-w-0 flex-1 truncate">
            <span className="text-(--t3)">{dir}</span>
            {name}
          </span>
          <Counts added={a} removed={r} className="text-[12px]" />
        </div>
      ))}
    </div>
  )
}

function took(secs: number) {
  return secs < 60 ? `${secs}s` : `${Math.floor(secs / 60)}m ${secs % 60}s`
}

function Working({ secs }: { secs: number }) {
  return (
    <div className="flex h-[22px] items-center gap-2 text-[12.5px] text-(--t3)">
      <span className="hs-breathe shrink-0 font-medium text-(--t2)">Working for {took(38 + secs)}</span>
      <span className="flex-1" />
      <span className="hidden shrink-0 items-center gap-[5px] sm:flex">
        <Keycap>Esc</Keycap>
        to stop
      </span>
    </div>
  )
}

function ReplyBox() {
  const chip = "flex h-[26px] min-w-0 items-center gap-1.5 rounded-lg px-2 text-[12.5px]"
  return (
    <div className="pb-3">
      <div className="rounded-[14px] border border-transparent bg-white/[0.045]">
        <div className="flex flex-col gap-1.5 px-2 pt-3 pb-2">
          <div className="px-2 text-[14px] leading-[22px] text-(--t3)">Queue a message for when it finishes</div>
          <div className="flex items-center gap-1">
            <div className="flex min-w-0 flex-1 items-center gap-1">
              <span className={chip}>
                <Mark agent="claude" />
                <span className="truncate font-medium">Opus 5.5</span>
              </span>
              <span className={cn(chip, "text-(--t3)")}>High</span>
              <span className={cn(chip, "hidden gap-[5px] text-(--t3) sm:flex")}>
                <FilePenLine className="size-3" />
                Auto edit
              </span>
            </div>
            <span className="flex size-7 items-center justify-center text-(--t3)">
              <Paperclip className="size-[15px]" />
            </span>
            <span className="flex size-[30px] items-center justify-center rounded-full bg-white/[0.1]">
              <i className="block size-2.5 rounded-[2px] bg-(--t1)" />
            </span>
          </div>
        </div>
      </div>
      <div className="flex items-center justify-between gap-3 px-3 pt-2 text-[11.5px] text-(--t3)">
        <span className="flex items-center gap-1.5">
          <Folder className="size-3" />
          hyprspace
        </span>
        <span className="flex items-center gap-3.5">
          <span className="flex items-center gap-1.5">
            <GitBranch className="size-3" />
            resume-fix
          </span>
          <span className="flex items-center gap-[5px]">
            <UsageRing pct={34} color="var(--t2)" size={13} />
            34% context
          </span>
        </span>
      </div>
    </div>
  )
}

export function Thread({ secs }: { secs: number }) {
  return (
    <div className="flex min-w-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1 flex-col justify-end overflow-hidden [mask-image:linear-gradient(to_bottom,transparent,#000_56px)]">
        <div className="mx-auto flex w-full max-w-[816px] flex-col gap-5 px-4 pt-6 pb-5 sm:px-6">
          <Prompt>
            When the app restarts mid-run, Claude starts a fresh conversation instead of picking up the old one.
            Make it resume the same session.
          </Prompt>
          <div className="flex flex-col gap-2">
            <Fold label="Thinking" />
            <Fold label="Read 4 files · searched 2 times" />
          </div>
          <Reply>
            The session id is saved from the first status line, so a restart before that line loses it. I&apos;ll
            record it from the <Code>SessionStart</Code> hook instead, which fires before the first prompt.
          </Reply>
          <div className="flex flex-col gap-2">
            <Fold
              label="Edit crates/engine/src/terminal.rs, hooks.rs"
              right={<Counts added={24} removed={6} className="gap-1 text-[13px]" />}
            />
            <Fold label="Run cargo test -p hyprspace-engine resume" open />
            <pre className="ml-[26px] overflow-hidden rounded-[10px] bg-white/[0.035] px-3 py-2 font-app-mono text-[11.5px] leading-[17px] text-(--t2)">
              {`running 3 tests
test resume::keeps_the_session ... ok
test resume::ignores_other_threads ... ok
test resume::survives_a_restart ... ok

test result: ok. 3 passed; 0 failed`}
            </pre>
          </div>
          <Reply>The id is written as soon as the session starts, and a restart now resumes the same conversation.</Reply>
          <Changes />
          <Prompt>Also cover a thread that was snoozed when the app closed.</Prompt>
          <Working secs={secs} />
        </div>
      </div>
      <div className="mx-auto w-full max-w-[816px] px-4 sm:px-6">
        <ReplyBox />
      </div>
    </div>
  )
}
