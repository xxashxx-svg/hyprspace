// The connection to the paired computer. It tries every address the computer gave at once and
// keeps the first that answers, says hello with the phone's token, and reconnects with a
// growing wait when the line drops. What comes in is kept per thread for the screens to show:
// the board, each watched transcript and each watched terminal.

package com.hyprspace.android.net

import android.util.Log
import com.hyprspace.android.BuildConfig
import com.hyprspace.android.data.Store
import com.hyprspace.android.model.TermBuffer
import com.hyprspace.android.model.TermView
import com.hyprspace.android.model.Transcript
import com.hyprspace.android.model.Item
import java.util.Collections
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

sealed interface Conn {
    /** Nothing paired yet. */
    data object None : Conn
    data object Connecting : Conn
    data class Online(val desktop: String, val version: String) : Conn
    /** Lost or couldn't reach it; trying again at [retry] (ms since the epoch). */
    data class Offline(val message: String, val retry: Long) : Conn
    /** The computer won't let this phone in until something changes (pair again, update). */
    data class Denied(val message: String) : Conn
}

data class TranscriptView(
    val items: List<Item> = emptyList(),
    val model: String? = null,
    val context: Pair<Long, Long>? = null,
    val loaded: Boolean = false,
)

private sealed interface WsEvent {
    data object Open : WsEvent
    data class Text(val text: String) : WsEvent
    data class Gone(val why: String) : WsEvent
}

/** One socket and what it hears, in order. */
private class Socket(val host: String, val ws: WebSocket, val events: Channel<WsEvent>)

class Link(private val store: Store, private val scope: CoroutineScope) {
    private val _conn = MutableStateFlow<Conn>(if (store.saved.value.current() == null) Conn.None else Conn.Connecting)
    val conn: StateFlow<Conn> = _conn
    private val _board = MutableStateFlow<Board?>(null)
    val board: StateFlow<Board?> = _board
    private val _failures = MutableSharedFlow<String>(extraBufferCapacity = 8)
    /** Things the computer couldn't do, worded for the user. */
    val failures: SharedFlow<String> = _failures
    private val _folders = MutableStateFlow<Down.Folders?>(null)
    val folders: StateFlow<Down.Folders?> = _folders

    private val transcripts = HashMap<Long, Pair<Transcript, MutableStateFlow<TranscriptView>>>()
    private val terms = HashMap<Long, Pair<TermBuffer, MutableStateFlow<TermView>>>()
    private val watching = LinkedHashSet<Long>()
    private val fits = HashMap<Long, Pair<Int, Int>>()
    private val lock = Any()

    @Volatile private var socket: Socket? = null
    /** The last try found nothing listening at any saved address. */
    @Volatile private var unreachable = false
    /** Looks for the computer with this fingerprint on the local network (`find`). */
    var finder: (suspend (String) -> Found?)? = null
    private var job: Job? = null
    /** Wakes the retry wait early: the app came to the front, or the network came back. */
    private val nudge = Channel<Unit>(Channel.CONFLATED)

    /** Connect to the current computer and stay connected until [stop]. */
    fun start() {
        synchronized(lock) {
            if (job?.isActive == true) {
                nudge.trySend(Unit)
                return
            }
            job = scope.launch { run() }
        }
    }

    fun stop() {
        synchronized(lock) {
            job?.cancel()
            job = null
        }
        socket?.ws?.close(1000, null)
        socket = null
        if (_conn.value !is Conn.Denied) {
            _conn.value = if (store.saved.value.current() == null) Conn.None else Conn.Offline("Paused", 0)
        }
    }

    /** Drop everything from the last computer, for a switch to another one. */
    fun restart() {
        stop()
        synchronized(lock) {
            transcripts.clear()
            terms.clear()
            watching.clear()
            fits.clear()
        }
        _board.value = null
        _conn.value = if (store.saved.value.current() == null) Conn.None else Conn.Connecting
        if (store.saved.value.current() != null) start()
    }

    fun nudge() {
        nudge.trySend(Unit)
    }

