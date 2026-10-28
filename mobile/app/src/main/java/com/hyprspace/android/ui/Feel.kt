// The app's haptics, named by what happened rather than by platform effect. They go through
// View.performHapticFeedback, so the phone's own touch feedback setting turns them all off.

package com.hyprspace.android.ui

import android.os.Build
import android.view.HapticFeedbackConstants as H
import android.view.View

// an effect newer than the phone plays nothing at all, so each falls back to the closest it has
private fun since(api: Int, effect: Int, older: Int) = if (Build.VERSION.SDK_INT >= api) effect else older

enum class Feel(val effect: Int) {
    /** A pick, a fold opening, a step on a slider. */
    Tick(since(34, H.SEGMENT_TICK, H.CLOCK_TICK)),

    /** A reply coming in on the open thread, a few times a second at most. */
    Stream(since(34, H.SEGMENT_FREQUENT_TICK, H.TEXT_HANDLE_MOVE)),

    /** An action sent: settle, pin, stop, copy. */
    Tap(H.CONTEXT_CLICK),

    /** A key on the terminal's key row. */
    Key(H.KEYBOARD_TAP),

    /** A long press, and something that needs you landing on the open thread. */
    Hold(H.LONG_PRESS),

    /** A swipe past the point where letting go acts. */
    Arm(since(34, H.GESTURE_THRESHOLD_ACTIVATE, H.LONG_PRESS)),

    On(since(34, H.TOGGLE_ON, H.CONTEXT_CLICK)),
    Off(since(34, H.TOGGLE_OFF, H.CLOCK_TICK)),

    /** It worked: allowed, paired, a run done. */
    Done(since(30, H.CONFIRM, H.CONTEXT_CLICK)),

    /** It didn't: not sent, not paired, a run failed. */
    Fail(since(30, H.REJECT, H.LONG_PRESS)),
}

fun View.feel(f: Feel) {
    performHapticFeedback(f.effect)
}

/** [ok] when [sent], else [Feel.Fail], for an ask that may not have gone out. */
fun View.felt(sent: Boolean, ok: Feel = Feel.Tap): Boolean {
    feel(if (sent) ok else Feel.Fail)
    return sent
}
