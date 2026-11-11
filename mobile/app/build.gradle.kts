import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// The app's own version, moved by mobile/deploy.ps1. versionCode follows it, so an update always
// carries a higher one.
val appVersion = rootDir.resolve("VERSION").readText().trim()
val appCode = appVersion.split(".").map { it.toInt() }.let { (a, b, c) -> a * 1_000_000 + b * 1_000 + c }
// the commit a build came from, so two builds of one version can be told apart
val commit = providers.exec {
    commandLine("git", "rev-parse", "--short", "HEAD")
    isIgnoreExitValue = true
}.standardOutput.asText.map { it.trim() }.getOrElse("")

// Release builds are signed with the key in these variables (CI has them as secrets). Without
// them a release build is signed with the debug key, which only suits a local install.
val keystore = System.getenv("HYPRSPACE_ANDROID_KEYSTORE")
val far = layout.buildDirectory.dir("far/jniLibs")

android {
    namespace = "com.hyprspace.android"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.hyprspace.android"
        minSdk = 29
        targetSdk = 36
        versionName = appVersion
        versionCode = appCode
        buildConfigField("String", "COMMIT", "\"$commit\"")
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("HYPRSPACE_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("HYPRSPACE_ANDROID_KEY_ALIAS") ?: "hyprspace"
                keyPassword = System.getenv("HYPRSPACE_ANDROID_KEY_PASSWORD") ?: storePassword
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
        debug {
            // installs beside the release build
            applicationIdSuffix = ".dev"
            versionNameSuffix = "-dev"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    packaging {
        resources.excludes += "/META-INF/{AL2.0,LGPL2.1}"
    }

    ndkVersion = "29.0.14206865"
    sourceSets["main"].jniLibs.directories += far.get().asFile.path
}

// The native library that reaches a computer from any network (mobile/far), built for phones
// and the emulator with cargo-ndk.
val farTask = tasks.register<Exec>("buildFar") {
    val src = rootDir.resolve("far")
    inputs.dir(src.resolve("src"))
    inputs.file(src.resolve("Cargo.toml"))
    inputs.dir(rootDir.resolve("../crates/far/src"))
    outputs.dir(far)
    workingDir = src
    environment("ANDROID_NDK_HOME", androidComponents.sdkComponents.ndkDirectory.get().asFile.path)
    commandLine(
        "cargo", "ndk", "-t", "arm64-v8a", "-t", "x86_64", "--platform", "29",
        "-o", far.get().asFile.path, "build", "--release",
    )
    // cargo-ndk copies every .so it finds, iroh's own build products among them
    doLast {
        far.get().asFile.walkTopDown()
            .filter { it.isFile && it.name != "libhyprspace_far.so" }
            .forEach { it.delete() }
    }
}
tasks.named("preBuild") { dependsOn(farTask) }

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    val bom = platform(libs.compose.bom)
    implementation(bom)
    implementation(libs.compose.ui)
    implementation(libs.compose.foundation)
    implementation(libs.compose.material3)
    implementation(libs.compose.tooling.preview)
    debugImplementation(libs.compose.tooling)
    implementation(libs.activity.compose)
    implementation(libs.lifecycle.runtime)
    implementation(libs.lifecycle.viewmodel)
    implementation(libs.lifecycle.process)
    implementation(libs.core.ktx)
    implementation(libs.okhttp)
    implementation(libs.serialization.json)
    implementation(libs.coroutines.android)
    implementation(libs.camera.camera2)
    implementation(libs.camera.lifecycle)
    implementation(libs.camera.view)
    implementation(libs.mlkit.barcode)
    implementation(libs.datastore)
    implementation(libs.commonmark)
    implementation(libs.commonmark.gfm.tables)
    implementation(libs.commonmark.gfm.strikethrough)
    implementation(libs.commonmark.autolink)

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
}
