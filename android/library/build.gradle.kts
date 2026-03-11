import BuildUtil.loadProperties
import BuildUtil.isCentralSnapshotsPublish
import com.android.build.api.dsl.LibraryExtension

plugins {
    id("com.android.library")
    id("maven-publish")
    id("signing")
    id("com.gradleup.nmcp")
    id("com.gradleup.nmcp.aggregation")
}

loadProperties()

val publicationVersion = if (isCentralSnapshotsPublish()) {
    Configuration.SNAPSHOT_VERSION
} else {
    Configuration.RELEASE_VERSION
}

configure<LibraryExtension> {
    namespace = "net.yangkx.mmkv"
    compileSdk = 36

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
    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

group = Configuration.GROUP_ID
version = publicationVersion

publishing {
    val artifactId = "mmkv"
    publications {
        register<MavenPublication>("release") {
            groupId = Configuration.GROUP_ID
            this.artifactId = artifactId
            this.version = publicationVersion

            afterEvaluate {
                from(components["release"])
            }
            pom {
                name.set(artifactId)
                description.set(Configuration.DESCRIPTION)
                url.set(Configuration.PROJECT_URL)
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
                    connection.set(Configuration.SCM_CONNECTION)
                    developerConnection.set(Configuration.SCM_DEVELOPER_CONNECTION)
                }
            }
        }
    }
}

signing {
    sign(publishing.publications)
}

nmcpAggregation {
    centralPortal {
        username.set(project.ext.get("sonatypeUsername")?.toString() ?: System.getenv("SONATYPEUSERNAME") ?: "")
        password.set(project.ext.get("sonatypePassword")?.toString() ?: System.getenv("SONATYPEPASSWORD") ?: "")
        publishingType = "AUTOMATIC"
    }
}

dependencies {
    implementation(Deps.KOTLIN)
    nmcpAggregation(project)
}
