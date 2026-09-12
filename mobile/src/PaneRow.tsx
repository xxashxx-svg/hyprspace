// A thread, laid out exactly like the desktop sidebar's row (src/components/SessionRow.tsx): the
// model and how long ago it spoke on top, the thread's name in the middle, and what it is doing
// underneath. Sub-agents it has running appear as their own small cards below it.
import { Pressable, StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import type { Pane } from "./store";
import { c, font, providerColor, providerLabel, sp, stateColor, stateLabel, t } from "./theme";
import { LiveDot, tint } from "./ui";
import { relTime } from "./fmt";

export function PaneRow({ pane, sub, last }: { pane: Pane; sub?: string; last?: boolean }) {
  const router = useRouter();
  const tone = stateColor[pane.state] ?? c.idle;
  const mark = providerColor[pane.provider] ?? c.text3;
  const label = pane.model || providerLabel[pane.provider] || pane.provider;
  const foot = sub ?? pane.activity ?? stateLabel[pane.state] ?? pane.state;
  const subs = pane.subAgents ?? [];

  return (
    <Pressable
      onPress={() => router.push(`/term/${pane.id}`)}
      android_ripple={{ color: c.accentDim }}
      style={[p.row, !last && p.divider, pane.state === "waiting" && { backgroundColor: tint(c.awaiting, 0.07) }]}
    >
      <View style={p.top}>
        <View style={[p.mark, { backgroundColor: mark }]} />
        <Text style={p.model} numberOfLines={1}>
          {label}
        </Text>
        {pane.subs > 0 && (
          <View style={p.subsChip}>
            <Text style={p.subsChipText}>{pane.subs}</Text>
          </View>
        )}
        {!pane.started && <Text style={p.model}>not started</Text>}
        <View style={{ flex: 1 }} />
        {!!pane.at && <Text style={p.time}>{relTime(pane.at)}</Text>}
      </View>

      <Text style={p.name} numberOfLines={1}>
        {pane.title}
      </Text>

      <View style={p.foot}>
        <Text style={[p.act, pane.state === "waiting" && { color: c.awaiting }]} numberOfLines={1}>
          {foot}
        </Text>
        <LiveDot color={tone} size={6} on={pane.state === "working"} />
      </View>

      {subs.length > 0 && (
        <View style={p.subs}>
          {subs.map((sa) => (
            <View key={sa.id} style={p.sub}>
              <View style={[p.subMark, { backgroundColor: sa.state === "working" ? c.busy : c.text3 }]} />
              <Text style={p.subLabel} numberOfLines={1}>
                {sa.label}
              </Text>
              {!!sa.at && <Text style={p.time}>{relTime(sa.at)}</Text>}
            </View>
          ))}
        </View>
      )}
    </Pressable>
  );
}

const p = StyleSheet.create({
  row: { paddingHorizontal: sp[4], paddingVertical: 9, gap: 2 },
  divider: { borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: c.border1 },

  top: { flexDirection: "row", alignItems: "center", gap: 6 },
  mark: { width: 7, height: 7, borderRadius: 4 },
  model: { color: c.text3, fontSize: t.xs, fontFamily: font.mono },
  time: { color: c.text3, fontSize: t.xs, fontFamily: font.mono },
  subsChip: {
    backgroundColor: "rgba(245,158,11,0.16)",
    borderRadius: 4,
    paddingHorizontal: 5,
    paddingVertical: 1,
  },
  subsChipText: { color: c.busy, fontSize: 10, fontFamily: font.mono },

  name: { color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },

  foot: { flexDirection: "row", alignItems: "center", gap: sp[2] },
  act: { flex: 1, color: c.text3, fontSize: t.sm, fontFamily: font.ui },

  // sub-agents sit inside the thread's own row, the way they do on the desktop
  subs: { marginTop: 5, gap: 3 },
  sub: {
    flexDirection: "row",
    alignItems: "center",
    gap: 7,
    paddingHorizontal: 8,
    paddingVertical: 5,
    borderRadius: 6,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border1,
    backgroundColor: c.s2,
  },
  subMark: { width: 6, height: 6, borderRadius: 3 },
  subLabel: { flex: 1, color: c.text2, fontSize: t.xs, fontFamily: font.ui },
});
