// What the phone and the desktop say to each other, mirrored from the desktop's
// crates/proto/src/phone.rs (and the run, journal and agent types it carries). A change there
// changes these too and moves PROTOCOL.

@file:OptIn(ExperimentalSerializationApi::class)

package com.hyprspace.android.net

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonClassDiscriminator
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonPrimitive

const val PROTOCOL = 1

val wire = Json {
    ignoreUnknownKeys = true
    classDiscriminator = "type"
    encodeDefaults = true
    explicitNulls = false
}

@Serializable
enum class Agent(val label: String) {
    @SerialName("claude") Claude("Claude"),
    @SerialName("codex") Codex("Codex"),
}

@Serializable
enum class Permission(val label: String, val about: String) {
    @SerialName("plan") Plan("Plan only", "Reads and plans. It changes nothing."),
    @SerialName("ask") Ask("Ask first", "Asks before every edit and command."),
    @SerialName("auto") Auto("Auto edit", "Edits files in the folder on its own. Asks before anything else."),
    @SerialName("bypass") Bypass("Full access", "Never asks. Use it only in folders you trust."),
}

@Serializable
enum class Answer {
    @SerialName("allow") Allow,
    @SerialName("allowAlways") AllowAlways,
    @SerialName("deny") Deny,
}

@Serializable
enum class RunStatus {
    @SerialName("done") Done,
    @SerialName("interrupted") Interrupted,
    @SerialName("failed") Failed,
}

@Serializable
enum class ChangeKind {
    @SerialName("add") Add,
    @SerialName("update") Update,
    @SerialName("delete") Delete,
}

@Serializable
enum class Scheme {
    @SerialName("system") System,
    @SerialName("light") Light,
    @SerialName("dark") Dark,
}

// ---- phone to desktop ----

@Serializable
sealed interface Up {
    @Serializable @SerialName("hello")
    data class Hello(val token: String, val device: String, val protocol: Int = PROTOCOL) : Up

    @Serializable @SerialName("pair")
    data class Pair(val code: String, val device: String, val protocol: Int = PROTOCOL) : Up

    @Serializable @SerialName("watch")
    data class Watch(val thread: Long) : Up

    @Serializable @SerialName("unwatch")
    data class Unwatch(val thread: Long) : Up

    @Serializable @SerialName("fit")
    data class Fit(val thread: Long, val cols: Int, val rows: Int) : Up

    @Serializable @SerialName("unfit")
    data class Unfit(val thread: Long) : Up

    @Serializable @SerialName("keys")
    data class Keys(val thread: Long, val text: String) : Up

    @Serializable @SerialName("paste")
    data class Paste(val thread: Long, val text: String) : Up

    @Serializable @SerialName("ask")
    data class Do(val ask: Ask) : Up

    @Serializable @SerialName("ping")
    data object Ping : Up
}

@Serializable
sealed interface Ask {
    @Serializable @SerialName("send")
    data class Send(val thread: Long, val text: String) : Ask

    @Serializable @SerialName("approve")
    data class Approve(val thread: Long, val request: String, val answer: Answer) : Ask

    @Serializable @SerialName("interrupt")
    data class Interrupt(val thread: Long) : Ask

    @Serializable @SerialName("new")
    data class New(val space: Long, val start: NewThread) : Ask

    @Serializable @SerialName("settle")
    data class Settle(val thread: Long, val on: Boolean) : Ask
}

@Serializable
data class NewThread(
    val agent: Agent?,
    val model: String,
    val effort: String,
    val permission: Permission,
    val terminal: Boolean,
    val prompt: String,
)

// ---- desktop to phone ----

@Serializable
sealed interface Down {
    @Serializable @SerialName("welcome")
    data class Welcome(val desktop: String, val version: String, val token: String? = null) : Down

    @Serializable @SerialName("denied")
    data class Denied(val message: String) : Down

    @Serializable @SerialName("board")
    data class BoardMsg(val board: Board) : Down

    @Serializable @SerialName("transcript")
    data class Transcript(val thread: Long, val entries: List<Entry>, val reset: Boolean) : Down

    @Serializable @SerialName("term")
    data class Term(val frame: TermFrame) : Down

    @Serializable @SerialName("failed")
    data class Failed(val thread: Long? = null, val message: String) : Down

    @Serializable @SerialName("pong")
    data object Pong : Down
}

