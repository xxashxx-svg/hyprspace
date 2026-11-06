import type { ReactNode } from "react"
import { ArrowLeft, ArrowRight, ChevronDown, PanelLeft, PanelRight, Plus } from "lucide-react"
import vscode from "@/assets/app/vscode.svg"
import { AppIcon } from "../primitives"
import { Tag, UsageRing, useSeconds } from "./bits"
import { Dock } from "./dock"
import { Sidebar } from "./sidebar"
import { Thread } from "./thread"

function BarButton({ on, children }: { on?: boolean; children: ReactNode }) {
  return (
    <span
      className={`flex size-7 shrink-0 items-center justify-center rounded-md ${on ? "bg-(--s2) text-(--t1)" : "text-(--t2)"}`}
    >
      {children}
    </span>
  )
}

function Caption() {
  const cell = "hidden h-full w-[46px] items-center justify-center text-(--t2) sm:flex"
  return (
    <div className="flex h-full shrink-0">
      <span className={cell}>
        <svg viewBox="0 0 10 10" className="size-2.5" fill="none" stroke="currentColor">
          <path d="M0 5.5h10" />
        </svg>
      </span>
      <span className={cell}>
        <svg viewBox="0 0 10 10" className="size-2.5" fill="none" stroke="currentColor">
          <rect x="0.5" y="0.5" width="9" height="9" />
        </svg>
      </span>
      <span className={cell}>
        <svg viewBox="0 0 10 10" className="size-2.5" fill="none" stroke="currentColor">
          <path d="M0 0l10 10M10 0L0 10" />
        </svg>
      </span>
    </div>
  )
}

function Titlebar() {
  return (
    <div className="flex h-10 shrink-0">
      <div className="hidden w-[272px] shrink-0 items-center gap-0.5 border-r border-(--b0) bg-(--s1) pl-2 lg:flex">
        <BarButton>
          <PanelLeft className="size-[15px]" />
        </BarButton>
        <span className="flex h-[26px] items-center gap-1.5 px-1.5 text-[13px] font-medium">
          <AppIcon className="size-3.5" />
          HyprSpace
        </span>
        <span className="flex size-[22px] items-center justify-center text-(--t2)">
          <ArrowLeft className="size-3.5" />
        </span>
        <span className="flex size-[22px] items-center justify-center text-(--t3) opacity-40">
          <ArrowRight className="size-3.5" />
        </span>
      </div>
      <div className="flex min-w-0 flex-1 items-center gap-2 pr-1.5 pl-3.5">
        <Tag space="hyprspace" />
        <span className="hidden shrink-0 text-[13px] text-(--t2) sm:block">hyprspace</span>
        <span className="hidden shrink-0 text-[13px] text-(--t3) sm:block">/</span>
        <span className="min-w-0 shrink truncate text-[13px] font-medium">
          Keep a conversation through a restart
        </span>
        <span className="flex-1" />
        <div className="flex shrink-0 items-center gap-1.5">
          <BarButton>
            <Plus className="size-3.5" />
          </BarButton>
          <span className="hidden h-[26px] items-center overflow-hidden rounded-md border border-(--b1) sm:flex">
            <span className="flex h-full items-center gap-1.5 pr-2 pl-[7px] text-[12.5px] font-medium">
              <img src={vscode} alt="" className="size-3.5" />
              Open
            </span>
            <span className="flex h-full w-5 items-center justify-center border-l border-(--b1) text-(--t3)">
              <ChevronDown className="size-[13px]" />
            </span>
          </span>
          <i className="mx-2 hidden h-4 w-px bg-(--b2) sm:block" />
          <BarButton>
            <UsageRing pct={34} />
          </BarButton>
          <BarButton on>
            <PanelRight className="size-3.5" />
          </BarButton>
        </div>
      </div>
      <Caption />
    </div>
  )
}

export function AppWindow() {
  const secs = useSeconds()
  return (
    <div className="app overflow-hidden rounded-2xl border border-white/[0.12] shadow-[0_40px_90px_-24px_rgba(0,0,0,0.85),inset_0_1px_0_rgba(255,255,255,0.04)]">
      <Titlebar />
      <div className="flex h-[640px] sm:h-[700px]">
        <Sidebar secs={secs} />
        <Thread secs={secs} />
        <Dock />
      </div>
    </div>
  )
}
