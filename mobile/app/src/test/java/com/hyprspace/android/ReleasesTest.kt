package com.hyprspace.android

import com.hyprspace.android.update.Releases
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReleasesTest {
    @Test
    fun readsTheApkFromTheLatestRelease() {
        val json = """
            {"tag_name":"v0.25.0","assets":[
              {"name":"HyprSpace-0.25.0-setup.exe","browser_download_url":"https://x/setup.exe","size":9},
              {"name":"HyprSpace-android.apk","browser_download_url":"https://x/a.apk","size":8633476,"digest":"sha256:ab12"}
            ]}
        """
        val r = Releases.parse(json)!!
        assertEquals("0.25.0", r.version)
        assertEquals("https://x/a.apk", r.url)
        assertEquals(8633476L, r.size)
        assertEquals("ab12", r.sha256)
        assertNull(Releases.parse("""{"tag_name":"v0.25.0","assets":[]}"""))
    }

    @Test
    fun comparesVersionsByNumber() {
        assertTrue(Releases.newer("0.25.0", "0.24.4"))
        assertTrue(Releases.newer("0.24.10", "0.24.9"))
        assertFalse(Releases.newer("0.24.4", "0.24.4"))
        assertFalse(Releases.newer("0.24.3", "0.24.4"))
        assertFalse(Releases.newer("0.24.4", "0.24.4-dev"))
    }
}
