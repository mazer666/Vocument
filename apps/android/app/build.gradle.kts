import org.gradle.api.tasks.TaskProvider

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "org.vocument.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "org.vocument.app"
        minSdk = 33 // PLATFORM-001: Android 13+
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        ndk {
            // PLATFORM-001: primary release ABI arm64-v8a
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // The Tesseract engine is **not** taken from a prebuilt AAR.
    //
    // The OCR runs through the Rust core (`document_core_ocr_frame`), the AAR
    // Java API was never used by the application. Shipping the AAR would have
    // three drawbacks: it duplicated the native library (the AAR and the
    // source build both provide `libtesseract.so`, which the packaging step
    // rejects as a conflict), it was an unpinned external download that was
    // not reachable from every build environment, and its libraries were not
    // built for 16 KB memory pages.
    //
    // `scripts/prepare-android-tesseract.sh` now cross-builds Tesseract and
    // Leptonica from source with the NDK, with a pinned version and with the
    // 16 KB page alignment.
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("net.java.dev.jna:jna:5.15.0@aar")

    val cameraVersion = "1.4.1"
    implementation("androidx.camera:camera-core:$cameraVersion")
    implementation("androidx.camera:camera-camera2:$cameraVersion")
    implementation("androidx.camera:camera-lifecycle:$cameraVersion")
    implementation("androidx.camera:camera-view:$cameraVersion")

    testImplementation("junit:junit:4.13.2")
    testImplementation("net.java.dev.jna:jna:5.15.0")
}

/**
 * Compiles the native Rust core library for Android ARM64.
 *
 * Every variant gets its **own** task. A single shared task cannot work: the
 * build profile is an environment variable of the task, and the environment of
 * a task that is configured for several variants is the one of the last
 * variant that was configured. The debug build would then ship the release
 * library, or the other way round.
 */
fun registerBuildRustCore(name: String, profile: String): TaskProvider<Exec> {
    return tasks.register<Exec>(name) {
        description = "Compiles native Rust core library (libvocument_ffi.so) for Android ARM64 ($profile)"
        val repoRoot = rootProject.projectDir.parentFile.parentFile
        val jniArm64File = projectDir.resolve("src/main/jniLibs/arm64-v8a/libvocument_ffi.so")

        inputs.dir(repoRoot.resolve("engines"))
        inputs.dir(repoRoot.resolve("core"))
        inputs.dir(repoRoot.resolve("platform"))
        inputs.file(repoRoot.resolve("Cargo.toml"))
        // The profile is an input, so that switching the variant rebuilds the
        // library instead of reusing the one of the other profile.
        inputs.property("buildProfile", profile)
        outputs.file(jniArm64File)

        workingDir(repoRoot)
        commandLine(repoRoot.resolve("scripts/build-android-rust.sh").absolutePath)
        environment("BUILD_PROFILE", profile)
        environment("ANDROID_MAX_PAGE_SIZE", "16384")
    }
}

/**
 * The build profile of the native library follows the variant that is built.
 *
 * A release build must not ship the debug library: the DWARF debug
 * information alone accounts for roughly 90 MB of the 101 MB library.
 */
androidComponents {
    onVariants { variant ->
        val profile = if (variant.buildType == "release") "release" else "debug"
        val variantName = variant.name.replaceFirstChar { it.uppercase() }
        val task = registerBuildRustCore("buildRustCore$variantName", profile)
        tasks.matching { it.name == "merge${variantName}JniLibFolders" }.configureEach {
            dependsOn(task)
        }
    }
}

tasks.withType<Test> {
    val repoRoot = rootProject.projectDir.parentFile.parentFile
    val targetDebug = File(repoRoot, "target/debug").absolutePath
    systemProperty("jna.library.path", targetDebug)
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    // The OCR of the test runs on the host and needs the trained data of the
    // application. Without the path the tests would look in the system
    // directory and fail with "Error opening data file".
    environment("TESSDATA_PREFIX", File(repoRoot, "apps/android/app/src/main/assets/tessdata").absolutePath)
}
