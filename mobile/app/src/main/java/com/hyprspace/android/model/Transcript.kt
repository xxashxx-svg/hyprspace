// A structured thread's transcript, built from its journal the way the desktop builds it
// (crates/ui/src/transcript/model.rs): prompts, replies, tool calls, subagents, approvals and
// the end of each run. Entries come in order, a whole journal first and then one at a time.

package com.hyprspace.android.model

import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Entry
import com.hyprspace.android.net.RunEvent
import com.hyprspace.android.net.RunStatus
import com.hyprspace.android.net.Tool

sealed interface Item {
    /** Stable across updates, for list keys. */
    val key: String

    /** [at] counts the thread's prompts up to this one, from its very first. */
    data class User(override val key: String, val text: String, val images: Int, val steer: Boolean, val at: Int = 0) : Item
    data class Reply(override val key: String, val text: String) : Item
    data class Thinking(override val key: String, val text: String) : Item
    data class Call(override val key: String, val id: String, val tool: Tool, val done: Done?) : Item
    data class Agent(
        override val key: String,
        val id: String,
        val description: String,
        val agentType: String,
        val prompt: String,
        val calls: List<Call>,
        val said: String,
        val state: AgentState,
        val report: String,
    ) : Item {
        /** The final report once there is one, else what it said so far. */
        val answer: String get() = report.ifBlank { said }
    }
    data class Approval(
        override val key: String,
        val request: String,
        val tool: Tool,
        val reason: String?,
        val always: Boolean,
        val answer: Answer?,
        val expired: Boolean,
        /** A question's answers, as (question, answer). */
        val given: List<Pair<String, String>> = emptyList(),
    ) : Item
    data class Error(override val key: String, val message: String) : Item
    data class Note(override val key: String, val text: String) : Item
    data class Finished(override val key: String, val status: RunStatus, val ms: Long, val error: String?) : Item
}

data class Done(val ok: Boolean, val output: String)

enum class AgentState { Working, Done, Failed, Stopped }

class Transcript {
    private val items = ArrayList<Item>()
    // keys count prompts and the items since each, which a whole journal and a live one agree on
    private var prompt = 0
    private var sub = 0
    private var running = false
    private var said = false

    /** The CLI's own name for its model, once it started. */
    var model: String? = null
        private set
    /** Tokens in the context window and its size, as of the latest reply. */
    var context: Pair<Long, Long>? = null
        private set

    /** The prompts before the first one here, which the computer left out. */
    var before = 0
        private set
    val prompts: Int get() = prompt

    fun items(): List<Item> = items.toList()

    fun reset(before: Int = 0) {
        this.before = before
        prompt = before
        sub = 0
        items.clear()
        running = false
        said = false
        model = null
        context = null
    }

    private fun key() = "p$prompt.${sub++}"

    /** [replay] for a journal sent whole. */
    fun add(entry: Entry, replay: Boolean = false) {
        when (entry) {
            is Entry.PromptEntry -> {
                prompt++
                sub = 0
                items += Item.User(key(), entry.prompt.text, entry.prompt.images.size, steer = running, at = prompt)
                if (!running) {
                    running = true
                    said = false
                }
            }
            is Entry.AnswerEntry -> answered(entry.request, entry.answer, entry.answers.mapNotNull { it.pair() })
            is Entry.Run -> {
                apply(entry.event)
                if (entry.event is RunEvent.Started) restarted(replay)
            }
        }
    }

    private fun answered(request: String, answer: Answer, given: List<Pair<String, String>>) {
        val i = items.indexOfLast { it is Item.Approval && it.request == request }
        if (i >= 0) items[i] = (items[i] as Item.Approval).copy(answer = answer, given = given)
    }

    private fun List<String>.pair(): Pair<String, String>? = if (size == 2) this[0] to this[1] else null

