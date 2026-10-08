package com.hyprspace.android

import com.hyprspace.android.update.Releases
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReleasesTest {
    private fun release(tag: String, draft: Boolean = false, apk: Boolean = true): String {
        val exe = """{"name":"HyprSpace-0.25.0-setup.exe","browser_download_url":"https://x/setup.exe","size":9}"""
        val android = """{"name":"HyprSpace-android.apk","browser_download_url":"https://x/$tag.apk","size":8633476,"digest":"sha256:ab12"}"""
        val assets = if (apk) "$exe,$android" else exe
        return """{"tag_name":"$tag","draft":$draft,"prerelease":false,"assets":[$assets]}"""
    }

    @Test
    fun picksTheNewestPhoneRelease() {
        val list = listOf(
            release("v0.30.0"),
            release("android-v1.2.0"),
            release("android-v1.10.0"),
            release("android-v2.0.0", draft = true),
            release("android-v1.11.0", apk = false),
        ).joinToString(",", "[", "]")
        val r = Releases.parse(list)!!
        assertEquals("1.10.0", r.version)
        assertEquals("https://x/android-v1.10.0.apk", r.url)
        assertEquals(8633476L, r.size)
        assertEquals("ab12", r.sha256)
        assertNull(Releases.parse("[${release("v0.30.0")}]"))
    }

    @Test
    fun comparesVersionsByNumber() {
        assertTrue(Releases.newer("1.0.0", "0.24.8"))
        assertTrue(Releases.newer("0.24.10", "0.24.9"))
        assertFalse(Releases.newer("0.24.4", "0.24.4"))
        assertFalse(Releases.newer("0.24.3", "0.24.4"))
        assertFalse(Releases.newer("0.24.4", "0.24.4-dev"))
    }
}
