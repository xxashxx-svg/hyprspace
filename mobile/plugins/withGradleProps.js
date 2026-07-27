// Gradle settings the Expo template gets wrong for this project.
//
// Like the signing plugin, this lives here because `android/` is generated — a hand-edit to
// android/gradle.properties disappears on the next `expo prebuild --clean`.

const { withGradleProperties } = require("expo/config-plugins");

const PROPS = {
  // The template's `-XX:MaxMetaspaceSize=512m` is not enough for this plugin set: the daemon dies
  // mid-build with `OutOfMemoryError: Metaspace`, which surfaces as the useless
  // "Gradle build daemon disappeared unexpectedly". Heap stays modest so the build coexists with
  // whatever else is running.
  "org.gradle.jvmargs": "-Xmx3072m -XX:MaxMetaspaceSize=1536m -XX:+HeapDumpOnOutOfMemoryError",

  // Every Android phone made since ~2016 is arm64. Building the other three ABIs compiles React
  // Native's C++ core four times over — most of the wall-clock on a cold build, and ~4x the APK —
  // for emulator support (x86/x86_64) and pre-2016 devices (armeabi-v7a) we don't ship to.
  // `npm run apk -- all` puts them back via -P, no prebuild needed.
  reactNativeArchitectures: "arm64-v8a",

  // 16 logical cores would happily start 16 memory-hungry compiler processes. Cap it so a build
  // doesn't push the machine into swap.
  "org.gradle.workers.max": "4",
};

module.exports = function withGradleProps(config) {
  return withGradleProperties(config, (cfg) => {
    for (const [key, value] of Object.entries(PROPS)) {
      const existing = cfg.modResults.find((item) => item.type === "property" && item.key === key);
      if (existing) existing.value = value;
      else cfg.modResults.push({ type: "property", key, value });
    }
    return cfg;
  });
};
