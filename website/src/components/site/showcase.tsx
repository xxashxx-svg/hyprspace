import { useEffect, useState } from "react"
import { Check, Copy } from "lucide-react"
import { DOWNLOAD_LINUX, DOWNLOAD_MAC, DOWNLOAD_WIN, LINUX_RELEASED, RELEASES, REPO } from "@/site"

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

/* ---------------------------------------------------------------- install block */

const TABS = [
  {
    key: "win",
    label: "Windows",
    cmd: `irm ${DOWNLOAD_WIN} -OutFile HyprSpace.exe; ./HyprSpace.exe`,
  },
  {
    key: "mac",
    label: "macOS",
    cmd: `curl -L -o HyprSpace.dmg ${DOWNLOAD_MAC} && open HyprSpace.dmg`,
  },
  // only once a release actually carries the AppImage, otherwise this curl 404s (see LINUX_RELEASED)
  ...(LINUX_RELEASED
    ? [
        {
          key: "linux",
          label: "Linux",
          // AppImages arrive without the execute bit, so chmod is part of the one-liner
          cmd: `curl -L -o HyprSpace.AppImage ${DOWNLOAD_LINUX} && chmod +x HyprSpace.AppImage && ./HyprSpace.AppImage`,
        },
      ]
    : []),
  {
    key: "src",
    label: "From source",
    cmd: `git clone ${REPO} && cd hyprspace && npm install && npm run tauri dev`,
  },
]

export function Install() {
  const [tab, setTab] = useState(0)
  const [copied, setCopied] = useState(false)

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
    <div className="overflow-hidden rounded-xl border border-white/[0.1] bg-[#0c0c0e]">
      {/* wraps because there are four tabs now: at phone widths they do not fit on one row */}
      <div className="flex flex-wrap items-center gap-1 border-b border-white/[0.07] px-2 py-1.5">
        {TABS.map((t, i) => (
          <button
            key={t.key}
            onClick={() => setTab(i)}
            className={`rounded-md px-3 py-1 font-mono text-[12px] transition ${
              i === tab ? "bg-white/[0.07] text-zinc-100" : "text-zinc-500 hover:text-zinc-300"
            }`}
          >
            {t.label}
          </button>
        ))}
        <button
          onClick={copy}
          aria-label="Copy command"
          className="ml-auto flex items-center gap-1.5 rounded-md px-2.5 py-1 font-mono text-[11.5px] text-zinc-500 transition hover:bg-white/[0.05] hover:text-zinc-200"
        >
          {copied ? <Check className="size-3.5 text-emerald-400" /> : <Copy className="size-3.5" />}
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      {/* wrap rather than scroll: this is the install section, so the command is the content. A
          horizontal scrollbar hides most of a long URL behind a gesture nobody makes. */}
      <pre className="px-4 py-3.5 font-mono text-[12.5px] leading-relaxed break-all whitespace-pre-wrap text-zinc-300">
        <span className="mr-2 select-none text-zinc-600">$</span>
        {TABS[tab].cmd}
      </pre>
    </div>
  )
}
