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

    data class User(override val key: String, val text: String, val images: Int, val steer: Boolean) : Item
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
    ) : Item
    data class Error(override val key: String, val message: String) : Item
    data class Finished(override val key: String, val status: RunStatus, val ms: Long, val error: String?) : Item
}

data class Done(val ok: Boolean, val output: String)

enum class AgentState { Working, Done, Failed, Stopped }

class Transcript {
    private val items = ArrayList<Item>()
    private var next = 0
    private var running = false
    private var said = false

    /** The CLI's own name for its model, once it started. */
    var model: String? = null
        private set
    /** Tokens in the context window and its size, as of the latest reply. */
    var context: Pair<Long, Long>? = null
        private set

    fun items(): List<Item> = items.toList()

    fun reset() {
        items.clear()
        running = false
        said = false
        model = null
        context = null
    }

    private fun key() = "i${next++}"

    fun add(entry: Entry) {
        when (entry) {
            is Entry.PromptEntry -> {
                items += Item.User(key(), entry.prompt.text, entry.prompt.images.size, steer = running)
                if (!running) {
                    running = true
                    said = false
                }
            }
            is Entry.AnswerEntry -> answered(entry.request, entry.answer)
            is Entry.Run -> apply(entry.event)
        }
    }

    private fun answered(request: String, answer: Answer) {
        val i = items.indexOfLast { it is Item.Approval && it.request == request }
        if (i >= 0) items[i] = (items[i] as Item.Approval).copy(answer = answer)
    }

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
            RunEvent.Steered, is RunEvent.Usage -> {}
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
