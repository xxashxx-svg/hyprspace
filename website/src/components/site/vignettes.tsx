import { useEffect, useRef, useState, type ReactNode } from "react"
import claudeIcon from "@/assets/brand/claude.svg"
import openaiIcon from "@/assets/brand/openai.svg"

/**
 * Slices of the real app, rebuilt in DOM against its own tokens — #161616 chrome, #1e1e1e raised
 * fills, the f5/a1/76 text ramp, 6px status dots. No generic "code window" furniture: the app has no
 * traffic lights and no lowercase window titles, so neither do these. Content is real too: this
 * repo's files, branches and commands rather than an invented acme-app.
 */

/** true once the element has been scrolled into view — never flips back */
function useInView<T extends HTMLElement>() {
  const ref = useRef<T>(null)
  const [seen, setSeen] = useState(false)
  useEffect(() => {
    const el = ref.current
    if (!el || seen) return
    const io = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting) {
          setSeen(true)
          io.disconnect()
        }
      },
      { threshold: 0.35 },
    )
    io.observe(el)
    return () => io.disconnect()
  }, [seen])
  return [ref, seen] as const
}

const S1 = "#161616" // chrome
const S2 = "#1e1e1e" // raised
const T1 = "#f5f5f5"
const T2 = "#a1a1a1"
const T3 = "#767676"
const B1 = "rgba(255,255,255,0.06)"

/** the app's panel: flat chrome, hairline border, an uppercase section label like the rail's */
function Panel({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div
      className="overflow-hidden rounded-[10px] border"
      style={{ background: S1, borderColor: B1 }}
    >
      <div
        className="px-3 pt-2.5 pb-1.5 text-[10px] font-semibold tracking-[0.07em] uppercase"
        style={{ color: T3 }}
      >
        {label}
      </div>
      {children}
    </div>
  )
}

const mono = "font-mono text-[11px] leading-[1.85]"

function Dot({ tone, className = "" }: { tone: "work" | "wait" | "done"; className?: string }) {
  const c = tone === "work" ? "#f59e0b" : tone === "wait" ? "#3b82f6" : "#10b981"
  return (
    <i
      className={`inline-block size-1.5 shrink-0 rounded-full ${tone !== "done" ? "hs-pulse" : ""} ${className}`}
      style={{ background: c }}
    />
  )
}

