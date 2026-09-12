import { useState } from "react";
import { ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useRouter } from "expo-router";
import Constants from "expo-constants";
import { endpointUrl, parseEndpoint, useConn } from "../src/store";
import { connect, disconnect, PROTOCOL } from "../src/rpc";
import { c, font, sp, t } from "../src/theme";
import { Btn, Card, Dialog, Label, Row, s as u } from "../src/ui";

const STATUS_TEXT: Record<string, string> = {
  unpaired: "Not paired",
  offline: "Offline",
  connecting: "Connecting",
  online: "Connected",
  retrying: "Reconnecting",
  failed: "Failed",
};

export default function Settings() {
  const router = useRouter();
  const { host, status, deviceName, desktopHost, desktopVersion, desktopProtocol, error, endpoints, activeEndpoint } =
    useConn();
  const [name, setName] = useState(deviceName);
  const away = endpoints.find((e) => e.label === "Away");
  const [remote, setRemote] = useState(away ? endpointUrl(away).replace(/\/$/, "") : "");
  const [remoteErr, setRemoteErr] = useState<string | null>(null);
  const [askUnpair, setAskUnpair] = useState(false);

  const forget = () => {
    setAskUnpair(false);
    disconnect();
    void useConn.getState().forget();
    router.replace("/");
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
          <Row last>
            <Text style={[u.title, { flex: 1 }]}>Running</Text>
            <Text style={u.sub}>
              {desktopHost ? `${desktopHost}${desktopVersion ? ` · v${desktopVersion}` : ""}` : "Not connected"}
            </Text>
          </Row>
        </Card>
        {!!error && <Text style={st.err}>{error}</Text>}
      </View>

      <View>
        <Label>Ways in</Label>
        <Card>
          {endpoints.length === 0 ? (
            <Row last>
              <Text style={u.sub}>Not paired yet</Text>
            </Row>
          ) : (
            endpoints.map((e, i) => (
              <Row key={e.label} last={i === endpoints.length - 1}>
                <View style={{ flex: 1, minWidth: 0 }}>
                  <Text style={u.title}>{e.label}</Text>
                  <Text style={u.mono} numberOfLines={1}>
                    {endpointUrl(e).replace(/\/$/, "")}
                  </Text>
                </View>
                {activeEndpoint === e.label && status === "online" ? (
                  <Text style={st.live}>in use</Text>
                ) : (
                  i === 0 && <Text style={u.sub}>first</Text>
                )}
              </Row>
            ))
          )}
        </Card>
        <Text style={st.hint}>
          These are tried in order, and whichever connects moves to the top. Add a second one so the
          app still reaches your desktop when you are off the home network.
        </Text>
      </View>

      <View>
        <Label>Reach it from outside</Label>
        <Card>
          <View style={st.pad}>
            <Text style={u.sub}>
              A VPN address or a tunnel's public URL. Tailscale gives this machine a 100.x.x.x one.
              A plain address uses ws://, and an https:// or wss:// URL is treated as encrypted.
            </Text>
            <TextInput
              style={[u.input, { fontFamily: font.mono, fontSize: t.sm }]}
              value={remote}
              onChangeText={setRemote}
              placeholder="100.90.1.2  ·  wss://box.trycloudflare.com"
              placeholderTextColor={c.text3}
              autoCapitalize="none"
              autoCorrect={false}
            />
            {!!remoteErr && <Text style={st.err}>{remoteErr}</Text>}
            <View style={st.row}>
              <Btn
                kind="primary"
                style={{ flex: 1 }}
                onPress={() => {
                  const e = parseEndpoint(remote, "Away");
                  if (!e) return setRemoteErr("That doesn't look like an address or URL.");
                  setRemoteErr(null);
                  void useConn.getState().setEndpoint(e);
                  connect();
                }}
              >
                Save
              </Btn>
              {endpoints.some((e) => e.label === "Away") && (
                <Btn
                  kind="danger"
                  style={{ flex: 1 }}
                  onPress={() => {
                    setRemote("");
                    void useConn.getState().removeEndpoint("Away");
                  }}
                >
                  Remove
                </Btn>
              )}
            </View>
          </View>
        </Card>
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
            <Text style={u.sub}>The name your desktop lists under Settings, Mobile, Connected.</Text>
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
            <Text style={u.sub}>{Constants.expoConfig?.version ?? "Unknown"}</Text>
          </Row>
          <Row last>
            <Text style={[u.title, { flex: 1 }]}>Bridge protocol</Text>
            <Text style={u.sub}>
              v{PROTOCOL}
              {desktopProtocol > 0 && desktopProtocol !== PROTOCOL ? ` · desktop v${desktopProtocol}` : ""}
            </Text>
          </Row>
        </Card>
        {desktopProtocol > PROTOCOL && (
          <Text style={st.hint}>
            Your desktop speaks a newer protocol than this app. Everything here still works. The
            releases page has a newer APK with whatever it added.
          </Text>
        )}
      </View>

      {!!host && (
        <Btn kind="danger" onPress={() => setAskUnpair(true)}>
          Unpair
        </Btn>
      )}

      <Text style={st.note}>
        Your phone talks straight to HyprSpace on your computer. There is no cloud service and no
        account, and terminals only mirror while the desktop app is open. That traffic is unencrypted
        on your own network, which is fine at home. To reach it from outside, use a VPN address or a
        wss:// tunnel rather than forwarding the port.
      </Text>
      <Dialog
        open={askUnpair}
        title="Unpair this phone?"
        message="You will need to scan the code on your desktop again to reconnect."
        confirm="Unpair"
        danger
        onConfirm={forget}
        onClose={() => setAskUnpair(false)}
      />
    </ScrollView>
  );
}

const st = StyleSheet.create({
  row: { flexDirection: "row", gap: sp[2] },
  pad: { padding: sp[4], gap: sp[3] },
  err: { color: c.error, fontSize: t.sm, fontFamily: font.ui, marginTop: sp[2], lineHeight: 19 },
  hint: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19, marginTop: sp[2] },
  live: { color: c.ok, fontSize: t.xs, fontFamily: font.ui, textTransform: "uppercase", letterSpacing: 0.6 },
  note: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19 },
});
