import { sidebar } from "./board.js";
import { Chat } from "./chat.js";
import { elapsed, h, rgba } from "./dom.js";
import { icon, mark } from "./icons.js";
import { Link } from "./link.js";
import { Term } from "./term.js";

const VARS = {
  bg: "bg",
  surface1: "surface1",
  surface2: "surface2",
  surface3: "surface3",
  accent: "accent",
  onAccent: "on-accent",
  link: "link",
  text1: "text1",
  text2: "text2",
  text3: "text3",
  border0: "border0",
  border1: "border1",
  border2: "border2",
  ink: "ink",
  busy: "busy",
  waiting: "waiting",
  ok: "ok",
  error: "error",
  diffAdd: "diff-add",
  diffDel: "diff-del",
  termBg: "term-bg",
  termFg: "term-fg",
  cursor: "cursor",
};

class App {
  constructor(root) {
    this.root = root;
    this.board = null;
    this.desktop = null;
    this.route = parse();
    this.views = new Map();
    this.watched = null;
    this.shelves = new Set();
    this.notice = null;
    this.requests = 0;
    this.link = new Link((d) => this.down(d));
    this.side = h("div", { class: "side-wrap" });
    this.main = h("main", { class: "main" });
    this.toast = h("div", { class: "toast" });
    this.search = "";
    this.searchBox = h("input", {
      class: "search-input",
      placeholder: "Search",
      oninput: () => {
        this.search = this.searchBox.value;
        this.refresh();
      },
    });
    setInterval(() => {
      for (const el of document.querySelectorAll("[data-since]")) el.textContent = elapsed(Number(el.dataset.since));
    }, 1000);
    window.addEventListener("hashchange", () => {
      this.route = parse();
      this.show();
    });
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => this.theme());
    this.link.start();
    this.show();
  }

  palette() {
    const t = this.board && this.board.theme;
    if (!t) return {};
    return document.documentElement.dataset.side === "dark" ? t.dark : t.light;
  }

  theme() {
    const t = this.board && this.board.theme;
    if (!t) return;
    const dark = t.scheme === "dark" || (t.scheme !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
    const root = document.documentElement;
    root.dataset.side = dark ? "dark" : "light";
    const p = dark ? t.dark : t.light;
    for (const [k, v] of Object.entries(VARS)) if (p[k] != null) root.style.setProperty(`--${v}`, rgba(p[k]));
    root.style.colorScheme = dark ? "dark" : "light";
  }

  down(d) {
    switch (d.type) {
      case "link":
        if (d.state === "online") this.watch(true);
        if (d.message) this.say(d.message);
        this.show();
        break;
      case "welcome":
        this.desktop = d.desktop;
        break;
      case "board": {
        const first = !this.board;
        this.board = d.board;
        this.theme();
        if (first) this.show();
        else this.refresh();
        break;
      }
      case "transcript": {
        const v = this.views.get(d.thread);
        if (v instanceof Chat) v.entries(d.entries, d.reset);
        break;
      }
      case "term": {
        const v = this.views.get(d.frame.thread);
        if (v instanceof Term) v.frame(d.frame);
        break;
      }
      case "failed":
      case "note":
        this.say(d.message);
        break;
      case "aside": {
        const v = this.views.get(d.thread);
        if (v instanceof Chat) v.side(d.aside || null);
        break;
      }
      case "started":
        if (d.request === this.requests) this.go({ thread: d.thread });
        break;
    }
  }

  send(up) {
    return this.link.send(up);
  }

  ask(ask) {
    const ok = this.send({ type: "ask", ask });
    if (!ok) this.say("Not connected. Your message wasn't sent.");
    return ok;
  }

  say(message) {
    this.toast.textContent = message;
    this.toast.classList.add("on");
    clearTimeout(this.notice);
    this.notice = setTimeout(() => this.toast.classList.remove("on"), 5000);
  }

  go(route) {
    location.hash = route.thread ? `#/t/${route.thread}` : route.compose ? "#/new" : "#/";
  }

  forget() {
    this.link.forget();
    this.board = null;
    this.show();
  }

  toggleShelf(name) {
    if (this.shelves.has(name)) this.shelves.delete(name);
    else this.shelves.add(name);
    this.refresh();
  }

  current() {
    return this.board && this.route.thread ? this.board.threads.find((t) => t.id === this.route.thread) : null;
  }

  watch(again) {
    const id = this.current() ? this.route.thread : null;
    if (id === this.watched && !again) return;
    if (this.watched != null && this.watched !== id) {
      this.send({ type: "unwatch", thread: this.watched });
      const old = this.views.get(this.watched);
      if (old instanceof Term) old.leave();
      this.views.delete(this.watched);
    }
    this.watched = id;
    if (id == null) return;
    const v = this.views.get(id);
    if (v instanceof Chat) v.side(null);
    this.send({ type: "watch", thread: id });
    if (v instanceof Term && again) {
      v.size = null;
      v.fit();
    }
  }

  drawSide() {
    const box = this.searchBox;
    const focused = document.activeElement === box;
    const [from, to] = [box.selectionStart, box.selectionEnd];
    this.side.replaceChildren(sidebar(this));
    if (focused) {
      box.focus({ preventScroll: true });
      box.setSelectionRange(from, to);
    }
  }

  refresh() {
    if (!this.board) return;
    this.drawSide();
    const t = this.current();
    const v = t && this.views.get(t.id);
    if (v instanceof Chat) v.thread(t);
    if (this.route.thread && !t) this.show();
    if (this.header && t) this.header.replaceWith((this.header = header(this, t)));
  }

  show() {
    if (!this.link.paired()) {
      this.root.replaceChildren(pairing(this), this.toast);
      return;
    }
    if (this.link.state === "denied") {
      this.root.replaceChildren(
        h("div", { class: "center" }, h("div", { class: "pair" }, h("p", { class: "dim" }, this.link.message || "HyprSpace didn't let this browser in."), h("div", { class: "pair-row" }, h("button", { class: "primary", onclick: () => this.link.start() }, "Try again"), h("button", { onclick: () => this.forget() }, "Sign out")))),
        this.toast,
      );
      return;
    }
    if (!this.board) {
      this.root.replaceChildren(h("div", { class: "center dim" }, this.link.state === "offline" ? "Can't reach HyprSpace. Retrying." : "Connecting to HyprSpace"), this.toast);
      return;
    }
    const t = this.current();
    this.watch(false);
    this.drawSide();
    this.header = null;
    let body;
    if (t) {
      let v = this.views.get(t.id);
      if (!v) {
        v = t.kind === "structured" ? new Chat(this, t.id) : new Term(this, t.id);
        this.views.set(t.id, v);
      }
      if (v instanceof Chat) v.thread(t);
      this.header = header(this, t);
      body = h("div", { class: "thread" }, this.header, v.el);
      if (v instanceof Term) requestAnimationFrame(() => v.focus());
    } else if (this.route.compose) body = compose(this);
    else body = h("div", { class: "center dim" }, "Pick a thread, or start a new one.");
    this.main.replaceChildren(body);
    this.root.replaceChildren(h("div", { class: `app${t || this.route.compose ? " deep" : ""}` }, this.side, this.main), this.toast);
  }
}

