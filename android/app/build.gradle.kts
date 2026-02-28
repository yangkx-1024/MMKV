import BuildUtil.loadProperties
import com.android.build.api.dsl.ApplicationExtension
import org.gradle.api.artifacts.Configuration

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

project.loadProperties()

configure<ApplicationExtension> {
    namespace = "net.yangkx.mmkv.demo"
    compileSdk = 36

    defaultConfig {
        applicationId = "net.yangkx.mmkv.demo"
        minSdk = 23
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    signingConfigs {
        create("release") {
            keyAlias = project.ext.get("SIGN_KEY_ALIAS") as String
            keyPassword = project.ext.get("SIGN_KEY_PASSWORD") as String
            storeFile = file(project.ext.get("SIGN_KEY_STORE_PATH") as String)
            storePassword = project.ext.get("SIGN_STORE_PASSWORD") as String
        }
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = signingConfigs.getByName("release")
        }
        create("staging") {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            signingConfig = signingConfigs.getByName("release")
        }
        debug {
            signingConfig = signingConfigs.getByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        viewBinding = true
        compose = true
    }
    flavorDimensions += "feature"
    productFlavors {
        create("default") {
            dimension = "feature"
        }
        create("encryption") {
            dimension = "feature"
        }
    }
}

val defaultDebugImplementation: Configuration by configurations.creating
val encryptionDebugImplementation: Configuration by configurations.creating
val defaultStagingImplementation: Configuration by configurations.creating
val encryptionStagingImplementation: Configuration by configurations.creating
val defaultReleaseImplementation: Configuration by configurations.creating
val encryptionReleaseImplementation: Configuration by configurations.creating

dependencies {
    implementation(Deps.KOTLIN)

    val composeBom = platform("androidx.compose:compose-bom:2026.02.01")
    implementation(composeBom)
    androidTestImplementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.appcompat:appcompat:1.7.1")
    defaultDebugImplementation(project(":library"))
    encryptionDebugImplementation(project(":library-encrypt"))
    defaultStagingImplementation(Deps.MMKV_SNAPSHOT)
    encryptionStagingImplementation(Deps.MMKV_ENCRYPT_SNAPSHOT)
    defaultReleaseImplementation(Deps.MMKV)
    encryptionReleaseImplementation(Deps.MMKV_ENCRYPT)

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.7.0")
}
//
//tasks.register("launchEmulator", Exec::class) {
//    workingDir = project.rootDir
//    commandLine = listOf("./start_android_emulator.sh")
//    environment["EMULATOR_NAME"] = "nexus"
//}
//
//if (System.getenv("CI")?.toBoolean() != true) {
//    tasks.register("killEmulator", Exec::class) {
//        workingDir = project.rootDir
//        commandLine = listOf("./kill_android_emulator.sh")
//    }
//
//    afterEvaluate {
//        tasks.findByName("connectedDefaultDebugAndroidTest")?.apply {
//            dependsOn(tasks.findByName("launchEmulator"))
//        }
//        tasks.findByName("connectedAndroidTest")?.apply {
//            finalizedBy(tasks.findByName("killEmulator"))
//        }
//    }
//}