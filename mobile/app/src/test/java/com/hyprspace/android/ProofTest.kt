package com.hyprspace.android

import com.hyprspace.android.net.prove
import org.junit.Assert.assertEquals
import org.junit.Test

class ProofTest {
    // the same vector as the desktop's proofs_match_the_phones
    @Test
    fun provesLikeTheDesktop() {
        assertEquals("Ty027fUmm9y1FhIoARQBY5b7pqndj_4jToliCUSyavk", prove("K7MXQ2RTH9WP", "fp-example"))
    }
}
