import { EditorView } from "@codemirror/view";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";
import type { Mode } from "../themes";

// The dock editor wears the app theme: chrome comes from the design tokens, so it follows every
// theme and both sides. Syntax colors are VS Code's Dark+ and Light+, picked by side.
function chrome(dark: boolean) {
  return EditorView.theme(
    {
      "&": { height: "100%", color: "var(--text-1)", backgroundColor: "var(--bg-base)" },
      ".cm-content": {
        fontFamily: "var(--font-mono)",
        fontSize: "13px",
        caretColor: "var(--text-1)",
        padding: "6px 0",
      },
      ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.5" },
      ".cm-gutters": { backgroundColor: "var(--bg-base)", color: "var(--text-3)", border: "none" },
      ".cm-lineNumbers .cm-gutterElement": { padding: "0 10px 0 16px", minWidth: "2ch" },
      ".cm-activeLine": { backgroundColor: "rgba(var(--ink), 0.04)" },
      ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--text-2)" },
      "&.cm-focused": { outline: "none" },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--text-1)", borderLeftWidth: "2px" },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
        backgroundColor: "color-mix(in srgb, var(--accent) 28%, transparent)",
      },
      ".cm-selectionMatch": { backgroundColor: "color-mix(in srgb, var(--accent) 18%, transparent)" },
      ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
        backgroundColor: "color-mix(in srgb, var(--accent) 22%, transparent)",
        outline: "1px solid color-mix(in srgb, var(--accent) 50%, transparent)",
      },
      ".cm-foldPlaceholder": { backgroundColor: "transparent", border: "none", color: "var(--text-3)" },
      ".cm-tooltip": { backgroundColor: "var(--surface-3)", border: "1px solid var(--border-2)", color: "var(--text-1)" },
      ".cm-tooltip-autocomplete ul li[aria-selected]": { backgroundColor: "var(--accent)", color: "var(--on-accent)" },
    },
    { dark },
  );
}

const darkHighlight = HighlightStyle.define([
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

const lightHighlight = HighlightStyle.define([
  { tag: t.comment, color: "#008000", fontStyle: "italic" },
  { tag: [t.keyword, t.moduleKeyword, t.operatorKeyword, t.definitionKeyword], color: "#0000ff" },
  { tag: t.controlKeyword, color: "#af00db" },
  { tag: [t.string, t.special(t.string), t.docString, t.character], color: "#a31515" },
  { tag: t.escape, color: "#ee0000" },
  { tag: [t.number, t.integer, t.float, t.unit], color: "#098658" },
  { tag: [t.bool, t.null, t.atom], color: "#0000ff" },
  { tag: [t.function(t.variableName), t.function(t.propertyName), t.labelName], color: "#795e26" },
  { tag: [t.typeName, t.className, t.namespace], color: "#267f99" },
  { tag: [t.variableName, t.propertyName, t.attributeName], color: "#001080" },
  { tag: [t.self, t.constant(t.variableName), t.standard(t.variableName)], color: "#0070c1" },
  { tag: [t.tagName, t.angleBracket], color: "#800000" },
  { tag: t.operator, color: "#000000" },
  { tag: t.regexp, color: "#811f3f" },
  { tag: [t.meta, t.annotation, t.processingInstruction], color: "#795e26" },
  { tag: [t.punctuation, t.separator, t.bracket, t.brace, t.paren, t.squareBracket], color: "#000000" },
  { tag: [t.heading], color: "#0000ff", fontWeight: "bold" },
  { tag: [t.link, t.url], color: "#0000ee", textDecoration: "underline" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strong, fontWeight: "bold" },
  { tag: [t.invalid, t.deleted], color: "#cd3131" },
  { tag: t.inserted, color: "#098658" },
]);

export function editorTheme(mode: Mode): Extension[] {
  const dark = mode === "dark";
  return [chrome(dark), syntaxHighlighting(dark ? darkHighlight : lightHighlight)];
}
