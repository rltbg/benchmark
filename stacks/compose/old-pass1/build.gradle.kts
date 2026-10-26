import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    kotlin("jvm") version "2.4.21"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
    id("org.jetbrains.compose") version "1.12.2"
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(compose.desktop.linux_x64)
    implementation(compose.foundation)
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.10.2")
}

compose.desktop {
    application {
        mainClass = "bench.MainKt"
        jvmArgs += listOf("-Dskiko.renderApi=SOFTWARE_FAST", "--enable-preview", "--enable-native-access=ALL-UNNAMED")
        nativeDistributions {
            targetFormats(TargetFormat.AppImage)
            packageName = "app"
            packageVersion = "1.0.0"
            modules("jdk.unsupported", "java.management")
        }
    }
}

