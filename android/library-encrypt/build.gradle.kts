import BuildUtil.loadProperties
import java.net.URI

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    id("signing")
}

project.loadProperties()

android {
    namespace = "net.yangkx.mmkv"
    compileSdk = 35

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
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

dependencies {
    implementation(Deps.KOTLIN)
}

publishing {
    val artifactId = "mmkv-encrypt"
    publications {
        register<MavenPublication>("release") {
            groupId = Configuration.GROUP_ID
            this.artifactId = artifactId
            this.version = Configuration.libVersion

            afterEvaluate {
                from(components["release"])
            }
            pom {
                name.set(artifactId)
                description.set(Configuration.DESCRIPTION)
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
                    url.set(Configuration.SCM_URL)
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
                username = project.ext.get("sonatypeUsername") as String?
                password = project.ext.get("sonatypePassword") as String?
            }
        }
    }
}

signing {
    sign(publishing.publications)
}
