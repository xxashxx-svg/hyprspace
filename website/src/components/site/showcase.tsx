import { useEffect, useState } from "react"
import { Check, Copy } from "lucide-react"
import { INSTALL_PS1, INSTALL_SH, LINUX_RELEASED, RELEASES, REPO } from "@/site"
import { usePlatform } from "./nav"

/* ---------------------------------------------------------------- release pill */

type Release = { tag: string; url: string; note: string }

/**
 * Pulls the newest release straight from GitHub so the pill can never go stale. Unauthenticated and
 * cached by the browser; if it fails for any reason the pill simply doesn't render.
 */
let pending: Promise<Release | null> | null = null

function fetchRelease(): Promise<Release | null> {
  pending ??= fetch("https://api.github.com/repos/xxashxx-svg/hyprspace/releases/latest")
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((d) => {
        // the release notes are a bullet list — the first bullet is the headline change
        const first = String(d.body ?? "")
          .split("\n")
          .map((l: string) => l.replace(/^[-*]\s*/, "").trim())
          .find(Boolean)
      const note = first ? first.split(/[—.:]/)[0].trim() : ""
      return {
        tag: d.tag_name as string,
        url: (d.html_url as string) ?? RELEASES,
        note: note.length > 46 ? note.slice(0, 45) + "…" : note,
      }
    })
    .catch(() => null)
  return pending
}

function useLatestRelease(): Release | null {
  const [rel, setRel] = useState<Release | null>(null)
  useEffect(() => {
    let dead = false
    void fetchRelease().then((r) => {
      if (!dead) setRel(r)
    })
    return () => {
      dead = true
    }
  }, [])
  return rel
}

export function ReleasePill() {
  const rel = useLatestRelease()
  if (!rel) return null
  return (
    <a
      href={rel.url}
      target="_blank"
      rel="noreferrer"
      className="group inline-flex items-center gap-2.5 rounded-full border border-white/[0.12] bg-white/[0.025] py-1 pr-3 pl-1.5 font-mono text-[11.5px] text-zinc-400 transition hover:border-white/25 hover:text-zinc-200"
    >
      <span className="rounded-full bg-white/[0.09] px-2 py-0.5 text-[10.5px] text-zinc-200">New</span>
      {rel.tag}
      {rel.note && <span className="hidden text-zinc-500 sm:inline">— {rel.note}</span>}
      <span className="transition group-hover:translate-x-0.5">→</span>
    </a>
  )
}

/* ---------------------------------------------------------------- download */

/**
 * One button for the visitor's own platform. Offering all three made everyone read three lines to
 * find their own, and handed a Mac visitor a .exe if they misread. The others stay reachable as a
 * quiet line underneath, for a browser that reports the wrong platform or someone fetching a build
 * for a different machine.
 */
export function DownloadCta() {
  const dl = usePlatform()
  const rel = useLatestRelease()
  return (
    <div className="flex flex-col items-center gap-4">
      <a
        href={dl.href}
        title={dl.label}
        className="inline-flex items-center gap-2.5 rounded-xl bg-zinc-50 px-7 py-[13px] text-[15px] font-semibold text-zinc-950 transition-colors duration-150 hover:bg-white active:translate-y-px"
      >
        <dl.Icon className="size-[18px]" />
        {dl.label}
      </a>

      <div className="flex flex-wrap items-center justify-center gap-x-2.5 gap-y-1.5 font-mono text-[11.5px] text-zinc-500">
        {rel?.tag && (
          <>
            <span className="text-zinc-400">{rel.tag}</span>
            <span className="h-3 w-px bg-white/[0.12]" />
          </>
        )}
        <span>{dl.req}</span>
        {dl.others.length > 0 && (
          <>
            <span className="h-3 w-px bg-white/[0.12]" />
            <span>
              also on{" "}
              {dl.others.map((o, i) => (
                <span key={o.href}>
                  {i > 0 && " and "}
                  <a
                    href={o.href}
                    title={o.label}
                    className="text-zinc-400 underline-offset-[3px] transition-colors hover:text-zinc-200 hover:underline"
                  >
                    {o.name}
                  </a>
                </span>
              ))}
            </span>
          </>
        )}
      </div>
    </div>
  )
}

