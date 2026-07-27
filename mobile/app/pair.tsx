// Pair with a desktop: scan the QR from Settings → Mobile, or type the address + code by hand.
import { useEffect, useRef, useState } from "react";
import { ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { useLocalSearchParams, useRouter } from "expo-router";
import { CameraView, useCameraPermissions } from "expo-camera";
import { parsePairing, useConn } from "../src/store";
import { connect } from "../src/rpc";
import { c, font, r, sp, t } from "../src/theme";
import { Btn, Card, Label, s as u } from "../src/ui";

export default function Pair() {
  const router = useRouter();
  const [perm, askPerm] = useCameraPermissions();
  const [scanning, setScanning] = useState(false);
  const [host, setHost] = useState(useConn.getState().host);
  const [port, setPort] = useState(String(useConn.getState().port || 6768));
  const [token, setToken] = useState("");
  const [err, setErr] = useState<string | null>(null);

  const save = async (v: { host: string; port: number; token: string }) => {
    await useConn.getState().pair(v);
    connect();
    router.replace("/");
  };

  // Opened via the pairing link itself (`hyprspace://pair?host=…&port=…&token=…`) — the same string
  // the desktop's QR encodes. Pair straight away instead of making you retype what you just tapped.
  const params = useLocalSearchParams<{
    host?: string;
    port?: string;
    token?: string;
    remote?: string;
  }>();
  const autoPaired = useRef(false);
  useEffect(() => {
    if (autoPaired.current || !params.host || !params.token) return;
    // rebuild the payload for parsePairing — every param has to survive the trip, `remote` included,
    // or the phone ends up with only the LAN address and no way in from outside
    const parsed = parsePairing(
      `host=${encodeURIComponent(params.host)}&port=${params.port ?? 6768}` +
        `&token=${encodeURIComponent(params.token)}` +
        (params.remote ? `&remote=${encodeURIComponent(params.remote)}` : ""),
    );
    if (!parsed) {
      setErr("That pairing link is missing something — check the address and code.");
      return;
    }
    autoPaired.current = true;
    void save(parsed);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [params.host, params.port, params.token]);

  const onScan = (data: string) => {
    if (!scanning) return;
    const parsed = parsePairing(data);
    if (!parsed) {
      setErr("That QR isn't a HyprSpace pairing code.");
      return;
    }
    setScanning(false);
    void save(parsed);
  };

  const onManual = () => {
    const p = Number(port);
    const parsed = parsePairing(
      `host=${encodeURIComponent(host.trim())}&port=${Number.isFinite(p) ? p : 6768}&token=${encodeURIComponent(token.trim())}`,
    );
    if (!parsed) {
      setErr("Check the address and the code — the code is 32 characters.");
      return;
    }
    void save(parsed);
  };

  const startScan = async () => {
    setErr(null);
    if (!perm?.granted) {
      const res = await askPerm();
      if (!res.granted) {
        setErr("Camera access was denied — you can still type the address and code below.");
        return;
      }
    }
    setScanning(true);
  };

  return (
    <ScrollView style={u.screen} contentContainerStyle={u.screenPad} keyboardShouldPersistTaps="handled">
      <View>
        <Label>Scan</Label>
        <Card>
          {scanning ? (
            <View style={p.camWrap}>
              <CameraView
                style={p.cam}
                facing="back"
                barcodeScannerSettings={{ barcodeTypes: ["qr"] }}
                onBarcodeScanned={(e) => onScan(e.data)}
              />
            </View>
          ) : (
            <View style={p.pad}>
              <Text style={u.sub}>
                On the desktop: Settings → Mobile → turn on “Sync to your phone”, then point your camera
                at the QR code.
              </Text>
              <Btn kind="primary" onPress={() => void startScan()} style={{ marginTop: sp[3] }}>
                Scan QR code
              </Btn>
            </View>
          )}
          {scanning && (
            <View style={p.pad}>
              <Btn onPress={() => setScanning(false)}>Cancel</Btn>
            </View>
          )}
        </Card>
      </View>

      <View>
        <Label>Or enter it manually</Label>
        <Card>
          <View style={p.pad}>
            <View style={p.field}>
              <Text style={p.key}>Address</Text>
              <TextInput
                style={[u.input, { flex: 1 }]}
                value={host}
                onChangeText={setHost}
                placeholder="192.168.1.20"
                placeholderTextColor={c.text3}
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="numbers-and-punctuation"
              />
            </View>
            <View style={p.field}>
              <Text style={p.key}>Port</Text>
              <TextInput
                style={[u.input, { width: 96 }]}
                value={port}
                onChangeText={(v) => setPort(v.replace(/\D/g, "").slice(0, 5))}
                keyboardType="number-pad"
                placeholderTextColor={c.text3}
              />
            </View>
            <View style={p.field}>
              <Text style={p.key}>Code</Text>
              <TextInput
                style={[u.input, { flex: 1 }]}
                value={token}
                onChangeText={(v) => setToken(v.trim())}
                placeholder="32 characters"
                placeholderTextColor={c.text3}
                autoCapitalize="none"
                autoCorrect={false}
              />
            </View>
            <Btn kind="primary" onPress={onManual} style={{ marginTop: sp[2] }}>
              Connect
            </Btn>
          </View>
        </Card>
      </View>

      {!!err && <Text style={p.err}>{err}</Text>}

      <Text style={p.note}>
        Your phone and this computer have to be on the same wifi. Nothing goes through the internet and
        no account is involved — the code is only checked by the desktop app itself.
      </Text>
    </ScrollView>
  );
}

const p = StyleSheet.create({
  pad: { padding: sp[4], gap: sp[3] },
  camWrap: { height: 300, backgroundColor: "#000", borderRadius: r.two, overflow: "hidden", margin: sp[3] },
  cam: { flex: 1 },
  field: { flexDirection: "row", alignItems: "center", gap: sp[3] },
  key: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, width: 64 },
  err: { color: c.error, fontSize: t.sm, fontFamily: font.ui },
  note: { color: c.text3, fontSize: t.sm, fontFamily: font.ui, lineHeight: 19 },
});
