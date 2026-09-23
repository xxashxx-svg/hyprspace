import { useEffect, useReducer, useRef, useState } from "react";
import { ArrowUp, Check, ChevronDown, CornerDownLeft, FileText, Folder, FolderOpen, GitBranch, ImagePlus, Sparkles, Terminal as TerminalIcon, X } from "lucide-react";
import { useWorkspaces } from "../../stores/workspace";
import { useSettings } from "../../stores/settings";
import { useUi } from "../../stores/ui";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { clipboardImageToTemp, getHomeDir, gitClone, pickFile, pickFolder, readImageFile, type AgentSession } from "../../api";
import { parseRepoUrl, splitRepoPrompt, type RepoRef } from "../../lib/repoUrl";
import { ClonePanel } from "./ClonePanel";
import { RecentSessions } from "./RecentSessions";
import { branchOf, onBranchResolved } from "../../lib/branches";
import { commandFor, resumeCmd } from "../../actions";
import { queuePrompt, withFiles } from "../../lib/composer";
import { CATALOG, EFFORT_LABEL, effortNote, effortsFor, modelLabel, type ProviderId } from "../../lib/models";
import { useProviders, catalogFor } from "../../stores/providers";
import { PROVIDER_COLOR, PROVIDER_LOGO, PROVIDER_NAME } from "../../lib/brand";
import { ModelPicker, type ModelChoice } from "./ModelPicker";
import { EffortSlider } from "./EffortSlider";
import { Menu } from "../Menu";

interface Props {
  /** absent on a fresh install: the composer then asks for a folder and makes the first thread */
  wsId?: string;
  /** the draft session this composer lives in; absent for an empty space or the home page */
  sessionId?: string;
  /** show the space switcher chip (home page only) */
  spacePicker?: boolean;
  /** compact heading, for a pane inside the grid */
  compact?: boolean;
}

const IMAGE_EXT = /\.(?:png|jpe?g|gif|webp|bmp|svg|avif)$/i;

/** A file that goes in with the prompt: its path, and a preview when it is an image. */
interface Attachment {
  path: string;
  name: string;
  preview?: string;
}

function shortPath(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? "…/" + parts.slice(-2).join("/") : parts.join("/") || p;
}

/**
 * The composer: where a session starts. Pick the agent and model, type a task, press Enter.
 * It opens a real terminal pane and types the task into the CLI once it is up.
 */
