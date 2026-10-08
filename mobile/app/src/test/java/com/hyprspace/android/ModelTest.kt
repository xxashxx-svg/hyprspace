package com.hyprspace.android

import com.hyprspace.android.model.Item
import com.hyprspace.android.model.TermBuffer
import com.hyprspace.android.model.Transcript
import com.hyprspace.android.model.label
import com.hyprspace.android.model.short
import com.hyprspace.android.model.summary
import com.hyprspace.android.model.withoutCd
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Entry
import com.hyprspace.android.net.PairLink
import com.hyprspace.android.net.Prompt
import com.hyprspace.android.net.RunEvent
import com.hyprspace.android.net.RunStatus
import com.hyprspace.android.net.TermFrame
import com.hyprspace.android.net.Tool
import com.hyprspace.android.ui.initials
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonArray
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ModelTest {
    private fun line(i: Int, text: String): JsonArray = buildJsonArray {
        add(JsonPrimitive(i))
        add(buildJsonArray { add(buildJsonObject { put("t", text) }) })
    }

    private fun text(b: com.hyprspace.android.model.TermView) = b.lines.map { l -> l.joinToString("") { it.t } }

    @Test
    fun framesBuildTheSameLinesTheDesktopHas() {
        val b = TermBuffer()
        b.apply(TermFrame(len = 3, reset = true, lines = listOf(line(0, "a"), line(1, "b"), line(2, "c"))))
        // two lines fell off a full history, one came in at the bottom
        val v = b.apply(TermFrame(drop = 2, len = 2, lines = listOf(line(1, "d"))))
        assertEquals(listOf("c", "d"), text(v))
        val again = b.apply(TermFrame(reset = true, len = 1, lines = listOf(line(0, "x")), cursor = listOf(0, 1)))
        assertEquals(listOf("x"), text(again))
        assertEquals(0 to 1, again.cursor)
    }

    @Test
    fun aRunReadsLikeTheDesktopTranscript() {
        val t = Transcript()
        fun run(e: RunEvent) = t.add(Entry.Run(e))
        t.add(Entry.PromptEntry(Prompt("go")))
        run(RunEvent.Text("Hel"))
        run(RunEvent.Text("lo"))
        run(RunEvent.ToolCall("1", Tool.Command("ls")))
        run(RunEvent.ToolDone("1", true, "a"))
        run(RunEvent.Approval("r", Tool.Read("x"), null, false))
        t.add(Entry.AnswerEntry("r", Answer.Deny))
        run(RunEvent.Approval("r2", Tool.Read("y"), null, false))
        run(RunEvent.Error("boom"))
        run(RunEvent.Finished(RunStatus.Failed, 10, "", "boom"))
        val items = t.items()
        assertEquals("Hello", (items[1] as Item.Reply).text)
        assertEquals(true, (items[2] as Item.Call).done?.ok)
        assertEquals(Answer.Deny, (items[3] as Item.Approval).answer)
        // an approval nobody answered before the run ended can't be answered now
        assertTrue((items[4] as Item.Approval).expired)
        // the error already showed, so the end of the run doesn't repeat it
        assertNull((items.last() as Item.Finished).error)
    }

    @Test
    fun aReplyOnlyInTheFinishShowsOnce() {
        val t = Transcript()
        t.add(Entry.PromptEntry(Prompt("go")))
        t.add(Entry.Run(RunEvent.Finished(RunStatus.Done, 5, "All done", null)))
        assertEquals("All done", (t.items()[1] as Item.Reply).text)
    }

    @Test
    fun toolsReadTheWayTheDesktopWritesThem() {
        assertEquals("git status", withoutCd("cd \"C:\\w x\" && git status"))
        assertEquals("npm test", withoutCd("cd /w; npm test"))
        assertEquals("cd /w", withoutCd("cd /w"))
        assertEquals("auth/login.ts", short("C:\\app\\src\\auth\\login.ts"))
        assertEquals("Run npm test", label(Tool.Command("cd /w && npm test")))
        assertEquals("Ran 2 commands · read 1 file", summary(listOf(Tool.Command("a"), Tool.Command("b"), Tool.Read("c"))))
    }

    @Test
    fun pairingLinksAndTypedCodes() {
        val l = PairLink.parse("hyprspace://pair?n=Ash%20PC&h=192.168.1.4,100.64.1.2&p=47821&f=abc-_d&c=s3cret")!!
        assertEquals("Ash PC", l.name)
        assertEquals(listOf("192.168.1.4", "100.64.1.2"), l.hosts)
        assertEquals("abc-_d", l.fingerprint)
        assertNull(PairLink.parse("https://example.com"))
        assertNull(PairLink.parse("hyprspace://pair?h=1.2.3.4&p=0&c=x"))
        val typed = PairLink.typed("192.168.1.4:47000", "K7MX-Q2RT")!!
        assertEquals(47000, typed.port)
        assertNull(typed.fingerprint)
        assertEquals(47821, PairLink.typed("192.168.1.4", "K7MX-Q2RT")!!.port)
    }

    @Test
    fun spaceTagsTakeTheDesktopsInitials() {
        assertEquals("HP", initials("hot-potato-game"))
        assertEquals("E2", initials("epicmaster2"))
        assertEquals("CE", initials(".claude"))
        assertEquals("V2", initials("vitanova279"))
    }
}
