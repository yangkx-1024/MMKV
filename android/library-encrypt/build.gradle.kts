import java.io.FileInputStream
import java.net.URI
import java.util.Properties

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    id("signing")
}

val prop = Properties().apply {
    load(FileInputStream(File(rootProject.rootDir, "local.properties")))
}
prop.forEach {
    ext[it.key as String] = it.value as String
}

android {
    namespace = "net.yangkx.mmkv"
    compileSdk = 33

    defaultConfig {
        minSdk = 24

        consumerProguardFiles("consumer-rules.pro")
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
    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.10.1")
}

publishing {
    val artifactId = "mmkv-encrypt"
    val version = "0.1.8"
    publications {
        register<MavenPublication>("release") {
            groupId = "net.yangkx"
            this.artifactId = artifactId
            this.version = version

            afterEvaluate {
                from(components["release"])
            }
            pom {
                name.set(artifactId)
                description.set("Library uses file-based mmap to store key-values")
                licenses {
                    license {
                        name.set("The Apache License, Version 2.0")
                        url.set("http://www.apache.org/licenses/LICENSE-2.0.txt")
                    }
                    license {
                        name.set("The MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }
                }
                developers {
                    developer {
                        name.set("Kexuan Yang")
                        email.set("kexuan.yang@gmail.com")
                    }
                }
                scm {
                    url.set("https://github.com/yangkx1024/MMKV")
                }
            }
        }
    }
    repositories {
        maven {
            val releasesRepoUrl = "https://s01.oss.sonatype.org/content/repositories/releases"
            val snapshotsRepoUrl = "https://s01.oss.sonatype.org/content/repositories/snapshots/"

            url = URI(
                if (version.endsWith("-SNAPSHOT")) {
                    snapshotsRepoUrl
                } else {
                    releasesRepoUrl
                }
            )
            credentials {
                username = prop["sonatypeUsername"] as String
                password = prop["sonatypePassword"] as String
            }
        }
    }
}

signing {
    sign(publishing.publications)
}
