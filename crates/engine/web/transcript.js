export class Transcript {
  constructor() {
    this.reset();
  }

  reset() {
    this.items = [];
    this.next = 0;
    this.running = false;
    this.said = false;
    this.model = null;
    this.context = null;
  }

  item(kind, fields) {
    const it = { kind, key: `i${this.next++}`, v: 0, ...fields };
    this.items.push(it);
    return it;
  }

  touch(it, fields) {
    Object.assign(it, fields);
    it.v++;
  }

  find(kind, id) {
    for (let i = this.items.length - 1; i >= 0; i--) {
      const it = this.items[i];
      if (it.kind === kind && it.id === id) return it;
    }
    return null;
  }

  add(entry, replay) {
    if (entry.type === "prompt") {
      this.item("user", { text: entry.prompt.text, images: entry.prompt.images.length, steer: this.running });
      if (!this.running) {
        this.running = true;
        this.said = false;
      }
    } else if (entry.type === "answer") {
      for (let i = this.items.length - 1; i >= 0; i--) {
        const it = this.items[i];
        if (it.kind === "approval" && it.request === entry.request) {
          this.touch(it, { answer: entry.answer });
          break;
        }
      }
    } else if (entry.type === "run") {
      this.apply(entry.event);
      if (entry.event.type === "started") this.restarted(replay);
    }
  }

  restarted(replay) {
    if (!this.running) return;
    const last = this.items[this.items.length - 1];
    if (last && last.kind === "user") {
      if (!last.steer) return;
      this.items.pop();
      this.cut();
      this.items.push(last);
      this.touch(last, { steer: false });
      this.running = true;
      this.said = false;
    } else if (this.said && replay) this.cut();
  }

  cut() {
    for (const it of this.items) if (it.kind === "call" && !it.done) this.touch(it, { done: { ok: false, output: "The app closed before it finished." } });
    this.running = false;
    this.expire();
    for (const it of this.items) if (it.kind === "agent" && it.state === "working") this.touch(it, { state: "stopped" });
    this.item("note", { text: "The app closed before this run finished." });
  }

  apply(e) {
    const last = this.items[this.items.length - 1];
    switch (e.type) {
      case "started":
        this.model = e.model;
        break;
      case "text":
        this.said = true;
        if (last && last.kind === "reply") this.touch(last, { text: last.text + e.text });
        else this.item("reply", { text: e.text });
        break;
      case "thinking":
        if (last && last.kind === "thinking") this.touch(last, { text: last.text + e.text });
        else this.item("thinking", { text: e.text });
        break;
      case "tool":
        if (e.tool.kind === "agent") {
          const a = this.find("agent", e.id);
          const f = { description: e.tool.description, agentType: e.tool.agent_type, prompt: e.tool.prompt };
          if (a) this.touch(a, f);
          else this.item("agent", { id: e.id, ...f, calls: [], said: "", state: "working", report: "" });
        } else {
          const c = this.find("call", e.id);
          if (c) this.touch(c, { tool: e.tool });
          else this.item("call", { id: e.id, tool: e.tool, done: null });
        }
        break;
      case "toolDone": {
        const a = this.find("agent", e.id);
        if (a) this.touch(a, { state: e.ok ? "done" : "failed", report: e.output });
        else {
          const c = this.find("call", e.id);
          if (c) this.touch(c, { done: { ok: e.ok, output: e.output } });
        }
        break;
      }
      case "subagentTool": {
        const a = this.find("agent", e.parent);
        if (!a) break;
        const c = a.calls.findLast((x) => x.id === e.id);
        if (c) c.tool = e.tool;
        else a.calls.push({ id: e.id, tool: e.tool, done: null });
        a.v++;
        break;
      }
      case "subagentToolDone": {
        const a = this.find("agent", e.parent);
        const c = a && a.calls.findLast((x) => x.id === e.id);
        if (c) {
          c.done = { ok: e.ok, output: e.output };
          a.v++;
        }
        break;
      }
      case "subagentText": {
        const a = this.find("agent", e.parent);
        if (a) this.touch(a, { said: a.said ? `${a.said}\n\n${e.text.trim()}` : e.text.trim() });
        break;
      }
      case "woke":
        if (!this.running) {
          this.running = true;
          this.said = false;
        }
        break;
      case "approval":
        this.item("approval", { request: e.request, tool: e.tool, reason: e.reason, always: e.always, answer: null, expired: false });
        break;
      case "context":
        this.context = [e.used, e.window];
        break;
      case "error":
        this.item("error", { message: e.message });
        break;
      case "finished": {
        if (e.text && !(this.running && this.said)) this.item("reply", { text: e.text });
        this.running = false;
        this.expire();
        let since = this.items.length - 1;
        while (since >= 0 && this.items[since].kind !== "user") since--;
        const shown = this.items.slice(since + 1).some((it) => it.kind === "error" && it.message === e.error);
        this.item("finished", { status: e.status, ms: e.ms, error: shown ? null : e.error });
        break;
      }
      case "failed":
        this.item("error", { message: e.message });
        this.running = false;
        this.expire();
        for (const it of this.items) if (it.kind === "agent" && it.state === "working") this.touch(it, { state: "stopped" });
        break;
    }
  }

  expire() {
    for (const it of this.items) if (it.kind === "approval" && !it.answer && !it.expired) this.touch(it, { expired: true });
  }

  waiting() {
    return this.items.some((it) => it.kind === "approval" && !it.answer && !it.expired);
  }
}
