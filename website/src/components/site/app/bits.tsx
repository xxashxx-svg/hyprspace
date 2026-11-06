import { useEffect, useState, type CSSProperties, type ReactNode } from "react"
import claude from "@/assets/brand/claude.svg"
import openai from "@/assets/brand/openai.svg"
import ring from "@/assets/app/ring.svg"
import { cn } from "@/lib/utils"

export type Agent = "claude" | "codex"

export function useSeconds() {
  const [secs, setSecs] = useState(0)
  useEffect(() => {
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) return
    const t = setInterval(() => setSecs((s) => s + 1), 1000)
    return () => clearInterval(t)
  }, [])
  return secs
}

const MARKS: Record<Agent, [string, string]> = {
  claude: [claude, "#d97757"],
  codex: [openai, "#10a37f"],
}

function masked(src: string, color: string, size: number): CSSProperties {
  const mask = `url("${src}") center / contain no-repeat`
  return { width: size, height: size, background: color, mask, WebkitMask: mask }
}

export function Mark({ agent, size = 13 }: { agent: Agent; size?: number }) {
  const [src, color] = MARKS[agent]
  return <i aria-hidden className="block shrink-0" style={masked(src, color, size)} />
}

export function AgentMark({ agent, working }: { agent: Agent; working?: boolean }) {
  return (
    <span className="relative flex size-4 shrink-0 items-center justify-center">
      <Mark agent={agent} />
      {working && (
        <i
          aria-hidden
          className="hs-spin absolute -top-[3px] -left-[3px] block"
          style={masked(ring, "var(--busy)", 22)}
        />
      )}
    </span>
  )
}

const TAGS: Record<string, [string, string, string]> = {
  hyprspace: ["HE", "#374310", "#cce298"],
  website: ["WE", "#573407", "#ffcb90"],
  "Agent-Sdk": ["AS", "#5d2b36", "#ffbccc"],
}

export function Tag({ space }: { space: string }) {
  const [initials, fill, ink] = TAGS[space]
  return (
    <span
      className="flex size-4 shrink-0 items-center justify-center rounded-[4px] font-app-mono text-[8.5px] font-bold"
      style={{ background: fill, color: ink }}
    >
      {initials}
    </span>
  )
}

export function Keycap({ children }: { children: ReactNode }) {
  return (
    <span className="rounded-[4px] bg-white/[0.08] px-[5px] text-[10px] leading-[16px] text-(--t2)">
      {children}
    </span>
  )
}

export function UsageRing({ pct, color = "var(--t1)", size = 16 }: { pct: number; color?: string; size?: number }) {
  const r = 6.4
  const c = 2 * Math.PI * r
  return (
    <svg viewBox="0 0 16 16" className="-rotate-90 shrink-0" style={{ width: size, height: size }}>
      <circle cx="8" cy="8" r={r} fill="none" strokeWidth="2.2" stroke="rgb(255 255 255 / 0.16)" />
      <circle
        cx="8"
        cy="8"
        r={r}
        fill="none"
        strokeWidth="2.2"
        strokeLinecap="round"
        stroke={color}
        strokeDasharray={`${((c * pct) / 100).toFixed(1)} ${c.toFixed(1)}`}
      />
    </svg>
  )
}

export function Counts({ added, removed, className }: { added: number; removed: number; className?: string }) {
  return (
    <span className={cn("flex shrink-0 gap-1.5 font-app-mono", className)}>
      {added > 0 && <span className="text-(--ok)">+{added}</span>}
      {removed > 0 && <span className="text-(--err)">-{removed}</span>}
    </span>
  )
}