@Serializable
data class Board(
    val spaces: List<BoardSpace> = emptyList(),
    val threads: List<BoardThread> = emptyList(),
    val agents: List<AgentInfo> = emptyList(),
    val start: StartPrefs = StartPrefs(),
    val theme: BoardTheme = BoardTheme(),
    /** Someone used the computer's keyboard or mouse in the last two minutes. */
    val present: Boolean = false,
)

@Serializable
data class BoardSpace(
    val id: Long = 0,
    val name: String = "",
    val path: String = "",
    /** Fill and lettering, light side then dark. */
    val tag: List<Long> = emptyList(),
)

@Serializable
enum class BoardKind {
    @SerialName("terminal") Terminal,
    @SerialName("structured") Structured,
}

@Serializable
enum class BoardStatus {
    @SerialName("idle") Idle,
    @SerialName("working") Working,
    @SerialName("waiting") Waiting,
    @SerialName("done") Done,
    @SerialName("failed") Failed,
}

@Serializable
enum class Shelf {
    @SerialName("active") Active,
    @SerialName("settled") Settled,
    @SerialName("snoozed") Snoozed,
}

@Serializable
data class BoardThread(
    val id: Long = 0,
    val space: Long = 0,
    val title: String = "",
    val kind: BoardKind = BoardKind.Terminal,
    val agent: Agent? = null,
    val model: String? = null,
    val status: BoardStatus = BoardStatus.Idle,
    val doing: String? = null,
    val since: Long? = null,
    val unseen: Boolean = false,
    val place: String = "",
    val branch: Boolean = false,
    val touched: Long = 0,
    val shelf: Shelf = Shelf.Active,
    val rank: Long = 0,
    val live: Boolean = false,
)

@Serializable
data class StartPrefs(
    val agent: Agent? = null,
    val permission: Permission = Permission.Ask,
    /** Per agent: [agent, model, effort]. */
    val picks: List<List<String>> = emptyList(),
    val structured: Boolean = false,
) {
    fun pick(agent: Agent): Pair<String, String> {
        val id = wire.encodeToString(Agent.serializer(), agent).trim('"')
        val p = picks.firstOrNull { it.firstOrNull() == id }
        return (p?.getOrNull(1) ?: "") to (p?.getOrNull(2) ?: "")
    }
}

@Serializable
data class AgentInfo(val agent: Agent, val status: ProviderStatus = ProviderStatus(), val catalog: AgentCatalog)

@Serializable
data class ProviderStatus(
    val id: String = "",
    val installed: Boolean = false,
    val version: String? = null,
    val account: String? = null,
    val plan: String? = null,
)

@Serializable
data class AgentCatalog(
    val agent: Agent,
    val models: List<ModelInfo> = emptyList(),
    val efforts: List<String> = emptyList(),
) {
    fun effortsFor(model: String): List<String> {
        val id = model.substringBefore('[')
        val m = models.firstOrNull { it.id == id }
        return if (m != null && m.efforts.isNotEmpty()) m.efforts else efforts
    }
}

@Serializable
data class ModelInfo(
    val id: String = "",
    val label: String = "",
    val note: String? = null,
    val efforts: List<String> = emptyList(),
    val defaultEffort: String? = null,
)

@Serializable
data class BoardTheme(
    val scheme: Scheme = Scheme.System,
    val light: Palette = Palette(),
    val dark: Palette = Palette(),
)

/** A theme's colors as 0xRRGGBBAA. */
@Serializable
data class Palette(
    val bg: Long = 0,
    val surface1: Long = 0,
    val surface2: Long = 0,
    val surface3: Long = 0,
    val accent: Long = 0,
    val onAccent: Long = 0,
    val link: Long = 0,
    val text1: Long = 0,
    val text2: Long = 0,
    val text3: Long = 0,
    val border0: Long = 0,
    val border1: Long = 0,
    val border2: Long = 0,
    val ink: Long = 0,
    val busy: Long = 0,
    val waiting: Long = 0,
    val ok: Long = 0,
    val error: Long = 0,
    val diffAdd: Long = 0,
    val diffDel: Long = 0,
    val termBg: Long = 0,
    val termFg: Long = 0,
    val cursor: Long = 0,
    val ansi: List<Long> = emptyList(),
)