/* ---------------------------------------------------------------- install block */

// macOS and Linux share one script (it branches on uname), so those two tabs carry the same command
// and the note is what tells them apart. Every tab says where the command puts things, because
// nobody should pipe a script into a shell without being told what it does.
const TABS = [
  {
    key: "win",
    label: "Windows",
    cmd: `irm ${INSTALL_PS1} | iex`,
    note: "Runs the signed installer from the latest release. Installs for you alone, so Windows never asks for admin.",
  },
  {
    key: "mac",
    label: "macOS",
    cmd: `curl -fsSL ${INSTALL_SH} | sh`,
    note: "Apple Silicon. Puts HyprSpace.app in /Applications.",
  },
  // only once a release actually carries the AppImage, otherwise this 404s (see LINUX_RELEASED)
  ...(LINUX_RELEASED
    ? [
        {
          key: "linux",
          label: "Linux",
          cmd: `curl -fsSL ${INSTALL_SH} | sh`,
          note: "x86_64. Puts the self-updating AppImage in ~/.local/bin and adds a menu entry.",
        },
      ]
    : []),
  {
    key: "src",
    label: "From source",
    cmd: `git clone ${REPO} && cd hyprspace && npm install && npm run tauri dev`,
    note: "Needs Node and the Rust toolchain.",
  },
]

export function Install() {
  const dl = usePlatform()
  // null means "follow whatever platform this is", so the right tab is already open on arrival.
  // Clicking pins a choice, which is why this is not just state seeded from the platform.
  const [picked, setPicked] = useState<number | null>(null)
  const [copied, setCopied] = useState(false)
  const auto = Math.max(0, TABS.findIndex((t) => t.label === dl.name))
  const tab = picked ?? auto

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(TABS[tab].cmd)
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard blocked — the command is selectable anyway */
    }
  }

  return (
    <div className="overflow-hidden rounded-xl border border-white/[0.09] bg-[#0c0c0e]">
      {/* wraps because there are four tabs: at phone widths they do not fit on one row */}
      <div className="flex flex-wrap items-center gap-0.5 border-b border-white/[0.06] p-1.5">
        {TABS.map((t, i) => (
          <button
            key={t.key}
            onClick={() => setPicked(i)}
            aria-pressed={i === tab}
            className={`rounded-lg px-3 py-1.5 text-[12.5px] transition-colors ${
              i === tab
                ? "bg-white/[0.07] text-zinc-100"
                : "text-zinc-500 hover:bg-white/[0.035] hover:text-zinc-300"
            }`}
          >
            {t.label}
          </button>
        ))}
        <button
          onClick={copy}
          aria-label="Copy command"
          className="ml-auto flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-[12px] text-zinc-500 transition-colors hover:bg-white/[0.05] hover:text-zinc-200"
        >
          {copied ? <Check className="size-3.5 text-emerald-400" /> : <Copy className="size-3.5" />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>

      {/* wrap rather than scroll: the command is the content here, and a horizontal scrollbar hides
          most of a long URL behind a gesture nobody makes */}
      <pre className="px-4 py-4 font-mono text-[13px] leading-relaxed break-all whitespace-pre-wrap text-zinc-200">
        <span className="mr-2.5 select-none text-zinc-600">$</span>
        {TABS[tab].cmd}
      </pre>

      {/* what it is about to do, on its own band so it reads as a footnote and not as more command */}
      <p className="border-t border-white/[0.05] bg-white/[0.015] px-4 py-2.5 text-[12px] leading-relaxed text-zinc-500">
        {TABS[tab].note}
      </p>
    </div>
  )
}
