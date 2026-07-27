// Your Claude rate-limit windows, mirrored from the desktop's usage meter. Read straight off the
// CLI's own status line there — display only, never an API call.
import { ScrollView, StyleSheet, Text, View } from "react-native";
import { useConn, type UsageWindow } from "../src/store";
import { c, font, r, sp, t } from "../src/theme";
import { Card, Empty, Label, s as u } from "../src/ui";
import { relTime, untilTime } from "../src/fmt";

function hot(pct: number) {
  return pct >= 90 ? c.error : pct >= 70 ? c.busy : c.accentHover;
}

function Meter({ label, win }: { label: string; win: UsageWindow }) {
  const pct = Math.max(0, Math.min(100, win.pct));
  return (
    <View style={m.meter}>
      <View style={m.meterTop}>
        <Text style={m.meterLabel}>{label}</Text>
        <Text style={[m.meterVal, { color: hot(pct) }]}>{Math.round(pct)}%</Text>
      </View>
      <View style={m.track}>
        <View style={[m.fill, { width: `${pct}%`, backgroundColor: hot(pct) }]} />
      </View>
      {!!win.resetsAt && <Text style={m.reset}>resets {untilTime(win.resetsAt)}</Text>}
    </View>
  );
}

export default function Usage() {
  const usage = useConn((s) => s.snap?.usage ?? null);

  if (!usage) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        <Card>
          <Empty
            title="Nothing reported yet"
            hint="Numbers show up once a Claude pane on the desktop has taken a turn."
          />
        </Card>
      </ScrollView>
    );
  }

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
      <View>
        <Label>Limits</Label>
        <Card>
          <View style={m.pad}>
            {usage.five && <Meter label="Session · 5h" win={usage.five} />}
            {usage.others.map((o) => (
              <Meter key={o.key} label={o.label} win={o.win} />
            ))}
            {!usage.five && usage.others.length === 0 && (
              <Text style={u.sub}>This plan doesn't report windows.</Text>
            )}
          </View>
        </Card>
      </View>

      {usage.models.length > 0 && (
        <View>
          <Label>In use</Label>
          <Card>
            <View style={m.pad}>
              {usage.models.map((mo) => (
                <Text key={mo} style={u.mono}>
                  {mo}
                </Text>
              ))}
            </View>
          </Card>
        </View>
      )}

      <Text style={m.note}>
        {usage.stale ? "No recent reports — " : ""}last update {relTime(usage.at)} ago.
      </Text>
    </ScrollView>
  );
}

const m = StyleSheet.create({
  pad: { padding: sp[4], gap: sp[4] },
  meter: { gap: sp[2] },
  meterTop: { flexDirection: "row", alignItems: "baseline", justifyContent: "space-between" },
  meterLabel: { color: c.text2, fontSize: t.sm, fontFamily: font.ui },
  meterVal: { fontSize: t.md, fontFamily: font.mono },
  track: { height: 6, borderRadius: r.one, backgroundColor: c.s3, overflow: "hidden" },
  fill: { height: "100%", borderRadius: r.one },
  reset: { color: c.text3, fontSize: t.xs, fontFamily: font.ui },
  note: { color: c.text3, fontSize: t.sm, fontFamily: font.ui },
});
