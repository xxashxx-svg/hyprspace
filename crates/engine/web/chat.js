import { add, elapsed, h } from "./dom.js";
import { icon, mark } from "./icons.js";
import { markdown } from "./markdown.js";
import { Transcript } from "./transcript.js";

const PERMISSIONS = [
  ["plan", "Plan only", "list-checks"],
  ["ask", "Ask first", "hand"],
  ["auto", "Auto edit", "file-pen-line"],
  ["bypass", "Full access", "lock-open"],
];

const TASKS = {
  command: ["terminal", "Background command"],
  monitor: ["gauge", "Monitor"],
  subagent: ["bot", "Background subagent"],
  other: ["zap", "Background task"],
};

export class Chat {
  constructor(app, id) {
    this.app = app;
    this.id = id;
    this.t = new Transcript();
    this.rows = new Map();
    this.open = new Set();
    this.follow = true;
    this.list = h("div", { class: "items" });
    this.scroller = h("div", { class: "scroll", onscroll: () => this.scrolled() }, this.list);
    this.input = h("textarea", {
      rows: 1,
      placeholder: "Ask for anything",
      oninput: () => this.grow(),
      onkeydown: (e) => {
        if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
          e.preventDefault();
          this.submit();
        }
      },
    });
    this.sendButton = h("button", { class: "send", onclick: () => this.submit(), title: "Send" });
    this.permission = h("select", { class: "pick", onchange: () => this.app.ask({ type: "permission", thread: this.id, permission: this.permission.value }) }, PERMISSIONS.map(([v, l]) => h("option", { value: v }, l)));
    this.lock = h("span", { class: "lock" });
    this.agentMark = h("span", { class: "chip-mark" });
    this.model = h("span", { class: "chip-model" });
    this.effort = h("span", { class: "chip chip-quiet" });
    this.workRow = h("div", { class: "working" });
    this.tray = h("div", { class: "tray" });
    this.aside = h("div", { class: "aside", hidden: true });
    this.folder = h("span", { class: "foot-item" });
    this.place = h("span", { class: "foot-item" });
    this.context = h("span", { class: "foot-item" });
    const pill = h(
      "div",
      { class: "pill" },
      this.input,
      h(
        "div",
        { class: "pill-row" },
        h("span", { class: "chip" }, this.agentMark, this.model),
        this.effort,
        h("label", { class: "chip chip-quiet" }, this.lock, this.permission),
        h("span", { class: "grow" }),
        this.sendButton,
      ),
    );
    const foot = h("div", { class: "composer-foot" }, this.folder, h("span", { class: "grow" }), this.place, this.context);
    this.el = h("div", { class: "chat" }, this.scroller, h("div", { class: "composer" }, this.aside, this.tray, this.workRow, pill, foot));
  }

  entries(entries, reset) {
    if (reset) {
      this.t.reset();
      this.rows.clear();
      this.list.replaceChildren();
    }
    for (const e of entries) this.t.add(e, reset);
    this.draw();
  }

  thread(t) {
    this.info = t;
    this.permission.value = t.permission;
    const perm = PERMISSIONS.find((p) => p[0] === t.permission) || PERMISSIONS[1];
    this.lock.replaceChildren(icon(perm[2], 12));
    this.agentMark.replaceChildren(mark(t.agent, 13));
    this.model.textContent = t.model || "Default";
    this.effort.textContent = t.effort ? t.effort[0].toUpperCase() + t.effort.slice(1) : "";
    this.effort.hidden = !t.effort;
    const space = this.app.board.spaces.find((s) => s.id === t.space);
    this.folder.replaceChildren(icon("folder", 12), h("span", null, space ? space.name : ""));
    this.place.replaceChildren(icon(t.branch ? "git-branch" : "folder", 12), h("span", null, t.place));
    this.place.hidden = !t.branch;
    this.draw();
  }

  working() {
    return this.t.running || (this.info && (this.info.status === "working" || this.info.status === "waiting"));
  }

  draw() {
    const keep = new Set();
    let prev = null;
    const folds = this.t.folds();
    const hidden = new Set(folds.flatMap((f) => (this.open.has(`fold${f.prompt}`) ? [] : f.hide)));
    const heads = new Map(folds.map((f) => [f.first, { kind: "fold", key: `fold${f.prompt}`, v: f.ms }]));
    const shown = this.t.items.flatMap((it) => [...(heads.has(it.key) ? [heads.get(it.key)] : []), ...(hidden.has(it.key) ? [] : [it])]);
    for (const it of shown) {
      if (it.kind === "finished" && it.status === "done") continue;
      keep.add(it.key);
      let row = this.rows.get(it.key);
      if (!row || row.v !== it.v || row.open !== this.open.has(it.key)) {
        const el = this.item(it);
        if (row) row.el.replaceWith(el);
        row = { el, v: it.v, open: this.open.has(it.key) };
        this.rows.set(it.key, row);
      }
      const want = prev ? prev.nextSibling : this.list.firstChild;
      if (want !== row.el) this.list.insertBefore(row.el, want);
      prev = row.el;
    }
    for (const [k, row] of this.rows) {
      if (!keep.has(k)) {
        row.el.remove();
        this.rows.delete(k);
      }
    }
    const busy = this.working();
    const stop = busy && !this.input.value.trim();
    this.sendButton.classList.toggle("stop", stop);
    this.sendButton.title = stop ? "Stop" : "Send";
    this.sendButton.replaceChildren(stop ? h("span", { class: "stop-square" }) : icon("arrow-up", 15));
    this.input.placeholder = this.t.denies() ? "Send a message to deny and redirect the agent" : busy ? "Queue a message for when it finishes" : "Ask for anything";
    const since = this.info && this.info.since;
    this.workRow.hidden = !busy;
    if (busy) this.workRow.replaceChildren(h("span", { class: "dots" }, h("i"), h("i")), h("span", null, this.info && this.info.status === "waiting" ? "Waiting on you" : "Working"), h("span", { class: "grow" }), since ? h("span", { class: "mono", "data-since": since }, elapsed(since)) : null);
    this.drawTray();
    const ctx = this.t.context;
    this.context.hidden = !ctx;
    if (ctx) this.context.textContent = `${Math.round((ctx[0] / ctx[1]) * 100)}% context`;
    if (this.follow) this.scroller.scrollTop = this.scroller.scrollHeight;
  }

  drawTray() {
    const queue = (this.info && this.info.queue) || [];
    const tasks = (this.info && this.info.tasks) || [];
    const key = JSON.stringify([queue, tasks]);
    if (key === this.trayKey) return;
    this.trayKey = key;
    this.tray.hidden = !queue.length && !tasks.length;
    const ask = (type, fields) => this.app.ask({ type, thread: this.id, ...fields });
    this.tray.replaceChildren();
    add(this.tray, [
      tasks.map((t) => {
        const [name, kind] = TASKS[t.kind] || TASKS.other;
        return h("div", { class: "tray-row" }, icon(name, 13), h("div", { class: "tray-text" }, h("div", { class: "tray-label" }, t.label || kind), h("div", { class: "tray-kind" }, kind)), h("span", { class: "dots" }, h("i"), h("i")), h("button", { class: "tray-pill", onclick: () => ask("stopTask", { task: t.id }) }, "Stop"));
      }),
      queue.length ? h("div", { class: "tray-head" }, queue.length === 1 ? "Queued for when it finishes" : `${queue.length} queued for when it finishes`) : null,
      queue.map((q, index) => {
        const images = q.images ? q.images.length : 0;
        const edit = () => {
          if (!ask("unqueue", { index, text: q.text })) return;
          this.input.value = this.input.value.trim() ? `${q.text}\n${this.input.value}` : q.text;
          this.grow();
          this.input.focus();
        };
        return h(
          "div",
          { class: "tray-row queued" },
          h("div", { class: "tray-text" }, h("div", { class: "tray-queued" }, q.text), images ? h("div", { class: "tray-kind" }, `${images} image${images === 1 ? "" : "s"}`) : null),
          h("button", { class: "tray-round", title: "Edit", onclick: edit }, icon("pencil", 13)),
          h("button", { class: "tray-round", title: "Remove", onclick: () => ask("unqueue", { index, text: q.text }) }, icon("x", 13)),
          h("button", { class: "tray-pill", onclick: () => ask("steer", { index, text: q.text }) }, "Steer"),
        );
      }),
    ]);
  }

  side(a) {
    this.aside.hidden = !a;
    if (!a) return this.aside.replaceChildren();
    const body = a.error ? h("div", { class: "dim" }, a.error) : a.text ? markdown(a.text) : h("div", { class: "dim" }, "Answering");
    this.aside.replaceChildren(
      h(
        "div",
        { class: "aside-head" },
        icon("message-square", 13),
        h("div", { class: "aside-question" }, a.question),
        a.done ? null : h("span", { class: "dots" }, h("i"), h("i")),
        h("button", { class: "tray-round", title: "Close", onclick: () => this.app.ask({ type: "dropAside", thread: this.id }) }, icon("x", 13)),
      ),
      h("div", { class: "aside-body" }, body),
    );
  }

  scrolled() {
    const s = this.scroller;
    this.follow = s.scrollHeight - s.scrollTop - s.clientHeight < 40;
  }

  grow() {
    this.input.style.height = "auto";
    this.input.style.height = `${Math.min(this.input.scrollHeight, 240)}px`;
    this.draw();
  }

  submit() {
    const text = this.input.value.trim();
    if (!text) {
      if (this.working()) this.app.ask({ type: "interrupt", thread: this.id });
      return;
    }
    if (this.app.ask({ type: "send", thread: this.id, text, images: [] })) {
      this.input.value = "";
      this.follow = true;
      this.grow();
    }
  }

  toggle(key) {
    if (this.open.has(key)) this.open.delete(key);
    else this.open.add(key);
    this.draw();
  }

  item(it) {
    const open = this.open.has(it.key);
    switch (it.kind) {
      case "user":
        return h("div", { class: "user" }, h("div", { class: "bubble" }, it.text, it.images ? h("div", { class: "dim" }, `${it.images} image${it.images === 1 ? "" : "s"}`) : null));
      case "reply":
        return h("div", { class: "reply" }, markdown(it.text));
      case "thinking":
        return fold(open, "Thinking", null, () => this.toggle(it.key), h("div", { class: "quiet" }, markdown(it.text)));
      case "call":
        return call(it, open, () => this.toggle(it.key));
      case "agent":
        return fold(
          open,
          it.description || "Subagent",
          state(it.state),
          () => this.toggle(it.key),
          h("div", { class: "quiet" }, it.prompt ? h("pre", { class: "out" }, it.prompt) : null, it.calls.map((c) => h("div", { class: "subcall" }, label(c.tool), c.done && !c.done.ok ? h("span", { class: "bad" }, " failed") : null)), it.report || it.said ? markdown(it.report || it.said) : null),
        );
      case "approval":
        return this.approval(it);
      case "error":
        return h("div", { class: "error" }, it.message);
      case "note":
        return h("div", { class: "dim small" }, it.text);
      case "fold":
        return fold(open, it.v == null ? "Continued" : `Worked for ${elapsed(Date.now() - it.v)}`, null, () => this.toggle(it.key), null);
      case "finished":
        return h("div", { class: it.status === "failed" ? "error small" : "dim small" }, it.status === "failed" ? (it.error ? `Failed: ${it.error}` : `Failed after ${(it.ms / 1000).toFixed(1)}s`) : `Stopped after ${(it.ms / 1000).toFixed(1)}s`);
    }
    return h("div");
  }

  approval(it) {
    const answer = (a, answers) => this.app.ask({ type: "approve", thread: this.id, request: it.request, answer: a, answers: answers || [] });
    const questions = asked(it.tool);
    const live = !it.answer && !it.expired;
    const head = h("div", { class: "approval-head" }, questions ? "Claude has a question" : label(it.tool), it.answer ? h("span", { class: "dim" }, it.answer === "deny" ? " · Denied" : " · Allowed") : it.expired ? h("span", { class: "dim" }, " · Expired") : null);
    if (questions) {
      const picks = questions.map(() => new Set());
      const body = questions.map((q, n) =>
        h(
          "div",
          { class: "question" },
          h("div", { class: "q" }, q.question || q.header || ""),
          (q.options || []).map((o) =>
            h(
              "label",
              { class: "option" },
              h("input", {
                type: q.multiSelect ? "checkbox" : "radio",
                name: `${it.key}-${n}`,
                disabled: !live,
                onchange: (e) => {
                  if (!q.multiSelect) picks[n].clear();
                  if (e.target.checked) picks[n].add(o.label);
                  else picks[n].delete(o.label);
                },
              }),
              h("span", null, o.label, o.description ? h("span", { class: "dim" }, ` ${o.description}`) : null),
            ),
          ),
        ),
      );
      const send = h("button", { class: "primary", disabled: !live, onclick: () => answer("allow", questions.map((q, n) => [q.question, [...picks[n]].join(", ")])) }, "Answer");
      return h("div", { class: "approval" }, head, body, live ? h("div", { class: "buttons" }, send, h("button", { onclick: () => answer("deny") }, "Skip")) : null);
    }
    return h(
      "div",
      { class: "approval" },
      head,
      it.reason ? h("div", { class: "dim" }, it.reason) : null,
      detail(it.tool),
      live ? h("div", { class: "buttons" }, h("button", { class: "primary", onclick: () => answer("allow") }, "Allow"), it.always ? h("button", { onclick: () => answer("allowAlways") }, "Always allow") : null, h("button", { onclick: () => answer("deny") }, "Deny")) : null,
    );
  }
}

