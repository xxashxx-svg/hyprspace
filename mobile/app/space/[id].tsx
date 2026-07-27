// One space: its panes, plus the actions worth having on a phone — wake it, launch an agent, look at
// what changed.
import { useEffect, useState } from "react";
import { Alert, ScrollView, StyleSheet, Text, View } from "react-native";
import { useLocalSearchParams, useNavigation, useRouter } from "expo-router";
import { useConn, useSpace } from "../../src/store";
import { req } from "../../src/rpc";
import { c, font, sp, t } from "../../src/theme";
import { Btn, Card, Empty, Label, Loading, Row, s as u } from "../../src/ui";
import { PaneRow } from "../../src/PaneRow";
import { shortPath } from "../../src/fmt";

const AGENTS = ["claude", "codex", "gemini", "terminal"] as const;

export default function SpaceScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const nav = useNavigation();
  const router = useRouter();
  const space = useSpace(id);
  const online = useConn((s) => s.status === "online");
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (space) nav.setOptions({ title: space.name });
  }, [nav, space]);

  if (!space) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        {online ? <Empty title="That space is gone" hint="It was closed on the desktop." /> : <Loading />}
      </ScrollView>
    );
  }

  const launch = async (provider: string) => {
    setBusy(provider);
    try {
      const res = await req<{ pane: string }>("space.launch", { ws: space.id, provider });
      router.push(`/term/${res.pane}`);
    } catch (e) {
      Alert.alert("Couldn't launch", String((e as Error).message));
    } finally {
      setBusy(null);
    }
  };

  const wake = async () => {
    setBusy("wake");
    try {
      await req("space.activate", { ws: space.id });
    } catch (e) {
      Alert.alert("Couldn't wake it", String((e as Error).message));
    } finally {
      setBusy(null);
    }
  };

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
      <View>
        <Text style={sc.folder}>{space.kind === "open" ? "Open space" : shortPath(space.cwd) || "No folder"}</Text>
        {!space.activated && (
          <Text style={sc.asleep}>
            This space hasn't been opened yet, so its panes aren't running. Waking it starts them on the
            desktop.
          </Text>
        )}
      </View>

      {!space.activated && (
        <Btn kind="primary" onPress={() => void wake()} disabled={busy === "wake"}>
          {busy === "wake" ? "Waking…" : "Wake this space"}
        </Btn>
      )}

      <View>
        <Label>Panes</Label>
        {space.panes.length === 0 ? (
          <Card>
            <Empty title="No panes" hint="Launch one below." />
          </Card>
        ) : (
          <Card>
            {space.panes.map((p, i) => (
              <PaneRow key={p.id} pane={p} last={i === space.panes.length - 1} />
            ))}
          </Card>
        )}
      </View>

      <View>
        <Label>Launch</Label>
        <View style={sc.grid}>
          {AGENTS.map((a) => (
            <Btn
              key={a}
              onPress={() => void launch(a)}
              disabled={!!busy || !online}
              style={sc.gridBtn}
            >
              {busy === a ? "…" : a === "terminal" ? "Shell" : a[0].toUpperCase() + a.slice(1)}
            </Btn>
          ))}
        </View>
      </View>

      {space.kind !== "open" && !!space.cwd && (
        <View>
          <Label>Repo</Label>
          <Card>
            <Row last onPress={() => router.push(`/git/${space.id}`)}>
              <Text style={[u.title, { flex: 1 }]}>Changes</Text>
              <Text style={sc.chev}>›</Text>
            </Row>
          </Card>
        </View>
      )}
    </ScrollView>
  );
}

const sc = StyleSheet.create({
  folder: { color: c.text2, fontSize: t.sm, fontFamily: font.mono },
  asleep: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19, marginTop: sp[2] },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: sp[2] },
  gridBtn: { flexGrow: 1, flexBasis: "45%" },
  chev: { color: c.text3, fontSize: 20 },
});
