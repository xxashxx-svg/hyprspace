package com.hyprspace.android

import com.hyprspace.android.model.Tint
import com.hyprspace.android.model.colored
import com.hyprspace.android.model.tint
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HighlightTest {
    private fun pieces(code: String, lang: String) = tint(code, lang).map { code.substring(it.start, it.end) to it.tint }

    @Test
    fun tellsCodeApart() {
        val got = pieces("val n = send(\"hi\", 42) // later", "kotlin")
        assertTrue("val" to Tint.Keyword in got)
        assertTrue("send" to Tint.Function in got)
        assertTrue("\"hi\"" to Tint.Quoted in got)
        assertTrue("42" to Tint.Number in got)
        assertTrue("// later" to Tint.Comment in got)
    }

    @Test
    fun aLifetimeIsNotAString() {
        val quoted = pieces("fn f<'a>(x: &'a str) -> char { 'x' }", "rs").filter { it.second == Tint.Quoted }
        assertEquals(listOf("'x'"), quoted.map { it.first })
    }

    @Test
    fun aHashInsideAWordIsNotAComment() {
        val got = pieces("echo \$# a#b # real", "bash")
        assertEquals(listOf("# real"), got.filter { it.second == Tint.Comment }.map { it.first })
    }

    @Test
    fun sqlKeywordsInAnyCase() {
        assertTrue("SELECT" to Tint.Keyword in pieces("SELECT a FROM t -- all", "sql"))
    }

    @Test
    fun halfABlockStillColors() {
        assertEquals(listOf("\"\"\"doc" to Tint.Quoted), pieces("\"\"\"doc", "python"))
        assertEquals(listOf("/* open" to Tint.Comment), pieces("/* open", "ts"))
        assertEquals(listOf("\"open\\" to Tint.Quoted), pieces("\"open\\", "js"))
    }

    @Test
    fun leavesWhatItDoesNotKnowAlone() {
        assertFalse(colored("text"))
        assertTrue(colored("src/main.rs"))
        assertEquals(emptyList<Any>(), tint("just words", ""))
    }
}
