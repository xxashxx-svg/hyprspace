package com.hyprspace.android

import com.hyprspace.android.ui.thread.stretches
import org.junit.Assert.assertEquals
import org.junit.Test

class StretchesTest {
    @Test
    fun blankLinesCutOutsideFencesOnly() {
        val text = "# Title\n\nOne line\nand more\n\n```kt\nval a = 1\n\nval b = 2\n```\n\n- item\n\n  still the item\n- next"
        assertEquals(
            listOf("# Title\n", "One line\nand more\n", "```kt\nval a = 1\n\nval b = 2\n```\n", "- item\n\n  still the item\n- next\n"),
            stretches(text),
        )
    }

    @Test
    fun anOpenFenceKeepsEverythingAfterIt() {
        assertEquals(listOf("intro\n", "~~~\na\n\nb\n"), stretches("intro\n\n~~~\na\n\nb"))
    }
}