export function ComposerPane({ wsId, sessionId, spacePicker, compact }: Props) {
  const ws = useWorkspaces((s) => s.workspaces.find((w) => w.id === wsId));
  const workspaces = useWorkspaces((s) => s.workspaces);
  const session = ws?.sessions.find((s) => s.id === sessionId);
  // a space with no folder of its own: the folder is picked here, before the launch
  const [pickedFolder, setPickedFolder] = useState("");
  const cwd = session?.cwd || ws?.cwd || pickedFolder;
  const chooseFolder = async () => {
    const picked = await pickFolder();
    if (!picked) return null;
    setPickedFolder(picked);
    const st = useWorkspaces.getState();
    if (!ws) {
      // nothing open yet: the picked folder becomes the first thread
      st.addWorkspace(picked.split(/[\\/]/).filter(Boolean).pop() || "Project", picked);
    } else if (!ws.cwd && !session?.cwd) {
      // a space that had no folder gets this one, so nothing asks for it again
      st.setWorkspaceCwd(ws.id, picked);
    } else if (spacePicker) {
      // a different folder means a different space: reuse one for it or open one
      const same = (a: string, b: string) => a.replace(/[\\/]+$/, "").toLowerCase() === b.replace(/[\\/]+$/, "").toLowerCase();
      const existing = st.workspaces.find((w) => w.cwd && same(w.cwd, picked));
      if (existing) st.setActive(existing.id);
      else st.addWorkspace(picked.split(/[\\/]/).filter(Boolean).pop() || "Project", picked);
    }
    return picked;
  };

  const codexModels = useProviders((s) => s.codexModels);
  const lastProvider = useSettings((s) => s.lastProvider);
  const agentModel = useSettings((s) => s.agentModel);
  const agentEffort = useSettings((s) => s.agentEffort);
  const provider = (CATALOG[lastProvider as ProviderId] ? lastProvider : "claude") as ProviderId;
  const choice: ModelChoice = {
    provider,
    model: agentModel[provider] ?? "",
    effort: agentEffort[provider] ?? "",
  };
  const setChoice = (next: ModelChoice) => {
    const s = useSettings.getState();
    s.setLastProvider(next.provider);
    s.setAgentModel(next.provider, next.model);
    s.setAgentEffort(next.provider, next.effort);
  };

  const [text, setText] = useState("");
  const [menu, setMenu] = useState<"model" | "effort" | "space" | null>(null);

  // files that go in with the prompt: a pasted screenshot lands in a temp file, a picked file is
  // used where it is. Their paths are typed in after the text.
  const [files, setFiles] = useState<Attachment[]>([]);
  const attach = (path: string) => {
    const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    setFiles((list) => (list.some((f) => f.path === path) ? list : [...list, { path, name }]));
    if (IMAGE_EXT.test(path)) {
      readImageFile(path)
        .then((src) => setFiles((list) => list.map((f) => (f.path === path ? { ...f, preview: src } : f))))
        .catch(() => {});
    }
  };
  const onPaste = (e: React.ClipboardEvent) => {
    const items = Array.from(e.clipboardData?.items ?? []);
    if (!items.some((it) => it.type.startsWith("image/"))) return;
    e.preventDefault();
    void clipboardImageToTemp()
      .then((path) => path && attach(path))
      .catch(() => {});
  };
  const prompt = () => withFiles(text, files.map((f) => f.path));
  const reset = () => {
    setText("");
    setFiles([]);
  };

  // A repository link at the start of the text turns the composer into a clone: the card asks
  // where, and the words after the link become the first task in the new space.
  const cloneReq = splitRepoPrompt(text);
  const [cloneParent, setCloneParent] = useState("");
  const [cloneName, setCloneName] = useState("");
  const [cloning, setCloning] = useState(false);
  const [cloneError, setCloneError] = useState<string | null>(null);
  const repoUrl = cloneReq?.repo.url;
  useEffect(() => {
    if (!repoUrl) return;
    setCloneName(parseRepoUrl(repoUrl)?.name ?? "");
    setCloneError(null);
  }, [repoUrl]);
  // the clone lands inside the folder this composer is in; with no folder, the home directory
  useEffect(() => {
    if (!repoUrl || cloneParent) return;
    if (cwd) setCloneParent(cwd);
    else void getHomeDir().then((h) => setCloneParent((p) => p || h)).catch(() => {});
  }, [repoUrl, cwd, cloneParent]);

  // a repository link on the clipboard gets a one-click offer while the box is empty
  const [clipRepo, setClipRepo] = useState<RepoRef | null>(null);
  useEffect(() => {
    let alive = true;
    const check = () =>
      readText()
        .then((t) => alive && setClipRepo(parseRepoUrl(t ?? "")))
        .catch(() => alive && setClipRepo(null));
    check();
    window.addEventListener("focus", check);
    return () => {
      alive = false;
      window.removeEventListener("focus", check);
    };
  }, []);
  // Files dropped on this composer arrive as a DOM event from App's window drop handler, which
  // aims it at whichever composer is under the cursor. A ref holds the latest `attach` so the
  // listener is bound once rather than rebound every keystroke.
  const rootRef = useRef<HTMLDivElement>(null);
  const attachRef = useRef(attach);
  attachRef.current = attach;
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const onFiles = (e: Event) => {
      for (const path of (e as CustomEvent<string[]>).detail ?? []) attachRef.current(path);
    };
    el.addEventListener("hyprspace-files", onFiles);
    return () => el.removeEventListener("hyprspace-files", onFiles);
  }, []);

  const taRef = useRef<HTMLTextAreaElement>(null);
  const modelChip = useRef<HTMLButtonElement>(null);
  const effortChip = useRef<HTMLButtonElement>(null);
  const spaceChip = useRef<HTMLButtonElement>(null);

  const [, bump] = useReducer((x: number) => x + 1, 0);
  useEffect(() => onBranchResolved(bump), []);
  const branch = cwd ? branchOf(cwd) : undefined;

  // grow with the prompt, up to the css max-height
  useEffect(() => {
    const el = taRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  }, [text]);

  const clone = async () => {
    if (!cloneReq || !cloneParent || !cloneName.trim() || cloning) return;
    setCloning(true);
    setCloneError(null);
    try {
      const path = await gitClone(cloneReq.repo.url, cloneParent, cloneName.trim());
      const st = useWorkspaces.getState();
      const id = st.addWorkspace(cloneName.trim(), path);
      useUi.getState().goSpace();
      if (cloneReq.rest) {
        const cmd = commandFor(choice.provider, { model: choice.model, effort: choice.effort });
        const paneId = st.addSession(id, cmd, path);
        if (choice.model) st.setSessionModel(paneId, choice.model);
        queuePrompt(paneId, cmd, withFiles(cloneReq.rest, files.map((f) => f.path)));
      }
      reset();
    } catch (e) {
      setCloneError(String(e));
    } finally {
      setCloning(false);
    }
  };

  // reopen a saved conversation as a pane; any text typed goes in as its next prompt
  const resume = (s: AgentSession) => {
    if (!ws || !cwd) return;
    const cmd = resumeCmd(provider, s.id, { model: choice.model, effort: choice.effort });
    if (!cmd) return;
    const st = useWorkspaces.getState();
    let paneId: string;
    if (session) {
      st.startDraft(session.id, cmd, choice.model || undefined);
      paneId = session.id;
    } else {
      paneId = st.addSession(ws.id, cmd, cwd);
      if (choice.model) st.setSessionModel(paneId, choice.model);
    }
    st.renameSession(paneId, s.title);
    st.setFocused(paneId);
    useUi.getState().goSpace();
    queuePrompt(paneId, cmd, prompt());
    reset();
  };

  const submit = async () => {
    if (cloneReq) return clone();
    let folder = cwd;
    let wsId = ws?.id;
    if (!folder || !wsId) {
      const picked = await chooseFolder();
      if (!picked) return;
      folder = picked;
      // with nothing open, picking made a thread and switched to it
      wsId = ws?.id ?? useWorkspaces.getState().activeId ?? undefined;
      if (!wsId) return;
    }
    const cmd = commandFor(choice.provider, { model: choice.model, effort: choice.effort });
    const st = useWorkspaces.getState();
    let paneId: string;
    if (session) {
      st.startDraft(session.id, cmd, choice.model || undefined);
      paneId = session.id;
    } else {
      paneId = st.addSession(wsId, cmd, folder);
      if (choice.model) st.setSessionModel(paneId, choice.model);
    }
    st.setFocused(paneId);
    useUi.getState().goSpace();
    queuePrompt(paneId, cmd, prompt());
    reset();
  };

  const cat = catalogFor(provider, codexModels);
  const efforts = effortsFor(provider, choice.model, cat);
  const isShell = provider === "terminal" || provider === "wsl";
  const name = ws?.name ?? "";
  // "ultrathink" is a Claude Code keyword: it asks Claude to think its hardest on this message.
  // the other CLIs read it as plain text, so only light it up for Claude
  const ultra = provider === "claude" && /\bultrathink\b/i.test(text);

  return (
    <div className={`composer-pane${compact ? " compact" : ""}`} ref={rootRef}>
      <div className="composer-wrap">
        <h2 className="composer-title">
          What should we work on{ws ? <> in <span>{name}</span></> : null}?
        </h2>

        <div className={`composer${ultra ? " ultra" : ""}`} style={{ "--brand": PROVIDER_COLOR[provider] ?? "var(--accent)" } as React.CSSProperties}>
          <div className="composer-top">
            {spacePicker && !ws && (
              <button className="composer-chip" onClick={() => void chooseFolder()}>
                <FolderOpen size={13} />
                <span className="composer-chip-label">Pick a folder</span>
              </button>
            )}
            {spacePicker && ws && (
              <>
                <button ref={spaceChip} className="composer-chip" onClick={() => setMenu(menu === "space" ? null : "space")}>
                  <Folder size={13} />
                  <span className="composer-chip-label">{name}</span>
                  <ChevronDown size={12} />
                </button>
                <Menu open={menu === "space"} anchorRef={spaceChip} onClose={() => setMenu(null)} compact>
                  {workspaces.filter((w) => !w.archived).map((w) => (
                    <button
                      key={w.id}
                      className={`pop-menu-item${w.id === ws?.id ? " on" : ""}`}
                      title={w.cwd || "Asks for a folder"}
                      onClick={() => {
                        useWorkspaces.getState().setActive(w.id);
                        setMenu(null);
                      }}
                    >
                      <span className="pop-menu-title">{w.name}</span>
                      {w.id === ws?.id && <Check size={13} className="pop-menu-check" />}
                    </button>
                  ))}
                  <div className="pop-menu-sep" />
                  <button
                    className="pop-menu-item"
                    onClick={() => {
                      setMenu(null);
                      void chooseFolder();
                    }}
                  >
                    <FolderOpen size={13} />
                    <span className="pop-menu-title">{cwd ? "Choose a different folder" : "Choose a folder"}</span>
                  </button>
                </Menu>
              </>
            )}
            {spacePicker && !cwd ? null : cwd && !(ws?.cwd || session?.cwd) ? (
              <button className="composer-chip" title={`${cwd}. Click to change.`} onClick={() => void chooseFolder()}>
                <Folder size={13} />
                <span className="composer-chip-label">{shortPath(cwd)}</span>
                <ChevronDown size={12} />
              </button>
            ) : cwd && ws?.cwd && cwd !== ws.cwd ? (
              // a thread in a subfolder of the space: show only the part below the space
              <span className="composer-path" title={cwd}>
                <Folder size={12} />
                {cwd.toLowerCase().startsWith(ws.cwd.toLowerCase()) ? cwd.slice(ws.cwd.length).replace(/^[\\/]+/, "") : shortPath(cwd)}
              </span>
            ) : cwd ? null : (
              <button className="composer-chip" onClick={() => void chooseFolder()}>
                <Folder size={13} />
                <span className="composer-chip-label">Choose a folder</span>
              </button>
            )}
            {branch && (
              <span className="composer-branch">
                <GitBranch size={12} />
                {branch}
              </span>
            )}
          </div>

          <textarea
            ref={taRef}
            className="composer-input"
            autoFocus
            rows={2}
            spellCheck={false}
            placeholder={isShell ? "Command to run, or leave empty for a shell" : "Describe the task, paste an image, or leave empty to open the agent"}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onPaste={onPaste}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                void submit();
              }
            }}
          />

          {files.length > 0 && (
            <div className="composer-files">
              {files.map((f) => (
                <div key={f.path} className={`composer-file${f.preview ? " img" : ""}`} title={f.path}>
                  {f.preview ? <img src={f.preview} alt="" /> : <FileText size={14} />}
                  {!f.preview && <span className="composer-file-name">{f.name}</span>}
                  <button className="composer-file-x" title="Remove" onClick={() => setFiles((l) => l.filter((x) => x.path !== f.path))}>
                    <X size={11} />
                  </button>
                </div>
              ))}
            </div>
          )}

          <div className="composer-bottom">
            <button ref={modelChip} className="composer-chip" onClick={() => setMenu(menu === "model" ? null : "model")}>
              {PROVIDER_LOGO[provider] ? <img src={PROVIDER_LOGO[provider]} alt="" width={14} height={14} /> : <TerminalIcon size={14} />}
              <span className="composer-chip-label">
                {isShell || !choice.model ? PROVIDER_NAME[provider] : modelLabel(provider, choice.model, cat)}
              </span>
              <ChevronDown size={12} />
            </button>
            <ModelPicker
              open={menu === "model"}
              anchorRef={modelChip}
              value={choice}
              onChange={(next) => {
                setChoice(next);
                taRef.current?.focus();
              }}
              onClose={() => setMenu(null)}
            />

            {efforts.length > 0 && (
              <>
                <button ref={effortChip} className="composer-chip" onClick={() => setMenu(menu === "effort" ? null : "effort")}>
                  <span className="composer-chip-label">{choice.effort ? EFFORT_LABEL[choice.effort] ?? choice.effort : "Effort"}</span>
                  <ChevronDown size={12} />
                </button>
                <EffortSlider
                  open={menu === "effort"}
                  anchorRef={effortChip}
                  provider={provider}
                  modelLabel={choice.model ? modelLabel(provider, choice.model, cat) : PROVIDER_NAME[provider]}
                  levels={efforts}
                  noteFor={(lvl) => effortNote(provider, choice.model, lvl, cat)}
                  value={choice.effort}
                  onChange={(lvl) => setChoice({ ...choice, effort: lvl })}
                  onClose={() => setMenu(null)}
                />
              </>
            )}
            {ultra && (
              <span className="ultra-badge" title="Claude thinks its hardest on this message">
                <Sparkles size={12} />
                <span className="ultra-badge-text">Ultrathink</span>
              </span>
            )}

            <button
              className="composer-icon"
              title="Attach a file or image"
              onClick={() => void pickFile().then((p) => p && attach(p))}
            >
              <ImagePlus size={15} />
            </button>
            <button className="composer-send" title="Start (Enter)" onClick={() => void submit()}>
              <ArrowUp size={15} />
            </button>
          </div>
        </div>

        {cloneReq ? (
          <ClonePanel
            repo={cloneReq.repo}
            parent={cloneParent}
            name={cloneName}
            busy={cloning}
            error={cloneError}
            onParent={() => void pickFolder().then((p) => p && setCloneParent(p))}
            onName={setCloneName}
            onClone={() => void clone()}
          />
        ) : (
          <>
            {clipRepo && !text && (
              <button className="suggest" onClick={() => setText(clipRepo.url)}>
                <span className="suggest-mark">
                  <GitBranch size={13} />
                </span>
                <span className="suggest-text">
                  <b>Clone {clipRepo.label}</b>
                  <span>The link is on your clipboard</span>
                </span>
                <span className="suggest-go">
                  <CornerDownLeft size={12} />
                </span>
              </button>
            )}
            <RecentSessions key={`${provider}:${cwd}`} provider={provider} cwd={cwd} onPick={resume} />
          </>
        )}
      </div>
    </div>
  );
}