@Serializable
data class TermFrame(
    val thread: Long = 0,
    val cols: Int = 0,
    val rows: Int = 0,
    val reset: Boolean = false,
    val drop: Int = 0,
    val len: Int = 0,
    /** Each [index, spans]. */
    val lines: List<JsonArray> = emptyList(),
    /** [line, column] when the cursor shows. */
    val cursor: List<Int>? = null,
    val fit: Boolean = false,
    val paste: Boolean = false,
) {
    fun changed(): List<Pair<Int, List<Span>>> = lines.map { a ->
        a[0].jsonPrimitive.int to wire.decodeFromJsonElement(ListSpans, a[1])
    }

    private companion object {
        val ListSpans = kotlinx.serialization.builtins.ListSerializer(Span.serializer())
    }
}

@Serializable
data class Span(val t: String = "", val fg: Long? = null, val bg: Long? = null, val s: Int = 0) {
    companion object {
        const val RGB = 1L shl 24
        const val BOLD = 1
        const val ITALIC = 2
        const val UNDERLINE = 4
        const val INVERSE = 8
        const val DIM = 16
        const val STRIKE = 32
        const val HIDDEN = 64
    }
}

// ---- a structured thread's journal ----

@Serializable
sealed interface Entry {
    @Serializable @SerialName("prompt")
    data class PromptEntry(val prompt: Prompt) : Entry

    @Serializable @SerialName("answer")
    data class AnswerEntry(val request: String, val answer: Answer) : Entry

    @Serializable @SerialName("run")
    data class Run(val event: RunEvent) : Entry
}

@Serializable
data class Prompt(val text: String = "", val images: List<String> = emptyList())

@Serializable
sealed interface RunEvent {
    @Serializable @SerialName("started")
    data class Started(val agent: Agent, val model: String, val thread: String, val cwd: String) : RunEvent

    @Serializable @SerialName("text")
    data class Text(val text: String) : RunEvent

    @Serializable @SerialName("thinking")
    data class Thinking(val text: String) : RunEvent

    @Serializable @SerialName("tool")
    data class ToolCall(val id: String, val tool: Tool) : RunEvent

    @Serializable @SerialName("toolDone")
    data class ToolDone(val id: String, val ok: Boolean, val output: String) : RunEvent

    @Serializable @SerialName("subagentTool")
    data class SubagentTool(val parent: String, val id: String, val tool: Tool) : RunEvent

    @Serializable @SerialName("subagentToolDone")
    data class SubagentToolDone(val parent: String, val id: String, val ok: Boolean, val output: String) : RunEvent

    @Serializable @SerialName("subagentText")
    data class SubagentText(val parent: String, val text: String) : RunEvent

    @Serializable @SerialName("woke")
    data object Woke : RunEvent

    @Serializable @SerialName("approval")
    data class Approval(val request: String, val tool: Tool, val reason: String? = null, val always: Boolean) : RunEvent

    @Serializable @SerialName("steered")
    data object Steered : RunEvent

    @Serializable @SerialName("usage")
    data class Usage(val input: Long, val output: Long) : RunEvent

    @Serializable @SerialName("context")
    data class Context(val used: Long, val window: Long) : RunEvent

    @Serializable @SerialName("error")
    data class Error(val message: String) : RunEvent

    @Serializable @SerialName("finished")
    data class Finished(val status: RunStatus, val ms: Long, val text: String = "", val error: String? = null) : RunEvent

    @Serializable @SerialName("failed")
    data class Failed(val message: String) : RunEvent
}

@Serializable
@JsonClassDiscriminator("kind")
sealed interface Tool {
    @Serializable @SerialName("command")
    data class Command(val command: String) : Tool

    @Serializable @SerialName("read")
    data class Read(val path: String) : Tool

    @Serializable @SerialName("edit")
    data class Edit(val changes: List<FileChange>) : Tool

    @Serializable @SerialName("search")
    data class Search(val pattern: String, val path: String? = null) : Tool

    @Serializable @SerialName("web")
    data class Web(val target: String) : Tool

    @Serializable @SerialName("mcp")
    data class Mcp(val server: String, val tool: String, val input: String) : Tool

    @Serializable @SerialName("agent")
    data class AgentCall(
        val description: String,
        @SerialName("agent_type") val agentType: String,
        val prompt: String,
    ) : Tool

    @Serializable @SerialName("other")
    data class Other(val name: String, val input: String) : Tool
}

@Serializable
data class FileChange(val path: String, val kind: ChangeKind, val diff: String)