    private suspend fun run() {
        var wait = 1_000L
        while (scope.isActive) {
            val d = store.saved.value.current()
            if (d == null) {
                _conn.value = Conn.None
                return
            }
            if (_conn.value !is Conn.Online) _conn.value = Conn.Connecting
            unreachable = false
            val why = session(d)
            // nothing answered where it was saved: look for it on the local network, and go
            // straight there if it moved
            if (unreachable) {
                val found = finder?.invoke(d.id)
                if (found != null && (found.port != d.port || !d.hosts.containsAll(found.hosts))) {
                    store.update { saved ->
                        saved.copy(desktops = saved.desktops.map {
                            if (it.id != d.id) it else it.copy(
                                hosts = (found.hosts + it.hosts).distinct().take(6),
                                port = found.port,
                                last = found.hosts.first(),
                            )
                        })
                    }
                    continue
                }
            }
            if (why is Conn.Denied) {
                _conn.value = why
                return
            }
            // a line that was up and dropped is worth a quick retry; one that never came up waits longer each time
            if (why is Conn.Online) wait = 1_000L
            val reason = (why as? Conn.Offline)?.message ?: "Lost the connection. Reconnecting."
            _conn.value = Conn.Offline(reason, System.currentTimeMillis() + wait)
            withTimeoutOrNull(wait) { nudge.receive() }
            wait = (wait * 2).coerceAtMost(15_000L)
        }
    }

    /** One connection, start to end. Returns why it ended. */
    private suspend fun session(d: Desktop): Conn {
        val trust = Pinned(d.id)
        val s = open(d.order(), d.port, trust)
        if (s == null) {
            unreachable = true
            return Conn.Offline("Can't reach ${d.name}. Is HyprSpace open there with Phone on?", 0)
        }
        socket = s
        try {
            send(s, Up.Hello(d.token, deviceName()))
            val first = withTimeoutOrNull(10_000) { next(s) }
            when (first) {
                is Down.Welcome -> {
                    _conn.value = Conn.Online(first.desktop, first.version)
                    store.update { saved ->
                        saved.copy(desktops = saved.desktops.map {
                            if (it.id == d.id) it.copy(last = s.host, name = first.desktop) else it
                        })
                    }
                }
                is Down.Denied -> return denied(d, first)
                else -> return Conn.Offline("${d.name} didn't answer", 0)
            }
            // what was open before the line dropped opens again
            val (watched, fitted) = synchronized(lock) { watching.toList() to fits.toMap() }
            for (t in watched) send(s, Up.Watch(t))
            for ((t, size) in fitted) send(s, Up.Fit(t, size.first, size.second))
            while (true) {
                val down = next(s) ?: break
                if (down is Down.Denied) return denied(d, down)
                handle(down)
            }
            return Conn.Online("", "")
        } finally {
            s.ws.cancel()
            if (socket === s) socket = null
        }
    }

    /** The next message, or null once the socket is gone. Unknown shapes are skipped. */
    private suspend fun next(s: Socket): Down? {
        while (true) {
            when (val e = s.events.receive()) {
                is WsEvent.Text -> runCatching { wire.decodeFromString(Down.serializer(), e.text) }
                    .onFailure { if (BuildConfig.DEBUG) Log.w(TAG, "unreadable: ${e.text.take(200)}", it) }
                    .getOrNull()?.let {
                        if (BuildConfig.DEBUG) Log.d(TAG, "down ${it::class.simpleName} ${e.text.length}b")
                        return it
                    }
                is WsEvent.Gone -> return null
                WsEvent.Open -> {}
            }
        }
    }

