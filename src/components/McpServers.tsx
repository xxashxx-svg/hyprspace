import { useEffect, useState } from "react";
import { mcpList, mcpSet, mcpRemove, type McpEntry } from "../api";
import { confirmDialog } from "../stores/confirm";
import { useNotifications } from "../stores/notifications";
import { Plus, Plug, Globe, Pencil, Trash2, ArrowLeft } from "lucide-react";

type Kind = "stdio" | "sse" | "http";

interface Draft {
  prevName: string | null; // null = creating a new one
  name: string;
  kind: Kind;
  command: string;
  args: string; // one per line
  env: string; // KEY=VALUE per line
  url: string;
  headers: string; // KEY=VALUE per line
}

const newDraft = (): Draft => ({
  prevName: null,
  name: "",
  kind: "stdio",
  command: "",
  args: "",
  env: "",
  url: "",
  headers: "",
});

const lines = (s: string) =>
  s
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

const toKv = (s: string): Record<string, string> => {
  const o: Record<string, string> = {};
  for (const l of lines(s)) {
    const i = l.indexOf("=");
    if (i > 0) o[l.slice(0, i).trim()] = l.slice(i + 1).trim();
  }
  return o;
};
const fromKv = (o: unknown) =>
  o && typeof o === "object"
    ? Object.entries(o as Record<string, unknown>)
        .map(([k, v]) => `${k}=${v}`)
        .join("\n")
    : "";

function kindOf(config: Record<string, unknown>): Kind {
  return config.type === "sse" ? "sse" : config.type === "http" ? "http" : "stdio";
}

function toDraft(e: McpEntry): Draft {
  const c = e.config || {};
  return {
    prevName: e.name,
    name: e.name,
    kind: kindOf(c),
    command: typeof c.command === "string" ? c.command : "",
    args: Array.isArray(c.args) ? (c.args as unknown[]).join("\n") : "",
    env: fromKv(c.env),
    url: typeof c.url === "string" ? c.url : "",
    headers: fromKv(c.headers),
  };
}

function buildConfig(d: Draft): Record<string, unknown> {
  if (d.kind === "stdio") {
    const cfg: Record<string, unknown> = { command: d.command.trim() };
    const a = lines(d.args);
    if (a.length) cfg.args = a;
    const e = toKv(d.env);
    if (Object.keys(e).length) cfg.env = e;
    return cfg;
  }
  const cfg: Record<string, unknown> = { type: d.kind, url: d.url.trim() };
  const h = toKv(d.headers);
  if (Object.keys(h).length) cfg.headers = h;
  return cfg;
}

// one-line summary of a server for the list
function summary(c: Record<string, unknown>): string {
  const k = kindOf(c);
  if (k === "stdio") {
    const args = Array.isArray(c.args) ? " " + (c.args as unknown[]).join(" ") : "";
    return `${c.command ?? ""}${args}`.trim() || "—";
  }
  return typeof c.url === "string" ? c.url : "—";
}

