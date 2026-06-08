import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent as RKeyboardEvent } from "react";
import type { SearchAddon } from "@xterm/addon-search";

interface Props {
  search: SearchAddon;
  onClose: () => void;
}

export function TerminalSearch({ search, onClose }: Props) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (query) search.findNext(query, { caseSensitive });
    else search.clearDecorations();
  }, [query, caseSensitive, search]);

  const close = () => {
    search.clearDecorations();
    onClose();
  };

  const onKey = (e: RKeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey) search.findPrevious(query, { caseSensitive });
      else search.findNext(query, { caseSensitive });
    }
  };

  return (
    <div className="term-search">
      <input
        ref={inputRef}
        className="term-search-input"
        value={query}
        placeholder="Search…"
        spellCheck={false}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKey}
      />
      <button
        className={`term-search-btn ${caseSensitive ? "active" : ""}`}
        title="Case sensitive"
        onClick={() => setCaseSensitive((p) => !p)}
      >
        Aa
      </button>
      <button
        className="term-search-btn"
        title="Previous (Shift+Enter)"
        onClick={() => search.findPrevious(query, { caseSensitive })}
      >
        ↑
      </button>
      <button
        className="term-search-btn"
        title="Next (Enter)"
        onClick={() => search.findNext(query, { caseSensitive })}
      >
        ↓
      </button>
      <button className="term-search-btn close" title="Close (Esc)" onClick={close}>
        ✕
      </button>
    </div>
  );
}