function asked(tool) {
  if (tool.kind !== "other" || tool.name !== "AskUserQuestion") return null;
  try {
    const q = JSON.parse(tool.input).questions;
    return Array.isArray(q) && q.length ? q : null;
  } catch {
    return null;
  }
}

function fold(open, title, right, onclick, body) {
  return h("div", { class: "fold" }, h("button", { class: "fold-head", onclick }, h("span", { class: `chev${open ? " open" : ""}` }, icon("chevron-right", 12)), h("span", { class: "fold-title" }, title), right), open ? body : null);
}

function state(s) {
  if (s === "working") return h("span", { class: "dots" }, h("i"), h("i"));
  if (s === "failed") return h("span", { class: "bad" }, "failed");
  if (s === "stopped") return h("span", { class: "dim" }, "stopped");
  return null;
}

function call(it, open, toggle) {
  const right = !it.done ? h("span", { class: "dots" }, h("i"), h("i")) : !it.done.ok ? h("span", { class: "bad" }, "failed") : null;
  return fold(open, label(it.tool), right, toggle, h("div", { class: "quiet" }, detail(it.tool), it.done && it.done.output ? h("pre", { class: "out" }, it.done.output) : null));
}

function detail(tool) {
  if (tool.kind === "edit") return tool.changes.map((c) => h("div", { class: "diff" }, h("div", { class: "diff-path" }, c.path), h("pre", null, c.diff.split("\n").map((l) => h("div", { class: l.startsWith("+") ? "add" : l.startsWith("-") ? "del" : l.startsWith("@@") ? "hunk" : "" }, l || " ")))));
  const text = input(tool);
  return text ? h("pre", { class: "out" }, text) : null;
}

