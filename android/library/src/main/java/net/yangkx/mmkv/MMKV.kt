package net.yangkx.mmkv

object MMKV {
    init {
        System.loadLibrary("mmkv")
    }

    external fun initialize(dir: String)

    external fun putString(key: String, value: String)

    external fun putInt(key: String, value: Int)

    external fun putBool(key: String, value: Boolean)

    @Throws(NoSuchElementException::class)
    external fun getString(key: String): String

    @Throws(NoSuchElementException::class)
    external fun getInt(key: String): Int

    @Throws(NoSuchElementException::class)
    external fun getBool(key: String): Boolean

    external fun clearData()

    external fun close()
}