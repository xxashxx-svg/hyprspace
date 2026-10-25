// Claude's questions, plans and task lists as cards, the way the desktop draws them
// (crates/ui/src/transcript/ask.rs): pick answers or type your own, approve a plan or keep
// planning, follow the task list.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import com.hyprspace.android.R
import com.hyprspace.android.model.Item
import com.hyprspace.android.model.Step
import com.hyprspace.android.model.Todo
import com.hyprspace.android.model.plan
import com.hyprspace.android.model.questions
import com.hyprspace.android.model.said
import com.hyprspace.android.model.todos
import com.hyprspace.android.net.Answer
import com.hyprspace.android.net.Tool
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.clickableQuiet

typealias Answers = (String, Answer, List<Pair<String, String>>) -> Unit

private fun input(tool: Tool) = (tool as? Tool.Other)?.input.orEmpty()

@Composable
private fun Card(pending: Boolean, content: @Composable () -> Unit) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(14.dp))
            .background(h.ink(0.035f))
            .then(if (pending) Modifier.border(1.dp, h.border1, RoundedCornerShape(14.dp)) else Modifier)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) { content() }
}

@Composable
private fun Heading(icon: Int, title: String, state: String?, modifier: Modifier = Modifier) {
    val h = LocalHues.current
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(painterResource(icon), null, Modifier.size(15.dp), tint = h.text2)
        Text(title, Modifier.weight(1f), style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = h.text1)
        state?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = h.text3) }
    }
}

@Composable
fun QuestionCard(a: Item.Approval, canAnswer: Boolean, onAnswer: Answers) {
    val h = LocalHues.current
    val qs = remember(a.request) { questions(input(a.tool)) }
    val pending = a.answer == null && !a.expired
    val title = if (qs.size <= 1) "Claude asked a question" else "Claude asked ${qs.size} questions"
    if (!pending) {
        val state = when {
            a.answer == Answer.Deny && a.given.isNotEmpty() -> "Chose to talk it over"
            a.answer == Answer.Deny -> "Skipped"
            a.answer != null -> "Answered"
            else -> "Not answered"
        }
        Card(false) {
            Heading(R.drawable.ic_circle_help, title, state)
            for (q in qs) {
                val given = a.given.firstOrNull { it.first == q.text }?.second.orEmpty()
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Text(q.text, style = MaterialTheme.typography.bodySmall, color = h.text3)
                    if (given.isEmpty()) {
                        Text("No answer", style = MaterialTheme.typography.bodyMedium, color = h.text3)
                    } else {
                        Row(horizontalArrangement = Arrangement.spacedBy(7.dp)) {
                            Icon(painterResource(R.drawable.ic_check), null, Modifier.size(14.dp).padding(top = 3.dp), tint = h.accent)
                            Text(given, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = h.text1)
                        }
                    }
                }
            }
        }
        return
    }
    val picks = remember(a.request) { mutableStateListOf(*Array(qs.size) { emptySet<Int>() }) }
    val others = remember(a.request) { mutableStateListOf(*Array(qs.size) { "" }) }
    val answers = qs.indices.map { qs[it].text to said(qs[it], picks[it], others[it]) }
    val ready = answers.all { it.second.isNotEmpty() }
    Card(true) {
        Heading(R.drawable.ic_circle_help, title, null)
        qs.forEachIndexed { i, q ->
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                if (q.header.isNotBlank()) Text(q.header.uppercase(), style = MaterialTheme.typography.labelSmall, color = h.text3)
                Text(q.text, style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.Medium, color = h.text1)
                val pick: (Int) -> Unit = { o ->
                    picks[i] = when {
                        !q.multi -> setOf(o)
                        o in picks[i] -> picks[i] - o
                        else -> picks[i] + o
                    }
                }
                q.options.forEachIndexed { o, (label, about) ->
                    Option(label, about, o in picks[i], q.multi, canAnswer) { pick(o) }
                }
                val other = q.options.size
                Option("Other", "Type your own answer", other in picks[i], q.multi, canAnswer) { pick(other) }
                if (other in picks[i]) {
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .clip(RoundedCornerShape(10.dp))
                            .background(h.ink(0.05f))
                            .padding(horizontal = 12.dp, vertical = 10.dp),
                    ) {
                        if (others[i].isEmpty()) Text("Your answer", style = MaterialTheme.typography.bodyMedium, color = h.text3)
                        BasicTextField(
                            value = others[i],
                            onValueChange = { others[i] = it },
                            modifier = Modifier.fillMaxWidth().heightIn(max = 120.dp),
                            textStyle = MaterialTheme.typography.bodyMedium.copy(color = h.text1),
                            cursorBrush = SolidColor(h.accent),
                        )
                    }
                }
            }
        }
        if (canAnswer) {
            Choice("Answer", Modifier.fillMaxWidth(), primary = true, enabled = ready) {
                onAnswer(a.request, Answer.Allow, answers)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Choice("Skip", Modifier.weight(1f)) { onAnswer(a.request, Answer.Deny, emptyList()) }
                // turned down with the answers so far, which Claude reads as a wish to talk it over
                Choice("Chat about this", Modifier.weight(1f)) { onAnswer(a.request, Answer.Deny, answers) }
            }
        } else {
            Text("Answer it on the computer.", style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
    }
}

