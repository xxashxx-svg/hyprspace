// Shared bits, so every screen reads the same. Deliberately small — the desktop look is mostly
// "neutral surfaces, hairline borders, one accent", which doesn't need much scaffolding.
import type { ReactNode } from "react";
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  type StyleProp,
  type ViewStyle,
} from "react-native";
import { c, font, r, sp, t } from "./theme";

export function Screen({ children, scroll = true }: { children: ReactNode; scroll?: boolean }) {
  if (!scroll) return <View style={s.screen}>{children}</View>;
  return (
    <ScrollView style={s.screen} contentContainerStyle={s.screenPad} keyboardShouldPersistTaps="handled">
      {children}
    </ScrollView>
  );
}

/** section label above a card — matches the desktop's `.set-label` */
export function Label({ children }: { children: ReactNode }) {
  return <Text style={s.label}>{children}</Text>;
}

export function Card({ children, style }: { children: ReactNode; style?: StyleProp<ViewStyle> }) {
  return <View style={[s.card, style]}>{children}</View>;
}

/** one tappable row inside a Card; rows divide themselves with a hairline, last one excepted */
export function Row({
  children,
  onPress,
  last,
}: {
  children: ReactNode;
  onPress?: () => void;
  last?: boolean;
}) {
  const body = <View style={[s.row, !last && s.rowDivider]}>{children}</View>;
  if (!onPress) return body;
  return (
    <Pressable onPress={onPress} android_ripple={{ color: c.accentDim }}>
      {body}
    </Pressable>
  );
}

export function Dot({ color, size = 8 }: { color: string; size?: number }) {
  return <View style={{ width: size, height: size, borderRadius: size, backgroundColor: color }} />;
}

export function Pill({ children, color = c.text3 }: { children: ReactNode; color?: string }) {
  return (
    <View style={[s.pill, { borderColor: color }]}>
      <Text style={[s.pillText, { color }]}>{children}</Text>
    </View>
  );
}

export function Btn({
  children,
  onPress,
  kind = "plain",
  disabled,
  style,
}: {
  children: ReactNode;
  onPress?: () => void;
  kind?: "plain" | "primary" | "danger";
  disabled?: boolean;
  style?: StyleProp<ViewStyle>;
}) {
  const tone =
    kind === "primary" ? s.btnPrimary : kind === "danger" ? s.btnDanger : s.btnPlain;
  const label =
    kind === "primary" ? s.btnLabelPrimary : kind === "danger" ? s.btnLabelDanger : s.btnLabel;
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      android_ripple={{ color: c.accentDim }}
      style={({ pressed }) => [s.btn, tone, disabled && s.btnOff, pressed && s.btnPressed, style]}
    >
      {typeof children === "string" ? <Text style={label}>{children}</Text> : children}
    </Pressable>
  );
}

export function Empty({ title, hint, children }: { title: string; hint?: string; children?: ReactNode }) {
  return (
    <View style={s.empty}>
      <Text style={s.emptyTitle}>{title}</Text>
      {!!hint && <Text style={s.emptyHint}>{hint}</Text>}
      {children}
    </View>
  );
}

export function Loading({ label }: { label?: string }) {
  return (
    <View style={s.empty}>
      <ActivityIndicator color={c.text3} />
      {!!label && <Text style={s.emptyHint}>{label}</Text>}
    </View>
  );
}

export const s = StyleSheet.create({
  screen: { flex: 1, backgroundColor: c.bg },
  screenPad: { padding: sp[4], paddingBottom: sp[6], gap: sp[5] },

  label: {
    color: c.text3,
    fontSize: t.xs,
    fontFamily: font.ui,
    textTransform: "uppercase",
    letterSpacing: 0.6,
    marginBottom: sp[2],
  },

  card: {
    backgroundColor: c.s2,
    borderRadius: r.two,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border1,
    overflow: "hidden",
  },

  row: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[3],
    paddingHorizontal: sp[4],
    paddingVertical: sp[3],
    minHeight: 52,
  },
  rowDivider: { borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: c.border1 },

  title: { color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },
  sub: { color: c.text3, fontSize: t.sm, fontFamily: font.ui },
  mono: { color: c.text2, fontSize: t.sm, fontFamily: font.mono },

  pill: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 999,
    paddingHorizontal: 7,
    paddingVertical: 1,
  },
  pillText: { fontSize: t.xs, fontFamily: font.ui },

  btn: {
    borderRadius: r.one,
    paddingHorizontal: sp[4],
    paddingVertical: 10,
    alignItems: "center",
    justifyContent: "center",
    borderWidth: StyleSheet.hairlineWidth,
  },
  btnPlain: { backgroundColor: c.s3, borderColor: c.border2 },
  btnPrimary: { backgroundColor: c.accent, borderColor: c.accent },
  btnDanger: { backgroundColor: "transparent", borderColor: c.error },
  btnOff: { opacity: 0.4 },
  btnPressed: { opacity: 0.75 },
  btnLabel: { color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },
  btnLabelPrimary: { color: c.onAccent, fontSize: t.md, fontFamily: font.uiMedium },
  btnLabelDanger: { color: c.error, fontSize: t.md, fontFamily: font.uiMedium },

  empty: { alignItems: "center", gap: sp[2], paddingVertical: sp[6], paddingHorizontal: sp[4] },
  emptyTitle: { color: c.text2, fontSize: t.md, fontFamily: font.uiMedium, textAlign: "center" },
  emptyHint: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, textAlign: "center", lineHeight: 19 },

  input: {
    backgroundColor: c.s2,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    borderRadius: r.one,
    color: c.text1,
    fontFamily: font.mono,
    fontSize: t.md,
    paddingHorizontal: sp[3],
    paddingVertical: 10,
  },
});
