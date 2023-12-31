object Configuration {
    const val snapshotVersion = "$version-SNAPSHOT"
    const val releaseVersion = version
    val libVersion: String
        get() = if (System.getenv("CI")?.toBoolean() == true){
            releaseVersion
        } else {
            snapshotVersion
        }
    const val groupId = "net.yangkx"
    const val description = "Library uses file-based mmap to store key-values"
    private const val releasesRepoUrl = "https://s01.oss.sonatype.org/content/repositories/releases/"
    private const val snapshotsRepoUrl = "https://s01.oss.sonatype.org/content/repositories/snapshots/"
    val publishUrl = if (System.getenv("CI")?.toBoolean() == true) {
        releasesRepoUrl
    } else {
        snapshotsRepoUrl
    }
    val licenceApache = "The Apache License, Version 2.0" to "http://www.apache.org/licenses/LICENSE-2.0.txt"
    val licenceMit = "The MIT License" to "https://opensource.org/licenses/MIT"
    val developer = "Kexuan Yang" to "kexuan.yang@gmail.com"
    const val scmUrl = "https://github.com/yangkx-1024/MMKV"
}

object Deps {
    const val kotlin = "androidx.core:core-ktx:1.10.1"
    const val mmkv_snapshot = "net.yangkx:mmkv:${Configuration.snapshotVersion}"
    const val mmkv_encrypt_snapshot = "net.yangkx:mmkv-encrypt:${Configuration.snapshotVersion}"
    const val mmkv = "net.yangkx:mmkv:${Configuration.releaseVersion}"
    const val mmkv_encrypt = "net.yangkx:mmkv-encrypt:${Configuration.releaseVersion}"
}