import { useEffect, useRef, useState, type ReactNode } from "react"
import { cn } from "@/lib/utils"

/** Fades a section up the first time it scrolls into view. */
export function Reveal({
  children,
  className,
  delay = 0,
}: {
  children: ReactNode
  className?: string
  delay?: number
}) {
  const ref = useRef<HTMLDivElement>(null)
  const [seen, setSeen] = useState(false)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    const io = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting) {
          setSeen(true)
          io.disconnect()
        }
      },
      { threshold: 0.1, rootMargin: "0px 0px -6% 0px" },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [])

  return (
    <div
      ref={ref}
      style={{ transitionDelay: `${delay}ms` }}
      className={cn(
        "transition-[opacity,transform] duration-700 ease-[cubic-bezier(0.2,0,0,1)] motion-reduce:transition-none",
        seen ? "translate-y-0 opacity-100" : "translate-y-4 opacity-0 motion-reduce:translate-y-0 motion-reduce:opacity-100",
        className,
      )}
    >
      {children}
    </div>
  )
}

export function Wrap({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("mx-auto w-full max-w-[1180px] px-7", className)}>{children}</div>
}

export function Eyebrow({ children }: { children: ReactNode }) {
  return (
    <span className="block font-mono text-[11.5px] tracking-[0.18em] text-zinc-500 uppercase">
      {children}
    </span>
  )
}

export function Section({
  id,
  children,
  className,
  divide = true,
}: {
  id?: string
  children: ReactNode
  className?: string
  divide?: boolean
}) {
  return (
    <section
      id={id}
      className={cn("py-24 md:py-28", divide && "border-t border-white/[0.07]", className)}
    >
      {children}
    </section>
  )
}

const btn =
  "inline-flex items-center gap-2 rounded-lg text-[14.5px] font-semibold transition active:translate-y-px focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-zinc-100"

export function ButtonLink({
  href,
  variant = "primary",
  external,
  className,
  children,
}: {
  href: string
  variant?: "primary" | "line" | "ghost"
  /** open in a new tab — for links that navigate away (github, docs), not file downloads */
  external?: boolean
  className?: string
  children: ReactNode
}) {
  const styles = {
    primary: "bg-zinc-50 px-[18px] py-[11px] text-zinc-950 hover:bg-white",
    line: "border border-white/15 bg-zinc-900 px-[18px] py-[11px] text-zinc-100 hover:bg-zinc-800",
    ghost: "px-2 py-[11px] text-zinc-400 hover:text-zinc-100",
  }[variant]

  return (
    <a
      href={href}
      {...(external ? { target: "_blank", rel: "noreferrer noopener" } : {})}
      className={cn(btn, styles, className)}
    >
      {children}
    </a>
  )
}

/** A bordered point in the alternating feature splits. */
export function Point({
  icon,
  title,
  children,
}: {
  icon: ReactNode
  title: string
  children: ReactNode
}) {
  return (
    <div className="flex gap-3.5 border-t border-white/[0.07] py-3.5">
      <span className="mt-0.5 shrink-0 text-zinc-100 [&>svg]:size-[17px]">{icon}</span>
      <div>
        <b className="text-[15px] font-semibold text-zinc-100">{title}</b>
        <span className="mt-0.5 block text-[14px] text-zinc-400">{children}</span>
      </div>
    </div>
  )
}

export function Code({ children }: { children: ReactNode }) {
  return (
    <code className="rounded bg-zinc-900 px-1.5 py-px font-mono text-[12px] text-zinc-100">
      {children}
    </code>
  )
}

/** The app-window chrome used to frame screenshots and mockups. */
export function AppFrame({
  title,
  className,
  children,
}: {
  title?: string
  className?: string
  children: ReactNode
}) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-2xl border border-white/[0.12] bg-gradient-to-b from-[#161618] to-[#0e0e11]",
        "shadow-[0_40px_90px_-24px_rgba(0,0,0,0.82),inset_0_1px_0_rgba(255,255,255,0.04)]",
        className,
      )}
    >
      {title && (
        <div className="flex h-10 items-center gap-2.5 border-b border-white/[0.07] px-3.5">
          <AppIcon className="size-[15px]" />
          <span className="text-[12.5px] font-semibold text-zinc-100">{title}</span>
          <span className="ml-auto flex gap-[7px]">
            <i className="size-2.5 rounded-full bg-zinc-700" />
            <i className="size-2.5 rounded-full bg-zinc-700" />
            <i className="size-2.5 rounded-full bg-zinc-700" />
          </span>
        </div>
      )}
      {children}
    </div>
  )
}

/** The app's mark: a bare isometric cube. */
export function AppIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden>
      <path d="M12 3 L20.5 8 L12 13 L3.5 8 Z" fill="#3d54e8" />
      <path d="M3.5 8 L12 13 L12 21 L3.5 16 Z" fill="#f5f5f5" />
      <path d="M20.5 8 L20.5 16 L12 21 L12 13 Z" fill="#767676" />
    </svg>
  )
}
