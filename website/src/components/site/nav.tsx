import { useEffect, useState } from "react"
import { Apple, Download, Github, Menu, X } from "lucide-react"
import { AppIcon, Wrap } from "./primitives"
import { DOWNLOAD_MAC, DOWNLOAD_WIN, REPO } from "@/site"

const LINKS: [string, string][] = [
  ["Install", "#install"],
  ["Features", "#features"],
]

/**
 * Which build to offer. The nav used to hand everyone the Windows installer, which meant a Mac
 * visitor downloaded a .exe — so this picks by platform and the hero uses it to order its buttons.
 */
export function usePlatform() {
  const [mac, setMac] = useState(false)
  useEffect(() => {
    const ua = navigator.userAgent
    setMac(/Mac|iPhone|iPad/.test(ua))
  }, [])
  const win = { href: DOWNLOAD_WIN, label: "Download for Windows", Icon: Download }
  const osx = { href: DOWNLOAD_MAC, label: "Download for macOS", Icon: Apple }
  // `other` lets the hero show both buttons with the visitor's platform first
  return mac ? { ...osx, short: "Download", other: win } : { ...win, short: "Download", other: osx }
}

/** the bar is invisible over the hero and only materialises once you start scrolling */
function useScrolled(px = 12) {
  const [past, setPast] = useState(false)
  useEffect(() => {
    const on = () => setPast(window.scrollY > px)
    on()
    window.addEventListener("scroll", on, { passive: true })
    return () => window.removeEventListener("scroll", on)
  }, [px])
  return past
}

/** highlights whichever section you're currently looking at */
function useActiveSection(ids: string[]) {
  const [active, setActive] = useState("")
  useEffect(() => {
    const els = ids.map((i) => document.getElementById(i)).filter(Boolean) as HTMLElement[]
    if (!els.length) return
    const io = new IntersectionObserver(
      (entries) => {
        const hit = entries
          .filter((e) => e.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0]
        if (hit) setActive(hit.target.id)
      },
      // a band across the middle of the viewport, so a section counts once it's genuinely in view
      { rootMargin: "-45% 0px -45% 0px", threshold: 0 },
    )
    els.forEach((e) => io.observe(e))
    return () => io.disconnect()
  }, [ids.join(",")])
  return active
}

export function Nav() {
  const scrolled = useScrolled()
  const active = useActiveSection(["install", "features"])
  const dl = usePlatform()
  const [open, setOpen] = useState(false)

  // never leave the menu open behind a navigation
  useEffect(() => {
    if (!open) return
    const close = () => setOpen(false)
    window.addEventListener("hashchange", close)
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false)
    window.addEventListener("keydown", onKey)
    return () => {
      window.removeEventListener("hashchange", close)
      window.removeEventListener("keydown", onKey)
    }
  }, [open])

  return (
    <header className="sticky top-0 z-30 pt-4">
      <Wrap>
        <div
          className={`relative mx-auto flex h-[48px] max-w-[640px] items-center justify-between rounded-xl pr-1.5 pl-3 transition-[background-color,border-color,box-shadow,backdrop-filter] duration-300 ${
            scrolled || open
              ? "border border-white/[0.09] bg-[#0e0e11]/85 shadow-[0_16px_40px_-20px_rgba(0,0,0,0.9),inset_0_1px_0_rgba(255,255,255,0.045)] backdrop-blur-xl"
              : "border border-transparent bg-transparent"
          }`}
        >
          <a
            href="#top"
            className="inline-flex items-center gap-2.5 text-[14.5px] font-semibold tracking-[-0.01em]"
          >
            <AppIcon className="size-[19px]" /> HyprSpace
          </a>

          <nav className="absolute left-1/2 hidden -translate-x-1/2 items-center gap-0.5 md:flex">
            {LINKS.map(([label, href]) => {
              const on = href === `#${active}`
              return (
                <a
                  key={href}
                  href={href}
                  className={`rounded-md px-2.5 py-1.5 text-[13px] font-medium transition-colors ${
                    on ? "bg-white/[0.06] text-zinc-100" : "text-zinc-400 hover:bg-white/[0.055] hover:text-zinc-100"
                  }`}
                >
                  {label}
                </a>
              )
            })}
          </nav>

          <div className="flex items-center gap-1.5">
            <a
              href={REPO}
              target="_blank"
              rel="noreferrer noopener"
              aria-label="HyprSpace on GitHub (opens in a new tab)"
              className="flex size-[31px] items-center justify-center rounded-md border border-white/[0.08] bg-white/[0.03] text-zinc-400 transition-colors hover:bg-white/[0.07] hover:text-zinc-100"
            >
              <Github className="size-4" />
            </a>
            <a
              href={dl.href}
              title={dl.label}
              className="flex h-[31px] items-center gap-1.5 rounded-md bg-zinc-50 px-3 text-[13px] font-semibold text-zinc-950 transition hover:bg-white active:translate-y-px"
            >
              <dl.Icon className="size-3.5" />
              {dl.short}
            </a>
            <button
              onClick={() => setOpen((o) => !o)}
              aria-label={open ? "Close menu" : "Open menu"}
              aria-expanded={open}
              className="flex size-[31px] items-center justify-center rounded-md border border-white/[0.08] bg-white/[0.03] text-zinc-400 transition-colors hover:text-zinc-100 md:hidden"
            >
              {open ? <X className="size-4" /> : <Menu className="size-4" />}
            </button>
          </div>

          {open && (
            <div className="absolute top-[calc(100%+8px)] right-0 left-0 grid gap-0.5 rounded-xl border border-white/[0.09] bg-[#0e0e11]/95 p-2 shadow-[0_24px_50px_-20px_rgba(0,0,0,0.9)] backdrop-blur-xl md:hidden">
              {LINKS.map(([label, href]) => (
                <a
                  key={href}
                  href={href}
                  onClick={() => setOpen(false)}
                  className="rounded-lg px-3 py-2.5 text-[14px] font-medium text-zinc-300 transition-colors hover:bg-white/[0.06] hover:text-zinc-100"
                >
                  {label}
                </a>
              ))}
            </div>
          )}
        </div>
      </Wrap>
    </header>
  )
}
