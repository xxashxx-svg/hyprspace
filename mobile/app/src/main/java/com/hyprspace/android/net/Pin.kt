package com.hyprspace.android.net

import android.annotation.SuppressLint
import android.util.Base64
import java.security.MessageDigest
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.X509TrustManager
import okhttp3.OkHttpClient

/** SHA-256 of a certificate, base64url without padding: how the desktop writes its fingerprint. */
fun fingerprint(cert: X509Certificate): String =
    Base64.encodeToString(
        MessageDigest.getInstance("SHA-256").digest(cert.encoded),
        Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP,
    )

/**
 * Trusts the one certificate the desktop showed when it paired, and nothing else. The desktop
 * makes its own certificate, so no authority vouches for it; the pin is the whole check. With no
 * pin (a code typed by hand) it takes the first certificate it sees and remembers it in [seen].
 */
@SuppressLint("CustomX509TrustManager")
class Pinned(private val pin: String?) : X509TrustManager {
    @Volatile var seen: String? = null
        private set

    override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
        val leaf = chain.firstOrNull() ?: throw CertificateException("no certificate")
        val got = fingerprint(leaf)
        if (pin != null && got != pin) throw CertificateException("not the paired computer")
        seen = got
    }

    override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) =
        throw CertificateException("no client certificates")

    override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
}

fun client(trust: Pinned): OkHttpClient {
    val ssl = SSLContext.getInstance("TLS").apply { init(null, arrayOf(trust), null) }
    return OkHttpClient.Builder()
        .sslSocketFactory(ssl.socketFactory, trust)
        // the pin already proves which computer this is; its name can be any address it has
        .hostnameVerifier { _, _ -> true }
        .connectTimeout(4, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS)
        .writeTimeout(10, TimeUnit.SECONDS)
        .pingInterval(20, TimeUnit.SECONDS)
        .build()
}
