package com.hyprspace.android

import com.hyprspace.android.model.Item
import com.hyprspace.android.model.Step
import com.hyprspace.android.model.Transcript
import com.hyprspace.android.model.doing
import com.hyprspace.android.model.folds
import com.hyprspace.android.model.plan
import com.hyprspace.android.model.questions
import com.hyprspace.android.model.said
import com.hyprspace.android.model.todos
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Entry
import com.hyprspace.android.net.Prompt
import com.hyprspace.android.net.RunEvent
import com.hyprspace.android.net.RunStatus
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
    fun theStatusLeavesTheLiveStepToTheRowAboveIt() {
        assertEquals("Working", doing(waiting = false, stopping = false, compacting = false))
        assertEquals("Waiting for you", doing(true, false, false))
        assertEquals("Stopping", doing(true, true, false))
        assertEquals("Compacting", doing(false, false, true))
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
        assertEquals(Item.User(items.last().key, "two", 0, steer = false, at = 2), items.last())

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
    fun keysAgreeBetweenALiveJournalAndALaterTail() {
        val live = Transcript()
        live.add(Entry.PromptEntry(Prompt("one")))
        for (piece in listOf("a", "b", "c")) live.add(Entry.Run(RunEvent.Text(piece)))
        live.add(Entry.PromptEntry(Prompt("two")))
        live.add(Entry.Run(RunEvent.Thinking("hm")))
        live.add(Entry.Run(RunEvent.Text("done")))

        // the journal file joins a reply's pieces, and a tail leaves out the first prompt
        val tail = Transcript()
        tail.reset(before = 1)
        listOf(Entry.PromptEntry(Prompt("two")), Entry.Run(RunEvent.Thinking("hm")), Entry.Run(RunEvent.Text("done")))
            .forEach { tail.add(it, replay = true) }

        assertEquals(live.items().drop(2).map { it.key }, tail.items().map { it.key })
        assertEquals(2, (tail.items().first() as Item.User).at)
        assertEquals(live.prompts, tail.prompts)
    }

    @Test
    fun anExportReadsAsMarkdown() {
        val t = Transcript()
        t.add(Entry.PromptEntry(Prompt("fix it")))
        t.add(Entry.Run(RunEvent.ToolCall("1", Tool.Command("cargo test"))))
        t.add(Entry.Run(RunEvent.Text("Fixed.")))
        assertEquals("## You\n\nfix it\n\n- Run cargo test\n\nFixed.\n", com.hyprspace.android.model.exported(t.items()))
    }

    @Test
    fun aSettledRunFoldsItsWorkAboveTheReply() {
        val done = Entry.Run(RunEvent.Finished(RunStatus.Done, 21_000, "", null))
        val t = Transcript()
        listOf(
            Entry.PromptEntry(Prompt("go")),
            Entry.Run(RunEvent.Thinking("hm")),
            Entry.Run(RunEvent.Text("checking")),
            Entry.Run(RunEvent.ToolCall("1", Tool.Command("ls"))),
            Entry.PromptEntry(Prompt("also")),
            Entry.Run(RunEvent.ToolCall("2", Tool.Command("pwd"))),
            Entry.Run(RunEvent.Text("done")),
            done,
        ).forEach { t.add(it, replay = true) }
        val items = t.items()
        val f = folds(items)
        assertEquals(listOf(items[0].key to 21_000L, items[4].key to null), f.map { it.prompt to it.ms })
        assertEquals(setOf(items[1].key, items[2].key, items[3].key), f[0].hide)
        assertEquals(setOf(items[5].key), f[1].hide)

        val live = Transcript()
        listOf(Entry.PromptEntry(Prompt("go")), Entry.Run(RunEvent.ToolCall("1", Tool.Command("ls")))).forEach { live.add(it) }
        assertTrue(folds(live.items()).isEmpty())
        val stopped = Transcript()
        listOf(
            Entry.PromptEntry(Prompt("go")),
            Entry.Run(RunEvent.ToolCall("1", Tool.Command("ls"))),
            Entry.Run(RunEvent.Finished(RunStatus.Interrupted, 5, "", null)),
        ).forEach { stopped.add(it) }
        assertTrue(folds(stopped.items()).isEmpty())

        val woken = Transcript()
        listOf(
            Entry.PromptEntry(Prompt("scan")),
            Entry.Run(RunEvent.Text("started")),
            Entry.Run(RunEvent.Finished(RunStatus.Done, 5, "", null)),
            Entry.Run(RunEvent.Woke),
            Entry.Run(RunEvent.ToolCall("1", Tool.Command("ls"))),
            Entry.Run(RunEvent.Text("done")),
            Entry.Run(RunEvent.Finished(RunStatus.Done, 28_000, "", null)),
        ).forEach { woken.add(it, replay = true) }
        val w = woken.items()
        assertEquals(listOf(Triple(w[3].key, w[3].key, 28_000L)), folds(w).map { Triple(it.prompt, it.first, it.ms) })
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
