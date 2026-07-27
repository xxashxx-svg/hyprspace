// Settings → Mobile: turn the LAN bridge on, show the pairing QR the Android app scans, and list
// whatever phones are connected right now.
import { useMemo, useState } from "react";
import qrcode from "qrcode-generator";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, Check, RefreshCw, Smartphone, WifiOff } from "lucide-react";
import { DEFAULT_BRIDGE_PORT, pairingUrl, peerLabel, useBridge } from "../stores/bridge";
import { relTime } from "../lib/time";

// the stable name the release workflow attaches, so this link never goes stale between versions
const ANDROID_APK_URL =
  "https://github.com/xxashxx-svg/hyprspace/releases/latest/download/HyprSpace-android.apk";

// draw the QR ourselves rather than pulling a renderer: one <svg> path of dark modules, so it
// inherits the theme's text color and stays crisp at any size
function Qr({ text, size = 208 }: { text: string; size?: number }) {
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

export function MobileSettings() {
  const { enabled, port, token, address, remote, info, setEnabled, setPort, setAddress, setRemote, regenToken } =
    useBridge();
  const [copied, setCopied] = useState<"url" | "token" | null>(null);
  const [portText, setPortText] = useState(String(port));

  const url = pairingUrl(info, token, port, address, remote);
  const running = !!info?.running;
  const reachable = running && !!info?.address;

  const copy = async (what: "url" | "token", text: string) => {
    await writeText(text);
    setCopied(what);
    setTimeout(() => setCopied(null), 1400);
  };

  return (
    <>
      <div className="set-section">
        <div className="set-label">Phone</div>
        <div className="set-group">
          <div className="set-row">
            <div className="set-row-info">
              <div className="set-key">Sync to your phone</div>
              <div className="set-desc">
                Runs a small server on your local network so the HyprSpace app on your phone can see
                your spaces, mirror a terminal live, and type into it. Nothing leaves your wifi and no
                account is involved.{" "}
                <button className="mob-link" onClick={() => void openUrl(ANDROID_APK_URL)}>
                  Get the Android app
                </button>
              </div>
            </div>
            <div className="set-control">
              <button
                className={`toggle ${enabled ? "on" : ""}`}
                onClick={() => setEnabled(!enabled)}
                aria-pressed={enabled}
              >
                <span className="toggle-knob" />
              </button>
            </div>
          </div>

          <div className="set-row">
            <div className="set-row-info">
              <div className="set-key">Port</div>
              <div className="set-desc">
                {running
                  ? `Listening on ${info.port}${info.port !== port ? " (the one you picked was busy)" : ""}`
                  : "Default is 6768"}
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
        </div>
      </div>

      {enabled && (
        <div className="set-section">
          <div className="set-label">Pairing</div>
          <div className="set-group pad">
            {reachable ? (
              <div className="mob-pair">
                <Qr text={url} />
                <div className="mob-pair-side">
                  <div className="mob-pair-title">Scan this in the HyprSpace app</div>
                  <div className="mob-pair-desc">
                    Your phone has to be on the same wifi as this computer — unless you add a way in
                    from outside below, which the app falls back to automatically.
                  </div>
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
                            {a.ip} — {a.label}
                            {a.preferred ? " (this network)" : ""}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <code>{info.address}</code>
                    )}
                    <code>:{info.port}</code>
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
                    <button
                      className="btn"
                      onClick={regenToken}
                      title="Every paired phone will have to scan again"
                    >
                      <RefreshCw size={13} />
                      New code
                    </button>
                  </div>
                </div>
              </div>
            ) : (
              <div className="mob-empty">
                <WifiOff size={16} strokeWidth={1.75} />
                <div>
                  {running
                    ? "Couldn't work out this machine's network address — check that you're on a network, not just loopback."
                    : "Starting the bridge…"}
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {enabled && (
        <div className="set-section">
          <div className="set-label">Reaching it from outside</div>
          <div className="set-group">
            <div className="set-row">
              <div className="set-row-info">
                <div className="set-key">Away address</div>
                <div className="set-desc">
                  Goes in the QR as a second way in, so the phone still finds this machine off your
                  wifi. A VPN address works best — install Tailscale on both and use the 100.x.x.x one
                  it gives this machine; it'll show in the dropdown above too. A tunnel's public URL
                  (<code>wss://…</code>) works as well.{" "}
                  <strong>Don't just forward the port</strong> — plain <code>ws://</code> over the
                  internet sends your pairing code and everything your agents print in the clear.
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
          </div>
        </div>
      )}

      {enabled && (
        <div className="set-section">
          <div className="set-label">Connected</div>
          <div className="set-group">
            {info?.peers.length ? (
              info.peers.map((p) => (
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
                    <span className="mob-live">live</span>
                  </div>
                </div>
              ))
            ) : (
              <div className="mob-empty">
                <Smartphone size={16} strokeWidth={1.75} />
                <div>No phone connected yet.</div>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
