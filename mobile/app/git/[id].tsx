// What changed in a space's repo, and the diff for one file. Read-only apart from commit — reviewing
// on a phone is the useful half; landing code is better done at a keyboard.
import { useCallback, useEffect, useState } from "react";
import { Alert, RefreshControl, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useLocalSearchParams, useNavigation } from "expo-router";
import { useConn, useSpace } from "../../src/store";
import { req } from "../../src/rpc";
import { c, font, r, sp, t } from "../../src/theme";
import { Btn, Card, Empty, Label, Loading, Row, s as u } from "../../src/ui";

interface FileChange {
  path: string;
  status: string;
  additions: number;
  deletions: number;
}

interface Branch {
  branch: string;
  ahead: number;
  behind: number;
  upstream: boolean;
  is_repo: boolean;
}

const STATUS_COLOR: Record<string, string> = {
  A: c.ok,
  "?": c.ok,
  M: c.busy,
  D: c.error,
  R: c.awaiting,
};

/** one diff hunk, colored the way the desktop's review dock does it */
function Diff({ text }: { text: string }) {
  return (
    <ScrollView horizontal style={g.diffScroll} contentContainerStyle={{ paddingRight: sp[4] }}>
      <View>
        {text.split("\n").map((line, i) => {
          const tone =
            line.startsWith("+") && !line.startsWith("+++")
              ? c.ok
              : line.startsWith("-") && !line.startsWith("---")
                ? c.error
                : line.startsWith("@@")
                  ? c.awaiting
                  : c.text3;
          return (
            <Text key={i} style={[g.diffLine, { color: tone }]}>
              {line || " "}
            </Text>
          );
        })}
      </View>
    </ScrollView>
  );
}

export default function GitScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const nav = useNavigation();
  const space = useSpace(id);
  const online = useConn((s) => s.status === "online");

  const [files, setFiles] = useState<FileChange[] | null>(null);
  const [branch, setBranch] = useState<Branch | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [diff, setDiff] = useState<string>("");
  const [msg, setMsg] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (space) nav.setOptions({ title: `${space.name} · changes` });
  }, [nav, space]);

  const load = useCallback(async () => {
    if (!space?.cwd || !online) return;
    setErr(null);
    try {
      const [ch, br] = await Promise.all([
        req<{ files: FileChange[] }>("git.changes", { cwd: space.cwd }),
        req<Branch>("git.branch", { cwd: space.cwd }),
      ]);
      setFiles(ch.files);
      setBranch(br);
    } catch (e) {
      setErr(String((e as Error).message));
    }
  }, [space?.cwd, online]);

  useEffect(() => {
    void load();
  }, [load]);

  const openFile = async (path: string) => {
    if (open === path) {
      setOpen(null);
      return;
    }
    setOpen(path);
    setDiff("");
    try {
      const res = await req<{ diff: string }>("git.diff", { cwd: space!.cwd, path });
      setDiff(res.diff || "(no textual diff)");
    } catch (e) {
      setDiff(String((e as Error).message));
    }
  };

  const commit = async (push: boolean) => {
    if (!msg.trim() || !space?.cwd) return;
    setBusy(true);
    try {
      await req("git.commit", { cwd: space.cwd, message: msg.trim(), push });
      setMsg("");
      await load();
    } catch (e) {
      Alert.alert("Commit failed", String((e as Error).message));
    } finally {
      setBusy(false);
    }
  };

  if (!space) {
    return (
      <ScrollView style={u.screen} contentContainerStyle={u.screenPad}>
        <Loading />
      </ScrollView>
    );
  }

  return (
    <ScrollView
      style={u.screen}
      contentContainerStyle={u.screenPad}
      refreshControl={
        <RefreshControl refreshing={false} onRefresh={() => void load()} tintColor={c.text3} colors={[c.accent]} />
      }
    >
      {!!branch && (
        <View>
          <Label>Branch</Label>
          <Card>
            <Row last>
              <Text style={[u.mono, { flex: 1 }]}>{branch.branch || "(detached)"}</Text>
              <Text style={u.sub}>
                {branch.upstream ? `↑${branch.ahead} ↓${branch.behind}` : "no upstream"}
              </Text>
            </Row>
          </Card>
        </View>
      )}

      <View>
        <Label>{files ? `${files.length} changed` : "Changed"}</Label>
        {files === null ? (
          <Card>{err ? <Empty title="Couldn't read the repo" hint={err} /> : <Loading />}</Card>
        ) : files.length === 0 ? (
          <Card>
            <Empty title="Nothing changed" hint="The working tree is clean." />
          </Card>
        ) : (
          <Card>
            {files.map((f, i) => (
              <View key={f.path}>
                <Row last={i === files.length - 1 && open !== f.path} onPress={() => void openFile(f.path)}>
                  <Text style={[g.status, { color: STATUS_COLOR[f.status] ?? c.text3 }]}>{f.status}</Text>
                  <View style={{ flex: 1, minWidth: 0 }}>
                    <Text style={u.mono} numberOfLines={1} ellipsizeMode="head">
                      {f.path}
                    </Text>
                  </View>
                  <Text style={g.plus}>+{f.additions}</Text>
                  <Text style={g.minus}>−{f.deletions}</Text>
                </Row>
                {open === f.path && (
                  <View style={g.diffBox}>{diff ? <Diff text={diff} /> : <Loading />}</View>
                )}
              </View>
            ))}
          </Card>
        )}
      </View>

      {!!files?.length && (
        <View>
          <Label>Commit everything</Label>
          <Card>
            <View style={g.pad}>
              <TextInput
                style={[u.input, { fontFamily: font.ui }]}
                value={msg}
                onChangeText={setMsg}
                placeholder="Commit message"
                placeholderTextColor={c.text3}
                multiline
              />
              <View style={g.btnRow}>
                <Btn onPress={() => void commit(false)} disabled={busy || !msg.trim()} style={{ flex: 1 }}>
                  Commit
                </Btn>
                <Btn
                  kind="primary"
                  onPress={() => void commit(true)}
                  disabled={busy || !msg.trim()}
                  style={{ flex: 1 }}
                >
                  Commit & push
                </Btn>
              </View>
            </View>
          </Card>
        </View>
      )}
    </ScrollView>
  );
}

const g = StyleSheet.create({
  status: { width: 16, fontSize: t.sm, fontFamily: font.mono, textAlign: "center" },
  plus: { color: c.ok, fontSize: t.xs, fontFamily: font.mono },
  minus: { color: c.error, fontSize: t.xs, fontFamily: font.mono },
  diffBox: {
    backgroundColor: c.bg,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: c.border1,
    maxHeight: 340,
  },
  diffScroll: { padding: sp[3] },
  diffLine: { fontSize: 10, fontFamily: font.mono, lineHeight: 15 },
  pad: { padding: sp[4], gap: sp[3] },
  btnRow: { flexDirection: "row", gap: sp[2] },
  card: { borderRadius: r.two },
});
