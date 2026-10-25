package com.hyprspace.android

import com.hyprspace.android.model.Item
import com.hyprspace.android.model.Step
import com.hyprspace.android.model.Transcript
import com.hyprspace.android.model.doing
import com.hyprspace.android.model.plan
import com.hyprspace.android.model.questions
import com.hyprspace.android.model.said
import com.hyprspace.android.model.todos
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Entry
import com.hyprspace.android.net.Prompt
import com.hyprspace.android.net.RunEvent
import com.hyprspace.android.net.Tool
import com.hyprspace.android.ui.thread.saysUltrathink
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AsksTest {
    private val ask = """{"questions":[{"header":"Stack","question":"Which one?","multiSelect":false,
        "options":[{"label":"Rust","description":"fast"},{"label":"Go","description":""}]}]}"""

    @Test
    fun aQuestionReadsAsTheDesktopReadsIt() {
        val q = questions(ask).single()
        assertEquals("Which one?", q.text)
        assertEquals(listOf("Rust" to "fast", "Go" to ""), q.options)
        assertEquals("Go", said(q, setOf(1), ""))
        // Other is one past the last option, and counts only once something is typed
        assertEquals("", said(q, setOf(2), " "))
        assertEquals("Rust, Zig", said(q, setOf(0, 2), "Zig"))
        assertTrue(questions("not json").isEmpty())
    }

    @Test
    fun plansAndTaskListsRead() {
        assertEquals("# Do it", plan("""{"plan":"# Do it"}"""))
        val list = todos("""{"todos":[{"content":"a","status":"completed"},{"content":"b","status":"in_progress"},{"content":"c"}]}""")
        assertEquals(listOf(Step.Done, Step.Doing, Step.Pending), list.map { it.step })
    }

    @Test
    fun theStatusSaysWhatTheRunIsOn() {
        val t = Transcript()
        t.add(Entry.PromptEntry(Prompt("go")))
        assertEquals("Working", doing(t.items(), waiting = false, stopping = false, compacting = false))
        t.add(Entry.Run(RunEvent.Thinking("hm")))
        assertEquals("Thinking", doing(t.items(), false, false, false))
        t.add(Entry.Run(RunEvent.Text("Sure")))
        assertEquals("Writing", doing(t.items(), false, false, false))
        t.add(Entry.Run(RunEvent.ToolCall("1", Tool.Command("cargo test"))))
        assertEquals("Run cargo test", doing(t.items(), false, false, false))
        assertEquals("Waiting for you", doing(t.items(), true, false, false))
        assertEquals("Stopping", doing(t.items(), true, true, false))
    }

    @Test
    fun anAnswerKeepsWhatWasPicked() {
        val t = Transcript()
        t.add(Entry.PromptEntry(Prompt("go")))
        t.add(Entry.Run(RunEvent.Approval("r", Tool.Other("AskUserQuestion", ask), always = false)))
        t.add(Entry.AnswerEntry("r", Answer.Allow, listOf(listOf("Which one?", "Go"))))
        val a = t.items().filterIsInstance<Item.Approval>().single()
        assertEquals(Answer.Allow, a.answer)
        assertEquals(listOf("Which one?" to "Go"), a.given)
    }

    @Test
    fun aPromptThatStartedANewProcessEndsTheRunCutOffBeforeIt() {
        val started = Entry.Run(RunEvent.Started(com.hyprspace.android.net.Agent.Claude, "m", "t", "/w"))
        val t = Transcript()
        listOf(Entry.PromptEntry(Prompt("one")), started, Entry.Run(RunEvent.Text("working")), Entry.PromptEntry(Prompt("two")), started)
            .forEach { t.add(it, replay = true) }
        val items = t.items()
        assertTrue(items[items.size - 2] is Item.Note)
        assertEquals(Item.User(items.last().key, "two", 0, steer = false), items.last())

        // the prompt after a cut-off run can arrive live
        val l = Transcript()
        listOf(Entry.PromptEntry(Prompt("one")), started, Entry.Run(RunEvent.Text("working"))).forEach { l.add(it, replay = true) }
        l.add(Entry.PromptEntry(Prompt("two")))
        l.add(started)
        assertTrue(l.items()[l.items().size - 2] is Item.Note)
        assertFalse((l.items().last() as Item.User).steer)

        // a real steer reaches the process already running, which never says it started again
        val s = Transcript()
        listOf(Entry.PromptEntry(Prompt("one")), started, Entry.Run(RunEvent.Text("working")), Entry.PromptEntry(Prompt("also")))
            .forEach { s.add(it, replay = true) }
        assertTrue((s.items().last() as Item.User).steer)
    }

    @Test
    fun ultrathinkCountsOnlyAsAWholeWord() {
        assertTrue(saysUltrathink("ultrathink"))
        assertTrue(saysUltrathink("Please ULTRATHINK about this."))
        assertTrue(saysUltrathink("fix it, ultrathink"))
        assertFalse(saysUltrathink("ultrathinking"))
        assertFalse(saysUltrathink("preultrathink"))
    }
}