@Composable
private fun Option(label: String, about: String, on: Boolean, multi: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val h = LocalHues.current
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(if (on) h.accent.copy(alpha = 0.12f) else h.ink(0.03f))
            .border(1.dp, if (on) h.accent.copy(alpha = 0.5f) else h.border0, RoundedCornerShape(10.dp))
            .then(if (enabled) Modifier.clickableQuiet(onClick) else Modifier)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val shape = if (multi) RoundedCornerShape(4.dp) else CircleShape
        Box(
            Modifier
                .padding(top = 3.dp)
                .size(14.dp)
                .clip(shape)
                .border(1.5.dp, if (on) h.accent else h.text3, shape)
                .background(if (on) h.accent else androidx.compose.ui.graphics.Color.Transparent),
            contentAlignment = Alignment.Center,
        ) {
            if (on) Icon(painterResource(R.drawable.ic_check), null, Modifier.size(10.dp), tint = h.onAccent)
        }
        Column(Modifier.weight(1f)) {
            Text(label, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Medium, color = h.text1)
            if (about.isNotBlank()) Text(about, style = MaterialTheme.typography.bodySmall, color = h.text3)
        }
    }
}

@Composable
fun PlanCard(a: Item.Approval, canAnswer: Boolean, onAnswer: Answers) {
    val h = LocalHues.current
    val pending = a.answer == null && !a.expired
    val state = when {
        a.answer == Answer.Deny -> "Kept planning"
        a.answer != null -> "Approved"
        a.expired -> "Not answered"
        else -> null
    }
    var open by rememberSaveable(a.request) { mutableStateOf(pending) }
    Card(pending) {
        Heading(R.drawable.ic_list_checks, "Plan", state, Modifier.clickableQuiet { open = !open })
        if (open || pending) {
            Box(
                Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .background(h.ink(0.035f))
                    .padding(horizontal = 14.dp, vertical = 10.dp),
            ) { Markdown(plan(input(a.tool))) }
        }
        if (pending) {
            if (canAnswer) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Choice("Keep planning", Modifier.weight(1f)) { onAnswer(a.request, Answer.Deny, emptyList()) }
                    Choice("Approve plan", Modifier.weight(1f), primary = true) { onAnswer(a.request, Answer.Allow, emptyList()) }
                }
            } else {
                Text("Answer it on the computer.", style = MaterialTheme.typography.bodySmall, color = h.text3)
            }
        }
    }
}

@Composable
fun TodoCard(tool: Tool) {
    val list = remember(tool) { todos(input(tool)) }
    if (list.isEmpty()) return
    val done = list.count { it.step == Step.Done }
    Card(false) {
        Heading(R.drawable.ic_list_checks, "Tasks", "$done of ${list.size} done")
        Column(verticalArrangement = Arrangement.spacedBy(7.dp)) {
            for (t in list) TodoRow(t)
        }
    }
}

@Composable
private fun TodoRow(t: Todo) {
    val h = LocalHues.current
    val (icon, tint) = when (t.step) {
        Step.Done -> R.drawable.ic_circle_check to h.ok
        Step.Doing -> R.drawable.ic_circle_play to h.busy
        Step.Pending -> R.drawable.ic_square to h.text3
    }
    Row(horizontalArrangement = Arrangement.spacedBy(9.dp)) {
        Icon(painterResource(icon), null, Modifier.size(14.dp).padding(top = 3.dp), tint = tint)
        Text(
            t.text,
            style = MaterialTheme.typography.bodyMedium,
            color = when (t.step) {
                Step.Done -> h.text3
                Step.Doing -> h.text1
                Step.Pending -> h.text2
            },
            textDecoration = if (t.step == Step.Done) TextDecoration.LineThrough else null,
        )
    }
}
