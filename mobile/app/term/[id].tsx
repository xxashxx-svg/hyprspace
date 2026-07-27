// A live mirror of one desktop pane, and a way to type into it.
//
// The terminal itself is xterm.js inside a WebView (src/terminal/termHtml.ts, generated) — an agent
// TUI is full of cursor addressing, so anything less than a real emulator renders as soup. It runs at
// the DESKTOP's cols/rows and is scaled to fit, so you're seeing the same screen rather than a
// reflowed guess. Input never goes through xterm: the composer and the key row send bytes over the
// bridge, straight to the PTY.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  KeyboardAvoidingView,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { useLocalSearchParams, useNavigation } from "expo-router";
import { WebView } from "react-native-webview";
import { useConn, usePane } from "../../src/store";
import { req, resyncPane, toBase64, watchPane, writePane } from "../../src/rpc";
import { TERM_HTML } from "../../src/terminal/termHtml";
import { c, font, providerLabel, r, sp, stateColor, stateLabel, t } from "../../src/theme";
import { Dot } from "../../src/ui";

// the keys you can't type on a phone keyboard but constantly need in an agent TUI
const KEYS: { label: string; bytes: string }[] = [
  { label: "esc", bytes: "\x1b" },
  { label: "tab", bytes: "\t" },
  { label: "↑", bytes: "\x1b[A" },
  { label: "↓", bytes: "\x1b[B" },
  { label: "←", bytes: "\x1b[D" },
  { label: "→", bytes: "\x1b[C" },
  { label: "⏎", bytes: "\r" },
  { label: "^C", bytes: "\x03" },
  { label: "^D", bytes: "\x04" },
  { label: "1", bytes: "1" },
  { label: "2", bytes: "2" },
  { label: "y", bytes: "y" },
];

const ZOOMS = [0.75, 1, 1.35, 1.8];

// how long the agent's TUI gets to settle a pasted prompt before Enter follows it (see submit)
const SETTLE_MS = 300;

