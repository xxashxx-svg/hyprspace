// Automations, as far as a phone usefully goes: see what's scheduled, what's running, and start or
// stop one. Editing them stays on the desktop, where the prompt and stop conditions live.
import { useState } from "react";
import { Alert, ScrollView, StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import { useConn, type Automation } from "../src/store";
import { req } from "../src/rpc";
import { c, font, sp, t } from "../src/theme";
import { Btn, Card, Dot, Empty, Label, Row, s as u } from "../src/ui";
import { folderName, relTime, untilTime } from "../src/fmt";

const STATUS_COLOR: Record<string, string> = {
  running: c.busy,
  paused: c.awaiting,
  done: c.ok,
  error: c.error,
  crashloop: c.error,
  stopped: c.idle,
  idle: c.idle,
};

const MODE_LABEL: Record<string, string> = {
  cron: "Scheduled",
  interval: "Interval",
  manual: "On demand",
  "until-done": "On demand",
};

function line(a: Automation): string {
  const bits = [MODE_LABEL[a.mode] ?? a.mode, folderName(a.folder)];
  if (a.status === "running") bits.push("running now");
  else if (a.nextRunAt && a.enabled) bits.push(`next ${untilTime(a.nextRunAt)}`);
  else if (a.lastRunAt) bits.push(`last ran ${relTime(a.lastRunAt)}`);
  return bits.filter(Boolean).join(" · ");
}

export default function Automations() {
  const router = useRouter();
  const list = useConn((s) => s.snap?.automations ?? []);
  const online = useConn((s) => s.status === "online");
  const [busy, setBusy] = useState<string | null>(null);

  const act = async (a: Automation, what: "automation.run" | "automation.stop") => {
    setBusy(a.id);
    try {
      await req(what, { id: a.id });
    } catch (e) {
      Alert.alert("Couldn't do that", String((e as Error).message));
    } finally {
      setBusy(null);
    }
  };

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
      <View>
        <Label>Automations</Label>
        {list.length === 0 ? (
          <Card>
            <Empty
              title="None yet"
              hint="Set them up on the desktop — the Automations page in the sidebar."
            />
          </Card>
        ) : (
          <Card>
            {list.map((a, i) => {
              const running = a.status === "running";
              return (
                <View key={a.id}>
                  <Row last={i === list.length - 1}>
                    <Dot color={STATUS_COLOR[a.status] ?? c.idle} />
                    <View style={{ flex: 1, minWidth: 0 }}>
                      <Text style={u.title} numberOfLines={1}>
                        {a.name}
                      </Text>
                      <Text style={u.sub} numberOfLines={1}>
                        {line(a)}
                      </Text>
                      {!!a.lastResult && (
                        <Text style={au.result} numberOfLines={2}>
                          {a.lastResult}
                        </Text>
                      )}
                    </View>
                    <View style={au.actions}>
                      <Btn
                        onPress={() => void act(a, running ? "automation.stop" : "automation.run")}
                        disabled={!online || busy === a.id}
                        kind={running ? "danger" : "plain"}
                        style={au.actionBtn}
                      >
                        {busy === a.id ? "…" : running ? "Stop" : "Run"}
                      </Btn>
                      {running && !!a.paneId && (
                        <Btn onPress={() => router.push(`/term/${a.paneId}`)} style={au.actionBtn}>
                          Watch
                        </Btn>
                      )}
                    </View>
                  </Row>
                </View>
              );
            })}
          </Card>
        )}
      </View>

      <Text style={au.note}>
        Automations only run while HyprSpace is open on your desktop — starting one from here starts it
        there.
      </Text>
    </ScrollView>
  );
}

const au = StyleSheet.create({
  result: { color: c.text3, fontSize: t.xs, fontFamily: font.ui, marginTop: 2, lineHeight: 16 },
  actions: { gap: sp[1], alignItems: "stretch" },
  actionBtn: { paddingHorizontal: sp[3], paddingVertical: 6, minWidth: 62 },
  note: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19 },
});
