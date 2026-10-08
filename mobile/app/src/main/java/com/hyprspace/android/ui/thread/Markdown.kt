// An agent's reply as markdown: paragraphs, headings, lists, quotes, code blocks and tables,
// with inline code, emphasis and links. Parsed with commonmark, drawn with the desktop's fonts.

package com.hyprspace.android.ui.thread

import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
    .extensions(listOf(TablesExtension.create(), StrikethroughExtension.create()))
    .build()

@Composable
fun Markdown(text: String, modifier: Modifier = Modifier) {
    val doc = remember(text) { parser.parse(text) }
    SelectionContainer(modifier) {
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Blocks(doc)
        }
    }
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
        is Paragraph -> Text(inline(n, h), style = body, color = h.text1)
        is Heading -> Text(
            inline(n, h),
            style = body.copy(
                fontSize = when (n.level) { 1 -> 20.sp; 2 -> 18.sp; else -> 16.sp },
                lineHeight = 26.sp,
            ),
            fontWeight = FontWeight.SemiBold,
            color = h.text1,
            modifier = Modifier.padding(top = 4.dp),
        )
        is BulletList, is OrderedList -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            var i = (n as? OrderedList)?.markerStartNumber ?: 1
            for (item in n.children()) {
                val marker = if (n is OrderedList) "${i++}." else "•"
                Row {
                    Text(marker, Modifier.width(22.dp), style = body, color = h.text3)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
            .clip(RoundedCornerShape(10.dp))
            .background(h.ink(0.045f))
            .border(1.dp, h.border1, RoundedCornerShape(10.dp)),
    ) {
        lang?.takeIf { it.isNotBlank() }?.let {
            Text(
                it.substringBefore(' '),
                Modifier.padding(start = 12.dp, top = 8.dp),
                fontSize = 11.sp,
                color = h.text3,
            )
        }
        Text(
            code,
            Modifier.horizontalScroll(rememberScrollState()).padding(12.dp),
            fontFamily = Mono,
            fontSize = 12.5.sp,
            lineHeight = 18.sp,
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
            .horizontalScroll(rememberScrollState())
            .clip(RoundedCornerShape(8.dp))
            .border(1.dp, h.border1, RoundedCornerShape(8.dp)),
    ) {
        for ((head, row) in rows) {
            Row(Modifier.background(if (head) h.ink(0.04f) else h.bg)) {
                for (cell in row.children().filterIsInstance<TableCell>()) {
                    Text(
                        inline(cell, h),
                        Modifier.width(140.dp).padding(horizontal = 10.dp, vertical = 7.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = if (head) FontWeight.SemiBold else FontWeight.Normal,
                        color = h.text1,
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
            is Code -> withStyle(SpanStyle(fontFamily = Mono, fontSize = 13.sp, background = h.ink(0.07f))) {
                append(" ${c.literal} ")
            }
            is Emphasis -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { walk(c) }
            is StrongEmphasis -> withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { walk(c) }
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