    private fun apply(event: RunEvent) {
        when (event) {
            is RunEvent.Started -> model = event.model
            is RunEvent.Text -> {
                said = true
                val last = items.lastOrNull()
                if (last is Item.Reply) items[items.lastIndex] = last.copy(text = last.text + event.text)
                else items += Item.Reply(key(), event.text)
            }
            is RunEvent.Thinking -> {
                val last = items.lastOrNull()
                if (last is Item.Thinking) items[items.lastIndex] = last.copy(text = last.text + event.text)
                else items += Item.Thinking(key(), event.text)
            }
            is RunEvent.ToolCall -> {
                val tool = event.tool
                if (tool is Tool.AgentCall) {
                    val i = agent(event.id)
                    if (i >= 0) {
                        items[i] = (items[i] as Item.Agent).copy(
                            description = tool.description,
                            agentType = tool.agentType,
                            prompt = tool.prompt,
                        )
                    } else {
                        items += Item.Agent(
                            key(), event.id, tool.description, tool.agentType, tool.prompt,
                            emptyList(), "", AgentState.Working, "",
                        )
                    }
                } else {
                    val i = call(event.id)
                    if (i >= 0) items[i] = (items[i] as Item.Call).copy(tool = tool)
                    else items += Item.Call(key(), event.id, tool, null)
                }
            }
            is RunEvent.ToolDone -> {
                val a = agent(event.id)
                if (a >= 0) {
                    items[a] = (items[a] as Item.Agent).copy(
                        state = if (event.ok) AgentState.Done else AgentState.Failed,
                        report = event.output,
                    )
                } else {
                    val i = call(event.id)
                    if (i >= 0) items[i] = (items[i] as Item.Call).copy(done = Done(event.ok, event.output))
                }
            }
            is RunEvent.SubagentTool -> {
                val a = agent(event.parent)
                if (a >= 0) {
                    val ag = items[a] as Item.Agent
                    val c = ag.calls.indexOfLast { it.id == event.id }
                    val calls = if (c >= 0) ag.calls.toMutableList().also { it[c] = it[c].copy(tool = event.tool) }
                    else ag.calls + Item.Call(key(), event.id, event.tool, null)
                    items[a] = ag.copy(calls = calls)
                }
            }
            is RunEvent.SubagentToolDone -> {
                val a = agent(event.parent)
                if (a >= 0) {
                    val ag = items[a] as Item.Agent
                    val c = ag.calls.indexOfLast { it.id == event.id }
                    if (c >= 0) {
                        val calls = ag.calls.toMutableList()
                        calls[c] = calls[c].copy(done = Done(event.ok, event.output))
                        items[a] = ag.copy(calls = calls)
                    }
                }
            }
            is RunEvent.SubagentText -> {
                val a = agent(event.parent)
                if (a >= 0) {
                    val ag = items[a] as Item.Agent
                    val said = if (ag.said.isEmpty()) event.text.trim() else ag.said + "\n\n" + event.text.trim()
                    items[a] = ag.copy(said = said)
                }
            }
            RunEvent.Woke -> if (!running) {
                running = true
                said = false
            }
            is RunEvent.Approval -> items += Item.Approval(
                key(), event.request, event.tool, event.reason, event.always, null, false,
            )
            is RunEvent.Context -> context = event.used to event.window
            RunEvent.Steered, is RunEvent.Usage, is RunEvent.Limited -> {}
            is RunEvent.Error -> items += Item.Error(key(), event.message)
            is RunEvent.Finished -> {
                if (event.text.isNotEmpty() && !(running && said)) items += Item.Reply(key(), event.text)
                running = false
                expire()
                // the CLI often reports a failure as an error and again as the run's end
                val since = items.indexOfLast { it is Item.User }
                val shown = items.drop(since + 1).any { it is Item.Error && it.message == event.error }
                items += Item.Finished(key(), event.status, event.ms, event.error.takeIf { !shown })
            }
            is RunEvent.Failed -> {
                items += Item.Error(key(), event.message)
                running = false
                expire()
                stopAgents()
            }
        }
    }

