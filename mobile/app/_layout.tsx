import { useEffect } from "react";
import { AppState, StyleSheet, Text, View } from "react-native";
import { Stack } from "expo-router";
import { StatusBar } from "expo-status-bar";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { c, font, sp, t } from "../src/theme";
import { useConn } from "../src/store";
import { connect, disconnect, isOpen } from "../src/rpc";

/** the thin strip under the header that says why you're not seeing live data */
function ConnBanner() {
  const { status, error, retryIn, desktopHost } = useConn();
  if (status === "online" || status === "unpaired") return null;

  const text =
    status === "connecting"
      ? `Connecting to ${desktopHost || "your desktop"}…`
      : status === "retrying"
        ? `Lost the connection — retrying in ${retryIn}s`
        : status === "failed"
          ? (error ?? "Couldn't connect")
          : "Offline";

  return (
    <View style={[b.bar, status === "failed" && b.barBad]}>
      <Text style={b.text}>{text}</Text>
    </View>
  );
}

export default function RootLayout() {
  const loaded = useConn((s) => s.loaded);
  const paired = useConn((s) => !!s.host && !!s.token);

  useEffect(() => {
    void useConn.getState().load();
  }, []);

  // connect once paired, and reconnect whenever we come back to the foreground — Android freezes
  // sockets in the background, so a resumed app is usually holding a dead one
  useEffect(() => {
    if (!loaded || !paired) return;
    connect();
    const sub = AppState.addEventListener("change", (st) => {
      if (st === "active" && !isOpen()) connect();
    });
    return () => {
      sub.remove();
      disconnect();
    };
  }, [loaded, paired]);

  return (
    <SafeAreaProvider>
      <StatusBar style="light" />
      <View style={{ flex: 1, backgroundColor: c.bg }}>
        <Stack
          screenOptions={{
            headerStyle: { backgroundColor: c.s1 },
            headerTintColor: c.text1,
            headerTitleStyle: { color: c.text1, fontFamily: font.uiMedium, fontSize: t.lg },
            headerShadowVisible: false,
            contentStyle: { backgroundColor: c.bg },
            animation: "slide_from_right",
          }}
        >
          <Stack.Screen name="index" options={{ title: "HyprSpace" }} />
          <Stack.Screen name="pair" options={{ title: "Pair with desktop" }} />
          <Stack.Screen name="new-project" options={{ title: "New project" }} />
          <Stack.Screen name="space/[id]" options={{ title: "Space" }} />
          <Stack.Screen name="term/[id]" options={{ title: "Terminal" }} />
          <Stack.Screen name="git/[id]" options={{ title: "Changes" }} />
          <Stack.Screen name="automations" options={{ title: "Automations" }} />
          <Stack.Screen name="usage" options={{ title: "Usage" }} />
          <Stack.Screen name="settings" options={{ title: "Settings" }} />
        </Stack>
        <ConnBanner />
      </View>
    </SafeAreaProvider>
  );
}

const b = StyleSheet.create({
  bar: {
    paddingVertical: sp[2],
    paddingHorizontal: sp[4],
    backgroundColor: c.s3,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },
  barBad: { backgroundColor: "rgba(239,68,68,0.14)" },
  text: { color: c.text2, fontSize: t.sm, fontFamily: font.ui, textAlign: "center" },
});
