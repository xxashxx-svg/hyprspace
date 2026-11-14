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
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Dispatchers
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
    /** [via] is how it's connected: LAN, Tailscale or Anywhere. */
    data class Online(val desktop: String, val version: String, val via: String = "") : Conn
    /** Lost or couldn't reach it; trying again at [retry] (ms since the epoch). */
    data class Offline(val message: String, val retry: Long) : Conn
    /** The computer won't let this phone in until something changes (pair again, update). */
    data class Denied(val message: String) : Conn
}

data class TranscriptView(
    val items: List<Item> = emptyList(),
    val model: String? = null,
    val context: Pair<Long, Long>? = null,
    /** The computer has prompts from before these. */
    val earlier: Boolean = false,
    val prompts: Int = 0,
    val loaded: Boolean = false,
    /** The [Link.lines] that brought the whole transcript, last time it came whole. */
    val line: Int = 0,
)

private sealed interface WsEvent {
    data object Open : WsEvent
    data class Text(val text: String) : WsEvent
    data class Gone(val why: String) : WsEvent
}

/** One socket and what it hears, in order. A [far] one goes through a port on this phone (Far). */
private class Socket(val host: String, val port: Int, val far: Boolean, val ws: WebSocket, val events: Channel<WsEvent>)

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
    // how much of each thread's journal to ask for, from its end
    private val tails = HashMap<Long, Int>()
    private val fits = HashMap<Long, Pair<Int, Int>>()
    private val lock = Any()

    @Volatile private var socket: Socket? = null
    /** A local line found while on the relays, for the next session to take over. */
    @Volatile private var handoff: Socket? = null
    /** Counts what the computer said on the open line. */
    @Volatile private var heard = 0L
    /** The app is on screen, so a dead line should be noticed in seconds rather than a minute. */
    @Volatile var awake = false
    /** Asks the open line to prove it's alive now: the network under it changed. */
    private val check = Channel<Unit>(Channel.CONFLATED)
    private val _lines = MutableStateFlow(0)
    /** Goes up each time a line to the computer comes up, so the screens know to check what they sent. */
    val lines: StateFlow<Int> = _lines
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
        handoff?.ws?.cancel()
        handoff = null
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
            tails.clear()
        }
        _board.value = null
        _commands.value = emptyList()
        _conn.value = if (store.saved.value.current() == null) Conn.None else Conn.Connecting
        if (store.saved.value.current() != null) start()
    }

    fun nudge() {
        nudge.trySend(Unit)
        check.trySend(Unit)
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
            if (handoff != null) continue
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
        val s = handoff?.also { handoff = null } ?: open(d.order(), d.port, d.far, trust)
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
                    _conn.value = Conn.Online(first.desktop, first.version, via(s))
                    seen = d.id to first.version
                    store.update { saved ->
                        saved.copy(desktops = saved.desktops.map {
                            if (it.id == d.id) it.copy(last = if (s.far) it.last else s.host, name = first.desktop, far = first.far) else it
                        })
                    }
                }
                is Down.Denied -> return denied(d, first)
                else -> return Conn.Offline("${d.name} didn't answer", 0)
            }
            // what was open before the line dropped opens again
            val (watched, fitted) = synchronized(lock) { watching.toList() to fits.toMap() }
            for (t in watched) send(s, Up.Watch(t, tail(t)))
            for ((t, size) in fitted) send(s, Up.Fit(t, size.first, size.second))
            _lines.value++
            // a line can go quiet without closing, and whatever is sent into it then is lost:
            // ping when it's quiet, and start over after two pings nobody answered. Ticks are
            // counted, not timed, so a phone waking from sleep pings before it gives up.
            while (check.tryReceive().isSuccess) Unit
            val watchdog = scope.launch {
                var last = heard
                var missed = 0
                while (true) {
                    val changed = withTimeoutOrNull(if (awake) 5_000L else 30_000L) { check.receive() } != null
                    if (changed) {
                        last = heard
                        send(s, Up.Ping)
                        delay(3_000)
                        if (heard == last) {
                            if (BuildConfig.DEBUG) Log.d(TAG, "no answer after a network change, starting over")
                            s.ws.cancel()
                            break
                        }
                    }
                    // a ping waits behind an upload on a slow line, so a quiet line isn't a dead one then
                    if (heard != last || uploads.isNotEmpty()) {
                        last = heard
                        missed = 0
                        continue
                    }
                    if (missed == 2) {
                        if (BuildConfig.DEBUG) Log.d(TAG, "two pings unanswered, starting over")
                        s.ws.cancel()
                        break
                    }
                    missed++
                    send(s, Up.Ping)
                }
            }
            val home = if (s.far && d.hosts.isNotEmpty()) scope.launch {
                delay(HOME_AFTER)
                while (true) {
                    val near = open(d.order(), d.port, null, trust)
                    if (near != null) {
                        handoff = near
                        s.ws.cancel()
                        break
                    }
                    nudge.receive()
                }
            } else null
            try {
                while (true) {
                    val down = next(s) ?: break
                    if (down is Down.Denied) return denied(d, down)
                    handle(down)
                }
            } finally {
                watchdog.cancel()
                home?.cancel()
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
                        heard++
                        if (BuildConfig.DEBUG) Log.d(TAG, "down ${it::class.simpleName} ${e.text.length}b")
                        return it
                    }
                is WsEvent.Gone -> {
                    if (BuildConfig.DEBUG) Log.d(TAG, "gone: ${e.why}")
                    return null
                }
                WsEvent.Open -> {}
            }
        }
    }

    /**
     * Opens a socket to whichever host answers first. Each starts a quarter second after the one
     * before, so the likeliest address gets a head start without the others waiting on it. A
     * computer with a [far] key is also tried through the relays, after the local addresses.
     */
    private suspend fun open(hosts: List<String>, port: Int, far: String?, trust: Pinned): Socket? = coroutineScope {
        val http = client(trust)
        val result = CompletableDeferred<Socket?>()
        val sockets = Collections.synchronizedList(ArrayList<Socket>())
        val failed = AtomicInteger(0)
        val routes = hosts.size + if (far != null) 1 else 0
        suspend fun attempt(host: String, port: Int, isFar: Boolean) {
            if (result.isCompleted) return
            val events = Channel<WsEvent>(Channel.UNLIMITED)
            val bracketed = if (host.contains(':')) "[$host]" else host
            val ws = http.newWebSocket(Request.Builder().url("wss://$bracketed:$port/").build(), Listener(events))
            val s = Socket(host, port, isFar, ws, events)
            sockets += s
            if (events.receive() is WsEvent.Open) {
                if (!result.complete(s)) ws.cancel()
            } else if (failed.incrementAndGet() == routes) {
                result.complete(null)
            }
        }
        val near = hosts.mapIndexed { i, host ->
            launch {
                delay(250L * i)
                attempt(host, port, false)
            }
        }
        val relayed = far?.let { key ->
            launch {
                delay(if (hosts.isEmpty()) 0 else FAR_AFTER)
                val local = withContext(Dispatchers.IO) { Far.open(key) }
                if (local != null) {
                    attempt(LOOPBACK, local, true)
                } else if (failed.incrementAndGet() == routes) {
                    result.complete(null)
                }
            }
        }
        val tries = near + listOfNotNull(relayed)
        var won: Socket? = null
        try {
            won = withTimeoutOrNull(if (far != null) 20_000 else 8_000) { result.await() }
        } finally {
            tries.forEach { it.cancel() }
            synchronized(sockets) { for (s in sockets) if (s !== won) s.ws.cancel() }
        }
        won
    }

    private fun via(s: Socket): String {
        val v4 = s.host.split('.').mapNotNull { it.toIntOrNull() }
        return when {
            s.far -> "Anywhere"
            v4.size == 4 && v4[0] == 100 && v4[1] in 64..127 -> "Tailscale"
            s.host.lowercase().startsWith("fd7a:115c:a1e0") -> "Tailscale"
            else -> "LAN"
        }
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
                val s = open(d.order(), d.port, d.far, Pinned(d.id)) ?: return@withTimeoutOrNull
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
                if (d.reset) t.reset(d.before)
                d.entries.forEach { t.add(it, replay = d.reset) }
                val line = if (d.reset) _lines.value else flow.value.line
                flow.value = TranscriptView(t.items(), t.model, t.context, loaded = true, earlier = t.before > 0, prompts = t.prompts, line = line)
            }
            is Down.Term -> synchronized(lock) {
                val (b, flow) = terms.getOrPut(d.frame.thread) { TermBuffer() to MutableStateFlow(TermView()) }
                flow.value = b.apply(d.frame)
            }
            is Down.Failed -> _failures.tryEmit(d.message)
            is Down.Note -> _failures.tryEmit(d.message)
            is Down.Commands -> _commands.value = d.list
            is Down.Folders -> _folders.value = d
            is Down.Uploaded -> uploads.remove(d.id)?.complete(d)
            is Down.Fetched -> fetches.remove(d.id)?.complete(d)
            is Down.Folder -> _folder.tryEmit(d.event)
            is Down.Welcome, is Down.Denied, Down.Pong -> {}
        }
    }

    fun transcript(thread: Long): StateFlow<TranscriptView> = synchronized(lock) {
        transcripts.getOrPut(thread) { Transcript() to MutableStateFlow(TranscriptView()) }.second
    }

    private val _commands = MutableStateFlow<List<Slash>>(emptyList())
    val commands: StateFlow<List<Slash>> = _commands

    fun term(thread: Long): StateFlow<TermView> = synchronized(lock) {
        terms.getOrPut(thread) { TermBuffer() to MutableStateFlow(TermView()) }.second
    }

    private val uploads = java.util.concurrent.ConcurrentHashMap<Long, CompletableDeferred<Down.Uploaded>>()
    private val nextUpload = java.util.concurrent.atomic.AtomicLong(1)

    /** Sends a photo, or a named file, to the computer and returns where it saved it, or null. */
    suspend fun upload(bytes: ByteArray, name: String? = null): String? {
        val id = nextUpload.getAndIncrement()
        val done = CompletableDeferred<Down.Uploaded>()
        uploads[id] = done
        if (!send(Up.Upload(id, java.util.Base64.getEncoder().encodeToString(bytes), name))) {
            uploads.remove(id)
            _failures.tryEmit("Not connected. Try again once it reconnects.")
            return null
        }
        val r = withTimeoutOrNull(60_000) { done.await() }
        uploads.remove(id)
        r?.error?.let { _failures.tryEmit(it) }
        if (r == null) _failures.tryEmit(if (name == null) "The photo didn't reach the computer." else "$name didn't reach the computer.")
        return r?.path
    }

    private val fetches = java.util.concurrent.ConcurrentHashMap<Long, CompletableDeferred<Down.Fetched>>()

    @Volatile private var media: Pair<String, okhttp3.OkHttpClient>? = null

    /**
     * An image or video from the computer, by its path there, saved to [into]. The computer
     * answers with a link on its own port, which this downloads over the pinned connection.
     */
    suspend fun fetch(path: String, into: java.io.File): Result<java.io.File> {
        val id = nextUpload.getAndIncrement()
        val done = CompletableDeferred<Down.Fetched>()
        fetches[id] = done
        if (!send(Up.Fetch(id, path))) {
            fetches.remove(id)
            return Result.failure(Exception("Not connected."))
        }
        val r = withTimeoutOrNull(60_000) { done.await() }
        fetches.remove(id)
        val url = r?.url ?: return Result.failure(Exception(r?.error ?: "The computer didn't answer."))
        val s = socket ?: return Result.failure(Exception("Not connected."))
        val d = store.saved.value.current() ?: return Result.failure(Exception("Not connected."))
        val http = media?.takeIf { it.first == d.id }?.second ?: client(Pinned(d.id)).also { media = d.id to it }
        val host = if (s.host.contains(':')) "[${s.host}]" else s.host
        return withContext(Dispatchers.IO) {
            runCatching {
                http.newCall(Request.Builder().url("https://$host:${s.port}$url").build()).execute().use { res ->
                    check(res.isSuccessful) { "The computer didn't send it." }
                    into.parentFile?.mkdirs()
                    into.outputStream().use { out -> res.body.byteStream().copyTo(out) }
                }
                into
            }
        }
    }

    private val _folder = MutableSharedFlow<FolderEvt>(extraBufferCapacity = 32)

    /** Answers about folders: listings, files, git status and diffs. */
    val folder: SharedFlow<FolderEvt> = _folder

    fun folder(cmd: FolderCmd): Boolean = send(Up.Folder(cmd))

    /** A line for the snackbar that isn't about something failing. */
    fun say(message: String) {
        _failures.tryEmit(message)
    }

    fun fail(message: String) {
        _failures.tryEmit(message)
    }

    // what the computer ran when last online, so the screen keeps its shape while reconnecting
    @Volatile
    private var seen: Pair<String, String>? = null

    fun desktopAtLeast(version: String): Boolean {
        val have = (_conn.value as? Conn.Online)?.version
            ?: seen?.takeIf { it.first == store.saved.value.current()?.id }?.second
            ?: return false
        return !com.hyprspace.android.update.Releases.newer(version, have)
    }

    fun browse(path: String) {
        _folders.value = null
        send(Up.Folders(path))
    }

    fun watch(thread: Long) {
        synchronized(lock) { watching += thread }
        send(Up.Watch(thread, tail(thread)))
    }

    private fun tail(thread: Long): Int = synchronized(lock) { tails[thread] ?: TAIL }

    /** Asks for the thread again with four times the journal. */
    fun earlier(thread: Long) {
        synchronized(lock) { tails[thread] = (tail(thread).toLong() * 4).coerceAtMost(Int.MAX_VALUE.toLong()).toInt() }
        send(Up.Watch(thread, tail(thread)))
    }

    /** Stops watching [thread]; [keepEarlier] keeps how far back it was read, for a return from another app. */
    fun unwatch(thread: Long, keepEarlier: Boolean = false) {
        synchronized(lock) {
            watching -= thread
            fits -= thread
            if (!keepEarlier) tails -= thread
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
        val s = open(link.hosts, link.port, link.far, trust)
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
                    val d = Desktop(
                        id, first.desktop, link.hosts, link.port, token, System.currentTimeMillis(),
                        last = s.host.takeIf { !s.far }, far = first.far ?: link.far,
                    )
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
        private const val LOOPBACK = "127.0.0.1"
        /** How long the local addresses get to answer before the relays are tried too. */
        private const val FAR_AFTER = 750L
        /** How long a line on the relays runs before it checks whether the local addresses answer now. */
        private const val HOME_AFTER = 3_000L
        private const val TAIL = 200_000

        /** What the computer's Settings lists this phone as. */
        var deviceName: () -> String = { android.os.Build.MODEL }
    }
}
