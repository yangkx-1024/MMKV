import org.gradle.api.artifacts.Configuration
import java.io.FileInputStream
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val propFile = File(rootProject.rootDir, "local.properties")
if (propFile.exists()) {
    val prop = Properties().apply {
        FileInputStream(propFile).use {
            load(it)
        }
    }
    prop.forEach {
        ext.set(it.key as String, it.value as String)
    }
}

android {
    namespace = "net.yangkx.mmkv.demo"
    compileSdk = 34

    defaultConfig {
        applicationId = "net.yangkx.mmkv.demo"
        minSdk = 21
        targetSdk = 34
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
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        viewBinding = true
        compose = true
    }
    composeOptions {
        kotlinCompilerExtensionVersion = "1.4.3"
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
    implementation(Deps.kotlin)

    val composeBom = platform("androidx.compose:compose-bom:2023.01.00")
    implementation(composeBom)
    androidTestImplementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")

    implementation("androidx.appcompat:appcompat:1.6.1")
    defaultDebugImplementation(project(":library"))
    encryptionDebugImplementation(project(":library-encrypt"))
    defaultStagingImplementation(Deps.mmkv_snapshot)
    encryptionStagingImplementation(Deps.mmkv_encrypt_snapshot)
    defaultReleaseImplementation(Deps.mmkv)
    encryptionReleaseImplementation(Deps.mmkv_encrypt)

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
}