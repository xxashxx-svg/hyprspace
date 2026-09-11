import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import { ExternalLink, FolderOpen } from "lucide-react";
import { revealPath } from "../api";
import { revealLabel } from "../platform";

const AUDIO = /\.(?:mp3|wav|flac|ogg|m4a|aac|opus)$/i;

interface Props {
  path: string;
  active: boolean;
}

/**
 * A video or audio file in a pane. The file streams straight from disk through the asset
 * protocol, so a large recording does not get read into memory. Formats the webview cannot decode
 * fall back to a button that opens the file in its default app.
 */
export function MediaViewer({ path, active }: Props) {
  const [failed, setFailed] = useState(false);
  const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  const src = convertFileSrc(path);
  const isAudio = AUDIO.test(path);

  return (
    <div className={`mv${active ? " active" : ""}`}>
      <div className="mv-stage">
        {failed ? (
          <div className="mv-fallback">
            <p>This format does not play here.</p>
            <button className="btn" onClick={() => void openPath(path).catch(() => {})}>
              <ExternalLink size={13} />
              Open in the default app
            </button>
          </div>
        ) : isAudio ? (
          <audio className="mv-audio" controls src={src} onError={() => setFailed(true)} />
        ) : (
          <video className="mv-video" controls src={src} onError={() => setFailed(true)} />
        )}
      </div>
      <div className="mv-bar">
        <span className="mv-name" title={path}>
          {name}
        </span>
        <button className="iv-btn" title={revealLabel} onClick={() => void revealPath(path).catch(() => {})}>
          <FolderOpen size={13} />
        </button>
      </div>
    </div>
  );
}