    /**
     * A new CLI process started. A run still open from the one before died with it, and the
     * journal never says so, so the prompt that started this process reads as a steer of it.
     * A process can also restart mid-run, so a run with no prompt after it ends only on replay.
     */
    private fun restarted(replay: Boolean) {
        if (!running) return
        val last = items.lastOrNull()
        when {
            last is Item.User && last.steer -> {
                items.removeAt(items.lastIndex)
                cut()
                items += last.copy(steer = false)
                running = true
                said = false
            }
            last is Item.User -> {}
            said && replay -> cut()
        }
    }

    private fun cut() {
        for (i in items.indices) {
            val it = items[i]
            if (it is Item.Call && it.done == null) items[i] = it.copy(done = Done(false, "The app closed before it finished."))
        }
        running = false
        expire()
        stopAgents()
        items += Item.Note(key(), "The app closed before this run finished.")
    }

    // an approval left open when its run ended can't be answered any more
    private fun expire() {
        for (i in items.indices) {
            val it = items[i]
            if (it is Item.Approval && it.answer == null && !it.expired) items[i] = it.copy(expired = true)
        }
    }

    private fun stopAgents() {
        for (i in items.indices) {
            val it = items[i]
            if (it is Item.Agent && it.state == AgentState.Working) items[i] = it.copy(state = AgentState.Stopped)
        }
    }

    private fun agent(id: String) = items.indexOfLast { it is Item.Agent && it.id == id }
    private fun call(id: String) = items.indexOfLast { it is Item.Call && it.id == id }
}

/** The conversation as Markdown: what was asked, what came back, and a line for each tool call. */
fun exported(items: List<Item>): String = items.mapNotNull {
    when (it) {
        is Item.User -> "## You\n\n${it.text.trim()}"
        is Item.Reply -> it.text.trim().ifEmpty { null }
        is Item.Call -> "- ${label(it.tool)}"
        else -> null
    }
}.joinToString("\n\n") + "\n"

data class Fold(val prompt: String, val first: String, val ms: Long?, val hide: Set<String>, val finish: String)

private fun foldable(item: Item): Boolean = when (item) {
    is Item.Thinking, is Item.Reply -> true
    is Item.Call -> special(item.tool).let { it != QUESTION && it != PLAN }
    is Item.Approval -> special(item.tool).let { it != QUESTION && it != PLAN }
    is Item.Agent -> item.state != AgentState.Working
    else -> false
}

private fun runs(items: List<Item>): List<Triple<Int, Int, Int>> {
    val out = ArrayList<Triple<Int, Int, Int>>()
    var open: Pair<Int, Int>? = null
    for ((ix, item) in items.withIndex()) {
        when {
            item is Item.User && !item.steer -> open = ix to ix + 1
            item is Item.Finished -> {
                open?.let { out += Triple(it.first, it.second, ix) }
                open = null
            }
            open == null && (foldable(item) || item is Item.Error) -> open = ix to ix
        }
    }
    return out
}

fun folds(items: List<Item>): List<Fold> {
    val out = ArrayList<Fold>()
    for ((anchor, first, end) in runs(items)) {
        val done = items[end] as Item.Finished
        val run = first until end
        if (done.status != RunStatus.Done || run.any { items[it] is Item.Error }) continue
        val reply = run.lastOrNull { items[it] is Item.Reply }
        val cuts = listOf(anchor to first) + run.filter { items[it] is Item.User }.map { it to it + 1 }
        for ((i, cut) in cuts.withIndex()) {
            val to = cuts.getOrNull(i + 1)?.first ?: end
            val hide = (cut.second until to).filter { it != reply && foldable(items[it]) }.map { items[it].key }.toSet()
            if (hide.isNotEmpty()) {
                out += Fold(items[cut.first].key, items[cut.second].key, if (i == 0) done.ms else null, hide, items[end].key)
            }
        }
    }
    return out
}
