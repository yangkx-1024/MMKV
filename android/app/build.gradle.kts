import org.gradle.api.artifacts.Configuration
import java.io.FileInputStream
import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

val prop = Properties().apply {
    load(FileInputStream(File(rootProject.rootDir, "local.properties")))
}

android {
    namespace = "net.yangkx.mmkv"
    compileSdk = 33

    defaultConfig {
        applicationId = "net.yangkx.mmkv"
        minSdk = 21
        targetSdk = 33
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    signingConfigs {
        create("release") {
            keyAlias = prop.getProperty("keyAlias")
            keyPassword = prop.getProperty("keyPassword")
            storeFile = file(prop.getProperty("storeFile"))
            storePassword = prop.getProperty("storePassword")
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
val defaultReleaseImplementation: Configuration by configurations.creating
val encryptionReleaseImplementation: Configuration by configurations.creating

dependencies {
    implementation(Deps.kotlin)
    implementation("androidx.appcompat:appcompat:1.6.1")
    defaultDebugImplementation(project(":library"))
    encryptionDebugImplementation(project(":library-encrypt"))
    defaultReleaseImplementation(Deps.mmkv)
    encryptionReleaseImplementation(Deps.mmkv_encrypt)
}