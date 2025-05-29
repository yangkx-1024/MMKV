pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // Enable snapshot repo
        maven("https://central.sonatype.com/repository/maven-snapshots/")
    }
}

rootProject.name = "MMKV"
include(":app")
include(":library")
include(":library-encrypt")
