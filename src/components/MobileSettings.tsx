// Settings → Mobile: turn the LAN bridge on, show the pairing QR the Android app scans, and list
// whatever phones are connected right now. The phone preview beside the QR renders this machine's
// real spaces in the shape the app's home screen uses (mobile/app/index.tsx), so what you see here
// is what the paired phone will list.
import { useEffect, useMemo, useState } from "react";
import qrcode from "qrcode-generator";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Copy, Download, ExternalLink, Globe, RefreshCw, ShieldCheck, Smartphone, TriangleAlert, WifiOff } from "lucide-react";
import { DEFAULT_BRIDGE_PORT, pairingUrl, peerLabel, tailscaleAddr, useBridge } from "../stores/bridge";
import { bridgeStatus } from "../api";
import { useWorkspaces } from "../stores/workspace";
import { displayState, useAgentStatus } from "../stores/agentStatus";
import { relTime } from "../lib/time";

// the stable name the release workflow attaches, so this link never goes stale between versions
const ANDROID_APK_URL =
  "https://github.com/xxashxx-svg/hyprspace/releases/latest/download/HyprSpace-android.apk";
const TAILSCALE_URL = "https://tailscale.com/download";

/** how often the interface list is re-read while this page is open */
const ADDR_POLL_MS = 5000;

/** how many spaces fit in the little screen before it stops listing them */
const PREVIEW_ROWS = 4;

// Drawn as one <svg> path of dark modules rather than pulling in a renderer, so it stays crisp at
// any size. The quiet zone and the white ground are baked in: a camera needs that contrast and the
// app around it is dark.
function Qr({ text, size = 204 }: { text: string; size?: number }) {
  const path = useMemo(() => {
    const qr = qrcode(0, "M");
    qr.addData(text);
    qr.make();
    const n = qr.getModuleCount();
    let d = "";
    for (let r = 0; r < n; r++) {
      for (let c = 0; c < n; c++) {
        if (qr.isDark(r, c)) d += `M${c} ${r}h1v1h-1z`;
      }
    }
    return { d, n };
  }, [text]);

  return (
    <svg className="mob-qr" width={size} height={size} viewBox={`-2 -2 ${path.n + 4} ${path.n + 4}`}>
      <rect x={-2} y={-2} width={path.n + 4} height={path.n + 4} fill="#ffffff" rx={1} />
      <path d={path.d} fill="#000000" />
    </svg>
  );
}

const folderName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? "";

/**
 * A phone showing this machine's spaces the way the Android app's home screen lays them out: the
 * desktop's name, anything waiting on you, then the spaces. Real data, so it doubles as an answer
 * to "what will my phone actually see".
 */
