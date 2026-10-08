package com.hyprspace.android.net

import java.util.Base64
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/**
 * HMAC-SHA256 keyed with a pairing code, base64url. Over the certificate's fingerprint it shows
 * the code was known without sending it; whoever sits in the middle sees another certificate,
 * so what they relay fails.
 */
fun prove(key: String, data: String): String {
    val mac = Mac.getInstance("HmacSHA256")
    mac.init(SecretKeySpec(key.toByteArray(), "HmacSHA256"))
    return Base64.getUrlEncoder().withoutPadding().encodeToString(mac.doFinal(data.toByteArray()))
}