    /**
     * Opens a socket to whichever host answers first. Each starts a quarter second after the one
     * before, so the likeliest address gets a head start without the others waiting on it.
     */
    private suspend fun open(hosts: List<String>, port: Int, trust: Pinned): Socket? = coroutineScope {
        val http = client(trust)
        val result = CompletableDeferred<Socket?>()
        val sockets = Collections.synchronizedList(ArrayList<Socket>())
        val failed = AtomicInteger(0)
        val tries = hosts.mapIndexed { i, host ->
            launch {
                delay(250L * i)
                if (result.isCompleted) return@launch
                val events = Channel<WsEvent>(Channel.UNLIMITED)
                val bracketed = if (host.contains(':')) "[$host]" else host
                val ws = http.newWebSocket(Request.Builder().url("wss://$bracketed:$port/").build(), Listener(events))
                val s = Socket(host, ws, events)
                sockets += s
                if (events.receive() is WsEvent.Open) {
                    if (!result.complete(s)) ws.cancel()
                } else if (failed.incrementAndGet() == hosts.size) {
                    result.complete(null)
                }
            }
        }
        val won = withTimeoutOrNull(8_000) { result.await() }
        tries.forEach { it.cancel() }
        synchronized(sockets) { for (s in sockets) if (s !== won) s.ws.cancel() }
        won
    }

    private fun send(s: Socket, up: Up) {
        s.ws.send(wire.encodeToString(Up.serializer(), up))
    }

    /** The computer won't have this phone; when it removed it, the phone lets the computer go too. */
    private fun denied(d: Desktop, why: Down.Denied): Conn {
        if (why.forget) store.forget(d.id)
        return Conn.Denied(why.message)
    }

    /**
     * Forgets [d] here and tells the computer to forget this phone, over the open line or a
     * quick one of its own. A computer that's off keeps the phone in its list until it's
     * removed there.
     */
    suspend fun leave(d: Desktop) {
        val live = socket
        if (live != null && store.saved.value.current()?.id == d.id && _conn.value is Conn.Online) {
            send(live, Up.Leave)
        } else {
            withTimeoutOrNull(6_000) {
                val s = open(d.order(), d.port, Pinned(d.id)) ?: return@withTimeoutOrNull
                try {
                    send(s, Up.Hello(d.token, deviceName()))
                    if (next(s) is Down.Welcome) send(s, Up.Leave)
                } finally {
                    s.ws.close(1000, null)
                }
            }
        }
        store.forget(d.id)
    }

    /** Sends when connected. Returns whether it went out. */
    fun send(up: Up): Boolean {
        val s = socket ?: return false
        if (_conn.value !is Conn.Online) return false
        return s.ws.send(wire.encodeToString(Up.serializer(), up))
    }

    fun ask(ask: Ask): Boolean = send(Up.Do(ask)).also {
        if (!it) _failures.tryEmit("Not connected. Try again once it reconnects.")
    }

    private fun handle(d: Down) {
        when (d) {
            is Down.BoardMsg -> {
                _board.value = d.board
                if (store.saved.value.theme != d.board.theme) store.update { it.copy(theme = d.board.theme) }
            }
            is Down.Transcript -> synchronized(lock) {
                val (t, flow) = transcripts.getOrPut(d.thread) { Transcript() to MutableStateFlow(TranscriptView()) }
                if (d.reset) t.reset()
                d.entries.forEach(t::add)
                flow.value = TranscriptView(t.items(), t.model, t.context, loaded = true)
            }
            is Down.Term -> synchronized(lock) {
                val (b, flow) = terms.getOrPut(d.frame.thread) { TermBuffer() to MutableStateFlow(TermView()) }
                flow.value = b.apply(d.frame)
            }
            is Down.Failed -> _failures.tryEmit(d.message)
            is Down.Folders -> _folders.value = d
            is Down.Uploaded -> uploads.remove(d.id)?.complete(d)
            is Down.Welcome, is Down.Denied, Down.Pong -> {}
        }
    }

    fun transcript(thread: Long): StateFlow<TranscriptView> = synchronized(lock) {
        transcripts.getOrPut(thread) { Transcript() to MutableStateFlow(TranscriptView()) }.second
    }

    fun term(thread: Long): StateFlow<TermView> = synchronized(lock) {
        terms.getOrPut(thread) { TermBuffer() to MutableStateFlow(TermView()) }.second
    }