/** the sidebar: project → branch → two-line agent rows, with a sub-agent under the first */
export function TreeVignette() {
  return (
    <Panel label="Projects">
      <div className="px-2 pb-3 text-[12px]">
        <div className="flex items-center gap-2 rounded-md px-2 py-1.5" style={{ background: "#282828" }}>
          <svg viewBox="0 0 24 24" className="size-3.5 shrink-0" fill="none" stroke={T2} strokeWidth="1.8">
            <path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13c0 1.1.9 2 2 2Z" />
          </svg>
          <span style={{ color: T1 }}>hyprspace-tauri</span>
          <span className="ml-auto font-mono text-[10px]" style={{ color: T3 }}>2</span>
        </div>

        <div className="mt-0.5 flex items-center gap-1.5 py-1 pl-6" style={{ color: T3 }}>
          <svg viewBox="0 0 24 24" className="size-3 shrink-0" fill="none" stroke="currentColor" strokeWidth="2">
            <line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" />
            <path d="M18 9a9 9 0 0 1-9 9" />
          </svg>
          <span className="text-[11.5px]" style={{ color: T2 }}>main</span>
          <i className="size-1 rounded-full" style={{ background: T2 }} />
        </div>

        {[
          [claudeIcon, "Claude", "Delegating audit pass", "work", "now"],
          [openaiIcon, "Codex", "cargo check", "done", "4m"],
        ].map(([ic, name, act, tone, when]) => (
          <div key={name as string}>
            <div className="flex items-start gap-2 py-1 pl-6">
              <span className="relative mt-[3px] flex size-3.5 shrink-0 items-center justify-center">
                <img src={ic as string} alt="" className="size-3.5" />
                <Dot tone={tone as "work"} className="absolute -right-1 -bottom-0.5 ring-2" />
              </span>
              <span className="flex min-w-0 flex-col">
                <span className="text-[12px] leading-tight" style={{ color: T1 }}>{name}</span>
                <span className="truncate font-mono text-[10px] leading-tight" style={{ color: T3 }}>{act}</span>
              </span>
              <span className="mt-px ml-auto font-mono text-[10px]" style={{ color: T3 }}>{when}</span>
            </div>

            {/* delegated agents hang off a lineage rail tinted with the app's accent, as in the rail */}
            {name === "Claude" && (
              <div
                className="ml-[38px] flex flex-col pl-2.5"
                style={{ borderLeft: "1px solid color-mix(in srgb, var(--brand) 40%, transparent)" }}
              >
                {[
                  ["security audit", "1m"],
                  ["test coverage", "40s"],
                ].map(([label, ago]) => (
                  <div key={label} className="flex items-center gap-2 py-[3px]">
                    <Dot tone="work" className="!size-1" />
                    <span className="truncate text-[11px]" style={{ color: T2 }}>{label}</span>
                    <span className="ml-auto font-mono text-[10px]" style={{ color: T3 }}>{ago}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </Panel>
  )
}

function Meter({ label, value, width, slow, tick }: { label: string; value: string; width: string; slow?: boolean; tick?: boolean }) {
  const [ref, seen] = useInView<HTMLDivElement>()
  return (
    <div ref={ref}>
      <div className="flex items-baseline justify-between">
        <span className="text-[10px] font-semibold tracking-[0.07em] uppercase" style={{ color: T2 }}>{label}</span>
        <span className="font-mono text-[13px]" style={{ color: T1 }}>{value}</span>
      </div>
      <div className="relative mt-1.5 h-1.5 overflow-hidden rounded-full" style={{ background: "rgba(255,255,255,0.14)" }}>
        {/* fills once when you reach it and then stays put — a looping meter reads as a progress
            spinner rather than a reading */}
        <i
          className="block h-full transition-[width] duration-[1100ms] ease-out"
          style={{
            width: seen ? width : "4%",
            background: slow ? T2 : T1,
            transitionDelay: slow ? "180ms" : "0ms",
          }}
        />
        {tick && <span className="absolute inset-y-0 left-[46%] w-px bg-white/90" />}
      </div>
    </div>
  )
}

/** the usage popover, exactly as it renders in the titlebar */
export function UsageVignette() {
  return (
    <div className="overflow-hidden rounded-[10px] border p-3.5" style={{ background: S2, borderColor: "rgba(255,255,255,0.1)" }}>
      <div className="grid gap-3">
        <div className="text-[10px] font-semibold tracking-[0.07em] uppercase" style={{ color: T3 }}>Claude</div>
        <Meter label="Session · 5h" value="34%" width="34%" tick />
        <Meter label="This week" value="54%" width="54%" slow />
        <div className="font-mono text-[10px]" style={{ color: T2 }}>resets in 2h 41m</div>
      </div>
    </div>
  )
}

/** the command palette */
export function PaletteVignette() {
  return (
    <Panel label="Ctrl K">
      <div className={`${mono} px-3 pb-3`} style={{ color: T3 }}>
        <div style={{ color: T2 }}>
          deploy
          <i className="ml-px inline-block h-[11px] w-1.5 align-middle hs-blink" style={{ background: T2 }} />
        </div>
        <div className="-mx-1 mt-1 rounded px-1" style={{ background: "#282828", color: T1 }}>
          Open · deploy.ps1
        </div>
        <div>Run · Commit &amp; push</div>
        <div>New · Open space</div>
      </div>
    </Panel>
  )
}

/** the review dock's diff */
export function DiffVignette() {
  return (
    <Panel label="src-tauri/src/agenthook.rs">
      <div className={`${mono} px-3 pb-3`}>
        <div style={{ color: T3 }}>126 fn post(port: u16, body: &amp;str) &#123;</div>
        <div style={{ background: "rgba(16,185,129,0.1)", color: "#86efac" }}>127 + let addr = SocketAddr::from(..);</div>
        <div style={{ background: "rgba(16,185,129,0.1)", color: "#86efac" }}>128 + TcpStream::connect_timeout(&amp;addr)</div>
        <div style={{ background: "rgba(239,68,68,0.1)", color: "#fca5a5" }}>129 - TcpStream::connect(("127.0.0.1"))</div>
      </div>
    </Panel>
  )
}

/** two real panes trading places in the grid */
function MiniPane({ icon, name, line, tone, lifted }: { icon: string; name: string; line: string; tone: "work" | "done"; lifted: boolean }) {
  return (
    <div
      className="h-full overflow-hidden rounded-md border"
      style={{
        background: S1,
        borderColor: lifted ? "rgba(255,255,255,0.18)" : B1,
        boxShadow: lifted ? "0 14px 30px -10px rgba(0,0,0,0.85)" : "none",
        transition: "border-color .25s, box-shadow .25s",
      }}
    >
      <div className="flex items-center gap-1.5 px-1.5 py-1" style={{ borderBottom: `1px solid ${B1}` }}>
        <img src={icon} alt="" className="size-2.5 shrink-0" />
        <span className="truncate text-[9.5px]" style={{ color: T2 }}>{name}</span>
        <Dot tone={tone} className="ml-auto !size-1" />
      </div>
      <div className="truncate px-1.5 py-1 font-mono text-[8.5px]" style={{ color: T3 }}>{line}</div>
    </div>
  )
}

const PANES: { icon: string; name: string; line: string; tone: "work" | "done" }[] = [
  { icon: claudeIcon, name: "Claude", line: "Edit Rail.tsx", tone: "work" },
  { icon: openaiIcon, name: "Codex", line: "cargo check", tone: "done" },
  { icon: claudeIcon, name: "Claude 2", line: "Grep providers", tone: "work" },
  { icon: openaiIcon, name: "Codex 2", line: "npm test", tone: "done" },
]

export function SwapVignette() {
  const [ref, seen] = useInView<HTMLDivElement>()
  // cell index each pane currently occupies; swapping two entries animates both via transform
  const [cells, setCells] = useState([0, 1, 2, 3])
  const [moving, setMoving] = useState<number[]>([])

  useEffect(() => {
    if (!seen || matchMedia("(prefers-reduced-motion: reduce)").matches) return
    // walk through genuinely different pairs instead of trading the same two forever
    const pairs = [
      [0, 1],
      [1, 3],
      [2, 3],
      [0, 2],
      [0, 3],
      [1, 2],
    ]
    let i = 0
    const t = setInterval(() => {
      const [a, b] = pairs[i++ % pairs.length]
      setMoving([a, b])
      setCells((c) => {
        const n = [...c]
        ;[n[a], n[b]] = [n[b], n[a]]
        return n
      })
      setTimeout(() => setMoving([]), 620)
    }, 2400)
    return () => clearInterval(t)
  }, [seen])

  return (
    <Panel label="Layout · drag to rearrange">
      <div ref={ref} className="relative mx-3 mb-3 h-[104px]">
        {PANES.map((p, i) => {
          const cell = cells[i]
          const col = cell % 2
          const row = Math.floor(cell / 2)
          return (
            <div
              key={p.name}
              className="absolute top-0 left-0 h-[calc(50%-3px)] w-[calc(50%-3px)]"
              style={{
                transform: `translate(calc(${col} * (100% + 6px)), calc(${row} * (100% + 6px)))`,
                transition: "transform .6s cubic-bezier(.4,0,.2,1)",
                zIndex: moving.includes(i) ? 10 : 1,
              }}
            >
              <MiniPane {...p} lifted={moving.includes(i)} />
            </div>
          )
        })}
      </div>
    </Panel>
  )
}

/** the built-in editor */
export function EditorVignette() {
  return (
    <Panel label="src/stores/usage.ts">
      <div className={`${mono} px-3 pb-3`} style={{ color: T3 }}>
        <div><span style={{ color: "#c084fc" }}>export function</span> <span style={{ color: "#93c5fd" }}>summarize</span>(byPane) &#123;</div>
        <div className="pl-3"><span style={{ color: "#c084fc" }}>const</span> all = Object.<span style={{ color: "#93c5fd" }}>entries</span>(byPane)</div>
        <div className="pl-3"><span style={{ color: "#c084fc" }}>return</span> &#123; five, others &#125;</div>
        <div>&#125;</div>
      </div>
    </Panel>
  )
}

/** a startup action running in its own pane */
export function StartupVignette() {
  return (
    <Panel label="On open · hyprspace-tauri">
      <div className={`${mono} px-3 pb-3`} style={{ color: T3 }}>
        <div className="flex items-center gap-2">
          <Dot tone="work" />
          <span style={{ color: T1 }}>npm run tauri dev</span>
        </div>
        <div>VITE ready in 1058 ms</div>
        <div style={{ color: "#86efac" }}>➜ localhost:1420</div>
      </div>
    </Panel>
  )
}
