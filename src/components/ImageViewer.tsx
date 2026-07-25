import { useEffect, useState } from "react";
import { FolderOpen, RotateCw, X } from "lucide-react";
import { readImageFile, revealPath } from "../api";

interface Props {
  path: string;
  active: boolean;
  onClose: () => void;
  /** the tab strip already shows the filename and a close × — don't repeat them */
  tabbed?: boolean;
}

export function ImageViewer({ path, active, onClose, tabbed }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [actual, setActual] = useState(false); // click toggles fit ↔ 1:1
  const [nonce, setNonce] = useState(0); // bumped by Retry to re-read the file

  // only hold the bytes while this tab is on screen. hidden tabs stay mounted, and a data URL is
  // ~1.33x the file, so a few open images would otherwise sit on tens of MB each for nothing.
  useEffect(() => {
    if (!active) {
      setSrc(null);
      setErr(null);
      return;
    }
    let alive = true;
    setSrc(null);
    setErr(null);
    setActual(false);
    readImageFile(path)
      .then((url) => alive && setSrc(url))
      .catch((e) => alive && setErr(String(e)));
    return () => {
      alive = false;
    };
  }, [path, active, nonce]);

  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  // reveal-in-folder wants the containing directory (reveal_path opens a folder, not a file)
  const sep = path.includes("\\") ? "\\" : "/";
  const dir = path.split(/[\\/]/).slice(0, -1).join(sep) || path;

  return (
    <div className="image-viewer">
      <div className="iv-bar">
        {!tabbed && (
          <span className="iv-name" title={path}>
            {name}
          </span>
        )}
        <span className="iv-gap" />
        <button className="iv-btn" title="Reveal in folder" onClick={() => void revealPath(dir).catch(() => {})}>
          <FolderOpen size={13} /> Reveal
        </button>
        {!tabbed && (
          <button className="iv-btn iv-close" title="Close image" onClick={onClose}>
            <X size={13} /> Close
          </button>
        )}
      </div>
      {err ? (
        <div className="iv-error">
          <div className="iv-error-msg">couldn't open image</div>
          <div className="iv-error-path">{path}</div>
          <div className="iv-error-detail">{err}</div>
          <button className="iv-btn iv-retry" onClick={() => setNonce((n) => n + 1)}>
            <RotateCw size={13} /> Retry
          </button>
        </div>
      ) : (
        <div className={`iv-stage${actual ? " actual" : ""}`}>
          {src ? (
            <img
              className="iv-img"
              src={src}
              alt={name}
              onClick={() => setActual((a) => !a)}
              onError={() => setErr("failed to decode image")}
            />
          ) : (
            active && <div className="iv-loading">loading…</div>
          )}
        </div>
      )}
    </div>
  );
}
