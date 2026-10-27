import org.jetbrains.compose.desktop.application.dsl.TargetFormat

// Versions épinglées (vérifiées le 2026-10-26 sur Maven Central / plugins.gradle.org / Google Maven) :
// Compose Multiplatform 1.12.2 (dernière stable ; 1.13.0-beta01 = bêta), Kotlin 2.4.21, kotlinx-coroutines 1.11.0.
plugins {
    kotlin("jvm") version "2.4.21"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
    id("org.jetbrains.compose") version "1.12.2"
}

kotlin { jvmToolchain(25) }

dependencies {
    // Dépendances directes (les accesseurs compose.desktop.linux_x64 / compose.foundation sont dépréciés en 1.12) ;
    // pas de material : seuls ui (Window/application), foundation et le runtime natif Skiko (version alignée sur CMP 1.12.2).
    implementation("org.jetbrains.compose.ui:ui:1.12.2")
    implementation("org.jetbrains.compose.foundation:foundation:1.12.2")
    implementation("org.jetbrains.skiko:skiko-awt-runtime-linux-x64:0.150.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.11.0")
}

compose.desktop {
    application {
        mainClass = "bench.MainKt"
        // FFM (java.lang.foreign) final depuis JDK 22 : plus de --enable-preview.
        jvmArgs += listOf("-Dskiko.renderApi=SOFTWARE_FAST", "--enable-native-access=ALL-UNNAMED", "-Dsun.java2d.uiScale=1")
        javaHome = System.getenv("JAVA_HOME") ?: "/usr/lib/jvm/java-25-openjdk-amd64"
        nativeDistributions {
            targetFormats(TargetFormat.AppImage)
            packageName = "app"
            packageVersion = "1.0.0"
            // java.management : GarbageCollectorMXBean ; jdk.management : notifications GC (durée de chaque pause)
            modules("jdk.unsupported", "java.management", "jdk.management")
        }
    }
}