export default function TermScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const nav = useNavigation();
  const found = usePane(id);
  const online = useConn((s) => s.status === "online");
  const web = useRef<WebView>(null);
  const ready = useRef(false);
  const queue = useRef<string[]>([]);
  const sent = useRef(""); // what the PTY has already been told about the composer's contents
  const [text, setText] = useState("");
  const [zoom, setZoom] = useState(1);
  const [gone, setGone] = useState(false);
  const [exited, setExited] = useState<number | null>(null);

  const pane = found?.pane;
  const space = found?.space;

  useEffect(() => {
    if (pane) nav.setOptions({ title: pane.title });
  }, [nav, pane]);

  // run the WebView's own functions; anything sent before the page boots is queued
  const run = useCallback((js: string) => {
    if (!ready.current) {
      queue.current.push(js);
      if (queue.current.length > 400) queue.current.splice(0, 200); // a long boot shouldn't hoard memory
      return;
    }
    web.current?.injectJavaScript(`${js};true;`);
  }, []);

  // subscribe once the space is awake — an unopened space has no PTY to mirror
  useEffect(() => {
    if (!id || !online || !space) return;
    let off: (() => void) | undefined;
    let cancelled = false;

    const start = async () => {
      if (!space.activated) {
        try {
          await req("space.activate", { ws: space.id });
        } catch {
          // if waking failed we still try to subscribe; the bridge answers "gone" and we say so
        }
      }
      if (cancelled) return;
      setGone(false);
      setExited(null);
      off = watchPane(id, {
        onData: (bytes) => run(`window.hsWrite(${JSON.stringify(toBase64(bytes))})`),
        onSize: (cols, rows) => run(`window.hsResize(${cols},${rows})`),
        onExit: (code) => setExited(code),
        onGone: () => setGone(true),
      });
    };
    void start();

    return () => {
      cancelled = true;
      off?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id, online, space?.id, space?.activated, run]);

  useEffect(() => {
    run(`window.hsZoom(${zoom})`);
  }, [zoom, run]);

  const onWebReady = () => {
    const reloaded = ready.current; // a second "ready" means Android threw the WebView away
    ready.current = true;
    const pending = queue.current;
    queue.current = [];
    for (const js of pending) web.current?.injectJavaScript(`${js};true;`);
    web.current?.injectJavaScript(`window.hsZoom(${zoom});true;`);
    // the reloaded page is blank and our subscription is still live, so nothing would redraw it
    // until the agent next printed something. ask for the replay again. (Not on first load — the
    // subscription's own replay is already on its way, and a second one would double it.)
    if (reloaded && id) resyncPane(id);
  };

  const send = (bytes: string) => {
    if (!id) return;
    writePane(id, bytes);
    // the TUI just consumed the input line, so our idea of what it holds has to reset with it
    if (bytes === "\r") {
      setText("");
      sent.current = "";
    }
    run("window.hsBottom()");
  };

  // Live typing: every edit goes to the PTY as it happens, so the agent sees you type exactly as it
  // would at the desktop — no "compose then send" step.
  //
  // We diff against what we've already sent rather than forwarding raw keys, because Android IMEs
  // don't emit clean keystrokes: autocorrect and gesture typing rewrite whole words at once. Taking
  // the common prefix and sending (backspaces + the new tail) turns any such rewrite into the same
  // edit a human would have made.
  const onType = (next: string) => {
    setText(next);
    if (!id) return;
    const prev = sent.current;
    let same = 0;
    while (same < prev.length && same < next.length && prev[same] === next[same]) same++;
    // \x7f is DEL — what a terminal expects from backspace
    const back = "\x7f".repeat(prev.length - same);
    // a soft-keyboard newline means submit, and TUIs want CR for that
    const added = next.slice(same).replace(/\n/g, "\r");
    sent.current = next;
    if (back || added) {
      writePane(id, back + added);
      run("window.hsBottom()");
    }
  };

  // Enter on its own — the text is already in the agent's input line, so this just commits it.
  // The small delay covers the one case where a whole block arrived at once (a paste): the TUI
  // needs a beat to settle it, and an early CR submits a truncated prompt.
  const submit = () => {
    if (!id) return;
    const pasted = text.length > 40;
    setText("");
    sent.current = "";
    setTimeout(() => writePane(id, "\r"), pasted ? SETTLE_MS : 0);
    run("window.hsBottom()");
  };

  const html = useMemo(() => ({ html: TERM_HTML, baseUrl: "about:blank" }), []);

  return (
    <KeyboardAvoidingView
      style={{ flex: 1, backgroundColor: c.bg }}
      behavior={Platform.OS === "ios" ? "padding" : undefined}
    >
      <View style={m.head}>
        <Dot color={stateColor[pane?.state ?? "idle"] ?? c.idle} />
        <View style={{ flex: 1, minWidth: 0 }}>
          <Text style={m.headTitle} numberOfLines={1}>
            {pane?.title ?? "Pane"}
          </Text>
          <Text style={m.headSub} numberOfLines={1}>
            {pane
              ? `${providerLabel[pane.provider] ?? pane.provider} · ${pane.activity ?? stateLabel[pane.state]}`
              : "not on the desktop any more"}
          </Text>
        </View>
        <Pressable
          hitSlop={8}
          onPress={() => setZoom(ZOOMS[(ZOOMS.indexOf(zoom) + 1) % ZOOMS.length])}
          style={m.zoom}
        >
          <Text style={m.zoomText}>{zoom === 1 ? "fit" : `${zoom}×`}</Text>
        </Pressable>
      </View>

      <View style={m.termWrap}>
        <WebView
          ref={web}
          source={html}
          originWhitelist={["*"]}
          onMessage={onWebReady}
          javaScriptEnabled
          domStorageEnabled={false}
          scrollEnabled={false}
          overScrollMode="never"
          setBuiltInZoomControls={false}
          androidLayerType="hardware"
          style={m.term}
          containerStyle={m.term}
        />
        {(gone || exited !== null) && (
          <View style={m.overlay} pointerEvents="none">
            <Text style={m.overlayText}>
              {gone
                ? "This pane isn't running on the desktop."
                : `The process exited (code ${exited}).`}
            </Text>
          </View>
        )}
      </View>

      <ScrollView horizontal showsHorizontalScrollIndicator={false} style={m.keysBar} contentContainerStyle={m.keys}>
        {KEYS.map((k) => (
          <Pressable
            key={k.label}
            onPress={() => send(k.bytes)}
            android_ripple={{ color: c.accentDim }}
            style={({ pressed }) => [m.key, pressed && m.keyOn]}
          >
            <Text style={m.keyText}>{k.label}</Text>
          </Pressable>
        ))}
      </ScrollView>

      <View style={m.composer}>
        <TextInput
          style={m.input}
          value={text}
          onChangeText={onType}
          placeholder={online ? "Type — it goes straight in" : "Offline"}
          placeholderTextColor={c.text3}
          editable={online}
          multiline
          autoCapitalize="none"
          autoCorrect={false}
          autoComplete="off"
          spellCheck={false}
        />
        <Pressable
          onPress={submit}
          disabled={!online}
          style={({ pressed }) => [m.send, !online && m.sendOff, pressed && m.keyOn]}
        >
          <Text style={m.sendText}>⏎</Text>
        </Pressable>
      </View>
    </KeyboardAvoidingView>
  );
}

