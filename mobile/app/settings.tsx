import { useState } from "react";
import { Alert, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useRouter } from "expo-router";
import Constants from "expo-constants";
import { useConn } from "../src/store";
import { connect, disconnect, PROTOCOL } from "../src/rpc";
import { c, font, sp, t } from "../src/theme";
import { Btn, Card, Label, Row, s as u } from "../src/ui";

const STATUS_TEXT: Record<string, string> = {
  unpaired: "Not paired",
  offline: "Offline",
  connecting: "Connecting…",
  online: "Connected",
  retrying: "Reconnecting…",
  failed: "Failed",
};

export default function Settings() {
  const router = useRouter();
  const { host, port, status, deviceName, desktopHost, desktopVersion, error } = useConn();
  const [name, setName] = useState(deviceName);

  const forget = () => {
    Alert.alert("Unpair this phone?", "You'll need to scan the QR again to reconnect.", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Unpair",
        style: "destructive",
        onPress: () => {
          disconnect();
          void useConn.getState().forget();
          router.replace("/");
        },
      },
    ]);
  };

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad} keyboardShouldPersistTaps="handled">
      <View>
        <Label>Desktop</Label>
        <Card>
          <Row>
            <Text style={[u.title, { flex: 1 }]}>Status</Text>
            <Text style={[u.sub, status === "online" && { color: c.ok }]}>
              {STATUS_TEXT[status] ?? status}
            </Text>
          </Row>
          <Row>
            <Text style={[u.title, { flex: 1 }]}>Address</Text>
            <Text style={u.mono}>{host ? `${host}:${port}` : "—"}</Text>
          </Row>
          <Row last>
            <Text style={[u.title, { flex: 1 }]}>Running</Text>
            <Text style={u.sub}>
              {desktopHost ? `${desktopHost}${desktopVersion ? ` · v${desktopVersion}` : ""}` : "—"}
            </Text>
          </Row>
        </Card>
        {!!error && <Text style={st.err}>{error}</Text>}
      </View>

      <View style={st.row}>
        <Btn onPress={() => connect()} style={{ flex: 1 }}>
          Reconnect
        </Btn>
        <Btn onPress={() => router.push("/pair")} kind="primary" style={{ flex: 1 }}>
          {host ? "Re-pair" : "Pair"}
        </Btn>
      </View>

      <View>
        <Label>This phone</Label>
        <Card>
          <View style={st.pad}>
            <Text style={u.sub}>The name the desktop shows under Settings → Mobile → Connected.</Text>
            <TextInput
              style={[u.input, { fontFamily: font.ui }]}
              value={name}
              onChangeText={setName}
              onBlur={() => void useConn.getState().setDeviceName(name)}
              placeholder="Android phone"
              placeholderTextColor={c.text3}
            />
          </View>
        </Card>
      </View>

      <View>
        <Label>About</Label>
        <Card>
          <Row>
            <Text style={[u.title, { flex: 1 }]}>App version</Text>
            <Text style={u.sub}>{Constants.expoConfig?.version ?? "—"}</Text>
          </Row>
          <Row last>
            <Text style={[u.title, { flex: 1 }]}>Bridge protocol</Text>
            <Text style={u.sub}>v{PROTOCOL}</Text>
          </Row>
        </Card>
      </View>

      {!!host && (
        <Btn kind="danger" onPress={forget}>
          Unpair
        </Btn>
      )}

      <Text style={st.note}>
        Everything runs over your local network — your phone talks straight to HyprSpace on your
        computer. There's no cloud service, no account, and terminals only mirror while the desktop app
        is open.
      </Text>
    </ScrollView>
  );
}

const st = StyleSheet.create({
  row: { flexDirection: "row", gap: sp[2] },
  pad: { padding: sp[4], gap: sp[3] },
  err: { color: c.error, fontSize: t.sm, fontFamily: font.ui, marginTop: sp[2], lineHeight: 19 },
  note: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19 },
});
