import { useEffect, useState } from "react";
import { FolderOpen, X } from "lucide-react";
import { readImageFile, revealPath } from "../api";

interface Props {
  path: string;
  active: boolean;
  onClose: () => void;
}

export function ImageViewer({ path, onClose }: Props) {
  const [src, setSrc] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [actual, setActual] = useState(false); // click toggles fit ↔ 1:1

  useEffect(() => {
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
  }, [path]);

  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  // reveal-in-folder wants the containing directory (reveal_path opens a folder, not a file)
  const sep = path.includes("\\") ? "\\" : "/";
  const dir = path.split(/[\\/]/).slice(0, -1).join(sep) || path;

  return (
    <div className="image-viewer">
      <div className="iv-bar">
        <span className="iv-name" title={path}>
          {name}
        </span>
        <button className="iv-btn" title="Reveal in folder" onClick={() => void revealPath(dir).catch(() => {})}>
          <FolderOpen size={13} /> Reveal
        </button>
        <button className="iv-btn iv-close" title="Close image" onClick={onClose}>
          <X size={13} /> Close
        </button>
      </div>
      {err ? (
        <div className="iv-error">
          <div className="iv-error-msg">couldn't open image</div>
          <div className="iv-error-path">{path}</div>
          <div className="iv-error-detail">{err}</div>
        </div>
      ) : (
        <div className={`iv-stage${actual ? " actual" : ""}`}>
          {src && (
            <img
              className="iv-img"
              src={src}
              alt={name}
              onClick={() => setActual((a) => !a)}
              onError={() => setErr("failed to decode image")}
            />
          )}
        </div>
      )}
    </div>
  );
}
