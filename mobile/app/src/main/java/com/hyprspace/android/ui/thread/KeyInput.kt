// The phone keyboard typing straight into a terminal, with no text box in between. A keyboard
// talks to an editor, so this view poses as one that holds nothing: what the keyboard commits
// goes to the terminal as typed, a delete as a backspace, Enter as Enter. With nothing kept, a
// Ctrl+B or an arrow leaves no stray letters behind.

package com.hyprspace.android.ui.thread

import android.content.Context
import android.text.InputType
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager

class KeyInput(context: Context) : View(context) {
    /** Bytes for the terminal. */
    var onKeys: (String) -> Unit = {}
    /** Whether the key bar's Ctrl is held for the next key, and how to let it go. */
    var ctrl: () -> Boolean = { false }
    var ctrlUsed: () -> Unit = {}

    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    fun show() {
        requestFocus()
        context.getSystemService(InputMethodManager::class.java).showSoftInput(this, 0)
    }

    fun hide() {
        context.getSystemService(InputMethodManager::class.java).hideSoftInputFromWindow(windowToken, 0)
    }

    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(out: EditorInfo): InputConnection {
        // a password field's keyboard: no suggestions, no autocorrect, letters as they're typed
        out.inputType = InputType.TYPE_CLASS_TEXT or
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
            InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        out.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_NONE
        return Connection()
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean = key(event) || super.onKeyDown(keyCode, event)

    /** Text the keyboard committed. Line breaks go in as Enter. */
    private fun type(text: String) {
        if (text.isEmpty()) return
        if (ctrl() && text.length == 1) {
            ctrlUsed()
            control(text[0])?.let { onKeys(it); return }
        }
        onKeys(text.replace("\r\n", "\r").replace('\n', '\r'))
    }

    /** A key press, from a hardware keyboard or a keyboard that sends keys instead of text. */
    private fun key(e: KeyEvent): Boolean {
        if (e.action != KeyEvent.ACTION_DOWN) return true
        val seq = when (e.keyCode) {
            KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> "\r"
            KeyEvent.KEYCODE_DEL -> "\u007f"
            KeyEvent.KEYCODE_FORWARD_DEL -> "\u001b[3~"
            KeyEvent.KEYCODE_TAB -> if (e.isShiftPressed) "\u001b[Z" else "\t"
            KeyEvent.KEYCODE_ESCAPE -> "\u001b"
            KeyEvent.KEYCODE_DPAD_UP -> "\u001b[A"
            KeyEvent.KEYCODE_DPAD_DOWN -> "\u001b[B"
            KeyEvent.KEYCODE_DPAD_RIGHT -> "\u001b[C"
            KeyEvent.KEYCODE_DPAD_LEFT -> "\u001b[D"
            KeyEvent.KEYCODE_MOVE_HOME -> "\u001b[H"
            KeyEvent.KEYCODE_MOVE_END -> "\u001b[F"
            KeyEvent.KEYCODE_PAGE_UP -> "\u001b[5~"
            KeyEvent.KEYCODE_PAGE_DOWN -> "\u001b[6~"
            else -> null
        }
        if (seq != null) {
            onKeys(seq)
            return true
        }
        val c = e.getUnicodeChar(e.metaState and KeyEvent.META_CTRL_MASK.inv()).takeIf { it != 0 }?.toChar()
            ?: return false
        if (e.isCtrlPressed || ctrl()) {
            if (!e.isCtrlPressed) ctrlUsed()
            control(c)?.let { onKeys(it); return true }
        }
        onKeys(if (e.isAltPressed) "\u001b$c" else c.toString())
        return true
    }

    /** Ctrl with a letter or one of @[\]^_ as the control byte a terminal sends. */
    private fun control(c: Char): String? {
        val u = c.uppercaseChar()
        return when (u) {
            in 'A'..'Z', '@', '[', '\\', ']', '^', '_' -> (u.code and 0x1f).toChar().toString()
            ' ' -> "\u0000"
            '?' -> "\u007f"
            else -> null
        }
    }

    private inner class Connection : BaseInputConnection(this@KeyInput, false) {
        /** What the keyboard is still composing; sent as it changes, so it shows as typed. */
        private var composing = ""

        private fun change(to: String) {
            val keep = composing.commonPrefixWith(to).length
            repeat(composing.length - keep) { onKeys("\u007f") }
            type(to.substring(keep))
            composing = to
        }

        override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
            change(text.toString())
            composing = ""
            return true
        }

        override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
            change(text.toString())
            return true
        }

        override fun finishComposingText(): Boolean {
            composing = ""
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            repeat(beforeLength.coerceAtMost(64)) { onKeys("\u007f") }
            return true
        }

        override fun sendKeyEvent(event: KeyEvent): Boolean = key(event)

        override fun performEditorAction(actionCode: Int): Boolean {
            onKeys("\r")
            return true
        }

        // the keyboard reads nothing back, so it never offers to change what's already sent
        override fun getTextBeforeCursor(n: Int, flags: Int): CharSequence = composing
        override fun getTextAfterCursor(n: Int, flags: Int): CharSequence = ""
    }
}
