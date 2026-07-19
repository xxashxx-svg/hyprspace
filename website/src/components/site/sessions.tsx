import { ClaudeMessage } from "@/components/brainless/claude/claude-message"
import { ClaudeTodoList } from "@/components/brainless/claude/claude-todo-list"
import { ClaudeToolCall } from "@/components/brainless/claude/claude-tool-call"
import { ClaudeDiff } from "@/components/brainless/claude/claude-diff"
import { ClaudeThinking } from "@/components/brainless/claude/claude-thinking"
import { ClaudePrompt } from "@/components/brainless/claude/claude-prompt"

import { CodexMessage } from "@/components/brainless/codex/codex-message"
import { CodexExec } from "@/components/brainless/codex/codex-exec"
import { CodexWorking } from "@/components/brainless/codex/codex-working"
import { CodexPrompt } from "@/components/brainless/codex/codex-prompt"

import { GrokMessage } from "@/components/brainless/grok/grok-message"
import { GrokThought } from "@/components/brainless/grok/grok-thought"
import { GrokTool } from "@/components/brainless/grok/grok-tool"
import { GrokWrite } from "@/components/brainless/grok/grok-write"
import { GrokTurnEnd } from "@/components/brainless/grok/grok-turn-end"
import { GrokPrompt } from "@/components/brainless/grok/grok-prompt"

/* Three panes, one feature, split across three providers — the whole pitch.
   Content is deliberately short so each pane reads at a glance in a tile. */

const shell = "space-y-2.5 font-mono text-[13px] leading-[1.6]"

export function ClaudePane() {
  return (
    <div className={`${shell} text-[#c0caf5]`}>
      <ClaudeMessage role="user">
        implement the session resume endpoint
      </ClaudeMessage>

      <ClaudeTodoList
        todos={[
          { label: "Read the pty session store", status: "done" },
          { label: "Add POST /session/:id/resume", status: "active" },
          { label: "Wire it to the pane store", status: "todo" },
        ]}
      />

      <ClaudeToolCall tool="Read" arg="src/pty.rs" result="Read 214 lines" />

      <ClaudeDiff
        file="src/pty.rs"
        summary="Updated src/pty.rs with 3 additions"
        lines={[
          { type: "ctx", n: 88, text: "pub fn resume(&self, id: &str)" },
          { type: "add", n: 89, text: "    let sess = self.get(id)?;" },
          { type: "add", n: 90, text: "    sess.replay_scrollback();" },
          { type: "add", n: 91, text: "    Ok(sess.handle())" },
        ]}
      />

      <ClaudeThinking />

      <div className="pt-1">
        <ClaudePrompt placeholder="ask anything" />
      </div>
    </div>
  )
}

export function CodexPane() {
  return (
    <div className={`${shell} text-[#ededed]`}>
      <CodexMessage role="user">
        cover the resume path with tests
      </CodexMessage>

      <CodexMessage>
        I&apos;ll add a replay case and a reconnect case, then run the suite.
      </CodexMessage>

      <div className="space-y-1">
        <CodexExec command="Read src/pty.rs" />
        <CodexExec command="Added tests/resume.rs" result="(+64 −0)" />
        <CodexExec command="Ran cargo test resume" result="→ passed">
          {`running 2 tests
test resume::replays_scrollback ... ok
test resume::reconnects_pty ... ok

test result: ok. 2 passed; 0 failed`}
        </CodexExec>
      </div>

      <CodexWorking />

      <div className="pt-1">
        <CodexPrompt directory="~/hyprspace" />
      </div>
    </div>
  )
}

export function GrokPane() {
  return (
    <div className={`${shell} text-[#e8e8e8]`}>
      <GrokMessage role="user" time="4:38 PM">
        document the resume behaviour
      </GrokMessage>

      <GrokThought elapsed="0.4s" />

      <GrokMessage time="4:38 PM">
        Adding a Resume section to the architecture doc.
      </GrokMessage>

      <GrokTool verb="read" path="docs/ARCHITECTURE.md" />
      <GrokWrite
        before={[{ n: 41, text: "## Panes" }]}
        after={[{ n: 41, text: "## Panes\n\n### Resume" }]}
      />

      <GrokTurnEnd elapsed="9.1s" />

      <div className="pt-1">
        <GrokPrompt showShortcuts={false} />
      </div>
    </div>
  )
}