function input(tool) {
  switch (tool.kind) {
    case "command":
      return tool.command;
    case "read":
      return tool.path;
    case "search":
      return tool.path ? `${tool.pattern}\nin ${tool.path}` : tool.pattern;
    case "web":
      return tool.target;
    case "agent":
      return tool.prompt || null;
    case "mcp":
    case "other":
      return tool.input && tool.input !== "{}" ? tool.input : null;
  }
  return null;
}

export function label(tool) {
  switch (tool.kind) {
    case "command":
      return `Run ${firstLine(withoutCd(tool.command))}`;
    case "read":
      return `Read ${short(tool.path)}`;
    case "edit": {
      const c = tool.changes;
      const verb = c.length === 1 && c[0].kind === "add" ? "Create" : c.length === 1 && c[0].kind === "delete" ? "Delete" : "Edit";
      return c.length ? `${verb} ${c.map((x) => short(x.path)).join(", ")}` : "Edit files";
    }
    case "search":
      return tool.path ? `Search ${short(tool.path)} for ${tool.pattern}` : `Search for ${tool.pattern}`;
    case "web":
      return `Look up ${tool.target}`;
    case "mcp":
      return `${tool.server}: ${tool.tool}`;
    case "agent":
      return tool.description || "Subagent";
    default:
      return tool.name;
  }
}

function short(path) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts.length <= 2 ? parts.join("/") || path : parts.slice(-2).join("/");
}

function firstLine(s) {
  const line = s.split("\n")[0] || "";
  return line.length > 90 ? `${line.slice(0, 90)}...` : line;
}

function withoutCd(cmd) {
  const t = cmd.trimStart();
  if (!t.startsWith("cd ")) return cmd;
  const rest = t.slice(3).trimStart();
  let after;
  if (rest.startsWith('"')) {
    const q = rest.indexOf('"', 1);
    if (q < 0) return cmd;
    after = rest.slice(q + 1);
  } else {
    const m = rest.search(/[\s;&]/);
    if (m < 0) return cmd;
    after = rest.slice(m);
  }
  after = after.trimStart();
  for (const sep of ["&&", ";"]) {
    if (after.startsWith(sep)) {
      const next = after.slice(sep.length).trimStart();
      if (next) return next;
    }
  }
  return cmd;
}

