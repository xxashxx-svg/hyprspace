# HyprSpace for Android

Your desktop's threads on your phone: follow your agents, answer approvals, reply, stop a run,
type into terminals and start new threads. The phone talks to HyprSpace on your computer
directly, over your network or Tailscale, under a certificate it pins when it pairs.

## Pairing

1. On the computer, open HyprSpace, then Settings, Phone, and switch on **Let your phone connect**.
2. Press **Show code**.
3. In the app, tap **Scan the code**. If the camera can't read it, type the address and the code
   shown under the QR code instead.
4. To remove the pairing, press **Forget** on either side. Both forget each other.

## Building

Kotlin, Compose, AGP 9 and Gradle 9. You need JDK 17 and the Android SDK with platform 37.

```bash
./gradlew assembleDebug        # installs beside a release as "HyprSpace" (.dev)
./gradlew testDebugUnitTest
./gradlew assembleRelease      # signed when the HYPRSPACE_ANDROID_* variables are set
```

How the bridge works, and how to try the app against a dev build of the desktop:
[docs/internals/phone.md](../docs/internals/phone.md) and
[docs/operations/development.md](../docs/operations/development.md). Releasing it:
[docs/operations/release.md](../docs/operations/release.md).
