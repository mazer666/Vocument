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

val tesseractAar: Configuration = configurations.create("tesseractAar")

dependencies {
    tesseractAar("cz.adaptech.tesseract4android:tesseract4android:4.9.0")
    implementation(platform("androidx.compose:compose-bom:2024.10.01"))
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")
    implementation("net.java.dev.jna:jna:5.15.0@aar")
    implementation("cz.adaptech.tesseract4android:tesseract4android:4.9.0")

    val cameraVersion = "1.4.1"
    implementation("androidx.camera:camera-core:$cameraVersion")
    implementation("androidx.camera:camera-camera2:$cameraVersion")
    implementation("androidx.camera:camera-lifecycle:$cameraVersion")
    implementation("androidx.camera:camera-view:$cameraVersion")

    testImplementation("junit:junit:4.13.2")
    testImplementation("net.java.dev.jna:jna:5.15.0")
}

tasks.register("extractTesseractNativeLibs") {
    description = "Extracts native .so libraries from tesseract4android AAR for Rust linking"
    val outputDir = rootProject.projectDir.parentFile.parentFile.resolve("engines/ocr/android-libs/arm64-v8a")
    inputs.files(tesseractAar)
    outputs.dir(outputDir)
    doLast {
        outputDir.mkdirs()
        tesseractAar.resolvedConfiguration.resolvedArtifacts.forEach { artifact ->
            if (artifact.name == "tesseract4android") {
                copy {
                    from(zipTree(artifact.file)) {
                        include("jni/arm64-v8a/*.so")
                        eachFile {
                            path = name
                        }
                        includeEmptyDirs = false
                    }
                    into(outputDir)
                }
            }
        }
    }
}

tasks.named("preBuild").configure {
    dependsOn("extractTesseractNativeLibs")
}

val buildRustCore = tasks.register<Exec>("buildRustCore") {
    description = "Compiles native Rust core library (libvocument_ffi.so) for Android ARM64"
    val repoRoot = rootProject.projectDir.parentFile.parentFile
    val jniArm64File = projectDir.resolve("src/main/jniLibs/arm64-v8a/libvocument_ffi.so")

    inputs.dir(repoRoot.resolve("engines"))
    inputs.dir(repoRoot.resolve("core"))
    inputs.dir(repoRoot.resolve("platform"))
    inputs.file(repoRoot.resolve("Cargo.toml"))
    outputs.file(jniArm64File)

    dependsOn("extractTesseractNativeLibs")

    workingDir(repoRoot)
    commandLine(repoRoot.resolve("scripts/build-android-rust.sh").absolutePath)
}

tasks.matching { it.name.startsWith("merge") && it.name.endsWith("JniLibFolders") }.configureEach {
    dependsOn(buildRustCore)
}

tasks.withType<Test> {
    val repoRoot = rootProject.projectDir.parentFile.parentFile
    val targetDebug = File(repoRoot, "target/debug").absolutePath
    systemProperty("jna.library.path", targetDebug)
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}
