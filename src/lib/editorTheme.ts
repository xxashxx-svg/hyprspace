import { EditorView } from "@codemirror/view";
import { HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

// VS Code "Dark+" for the dock editor — chrome + syntax colors, so it reads like VS Code.
export const vscodeChrome = EditorView.theme(
  {
    "&": { height: "100%", color: "#d4d4d4", backgroundColor: "#1e1e1e" },
    ".cm-content": {
      fontFamily: "var(--font-mono)",
      fontSize: "13px",
      caretColor: "#aeafad",
      padding: "6px 0",
    },
    ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.5" },
    ".cm-gutters": { backgroundColor: "#1e1e1e", color: "#858585", border: "none" },
    ".cm-lineNumbers .cm-gutterElement": { padding: "0 10px 0 16px", minWidth: "2ch" },
    ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.04)" },
    ".cm-activeLineGutter": { backgroundColor: "transparent", color: "#c6c6c6" },
    "&.cm-focused": { outline: "none" },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#aeafad", borderLeftWidth: "2px" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
      backgroundColor: "#264f78",
    },
    ".cm-selectionMatch": { backgroundColor: "rgba(88,124,166,0.32)" },
    ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
      backgroundColor: "rgba(88,124,166,0.35)",
      outline: "1px solid #517191",
    },
    ".cm-foldPlaceholder": { backgroundColor: "transparent", border: "none", color: "#858585" },
    ".cm-tooltip": { backgroundColor: "#252526", border: "1px solid #454545", color: "#d4d4d4" },
    ".cm-tooltip-autocomplete ul li[aria-selected]": { backgroundColor: "#04395e", color: "#fff" },
  },
  { dark: true },
);

export const vscodeHighlight = HighlightStyle.define([
  { tag: t.comment, color: "#6a9955", fontStyle: "italic" },
  { tag: [t.keyword, t.moduleKeyword, t.operatorKeyword, t.definitionKeyword], color: "#569cd6" },
  { tag: t.controlKeyword, color: "#c586c0" },
  { tag: [t.string, t.special(t.string), t.docString, t.character], color: "#ce9178" },
  { tag: t.escape, color: "#d7ba7d" },
  { tag: [t.number, t.integer, t.float, t.unit], color: "#b5cea8" },
  { tag: [t.bool, t.null, t.atom], color: "#569cd6" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.labelName], color: "#dcdcaa" },
  { tag: [t.typeName, t.className, t.namespace], color: "#4ec9b0" },
  { tag: [t.variableName, t.propertyName, t.attributeName], color: "#9cdcfe" },
  { tag: [t.self, t.constant(t.variableName), t.standard(t.variableName)], color: "#4fc1ff" },
  { tag: [t.tagName, t.angleBracket], color: "#569cd6" },
  { tag: t.operator, color: "#d4d4d4" },
  { tag: t.regexp, color: "#d16969" },
  { tag: [t.meta, t.annotation, t.processingInstruction], color: "#dcdcaa" },
  { tag: [t.punctuation, t.separator, t.bracket, t.brace, t.paren, t.squareBracket], color: "#d4d4d4" },
  { tag: [t.heading], color: "#569cd6", fontWeight: "bold" },
  { tag: [t.link, t.url], color: "#9cdcfe", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: [t.invalid, t.deleted], color: "#f44747" },
  { tag: t.inserted, color: "#b5cea8" },
]);
