// A space in the home list, shaped like the desktop sidebar: a header that folds open to the
// threads underneath. The plus starts a thread the way the desktop's does, through the composer.
import { useState } from "react";
import { Pressable, StyleSheet, Text, View } from "react-native";
import { useRouter } from "expo-router";
import type { Space } from "./store";
import { req } from "./rpc";
import { c, font, sp, t } from "./theme";
import { PaneRow } from "./PaneRow";
import { animateNext, Dot, LiveDot, Twist } from "./ui";
import { toast } from "./toast";

export function SpaceSection({
  space,
  open,
  onToggle,
}: {
  space: Space;
  open: boolean;
  onToggle: () => void;
}) {
  const router = useRouter();
  const [waking, setWaking] = useState(false);
  const waiting = space.panes.some((p) => p.state === "waiting");
  const working = space.panes.some((p) => p.state === "working");

  const wake = async () => {
    setWaking(true);
    try {
      await req("space.activate", { ws: space.id });
    } catch (e) {
      toast.error(e);
    } finally {
      setWaking(false);
    }
  };

  return (
    <View>
      <Pressable
        onPress={() => {
          animateNext();
          onToggle();
        }}
        android_ripple={{ color: c.accentDim }}
        style={s.head}
      >
        <Twist open={open} />
        <Text style={s.name} numberOfLines={1}>
          {space.name}
        </Text>
        {waiting ? <Dot color={c.awaiting} size={7} /> : working ? <LiveDot color={c.busy} size={7} /> : null}
        <Pressable
          onPress={() => router.push(`/compose?ws=${space.id}`)}
          hitSlop={10}
          style={s.plus}
          android_ripple={{ color: c.accentDim, radius: 18 }}
        >
          <Text style={s.plusText}>+</Text>
        </Pressable>
        <Text style={s.count}>{space.panes.length}</Text>
      </Pressable>

      {open && (
        <View style={s.body}>
          {space.panes.length === 0 ? (
            <Text style={s.none}>No threads yet</Text>
          ) : (
            space.panes.map((p, i) => (
              <PaneRow key={p.id} pane={p} last={i === space.panes.length - 1 && space.activated && !space.cwd} />
            ))
          )}

          {!space.activated && (
            <Pressable onPress={() => void wake()} android_ripple={{ color: c.accentDim }} style={s.action}>
              <Text style={s.actionText}>{waking ? "Waking" : "Wake this space"}</Text>
              <Text style={s.hint}>its threads are not running yet</Text>
            </Pressable>
          )}

          {!!space.cwd && (
            <Pressable
              onPress={() => router.push(`/git/${space.id}`)}
              android_ripple={{ color: c.accentDim }}
              style={s.action}
            >
              <Text style={s.actionText}>Changes</Text>
              <Text style={s.chev}>›</Text>
            </Pressable>
          )}
        </View>
      )}
    </View>
  );
}

const s = StyleSheet.create({
  head: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[2],
    paddingVertical: 11,
    paddingRight: sp[1],
    minHeight: 44,
  },
  name: { flex: 1, minWidth: 0, color: c.text1, fontSize: t.md, fontFamily: font.uiMedium },
  plus: { width: 28, height: 28, alignItems: "center", justifyContent: "center" },
  plusText: { color: c.text3, fontSize: 20, lineHeight: 22 },
  count: { color: c.text3, fontSize: t.sm, fontFamily: font.mono, minWidth: 14, textAlign: "right" },

  // the threads sit indented under their space, the way the desktop nests them
  body: { marginLeft: sp[3], marginBottom: sp[3], borderRadius: 10, backgroundColor: c.s2, overflow: "hidden" },
  none: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, padding: sp[4] },

  action: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[2],
    paddingHorizontal: sp[4],
    paddingVertical: 11,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },
  actionText: { color: c.text2, fontSize: t.sm, fontFamily: font.ui },
  hint: { flex: 1, color: c.text3, fontSize: t.xs, fontFamily: font.ui, textAlign: "right" },
  chev: { flex: 1, color: c.text3, fontSize: 18, textAlign: "right" },
});
