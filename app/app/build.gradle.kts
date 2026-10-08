plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
    id("org.jetbrains.kotlin.plugin.serialization")
}

/** 读取仓库根目录 `.env`（app/ 的上一级），供默认服务器地址注入。 */
fun loadRootEnv(): Map<String, String> {
    val envFile = rootDir.parentFile.resolve(".env")
    if (!envFile.isFile) return emptyMap()
    val props = mutableMapOf<String, String>()
    envFile.readLines().forEach { raw ->
        val line = raw.trim()
        if (line.isEmpty() || line.startsWith("#")) return@forEach
        val idx = line.indexOf('=')
        if (idx <= 0) return@forEach
        val key = line.substring(0, idx).trim()
        var value = line.substring(idx + 1).trim()
        if ((value.startsWith("\"") && value.endsWith("\"")) ||
            (value.startsWith("'") && value.endsWith("'"))
        ) {
            value = value.substring(1, value.length - 1)
        }
        props[key] = value
    }
    return props
}

val rootEnv = loadRootEnv()
val defaultServerHost =
    rootEnv["app_server_host"]
        ?: rootEnv["ssh_server"]
        ?: "10.0.2.2"
val defaultHttpPort = rootEnv["app_http_port"] ?: "8000"
val defaultWsPort = rootEnv["app_ws_port"] ?: defaultHttpPort
val defaultUseTls = (rootEnv["app_use_tls"] ?: "false").lowercase() in setOf("1", "true", "yes")
val defaultWsPath = rootEnv["app_ws_path"] ?: "/ws"

android {
    namespace = "com.lingxi.app"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.lingxi.app"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"

        // 默认服务器来自仓库根目录 .env（见 app_server_host / ssh_server）
        buildConfigField("String", "DEFAULT_SERVER_HOST", "\"$defaultServerHost\"")
        buildConfigField("int", "DEFAULT_HTTP_PORT", defaultHttpPort)
        buildConfigField("int", "DEFAULT_WS_PORT", defaultWsPort)
        buildConfigField("boolean", "DEFAULT_USE_TLS", defaultUseTls.toString())
        buildConfigField("String", "DEFAULT_WS_PATH", "\"$defaultWsPath\"")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
        buildConfig = true
    }
}

println(
    "[lingxi] default server from .env: " +
        "$defaultServerHost:$defaultHttpPort (tls=$defaultUseTls)",
)

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.navigation:navigation-compose:2.8.5")
    implementation("androidx.datastore:datastore-preferences:1.1.1")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    // TODO: 接入真实后端时启用；当前 UI 走 MockRepository，尚未调用
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.squareup.retrofit2:retrofit:2.11.0")
    implementation("com.jakewharton.retrofit:retrofit2-kotlinx-serialization-converter:1.0.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")

    // 桌面图标角标（微信式未读数）
    implementation("me.leolin:ShortcutBadger:1.1.22@aar")
}
