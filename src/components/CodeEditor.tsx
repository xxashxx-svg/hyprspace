import { useEffect, useRef, useState } from "react";
import { EditorView, keymap } from "@codemirror/view";
import { EditorState, Prec, type Extension } from "@codemirror/state";
import { syntaxHighlighting } from "@codemirror/language";
import { basicSetup } from "codemirror";
import { oneDarkHighlightStyle } from "@codemirror/theme-one-dark";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { markdown } from "@codemirror/lang-markdown";
import { python } from "@codemirror/lang-python";
import { rust } from "@codemirror/lang-rust";
import { readFile, writeFile } from "../api";
import { Save, AlertCircle, FileCode } from "lucide-react";

// language support by file extension (the few that cover most of what people edit here)
function langFor(path: string): Extension[] {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (["js", "jsx", "ts", "tsx", "mjs", "cjs"].includes(ext))
    return [javascript({ jsx: ext.endsWith("x"), typescript: ext.startsWith("ts") })];
  if (ext === "json") return [json()];
  if (["css", "scss", "less"].includes(ext)) return [css()];
  if (["html", "htm", "xml", "svg", "vue"].includes(ext)) return [html()];
  if (["md", "markdown", "mdx"].includes(ext)) return [markdown()];
  if (ext === "py") return [python()];
  if (ext === "rs") return [rust()];
  return [];
}

// editor chrome themed to our tokens; syntax colors come from one-dark
const chrome = EditorView.theme(
  {
    "&": { height: "100%", color: "var(--text-1)", backgroundColor: "var(--bg-terminal)" },
    ".cm-content": { fontFamily: "var(--font-mono)", fontSize: "12.5px", caretColor: "var(--text-1)" },
    ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.55" },
    ".cm-gutters": { backgroundColor: "var(--bg-terminal)", color: "var(--text-3)", border: "none" },
    ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.03)" },
    ".cm-activeLineGutter": { backgroundColor: "rgba(255,255,255,0.04)", color: "var(--text-2)" },
    "&.cm-focused": { outline: "none" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text-1)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "rgba(255,255,255,0.12)",
    },
    ".cm-selectionMatch": { backgroundColor: "rgba(255,255,255,0.08)" },
  },
  { dark: true },
);

export function CodeEditor({ path }: { path: string }) {
  const elRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [autosave, setAutosave] = useState(false);

  // keep the latest autosave flag + save fn reachable from the long-lived editor extensions
  const autosaveRef = useRef(autosave);
  autosaveRef.current = autosave;
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  const name = path.split(/[\\/]/).pop() ?? path;

  // a ref so the editor's long-lived keymap/listener always call the latest path-bound save
  const save = useRef(async () => {});
  save.current = async () => {
    const view = viewRef.current;
    if (!view) return;
    setSaving(true);
    try {
      await writeFile(path, view.state.doc.toString());
      setDirty(false);
    } catch (e) {
      setErr(String(e));
    }
    setSaving(false);
  };

  useEffect(() => {
    let disposed = false;
    setErr(null);
    setDirty(false);
    readFile(path)
      .then((content) => {
        if (disposed || !elRef.current) return;
        const state = EditorState.create({
          doc: content,
          extensions: [
            basicSetup,
            ...langFor(path),
            syntaxHighlighting(oneDarkHighlightStyle),
            chrome,
            Prec.highest(
              keymap.of([
                { key: "Mod-s", preventDefault: true, run: () => (void save.current(), true) },
              ]),
            ),
            EditorView.updateListener.of((u) => {
              if (!u.docChanged) return;
              setDirty(true);
              if (autosaveRef.current) {
                clearTimeout(saveTimer.current);
                saveTimer.current = setTimeout(() => void save.current(), 800);
              }
            }),
          ],
        });
        viewRef.current = new EditorView({ state, parent: elRef.current });
      })
      .catch((e) => {
        if (!disposed) setErr(String(e));
      });
    return () => {
      disposed = true;
      clearTimeout(saveTimer.current);
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
