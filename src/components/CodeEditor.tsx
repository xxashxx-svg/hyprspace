import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { Compartment, EditorState, Prec, type Extension } from "@codemirror/state";
import { syntaxHighlighting } from "@codemirror/language";
import { basicSetup } from "codemirror";
import { vscodeChrome, vscodeHighlight } from "../lib/editorTheme";
import { readFile, writeFile } from "../api";
import { useUi } from "../stores/ui";
import { confirmDialog, useConfirm } from "../stores/confirm";
import { Save, AlertCircle, FileCode, Maximize2, Minimize2, X } from "lucide-react";

// language support by file extension (the few that cover most of what people edit here) —
// each pack is imported on demand so its grammar only loads when a matching file is opened
async function langFor(path: string): Promise<Extension[]> {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  try {
    if (["js", "jsx", "ts", "tsx", "mjs", "cjs"].includes(ext)) {
      const { javascript } = await import("@codemirror/lang-javascript");
      return [javascript({ jsx: ext.endsWith("x"), typescript: ext.startsWith("ts") })];
    }
    if (ext === "json") return [(await import("@codemirror/lang-json")).json()];
    if (["css", "scss", "less"].includes(ext)) return [(await import("@codemirror/lang-css")).css()];
    if (["html", "htm", "xml", "svg", "vue"].includes(ext))
      return [(await import("@codemirror/lang-html")).html()];
    if (["md", "markdown", "mdx"].includes(ext))
      return [(await import("@codemirror/lang-markdown")).markdown()];
    if (ext === "py") return [(await import("@codemirror/lang-python")).python()];
    if (ext === "rs") return [(await import("@codemirror/lang-rust")).rust()];
  } catch {
    // a failed chunk load just means no highlighting
  }
  return [];
}

export function CodeEditor({ path }: { path: string }) {
  const elRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [autosave, setAutosave] = useState(false);
  const editorMax = useUi((s) => s.editorMax);
  const toggleEditorMax = useUi((s) => s.toggleEditorMax);

  // keep the latest autosave flag + save fn reachable from the long-lived editor extensions
  const autosaveRef = useRef(autosave);
  autosaveRef.current = autosave;
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // dirty as a ref too, so the unmount cleanup can see it; discardRef marks an explicit
  // "Discard" choice so the unmount flush doesn't resurrect what the user just threw away
  const dirtyRef = useRef(false);
  const discardRef = useRef(false);

  const name = path.split(/[\\/]/).pop() ?? path;

  const close = async () => {
    // a pending autosave mustn't fire mid-dialog — that would save what the user then "discards"
    clearTimeout(saveTimer.current);
    if (dirty) {
      if (autosave) {
        // autosave is on: the user already opted into persistence — flush instead of asking
        await save.current();
      } else {
        const ok = await confirmDialog({
          title: "Discard changes?",
          message: `${name} has unsaved changes.`,
          confirmLabel: "Discard",
          danger: true,
        });
        if (!ok) return;
        discardRef.current = true;
      }
    }
    useUi.getState().closeFile();
  };

  // Esc drops out of full screen (but never closes the file, and never steals a dialog's Esc)
  useEffect(() => {
    if (!editorMax) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (useConfirm.getState().req) return; // a confirm dialog owns this Esc
      toggleEditorMax();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [editorMax, toggleEditorMax]);

  // a ref so the editor's long-lived keymap/listener always call the latest path-bound save
  const save = useRef(async () => {});
  save.current = async () => {
    const view = viewRef.current;
    if (!view) return;
    setSaving(true);
    try {
      await writeFile(path, view.state.doc.toString());
      setDirty(false);
      dirtyRef.current = false;
    } catch (e) {
      setErr(String(e));
    }
    setSaving(false);
  };

  useEffect(() => {
    let disposed = false;
    setErr(null);
    setDirty(false);
    // the editor opens right away without a language; the pack slots in via this compartment
    // a tick later once its dynamic import resolves
    const langComp = new Compartment();
    readFile(path)
      .then((content) => {
        if (disposed || !elRef.current) return;
        const state = EditorState.create({
          doc: content,
          extensions: [
            basicSetup,
            langComp.of([]),
            syntaxHighlighting(vscodeHighlight),
            vscodeChrome,
            Prec.highest(
              keymap.of([
                { key: "Mod-s", preventDefault: true, run: () => (void save.current(), true) },
              ]),
            ),
            EditorView.updateListener.of((u) => {
              if (!u.docChanged) return;
              setDirty(true);
              dirtyRef.current = true;
              if (autosaveRef.current) {
                clearTimeout(saveTimer.current);
                saveTimer.current = setTimeout(() => void save.current(), 800);
              }
            }),
          ],
        });
        viewRef.current = new EditorView({ state, parent: elRef.current });
        void langFor(path).then((lang) => {
          if (disposed || !lang.length || !viewRef.current) return;
          viewRef.current.dispatch({ effects: langComp.reconfigure(lang) });
        });
      })
      .catch((e) => {
        if (!disposed) setErr(String(e));
      });
    return () => {
      disposed = true;
      clearTimeout(saveTimer.current);
      // the editor can unmount from MANY paths (another file opened, dock tab switched, dock
      // hidden, Home) — never silently drop unsaved work: flush it, unless the user explicitly
      // chose Discard in the close dialog.
      if (dirtyRef.current && !discardRef.current && viewRef.current) {
        const doc = viewRef.current.state.doc.toString();
        void writeFile(path, doc).catch(() => {});
      }
      dirtyRef.current = false;
      discardRef.current = false;
      viewRef.current?.destroy();
      viewRef.current = null;
    };
  }, [path]);

  return (
    <div className="editor">
      <div className="editor-head">
        <FileCode size={13} className="editor-head-ico" />
        <span className="editor-name" title={path}>
          {name}
          {dirty && <span className="editor-dirty" title="unsaved changes" />}
        </span>
        <label className="editor-autosave">
          <input type="checkbox" checked={autosave} onChange={(e) => setAutosave(e.target.checked)} />
          Autosave
        </label>
        <button className="editor-save" onClick={() => void save.current()} disabled={!dirty || saving}>
          <Save size={12} /> {saving ? "Saving…" : "Save"}
        </button>
        <button
          className="editor-icon-btn"
          onClick={toggleEditorMax}
          title={editorMax ? "Exit full screen (Esc)" : "Full screen"}
        >
          {editorMax ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
        </button>
        <button className="editor-icon-btn" onClick={() => void close()} title="Close file">
          <X size={14} />
        </button>
      </div>
      {err ? (
        <div className="editor-err">
          <AlertCircle size={14} /> {err}
        </div>
      ) : (
        <div className="editor-cm" ref={elRef} />
      )}
    </div>
  );
}