const m = StyleSheet.create({
  head: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[3],
    paddingHorizontal: sp[4],
    paddingVertical: sp[2],
    backgroundColor: c.s1,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: c.border1,
  },
  headTitle: { color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },
  headSub: { color: c.text3, fontSize: t.sm, fontFamily: font.ui },
  zoom: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    borderRadius: r.one,
    paddingHorizontal: sp[2],
    paddingVertical: 3,
  },
  zoomText: { color: c.text2, fontSize: t.xs, fontFamily: font.mono },

  termWrap: { flex: 1, backgroundColor: c.bg },
  term: { flex: 1, backgroundColor: c.bg },
  overlay: {
    position: "absolute",
    left: 0,
    right: 0,
    bottom: 0,
    padding: sp[3],
    backgroundColor: "rgba(22,22,22,0.92)",
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },
  overlayText: { color: c.text2, fontSize: t.sm, fontFamily: font.ui, textAlign: "center" },

  keysBar: {
    flexGrow: 0,
    backgroundColor: c.s1,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },
  keys: { gap: sp[2], paddingHorizontal: sp[3], paddingVertical: sp[2] },
  key: {
    minWidth: 44,
    alignItems: "center",
    backgroundColor: c.s2,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    borderRadius: r.one,
    paddingHorizontal: sp[3],
    paddingVertical: sp[2],
  },
  keyOn: { backgroundColor: c.s3 },
  keyText: { color: c.text1, fontSize: t.md, fontFamily: font.mono },

  composer: {
    flexDirection: "row",
    alignItems: "flex-end",
    gap: sp[2],
    padding: sp[3],
    backgroundColor: c.s1,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },
  input: {
    flex: 1,
    maxHeight: 120,
    backgroundColor: c.s2,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    borderRadius: r.one,
    color: c.text1,
    fontFamily: font.ui,
    fontSize: t.md,
    paddingHorizontal: sp[3],
    paddingVertical: sp[2],
  },
  send: {
    backgroundColor: c.accent,
    borderRadius: r.one,
    paddingHorizontal: sp[4],
    paddingVertical: 11,
  },
  sendOff: { opacity: 0.4 },
  sendText: { color: c.onAccent, fontSize: t.md, fontFamily: font.uiMedium },
});
