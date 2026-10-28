// An agent's reply as markdown: paragraphs, headings, lists, quotes, code blocks and tables,
// with inline code, emphasis and links. Parsed with commonmark, drawn with the desktop's fonts.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.hyprspace.android.ui.Hues
import com.hyprspace.android.ui.LocalHues
import com.hyprspace.android.ui.Mono
import org.commonmark.ext.autolink.AutolinkExtension
import org.commonmark.ext.gfm.strikethrough.Strikethrough
import org.commonmark.ext.gfm.strikethrough.StrikethroughExtension
import org.commonmark.ext.gfm.tables.TableBlock
import org.commonmark.ext.gfm.tables.TableBody
import org.commonmark.ext.gfm.tables.TableCell
import org.commonmark.ext.gfm.tables.TableHead
import org.commonmark.ext.gfm.tables.TableRow
import org.commonmark.ext.gfm.tables.TablesExtension
import org.commonmark.node.BlockQuote
import org.commonmark.node.BulletList
import org.commonmark.node.Code
import org.commonmark.node.Emphasis
import org.commonmark.node.FencedCodeBlock
import org.commonmark.node.HardLineBreak
import org.commonmark.node.Heading
import org.commonmark.node.HtmlBlock
import org.commonmark.node.HtmlInline
import org.commonmark.node.Image
import org.commonmark.node.IndentedCodeBlock
import org.commonmark.node.Link
import org.commonmark.node.ListItem
import org.commonmark.node.Node
import org.commonmark.node.OrderedList
import org.commonmark.node.Paragraph
import org.commonmark.node.SoftLineBreak
import org.commonmark.node.StrongEmphasis
import org.commonmark.node.ThematicBreak
import org.commonmark.parser.Parser

private val parser: Parser = Parser.builder()
    .extensions(listOf(TablesExtension.create(), StrikethroughExtension.create(), AutolinkExtension.create()))
    .build()

@Composable
fun Markdown(text: String, modifier: Modifier = Modifier) {
    val parts = remember(text) { stretches(text) }
    SelectionContainer(modifier) {
        Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
            for (p in parts) Stretch(p)
        }
    }
}

// a layer each, so a stretch that stopped changing keeps its drawing while the last one grows
@Composable
private fun Stretch(text: String) {
    val doc = remember(text) { parser.parse(text) }
    Column(Modifier.graphicsLayer(), verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Blocks(doc)
    }
}

/**
 * [text] cut at blank lines outside code fences, so a reply that grows parses and lays out only
 * its last stretch. A line indented after a blank one still belongs to the list item above it.
 */
internal fun stretches(text: String): List<String> {
    val out = ArrayList<String>()
    val cur = StringBuilder()
    var fence: String? = null
    var gap = false
    for (line in text.split('\n')) {
        val lead = line.trimStart()
        if (fence != null) {
            cur.append(line).append('\n')
            if (lead.startsWith(fence) && lead.trimEnd().all { it == fence!![0] }) fence = null
            continue
        }
        if (line.isBlank()) {
            gap = true
            continue
        }
        if (gap && cur.isNotEmpty()) {
            if (line[0] == ' ' || line[0] == '\t') {
                cur.append('\n')
            } else {
                out += cur.toString()
                cur.clear()
            }
        }
        gap = false
        val tick = lead.takeWhile { it == '`' || it == '~' }
        if (tick.length >= 3 && tick.all { it == tick[0] }) fence = tick
        cur.append(line).append('\n')
    }
    if (cur.isNotEmpty()) out += cur.toString()
    return out
}

private fun Node.children(): List<Node> = generateSequence(firstChild) { it.next }.toList()

@Composable
private fun Blocks(parent: Node) {
    for (n in parent.children()) Block(n)
}

