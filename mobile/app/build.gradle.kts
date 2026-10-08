import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// versionCode follows the version, so an update always carries a higher one
val appVersion = providers.gradleProperty("hyprspace.version").get()
val appCode = appVersion.split(".").map { it.toInt() }.let { (a, b, c) -> a * 10_000 + b * 100 + c }

// Release builds are signed with the key in these variables (CI has them as secrets). Without
// them a release build is signed with the debug key, which only suits a local install.
val keystore = System.getenv("HYPRSPACE_ANDROID_KEYSTORE")

android {
    namespace = "com.hyprspace.android"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.hyprspace.android"
        minSdk = 29
        targetSdk = 36
        versionName = appVersion
        versionCode = appCode
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
}

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

    testImplementation(libs.junit)
    testImplementation(libs.coroutines.test)
}
