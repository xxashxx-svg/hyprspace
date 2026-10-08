package com.hyprspace.android

import com.hyprspace.android.net.Down
import com.hyprspace.android.net.Entry
import com.hyprspace.android.net.RunEvent
import com.hyprspace.android.net.Span
import com.hyprspace.android.net.Tool
import com.hyprspace.android.net.Up
import com.hyprspace.android.net.wire
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The lines in resources/wire are what the desktop writes and reads (crates/proto/src/phone.rs
 * keeps them current). The app has to read every one, and write the phone's own exactly the same.
 */
class WireTest {
    private fun lines(name: String): List<String> =
        javaClass.classLoader!!.getResource("wire/$name")!!.readText().lines().filter { it.isNotBlank() }

    @Test
    fun readsEveryMessageTheDesktopSends() {
        val all = lines("down.jsonl").map { wire.decodeFromString(Down.serializer(), it) }
        assertEquals(7, all.size)
        val board = (all[2] as Down.BoardMsg).board
        assertEquals("structured", wire.encodeToString(com.hyprspace.android.net.BoardKind.serializer(), board.threads[0].kind).trim('"'))
        assertEquals("opus" to "high", board.start.pick(com.hyprspace.android.net.Agent.Claude))
        assertEquals(0x161616FFL, board.theme.dark.bg)
        val entries = (all[3] as Down.Transcript).entries
        val tools = entries.filterIsInstance<Entry.Run>().map { it.event }.filterIsInstance<RunEvent.ToolCall>().map { it.tool }
        assertTrue(tools.any { it is Tool.AgentCall && it.agentType == "general-purpose" })
        assertEquals(6, tools.map { it::class }.toSet().size)
        val frame = (all[4] as Down.Term).frame
        val (index, spans) = frame.changed().single()
        assertEquals(1, index)
        assertEquals(Span.RGB or 0x102030L, spans[0].bg)
        assertEquals(Span.BOLD or Span.INVERSE, spans[0].s)
        assertEquals(listOf(1, 4), frame.cursor)
    }

    private fun bare(e: JsonElement): JsonElement = when (e) {
        is JsonObject -> JsonObject(e.filterValues { it !is JsonNull }.mapValues { bare(it.value) })
        is JsonArray -> JsonArray(e.map(::bare))
        else -> e
    }

    @Test
    fun writesWhatTheDesktopReads() {
        for (line in lines("up.jsonl")) {
            val up = wire.decodeFromString(Up.serializer(), line)
            val back = wire.encodeToString(Up.serializer(), up)
            // the app leaves out a null field, which the desktop reads as None all the same
            assertEquals(bare(wire.parseToJsonElement(line)), wire.parseToJsonElement(back))
        }
    }
}
