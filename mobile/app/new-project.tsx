// New project, from the phone. Mirrors the desktop's New Project dialog: pick a location, name it,
// choose agents, create. The desktop does the actual work (mobileBridge.ts → project.create) and the
// state push that follows is what makes it appear on both screens at once.
//
// Path handling deliberately lives on the desktop — it knows whether it's \ or /, we don't. So
// browsing is a round trip (fs.browse) rather than us stitching paths together locally.
import { useCallback, useEffect, useState } from "react";
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useRouter } from "expo-router";
import { req } from "../src/rpc";
import { useConn } from "../src/store";
import { c, font, providerLabel, r, sp, t } from "../src/theme";
import { Btn, Card, Empty, Label, Row, s as u } from "../src/ui";

type Entry = { name: string; dir: boolean };
// `sep` is the desktop's path separator, sent so we can render a "<folder><sep><name>" preview.
// It is for DISPLAY only — every real path is built desktop-side.
type Browse = { path: string; parent: string; sep: string; entries: Entry[] };

// the providers you can stack into a new project, in the desktop's order. `wsl` is intentionally
// absent: it's Windows-only and the phone can't know the desktop's OS, so it stays a desktop choice.
const PROVIDERS = ["claude", "codex", "gemini", "opencode", "grok", "terminal"] as const;
type Prov = (typeof PROVIDERS)[number];
const MAX = 6;

export default function NewProject() {
  const router = useRouter();
  const status = useConn((s) => s.status);
  const online = status === "online";

  const [name, setName] = useState("");
  const [mode, setMode] = useState<"new" | "existing">("new");
  const [browse, setBrowse] = useState<Browse | null>(null);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [git, setGit] = useState(true);
  const [readme, setReadme] = useState(true);
  const [gitignore, setGitignore] = useState(true);
  const [counts, setCounts] = useState<Record<Prov, number>>({
    claude: 1,
    codex: 0,
    gemini: 0,
    opencode: 0,
    grok: 0,
    terminal: 0,
  });

  // `into` descends by name and `path` jumps to an absolute one the desktop gave us — either way the
  // desktop does the joining, so this works whether it's C:\… or /home/…
  const go = useCallback(async (p: { path?: string; into?: string }) => {
    setLoading(true);
    setErr(null);
    try {
      setBrowse(await req<Browse>("fs.browse", p));
    } catch (e) {
      setErr(String((e as Error)?.message ?? e));
    } finally {
      setLoading(false);
    }
  }, []);

  // open at the desktop's projects folder (fs.browse with no path picks it)
  useEffect(() => {
    if (online) void go({});
  }, [online, go]);

  const bump = (k: Prov, d: number) =>
    setCounts((p) => ({ ...p, [k]: Math.max(0, Math.min(MAX, p[k] + d)) }));

  const total = PROVIDERS.reduce((n, k) => n + counts[k], 0);
  const here = browse?.path ?? "";
  const sep = browse?.sep ?? "/";
  // "new" makes a subfolder of the folder you're browsing; "existing" uses it as-is
  const canCreate = !!name.trim() && !!here && !busy && online;

  const create = async () => {
    if (!canCreate) return;
    setBusy(true);
    setErr(null);
    try {
      const res = await req<{ ws: string }>("project.create", {
        name: name.trim(),
        ...(mode === "existing" ? { folder: here } : { parent: here }),
        git,
        // starter files only make sense for a folder we're creating
        readme: mode === "new" && readme,
        gitignore: mode === "new" && gitignore,
        panes: counts,
      });
      // replace, so Back goes home rather than back into a form for a project that now exists
      router.replace(`/space/${res.ws}`);
    } catch (e) {
      setErr(String((e as Error)?.message ?? e));
      setBusy(false);
    }
  };

  if (!online) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        <Empty
          title="Not connected"
          hint="Creating a project runs on the desktop, so it has to be reachable. Check the connection and try again."
        />
      </ScrollView>
    );
  }

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad} keyboardShouldPersistTaps="handled">
      <View>
        <Label>Project name</Label>
        <TextInput
          style={[u.input, { fontFamily: font.ui }]}
          value={name}
          onChangeText={setName}
          placeholder="my-project"
          placeholderTextColor={c.text3}
          autoCapitalize="none"
          autoCorrect={false}
          autoFocus
        />
      </View>

      <View>
        <Label>Location</Label>
        <View style={st.seg}>
          {(["new", "existing"] as const).map((m) => (
            <Pressable
              key={m}
              onPress={() => setMode(m)}
              style={[st.segBtn, mode === m && st.segOn]}
              hitSlop={4}
            >
              <Text style={[st.segText, mode === m && st.segTextOn]}>
                {m === "new" ? "New folder here" : "Use this folder"}
              </Text>
            </Pressable>
          ))}
        </View>

        <Card style={{ marginTop: sp[2] }}>
          <View style={st.pathBar}>
            <Text style={st.path} numberOfLines={1} ellipsizeMode="head">
              {here || "…"}
            </Text>
            {loading && <ActivityIndicator size="small" color={c.text3} />}
          </View>

          {/* the folder that will hold (or be) the project */}
          <View style={[st.target, u.rowDivider]}>
            <Text style={st.targetLabel}>{mode === "new" ? "Creates" : "Opens"}</Text>
            <Text style={st.targetVal} numberOfLines={1} ellipsizeMode="head">
              {mode === "new" ? `${here}${sep}${name.trim() || "…"}` : here}
            </Text>
          </View>

          {browse && browse.parent !== browse.path && (
            <Row onPress={() => void go({ path: browse.parent })}>
              <Text style={st.up}>↑</Text>
              <Text style={[u.title, { flex: 1 }]}>Up one level</Text>
            </Row>
          )}

          {browse?.entries.filter((e) => e.dir).length === 0 && !loading ? (
            <View style={st.pad}>
              <Text style={u.sub}>No subfolders here.</Text>
            </View>
          ) : (
            browse?.entries
              .filter((e) => e.dir)
              .map((e, i, arr) => (
                <Row key={e.name} last={i === arr.length - 1} onPress={() => void go({ into: e.name })}>
                  <Text style={st.folder}>▸</Text>
                  <Text style={[u.title, { flex: 1 }]} numberOfLines={1}>
                    {e.name}
                  </Text>
                  <Text style={st.chev}>›</Text>
                </Row>
              ))
          )}
        </Card>
      </View>

      <View>
        <Label>Agents</Label>
        <Card>
          {PROVIDERS.map((k, i) => (
            <View key={k} style={[u.row, i < PROVIDERS.length - 1 && u.rowDivider]}>
              <Text style={[u.title, { flex: 1 }]}>{providerLabel[k] ?? k}</Text>
              <Pressable onPress={() => bump(k, -1)} hitSlop={8} style={st.step}>
                <Text style={st.stepText}>−</Text>
              </Pressable>
              <Text style={st.count}>{counts[k]}</Text>
              <Pressable onPress={() => bump(k, 1)} hitSlop={8} style={st.step}>
                <Text style={st.stepText}>+</Text>
              </Pressable>
            </View>
          ))}
        </Card>
      </View>

      <View>
        <Label>Setup</Label>
        <Card>
          <Toggle label="Initialise git" on={git} onPress={() => setGit((v) => !v)} />
          {mode === "new" && (
            <>
              <Toggle label="Add README.md" on={readme} onPress={() => setReadme((v) => !v)} />
              <Toggle label="Add .gitignore" on={gitignore} onPress={() => setGitignore((v) => !v)} last />
            </>
          )}
        </Card>
      </View>

      {!!err && <Text style={st.err}>{err}</Text>}

      <Btn kind="primary" onPress={() => void create()} disabled={!canCreate}>
        {busy
          ? "Creating…"
          : total > 0
            ? `Create and launch ${total} pane${total === 1 ? "" : "s"}`
            : "Create project"}
      </Btn>
    </ScrollView>
  );
}

