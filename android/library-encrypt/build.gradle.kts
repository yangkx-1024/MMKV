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
        minSdk = 21

        consumerProguardFiles("consumer-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = true
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
    implementation(Deps.kotlin)
}

publishing {
    val artifactId = "mmkv-encrypt"
    publications {
        register<MavenPublication>("release") {
            groupId = "net.yangkx"
            this.artifactId = artifactId
            this.version = Configuration.libVersion

            afterEvaluate {
                from(components["release"])
            }
            pom {
                name.set(artifactId)
                description.set(Configuration.description)
                licenses {
                    license {
                        name.set(Configuration.licenceApache.first)
                        url.set(Configuration.licenceApache.second)
                    }
                    license {
                        name.set(Configuration.licenceMit.first)
                        url.set(Configuration.licenceMit.second)
                    }
                }
                developers {
                    developer {
                        name.set(Configuration.developer.first)
                        email.set(Configuration.developer.second)
                    }
                }
                scm {
                    url.set(Configuration.scmUrl)
                }
            }
        }
    }
    repositories {
        maven {
            url = URI(
                Configuration.publishUrl
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
