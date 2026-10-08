# HyprSpace for Android

Your desktop's threads on your phone: follow your agents, answer approvals, reply, stop a run,
type into terminals and start new threads. The phone talks to HyprSpace on your computer
directly, over your network or Tailscale, under a certificate it pins when it pairs.

## Pairing

1. On the computer, open HyprSpace, then Settings, Phone, and switch on **Let your phone connect**.
2. Press **Show code**.
3. In the app, tap **Scan the code**. If the camera can't read it, type the address and the code
   shown under the QR code instead.
4. To remove the pairing, press **Revoke** on the computer or **Forget** on the phone. Both drop
   the pairing.

## Updates

The app updates itself from GitHub releases. It checks every six hours, downloads in the
background and shows **Install** when the update is ready. Android asks to confirm the first one;
after that, Android 12 and newer let it install updates on its own while it's off screen. An
install signed with a different key, like a local build, can't update this way.

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
