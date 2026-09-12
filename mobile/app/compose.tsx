// Start a thread, the same way the desktop's composer does: pick the space, the agent and how hard
// it should think, type the task, send. The desktop builds the launch command and types the task in
// once the CLI is up (mobileBridge.ts, space.launch).
import { useCallback, useEffect, useState } from "react";
import { Modal, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useConn } from "../src/store";
import { req } from "../src/rpc";
import { c, font, providerColor, r, sp, t } from "../src/theme";
import { Card, Label, Loading, s as u } from "../src/ui";
import { toast } from "../src/toast";
import { folderName, relTime } from "../src/fmt";

interface Agent {
  id: string;
  label: string;
  installed: boolean;
  models: { id: string; label: string }[];
  efforts: string[];
}
interface Saved {
  id: string;
  title: string;
  modified: number;
}

/** the agents whose saved conversations can be reopened by id */
const CAN_RESUME = new Set(["claude", "codex"]);

/** a chip in the composer's control rows */
function Chip({ label, value, onPress }: { label?: string; value: string; onPress: () => void }) {
  return (
    <Pressable onPress={onPress} android_ripple={{ color: c.accentDim }} style={s.chip}>
      {!!label && <Text style={s.chipLabel}>{label}</Text>}
      <Text style={s.chipValue} numberOfLines={1}>
        {value}
      </Text>
      <Text style={s.chipCaret}>⌄</Text>
    </Pressable>
  );
}

/** a bottom sheet of choices, in place of a platform picker */
function Sheet<T extends { key: string; label: string; sub?: string; on?: boolean }>({
  open,
  title,
  items,
  onPick,
  onClose,
}: {
  open: boolean;
  title: string;
  items: T[];
  onPick: (key: string) => void;
  onClose: () => void;
}) {
  return (
    <Modal visible={open} transparent animationType="slide" statusBarTranslucent onRequestClose={onClose}>
      <Pressable style={s.sheetBack} onPress={onClose}>
        <Pressable style={s.sheet} onPress={() => {}}>
          <Text style={s.sheetTitle}>{title}</Text>
          <ScrollView style={{ maxHeight: 380 }}>
            {items.map((it) => (
              <Pressable
                key={it.key}
                onPress={() => {
                  onPick(it.key);
                  onClose();
                }}
                android_ripple={{ color: c.accentDim }}
                style={s.sheetRow}
              >
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Text style={s.sheetLabel} numberOfLines={1}>
                    {it.label}
                  </Text>
                  {!!it.sub && (
                    <Text style={s.sheetSub} numberOfLines={1}>
                      {it.sub}
                    </Text>
                  )}
                </View>
                {it.on && <Text style={s.sheetTick}>✓</Text>}
              </Pressable>
            ))}
          </ScrollView>
        </Pressable>
      </Pressable>
    </Modal>
  );
}

export default function Compose() {
  const router = useRouter();
  const { ws } = useLocalSearchParams<{ ws?: string }>();
  const spaces = useConn((st) => st.snap?.spaces ?? []);
  const online = useConn((st) => st.status === "online");

  const [wsId, setWsId] = useState(ws ?? "");
  const space = spaces.find((x) => x.id === wsId) ?? spaces[0];

  const [text, setText] = useState("");
  const [agents, setAgents] = useState<Agent[]>([]);
  const [provider, setProvider] = useState("claude");
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  const [sheet, setSheet] = useState<"space" | "agent" | "effort" | null>(null);
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<Saved[]>([]);

  const agent = agents.find((a) => a.id === provider);

  useEffect(() => {
    if (!online) return;
    req<{ agents: Agent[] }>("agents.catalog")
      .then((r) => setAgents(r.agents.filter((a) => a.installed)))
      .catch(() => {});
  }, [online]);

  // the saved conversations for this folder, so a thread can be picked up rather than restarted
  const loadSaved = useCallback(() => {
    if (!online || !space?.cwd || !CAN_RESUME.has(provider)) return setSaved([]);
    req<{ sessions: Saved[] }>("agent.sessions", { provider, cwd: space.cwd })
      .then((r) => setSaved(r.sessions ?? []))
      .catch(() => setSaved([]));
  }, [online, space?.cwd, provider]);
  useEffect(loadSaved, [loadSaved]);

  const start = async (resume?: string) => {
    if (!space || busy) return;
    setBusy(true);
    try {
      const res = await req<{ pane: string }>("space.launch", {
        ws: space.id,
        provider,
        model,
        effort,
        prompt: text.trim(),
        resume: resume ?? "",
      });
      router.replace(`/term/${res.pane}`);
    } catch (e) {
      toast.error(e);
      setBusy(false);
    }
  };

  if (!space) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        {online ? <Text style={u.sub}>No spaces yet. Open a folder on the desktop first.</Text> : <Loading />}
      </ScrollView>
    );
  }

  const modelLabel = agent?.models.find((m) => m.id === model)?.label ?? "Default";

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad} keyboardShouldPersistTaps="handled">
      <Text style={s.title}>What should we work on in {space.name}?</Text>

      <View style={s.box}>
        <View style={s.chips}>
          <Chip value={space.name} onPress={() => setSheet("space")} />
          {!!space.cwd && <Text style={s.folder}>{folderName(space.cwd)}</Text>}
        </View>

        <TextInput
          style={s.input}
          value={text}
          onChangeText={setText}
          placeholder="Describe the task, or leave empty to open the agent"
          placeholderTextColor={c.text3}
          multiline
          textAlignVertical="top"
        />

        <View style={s.bottom}>
          <Chip value={`${agent?.label ?? provider}${model ? ` · ${modelLabel}` : ""}`} onPress={() => setSheet("agent")} />
          {!!agent?.efforts.length && <Chip value={effort || "Effort"} onPress={() => setSheet("effort")} />}
          <View style={{ flex: 1 }} />
          <Pressable
            onPress={() => void start()}
            disabled={busy || !online}
            android_ripple={{ color: c.accentDim, radius: 22 }}
            style={[s.send, (busy || !online) && { opacity: 0.5 }]}
          >
            <Text style={s.sendText}>↑</Text>
          </Pressable>
        </View>
      </View>

      {saved.length > 0 && (
        <View>
          <Label>Continue a {agent?.label ?? provider} thread</Label>
          <Card>
            {saved.map((sv, i) => (
              <Pressable
                key={sv.id}
                onPress={() => void start(sv.id)}
                android_ripple={{ color: c.accentDim }}
                style={[s.savedRow, i < saved.length - 1 && s.savedDivider]}
              >
                <Text style={s.savedTitle} numberOfLines={1}>
                  {sv.title}
                </Text>
                <Text style={s.savedTime}>{relTime(sv.modified)}</Text>
              </Pressable>
            ))}
          </Card>
        </View>
      )}

      <Sheet
        open={sheet === "space"}
        title="Space"
        items={spaces.map((x) => ({ key: x.id, label: x.name, sub: folderName(x.cwd), on: x.id === space.id }))}
        onPick={setWsId}
        onClose={() => setSheet(null)}
      />
      <Sheet
        open={sheet === "agent"}
        title="Agent and model"
        items={agents.flatMap((a) => [
          { key: `${a.id}|`, label: a.label, sub: "the CLI's own default", on: a.id === provider && !model },
          ...a.models.map((m) => ({
            key: `${a.id}|${m.id}`,
            label: `${a.label} · ${m.label}`,
            on: a.id === provider && m.id === model,
          })),
        ])}
        onPick={(k) => {
          const [pid, mid] = k.split("|");
          setProvider(pid);
          setModel(mid ?? "");
        }}
        onClose={() => setSheet(null)}
      />
      <Sheet
        open={sheet === "effort"}
        title="Effort"
        items={[
          { key: "", label: "Default", sub: "whatever the CLI is set to", on: !effort },
          ...(agent?.efforts ?? []).map((e) => ({ key: e, label: e, on: e === effort })),
        ]}
        onPick={setEffort}
        onClose={() => setSheet(null)}
      />
    </ScrollView>
  );
}

