// Shared bits, so every screen reads the same. Deliberately small — the desktop look is mostly
// "neutral surfaces, hairline borders, one accent", which doesn't need much scaffolding.
//
// Buttons and dialogs here mirror the desktop's current design (src/styles/layout.css `.btn`,
// src/styles/menus.css). Nothing in this app uses a platform Alert: a system dialog looks nothing
// like the rest of the app, so confirms go through `Dialog` and failures through the toaster.
import { useEffect, useRef, type ReactNode } from "react";
import {
  ActivityIndicator,
  Animated,
  Easing,
  LayoutAnimation,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  UIManager,
  View,
  type StyleProp,
  type ViewStyle,
} from "react-native";
import { c, font, r, sp, t } from "./theme";

// Android needs this switched on before LayoutAnimation does anything. Absent under the new
// architecture, where it is already on, hence the guard rather than a bare call.
if (Platform.OS === "android" && UIManager.setLayoutAnimationEnabledExperimental) {
  UIManager.setLayoutAnimationEnabledExperimental(true);
}

/** Ease the next layout pass. Used by anything that folds open, so rows slide instead of popping. */
export function animateNext() {
  LayoutAnimation.configureNext(LayoutAnimation.create(160, "easeInEaseOut", "opacity"));
}

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
  style,
}: {
  children: ReactNode;
  onPress?: () => void;
  last?: boolean;
  style?: StyleProp<ViewStyle>;
}) {
  const body = <View style={[s.row, !last && s.rowDivider, style]}>{children}</View>;
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

/** A dot that breathes while something is running, so a working pane is obvious in a long list. */
export function LiveDot({ color, size = 8, on = true }: { color: string; size?: number; on?: boolean }) {
  const v = useRef(new Animated.Value(1)).current;
  useEffect(() => {
    if (!on) {
      v.setValue(1);
      return;
    }
    const loop = Animated.loop(
      Animated.sequence([
        Animated.timing(v, { toValue: 0.3, duration: 700, easing: Easing.inOut(Easing.quad), useNativeDriver: true }),
        Animated.timing(v, { toValue: 1, duration: 700, easing: Easing.inOut(Easing.quad), useNativeDriver: true }),
      ]),
    );
    loop.start();
    return () => loop.stop();
  }, [on, v]);
  return (
    <Animated.View
      style={{ width: size, height: size, borderRadius: size, backgroundColor: color, opacity: v }}
    />
  );
}

/** the fold arrow on a section header; rotates rather than swapping glyphs */
export function Twist({ open }: { open: boolean }) {
  const v = useRef(new Animated.Value(open ? 1 : 0)).current;
  useEffect(() => {
    Animated.timing(v, { toValue: open ? 1 : 0, duration: 150, easing: Easing.out(Easing.quad), useNativeDriver: true }).start();
  }, [open, v]);
  const rotate = v.interpolate({ inputRange: [0, 1], outputRange: ["0deg", "90deg"] });
  return (
    <Animated.View style={{ transform: [{ rotate }], width: 14, alignItems: "center" }}>
      <Text style={s.twist}>›</Text>
    </Animated.View>
  );
}

export function Pill({ children, color = c.text3 }: { children: ReactNode; color?: string }) {
  return (
    <View style={[s.pill, { borderColor: color }]}>
      <Text style={[s.pillText, { color }]}>{children}</Text>
    </View>
  );
}

/** a filled count tag — the desktop's sub-agent chip on a thread row */
export function Chip({ children, color = c.text3 }: { children: ReactNode; color?: string }) {
  return (
    <View style={[s.chip, { backgroundColor: tint(color, 0.16) }]}>
      <Text style={[s.chipText, { color }]}>{children}</Text>
    </View>
  );
}

/** #rrggbb → rgba at the given alpha. The palette is all hex, so this covers it. */
export function tint(hex: string, alpha: number): string {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return hex;
  const n = parseInt(m[1], 16);
  return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${alpha})`;
}

export function Btn({
  children,
  onPress,
  kind = "plain",
  size = "md",
  disabled,
  style,
}: {
  children: ReactNode;
  onPress?: () => void;
  kind?: "plain" | "primary" | "danger" | "ghost";
  size?: "md" | "sm";
  disabled?: boolean;
  style?: StyleProp<ViewStyle>;
}) {
  const tone =
    kind === "primary" ? s.btnPrimary : kind === "danger" ? s.btnDanger : kind === "ghost" ? s.btnGhost : s.btnPlain;
  const label =
    kind === "primary"
      ? s.btnLabelPrimary
      : kind === "danger"
        ? s.btnLabelDanger
        : kind === "ghost"
          ? s.btnLabelGhost
          : s.btnLabel;
  return (
    <Pressable
      onPress={onPress}
      disabled={disabled}
      android_ripple={{ color: c.accentDim }}
      style={({ pressed }) => [
        s.btn,
        size === "sm" && s.btnSm,
        tone,
        disabled && s.btnOff,
        pressed && s.btnPressed,
        style,
      ]}
    >
      {typeof children === "string" ? (
        <Text style={[label, size === "sm" && s.btnLabelSm]}>{children}</Text>
      ) : (
        children
      )}
    </Pressable>
  );
}

/**
 * The app's own confirm, in place of a platform Alert. Same surface, border and buttons as every
 * other panel, so a destructive question doesn't suddenly look like a different app.
 */
export function Dialog({
  open,
  title,
  message,
  confirm = "Confirm",
  cancel = "Cancel",
  danger,
  onConfirm,
  onClose,
}: {
  open: boolean;
  title: string;
  message?: string;
  confirm?: string;
  cancel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  return (
    <Modal visible={open} transparent animationType="fade" statusBarTranslucent onRequestClose={onClose}>
      <Pressable style={s.dlgBack} onPress={onClose}>
        {/* swallow taps on the card so only the backdrop dismisses */}
        <Pressable style={s.dlg} onPress={() => {}}>
          <Text style={s.dlgTitle}>{title}</Text>
          {!!message && <Text style={s.dlgMsg}>{message}</Text>}
          <View style={s.dlgActions}>
            <Btn onPress={onClose} style={{ flex: 1 }}>
              {cancel}
            </Btn>
            <Btn kind={danger ? "danger" : "primary"} onPress={onConfirm} style={{ flex: 1 }}>
              {confirm}
            </Btn>
          </View>
        </Pressable>
      </Pressable>
    </Modal>
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

  twist: { color: c.text3, fontSize: 17, lineHeight: 19 },

  pill: {
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 999,
    paddingHorizontal: 7,
    paddingVertical: 1,
  },
  pillText: { fontSize: t.xs, fontFamily: font.ui },

  chip: { borderRadius: r.one, paddingHorizontal: 6, paddingVertical: 2 },
  chipText: { fontSize: t.xs, fontFamily: font.mono },

  // the desktop's .btn: a quiet surface with a hairline, and one filled accent for the main action
  btn: {
    borderRadius: r.one,
    paddingHorizontal: sp[4],
    paddingVertical: 11,
    alignItems: "center",
    justifyContent: "center",
    borderWidth: StyleSheet.hairlineWidth,
  },
  btnSm: { paddingHorizontal: sp[3], paddingVertical: 7 },
  btnPlain: { backgroundColor: c.s2, borderColor: c.border2 },
  btnPrimary: { backgroundColor: c.accent, borderColor: c.accent },
  btnDanger: { backgroundColor: "rgba(239,68,68,0.12)", borderColor: "rgba(239,68,68,0.45)" },
  btnGhost: { backgroundColor: "transparent", borderColor: "transparent" },
  btnOff: { opacity: 0.4 },
  btnPressed: { opacity: 0.75 },
  btnLabel: { color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },
  btnLabelPrimary: { color: c.onAccent, fontSize: t.md, fontFamily: font.uiMedium },
  btnLabelDanger: { color: c.error, fontSize: t.md, fontFamily: font.uiMedium },
  btnLabelGhost: { color: c.text2, fontSize: t.md, fontFamily: font.ui },
  btnLabelSm: { fontSize: t.sm },

  dlgBack: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.6)",
    alignItems: "center",
    justifyContent: "center",
    padding: sp[5],
  },
  dlg: {
    width: "100%",
    maxWidth: 380,
    backgroundColor: c.s2,
    borderRadius: r.three,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    padding: sp[4],
    gap: sp[2],
  },
  dlgTitle: { color: c.text1, fontSize: t.lg, fontFamily: font.uiMedium },
  dlgMsg: { color: c.text3, fontSize: t.md, fontFamily: font.ui, lineHeight: 20 },
  dlgActions: { flexDirection: "row", gap: sp[2], marginTop: sp[3] },

  empty: { alignItems: "center", gap: sp[2], paddingVertical: sp[6], paddingHorizontal: sp[4] },
  emptyTitle: { color: c.text2, fontSize: t.md, fontFamily: font.uiMedium, textAlign: "center" },
  emptyHint: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, textAlign: "center", lineHeight: 19 },

  input: {
    backgroundColor: c.bg,
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
