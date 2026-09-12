// Failures that aren't worth a dialog. A platform Alert stops everything and looks like a different
// app, so anything the user can't act on immediately ("couldn't launch", "commit failed") slides up
// here instead and gets out of the way on its own.
import { useEffect, useRef } from "react";
import { Animated, Easing, Pressable, StyleSheet, Text, View } from "react-native";
import { create } from "zustand";
import { c, font, r, sp, t } from "./theme";

type Kind = "error" | "info";

interface Item {
  id: number;
  kind: Kind;
  text: string;
}

interface ToastState {
  items: Item[];
  push: (kind: Kind, text: string) => void;
  drop: (id: number) => void;
}

/** at most this many on screen; older ones fall off the top rather than filling the display */
const MAX = 3;
const LIFE_MS = 4500;

let seq = 0;

const useToasts = create<ToastState>()((set) => ({
  items: [],
  push: (kind, text) =>
    set((s) => ({ items: [...s.items, { id: ++seq, kind, text }].slice(-MAX) })),
  drop: (id) => set((s) => ({ items: s.items.filter((i) => i.id !== id) })),
}));

/** `toast.error(e)` from anywhere — no hook, no provider, works inside a catch. */
export const toast = {
  error: (e: unknown) => useToasts.getState().push("error", message(e)),
  info: (text: string) => useToasts.getState().push("info", text),
};

function message(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message || "Something went wrong";
  return String(e ?? "Something went wrong");
}

function Toast({ item }: { item: Item }) {
  const v = useRef(new Animated.Value(0)).current;
  const drop = useToasts((s) => s.drop);

  useEffect(() => {
    Animated.timing(v, { toValue: 1, duration: 160, easing: Easing.out(Easing.quad), useNativeDriver: true }).start();
    const timer = setTimeout(() => {
      Animated.timing(v, { toValue: 0, duration: 140, easing: Easing.in(Easing.quad), useNativeDriver: true }).start(
        () => drop(item.id),
      );
    }, LIFE_MS);
    return () => clearTimeout(timer);
  }, [item.id, v, drop]);

  return (
    <Animated.View
      style={[
        s.toast,
        item.kind === "error" && s.error,
        { opacity: v, transform: [{ translateY: v.interpolate({ inputRange: [0, 1], outputRange: [12, 0] }) }] },
      ]}
    >
      <Pressable onPress={() => drop(item.id)} style={s.press}>
        <Text style={[s.text, item.kind === "error" && s.errorText]} numberOfLines={4}>
          {item.text}
        </Text>
      </Pressable>
    </Animated.View>
  );
}

/** Mounted once, in the root layout, under everything else. */
export function Toaster() {
  const items = useToasts((s) => s.items);
  if (!items.length) return null;
  return (
    <View style={s.wrap} pointerEvents="box-none">
      {items.map((i) => (
        <Toast key={i.id} item={i} />
      ))}
    </View>
  );
}

const s = StyleSheet.create({
  wrap: { paddingHorizontal: sp[4], paddingBottom: sp[2], gap: sp[2] },
  toast: {
    backgroundColor: c.s3,
    borderRadius: r.two,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
  },
  error: { backgroundColor: "rgba(239,68,68,0.14)", borderColor: "rgba(239,68,68,0.4)" },
  press: { paddingHorizontal: sp[4], paddingVertical: sp[3] },
  text: { color: c.text1, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19 },
  errorText: { color: "#ffb3ad" },
});