const s = StyleSheet.create({
  title: { color: c.text1, fontSize: t.xl, fontFamily: font.uiMedium, textAlign: "center" },

  box: {
    backgroundColor: c.s2,
    borderRadius: r.three,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    overflow: "hidden",
  },
  chips: { flexDirection: "row", alignItems: "center", gap: sp[2], padding: sp[3], paddingBottom: 0 },
  folder: { color: c.text3, fontSize: t.xs, fontFamily: font.mono },
  input: {
    color: c.text1,
    fontFamily: font.ui,
    fontSize: t.lg,
    paddingHorizontal: sp[4],
    paddingTop: sp[3],
    paddingBottom: sp[3],
    minHeight: 96,
  },
  bottom: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[2],
    padding: sp[3],
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
  },

  chip: {
    flexDirection: "row",
    alignItems: "center",
    gap: 5,
    maxWidth: 210,
    paddingHorizontal: sp[3],
    paddingVertical: 7,
    borderRadius: 999,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    backgroundColor: c.bg,
  },
  chipLabel: { color: c.text3, fontSize: t.xs, fontFamily: font.ui },
  chipValue: { color: c.text1, fontSize: t.sm, fontFamily: font.ui, flexShrink: 1 },
  chipCaret: { color: c.text3, fontSize: t.xs },

  send: {
    width: 40,
    height: 40,
    borderRadius: 12,
    backgroundColor: c.accent,
    alignItems: "center",
    justifyContent: "center",
  },
  sendText: { color: c.onAccent, fontSize: 20, lineHeight: 22 },

  savedRow: { flexDirection: "row", alignItems: "center", gap: sp[3], paddingHorizontal: sp[4], paddingVertical: 12 },
  savedDivider: { borderBottomWidth: StyleSheet.hairlineWidth, borderBottomColor: c.border1 },
  savedTitle: { flex: 1, color: c.text2, fontSize: t.sm, fontFamily: font.ui },
  savedTime: { color: c.text3, fontSize: t.xs, fontFamily: font.mono },

  sheetBack: { flex: 1, backgroundColor: "rgba(0,0,0,0.6)", justifyContent: "flex-end" },
  sheet: {
    backgroundColor: c.s2,
    borderTopLeftRadius: 18,
    borderTopRightRadius: 18,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    paddingTop: sp[4],
    paddingBottom: sp[6],
  },
  sheetTitle: {
    color: c.text3,
    fontSize: t.xs,
    fontFamily: font.ui,
    textTransform: "uppercase",
    letterSpacing: 0.6,
    paddingHorizontal: sp[4],
    paddingBottom: sp[2],
  },
  sheetRow: { flexDirection: "row", alignItems: "center", gap: sp[3], paddingHorizontal: sp[4], paddingVertical: 13 },
  sheetLabel: { color: c.text1, fontSize: t.md, fontFamily: font.ui },
  sheetSub: { color: c.text3, fontSize: t.xs, fontFamily: font.mono, marginTop: 1 },
  sheetTick: { color: c.ok, fontSize: t.md },
});