@Composable
private fun Block(n: Node) {
    val h = LocalHues.current
    val body = MaterialTheme.typography.bodyLarge
    when (n) {
        is Paragraph -> {
            val kids = n.children()
            val pics = kids.filterIsInstance<Image>()
            val words = kids.any { it !is Image && it !is SoftLineBreak && it !is HardLineBreak && !(it is org.commonmark.node.Text && it.literal.isBlank()) }
            if (words || pics.isEmpty()) Text(inline(n, h), style = body, color = h.text1)
            for (p in pics) Media(p.destination, inline(p, h).text)
        }
        is Heading -> {
            val size = when (n.level) { 1 -> 21; 2 -> 19; 3 -> 17; else -> 16 }
            Text(
                inline(n, h),
                style = body.copy(fontSize = size.sp, lineHeight = (size * 1.35f).sp),
                fontWeight = FontWeight.SemiBold,
                color = h.text1,
                modifier = Modifier.padding(top = 6.dp),
            )
        }
        is BulletList, is OrderedList -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val first = (n as? OrderedList)?.markerStartNumber ?: 1
            val items = n.children()
            // wide enough for the longest number, so "10." never wraps its dot
            val gutter = if (n is OrderedList) (12 + 9 * "${first + items.size - 1}".length).dp else 18.dp
            items.forEachIndexed { i, item ->
                Row {
                    Text(
                        if (n is OrderedList) "${first + i}." else "•",
                        Modifier.width(gutter),
                        style = body,
                        fontWeight = if (n is OrderedList) FontWeight.Normal else FontWeight.Bold,
                        color = if (n is OrderedList) h.text2 else h.text1,
                        softWrap = false,
                    )
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        for (c in item.children()) Block(c)
                    }
                }
            }
        }
        is ListItem -> Blocks(n)
        is BlockQuote -> Row(Modifier.height(IntrinsicSize.Min)) {
            Box(Modifier.width(3.dp).fillMaxHeight().clip(RoundedCornerShape(2.dp)).background(h.border2))
            Column(Modifier.padding(start = 12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) { Blocks(n) }
        }
        is FencedCodeBlock -> CodeBlock(n.literal.trimEnd('\n'), n.info)
        is IndentedCodeBlock -> CodeBlock(n.literal.trimEnd('\n'), "")
        is ThematicBreak -> HorizontalDivider(color = h.border1)
        is TableBlock -> Table(n)
        is HtmlBlock -> Text(n.literal.trim(), style = body, color = h.text2)
        else -> Blocks(n)
    }
}

@Composable
fun CodeBlock(code: String, lang: String?) {
    val h = LocalHues.current
    Column(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(h.ink(0.045f)),
    ) {
        lang?.takeIf { it.isNotBlank() }?.let {
            Text(
                it.substringBefore(' '),
                Modifier.padding(start = 14.dp, top = 9.dp),
                fontSize = 11.sp,
                color = h.text3,
            )
        }
        Text(
            code,
            Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 14.dp, vertical = 10.dp),
            fontFamily = Mono,
            fontSize = 13.sp,
            lineHeight = 19.sp,
            color = h.text1,
            softWrap = false,
        )
    }
}

@Composable
private fun Table(t: TableBlock) {
    val h = LocalHues.current
    val rows = t.children().flatMap { part ->
        when (part) {
            is TableHead, is TableBody -> part.children().filterIsInstance<TableRow>().map { Pair(part is TableHead, it) }
            else -> emptyList()
        }
    }
    Column(
        Modifier
            .clip(RoundedCornerShape(10.dp))
            .background(h.ink(0.035f))
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 4.dp, vertical = 2.dp),
    ) {
        rows.forEachIndexed { i, (head, row) ->
            if (i > 0) HorizontalDivider(Modifier.width(140.dp * row.children().count { it is TableCell }), color = h.border1)
            Row {
                for (cell in row.children().filterIsInstance<TableCell>()) {
                    Text(
                        inline(cell, h),
                        Modifier.width(140.dp).padding(horizontal = 10.dp, vertical = 7.dp),
                        style = if (head) MaterialTheme.typography.bodySmall else MaterialTheme.typography.bodyMedium,
                        fontWeight = if (head) FontWeight.Medium else FontWeight.Normal,
                        color = if (head) h.text2 else h.text1,
                    )
                }
            }
        }
    }
}

/** A block's inline content as styled text. */
private fun inline(n: Node, h: Hues): AnnotatedString = buildAnnotatedString {
    fun walk(node: Node) {
        for (c in node.children()) when (c) {
            is org.commonmark.node.Text -> append(c.literal)
            is Code -> withStyle(SpanStyle(fontFamily = Mono, fontSize = 14.sp, color = h.text2, background = h.ink(0.06f))) {
                append(" ${c.literal} ")
            }
            is Emphasis -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { walk(c) }
            is StrongEmphasis -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { walk(c) }
            is Strikethrough -> withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { walk(c) }
            is Link -> withLink(
                LinkAnnotation.Url(c.destination, TextLinkStyles(SpanStyle(color = h.link, textDecoration = TextDecoration.Underline))),
            ) { walk(c) }
            is SoftLineBreak -> append(' ')
            is HardLineBreak -> append('\n')
            is HtmlInline -> append(c.literal)
            else -> walk(c)
        }
    }
    walk(n)
}