function parse() {
  const m = location.hash.match(/^#\/t\/(\d+)/);
  return { thread: m ? Number(m[1]) : null, compose: location.hash === "#/new" };
}

function header(app, t) {
  const space = app.board.spaces.find((s) => s.id === t.space);
  const path = space && space.path ? space.path.split(/[\\/]/).filter(Boolean).slice(-2).join("/") : "";
  return h(
    "header",
    { class: "thread-head" },
    h("button", { class: "back", onclick: () => app.go({}) }, icon("chevron-right", 16, "flip")),
    t.agent ? mark(t.agent, 14) : icon("terminal", 14),
    h("span", { class: "head-title" }, t.title || "New thread"),
    h("span", { class: "head-path" }, [path, t.model].filter(Boolean).join(" · ")),
  );
}

function pairing(app) {
  const code = h("input", { class: "pair-code", placeholder: "ABCD EFGH JKLM", autocomplete: "off", spellcheck: false, autofocus: true });
  const go = (e) => {
    e.preventDefault();
    if (code.value.replace(/[^a-z0-9]/gi, "").length < 12) return app.say("The code has 12 letters and digits.");
    app.link.pair(code.value);
  };
  return h(
    "div",
    { class: "center" },
    h(
      "form",
      { class: "pair", onsubmit: go },
      h("h1", null, "Open HyprSpace here"),
      h("p", { class: "dim" }, "On the computer running HyprSpace, open Settings, Phone, and choose Pair a phone. Type the code it shows."),
      code,
      h("button", { class: "primary", type: "submit" }, app.link.state === "connecting" ? "Pairing" : "Pair"),
    ),
  );
}

function compose(app) {
  const b = app.board;
  const spaces = b.spaces.filter((s) => s.path);
  const agents = b.agents.filter((a) => a.status.installed);
  const space = h("select", null, spaces.map((s) => h("option", { value: s.id }, s.name)));
  const agent = h("select", null, agents.map((a) => h("option", { value: a.agent }, a.agent === "claude" ? "Claude" : a.agent === "codex" ? "Codex" : a.agent)), h("option", { value: "" }, "Shell"));
  if ([...agent.options].some((o) => o.value === b.start.agent)) agent.value = b.start.agent;
  const mode = h("select", null, h("option", { value: "structured" }, "Structured"), h("option", { value: "terminal" }, "Terminal"));
  mode.value = b.start.structured ? "structured" : "terminal";
  const prompt = h("textarea", { rows: 5, placeholder: "What should we work on?" });
  const start = (e) => {
    e.preventDefault();
    const pick = (b.start.picks || []).find((p) => p[0] === agent.value);
    app.requests += 1;
    app.ask({
      type: "new",
      space: Number(space.value),
      start: { agent: agent.value || null, model: pick ? pick[1] : "", effort: pick ? pick[2] : "", permission: b.start.permission, terminal: mode.value === "terminal" || !agent.value, prompt: prompt.value.trim(), images: [] },
      request: app.requests,
    });
  };
  return h(
    "form",
    { class: "compose", onsubmit: start },
    h("header", { class: "thread-head" }, h("button", { class: "back", type: "button", onclick: () => app.go({}) }, icon("chevron-right", 16, "flip")), h("div", { class: "head-title" }, "New thread")),
    h("div", { class: "compose-body" }, h("label", null, "Folder", space), h("div", { class: "pair-row" }, h("label", null, "Agent", agent), h("label", null, "Mode", mode)), prompt, h("button", { class: "primary", type: "submit" }, "Start")),
  );
}

new App(document.getElementById("app"));
