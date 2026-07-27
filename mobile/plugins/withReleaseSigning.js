// Sign release builds with a real keystore instead of the debug one.
//
// `android/` is generated (and gitignored), so a hand-edit to build.gradle vanishes the next time
// anyone runs `expo prebuild --clean`. This puts the change in the repo, where it survives.
//
// The keystore and its passwords live OUTSIDE the repo — in ~/.gradle/gradle.properties, written by
// `npm run keystore`. If those properties aren't set (a fresh clone, CI, a contributor), the build
// falls back to the debug signing config so `npm run apk` still produces something installable.

const { withAppBuildGradle } = require("expo/config-plugins");

const RELEASE_CONFIG = `
        release {
            if (project.hasProperty('HYPRSPACE_STORE_FILE')) {
                storeFile file(HYPRSPACE_STORE_FILE)
                storePassword HYPRSPACE_STORE_PASSWORD
                keyAlias HYPRSPACE_KEY_ALIAS
                keyPassword HYPRSPACE_KEY_PASSWORD
            }
        }`;

module.exports = function withReleaseSigning(config) {
  return withAppBuildGradle(config, (cfg) => {
    let src = cfg.modResults.contents;

    if (!src.includes("HYPRSPACE_STORE_FILE")) {
      // add a `release` entry alongside the template's `debug` one
      const anchor = "    signingConfigs {";
      if (!src.includes(anchor)) {
        throw new Error("withReleaseSigning: couldn't find signingConfigs in build.gradle");
      }
      src = src.replace(anchor, anchor + RELEASE_CONFIG);
    }

    // point the release buildType at it, but only when the properties are actually there
    const before = "            signingConfig signingConfigs.debug\n            def enableShrinkResources";
    const after =
      "            signingConfig project.hasProperty('HYPRSPACE_STORE_FILE') ? signingConfigs.release : signingConfigs.debug\n            def enableShrinkResources";
    if (src.includes(before)) {
      src = src.replace(before, after);
    } else if (!src.includes("signingConfigs.release :")) {
      throw new Error("withReleaseSigning: couldn't find the release buildType's signingConfig");
    }

    cfg.modResults.contents = src;
    return cfg;
  });
};
