import { StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import type { Pane } from "./store";
import { c, providerLabel, stateColor, stateLabel } from "./theme";
import { Dot, Row, s as u } from "./ui";

/** A pane in a list: status dot, name, and one line of what it's doing. Taps into the terminal. */
export function PaneRow({ pane, sub, last }: { pane: Pane; sub?: string; last?: boolean }) {
  const router = useRouter();
  const line =
    sub ??
    `${providerLabel[pane.provider] ?? pane.provider}${
      pane.activity ? ` · ${pane.activity}` : ` · ${stateLabel[pane.state]}`
    }${pane.subs > 0 ? ` · ${pane.subs} sub-agent${pane.subs > 1 ? "s" : ""}` : ""}`;

  return (
    <Row last={last} onPress={() => router.push(`/term/${pane.id}`)}>
      <Dot color={stateColor[pane.state] ?? c.idle} />
      <View style={{ flex: 1, minWidth: 0 }}>
        <Text style={u.title} numberOfLines={1}>
          {pane.title}
        </Text>
        <Text style={u.sub} numberOfLines={1}>
          {line}
        </Text>
      </View>
      <Text style={p.chev}>›</Text>
    </Row>
  );
}

const p = StyleSheet.create({ chev: { color: c.text3, fontSize: 20, marginLeft: 4 } });
