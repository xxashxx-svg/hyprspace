// What Claude's question, plan and task list calls carry, read from their input the way the
// desktop reads it (crates/ui/src/transcript/ask.rs), and what a live run is doing in a few words
// (crates/ui/src/transcript/doing.rs).

package com.hyprspace.android.model

import com.hyprspace.android.net.Tool
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull

const val QUESTION = "AskUserQuestion"
const val PLAN = "ExitPlanMode"
const val TODO = "TodoWrite"

/** The name of a question, plan or task list call, or null for any other. */
fun special(tool: Tool): String? = (tool as? Tool.Other)?.name?.takeIf { it == QUESTION || it == PLAN || it == TODO }

data class Question(val header: String, val text: String, val options: List<Pair<String, String>>, val multi: Boolean)

enum class Step { Pending, Doing, Done }

data class Todo(val text: String, val step: Step)

private fun obj(input: String): JsonObject? = runCatching { Json.parseToJsonElement(input) as? JsonObject }.getOrNull()

private fun JsonElement?.str(): String = ((this as? JsonPrimitive)?.contentOrNull).orEmpty()

private fun JsonElement?.list(): List<JsonObject> = (this as? JsonArray)?.mapNotNull { it as? JsonObject }.orEmpty()

fun questions(input: String): List<Question> = obj(input)?.get("questions").list().map { q ->
    Question(
        header = q["header"].str(),
        text = q["question"].str(),
        options = q["options"].list().map { it["label"].str() to it["description"].str() },
        multi = (q["multiSelect"] as? JsonPrimitive)?.booleanOrNull ?: false,
    )
}

fun plan(input: String): String = obj(input)?.get("plan").str()

fun todos(input: String): List<Todo> = obj(input)?.get("todos").list().map { t ->
    Todo(
        text = t["content"].str(),
        step = when (t["status"].str()) {
            "completed" -> Step.Done
            "in_progress" -> Step.Doing
            else -> Step.Pending
        },
    )
}

/**
 * What one question's picks say: the labels picked, and the typed answer when Other, one past
 * the last option, is picked too.
 */
fun said(q: Question, picked: Set<Int>, other: String): String {
    val parts = picked.sorted().mapNotNull { q.options.getOrNull(it)?.first }.toMutableList()
    if (q.options.size in picked && other.isNotBlank()) parts += other.trim()
    return parts.joinToString(", ")
}

/** What a live run is doing now, as the desktop's status line says it. */
fun doing(waiting: Boolean, stopping: Boolean, compacting: Boolean): String = when {
    stopping -> "Stopping"
    compacting -> "Compacting"
    waiting -> "Waiting for you"
    else -> "Working"
}
