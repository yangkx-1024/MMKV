object Configuration {
    const val libVersion = "0.2.3-SNAPSHOT"
//    const val libVersion = "0.2.3"
    const val groupId = "net.yangkx"
    const val description = "Library uses file-based mmap to store key-values"
    const val releasesRepoUrl = "https://s01.oss.sonatype.org/content/repositories/releases/"
    const val snapshotsRepoUrl = "https://s01.oss.sonatype.org/content/repositories/snapshots/"
    val publishUrl = if (libVersion.endsWith("-SNAPSHOT")) {
        snapshotsRepoUrl
    } else {
        releasesRepoUrl
    }
    val licenceApache = "The Apache License, Version 2.0" to "http://www.apache.org/licenses/LICENSE-2.0.txt"
    val licenceMit = "The MIT License" to "https://opensource.org/licenses/MIT"
    val developer = "Kexuan Yang" to "kexuan.yang@gmail.com"
    const val scmUrl = "https://github.com/yangkx1024/MMKV"
}

object Deps {
    val kotlin = "androidx.core:core-ktx:1.10.1"
    val mmkv = "net.yangkx:mmkv:${Configuration.libVersion}"
    val mmkv_encrypt = "net.yangkx:mmkv-encrypt:${Configuration.libVersion}"
}