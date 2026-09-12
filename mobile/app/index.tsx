// Home: every space on the desktop, folding open to its panes the way the desktop sidebar does.
// Anything waiting on you is lifted to the top, because that is the reason to look at this on a
// phone at all.
import { useMemo, useState } from "react";
import { Pressable, RefreshControl, ScrollView, StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useConn } from "../src/store";
import { req } from "../src/rpc";
import { c, font, sp, t } from "../src/theme";
import { Btn, Card, Empty, Label, Loading, Row, s as u } from "../src/ui";
import { PaneRow } from "../src/PaneRow";
import { SpaceSection } from "../src/SpaceSection";
import { relTime } from "../src/fmt";

export default function Home() {
  const router = useRouter();
  const insets = useSafeAreaInsets();
  const { status, snap, host, token, desktopHost } = useConn();
  const paired = !!host && !!token;

  // Folded by hand wins; otherwise the space you are in on the desktop is the one already open.
  const [byHand, setByHand] = useState<Map<string, boolean>>(new Map());
  const isOpen = (id: string) => byHand.get(id) ?? id === snap?.activeId;
  const toggle = (id: string) => setByHand((m) => new Map(m).set(id, !isOpen(id)));

  const spaces = snap?.spaces ?? [];
  const waiting = useMemo(
    () =>
      spaces.flatMap((w) => w.panes.filter((p) => p.state === "waiting").map((p) => ({ p, w }))),
    [spaces],
  );

  const top = { paddingTop: insets.top + sp[3] };

  if (!paired) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={[u.screenPad, top]}>
        <Empty
          title="Not paired yet"
          hint="On your desktop open Settings, then Mobile, turn on Sync to your phone, and scan the code it shows."
        >
          <Btn kind="primary" onPress={() => router.push("/pair")} style={{ marginTop: sp[3], minWidth: 180 }}>
            Pair with desktop
          </Btn>
        </Empty>
      </ScrollView>
    );
  }

  return (
    <ScrollView
      style={u.screen}
      contentContainerStyle={[u.screenPad, top]}
      refreshControl={
        <RefreshControl
          refreshing={status === "connecting"}
          onRefresh={() => void req("state").catch(() => {})}
          tintColor={c.text3}
          colors={[c.accent]}
          progressViewOffset={insets.top}
        />
      }
    >
      {/* our own header: the stack's would print the app name above this one and waste the space */}
      <View style={h.top}>
        <View style={{ flex: 1, minWidth: 0 }}>
          <Text style={h.hostName} numberOfLines={1}>
            {desktopHost || "Desktop"}
          </Text>
          <Text style={u.sub}>
            {status === "online"
              ? snap
                ? `${spaces.length} space${spaces.length === 1 ? "" : "s"} · updated ${relTime(snap.at)}`
                : "Connected"
              : status}
          </Text>
        </View>
        <Pressable onPress={() => router.push("/settings")} hitSlop={12} style={h.gearBtn}>
          <Text style={h.gear}>⚙</Text>
        </Pressable>
      </View>

      {waiting.length > 0 && (
        <View>
          <Label>Waiting on you</Label>
          <Card>
            {waiting.map(({ p, w }, i) => (
              <PaneRow key={p.id} pane={p} sub={`${w.name} · ${p.activity ?? "needs you"}`} last={i === waiting.length - 1} />
            ))}
          </Card>
        </View>
      )}

      <View>
        <Label>Spaces</Label>
        {spaces.length === 0 ? (
          <Card>
            {status === "online" ? (
              <Empty title="No spaces yet" hint="Open a folder on the desktop, or create a project here. Either way it shows up on both.">
                <Btn kind="primary" onPress={() => router.push("/new-project")} style={{ marginTop: sp[3], minWidth: 180 }}>
                  New project
                </Btn>
              </Empty>
            ) : (
              <Loading label="Waiting for the desktop" />
            )}
          </Card>
        ) : (
          <View>
            {spaces.map((w) => (
              <SpaceSection key={w.id} space={w} open={isOpen(w.id)} onToggle={() => toggle(w.id)} />
            ))}
          </View>
        )}
      </View>

      <View>
        <Label>More</Label>
        <Card>
          <Row onPress={() => router.push("/compose")}>
            <Text style={[u.title, { flex: 1 }]}>New thread</Text>
            <Text style={h.chev}>›</Text>
          </Row>
          <Row onPress={() => router.push("/new-project")}>
            <Text style={[u.title, { flex: 1 }]}>New project</Text>
            <Text style={h.chev}>›</Text>
          </Row>
          <Row onPress={() => router.push("/usage")}>
            <Text style={[u.title, { flex: 1 }]}>Usage</Text>
            <Text style={u.sub}>{snap?.usage?.five ? `${Math.round(snap.usage.five.pct)}%` : ""}</Text>
            <Text style={h.chev}>›</Text>
          </Row>
          <Row last onPress={() => router.push("/settings")}>
            <Text style={[u.title, { flex: 1 }]}>Settings</Text>
            <Text style={h.chev}>›</Text>
          </Row>
        </Card>
      </View>
    </ScrollView>
  );
}

const h = StyleSheet.create({
  top: { flexDirection: "row", alignItems: "center", gap: sp[3] },
  hostName: { color: c.text1, fontSize: t.xl, fontFamily: font.uiMedium },
  gearBtn: { padding: sp[1] },
  gear: { color: c.text3, fontSize: 22 },
  chev: { color: c.text3, fontSize: 20, marginLeft: sp[1] },
});