function Toggle({
  label,
  on,
  onPress,
  last,
}: {
  label: string;
  on: boolean;
  onPress: () => void;
  last?: boolean;
}) {
  return (
    <Row last={last} onPress={onPress}>
      <Text style={[u.title, { flex: 1 }]}>{label}</Text>
      <View style={[st.check, on && st.checkOn]}>{on && <Text style={st.checkMark}>✓</Text>}</View>
    </Row>
  );
}

const st = StyleSheet.create({
  seg: { flexDirection: "row", gap: sp[2] },
  segBtn: {
    flex: 1,
    paddingVertical: 10,
    borderRadius: r.one,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    backgroundColor: c.s2,
    alignItems: "center",
  },
  segOn: { backgroundColor: c.s3, borderColor: c.border2 },
  segText: { color: c.text3, fontSize: t.sm, fontFamily: font.ui },
  segTextOn: { color: c.text1, fontFamily: font.uiMedium },

  pathBar: {
    flexDirection: "row",
    alignItems: "center",
    gap: sp[2],
    paddingHorizontal: sp[4],
    paddingVertical: sp[3],
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: c.border1,
  },
  path: { flex: 1, color: c.text2, fontSize: t.sm, fontFamily: font.mono },

  target: { paddingHorizontal: sp[4], paddingVertical: sp[3], gap: 2 },
  targetLabel: {
    color: c.text3,
    fontSize: t.xs,
    fontFamily: font.ui,
    textTransform: "uppercase",
    letterSpacing: 0.6,
  },
  targetVal: { color: c.text1, fontSize: t.sm, fontFamily: font.mono },

  pad: { padding: sp[4] },
  up: { color: c.text3, fontSize: t.md, width: 16, textAlign: "center" },
  folder: { color: c.text3, fontSize: t.sm, width: 16, textAlign: "center" },
  chev: { color: c.text3, fontSize: t.lg },

  step: {
    width: 34,
    height: 34,
    borderRadius: r.one,
    backgroundColor: c.s3,
    alignItems: "center",
    justifyContent: "center",
  },
  stepText: { color: c.text1, fontSize: t.lg, fontFamily: font.uiMedium },
  count: { color: c.text1, fontSize: t.md, fontFamily: font.mono, minWidth: 22, textAlign: "center" },

  check: {
    width: 22,
    height: 22,
    borderRadius: r.one,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: c.border2,
    backgroundColor: c.s2,
    alignItems: "center",
    justifyContent: "center",
  },
  checkOn: { backgroundColor: c.accent, borderColor: c.accent },
  checkMark: { color: c.onAccent, fontSize: t.sm, fontFamily: font.uiMedium },

  err: { color: c.error, fontSize: t.sm, fontFamily: font.ui },
});