    private val uploads = java.util.concurrent.ConcurrentHashMap<Long, CompletableDeferred<Down.Uploaded>>()
    private val nextUpload = java.util.concurrent.atomic.AtomicLong(1)

    /** Sends a photo to the computer and returns where it saved it, or null. */
    suspend fun upload(bytes: ByteArray): String? {
        val id = nextUpload.getAndIncrement()
        val done = CompletableDeferred<Down.Uploaded>()
        uploads[id] = done
        if (!send(Up.Upload(id, java.util.Base64.getEncoder().encodeToString(bytes)))) {
            uploads.remove(id)
            _failures.tryEmit("Not connected. Try again once it reconnects.")
            return null
        }
        val r = withTimeoutOrNull(60_000) { done.await() }
        uploads.remove(id)
        r?.error?.let { _failures.tryEmit(it) }
        if (r == null) _failures.tryEmit("The photo didn't reach the computer.")
        return r?.path
    }

    fun desktopAtLeast(version: String): Boolean {
        val have = (_conn.value as? Conn.Online)?.version ?: return false
        return !com.hyprspace.android.update.Releases.newer(version, have)
    }

    fun browse(path: String) {
        _folders.value = null
        send(Up.Folders(path))
    }

    fun watch(thread: Long) {
        synchronized(lock) { watching += thread }
        send(Up.Watch(thread))
    }

    fun unwatch(thread: Long) {
        synchronized(lock) {
            watching -= thread
            fits -= thread
        }
        send(Up.Unwatch(thread))
    }

    fun fit(thread: Long, cols: Int, rows: Int) {
        synchronized(lock) { fits[thread] = cols to rows }
        send(Up.Fit(thread, cols, rows))
    }

    fun unfit(thread: Long) {
        synchronized(lock) { fits -= thread }
        send(Up.Unfit(thread))
    }

    /**
     * Pairs with the computer behind [link] and saves it as the current one. The socket used
     * to pair closes; the usual connection takes over from there.
     */
    suspend fun pair(link: PairLink): Result<Desktop> {
        val trust = Pinned(link.fingerprint)
        val s = open(link.hosts, link.port, trust)
            ?: return Result.failure(Exception("Can't reach ${link.name}. Check that this phone is on the same network or on Tailscale, and that Phone is on in HyprSpace's Settings."))
        try {
            val id = trust.seen ?: return Result.failure(Exception("The computer's certificate went missing."))
            val key = link.key()
            send(s, Up.Pair(prove(key, id), deviceName()))
            return when (val first = withTimeoutOrNull(10_000) { next(s) }) {
                is Down.Welcome -> {
                    val token = first.token ?: return Result.failure(Exception("The computer didn't hand out a token."))
                    if (first.proof != prove(key, "desktop $id")) {
                        return Result.failure(Exception("That computer couldn't show it knows the code, so this phone didn't pair. Check that the address is your computer's."))
                    }
                    val d = Desktop(id, first.desktop, link.hosts, link.port, token, System.currentTimeMillis(), s.host)
                    store.remember(d)
                    Result.success(d)
                }
                is Down.Denied -> Result.failure(Exception(first.message))
                else -> Result.failure(Exception("${link.name} didn't answer."))
            }
        } finally {
            s.ws.close(1000, null)
        }
    }

    private class Listener(val events: Channel<WsEvent>) : WebSocketListener() {
        override fun onOpen(webSocket: WebSocket, response: Response) {
            events.trySend(WsEvent.Open)
        }

        override fun onMessage(webSocket: WebSocket, text: String) {
            events.trySend(WsEvent.Text(text))
        }

        override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
            events.trySend(WsEvent.Gone(reason))
        }

        override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
            events.trySend(WsEvent.Gone(t.message ?: "failed"))
        }
    }

    companion object {
        private const val TAG = "HyprLink"

        /** What the computer's Settings lists this phone as. */
        var deviceName: () -> String = { android.os.Build.MODEL }
    }
}
