plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "net.yangkx.mmkv"
    compileSdk = 33

    defaultConfig {
        applicationId = "net.yangkx.mmkv"
        minSdk = 24
        targetSdk = 33
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
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

dependencies {
    implementation("androidx.core:core-ktx:1.10.1")
    implementation("androidx.appcompat:appcompat:1.6.1")
    "defaultImplementation"(project(":library"))
    "encryptionImplementation"(project(":library-encrypt"))
//    "defaultImplementation"("net.yangkx:mmkv:0.1.7")
//    "encryptionImplementation"("net.yangkx:mmkv-encrypt:0.1.7")
}