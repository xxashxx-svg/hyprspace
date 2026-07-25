// Which open editor files have unsaved edits. The dirty flag lives inside CodeEditor, but closing
// happens from the outside (a tab's ×, "Close tab", closing a space), so the close path needs to be
// able to ask. Module-level rather than a store: it's read imperatively, never rendered.
const dirty = new Set<string>();

export function markFileDirty(path: string, isDirty: boolean) {
  if (isDirty) dirty.add(path);
  else dirty.delete(path);
}

export function isFileDirty(path: string): boolean {
  return dirty.has(path);
}

// "Discard" chosen in the close dialog. The editor unmounts a tick later and would otherwise flush
// the buffer to disk — this lets it know the write was explicitly refused. One-shot.
const discarded = new Set<string>();

export function markDiscarded(path: string) {
  discarded.add(path);
}

export function takeDiscarded(path: string): boolean {
  const was = discarded.has(path);
  discarded.delete(path);
  return was;
}