export function McpServers() {
  const [list, setList] = useState<McpEntry[] | null>(null);
  const [editing, setEditing] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = () =>
    mcpList()
      .then(setList)
      .catch(() => setList([]));
  useEffect(() => {
    void reload();
  }, []);

  const save = async () => {
    if (!editing) return;
    const d = editing;
    if (!d.name.trim()) return;
    if (d.kind === "stdio" ? !d.command.trim() : !d.url.trim()) return;
    setBusy(true);
    try {
      await mcpSet(d.name.trim(), buildConfig(d), d.prevName);
      await reload();
      setEditing(null);
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't save MCP server", body: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const remove = async (name: string) => {
    const ok = await confirmDialog({
      title: "Remove MCP server",
      message: `Remove "${name}"? Your Claude agents will stop loading it on next launch.`,
      confirmLabel: "Remove",
      cancelLabel: "Cancel",
      danger: true,
    });
    if (!ok) return;
    try {
      await mcpRemove(name);
      await reload();
    } catch (e) {
      useNotifications.getState().add({ title: "Couldn't remove MCP server", body: String(e) });
    }
  };

  // ---- editor view ----
  if (editing) {
    const d = editing;
    const set = (patch: Partial<Draft>) => setEditing({ ...d, ...patch });
    const remote = d.kind !== "stdio";
    return (
      <div className="mcp">
        <div className="mcp-edit-head">
          <button className="mcp-back" onClick={() => setEditing(null)}>
            <ArrowLeft size={15} />
          </button>
          <span className="mcp-edit-title">{d.prevName ? "Edit MCP server" : "Add MCP server"}</span>
        </div>

        <label className="mcp-field">
          <span className="mcp-flabel">Name</span>
          <input
            className="np-input"
            autoFocus
            placeholder="filesystem"
            value={d.name}
            onChange={(e) => set({ name: e.target.value })}
          />
        </label>

        <div className="mcp-field">
          <span className="mcp-flabel">Type</span>
          <div className="np-seg">
            <button className={d.kind === "stdio" ? "active" : ""} onClick={() => set({ kind: "stdio" })}>
              Local (stdio)
            </button>
            <button className={d.kind === "sse" ? "active" : ""} onClick={() => set({ kind: "sse" })}>
              SSE
            </button>
            <button className={d.kind === "http" ? "active" : ""} onClick={() => set({ kind: "http" })}>
              HTTP
            </button>
          </div>
        </div>

        {!remote ? (
          <>
            <label className="mcp-field">
              <span className="mcp-flabel">Command</span>
              <input
                className="np-input"
                placeholder="npx"
                value={d.command}
                onChange={(e) => set({ command: e.target.value })}
              />
            </label>
            <label className="mcp-field">
              <span className="mcp-flabel">Arguments <span className="mcp-hint">one per line</span></span>
              <textarea
                className="mcp-area"
                rows={3}
                placeholder={"-y\n@modelcontextprotocol/server-filesystem\nC:\\path"}
                value={d.args}
                onChange={(e) => set({ args: e.target.value })}
              />
            </label>
            <label className="mcp-field">
              <span className="mcp-flabel">Environment <span className="mcp-hint">KEY=value per line</span></span>
              <textarea
                className="mcp-area"
                rows={2}
                placeholder={"API_KEY=sk-..."}
                value={d.env}
                onChange={(e) => set({ env: e.target.value })}
              />
            </label>
          </>
        ) : (
          <>
            <label className="mcp-field">
              <span className="mcp-flabel">URL</span>
              <input
                className="np-input"
                placeholder="https://example.com/mcp"
                value={d.url}
                onChange={(e) => set({ url: e.target.value })}
              />
            </label>
            <label className="mcp-field">
              <span className="mcp-flabel">Headers <span className="mcp-hint">KEY=value per line</span></span>
              <textarea
                className="mcp-area"
                rows={2}
                placeholder={"Authorization=Bearer ..."}
                value={d.headers}
                onChange={(e) => set({ headers: e.target.value })}
              />
            </label>
          </>
        )}

        <div className="mcp-edit-foot">
          <button className="btn" onClick={() => setEditing(null)} disabled={busy}>
            Cancel
          </button>
          <button
            className="btn primary"
            onClick={() => void save()}
            disabled={busy || !d.name.trim() || (d.kind === "stdio" ? !d.command.trim() : !d.url.trim())}
          >
            {busy ? "Saving…" : "Save server"}
          </button>
        </div>
      </div>
    );
  }

  // ---- list view ----
  return (
    <div className="mcp">
      <div className="mcp-bar">
        <span className="set-hint">Servers your Claude agents load on launch (written to ~/.claude.json).</span>
        <button className="btn primary mcp-add" onClick={() => setEditing(newDraft())}>
          <Plus size={14} /> Add server
        </button>
      </div>

      {list === null ? (
        <div className="mcp-empty">Loading…</div>
      ) : list.length === 0 ? (
        <div className="mcp-empty">No MCP servers yet — add one to give your agents extra tools.</div>
      ) : (
        <div className="mcp-list">
          {list.map((e) => {
            const k = kindOf(e.config);
            return (
              <div className="mcp-row" key={e.name}>
                <span className="mcp-ico">{k === "stdio" ? <Plug size={15} /> : <Globe size={15} />}</span>
                <span className="mcp-info">
                  <span className="mcp-name">
                    {e.name}
                    <span className="mcp-kind">{k}</span>
                  </span>
                  <span className="mcp-sum">{summary(e.config)}</span>
                </span>
                <button className="mcp-btn" title="Edit" onClick={() => setEditing(toDraft(e))}>
                  <Pencil size={14} />
                </button>
                <button className="mcp-btn danger" title="Remove" onClick={() => void remove(e.name)}>
                  <Trash2 size={14} />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
