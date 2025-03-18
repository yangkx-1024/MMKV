object Configuration {
    const val SNAP_SHOT_VERSION = "$version-SNAPSHOT"
    const val RELEASE_VERSION = version
    val libVersion: String
        get() = if (System.getenv("CI")?.toBoolean() == true){
            RELEASE_VERSION
        } else {
            SNAP_SHOT_VERSION
        }
    const val GROUP_ID = "net.yangkx"
    const val DESCRIPTION = "Library uses file-based mmap to store key-values"
    private const val RELEASES_REPO_URL = "https://s01.oss.sonatype.org/content/repositories/releases/"
    private const val SNAPSHOTS_REPO_URL = "https://s01.oss.sonatype.org/content/repositories/snapshots/"
    val publishUrl = if (System.getenv("CI")?.toBoolean() == true) {
        RELEASES_REPO_URL
    } else {
        SNAPSHOTS_REPO_URL
    }
    val licenceApache = "The Apache License, Version 2.0" to "http://www.apache.org/licenses/LICENSE-2.0.txt"
    val licenceMit = "The MIT License" to "https://opensource.org/licenses/MIT"
    val developer = "Kexuan Yang" to "kexuan.yang@gmail.com"
    const val SCM_URL = "https://github.com/yangkx-1024/MMKV"
}

object Deps {
    const val KOTLIN = "androidx.core:core-ktx:1.10.1"
    const val MMKV_SNAPSHOT = "net.yangkx:mmkv:${Configuration.SNAP_SHOT_VERSION}"
    const val MMKV_ENCRYPT_SNAPSHOT = "net.yangkx:mmkv-encrypt:${Configuration.SNAP_SHOT_VERSION}"
    const val MMKV = "net.yangkx:mmkv:${Configuration.RELEASE_VERSION}"
    const val MMKV_ENCRYPT = "net.yangkx:mmkv-encrypt:${Configuration.RELEASE_VERSION}"
}