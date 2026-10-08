package com.hyprspace.android

import com.hyprspace.android.ui.home.Wakes
import java.time.LocalDateTime
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Test

class WakesTest {
    @Test
    fun snoozeChoicesMatchTheDesktops() {
        val wed = LocalDateTime.of(2026, 10, 7, 14, 30)
        assertEquals(
            listOf("In 1 hour", "In 3 hours", "This evening", "Tomorrow", "Next week"),
            Wakes.presets(wed).map { it.label },
        )
        val week = Wakes.presets(wed).last().at
        assertEquals(LocalDateTime.of(2026, 10, 12, 9, 0), week)
        val late = LocalDateTime.of(2026, 10, 12, 20, 0)
        assertEquals(listOf("In 1 hour", "In 3 hours", "Tomorrow", "Next week"), Wakes.presets(late).map { it.label })
        assertEquals(LocalDateTime.of(2026, 10, 19, 9, 0), Wakes.presets(late).last().at)
        assertEquals("Tomorrow 9:00 AM", Wakes.short(LocalDateTime.of(2026, 10, 8, 9, 0), wed.toLocalDate(), Locale.US))
        assertEquals("Mon 9:00 AM", Wakes.short(week, wed.toLocalDate(), Locale.US))
    }
}
