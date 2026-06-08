import { useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// checks GitHub releases once on launch; if there's a newer signed build, offers a one-click update
export function Updater() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => {
    let cancelled = false;
    check()
      .then((u) => {
        if (!cancelled && u?.available) setUpdate(u);
      })
      .catch(() => {
        /* offline / no release yet — just don't show anything */
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!update) return null;

  const run = async () => {
    setBusy(true);
    setStatus("downloading…");
    try {
      let total = 0;
      let got = 0;
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") {
          total = e.data.contentLength ?? 0;
        } else if (e.event === "Progress") {
          got += e.data.chunkLength;
          setStatus(total ? `downloading ${Math.round((got / total) * 100)}%` : "downloading…");
        } else if (e.event === "Finished") {
          setStatus("installing…");
        }
      });
      await relaunch();
    } catch (err) {
      console.error("update failed:", err);
      setStatus("update failed — try again later");
      setBusy(false);
    }
  };

  return (
    <div className="updater">
      <span className="updater-dot" />
      <span className="updater-text">
        {busy ? status : `Update ${update.version} available`}
      </span>
      {!busy && (
        <>
          <button className="updater-btn" onClick={run}>
            Restart &amp; update
          </button>
          <button className="updater-x" title="Later" onClick={() => setUpdate(null)}>
            ×
          </button>
        </>
      )}
    </div>
  );
}
