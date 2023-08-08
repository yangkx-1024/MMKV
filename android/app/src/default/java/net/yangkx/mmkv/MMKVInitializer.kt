package net.yangkx.mmkv

import android.content.Context

object MMKVInitializer {
    fun init(context: Context) {
        val dir = context.getDir("mmkv", Context.MODE_PRIVATE)
        MMKV.initialize(dir.absolutePath)
    }
}