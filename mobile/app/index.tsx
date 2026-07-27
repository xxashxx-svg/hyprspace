// Home: every space on the desktop, with the panes that want your attention surfaced first.
import { useMemo } from "react";
import { Pressable, RefreshControl, ScrollView, StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import { useConn } from "../src/store";
import { req } from "../src/rpc";
import { c, font, r, sp, t } from "../src/theme";
import { Btn, Card, Dot, Empty, Label, Loading, Row, s as u } from "../src/ui";
import { PaneRow } from "../src/PaneRow";
import { folderName, relTime } from "../src/fmt";

export default function Home() {
  const router = useRouter();
  const { status, snap, host, token, desktopHost } = useConn();
  const paired = !!host && !!token;

  const waiting = useMemo(
    () =>
      (snap?.spaces ?? []).flatMap((w) => w.panes.filter((p) => p.state === "waiting").map((p) => ({ p, w }))),
    [snap],
  );

  if (!paired) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        <Empty
          title="Not paired yet"
          hint="On your desktop open Settings → Mobile, turn on “Sync to your phone”, and scan the QR it shows."
        >
          <Btn kind="primary" onPress={() => router.push("/pair")} style={{ marginTop: sp[3], minWidth: 180 }}>
            Pair with desktop
          </Btn>
        </Empty>
      </ScrollView>
    );
  }

  const spaces = snap?.spaces ?? [];

  return (
    <ScrollView
      style={u.screen}
      contentContainerStyle={u.screenPad}
      refreshControl={
        <RefreshControl
          refreshing={status === "connecting"}
          onRefresh={() => void req("state").catch(() => {})}
          tintColor={c.text3}
          colors={[c.accent]}
        />
      }
    >
      <View style={h.top}>
        <View style={{ flex: 1 }}>
          <Text style={h.hostName}>{desktopHost || "Desktop"}</Text>
          <Text style={u.sub}>
            {status === "online"
              ? snap
                ? `${spaces.length} space${spaces.length === 1 ? "" : "s"} · updated ${relTime(snap.at)}`
                : "Connected"
              : status}
          </Text>
        </View>
        <Pressable onPress={() => router.push("/settings")} hitSlop={10}>
          <Text style={h.gear}>⚙</Text>
        </Pressable>
      </View>

      {waiting.length > 0 && (
        <View>
          <Label>Waiting on you</Label>
          <Card>
            {waiting.map(({ p, w }, i) => (
              <PaneRow
                key={p.id}
                pane={p}
                sub={`${w.name} · ${p.activity ?? "needs you"}`}
                last={i === waiting.length - 1}
              />
            ))}
          </Card>
        </View>
      )}

      <View>
        <Label>Spaces</Label>
        {spaces.length === 0 ? (
          <Card>
            {status === "online" ? (
              <Empty title="No spaces yet" hint="Create one on the desktop and it'll show up here." />
            ) : (
              <Loading label="Waiting for the desktop…" />
            )}
          </Card>
        ) : (
          <Card>
            {spaces.map((w, i) => (
              <Row key={w.id} last={i === spaces.length - 1} onPress={() => router.push(`/space/${w.id}`)}>
                <View style={[h.swatch, { backgroundColor: w.color }]} />
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Text style={u.title} numberOfLines={1}>
                    {w.name}
                  </Text>
                  <Text style={u.sub} numberOfLines={1}>
                    {w.kind === "open" ? "Open space" : folderName(w.cwd) || "No folder"} ·{" "}
                    {w.panes.length} pane{w.panes.length === 1 ? "" : "s"}
                    {w.activated ? "" : " · asleep"}
                  </Text>
                </View>
                {w.panes.some((p) => p.state === "working") && <Dot color={c.busy} />}
                <Text style={h.chev}>›</Text>
              </Row>
            ))}
          </Card>
        )}
      </View>

      <View>
        <Label>More</Label>
        <Card>
          <Row onPress={() => router.push("/automations")}>
            <Text style={[u.title, { flex: 1 }]}>Automations</Text>
            <Text style={u.sub}>{snap?.automations.length ?? 0}</Text>
            <Text style={h.chev}>›</Text>
          </Row>
          <Row onPress={() => router.push("/usage")}>
            <Text style={[u.title, { flex: 1 }]}>Usage</Text>
            <Text style={u.sub}>{snap?.usage?.five ? `${Math.round(snap.usage.five.pct)}%` : "—"}</Text>
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
  gear: { color: c.text3, fontSize: 22 },
  chev: { color: c.text3, fontSize: 20, marginLeft: sp[1] },
  swatch: { width: 8, height: 26, borderRadius: r.one },
});