function PhonePreview({ on, live }: { on: boolean; live: boolean }) {
  const workspaces = useWorkspaces((s) => s.workspaces);
  const byPane = useAgentStatus((s) => s.byPane);

  const spaces = useMemo(() => workspaces.filter((w) => !w.archived), [workspaces]);
  // the bridge only mirrors terminals — viewer tabs and unsent composers have nothing to stream
  const paneCount = (w: (typeof spaces)[number]) =>
    w.sessions.filter((s) => !s.image && !s.file && !s.media && !s.diff && !s.draft).length;

  const waiting = useMemo(() => {
    const now = Date.now();
    return spaces.flatMap((w) =>
      w.sessions
        .filter((s) => displayState(byPane[s.id], now) === "waiting")
        .map((s) => ({ id: s.id, title: s.title, space: w.name })),
    );
  }, [spaces, byPane]);

  const shown = spaces.slice(0, PREVIEW_ROWS);
  const more = spaces.length - shown.length;

  return (
    <div className={`mob-phone${on ? "" : " off"}`}>
      <div className="mob-phone-frame">
        <span className="mob-phone-speaker" />
        <div className="mob-phone-screen">
          {on ? (
            <>
              <div className="mob-ph-top">
                <div className="mob-ph-host">This computer</div>
                <div className="mob-ph-sub">
                  {spaces.length} {spaces.length === 1 ? "space" : "spaces"}
                  {live ? " · live" : ""}
                </div>
              </div>

              {waiting.length > 0 && (
                <>
                  <div className="mob-ph-label">Waiting on you</div>
                  <div className="mob-ph-card">
                    {waiting.slice(0, 2).map((w) => (
                      <div className="mob-ph-row" key={w.id}>
                        <span className="mob-ph-dot waiting" />
                        <span className="mob-ph-rows">
                          <span className="mob-ph-name">{w.title}</span>
                          <span className="mob-ph-meta">{w.space}</span>
                        </span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              <div className="mob-ph-label">Spaces</div>
              <div className="mob-ph-card">
                {shown.length === 0 && <div className="mob-ph-none">No spaces yet</div>}
                {shown.map((w) => (
                  <div className="mob-ph-row" key={w.id}>
                    <span className="mob-ph-swatch" style={{ background: w.color }} />
                    <span className="mob-ph-rows">
                      <span className="mob-ph-name">{w.name}</span>
                      <span className="mob-ph-meta">
                        {folderName(w.cwd) || "No folder"} · {paneCount(w)}
                      </span>
                    </span>
                  </div>
                ))}
                {more > 0 && <div className="mob-ph-none">{more} more</div>}
              </div>
            </>
          ) : (
            <div className="mob-ph-idle">
              <Smartphone size={20} strokeWidth={1.5} />
              <span>Turn on to mirror your spaces here</span>
            </div>
          )}
        </div>
      </div>
      <div className="mob-phone-cap">{live ? "Your phone, live" : "What your phone will show"}</div>
    </div>
  );
}

export function MobileSettings() {
  const { enabled, port, token, address, remote, info, setEnabled, setPort, setAddress, setRemote, regenToken } =
    useBridge();
  const [copied, setCopied] = useState<"url" | "token" | null>(null);
  const [portText, setPortText] = useState(String(port));

  // Tailscale (or any new adapter) can come up while this page is open. Re-reading the interface
  // list on a slow timer is what lets the address appear without toggling the bridge off and on,
  // which is the difference between "install it and come back" working and not. Local enumeration
  // only, no network.
  useEffect(() => {
    if (!enabled) return;
    let last = "";
    const t = setInterval(() => {
      bridgeStatus()
        .then((i) => {
          // only publish a real change, or the preview would re-render every tick for nothing
          const sig = JSON.stringify(i);
          if (sig === last) return;
          last = sig;
          useBridge.getState().setInfo(i);
        })
        .catch(() => {});
    }, ADDR_POLL_MS);
    return () => clearInterval(t);
  }, [enabled]);

  const url = pairingUrl(info, token, port, address, remote);
  const running = !!info?.running;
  const reachable = running && !!info?.address;
  const peers = info?.peers ?? [];
  // plain ws:// over the internet hands out the pairing code and everything the agents print
  const insecure = /^ws:\/\//i.test(remote.trim());
  const ts = tailscaleAddr(info);
  const onTailscale = !!ts && remote.trim() === ts.ip;

  const copy = async (what: "url" | "token", text: string) => {
    await writeText(text);
    setCopied(what);
    setTimeout(() => setCopied(null), 1400);
  };

  return (
    <>
      {/* the switch and what it's doing right now, as one strip rather than a labelled section */}
      <div className="mob-hero">
        <div className="mob-hero-text">
          <div className="mob-hero-title">Sync to your phone</div>
          <div className="mob-hero-desc">
            Runs a small server on your wifi so the app on your phone can see your spaces, mirror a
            terminal, and type into it. Nothing leaves your network and no account is involved.
          </div>
          <div className="mob-hero-foot">
            <span className={`mob-state${enabled ? (reachable ? " on" : " pending") : ""}`}>
              <span className="mob-state-dot" />
              {enabled ? (running ? `Listening on ${info.port}` : "Starting") : "Off"}
            </span>
            <button className="mob-link" onClick={() => void openUrl(ANDROID_APK_URL)}>
              <Download size={12} />
              Get the Android app
            </button>
          </div>
        </div>
        <button
          className={`toggle ${enabled ? "on" : ""}`}
          onClick={() => setEnabled(!enabled)}
          aria-pressed={enabled}
          aria-label="Sync to your phone"
        >
          <span className="toggle-knob" />
        </button>
      </div>

      {enabled && (
        <div className="set-section">
          <div className="set-label">Pairing</div>
          <div className="mob-card">
            {reachable ? (
              <>
                <div className="mob-pair">
                  <div className="mob-pair-main">
                    <div className="mob-pair-title">Scan to pair</div>
                    <div className="mob-pair-desc">
                      Open HyprSpace on your phone, tap Pair, and point it at this code.
                    </div>
                    <div className="mob-pair-row">
                      <Qr text={url} />
                      <div className="mob-fields">
                        <div className="mob-field">
                          <span className="mob-field-key">Address</span>
                          {(info.addresses ?? []).length > 1 ? (
                            <select
                              className="mob-select"
                              value={address || info.address || ""}
                              onChange={(e) => setAddress(e.target.value)}
                            >
                              {(info.addresses ?? []).map((a) => (
                                <option key={a.ip} value={a.ip}>
                                  {a.ip} ({a.label})
                                  {a.preferred ? " · this network" : ""}
                                </option>
                              ))}
                            </select>
                          ) : (
                            <code>
                              {info.address}:{info.port}
                            </code>
                          )}
                        </div>
                        <div className="mob-field">
                          <span className="mob-field-key">Code</span>
                          <code className="mob-token">{token}</code>
                        </div>
                        <div className="mob-actions">
                          <button className="btn" onClick={() => void copy("url", url)}>
                            {copied === "url" ? <Check size={13} /> : <Copy size={13} />}
                            {copied === "url" ? "Copied" : "Copy link"}
                          </button>
                          <button className="btn" onClick={() => void copy("token", token)}>
                            {copied === "token" ? <Check size={13} /> : <Copy size={13} />}
                            {copied === "token" ? "Copied" : "Copy code"}
                          </button>
                          <button className="btn" onClick={regenToken} title="Every paired phone will have to scan again">
                            <RefreshCw size={13} />
                            New code
                          </button>
                        </div>
                      </div>
                    </div>
                  </div>
                  <PhonePreview on={enabled} live={peers.length > 0} />
                </div>
              </>
            ) : (
              <div className="mob-empty">
                <WifiOff size={16} strokeWidth={1.75} />
                <div>
                  {running
                    ? "Could not find this machine's network address. Check that you are on a network, not just loopback."
                    : "Starting the bridge."}
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {enabled && (
        <div className="set-section">
          <div className="set-label">Network</div>
          <div className="set-group">
            <div className="set-row">
              <div className="set-row-info">
                <div className="set-key">Port</div>
                <div className="set-desc">
                  {running && info.port !== port
                    ? `The one you picked was busy, so it took ${info.port}`
                    : `Default is ${DEFAULT_BRIDGE_PORT}`}
                </div>
              </div>
              <div className="set-control">
                <input
                  className="mob-port"
                  value={portText}
                  inputMode="numeric"
                  onChange={(e) => setPortText(e.target.value.replace(/\D/g, "").slice(0, 5))}
                  onBlur={() => {
                    const n = Number(portText);
                    const next = n >= 1024 && n <= 65535 ? n : DEFAULT_BRIDGE_PORT;
                    setPortText(String(next));
                    setPort(next);
                  }}
                />
              </div>
            </div>

            {/* Reaching this machine from outside the wifi. Tailscale is the easy answer, so it
                gets the space and the one-click apply; the free-text field stays for tunnels. */}
            <div className={`mob-ts${ts ? " found" : ""}`}>
              <span className="mob-ts-mark">
                {ts ? <ShieldCheck size={15} /> : <Globe size={15} />}
              </span>
              <div className="mob-ts-body">
                <div className="mob-ts-title">
                  {ts
                    ? onTailscale
                      ? "Tailscale is set up"
                      : "Tailscale is running on this machine"
                    : "Reach it from anywhere with Tailscale"}
                </div>
                <div className="mob-ts-desc">
                  {ts ? (
                    onTailscale ? (
                      <>
                        Your phone can reach this machine on <code>{ts.ip}</code> from anywhere. A
                        phone you paired before now needs to scan again.
                      </>
                    ) : (
                      "One click puts its address in the QR as a second way in, so your phone still finds this machine when you are out."
                    )
                  ) : (
                    "A free VPN that puts this machine and your phone on one private network. Nothing is exposed to the internet and there are no ports to forward."
                  )}
                </div>
                {!ts && (
                  <ol className="mob-ts-steps">
                    <li>Install Tailscale on this computer and on your phone</li>
                    <li>Sign in to the same account on both</li>
                    <li>Come back here. The address fills itself in.</li>
                  </ol>
                )}
              </div>
              <div className="mob-ts-act">
                {ts ? (
                  onTailscale ? (
                    <span className="mob-ts-on">
                      <Check size={13} />
                      In use
                    </span>
                  ) : (
                    <button className="btn primary" onClick={() => setRemote(ts.ip)}>
                      Use {ts.ip}
                    </button>
                  )
                ) : (
                  <button className="btn" onClick={() => void openUrl(TAILSCALE_URL)}>
                    <ExternalLink size={13} />
                    Install Tailscale
                  </button>
                )}
              </div>
            </div>

            <div className="set-row mob-remote-row">
              <div className="set-row-info">
                <div className="set-key">Away address</div>
                <div className="set-desc">
                  What goes in the QR as the second way in. Tailscale fills this for you. A tunnel's
                  public <code>wss://</code> URL works too.
                </div>
              </div>
              <div className="set-control">
                <input
                  className="mob-remote"
                  value={remote}
                  spellCheck={false}
                  placeholder="100.90.1.2 or wss://box.example.com"
                  onChange={(e) => setRemote(e.target.value)}
                />
              </div>
            </div>
            {/* only worth saying when they have actually typed the unsafe thing */}
            {insecure && (
              <div className="mob-warn">
                <TriangleAlert size={13} />
                <span>
                  Plain <code>ws://</code> over the internet sends your pairing code and everything
                  your agents print in the clear. Use <code>wss://</code> or a VPN address.
                </span>
              </div>
            )}
          </div>
        </div>
      )}

      {enabled && (
        <div className="set-section">
          <div className="set-label">Connected</div>
          <div className="set-group">
            {peers.length ? (
              peers.map((p) => (
                <div className="set-row" key={p.id}>
                  <div className="set-row-info">
                    <div className="set-key mob-peer">
                      <Smartphone size={14} strokeWidth={1.75} />
                      {peerLabel(p)}
                    </div>
                    <div className="set-desc">
                      {p.addr} · connected {relTime(p.since * 1000)}
                    </div>
                  </div>
                  <div className="set-control">
                    <span className="mob-live">
                      <span className="mob-live-dot" />
                      live
                    </span>
                  </div>
                </div>
              ))
            ) : (
              <div className="mob-empty">
                <Smartphone size={16} strokeWidth={1.75} />
                <div>No phone connected yet. Scan the code above to pair one.</div>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
