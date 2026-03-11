object Configuration {
    const val SNAPSHOT_VERSION = "$version-SNAPSHOT"
    const val RELEASE_VERSION = version
    const val GROUP_ID = "net.yangkx"
    const val DESCRIPTION = "Library uses file-based mmap to store key-values"
    const val PROJECT_URL = "https://github.com/yangkx-1024/MMKV"
    val licenceApache = "The Apache License, Version 2.0" to "http://www.apache.org/licenses/LICENSE-2.0.txt"
    val licenceMit = "The MIT License" to "https://opensource.org/licenses/MIT"
    val developer = "Kexuan Yang" to "kexuan.yang@gmail.com"
    const val SCM_URL = PROJECT_URL
    const val SCM_CONNECTION = "scm:git:https://github.com/yangkx-1024/MMKV.git"
    const val SCM_DEVELOPER_CONNECTION = "scm:git:ssh://git@github.com/yangkx-1024/MMKV.git"
}

object Deps {
    const val KOTLIN = "androidx.core:core-ktx:1.17.0"
    const val MMKV_SNAPSHOT = "net.yangkx:mmkv:${Configuration.SNAPSHOT_VERSION}"
    const val MMKV_ENCRYPT_SNAPSHOT = "net.yangkx:mmkv-encrypt:${Configuration.SNAPSHOT_VERSION}"
    const val MMKV = "net.yangkx:mmkv:${Configuration.RELEASE_VERSION}"
    const val MMKV_ENCRYPT = "net.yangkx:mmkv-encrypt:${Configuration.RELEASE_VERSION}"
}
